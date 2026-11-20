use std::sync::Arc;
use std::time::Instant;

use axum::{
    extract::State,
    http::HeaderMap,
    response::{IntoResponse, Json},
};
use serde::Deserialize;
use serde_json::{json, Value};

use aegismesh_core::config::TokenFormat;
use aegismesh_core::layer4_breaker::{AccountContext, CircuitBreakerEvaluator, ToolCall};
use aegismesh_core::pipeline::{AegisMeshEngine, PipelineRequest};

use super::{check_admin_auth, ApiError};

#[derive(Debug, Deserialize)]
pub struct ToggleShieldRequest {
    pub active: bool,
    pub officer: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct InspectRequest {
    pub text: Option<String>,
    pub tool_call: Option<Value>,
    pub account_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ChatMessageIn {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
pub struct ChatCompletionIn {
    pub messages: Option<Vec<ChatMessageIn>>,
    pub prompt: Option<String>,
    pub session_id: Option<String>,
    pub account_id: Option<String>,
    pub tools_called: Option<Vec<ToolCall>>,
    pub tool_call: Option<ToolCall>,
    pub token_format: Option<String>,
    pub persist_chat: Option<bool>,
}

pub async fn health_handler(State(engine): State<Arc<AegisMeshEngine>>) -> impl IntoResponse {
    let cfg = engine.config.snapshot();
    Json(json!({
        "status": "ok",
        "service": "AegisMesh Security Gateway",
        "version": env!("CARGO_PKG_VERSION"),
        "shield_active": engine.is_shield_active(),
        "gemini_configured": cfg.google_ai.has_live_key(),
        "runtime": "Rust Tokio/Axum Native"
    }))
}

pub async fn get_shield_status(State(engine): State<Arc<AegisMeshEngine>>) -> impl IntoResponse {
    let cfg = engine.config.snapshot();
    let stats = engine.telemetry.get_stats();
    let pending_overrides = engine.state_store.count_pending_overrides().unwrap_or(0);

    Json(json!({
        "shield_active": engine.is_shield_active(),
        "policy_preset": cfg.firewall.policy_preset,
        "decision_model": cfg.google_ai.decision_model,
        "decisions_mode": cfg.google_ai.decisions_mode,
        "agent_model": cfg.google_ai.agent_model,
        "gemini_configured": cfg.google_ai.has_live_key(),
        "admin_key_required": engine.config.has_admin_key(),
        "started_at": engine.started_at,
        "pending_overrides": pending_overrides,
        "metrics": stats,
    }))
}

pub async fn toggle_shield(
    State(engine): State<Arc<AegisMeshEngine>>,
    headers: HeaderMap,
    Json(req): Json<ToggleShieldRequest>,
) -> Result<impl IntoResponse, ApiError> {
    check_admin_auth(&engine, &headers)?;
    engine.set_shield_active(req.active);
    let officer = req.officer.unwrap_or_else(|| "Security Officer".into());
    let _ = engine.state_store.log_staff_action(
        &officer,
        if req.active { "ENABLE_SHIELD" } else { "BYPASS_SHIELD" },
        "GATEWAY",
        if req.active {
            "AegisMesh 6-layer shield activated."
        } else {
            "AegisMesh shield bypassed for unprotected comparison."
        },
    );

    Ok(Json(json!({
        "success": true,
        "shield_active": req.active,
        "message": format!("AegisMesh Shield is now {}", if req.active { "ACTIVE" } else { "BYPASSED" })
    })))
}

pub async fn inspect_payload(
    State(engine): State<Arc<AegisMeshEngine>>,
    Json(req): Json<InspectRequest>,
) -> impl IntoResponse {
    let t0 = Instant::now();
    let cfg = engine.config.snapshot();
    let mut res = json!({ "overhead_ms": 0.0 });

    if let Some(ref text) = req.text {
        let safety = engine.layer1.inspect(text);
        let pii = engine
            .tokenizer
            .tokenize_with_config(text, "inspect_ephemeral", &cfg.dpdp_act, None);
        let decision = engine
            .decision_client
            .evaluate_prompt(&pii.sanitized_text, &cfg.google_ai)
            .await;

        res["safety"] = json!(safety);
        res["gemini_decision"] = json!(decision);
        res["pii"] = json!({
            "has_pii": pii.has_pii,
            "counts": pii.pii_counts,
            "sanitized_text": pii.sanitized_text,
            "aadhaar_verhoeff_verified": pii.aadhaar_verhoeff_verified,
        });
    }

    if let Some(ref tc_val) = req.tool_call {
        let name = tc_val
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let args = tc_val.get("args").cloned();
        let tc = ToolCall { name, args };
        let ctx = req.account_id.as_ref().map(|acc| AccountContext {
            account_id: Some(acc.clone()),
            ..Default::default()
        });
        let cb = CircuitBreakerEvaluator::new().evaluate(
            &tc,
            &cfg.statutory_limits,
            ctx.as_ref(),
            Some(&engine.moneymax_db),
            Some(&engine.state_store),
        );
        res["circuit_breaker"] = json!(cb);
    }

    let overhead = ((t0.elapsed().as_micros() as f64) / 1000.0 * 1000.0).round() / 1000.0;
    res["overhead_ms"] = json!(overhead);
    Json(res)
}

pub async fn proxy_chat_completions(
    State(engine): State<Arc<AegisMeshEngine>>,
    headers: HeaderMap,
    Json(req): Json<ChatCompletionIn>,
) -> Result<impl IntoResponse, ApiError> {
    let raw_prompt = if let Some(ref msgs) = req.messages {
        msgs.iter()
            .rev()
            .find(|m| m.role.eq_ignore_ascii_case("user"))
            .map(|m| m.content.clone())
            .unwrap_or_default()
    } else {
        req.prompt.clone().unwrap_or_default()
    };

    // Honor X-Aegis-Mode only if authenticated when disabling shield
    let mut shield_override: Option<bool> = None;
    if let Some(hdr) = headers.get("x-aegis-mode").and_then(|v| v.to_str().ok()) {
        let v = hdr.trim().to_lowercase();
        if v == "on" || v == "true" || v == "1" {
            shield_override = Some(true);
        } else if (v == "off" || v == "false" || v == "0")
            && check_admin_auth(&engine, &headers).is_ok()
        {
            shield_override = Some(false);
        }
    }

    let mut explicit_tools = req.tools_called.clone().unwrap_or_default();
    if let Some(single) = req.tool_call {
        explicit_tools.push(single);
    }

    if !explicit_tools.is_empty() {
        check_admin_auth(&engine, &headers)?;
    }

    let token_format = req.token_format.as_deref().map(|f| {
        if f.to_lowercase().contains("bracket") {
            TokenFormat::BracketIndexed
        } else {
            TokenFormat::AngleUuid
        }
    });

    let outcome = engine
        .process_request(PipelineRequest {
            prompt: raw_prompt,
            session_id: req.session_id,
            account_id: req.account_id,
            client_ip: Some("127.0.0.1".into()),
            explicit_tools: if explicit_tools.is_empty() {
                None
            } else {
                Some(explicit_tools)
            },
            token_format,
            shield_override,
            persist_chat: req.persist_chat.unwrap_or(true),
        })
        .await;

    Ok(Json(outcome))
}
