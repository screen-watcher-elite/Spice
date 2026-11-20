use std::collections::HashMap;
use aegismesh_core::config::{AegisConfig, ConfigStore, SettingsPatch};
use aegismesh_core::db::moneymax::MoneyMaxDb;
use aegismesh_core::db::state_store::{SentinelAuditRecord, SessionStatus, StateStore};
use serde_json::json;
use tempfile::TempDir;

#[test]
fn test_moneymax_bootstrap_and_transactions() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("moneymax.db");
    let db = MoneyMaxDb::open_or_bootstrap(&db_path).unwrap();

    let customers = db.list_customers().unwrap();
    assert_eq!(customers.len(), 10);

    let prof = db.get_customer_profile("MMX-AC-5001").unwrap().unwrap();
    assert_eq!(prof.full_name, "Sneha Chikane");
    let orig_bal = prof.balance;

    // Valid refund
    let outcome = db
        .execute_refund("MMX-AC-5001", "TXN-MMX-100010", 450.0, "Duplicate debit", "TEST", false)
        .unwrap();
    assert!(outcome.success);
    assert!((outcome.new_balance - (orig_bal + 450.0)).abs() < 0.01);

    // Revert unauthorized payout
    let rev = db
        .revert_unauthorized_payout("MMX-AC-5001", 450.0, "Sentinel test revert", "AEGISMESH_SENTINEL")
        .unwrap();
    assert!((rev.new_balance - orig_bal).abs() < 0.01);

    // Frozen account rejects agent refund without staff override
    let frozen_err = db.execute_refund("MMX-AC-5004", "TXN-MMX-100040", 100.0, "test", "AGENT", false);
    assert!(frozen_err.is_err());

    // Staff override can operate when explicitly allowed
    let override_ok = db.execute_refund("MMX-AC-5004", "TXN-MMX-100040", 100.0, "staff", "STAFF_OVERRIDE:Admin", true);
    assert!(override_ok.is_ok());

    let logs = db.get_recent_audit_logs(Some("MMX-AC-5001"), 10).unwrap();
    assert!(logs.len() >= 2);
}

#[test]
fn test_real_existing_moneymax_db_readable() {
    let existing = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("database")
        .join("moneymax.db");
    if existing.exists() {
        let dir = TempDir::new().unwrap();
        let copy_path = dir.path().join("moneymax_copy.db");
        std::fs::copy(&existing, &copy_path).unwrap();
        let db = MoneyMaxDb::open_or_bootstrap(&copy_path).unwrap();
        let list = db.list_customers().unwrap();
        assert_eq!(list.len(), 100);
        let c = db.get_customer_profile("MMX-AC-5001").unwrap().unwrap();
        assert_eq!(c.full_name, "Sneha Chikane");
    }
}

#[test]
fn test_state_store_sessions_overrides_and_velocity() {
    let dir = TempDir::new().unwrap();
    let st_path = dir.path().join("aegismesh_state.db");
    let store = StateStore::open(&st_path).unwrap();

    let sess = store
        .create_or_get_session("SESS-001", "MMX-AC-5002", "Kavita Roy")
        .unwrap();
    assert_eq!(sess.status, "AI_ACTIVE");

    let mut counts = HashMap::new();
    counts.insert("PAN".to_string(), 1);
    store
        .append_message(
            "MSG-1",
            "SESS-001",
            "customer",
            "My PAN is <PII:PAN:123>",
            "My PAN is RNFPR8573N",
            &counts,
            Some(&json!([])),
            None,
            Some("SMART_FAST_PATH"),
            Some("CLEAR"),
            0.12,
        )
        .unwrap();

    // Standard fetch must NOT return content_unmasked
    let masked_msgs = store.get_session_messages("SESS-001", false).unwrap();
    assert_eq!(masked_msgs.len(), 1);
    assert!(masked_msgs[0].content_unmasked.is_none());

    // Staff reveal returns content_unmasked
    let revealed = store.get_session_messages("SESS-001", true).unwrap();
    assert_eq!(revealed[0].content_unmasked.as_deref(), Some("My PAN is RNFPR8573N"));

    // Pending override & velocity
    let ov = store
        .create_pending_override(
            "OV-1",
            "SESS-001",
            "MMX-AC-5002",
            "execute_refund",
            &json!({"account_id": "MMX-AC-5002", "amount": 25000.0}),
            "ERR_REFUND_LIMIT_EXCEEDED",
            "Exceeds RBI limit",
        )
        .unwrap();
    assert_eq!(ov.status, "PENDING");
    store
        .update_session_status("SESS-001", SessionStatus::NeedsOverride, Some("Officer Rao"), Some(0.85))
        .unwrap();

    assert_eq!(store.count_pending_overrides().unwrap(), 1);
    store
        .resolve_override("OV-1", "APPROVED", "Officer Rao", "Verified bill", Some(&json!({"ok": true})))
        .unwrap();
    assert_eq!(store.count_pending_overrides().unwrap(), 0);

    // Velocity tracking
    store.record_refund_velocity("MMX-AC-5002", 4999.0).unwrap();
    store.record_refund_velocity("MMX-AC-5002", 4999.0).unwrap();
    let total = store.sum_refunds_last_24h("MMX-AC-5002").unwrap();
    assert!((total - 9998.0).abs() < 0.01);

    // Sentinel audit persistence
    store
        .record_sentinel_audit(&SentinelAuditRecord {
            audit_id: "AUD-1".into(),
            session_id: Some("SESS-001".into()),
            event_id: Some("AEGIS-1".into()),
            engine: "HEURISTIC_SENTINEL".into(),
            severity: "LOW".into(),
            drift_score: 98.5,
            rbi_grade: "AAA".into(),
            verdict: "COMPLIANT".into(),
            intervention: "NONE".into(),
            summary: "Clean turn".into(),
            details: json!({}),
            acknowledged: false,
            created_at: "2026-11-20T00:00:00Z".into(),
        })
        .unwrap();
    assert_eq!(store.list_sentinel_audits(10).unwrap().len(), 1);
}

