import sqlite3
import os
from datetime import datetime
from typing import Dict, Any, List, Optional

DB_PATH = os.path.join(os.path.dirname(__file__), "moneymax.db")

class MoneyMaxDBService:
    def __init__(self, db_path: str = DB_PATH):
        self.db_path = db_path

    def _get_connection(self):
        conn = sqlite3.connect(self.db_path)
        conn.row_factory = sqlite3.Row
        conn.execute("PRAGMA foreign_keys = ON;")
        return conn

    def get_customer_profile(self, identifier: str) -> Optional[Dict[str, Any]]:
        query = """
            SELECT c.customer_id, c.full_name, c.email, c.phone, c.pan_number, c.aadhaar_number,
                   c.date_of_birth, c.address, c.city, c.state, c.pincode,
                   a.account_id, a.account_number, a.ifsc_code, a.upi_vpa, a.balance,
                   a.credit_limit, a.available_credit, a.cibil_score, a.kyc_status, a.risk_tier, a.is_frozen
            FROM customers c
            JOIN accounts a ON c.customer_id = a.customer_id
            WHERE a.account_id = ? OR c.customer_id = ? OR c.phone = ? OR c.pan_number = ? OR a.upi_vpa = ?
        """
        with self._get_connection() as conn:
            cursor = conn.cursor()
            cursor.execute(query, (identifier, identifier, identifier, identifier, identifier))
            row = cursor.fetchone()
            if not row:
                return None
            return dict(row)

    def get_transaction_history(self, account_id: str, limit: int = 5) -> List[Dict[str, Any]]:
        query = """
            SELECT txn_id, account_id, txn_type, amount, direction, counterparty, status, utr_number, notes, created_at
            FROM transactions
            WHERE account_id = ?
            ORDER BY created_at DESC
            LIMIT ?
        """
        with self._get_connection() as conn:
            cursor = conn.cursor()
            cursor.execute(query, (account_id, limit))
            rows = cursor.fetchall()
            return [dict(r) for r in rows]

    def get_disputes(self, account_id: str) -> List[Dict[str, Any]]:
        query = """
            SELECT dispute_id, account_id, txn_id, dispute_type, claimed_amount, status, customer_statement, agent_resolution_notes, created_at, resolved_at
            FROM disputes
            WHERE account_id = ?
            ORDER BY created_at DESC
        """
        with self._get_connection() as conn:
            cursor = conn.cursor()
            cursor.execute(query, (account_id,))
            rows = cursor.fetchall()
            return [dict(r) for r in rows]

    def execute_refund(self, account_id: str, txn_id: str, amount: float, reason: str, actor: str = "FINBOT_AGENT") -> Dict[str, Any]:
        with self._get_connection() as conn:
            cursor = conn.cursor()
            cursor.execute("SELECT balance, is_frozen FROM accounts WHERE account_id = ?", (account_id,))
            acc = cursor.fetchone()
            if not acc:
                return {"success": False, "error": f"Account {account_id} not found."}
            if acc["is_frozen"] == 1:
                return {"success": False, "error": f"Account {account_id} is frozen. Transactions prohibited."}

            new_balance = round(acc["balance"] + amount, 2)
            cursor.execute("UPDATE accounts SET balance = ? WHERE account_id = ?", (new_balance, account_id))

            refund_txn_id = f"TXN-REF-{int(datetime.now().timestamp()*1000) % 10000000}"
            utr = f"REF{int(datetime.now().timestamp()*1000) % 1000000000}"
            now_str = datetime.now().strftime("%Y-%m-%d %H:%M:%S")

            cursor.execute("""
                INSERT INTO transactions (txn_id, account_id, txn_type, amount, direction, counterparty, status, utr_number, notes, created_at)
                VALUES (?, ?, 'REFUND', ?, 'CREDIT', 'MoneyMax Auto-Refund Gateway', 'SUCCESS', ?, ?, ?)
            """, (refund_txn_id, account_id, amount, utr, f"Refund for {txn_id}: {reason}", now_str))

            cursor.execute("""
                UPDATE disputes
                SET status = 'RESOLVED_REFUNDED', agent_resolution_notes = ?, resolved_at = ?
                WHERE account_id = ? AND txn_id = ?
            """, (f"Refunded INR {amount} via {actor}: {reason}", now_str, account_id, txn_id))

            cursor.execute("""
                INSERT INTO audit_logs (timestamp, actor, action, target_account_id, details, status)
                VALUES (?, ?, 'ISSUE_REFUND', ?, ?, 'SUCCESS')
            """, (now_str, actor, account_id, f"Refunded INR {amount} for txn {txn_id}. New balance: INR {new_balance}"))

            conn.commit()
            return {
                "success": True,
                "refund_txn_id": refund_txn_id,
                "account_id": account_id,
                "refunded_amount": amount,
                "new_balance": new_balance,
                "timestamp": now_str
            }

    def revert_unauthorized_payout(self, account_id: str, amount: float, reason: str) -> Dict[str, Any]:
        """Reverts an unauthorized payout safely without violating DB check constraints."""
        with self._get_connection() as conn:
            cursor = conn.cursor()
            cursor.execute("SELECT balance FROM accounts WHERE account_id = ?", (account_id,))
            acc = cursor.fetchone()
            if not acc:
                return {"success": False, "error": "Account not found"}

            new_bal = round(max(0.0, acc["balance"] - amount), 2)
            cursor.execute("UPDATE accounts SET balance = ? WHERE account_id = ?", (new_bal, account_id))

            now_str = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
            cursor.execute("""
                INSERT INTO audit_logs (timestamp, actor, action, target_account_id, details, status)
                VALUES (?, 'AEGISMESH_FIREWALL', 'REVERT_ROGUE_PAYOUT', ?, ?, 'SUCCESS')
            """, (now_str, account_id, f"Reverted unauthorized INR {amount}: {reason}"))
            conn.commit()
            return {"success": True, "account_id": account_id, "new_balance": new_bal}

    def update_kyc_status(self, account_id: str, new_status: str, notes: str, actor: str = "FINBOT_AGENT") -> Dict[str, Any]:
        valid_statuses = ("VERIFIED", "PENDING_REVIEW", "FLAGGED", "REJECTED")
        if new_status not in valid_statuses:
            return {"success": False, "error": f"Invalid status: {new_status}"}

        with self._get_connection() as conn:
            cursor = conn.cursor()
            cursor.execute("UPDATE accounts SET kyc_status = ? WHERE account_id = ?", (new_status, account_id))
            now_str = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
            cursor.execute("""
                INSERT INTO audit_logs (timestamp, actor, action, target_account_id, details, status)
                VALUES (?, ?, 'UPDATE_KYC', ?, ?, 'SUCCESS')
            """, (now_str, actor, account_id, f"KYC updated to {new_status}. Notes: {notes}"))
            conn.commit()
            return {"success": True, "account_id": account_id, "new_kyc_status": new_status}

    def update_credit_limit(self, account_id: str, new_limit: float, reason: str, actor: str = "FINBOT_AGENT") -> Dict[str, Any]:
        with self._get_connection() as conn:
            cursor = conn.cursor()
            cursor.execute("UPDATE accounts SET credit_limit = ?, available_credit = ? WHERE account_id = ?", (new_limit, new_limit, account_id))
            now_str = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
            cursor.execute("""
                INSERT INTO audit_logs (timestamp, actor, action, target_account_id, details, status)
                VALUES (?, ?, 'ADJUST_CREDIT_LIMIT', ?, ?, 'SUCCESS')
            """, (now_str, actor, account_id, f"Credit limit set to INR {new_limit}. Reason: {reason}"))
            conn.commit()
            return {"success": True, "account_id": account_id, "new_credit_limit": new_limit}
