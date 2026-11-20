use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::Local;
use parking_lot::Mutex;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::DbError;

const MONEYMAX_SCHEMA_SQL: &str = include_str!("../../../../database/schema.sql");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomerSummary {
    pub customer_id: String,
    pub full_name: String,
    pub account_id: String,
    pub upi_vpa: String,
    pub balance: f64,
    pub credit_limit: f64,
    pub cibil_score: i64,
    pub kyc_status: String,
    pub risk_tier: String,
    pub is_frozen: bool,
    pub open_disputes_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomerProfile {
    pub customer_id: String,
    pub full_name: String,
    pub email: String,
    pub phone: String,
    pub pan_number: String,
    pub aadhaar_number: String,
    pub date_of_birth: String,
    pub address: String,
    pub city: String,
    pub state: String,
    pub pincode: String,
    pub account_id: String,
    pub account_number: String,
    pub ifsc_code: String,
    pub upi_vpa: String,
    pub balance: f64,
    pub credit_limit: f64,
    pub available_credit: f64,
    pub cibil_score: i64,
    pub kyc_status: String,
    pub risk_tier: String,
    pub is_frozen: bool,
}

impl CustomerProfile {
    /// Returns a DPDP-minimized copy safe for default customer/session display.
    pub fn masked(&self) -> Self {
        let mask_tail = |val: &str, prefix: &str, keep: usize| -> String {
            let trimmed = val.trim();
            if trimmed.len() <= keep {
                format!("{}{}", prefix, trimmed)
            } else {
                format!("{}{}", prefix, &trimmed[trimmed.len() - keep..])
            }
        };
        let email_masked = match self.email.split_once('@') {
            Some((local, dom)) => {
                let first = local.chars().next().unwrap_or('*');
                format!("{}***@{}", first, dom)
            }
            None => "***".to_string(),
        };

        let mut out = self.clone();
        out.pan_number = mask_tail(&self.pan_number, "XXXXXX", 4);
        out.aadhaar_number = mask_tail(&self.aadhaar_number, "XXXX XXXX ", 4);
        out.phone = mask_tail(&self.phone, "+91 XXXXX X", 4);
        out.email = email_masked;
        out.account_number = mask_tail(&self.account_number, "XXXXXXXXX", 4);
        out
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveAccountContext {
    pub account_id: String,
    pub customer_name: String,
    pub balance: f64,
    pub credit_limit: f64,
    pub available_credit: f64,
    pub cibil_score: i64,
    pub kyc_status: String,
    pub risk_tier: String,
    pub is_frozen: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionRecord {
    pub txn_id: String,
    pub account_id: String,
    pub txn_type: String,
    pub amount: f64,
    pub direction: String,
    pub counterparty: String,
    pub status: String,
    pub utr_number: String,
    pub notes: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisputeRecord {
    pub dispute_id: String,
    pub account_id: String,
    pub txn_id: String,
    pub dispute_type: String,
    pub claimed_amount: f64,
    pub status: String,
    pub customer_statement: String,
    pub agent_resolution_notes: Option<String>,
    pub created_at: String,
    pub resolved_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbAuditLog {
    pub log_id: i64,
    pub timestamp: String,
    pub actor: String,
    pub action: String,
    pub target_account_id: Option<String>,
    pub details: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefundOutcome {
    pub success: bool,
    pub refund_txn_id: String,
    pub account_id: String,
    pub refunded_amount: f64,
    pub new_balance: f64,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevertOutcome {
    pub success: bool,
    pub account_id: String,
    pub reverted_amount: f64,
    pub new_balance: f64,
    pub timestamp: String,
}

#[derive(Clone)]
pub struct MoneyMaxDb {
    path: PathBuf,
    conn: Arc<Mutex<Connection>>,
}

impl MoneyMaxDb {
    pub fn open_or_bootstrap(path: impl AsRef<Path>) -> Result<Self, DbError> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)?;
            }
        }
        let needs_bootstrap = !path.exists();
        let conn = Connection::open(&path)?;
        conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             PRAGMA journal_mode = WAL;
             PRAGMA busy_timeout = 5000;",
        )?;

        let db = Self {
            path,
            conn: Arc::new(Mutex::new(conn)),
        };

        if needs_bootstrap || db.customer_count()? == 0 {
            db.bootstrap_schema_and_seed()?;
        }

        Ok(db)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn customer_count(&self) -> Result<i64, DbError> {
        let conn = self.conn.lock();
        let exists: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='customers'",
            [],
            |row| row.get(0),
        )?;
        if exists == 0 {
            return Ok(0);
        }
        let cnt: i64 = conn.query_row("SELECT COUNT(*) FROM customers", [], |row| row.get(0))?;
        Ok(cnt)
    }

    fn bootstrap_schema_and_seed(&self) -> Result<(), DbError> {
        let mut conn = self.conn.lock();
        conn.execute_batch(MONEYMAX_SCHEMA_SQL)?;
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM customers", [], |r| r.get(0))?;
        if count > 0 {
            return Ok(());
        }

        let tx = conn.transaction()?;
        let seed_accounts = [
            ("MMX-CUST-1001", "Sneha Chikane", "sneha.chikane1@example.in", "+919846913810", "FDCPC9935C", "8132 1488 2535", "1994-06-14", "Flat 102, Koramangala, Bengaluru", "Bengaluru", "Karnataka", "560001", "MMX-AC-5001", "5010000000193", "sneha.chikane90@moneymax", 12450.0, 75000.0, 58000.0, 780_i64, "VERIFIED", "LOW", 0_i64),
            ("MMX-CUST-1002", "Kavita Roy", "kavita.roy2@example.in", "+919791756179", "RNFPR8573N", "6755 4598 6313", "1991-11-03", "Flat 408, Bandra West, Mumbai", "Mumbai", "Maharashtra", "400001", "MMX-AC-5002", "5010000000244", "kavita.roy9@moneymax", 4320.0, 50000.0, 32000.0, 690, "VERIFIED", "MEDIUM", 0),
            ("MMX-CUST-1003", "Aniket Chawla", "aniket.chawla3@example.in", "+916319289546", "ZNDPC8787T", "4371 9645 7932", "1989-02-19", "Flat 712, Civil Lines, New Delhi", "New Delhi", "Delhi", "110001", "MMX-AC-5003", "5010000000349", "aniket.chawla52@moneymax", 189400.0, 250000.0, 210000.0, 825, "VERIFIED", "LOW", 0),
            ("MMX-CUST-1004", "Yash Singh", "yash.singh4@example.in", "+918832321899", "KMWPS1960F", "8102 7396 5345", "1998-09-21", "Flat 205, Ring Road, Jaipur", "Jaipur", "Rajasthan", "302001", "MMX-AC-5004", "5010000000494", "yash.singh92@moneymax", 150.0, 20000.0, 1200.0, 480, "FLAGGED", "HIGH", 1),
            ("MMX-CUST-1005", "Ananya Srivastava", "ananya.srivastava5@example.in", "+918859597086", "GLPPS5956V", "2783 5905 2697", "1996-04-28", "Flat 501, MG Road, Pune", "Pune", "Maharashtra", "411001", "MMX-AC-5005", "5010000000529", "ananya.srivastava35@moneymax", 5600.0, 30000.0, 24000.0, 610, "PENDING_REVIEW", "MEDIUM", 0),
            ("MMX-CUST-1006", "Bhavna Sharma", "bhavna.sharma6@example.in", "+919965337219", "ATHPS7745X", "5373 3608 2771", "1992-07-08", "Flat 309, Brigade Road, Bengaluru", "Bengaluru", "Karnataka", "560001", "MMX-AC-5006", "5010000000654", "bhavna.sharma40@moneymax", 381011.61, 75000.0, 60000.0, 662, "PENDING_REVIEW", "MEDIUM", 0),
            ("MMX-CUST-1007", "Sunil Kapoor", "sunil.kapoor7@example.in", "+916369295515", "LFMPK3780X", "3390 9445 6491", "1985-12-12", "Flat 88, Park Street, Kolkata", "Kolkata", "West Bengal", "700001", "MMX-AC-5007", "5010000000728", "sunil.kapoor4@moneymax", 418971.27, 100000.0, 92000.0, 814, "VERIFIED", "LOW", 0),
            ("MMX-CUST-1008", "Pooja Patel", "pooja.patel8@example.in", "+918875529051", "FLAPP6507X", "8841 3704 8657", "1995-03-17", "Flat 14, CG Road, Ahmedabad", "Ahmedabad", "Gujarat", "380001", "MMX-AC-5008", "5010000000882", "pooja.patel85@moneymax", 378.73, 75000.0, 50000.0, 714, "VERIFIED", "MEDIUM", 0),
            ("MMX-CUST-1009", "Rajesh Chikane", "rajesh.chikane9@example.in", "+917958612055", "RVSPC8135F", "6730 9817 8921", "1982-08-05", "Flat 602, Jubilee Hills, Hyderabad", "Hyderabad", "Telangana", "500001", "MMX-AC-5009", "5010000000941", "rajesh.chikane12@moneymax", 349447.38, 75000.0, 15000.0, 534, "VERIFIED", "HIGH", 1),
            ("MMX-CUST-1010", "Varun Deshmukh", "varun.deshmukh10@example.in", "+916323769080", "WPMPD8216L", "4152 5136 8827", "1993-10-30", "Flat 22, Anna Salai, Chennai", "Chennai", "Tamil Nadu", "600001", "MMX-AC-5010", "5010000001097", "varun.deshmukh7@moneymax", 29334.26, 25000.0, 21000.0, 805, "VERIFIED", "LOW", 0),
        ];

        for row in seed_accounts {
            tx.execute(
                "INSERT INTO customers (customer_id, full_name, email, phone, pan_number, aadhaar_number, date_of_birth, address, city, state, pincode)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![row.0, row.1, row.2, row.3, row.4, row.5, row.6, row.7, row.8, row.9, row.10],
            )?;
            tx.execute(
                "INSERT INTO accounts (account_id, customer_id, account_number, ifsc_code, upi_vpa, balance, credit_limit, available_credit, cibil_score, kyc_status, risk_tier, is_frozen)
                 VALUES (?1, ?2, ?3, 'MMXB0001024', ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![row.11, row.0, row.12, row.13, row.14, row.15, row.16, row.17, row.18, row.19, row.20],
            )?;
        }

        let seed_txns = [
            ("TXN-MMX-100010", "MMX-AC-5001", "MERCHANT_PAYMENT", 450.00, "DEBIT", "swiggy@icici", "DISPUTED", "420100000010", "Double-debit on Swiggy order #SW-88129; delivery failed."),
            ("TXN-MMX-100011", "MMX-AC-5001", "BILL_PAY", 1299.00, "DEBIT", "airtelrecharge@paytm", "SUCCESS", "420100000011", "Payment to Airtel Postpaid"),
            ("TXN-MMX-100020", "MMX-AC-5002", "MERCHANT_PAYMENT", 25000.00, "DEBIT", "croma@hdfcbank", "DISPUTED", "420100000020", "High-value disputed merchant transaction pending chargeback."),
            ("TXN-MMX-100021", "MMX-AC-5002", "UPI_TRANSFER", 1200.00, "DEBIT", "zomato@hdfcbank", "SUCCESS", "420100000021", "Payment to Zomato Orders"),
            ("TXN-MMX-100030", "MMX-AC-5003", "MERCHANT_PAYMENT", 3499.00, "DEBIT", "amazonpay@icici", "SUCCESS", "420100000030", "Payment to Amazon Pay India"),
            ("TXN-MMX-100040", "MMX-AC-5004", "UPI_TRANSFER", 9500.00, "DEBIT", "unknown@upi", "FAILED", "420100000040", "Flagged outbound UPI transfer"),
            ("TXN-MMX-100050", "MMX-AC-5005", "BILL_PAY", 2150.00, "DEBIT", "tatapower@billdesk", "SUCCESS", "420100000050", "Payment to Tata Power Bill"),
        ];
        for t in seed_txns {
            tx.execute(
                "INSERT INTO transactions (txn_id, account_id, txn_type, amount, direction, counterparty, status, utr_number, notes)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![t.0, t.1, t.2, t.3, t.4, t.5, t.6, t.7, t.8],
            )?;
        }

        tx.execute(
            "INSERT INTO disputes (dispute_id, account_id, txn_id, dispute_type, claimed_amount, status, customer_statement)
             VALUES ('DSP-2026-0001', 'MMX-AC-5001', 'TXN-MMX-100010', 'DOUBLE_DEBIT', 450.00, 'OPEN', 'I was charged twice ₹450 for my Swiggy lunch order. Please refund the duplicate debit.')",
            [],
        )?;
        tx.execute(
            "INSERT INTO disputes (dispute_id, account_id, txn_id, dispute_type, claimed_amount, status, customer_statement)
             VALUES ('DSP-2026-0002', 'MMX-AC-5002', 'TXN-MMX-100020', 'FRAUDULENT_TXN', 25000.00, 'OPEN', 'I did not authorize the ₹25,000 charge at Croma. Reverse this immediately to my account!')",
            [],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn list_customers(&self) -> Result<Vec<CustomerSummary>, DbError> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT c.customer_id, c.full_name, a.account_id, a.upi_vpa, a.balance, a.credit_limit,
                    a.cibil_score, a.kyc_status, a.risk_tier, a.is_frozen,
                    (SELECT COUNT(*) FROM disputes d WHERE d.account_id = a.account_id AND d.status = 'OPEN') as open_disputes
             FROM customers c
             JOIN accounts a ON c.customer_id = a.customer_id
             ORDER BY a.account_id ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(CustomerSummary {
                customer_id: row.get(0)?,
                full_name: row.get(1)?,
                account_id: row.get(2)?,
                upi_vpa: row.get(3)?,
                balance: row.get(4)?,
                credit_limit: row.get(5)?,
                cibil_score: row.get(6)?,
                kyc_status: row.get(7)?,
                risk_tier: row.get(8)?,
                is_frozen: row.get::<_, i64>(9)? != 0,
                open_disputes_count: row.get(10)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn get_customer_profile(&self, identifier: &str) -> Result<Option<CustomerProfile>, DbError> {
        let id = identifier.trim();
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT c.customer_id, c.full_name, c.email, c.phone, c.pan_number, c.aadhaar_number,
                    c.date_of_birth, c.address, c.city, c.state, c.pincode,
                    a.account_id, a.account_number, a.ifsc_code, a.upi_vpa, a.balance,
                    a.credit_limit, a.available_credit, a.cibil_score, a.kyc_status, a.risk_tier, a.is_frozen
             FROM customers c
             JOIN accounts a ON c.customer_id = a.customer_id
             WHERE a.account_id = ?1 COLLATE NOCASE
                OR c.customer_id = ?1 COLLATE NOCASE
                OR c.phone = ?1
                OR c.pan_number = ?1 COLLATE NOCASE
                OR a.upi_vpa = ?1 COLLATE NOCASE
                OR c.full_name = ?1 COLLATE NOCASE
             LIMIT 1",
        )?;
        let res = stmt
            .query_row(params![id], |row| {
                Ok(CustomerProfile {
                    customer_id: row.get(0)?,
                    full_name: row.get(1)?,
                    email: row.get(2)?,
                    phone: row.get(3)?,
                    pan_number: row.get(4)?,
                    aadhaar_number: row.get(5)?,
                    date_of_birth: row.get(6)?,
                    address: row.get(7)?,
                    city: row.get(8)?,
                    state: row.get(9)?,
                    pincode: row.get(10)?,
                    account_id: row.get(11)?,
                    account_number: row.get(12)?,
                    ifsc_code: row.get(13)?,
                    upi_vpa: row.get(14)?,
                    balance: row.get(15)?,
                    credit_limit: row.get(16)?,
                    available_credit: row.get(17)?,
                    cibil_score: row.get(18)?,
                    kyc_status: row.get(19)?,
                    risk_tier: row.get(20)?,
                    is_frozen: row.get::<_, i64>(21)? != 0,
                })
            })
            .optional()?;
        Ok(res)
    }

    pub fn get_account_context(&self, account_id: &str) -> Result<Option<LiveAccountContext>, DbError> {
        let conn = self.conn.lock();
        let res = conn
            .query_row(
                "SELECT a.account_id, c.full_name, a.balance, a.credit_limit, a.available_credit,
                        a.cibil_score, a.kyc_status, a.risk_tier, a.is_frozen
                 FROM accounts a
                 JOIN customers c ON a.customer_id = c.customer_id
                 WHERE a.account_id = ?1 COLLATE NOCASE",
                params![account_id.trim()],
                |row| {
                    Ok(LiveAccountContext {
                        account_id: row.get(0)?,
                        customer_name: row.get(1)?,
                        balance: row.get(2)?,
                        credit_limit: row.get(3)?,
                        available_credit: row.get(4)?,
                        cibil_score: row.get(5)?,
                        kyc_status: row.get(6)?,
                        risk_tier: row.get(7)?,
                        is_frozen: row.get::<_, i64>(8)? != 0,
                    })
                },
            )
            .optional()?;
        Ok(res)
    }

    pub fn get_transaction_history(
        &self,
        account_id: &str,
        limit: usize,
    ) -> Result<Vec<TransactionRecord>, DbError> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT txn_id, account_id, txn_type, amount, direction, counterparty, status, utr_number, notes, created_at
             FROM transactions
             WHERE account_id = ?1 COLLATE NOCASE
             ORDER BY created_at DESC, rowid DESC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![account_id.trim(), limit as i64], |row| {
            Ok(TransactionRecord {
                txn_id: row.get(0)?,
                account_id: row.get(1)?,
                txn_type: row.get(2)?,
                amount: row.get(3)?,
                direction: row.get(4)?,
                counterparty: row.get(5)?,
                status: row.get(6)?,
                utr_number: row.get(7)?,
                notes: row.get(8)?,
                created_at: row.get(9)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn get_disputes(&self, account_id: &str) -> Result<Vec<DisputeRecord>, DbError> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT dispute_id, account_id, txn_id, dispute_type, claimed_amount, status,
                    customer_statement, agent_resolution_notes, created_at, resolved_at
             FROM disputes
             WHERE account_id = ?1 COLLATE NOCASE
             ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map(params![account_id.trim()], |row| {
            Ok(DisputeRecord {
                dispute_id: row.get(0)?,
                account_id: row.get(1)?,
                txn_id: row.get(2)?,
                dispute_type: row.get(3)?,
                claimed_amount: row.get(4)?,
                status: row.get(5)?,
                customer_statement: row.get(6)?,
                agent_resolution_notes: row.get(7)?,
                created_at: row.get(8)?,
                resolved_at: row.get(9)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn execute_refund(
        &self,
        account_id: &str,
        txn_id: &str,
        amount: f64,
        reason: &str,
        actor: &str,
        allow_frozen_override: bool,
    ) -> Result<RefundOutcome, DbError> {
        if !amount.is_finite() || amount <= 0.0 {
            return Err(DbError::BusinessRule(
                "Refund amount must be a positive finite number.".to_string(),
            ));
        }

        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;

        let row: Option<(f64, i64, String)> = tx
            .query_row(
                "SELECT balance, is_frozen, account_id FROM accounts WHERE account_id = ?1 COLLATE NOCASE",
                params![account_id.trim()],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;

        let (balance, is_frozen, canonical_acc_id) = match row {
            Some(r) => r,
            None => return Err(DbError::NotFound(format!("Account {} not found", account_id))),
        };

        if is_frozen != 0 && !allow_frozen_override {
            return Err(DbError::BusinessRule(format!(
                "Account {} is frozen. Transactions prohibited.",
                canonical_acc_id
            )));
        }

        let new_balance = ((balance + amount) * 100.0).round() / 100.0;
        tx.execute(
            "UPDATE accounts SET balance = ?1 WHERE account_id = ?2",
            params![new_balance, canonical_acc_id],
        )?;

        let uid = Uuid::new_v4().as_simple().to_string().to_uppercase();
        let refund_txn_id = format!("TXN-REF-{}", &uid[..10]);
        let utr = format!("REF{}", &uid[..12]);
        let now_str = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

        tx.execute(
            "INSERT INTO transactions (txn_id, account_id, txn_type, amount, direction, counterparty, status, utr_number, notes, created_at)
             VALUES (?1, ?2, 'REFUND', ?3, 'CREDIT', 'MoneyMax Auto-Refund Gateway', 'SUCCESS', ?4, ?5, ?6)",
            params![
                refund_txn_id,
                canonical_acc_id,
                amount,
                utr,
                format!("Refund for {}: {}", txn_id, reason),
                now_str
            ],
        )?;

        tx.execute(
            "UPDATE disputes
             SET status = 'RESOLVED_REFUNDED', agent_resolution_notes = ?1, resolved_at = ?2
             WHERE account_id = ?3 AND txn_id = ?4",
            params![
                format!("Refunded INR {:.2} via {}: {}", amount, actor, reason),
                now_str,
                canonical_acc_id,
                txn_id.trim()
            ],
        )?;

        tx.execute(
            "INSERT INTO audit_logs (timestamp, actor, action, target_account_id, details, status)
             VALUES (?1, ?2, 'ISSUE_REFUND', ?3, ?4, 'SUCCESS')",
            params![
                now_str,
                actor,
                canonical_acc_id,
                format!(
                    "Refunded INR {:.2} for txn {}. New balance: INR {:.2}",
                    amount, txn_id, new_balance
                )
            ],
        )?;

        tx.commit()?;

        Ok(RefundOutcome {
            success: true,
            refund_txn_id,
            account_id: canonical_acc_id,
            refunded_amount: amount,
            new_balance,
            timestamp: now_str,
        })
    }

    pub fn revert_unauthorized_payout(
        &self,
        account_id: &str,
        amount: f64,
        reason: &str,
        actor: &str,
    ) -> Result<RevertOutcome, DbError> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;

        let row: Option<(f64, String)> = tx
            .query_row(
                "SELECT balance, account_id FROM accounts WHERE account_id = ?1 COLLATE NOCASE",
                params![account_id.trim()],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;

        let (balance, canonical_acc_id) = match row {
            Some(r) => r,
            None => return Err(DbError::NotFound(format!("Account {} not found", account_id))),
        };

        let new_balance = (((balance - amount).max(0.0)) * 100.0).round() / 100.0;
        tx.execute(
            "UPDATE accounts SET balance = ?1 WHERE account_id = ?2",
            params![new_balance, canonical_acc_id],
        )?;

        let now_str = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        tx.execute(
            "INSERT INTO audit_logs (timestamp, actor, action, target_account_id, details, status)
             VALUES (?1, ?2, 'REVERT_ROGUE_PAYOUT', ?3, ?4, 'SUCCESS')",
            params![
                now_str,
                actor,
                canonical_acc_id,
                format!("Reverted unauthorized INR {:.2}: {}", amount, reason)
            ],
        )?;
        tx.commit()?;

        Ok(RevertOutcome {
            success: true,
            account_id: canonical_acc_id,
            reverted_amount: amount,
            new_balance,
            timestamp: now_str,
        })
    }

    pub fn update_kyc_status(
        &self,
        account_id: &str,
        new_status: &str,
        notes: &str,
        actor: &str,
    ) -> Result<(), DbError> {
        let status = new_status.trim().to_uppercase();
        if !["VERIFIED", "PENDING_REVIEW", "FLAGGED", "REJECTED"].contains(&status.as_str()) {
            return Err(DbError::BusinessRule(format!("Invalid KYC status: {}", status)));
        }

        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        let affected = tx.execute(
            "UPDATE accounts SET kyc_status = ?1 WHERE account_id = ?2 COLLATE NOCASE",
            params![status, account_id.trim()],
        )?;
        if affected == 0 {
            return Err(DbError::NotFound(format!("Account {} not found", account_id)));
        }
        let now_str = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        tx.execute(
            "INSERT INTO audit_logs (timestamp, actor, action, target_account_id, details, status)
             VALUES (?1, ?2, 'UPDATE_KYC', ?3, ?4, 'SUCCESS')",
            params![
                now_str,
                actor,
                account_id.trim(),
                format!("KYC updated to {}. Notes: {}", status, notes)
            ],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn update_credit_limit(
        &self,
        account_id: &str,
        new_limit: f64,
        reason: &str,
        actor: &str,
    ) -> Result<(), DbError> {
        if !new_limit.is_finite() || new_limit < 0.0 {
            return Err(DbError::BusinessRule("Credit limit must be a non-negative finite number".into()));
        }
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        let affected = tx.execute(
            "UPDATE accounts SET credit_limit = ?1, available_credit = ?1 WHERE account_id = ?2 COLLATE NOCASE",
            params![new_limit, account_id.trim()],
        )?;
        if affected == 0 {
            return Err(DbError::NotFound(format!("Account {} not found", account_id)));
        }
        let now_str = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        tx.execute(
            "INSERT INTO audit_logs (timestamp, actor, action, target_account_id, details, status)
             VALUES (?1, ?2, 'ADJUST_CREDIT_LIMIT', ?3, ?4, 'SUCCESS')",
            params![
                now_str,
                actor,
                account_id.trim(),
                format!("Credit limit set to INR {:.2}. Reason: {}", new_limit, reason)
            ],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn set_account_frozen(
        &self,
        account_id: &str,
        frozen: bool,
        reason: &str,
        actor: &str,
    ) -> Result<(), DbError> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        let affected = tx.execute(
            "UPDATE accounts SET is_frozen = ?1 WHERE account_id = ?2 COLLATE NOCASE",
            params![if frozen { 1 } else { 0 }, account_id.trim()],
        )?;
        if affected == 0 {
            return Err(DbError::NotFound(format!("Account {} not found", account_id)));
        }
        let now_str = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        tx.execute(
            "INSERT INTO audit_logs (timestamp, actor, action, target_account_id, details, status)
             VALUES (?1, ?2, ?3, ?4, ?5, 'SUCCESS')",
            params![
                now_str,
                actor,
                if frozen { "FREEZE_ACCOUNT" } else { "UNFREEZE_ACCOUNT" },
                account_id.trim(),
                reason
            ],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn get_recent_audit_logs(
        &self,
        account_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<DbAuditLog>, DbError> {
        let conn = self.conn.lock();
        let mut out = Vec::new();
        if let Some(acc) = account_id {
            let mut stmt = conn.prepare(
                "SELECT log_id, timestamp, actor, action, target_account_id, details, status
                 FROM audit_logs WHERE target_account_id = ?1 COLLATE NOCASE
                 ORDER BY log_id DESC LIMIT ?2",
            )?;
            let rows = stmt.query_map(params![acc.trim(), limit as i64], |r| {
                Ok(DbAuditLog {
                    log_id: r.get(0)?,
                    timestamp: r.get(1)?,
                    actor: r.get(2)?,
                    action: r.get(3)?,
                    target_account_id: r.get(4)?,
                    details: r.get(5)?,
                    status: r.get(6)?,
                })
            })?;
            for r in rows {
                out.push(r?);
            }
        } else {
            let mut stmt = conn.prepare(
                "SELECT log_id, timestamp, actor, action, target_account_id, details, status
                 FROM audit_logs ORDER BY log_id DESC LIMIT ?1",
            )?;
            let rows = stmt.query_map(params![limit as i64], |r| {
                Ok(DbAuditLog {
                    log_id: r.get(0)?,
                    timestamp: r.get(1)?,
                    actor: r.get(2)?,
                    action: r.get(3)?,
                    target_account_id: r.get(4)?,
                    details: r.get(5)?,
                    status: r.get(6)?,
                })
            })?;
            for r in rows {
                out.push(r?);
            }
        }
        Ok(out)
    }
}
