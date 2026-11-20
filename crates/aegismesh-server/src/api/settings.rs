use std::sync::Arc;

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Json},
};
use serde::Deserialize;
use serde_json::json;

use aegismesh_core::config::{PolicyConfig, SettingsPatch};
use aegismesh_core::pipeline::AegisMeshEngine;

use super::{check_admin_auth, err_resp, ApiError};

#[derive(Debug, Deserialize)]
pub struct ApplyPresetReq {
    pub preset: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePolicyReq {
    pub policy_preset: Option<String>,
    pub statutory_limits: PolicyConfig,
}

pub async fn verify_admin_auth_endpoint(
    State(engine): State<Arc<AegisMeshEngine>>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, ApiError> {
    check_admin_auth(&engine, &headers)?;
    Ok(Json(json!({
        "valid": true,
        "admin_key_required": engine.config.has_admin_key(),
    })))
}

pub async fn get_policy(State(engine): State<Arc<AegisMeshEngine>>) -> impl IntoResponse {
    let cfg = engine.config.snapshot();
    Json(json!({
        "policy_preset": cfg.firewall.policy_preset,
        "statutory_limits": cfg.statutory_limits,
    }))
}

pub async fn update_policy(
    State(engine): State<Arc<AegisMeshEngine>>,
    headers: HeaderMap,
    Json(req): Json<UpdatePolicyReq>,
) -> Result<impl IntoResponse, ApiError> {
    check_admin_auth(&engine, &headers)?;
    let preset = req
        .policy_preset
        .unwrap_or_else(|| "CUSTOM".to_string());
    engine.config.update_policy(preset.clone(), req.statutory_limits.clone());

    let cfg = engine.config.snapshot();
    if let Ok(val) = serde_json::to_value(&cfg) {
        let _ = engine.state_store.save_kv_setting("aegis_config", &val);
    }
    let _ = engine.state_store.log_staff_action(
        "Admin",
        "UPDATE_POLICY",
        &preset,
        &format!(
            "Updated RBI cap=₹{:.2}, 24h cap=₹{:.2}, credit_mult={:.2}x",
            req.statutory_limits.max_auto_refund_amount,
            req.statutory_limits.max_24h_refund_total,
            req.statutory_limits.max_credit_limit_multiplier
        ),
    );

    Ok(Json(json!({
        "success": true,
        "policy_preset": preset,
        "statutory_limits": req.statutory_limits
    })))
}

pub async fn apply_preset(
    State(engine): State<Arc<AegisMeshEngine>>,
    headers: HeaderMap,
    Json(req): Json<ApplyPresetReq>,
) -> Result<impl IntoResponse, ApiError> {
    check_admin_auth(&engine, &headers)?;
    let preset = req.preset.trim().to_uppercase();
    let policy = match preset.as_str() {
        "RBI_DPDP_FINTECH_V1" => PolicyConfig::rbi_dpdp_default(),
        "STRICT_BANKING" => PolicyConfig::strict_banking(),
        other => {
            return Err(err_resp(
                StatusCode::BAD_REQUEST,
                format!("Unknown policy preset: {}", other),
            ));
        }
    };
    engine.config.update_policy(preset.clone(), policy.clone());
    let cfg = engine.config.snapshot();
    if let Ok(val) = serde_json::to_value(&cfg) {
        let _ = engine.state_store.save_kv_setting("aegis_config", &val);
    }

    Ok(Json(json!({
        "success": true,
        "policy_preset": preset,
        "statutory_limits": policy
    })))
}

pub async fn get_settings(State(engine): State<Arc<AegisMeshEngine>>) -> impl IntoResponse {
    Json(engine.config.public_view())
}

pub async fn update_settings(
    State(engine): State<Arc<AegisMeshEngine>>,
    headers: HeaderMap,
    Json(patch): Json<SettingsPatch>,
) -> Result<impl IntoResponse, ApiError> {
    check_admin_auth(&engine, &headers)?;
    let updated = engine.config.apply_patch(patch);
    if let Ok(val) = serde_json::to_value(&updated) {
        let _ = engine.state_store.save_kv_setting("aegis_config", &val);
    }
    let _ = engine.state_store.log_staff_action(
        "Admin",
        "UPDATE_SETTINGS",
        "CONFIG",
        "Updated AegisMesh runtime configuration via SaaS Settings.",
    );
    Ok(Json(json!({
        "success": true,
        "settings": engine.config.public_view()
    })))
}
