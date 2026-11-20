use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::Utc;
use parking_lot::Mutex;
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::DbError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SessionStatus {
    AiActive,
    NeedsOverride,
    StaffTakeover,
    Quarantined,
    Closed,
}

impl SessionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::AiActive => "AI_ACTIVE",
            Self::NeedsOverride => "NEEDS_OVERRIDE",
            Self::StaffTakeover => "STAFF_TAKEOVER",
            Self::Quarantined => "QUARANTINED",
            Self::Closed => "CLOSED",
        }
    }

    pub fn from_db(s: &str) -> Self {
        match s {
            "NEEDS_OVERRIDE" => Self::NeedsOverride,
            "STAFF_TAKEOVER" => Self::StaffTakeover,
            "QUARANTINED" => Self::Quarantined,
            "CLOSED" => Self::Closed,
            _ => Self::AiActive,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRecord {
    pub session_id: String,
    pub account_id: String,
    pub customer_name: String,
    pub status: String,
    pub assigned_officer: Option<String>,
    pub risk_score: f64,
    pub pending_overrides_count: i64,
    pub message_count: i64,
    pub last_message_preview: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessageRecord {
    pub msg_id: String,
    pub session_id: String,
    pub role: String,
    pub content_sanitized: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_unmasked: Option<String>,
    pub pii_counts: HashMap<String, usize>,
    pub layer_trace: Option<Value>,
    pub tool_calls: Option<Value>,
    pub decision_source: Option<String>,
    pub sentinel_verdict: Option<String>,
    pub overhead_ms: f64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingOverrideRecord {
    pub override_id: String,
    pub session_id: String,
    pub account_id: String,
    pub tool_name: String,
    pub tool_args: Value,
    pub violation_code: String,
    pub reason: String,
    pub status: String,
    pub decided_by: Option<String>,
    pub decision_notes: Option<String>,
    pub execution_result: Option<Value>,
    pub created_at: String,
    pub decided_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentinelAuditRecord {
    pub audit_id: String,
    pub session_id: Option<String>,
    pub event_id: Option<String>,
    pub engine: String,
    pub severity: String,
    pub drift_score: f64,
    pub rbi_grade: String,
    pub verdict: String,
    pub intervention: String,
    pub summary: String,
    pub details: Value,
    pub acknowledged: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaffAuditRecord {
    pub id: i64,
    pub officer: String,
    pub action: String,
    pub target_id: String,
    pub details: String,
    pub created_at: String,
}

#[derive(Clone)]
pub struct StateStore {
    path: PathBuf,
    conn: Arc<Mutex<Connection>>,
}

impl StateStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, DbError> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)?;
            }
        }
        let conn = Connection::open(&path)?;
        conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             PRAGMA journal_mode = WAL;
             PRAGMA busy_timeout = 5000;",
        )?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS sessions (
                session_id TEXT PRIMARY KEY,
                account_id TEXT NOT NULL,
                customer_name TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'AI_ACTIVE',
                assigned_officer TEXT,
                risk_score REAL NOT NULL DEFAULT 0.0,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS chat_messages (
                msg_id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                role TEXT NOT NULL,
                content_sanitized TEXT NOT NULL,
                content_unmasked TEXT NOT NULL,
                pii_counts_json TEXT NOT NULL,
                layer_trace_json TEXT,
                tool_calls_json TEXT,
                decision_source TEXT,
                sentinel_verdict TEXT,
                overhead_ms REAL NOT NULL DEFAULT 0.0,
                created_at TEXT NOT NULL,
                FOREIGN KEY (session_id) REFERENCES sessions(session_id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS pending_overrides (
                override_id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                account_id TEXT NOT NULL,
                tool_name TEXT NOT NULL,
                tool_args_json TEXT NOT NULL,
                violation_code TEXT NOT NULL,
                reason TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'PENDING',
                decided_by TEXT,
                decision_notes TEXT,
                execution_result_json TEXT,
                created_at TEXT NOT NULL,
                decided_at TEXT,
                FOREIGN KEY (session_id) REFERENCES sessions(session_id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS refund_velocity (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                account_id TEXT NOT NULL,
                amount REAL NOT NULL,
                created_at_epoch INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS telemetry_events (
                event_id TEXT PRIMARY KEY,
                payload_json TEXT NOT NULL,
                created_at_epoch INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS sentinel_audits (
                audit_id TEXT PRIMARY KEY,
                session_id TEXT,
                event_id TEXT,
                engine TEXT NOT NULL,
                severity TEXT NOT NULL,
                drift_score REAL NOT NULL,
                rbi_grade TEXT NOT NULL,
                verdict TEXT NOT NULL,
                intervention TEXT NOT NULL,
                summary TEXT NOT NULL,
                details_json TEXT NOT NULL,
                acknowledged INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS staff_audit_logs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                officer TEXT NOT NULL,
                action TEXT NOT NULL,
                target_id TEXT NOT NULL,
                details TEXT NOT NULL,
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS kv_settings (
                key TEXT PRIMARY KEY,
                value_json TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_messages_session ON chat_messages(session_id, created_at);
            CREATE INDEX IF NOT EXISTS idx_overrides_status ON pending_overrides(status, created_at);
            CREATE INDEX IF NOT EXISTS idx_velocity_acc ON refund_velocity(account_id, created_at_epoch);
            CREATE INDEX IF NOT EXISTS idx_telemetry_epoch ON telemetry_events(created_at_epoch);
            CREATE INDEX IF NOT EXISTS idx_sentinel_created ON sentinel_audits(created_at);",
        )?;

        Ok(Self {
            path,
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn now_iso() -> String {
        Utc::now().to_rfc3339()
    }

    fn now_epoch() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64
    }

    pub fn create_or_get_session(
        &self,
        session_id: &str,
        account_id: &str,
        customer_name: &str,
    ) -> Result<SessionRecord, DbError> {
        {
            let conn = self.conn.lock();
            let now = Self::now_iso();
            conn.execute(
                "INSERT OR IGNORE INTO sessions (session_id, account_id, customer_name, status, risk_score, created_at, updated_at)
                 VALUES (?1, ?2, ?3, 'AI_ACTIVE', 0.0, ?4, ?4)",
                params![session_id, account_id, customer_name, now],
            )?;
        }
        self.get_session(session_id)?
            .ok_or_else(|| DbError::NotFound(format!("Session {} not found", session_id)))
    }

    pub fn get_session(&self, session_id: &str) -> Result<Option<SessionRecord>, DbError> {
        let conn = self.conn.lock();
        let res = conn
            .query_row(
                "SELECT s.session_id, s.account_id, s.customer_name, s.status, s.assigned_officer, s.risk_score,
                        (SELECT COUNT(*) FROM pending_overrides o WHERE o.session_id = s.session_id AND o.status = 'PENDING'),
                        (SELECT COUNT(*) FROM chat_messages m WHERE m.session_id = s.session_id),
                        (SELECT m2.content_sanitized FROM chat_messages m2 WHERE m2.session_id = s.session_id ORDER BY m2.created_at DESC, m2.rowid DESC LIMIT 1),
                        s.created_at, s.updated_at
                 FROM sessions s WHERE s.session_id = ?1",
                params![session_id],
                |r| {
                    Ok(SessionRecord {
                        session_id: r.get(0)?,
                        account_id: r.get(1)?,
                        customer_name: r.get(2)?,
                        status: r.get(3)?,
                        assigned_officer: r.get(4)?,
                        risk_score: r.get(5)?,
                        pending_overrides_count: r.get(6)?,
                        message_count: r.get(7)?,
                        last_message_preview: r.get(8)?,
                        created_at: r.get(9)?,
                        updated_at: r.get(10)?,
                    })
                },
            )
            .optional()?;
        Ok(res)
    }

    pub fn list_sessions(&self, status_filter: Option<&str>) -> Result<Vec<SessionRecord>, DbError> {
        let conn = self.conn.lock();
        let base_sql = "SELECT s.session_id, s.account_id, s.customer_name, s.status, s.assigned_officer, s.risk_score,
                (SELECT COUNT(*) FROM pending_overrides o WHERE o.session_id = s.session_id AND o.status = 'PENDING'),
                (SELECT COUNT(*) FROM chat_messages m WHERE m.session_id = s.session_id),
                (SELECT m2.content_sanitized FROM chat_messages m2 WHERE m2.session_id = s.session_id ORDER BY m2.created_at DESC, m2.rowid DESC LIMIT 1),
                s.created_at, s.updated_at
         FROM sessions s ";

        let mut out = Vec::new();
        if let Some(st) = status_filter.filter(|s| !s.is_empty() && !s.eq_ignore_ascii_case("ALL")) {
            let mut stmt = conn.prepare(&format!(
                "{} WHERE s.status = ?1 ORDER BY s.updated_at DESC",
                base_sql
            ))?;
            let rows = stmt.query_map(params![st], |r| {
                Ok(SessionRecord {
                    session_id: r.get(0)?,
                    account_id: r.get(1)?,
                    customer_name: r.get(2)?,
                    status: r.get(3)?,
                    assigned_officer: r.get(4)?,
                    risk_score: r.get(5)?,
                    pending_overrides_count: r.get(6)?,
                    message_count: r.get(7)?,
                    last_message_preview: r.get(8)?,
                    created_at: r.get(9)?,
                    updated_at: r.get(10)?,
                })
            })?;
            for r in rows {
                out.push(r?);
            }
        } else {
            let mut stmt = conn.prepare(&format!("{} ORDER BY s.updated_at DESC", base_sql))?;
            let rows = stmt.query_map([], |r| {
                Ok(SessionRecord {
                    session_id: r.get(0)?,
                    account_id: r.get(1)?,
                    customer_name: r.get(2)?,
                    status: r.get(3)?,
                    assigned_officer: r.get(4)?,
                    risk_score: r.get(5)?,
                    pending_overrides_count: r.get(6)?,
                    message_count: r.get(7)?,
                    last_message_preview: r.get(8)?,
                    created_at: r.get(9)?,
                    updated_at: r.get(10)?,
                })
            })?;
            for r in rows {
                out.push(r?);
            }
        }
        Ok(out)
    }

    pub fn update_session_status(
        &self,
        session_id: &str,
        status: SessionStatus,
        assigned_officer: Option<&str>,
        risk_score: Option<f64>,
    ) -> Result<(), DbError> {
        let conn = self.conn.lock();
        let now = Self::now_iso();
        if let Some(score) = risk_score {
            conn.execute(
                "UPDATE sessions SET status = ?1, assigned_officer = COALESCE(?2, assigned_officer), risk_score = ?3, updated_at = ?4 WHERE session_id = ?5",
                params![status.as_str(), assigned_officer, score, now, session_id],
            )?;
        } else {
            conn.execute(
                "UPDATE sessions SET status = ?1, assigned_officer = COALESCE(?2, assigned_officer), updated_at = ?3 WHERE session_id = ?4",
                params![status.as_str(), assigned_officer, now, session_id],
            )?;
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn append_message(
        &self,
        msg_id: &str,
        session_id: &str,
        role: &str,
        content_sanitized: &str,
        content_unmasked: &str,
        pii_counts: &HashMap<String, usize>,
        layer_trace: Option<&Value>,
        tool_calls: Option<&Value>,
        decision_source: Option<&str>,
        sentinel_verdict: Option<&str>,
        overhead_ms: f64,
    ) -> Result<ChatMessageRecord, DbError> {
        let conn = self.conn.lock();
        let now = Self::now_iso();
        let pii_json = serde_json::to_string(pii_counts)?;
        let trace_json = layer_trace.map(serde_json::to_string).transpose()?;
        let tools_json = tool_calls.map(serde_json::to_string).transpose()?;

        conn.execute(
            "INSERT INTO chat_messages (
                msg_id, session_id, role, content_sanitized, content_unmasked,
                pii_counts_json, layer_trace_json, tool_calls_json, decision_source,
                sentinel_verdict, overhead_ms, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                msg_id,
                session_id,
                role,
                content_sanitized,
                content_unmasked,
                pii_json,
                trace_json,
                tools_json,
                decision_source,
                sentinel_verdict,
                overhead_ms,
                now
            ],
        )?;
        conn.execute(
            "UPDATE sessions SET updated_at = ?1 WHERE session_id = ?2",
            params![now, session_id],
        )?;

        Ok(ChatMessageRecord {
            msg_id: msg_id.to_string(),
            session_id: session_id.to_string(),
            role: role.to_string(),
            content_sanitized: content_sanitized.to_string(),
            content_unmasked: None,
            pii_counts: pii_counts.clone(),
            layer_trace: layer_trace.cloned(),
            tool_calls: tool_calls.cloned(),
            decision_source: decision_source.map(str::to_string),
            sentinel_verdict: sentinel_verdict.map(str::to_string),
            overhead_ms,
            created_at: now,
        })
    }

    pub fn get_session_messages(
        &self,
        session_id: &str,
        include_unmasked: bool,
    ) -> Result<Vec<ChatMessageRecord>, DbError> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT msg_id, session_id, role, content_sanitized, content_unmasked,
                    pii_counts_json, layer_trace_json, tool_calls_json, decision_source,
                    sentinel_verdict, overhead_ms, created_at
             FROM chat_messages
             WHERE session_id = ?1
             ORDER BY created_at ASC, rowid ASC",
        )?;
        let rows = stmt.query_map(params![session_id], |r| {
            let pii_str: String = r.get(5)?;
            let trace_str: Option<String> = r.get(6)?;
            let tools_str: Option<String> = r.get(7)?;
            let unmasked: String = r.get(4)?;
            Ok(ChatMessageRecord {
                msg_id: r.get(0)?,
                session_id: r.get(1)?,
                role: r.get(2)?,
                content_sanitized: r.get(3)?,
                content_unmasked: if include_unmasked { Some(unmasked) } else { None },
                pii_counts: serde_json::from_str(&pii_str).unwrap_or_default(),
                layer_trace: trace_str.and_then(|s| serde_json::from_str(&s).ok()),
                tool_calls: tools_str.and_then(|s| serde_json::from_str(&s).ok()),
                decision_source: r.get(8)?,
                sentinel_verdict: r.get(9)?,
                overhead_ms: r.get(10)?,
                created_at: r.get(11)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create_pending_override(
        &self,
        override_id: &str,
        session_id: &str,
        account_id: &str,
        tool_name: &str,
        tool_args: &Value,
        violation_code: &str,
        reason: &str,
    ) -> Result<PendingOverrideRecord, DbError> {
        let conn = self.conn.lock();
        let now = Self::now_iso();
        let args_json = serde_json::to_string(tool_args)?;
        conn.execute(
            "INSERT INTO pending_overrides (
                override_id, session_id, account_id, tool_name, tool_args_json,
                violation_code, reason, status, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'PENDING', ?8)",
            params![
                override_id,
                session_id,
                account_id,
                tool_name,
                args_json,
                violation_code,
                reason,
                now
            ],
        )?;

        Ok(PendingOverrideRecord {
            override_id: override_id.to_string(),
            session_id: session_id.to_string(),
            account_id: account_id.to_string(),
            tool_name: tool_name.to_string(),
            tool_args: tool_args.clone(),
            violation_code: violation_code.to_string(),
            reason: reason.to_string(),
            status: "PENDING".to_string(),
            decided_by: None,
            decision_notes: None,
            execution_result: None,
            created_at: now,
            decided_at: None,
        })
    }

    pub fn get_override(&self, override_id: &str) -> Result<Option<PendingOverrideRecord>, DbError> {
        let conn = self.conn.lock();
        let res = conn
            .query_row(
                "SELECT override_id, session_id, account_id, tool_name, tool_args_json,
                        violation_code, reason, status, decided_by, decision_notes,
                        execution_result_json, created_at, decided_at
                 FROM pending_overrides WHERE override_id = ?1",
                params![override_id],
                |r| {
                    let args_str: String = r.get(4)?;
                    let exec_str: Option<String> = r.get(10)?;
                    Ok(PendingOverrideRecord {
                        override_id: r.get(0)?,
                        session_id: r.get(1)?,
                        account_id: r.get(2)?,
                        tool_name: r.get(3)?,
                        tool_args: serde_json::from_str(&args_str).unwrap_or(Value::Null),
                        violation_code: r.get(5)?,
                        reason: r.get(6)?,
                        status: r.get(7)?,
                        decided_by: r.get(8)?,
                        decision_notes: r.get(9)?,
                        execution_result: exec_str.and_then(|s| serde_json::from_str(&s).ok()),
                        created_at: r.get(11)?,
                        decided_at: r.get(12)?,
                    })
                },
            )
            .optional()?;
        Ok(res)
    }

    pub fn list_overrides(
        &self,
        session_id: Option<&str>,
        status_filter: Option<&str>,
    ) -> Result<Vec<PendingOverrideRecord>, DbError> {
        let conn = self.conn.lock();
        let mut sql = String::from(
            "SELECT override_id, session_id, account_id, tool_name, tool_args_json,
                    violation_code, reason, status, decided_by, decision_notes,
                    execution_result_json, created_at, decided_at
             FROM pending_overrides WHERE 1=1",
        );
        let mut bind_vals: Vec<String> = Vec::new();
        if let Some(sid) = session_id {
            bind_vals.push(sid.to_string());
            sql.push_str(&format!(" AND session_id = ?{}", bind_vals.len()));
        }
        if let Some(st) = status_filter.filter(|s| !s.is_empty() && !s.eq_ignore_ascii_case("ALL")) {
            bind_vals.push(st.to_string());
            sql.push_str(&format!(" AND status = ?{}", bind_vals.len()));
        }
        sql.push_str(" ORDER BY created_at DESC");

        let mut stmt = conn.prepare(&sql)?;
        let params_refs: Vec<&dyn rusqlite::ToSql> =
            bind_vals.iter().map(|s| s as &dyn rusqlite::ToSql).collect();
        let rows = stmt.query_map(params_refs.as_slice(), |r| {
            let args_str: String = r.get(4)?;
            let exec_str: Option<String> = r.get(10)?;
            Ok(PendingOverrideRecord {
                override_id: r.get(0)?,
                session_id: r.get(1)?,
                account_id: r.get(2)?,
                tool_name: r.get(3)?,
                tool_args: serde_json::from_str(&args_str).unwrap_or(Value::Null),
                violation_code: r.get(5)?,
                reason: r.get(6)?,
                status: r.get(7)?,
                decided_by: r.get(8)?,
                decision_notes: r.get(9)?,
                execution_result: exec_str.and_then(|s| serde_json::from_str(&s).ok()),
                created_at: r.get(11)?,
                decided_at: r.get(12)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn claim_override_for_decision(&self, override_id: &str) -> Result<bool, DbError> {
        let conn = self.conn.lock();
        let affected = conn.execute(
            "UPDATE pending_overrides SET status = 'PROCESSING' WHERE override_id = ?1 AND status = 'PENDING'",
            params![override_id],
        )?;
        Ok(affected == 1)
    }

    pub fn resolve_override(
        &self,
        override_id: &str,
        new_status: &str,
        officer: &str,
        notes: &str,
        execution_result: Option<&Value>,
    ) -> Result<(), DbError> {
        let conn = self.conn.lock();
        let now = Self::now_iso();
        let exec_json = execution_result.map(serde_json::to_string).transpose()?;
        let affected = conn.execute(
            "UPDATE pending_overrides
             SET status = ?1, decided_by = ?2, decision_notes = ?3, execution_result_json = ?4, decided_at = ?5
             WHERE override_id = ?6",
            params![new_status, officer, notes, exec_json, now, override_id],
        )?;
        if affected == 0 {
            return Err(DbError::NotFound(format!("Override {} not found", override_id)));
        }
        Ok(())
    }

    pub fn purge_session_unmasked_pii(&self, session_id: &str) -> Result<usize, DbError> {
        let conn = self.conn.lock();
        let affected = conn.execute(
            "UPDATE chat_messages SET content_unmasked = content_sanitized WHERE session_id = ?1",
            params![session_id],
        )?;
        Ok(affected)
    }

    pub fn count_pending_overrides(&self) -> Result<usize, DbError> {
        let conn = self.conn.lock();
        let cnt: i64 = conn.query_row(
            "SELECT COUNT(*) FROM pending_overrides WHERE status = 'PENDING'",
            [],
            |r| r.get(0),
        )?;
        Ok(cnt.max(0) as usize)
    }

    /// Atomically checks 24h rolling refund total and reserves quota under BEGIN IMMEDIATE
    /// to prevent concurrent TOCTOU smurfing races. Returns Some(row_id) if reserved, None if cap exceeded.
    pub fn reserve_refund_velocity(
        &self,
        account_id: &str,
        amount: f64,
        max_24h_total: f64,
    ) -> Result<Option<i64>, DbError> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let now = Self::now_epoch();
        let cutoff = now - 86_400;
        let canon = account_id.trim().to_uppercase();

        tx.execute(
            "DELETE FROM refund_velocity WHERE created_at_epoch < ?1",
            params![cutoff],
        )?;
        let prior: f64 = tx.query_row(
            "SELECT COALESCE(SUM(amount), 0.0) FROM refund_velocity
             WHERE account_id = ?1 AND created_at_epoch >= ?2",
            params![canon, cutoff],
            |r| r.get(0),
        )?;

        if prior + amount > max_24h_total {
            return Ok(None);
        }

        tx.execute(
            "INSERT INTO refund_velocity (account_id, amount, created_at_epoch) VALUES (?1, ?2, ?3)",
            params![canon, amount, now],
        )?;
        let row_id = tx.last_insert_rowid();
        tx.commit()?;
        Ok(Some(row_id))
    }

    pub fn release_refund_velocity(&self, row_id: i64) -> Result<(), DbError> {
        let conn = self.conn.lock();
        conn.execute("DELETE FROM refund_velocity WHERE id = ?1", params![row_id])?;
        Ok(())
    }

    pub fn record_refund_velocity(&self, account_id: &str, amount: f64) -> Result<(), DbError> {
        let conn = self.conn.lock();
        let now = Self::now_epoch();
        let cutoff = now - 86_400;
        conn.execute(
            "DELETE FROM refund_velocity WHERE created_at_epoch < ?1",
            params![cutoff],
        )?;
        conn.execute(
            "INSERT INTO refund_velocity (account_id, amount, created_at_epoch) VALUES (?1, ?2, ?3)",
            params![account_id.trim().to_uppercase(), amount, now],
        )?;
        Ok(())
    }

    pub fn sum_refunds_last_24h(&self, account_id: &str) -> Result<f64, DbError> {
        let conn = self.conn.lock();
        let cutoff = Self::now_epoch() - 86_400;
        let total: f64 = conn.query_row(
            "SELECT COALESCE(SUM(amount), 0.0) FROM refund_velocity
             WHERE account_id = ?1 AND created_at_epoch >= ?2",
            params![account_id.trim().to_uppercase(), cutoff],
            |r| r.get(0),
        )?;
        Ok(total)
    }

    pub fn persist_telemetry_event(&self, event_id: &str, payload: &Value) -> Result<(), DbError> {
        let conn = self.conn.lock();
        let now = Self::now_epoch();
        let json = serde_json::to_string(payload)?;
        conn.execute(
            "INSERT OR REPLACE INTO telemetry_events (event_id, payload_json, created_at_epoch) VALUES (?1, ?2, ?3)",
            params![event_id, json, now],
        )?;
        Ok(())
    }

    pub fn load_recent_telemetry(&self, limit: usize) -> Result<Vec<Value>, DbError> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT payload_json FROM telemetry_events ORDER BY rowid DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |r| r.get::<_, String>(0))?;
        let mut out = Vec::new();
        for r in rows {
            if let Ok(v) = serde_json::from_str::<Value>(&r?) {
                out.push(v);
            }
        }
        Ok(out)
    }

    pub fn record_sentinel_audit(&self, audit: &SentinelAuditRecord) -> Result<(), DbError> {
        let conn = self.conn.lock();
        let details_json = serde_json::to_string(&audit.details)?;
        conn.execute(
            "INSERT OR REPLACE INTO sentinel_audits (
                audit_id, session_id, event_id, engine, severity, drift_score,
                rbi_grade, verdict, intervention, summary, details_json, acknowledged, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                audit.audit_id,
                audit.session_id,
                audit.event_id,
                audit.engine,
                audit.severity,
                audit.drift_score,
                audit.rbi_grade,
                audit.verdict,
                audit.intervention,
                audit.summary,
                details_json,
                if audit.acknowledged { 1 } else { 0 },
                audit.created_at
            ],
        )?;
        Ok(())
    }

    pub fn list_sentinel_audits(&self, limit: usize) -> Result<Vec<SentinelAuditRecord>, DbError> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT audit_id, session_id, event_id, engine, severity, drift_score,
                    rbi_grade, verdict, intervention, summary, details_json, acknowledged, created_at
             FROM sentinel_audits ORDER BY rowid DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |r| {
            let det: String = r.get(10)?;
            Ok(SentinelAuditRecord {
                audit_id: r.get(0)?,
                session_id: r.get(1)?,
                event_id: r.get(2)?,
                engine: r.get(3)?,
                severity: r.get(4)?,
                drift_score: r.get(5)?,
                rbi_grade: r.get(6)?,
                verdict: r.get(7)?,
                intervention: r.get(8)?,
                summary: r.get(9)?,
                details: serde_json::from_str(&det).unwrap_or(Value::Null),
                acknowledged: r.get::<_, i64>(11)? != 0,
                created_at: r.get(12)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn acknowledge_sentinel_audit(&self, audit_id: &str) -> Result<bool, DbError> {
        let conn = self.conn.lock();
        let affected = conn.execute(
            "UPDATE sentinel_audits SET acknowledged = 1 WHERE audit_id = ?1",
            params![audit_id],
        )?;
        Ok(affected > 0)
    }

    pub fn log_staff_action(
        &self,
        officer: &str,
        action: &str,
        target_id: &str,
        details: &str,
    ) -> Result<(), DbError> {
        let conn = self.conn.lock();
        let now = Self::now_iso();
        conn.execute(
            "INSERT INTO staff_audit_logs (officer, action, target_id, details, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![officer, action, target_id, details, now],
        )?;
        Ok(())
    }

    pub fn list_staff_audit_logs(&self, limit: usize) -> Result<Vec<StaffAuditRecord>, DbError> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, officer, action, target_id, details, created_at
             FROM staff_audit_logs ORDER BY id DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |r| {
            Ok(StaffAuditRecord {
                id: r.get(0)?,
                officer: r.get(1)?,
                action: r.get(2)?,
                target_id: r.get(3)?,
                details: r.get(4)?,
                created_at: r.get(5)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn save_kv_setting(&self, key: &str, value: &Value) -> Result<(), DbError> {
        let conn = self.conn.lock();
        let now = Self::now_iso();
        let json = serde_json::to_string(value)?;
        conn.execute(
            "INSERT OR REPLACE INTO kv_settings (key, value_json, updated_at) VALUES (?1, ?2, ?3)",
            params![key, json, now],
        )?;
        Ok(())
    }

    pub fn load_kv_setting(&self, key: &str) -> Result<Option<Value>, DbError> {
        let conn = self.conn.lock();
        let res: Option<String> = conn
            .query_row(
                "SELECT value_json FROM kv_settings WHERE key = ?1",
                params![key],
                |r| r.get(0),
            )
            .optional()?;
        Ok(res.and_then(|s| serde_json::from_str(&s).ok()))
    }
}
