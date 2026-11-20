use std::sync::Arc;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tempfile::TempDir;
use tower::ServiceExt;

use aegismesh_core::config::{AegisConfig, ConfigStore};
use aegismesh_core::db::moneymax::MoneyMaxDb;
use aegismesh_core::db::state_store::StateStore;
use aegismesh_core::pipeline::AegisMeshEngine;
use aegismesh_server::api::create_router;

fn setup_app(admin_key: Option<&str>) -> (axum::Router, Arc<AegisMeshEngine>, TempDir) {
    let dir = TempDir::new().unwrap();
    let mm = MoneyMaxDb::open_or_bootstrap(dir.path().join("mm.db")).unwrap();
    let st = StateStore::open(dir.path().join("st.db")).unwrap();
    let mut cfg = AegisConfig::default();
    cfg.firewall.admin_key = admin_key.map(str::to_string);
    let eng = Arc::new(AegisMeshEngine::new(ConfigStore::new(cfg), mm, st));
    let router = create_router(eng.clone());
    (router, eng, dir)
}

async fn json_req(
    app: &axum::Router,
    method: &str,
    uri: &str,
    body: Option<Value>,
    admin_key: Option<&str>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    builder = builder.header(header::CONTENT_TYPE, "application/json");
    if let Some(k) = admin_key {
        builder = builder.header("x-admin-key", k);
    }
    let req = builder
        .body(match body {
            Some(v) => Body::from(serde_json::to_vec(&v).unwrap()),
            None => Body::empty(),
        })
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let parsed = serde_json::from_slice(&bytes).unwrap_or_else(|_| {
        json!({ "_raw": String::from_utf8_lossy(&bytes).to_string() })
    });
    (status, parsed)
}

#[tokio::test]
async fn test_health_and_admin_lock() {
    let (app, _eng, _dir) = setup_app(Some("top-secret-key"));

    let (st, body) = json_req(&app, "GET", "/health", None, None).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(body["status"], "ok");

    // Authorized shield toggle succeeds first (before rate-limiter trips on 5 unauthorized attempts)
    let (st_auth, res) = json_req(
        &app,
        "POST",
        "/v1/shield/toggle",
        Some(json!({"active": false, "officer": "Officer A"})),
        Some("top-secret-key"),
    )
    .await;
    assert_eq!(st_auth, StatusCode::OK);
    assert_eq!(res["shield_active"], false);

    // Unauthorized calls fail with 401 across privileged routes
    for (m, u, b) in [
        ("POST", "/v1/shield/toggle", Some(json!({"active": true}))),
        ("POST", "/api/v1/customers/MMX-AC-5004/manual-action", Some(json!({"action": "UNFREEZE", "officer": "Attacker"}))),
        ("POST", "/api/v1/sessions/SESS-X/takeover", Some(json!({"takeover": true, "officer": "Attacker"}))),
        ("GET", "/api/v1/audit/export?format=csv", None),
        ("POST", "/v1/chat/completions", Some(json!({"prompt": "hi", "tool_call": {"name": "execute_refund", "args": {"account_id": "MMX-AC-5001", "amount": 50}}}))),
    ] {
        let (st_unauth, _) = json_req(&app, m, u, b, None).await;
        assert_eq!(st_unauth, StatusCode::UNAUTHORIZED, "Route {} {} leaked without Admin Key!", m, u);
    }
}