#[test]
fn test_config_store_redaction() {
    let mut cfg = AegisConfig::default();
    cfg.google_ai.api_key = Some("AIzaSySuperSecretKey123456".into());
    cfg.firewall.admin_key = Some("my-admin-secret".into());
    let store = ConfigStore::new(cfg);

    let view = store.public_view();
    assert!(view.gemini_api_key_configured);
    assert_eq!(view.gemini_api_key_masked.as_deref(), Some("••••••••3456"));
    assert!(view.admin_key_configured);
    assert!(!store.verify_admin_key(Some("wrong")));
    assert!(store.verify_admin_key(Some("my-admin-secret")));

    store.apply_patch(SettingsPatch {
        clear_gemini_api_key: Some(true),
        ..Default::default()
    });
    assert!(!store.public_view().gemini_api_key_configured);
}

#[test]
fn test_dotenv_three_model_loading() {
    let dir = TempDir::new().unwrap();
    let env_file = dir.path().join(".env");
    std::fs::write(
        &env_file,
        "GEMINI_API_KEY=AIzaSyTestDummyKey1234567890\n\
         GEMINI_MODEL=gemini-3.1-flash-lite\n\
         GEMINI_AGENT_MODEL=gemini-3.5-flash-lite\n\
         GEMINI_DECISION_MODEL=gemini-decision-1\n\
         GEMINI_SENTINEL_MODEL=gemini-3-flash-preview\n",
    )
    .unwrap();

    let cfg = AegisConfig::load_from_dir(dir.path());
    assert!(cfg.google_ai.has_live_key());
    assert_eq!(cfg.google_ai.agent_model, "gemini-3.5-flash-lite");
    assert_eq!(cfg.google_ai.decision_model, "gemini-decision-1");
    assert_eq!(cfg.sentinel.model, "gemini-3-flash-preview");

    // Verify legacy GEMINI_MODEL only sets agent_model and never overrides sentinel.model
    let legacy_dir = TempDir::new().unwrap();
    std::fs::write(
        legacy_dir.path().join(".env"),
        "GEMINI_API_KEY=AIzaSyTestDummyKey1234567890\nGEMINI_MODEL=gemini-3.1-flash-lite\n",
    )
    .unwrap();
    let legacy_cfg = AegisConfig::load_from_dir(legacy_dir.path());
    assert_eq!(legacy_cfg.google_ai.agent_model, "gemini-3.1-flash-lite");
    assert_eq!(legacy_cfg.sentinel.model, "gemini-3-flash-preview");
}

#[test]
fn test_cloud_run_env_defaults() {
    let mut cfg = AegisConfig::default();
    assert_eq!(cfg.server.host, "127.0.0.1");
    assert_eq!(cfg.server.port, 8080);
    assert!(cfg.server.auto_open_browser);

    AegisConfig::apply_cloud_env(&mut cfg, None, None, Some("9090"), Some("aegismesh"));
    assert_eq!(cfg.server.host, "0.0.0.0");
    assert_eq!(cfg.server.port, 9090);
    assert!(!cfg.server.auto_open_browser);

    // Explicit AEGIS_HOST & AEGIS_PORT override Cloud Run fallbacks
    let mut explicit = AegisConfig::default();
    AegisConfig::apply_cloud_env(&mut explicit, Some("127.0.0.1"), Some("7777"), Some("9090"), Some("aegismesh"));
    assert_eq!(explicit.server.host, "127.0.0.1");
    assert_eq!(explicit.server.port, 7777);
}
