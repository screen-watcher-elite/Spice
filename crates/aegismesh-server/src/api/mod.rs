pub mod customers;
pub mod gateway;
pub mod sessions;
pub mod settings;
pub mod telemetry;

use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::{
    extract::DefaultBodyLimit,
    http::{header, HeaderMap, HeaderName, HeaderValue, Method, StatusCode},
    response::{IntoResponse, Json},
    routing::{get, post},
    Router,
};
use serde_json::{json, Value};
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::timeout::TimeoutLayer;

use aegismesh_core::pipeline::AegisMeshEngine;

use crate::embedded_ui::static_handler;

pub type ApiError = (StatusCode, Json<Value>);

pub fn err_resp(code: StatusCode, msg: impl Into<String>) -> ApiError {
    (code, Json(json!({ "error": msg.into() })))
}

fn client_auth_bucket(headers: &HeaderMap) -> String {
    headers
        .get("x-forwarded-for")
        .or_else(|| headers.get("x-real-ip"))
        .or_else(|| headers.get("host"))
        .and_then(|v| v.to_str().ok())
        .unwrap_or("local")
        .trim()
        .to_string()
}

pub fn check_admin_auth(engine: &AegisMeshEngine, headers: &HeaderMap) -> Result<(), ApiError> {
    if !engine.config.has_admin_key() {
        return Ok(());
    }

    let bucket = client_auth_bucket(headers);
    let now = Instant::now();
    {
        let mut map = engine.auth_failures.lock();
        if map.len() > 1024 {
            map.retain(|_, (_, ws)| now.duration_since(*ws).as_secs() < 120);
        }
        if let Some((fails, ws)) = map.get(&bucket) {
            if now.duration_since(*ws).as_secs() < 60 && *fails >= 5 {
                return Err(err_resp(
                    StatusCode::TOO_MANY_REQUESTS,
                    "Too many failed Admin Key attempts. Locked for 60 seconds.",
                ));
            }
        }
    }

    let supplied = headers
        .get("x-admin-key")
        .or_else(|| headers.get("authorization"))
        .and_then(|v| v.to_str().ok())
        .map(|s| s.strip_prefix("Bearer ").unwrap_or(s).trim());

    if engine.config.verify_admin_key(supplied) {
        engine.auth_failures.lock().remove(&bucket);
        Ok(())
    } else {
        let mut map = engine.auth_failures.lock();
        let entry = map.entry(bucket).or_insert((0, now));
        if now.duration_since(entry.1).as_secs() >= 60 {
            entry.0 = 1;
            entry.1 = now;
        } else {
            entry.0 += 1;
        }
        Err(err_resp(
            StatusCode::UNAUTHORIZED,
            "Unauthorized: valid Admin Key required via X-Admin-Key header.",
        ))
    }
}

fn is_allowed_origin(origin: &str) -> bool {
    let trimmed = origin.trim();
    if trimmed.starts_with("http://127.0.0.1:")
        || trimmed.starts_with("http://localhost:")
        || trimmed == "http://127.0.0.1"
        || trimmed == "http://localhost"
    {
        return true;
    }

    // Google Cloud Run HTTPS service origins (strict host parsing, no path/userinfo tricks)
    if let Some(rest) = trimmed.strip_prefix("https://") {
        if !rest.contains('/') && !rest.contains('@') && !rest.contains('?') {
            let host = rest.split(':').next().unwrap_or("");
            if (host.ends_with(".run.app") || host.ends_with(".a.run.app"))
                && host.len() > ".run.app".len()
            {
                return true;
            }
        }
    }

    if let Ok(extra) = std::env::var("AEGIS_ALLOWED_ORIGINS") {
        return extra
            .split(',')
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .any(|v| v == trimmed);
    }
    false
}

