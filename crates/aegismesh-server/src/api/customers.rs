use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Json},
};
use chrono::{Local, Utc};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use aegismesh_core::pipeline::AegisMeshEngine;
use aegismesh_core::telemetry::bus::TelemetryEvent;

use super::{check_admin_auth, err_resp, ApiError};

#[derive(Debug, Deserialize)]
pub struct ManualStaffActionReq {
    pub action: String, // "REFUND" | "UPDATE_KYC" | "UPDATE_CREDIT_LIMIT" | "FREEZE" | "UNFREEZE"
    pub officer: String,
    pub reason: Option<String>,
    pub amount: Option<f64>,
    pub txn_id: Option<String>,
    pub kyc_status: Option<String>,
    pub new_limit: Option<f64>,
    pub session_id: Option<String>,
}

pub async fn list_customers(State(engine): State<Arc<AegisMeshEngine>>) -> Result<impl IntoResponse, ApiError> {
    let list = engine
        .moneymax_db
        .list_customers()
        .map_err(|e| err_resp(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(json!({ "customers": list })))
}

pub async fn get_customer_detail(
    State(engine): State<Arc<AegisMeshEngine>>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let profile = engine
        .moneymax_db
        .get_customer_profile(&id)
        .map_err(|e| err_resp(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| err_resp(StatusCode::NOT_FOUND, format!("Customer {} not found", id)))?;

    let txns = engine
        .moneymax_db
        .get_transaction_history(&profile.account_id, 15)
        .map_err(|e| err_resp(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let disputes = engine
        .moneymax_db
        .get_disputes(&profile.account_id)
        .map_err(|e| err_resp(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let db_logs = engine
        .moneymax_db
        .get_recent_audit_logs(Some(&profile.account_id), 15)
        .map_err(|e| err_resp(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({
        "profile": profile.masked(),
        "transactions": txns,
        "disputes": disputes,
        "audit_logs": db_logs
    })))
}

pub async fn manual_staff_action(
    State(engine): State<Arc<AegisMeshEngine>>,
    headers: HeaderMap,
    Path(account_id): Path<String>,
    Json(req): Json<ManualStaffActionReq>,
) -> Result<impl IntoResponse, ApiError> {
    check_admin_auth(&engine, &headers)?;
    let officer = if req.officer.trim().is_empty() {
        "Compliance Officer".to_string()
    } else {
        req.officer.trim().to_string()
    };
    let actor = format!("STAFF_MANUAL:{}", officer);
    let reason = req
        .reason
        .clone()
        .unwrap_or_else(|| "Manual staff console action".to_string());

    let result = match req.action.trim().to_uppercase().as_str() {
        "REFUND" => {
            let amt = req
                .amount
                .filter(|a| a.is_finite() && *a >= 1.0)
                .ok_or_else(|| err_resp(StatusCode::BAD_REQUEST, "Valid refund amount >= ₹1.00 required"))?;
            let txn = req
                .txn_id
                .as_deref()
                .unwrap_or("MANUAL-STAFF-REFUND");
            let out = engine
                .moneymax_db
                .execute_refund(&account_id, txn, amt, &reason, &actor, true)
                .map_err(|e| err_resp(StatusCode::BAD_REQUEST, e.to_string()))?;
            json!({ "success": true, "refund": out })
        }
        "UPDATE_KYC" => {
            let st = req
                .kyc_status
                .as_deref()
                .ok_or_else(|| err_resp(StatusCode::BAD_REQUEST, "kyc_status required"))?;
            engine
                .moneymax_db
                .update_kyc_status(&account_id, st, &reason, &actor)
                .map_err(|e| err_resp(StatusCode::BAD_REQUEST, e.to_string()))?;
            json!({ "success": true, "account_id": account_id, "kyc_status": st.to_uppercase() })
        }
        "UPDATE_CREDIT_LIMIT" => {
            let lim = req
                .new_limit
                .filter(|l| l.is_finite() && *l > 0.0)
                .ok_or_else(|| err_resp(StatusCode::BAD_REQUEST, "Valid positive credit limit required"))?;
            engine
                .moneymax_db
                .update_credit_limit(&account_id, lim, &reason, &actor)
                .map_err(|e| err_resp(StatusCode::BAD_REQUEST, e.to_string()))?;
            json!({ "success": true, "account_id": account_id, "new_limit": lim })
        }
        "FREEZE" => {
            engine
                .moneymax_db
                .set_account_frozen(&account_id, true, &reason, &actor)
                .map_err(|e| err_resp(StatusCode::BAD_REQUEST, e.to_string()))?;
            json!({ "success": true, "account_id": account_id, "is_frozen": true })
        }
        "UNFREEZE" => {
            engine
                .moneymax_db
                .set_account_frozen(&account_id, false, &reason, &actor)
                .map_err(|e| err_resp(StatusCode::BAD_REQUEST, e.to_string()))?;
            json!({ "success": true, "account_id": account_id, "is_frozen": false })
        }
        other => {
            return Err(err_resp(
                StatusCode::BAD_REQUEST,
                format!("Unsupported manual staff action: {}", other),
            ));
        }
    };

    let _ = engine.state_store.log_staff_action(
        &officer,
        &format!("MANUAL_{}", req.action.to_uppercase()),
        &account_id,
        &reason,
    );

    if let Some(ref sid) = req.session_id {
        let _ = engine.state_store.append_message(
            &format!("MSG-STF-{}", Uuid::new_v4().as_simple().to_string()[..8].to_uppercase()),
            sid,
            "system",
            &format!("[Staff Manual Action by {}]: {} ({})", officer, req.action.to_uppercase(), reason),
            &format!("[Staff Manual Action by {}]: {} ({})", officer, req.action.to_uppercase(), reason),
            &HashMap::new(),
            None,
            Some(&result),
            Some("STAFF_MANUAL"),
            Some("APPROVED"),
            0.0,
        );
    }

    engine.telemetry.record_event(TelemetryEvent {
        event_id: format!("STF-{}", Uuid::new_v4().as_simple().to_string()[..10].to_uppercase()),
        session_id: req.session_id,
        account_id: Some(account_id.clone()),
        timestamp: Utc::now().timestamp_millis() as f64 / 1000.0,
        iso_time: Local::now().format("%H:%M:%S").to_string(),
        event_type: "STAFF_OVERRIDE".into(),
        target_model: "HUMAN_OFFICER".into(),
        decision_source: Some(officer),
        overhead_ms: 0.0,
        user_prompt_sanitized: format!("Manual staff action: {}", req.action.to_uppercase()),
        pii_masked: HashMap::new(),
        attack_type: None,
        tool_name: Some(req.action.to_lowercase()),
        tool_args: Some(result.clone()),
        violation_code: None,
        reason: Some(reason),
        layers: vec![],
    });

    Ok(Json(result))
}
