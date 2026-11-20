import sys
import os

if sys.stdout.encoding != 'utf-8':
    try:
        sys.stdout.reconfigure(encoding='utf-8')
    except Exception:
        pass

from db_service import MoneyMaxDBService

def test():
    db = MoneyMaxDBService()
    
    print("1. Testing get_customer_profile by Account ID 'MMX-AC-5001'...")
    profile = db.get_customer_profile("MMX-AC-5001")
    assert profile is not None, "Profile not found!"
    print(f"   Found customer: {profile['full_name']}, PAN: {profile['pan_number']}, Balance: INR {profile['balance']}")
    
    print("\n2. Testing get_transaction_history for 'MMX-AC-5001'...")
    txns = db.get_transaction_history("MMX-AC-5001", limit=3)
    print(f"   Retrieved {len(txns)} transactions.")
    for t in txns:
        print(f"   - {t['txn_id']}: INR {t['amount']} ({t['direction']}) -> {t['notes']}")
        
    print("\n3. Testing get_disputes for 'MMX-AC-5001'...")
    disputes = db.get_disputes("MMX-AC-5001")
    print(f"   Retrieved {len(disputes)} disputes.")
    if disputes:
        print(f"   - Dispute: {disputes[0]['dispute_id']}, Amount: INR {disputes[0]['claimed_amount']}, Issue: {disputes[0]['dispute_type']}")
        
    print("\n4. Testing execute_refund on 'MMX-AC-5001' for INR 450.00...")
    old_balance = profile['balance']
    refund_res = db.execute_refund("MMX-AC-5001", "TXN-MMX-100010", 450.00, "Duplicate debit test")
    assert refund_res["success"], f"Refund failed: {refund_res}"
    print(f"   Refund success! New balance: INR {refund_res['new_balance']} (Old: INR {old_balance})")
    
    print("\n5. Verifying frozen account protection on 'MMX-AC-5004' (Frozen account)...")
    frozen_res = db.execute_refund("MMX-AC-5004", "TXN-MMX-TEST", 1000.0, "Test on frozen account")
    assert not frozen_res["success"], "Frozen account allowed refund!"
    print(f"   Correctly rejected: {frozen_res['error']}")
    
    print("\nAll database service tests passed successfully!")

if __name__ == "__main__":
    test()