pub fn create_router(engine: Arc<AegisMeshEngine>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::predicate(|origin, _req| {
            is_allowed_origin(origin.to_str().unwrap_or(""))
        }))
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::OPTIONS])
        .allow_headers([
            header::CONTENT_TYPE,
            header::AUTHORIZATION,
            HeaderName::from_static("x-admin-key"),
            HeaderName::from_static("x-aegis-mode"),
        ]);

    Router::new()
        // Core Health & Gateway Compatibility
        .route("/health", get(gateway::health_handler))
        .route("/v1/shield/status", get(gateway::get_shield_status))
        .route("/v1/shield/toggle", post(gateway::toggle_shield))
        .route("/v1/shield/inspect", post(gateway::inspect_payload))
        .route("/v1/chat/completions", post(gateway::proxy_chat_completions))
        // Auth verification
        .route("/api/v1/auth/verify", post(settings::verify_admin_auth_endpoint))
        // Customers & Direct Ledger Actions
        .route("/api/v1/customers", get(customers::list_customers))
        .route("/api/v1/customers/:id", get(customers::get_customer_detail))
        .route(
            "/api/v1/customers/:id/manual-action",
            post(customers::manual_staff_action),
        )
        // Customer Chats & Staff Override Center
        .route(
            "/api/v1/sessions",
            get(sessions::list_sessions).post(sessions::create_session),
        )
        .route("/api/v1/sessions/:id", get(sessions::get_session_detail))
        .route(
            "/api/v1/sessions/:id/message",
            post(sessions::post_customer_message),
        )
        .route(
            "/api/v1/sessions/:id/takeover",
            post(sessions::toggle_takeover),
        )
        .route(
            "/api/v1/sessions/:id/staff-reply",
            post(sessions::post_staff_reply),
        )
        .route(
            "/api/v1/sessions/:id/quarantine",
            post(sessions::toggle_quarantine),
        )
        .route(
            "/api/v1/sessions/:id/reveal-pii",
            post(sessions::reveal_session_pii),
        )
        .route(
            "/api/v1/sessions/:id/purge-pii",
            post(sessions::purge_session_pii),
        )
        // Maker-Checker Override Queue
        .route("/api/v1/overrides", get(sessions::list_overrides))
        .route(
            "/api/v1/overrides/:id/decision",
            post(sessions::decide_override),
        )
        // Telemetry & Async Sentinel
        .route("/api/v1/telemetry/stats", get(telemetry::get_stats))
        .route("/api/v1/telemetry/history", get(telemetry::get_history))
        .route("/api/v1/telemetry/stream", get(telemetry::stream_events))
        .route("/v1/telemetry/events", get(telemetry::stream_events))
        .route("/v1/telemetry/stats", get(telemetry::get_stats))
        .route("/v1/telemetry/history", get(telemetry::get_history))
        .route("/api/v1/sentinel/state", get(telemetry::get_sentinel_state))
        .route(
            "/api/v1/sentinel/alerts/:id/ack",
            post(telemetry::ack_sentinel_alert),
        )
        .route("/api/v1/audit/export", get(telemetry::export_audit_report))
        .route("/api/v1/staff-logs", get(telemetry::get_staff_logs))
        // Policy & Live Settings
        .route(
            "/api/v1/policy",
            get(settings::get_policy).put(settings::update_policy),
        )
        .route("/api/v1/policy/preset", post(settings::apply_preset))
        .route(
            "/api/v1/settings",
            get(settings::get_settings).put(settings::update_settings),
        )
        // Embedded Svelte 5 Frontend + SPA Fallback
        .fallback(static_handler)
        .layer(DefaultBodyLimit::max(256 * 1024))
        .layer(TimeoutLayer::new(Duration::from_secs(30)))
        .layer(SetResponseHeaderLayer::overriding(
            header::X_FRAME_OPTIONS,
            HeaderValue::from_static("DENY"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::REFERRER_POLICY,
            HeaderValue::from_static("no-referrer"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_static("cross-origin-resource-policy"),
            HeaderValue::from_static("same-origin"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_static("permissions-policy"),
            HeaderValue::from_static("camera=(), microphone=(), geolocation=()"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::CONTENT_SECURITY_POLICY,
            HeaderValue::from_static(
                "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline' https://fonts.googleapis.com; font-src 'self' https://fonts.gstatic.com data:; img-src 'self' data:; connect-src 'self';",
            ),
        ))
        .layer(cors)
        .with_state(engine)
}

pub async fn ok_json(val: Value) -> impl IntoResponse {
    Json(val)
}