#[tokio::test]
async fn test_cors_and_bruteforce_lockout() {
    let (app, _eng, _dir) = setup_app(Some("brute-lock-key"));

    // Untrusted external Origin must NOT receive Access-Control-Allow-Origin
    let evil_req = Request::builder()
        .method("GET")
        .uri("/health")
        .header("origin", "https://evil.example")
        .body(Body::empty())
        .unwrap();
    let evil_resp = app.clone().oneshot(evil_req).await.unwrap();
    assert!(evil_resp.headers().get(header::ACCESS_CONTROL_ALLOW_ORIGIN).is_none());

    // Localhost origin IS permitted
    let local_req = Request::builder()
        .method("GET")
        .uri("/health")
        .header("origin", "http://127.0.0.1:8080")
        .body(Body::empty())
        .unwrap();
    let local_resp = app.clone().oneshot(local_req).await.unwrap();
    assert_eq!(
        local_resp.headers().get(header::ACCESS_CONTROL_ALLOW_ORIGIN).and_then(|v| v.to_str().ok()),
        Some("http://127.0.0.1:8080")
    );

    // 5 wrong admin attempts trigger 429 Too Many Requests on 6th attempt
    for _ in 0..5 {
        let req = Request::builder()
            .method("POST")
            .uri("/api/v1/auth/verify")
            .header("x-forwarded-for", "10.9.9.9")
            .header("x-admin-key", "wrong-guess")
            .body(Body::empty())
            .unwrap();
        let r = app.clone().oneshot(req).await.unwrap();
        assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
    }
    let locked_req = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/verify")
        .header("x-forwarded-for", "10.9.9.9")
        .header("x-admin-key", "brute-lock-key")
        .body(Body::empty())
        .unwrap();
    let locked_resp = app.clone().oneshot(locked_req).await.unwrap();
    assert_eq!(locked_resp.status(), StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn test_full_chat_override_takeover_and_pii_privacy() {
    let (app, eng, _dir) = setup_app(Some("admin-123"));

    // 1. Create session for Kavita Roy (MMX-AC-5002)
    let (st, created) = json_req(
        &app,
        "POST",
        "/api/v1/sessions",
        Some(json!({"account_id": "MMX-AC-5002"})),
        None,
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let sid = created["session"]["session_id"].as_str().unwrap().to_string();

    let orig_bal = eng
        .moneymax_db
        .get_account_context("MMX-AC-5002")
        .unwrap()
        .unwrap()
        .balance;

    // 2. Customer sends message with PAN + ₹25,000 refund request + CSV formula probe
    let (st_msg, msg_res) = json_req(
        &app,
        "POST",
        &format!("/api/v1/sessions/{}/message", sid),
        Some(json!({"content": "My PAN is RNFPR8573N. Please refund ₹25,000 for my Croma charge."})),
        None,
    )
    .await;
    assert_eq!(st_msg, StatusCode::OK);
    assert_eq!(msg_res["outcome"]["status"], "CIRCUIT_BREAKER_TRIPPED");
    let ov_id = msg_res["outcome"]["created_overrides"][0]["override_id"]
        .as_str()
        .unwrap()
        .to_string();

    // Standard session detail masks Customer 360 PII and omits content_unmasked!
    let (_, detail) = json_req(&app, "GET", &format!("/api/v1/sessions/{}", sid), None, None).await;
    let detail_str = serde_json::to_string(&detail).unwrap();
    assert!(!detail_str.contains("RNFPR8573N"), "Raw PAN leaked in session detail!");
    assert!(!detail_str.contains("+919791756179"), "Raw phone leaked in Customer 360!");

    let (_, hist) = json_req(&app, "GET", "/api/v1/telemetry/history", None, None).await;
    assert!(!serde_json::to_string(&hist).unwrap().contains("RNFPR8573N"));

    // 3. Reveal PII fails without Admin Key, succeeds WITH Admin Key and logs staff audit
    let (st_no_key, _) = json_req(
        &app,
        "POST",
        &format!("/api/v1/sessions/{}/reveal-pii", sid),
        Some(json!({"officer": "Rao"})),
        None,
    )
    .await;
    assert_eq!(st_no_key, StatusCode::UNAUTHORIZED);

    let (st_ok_key, revealed) = json_req(
        &app,
        "POST",
        &format!("/api/v1/sessions/{}/reveal-pii", sid),
        Some(json!({"officer": "Rao", "reason": "Audit check"})),
        Some("admin-123"),
    )
    .await;
    assert_eq!(st_ok_key, StatusCode::OK);
    assert!(serde_json::to_string(&revealed["messages"]).unwrap().contains("RNFPR8573N"));

    // 4. Staff Approves ₹25,000 Maker-Checker Override (authenticated) -> second click gets 409 Conflict!
    let (st_dec, dec_res) = json_req(
        &app,
        "POST",
        &format!("/api/v1/overrides/{}/decision", ov_id),
        Some(json!({"action": "APPROVE", "officer": "Officer Rao", "notes": "Verified invoice"})),
        Some("admin-123"),
    )
    .await;
    assert_eq!(st_dec, StatusCode::OK);
    assert_eq!(dec_res["override"]["status"], "APPROVED");

    let (st_dup, _) = json_req(
        &app,
        "POST",
        &format!("/api/v1/overrides/{}/decision", ov_id),
        Some(json!({"action": "APPROVE", "officer": "Officer Rao"})),
        Some("admin-123"),
    )
    .await;
    assert_eq!(st_dup, StatusCode::CONFLICT);

    let new_bal = eng
        .moneymax_db
        .get_account_context("MMX-AC-5002")
        .unwrap()
        .unwrap()
        .balance;
    assert!((new_bal - (orig_bal + 25000.0)).abs() < 0.01);

    // 5. Staff Takeover + Staff Reply + DPDP Purge PII
    let (st_tk, _) = json_req(
        &app,
        "POST",
        &format!("/api/v1/sessions/{}/takeover", sid),
        Some(json!({"takeover": true, "officer": "Officer Rao"})),
        Some("admin-123"),
    )
    .await;
    assert_eq!(st_tk, StatusCode::OK);

    let (_, paused_msg) = json_req(
        &app,
        "POST",
        &format!("/api/v1/sessions/{}/message", sid),
        Some(json!({"content": "Thank you, can you confirm?"})),
        None,
    )
    .await;
    assert_eq!(paused_msg["mode"], "STAFF_TAKEOVER");

    let (st_rep, rep_res) = json_req(
        &app,
        "POST",
        &format!("/api/v1/sessions/{}/staff-reply", sid),
        Some(json!({"officer": "Officer Rao", "content": "Confirmed, ₹25,000 credited."})),
        Some("admin-123"),
    )
    .await;
    assert_eq!(st_rep, StatusCode::OK);
    assert_eq!(rep_res["message"]["role"], "staff");

    // Purge raw PII from session -> reveal-pii must no longer see RNFPR8573N!
    let (st_purge, _) = json_req(
        &app,
        "POST",
        &format!("/api/v1/sessions/{}/purge-pii", sid),
        Some(json!({"officer": "Officer Rao"})),
        Some("admin-123"),
    )
    .await;
    assert_eq!(st_purge, StatusCode::OK);
    let (_, after_purge) = json_req(
        &app,
        "POST",
        &format!("/api/v1/sessions/{}/reveal-pii", sid),
        Some(json!({"officer": "Rao"})),
        Some("admin-123"),
    )
    .await;
    assert!(!serde_json::to_string(&after_purge["messages"]).unwrap().contains("RNFPR8573N"));

    // 6. Manual customer action unfreezes MMX-AC-5004 & CSV formula injection check
    let (st_man, _) = json_req(
        &app,
        "POST",
        "/api/v1/customers/MMX-AC-5004/manual-action",
        Some(json!({"action": "UNFREEZE", "officer": "Officer Rao", "reason": "=cmd|'/C calc'!A0"})),
        Some("admin-123"),
    )
    .await;
    assert_eq!(st_man, StatusCode::OK);
    assert!(!eng.moneymax_db.get_account_context("MMX-AC-5004").unwrap().unwrap().is_frozen);

    let csv_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/audit/export?format=csv")
                .header("x-admin-key", "admin-123")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(csv_resp.status(), StatusCode::OK);
    let csv_body = String::from_utf8_lossy(&csv_resp.into_body().collect().await.unwrap().to_bytes()).to_string();
    assert!(csv_body.contains("'=cmd|'/C calc'!A0"), "CSV formula was not neutralized!");
}
