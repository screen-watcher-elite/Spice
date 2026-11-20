-- MoneyMax Client Core Banking & Fintech Database Schema
-- Designed for autonomous agent tool testing, PII masking verification, and financial guardrails.

PRAGMA foreign_keys = ON;

-- 1. Customers Table (Contains Primary PII subject to DPDP Act 2023)
CREATE TABLE IF NOT EXISTS customers (
    customer_id TEXT PRIMARY KEY,
    full_name TEXT NOT NULL,
    email TEXT UNIQUE NOT NULL,
    phone TEXT UNIQUE NOT NULL,
    pan_number TEXT UNIQUE NOT NULL,       -- Primary PAN: e.g. ABCDE1234F
    aadhaar_number TEXT UNIQUE NOT NULL,   -- 12-digit Aadhaar: e.g. 5432 1098 7654
    date_of_birth TEXT NOT NULL,
    address TEXT NOT NULL,
    city TEXT NOT NULL,
    state TEXT NOT NULL,
    pincode TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- 2. Accounts Table (Financial balances, limits, KYC, risk scoring)
CREATE TABLE IF NOT EXISTS accounts (
    account_id TEXT PRIMARY KEY,
    customer_id TEXT NOT NULL,
    account_number TEXT UNIQUE NOT NULL,
    ifsc_code TEXT NOT NULL DEFAULT 'MMXB0001024',
    upi_vpa TEXT UNIQUE NOT NULL,
    balance REAL NOT NULL DEFAULT 0.0 CHECK(balance >= 0.0),
    credit_limit REAL NOT NULL DEFAULT 50000.0 CHECK(credit_limit >= 0.0),
    available_credit REAL NOT NULL DEFAULT 50000.0,
    cibil_score INTEGER NOT NULL CHECK(cibil_score BETWEEN 300 AND 900),
    kyc_status TEXT NOT NULL CHECK(kyc_status IN ('VERIFIED', 'PENDING_REVIEW', 'FLAGGED', 'REJECTED')),
    risk_tier TEXT NOT NULL CHECK(risk_tier IN ('LOW', 'MEDIUM', 'HIGH')),
    is_frozen INTEGER NOT NULL DEFAULT 0 CHECK(is_frozen IN (0, 1)),
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (customer_id) REFERENCES customers(customer_id) ON DELETE CASCADE
);

-- 3. Transactions Table (UPI transfers, merchant payments, refunds)
CREATE TABLE IF NOT EXISTS transactions (
    txn_id TEXT PRIMARY KEY,
    account_id TEXT NOT NULL,
    txn_type TEXT NOT NULL CHECK(txn_type IN ('UPI_TRANSFER', 'MERCHANT_PAYMENT', 'BILL_PAY', 'REFUND', 'CREDIT_DRAWDOWN', 'CASHBACK')),
    amount REAL NOT NULL CHECK(amount > 0.0),
    direction TEXT NOT NULL CHECK(direction IN ('DEBIT', 'CREDIT')),
    counterparty TEXT NOT NULL,
    status TEXT NOT NULL CHECK(status IN ('SUCCESS', 'FAILED', 'PENDING', 'DISPUTED')),
    utr_number TEXT UNIQUE NOT NULL,
    notes TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (account_id) REFERENCES accounts(account_id) ON DELETE CASCADE
);

-- 4. Disputes & Support Tickets (Simulating real-world customer complaints and refund requests)
CREATE TABLE IF NOT EXISTS disputes (
    dispute_id TEXT PRIMARY KEY,
    account_id TEXT NOT NULL,
    txn_id TEXT NOT NULL,
    dispute_type TEXT NOT NULL CHECK(dispute_type IN ('DEBIT_WITHOUT_CREDIT', 'DOUBLE_DEBIT', 'FRAUDULENT_TXN', 'WRONG_BENEFICIARY', 'MERCHANT_ISSUE')),
    claimed_amount REAL NOT NULL CHECK(claimed_amount > 0.0),
    status TEXT NOT NULL CHECK(status IN ('OPEN', 'INVESTIGATING', 'RESOLVED_REFUNDED', 'REJECTED')),
    customer_statement TEXT NOT NULL,
    agent_resolution_notes TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    resolved_at TEXT,
    FOREIGN KEY (account_id) REFERENCES accounts(account_id) ON DELETE CASCADE,
    FOREIGN KEY (txn_id) REFERENCES transactions(txn_id) ON DELETE CASCADE
);

-- 5. Audit Log (Tracks every tool call and agent modification)
CREATE TABLE IF NOT EXISTS audit_logs (
    log_id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp TEXT NOT NULL DEFAULT (datetime('now')),
    actor TEXT NOT NULL,
    action TEXT NOT NULL,
    target_account_id TEXT,
    details TEXT NOT NULL,
    status TEXT NOT NULL
);

-- Indexes for high-speed agent lookups
CREATE INDEX IF NOT EXISTS idx_customers_phone ON customers(phone);
CREATE INDEX IF NOT EXISTS idx_customers_pan ON customers(pan_number);
CREATE INDEX IF NOT EXISTS idx_accounts_vpa ON accounts(upi_vpa);
CREATE INDEX IF NOT EXISTS idx_accounts_cust ON accounts(customer_id);
CREATE INDEX IF NOT EXISTS idx_transactions_acc ON transactions(account_id);
CREATE INDEX IF NOT EXISTS idx_disputes_acc ON disputes(account_id);
CREATE INDEX IF NOT EXISTS idx_disputes_status ON disputes(status);
