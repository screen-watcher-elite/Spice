use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Json},
};
use chrono::{Local, Utc};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use aegismesh_core::agent::finbot::execute_moneymax_tool;
use aegismesh_core::db::state_store::SessionStatus;
use aegismesh_core::pipeline::{AegisMeshEngine, PipelineRequest};
use aegismesh_core::telemetry::bus::TelemetryEvent;

use super::{check_admin_auth, err_resp, ApiError};

#[derive(Debug, Deserialize)]
pub struct SessionsQuery {
    pub status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateSessionReq {
    pub account_id: String,
    pub session_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CustomerMessageReq {
    pub content: String,
}

#[derive(Debug, Deserialize)]
pub struct TakeoverReq {
    pub takeover: bool,
    pub officer: String,
}

#[derive(Debug, Deserialize)]
pub struct StaffReplyReq {
    pub officer: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
pub struct QuarantineReq {
    pub quarantined: bool,
    pub officer: String,
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RevealPiiReq {
    pub officer: String,
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct OverridesQuery {
    pub session_id: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct OverrideDecisionReq {
    pub action: String, // "APPROVE" | "REJECT" | "MODIFY_APPROVE"
    pub officer: String,
    pub notes: Option<String>,
    pub modified_args: Option<Value>,
}

pub async fn list_sessions(
    State(engine): State<Arc<AegisMeshEngine>>,
    Query(q): Query<SessionsQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let sessions = engine
        .state_store
        .list_sessions(q.status.as_deref())
        .map_err(|e| err_resp(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(json!({ "sessions": sessions })))
}

pub async fn create_session(
    State(engine): State<Arc<AegisMeshEngine>>,
    Json(req): Json<CreateSessionReq>,
) -> Result<impl IntoResponse, ApiError> {
    let profile = engine
        .moneymax_db
        .get_customer_profile(&req.account_id)
        .map_err(|e| err_resp(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| err_resp(StatusCode::NOT_FOUND, format!("Account {} not found", req.account_id)))?;

    let sid = req.session_id.unwrap_or_else(|| {
        format!(
            "SESS-{}-{}",
            profile.account_id.replace("MMX-AC-", ""),
            Uuid::new_v4().as_simple().to_string()[..4].to_uppercase()
        )
    });

    let sess = engine
        .state_store
        .create_or_get_session(&sid, &profile.account_id, &profile.full_name)
        .map_err(|e| err_resp(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let _ = engine.state_store.append_message(
        &format!("MSG-INIT-{}", Uuid::new_v4().as_simple().to_string()[..8].to_uppercase()),
        &sid,
        "system",
        &format!("Started monitored support session for {} ({}).", profile.full_name, profile.account_id),
        &format!("Started monitored support session for {} ({}).", profile.full_name, profile.account_id),
        &HashMap::new(),
        None,
        None,
        None,
        None,
        0.0,
    );

    Ok(Json(json!({ "session": sess })))
}

pub async fn get_session_detail(
    State(engine): State<Arc<AegisMeshEngine>>,
    Path(session_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let session = engine
        .state_store
        .get_session(&session_id)
        .map_err(|e| err_resp(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| err_resp(StatusCode::NOT_FOUND, "Session not found"))?;

    let messages = engine
        .state_store
        .get_session_messages(&session_id, false)
        .map_err(|e| err_resp(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let overrides = engine
        .state_store
        .list_overrides(Some(&session_id), None)
        .map_err(|e| err_resp(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let customer = engine
        .moneymax_db
        .get_customer_profile(&session.account_id)
        .ok()
        .flatten()
        .map(|c| c.masked());
    let transactions = engine
        .moneymax_db
        .get_transaction_history(&session.account_id, 10)
        .unwrap_or_default();
    let disputes = engine
        .moneymax_db
        .get_disputes(&session.account_id)
        .unwrap_or_default();

    Ok(Json(json!({
        "session": session,
        "messages": messages,
        "overrides": overrides,
        "customer": customer,
        "transactions": transactions,
        "disputes": disputes,
    })))
}

pub async fn post_customer_message(
    State(engine): State<Arc<AegisMeshEngine>>,
    Path(session_id): Path<String>,
    Json(req): Json<CustomerMessageReq>,
) -> Result<impl IntoResponse, ApiError> {
    let session = engine
        .state_store
        .get_session(&session_id)
        .map_err(|e| err_resp(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| err_resp(StatusCode::NOT_FOUND, "Session not found"))?;

    let status = SessionStatus::from_db(&session.status);

    // If staff has taken over the chat, sanitize & append customer message WITHOUT invoking AI agent!
    if status == SessionStatus::StaffTakeover {
        let cfg = engine.config.snapshot();
        let tok = engine
            .tokenizer
            .tokenize_with_config(&req.content, &session_id, &cfg.dpdp_act, None);
        let msg = engine
            .state_store
            .append_message(
                &format!("MSG-U-{}", Uuid::new_v4().as_simple().to_string()[..8].to_uppercase()),
                &session_id,
                "customer",
                &tok.sanitized_text,
                &req.content,
                &tok.pii_counts,
                None,
                None,
                Some("STAFF_TAKEOVER_MODE"),
                None,
                0.0,
            )
            .map_err(|e| err_resp(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        return Ok(Json(json!({
            "mode": "STAFF_TAKEOVER",
            "message": msg,
            "note": "Session is currently under human staff control; AI model paused."
        })));
    }

    if status == SessionStatus::Quarantined {
        return Err(err_resp(
            StatusCode::LOCKED,
            "Session is QUARANTINED by AegisMesh Sentinel/Staff. Release quarantine before resuming.",
        ));
    }

    let outcome = engine
        .process_request(PipelineRequest {
            prompt: req.content,
            session_id: Some(session_id.clone()),
            account_id: Some(session.account_id),
            client_ip: Some("127.0.0.1".into()),
            explicit_tools: None,
            token_format: None,
            shield_override: None,
            persist_chat: true,
        })
        .await;

    Ok(Json(json!({
        "mode": "AI_PIPELINE",
        "outcome": outcome
    })))
}

pub async fn toggle_takeover(
    State(engine): State<Arc<AegisMeshEngine>>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
    Json(req): Json<TakeoverReq>,
) -> Result<impl IntoResponse, ApiError> {
    check_admin_auth(&engine, &headers)?;
    let _ = engine
        .state_store
        .get_session(&session_id)
        .map_err(|e| err_resp(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| err_resp(StatusCode::NOT_FOUND, "Session not found"))?;

    let new_status = if req.takeover {
        SessionStatus::StaffTakeover
    } else {
        SessionStatus::AiActive
    };

    engine
        .state_store
        .update_session_status(&session_id, new_status, Some(&req.officer), None)
        .map_err(|e| err_resp(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let note = if req.takeover {
        format!("Staff Officer {} took over live chat control (AI paused).", req.officer)
    } else {
        format!("Staff Officer {} handed session back to FinBot AI.", req.officer)
    };

    let _ = engine.state_store.append_message(
        &format!("MSG-SYS-{}", Uuid::new_v4().as_simple().to_string()[..8].to_uppercase()),
        &session_id,
        "system",
        &note,
        &note,
        &HashMap::new(),
        None,
        None,
        Some("STAFF_CONTROL"),
        None,
        0.0,
    );

    let _ = engine.state_store.log_staff_action(
        &req.officer,
        if req.takeover { "TAKEOVER_CHAT" } else { "RELEASE_TAKEOVER" },
        &session_id,
        &note,
    );

    Ok(Json(json!({
        "success": true,
        "session_id": session_id,
        "status": new_status.as_str(),
        "officer": req.officer
    })))
}

pub async fn post_staff_reply(
    State(engine): State<Arc<AegisMeshEngine>>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
    Json(req): Json<StaffReplyReq>,
) -> Result<impl IntoResponse, ApiError> {
    check_admin_auth(&engine, &headers)?;
    let session = engine
        .state_store
        .get_session(&session_id)
        .map_err(|e| err_resp(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| err_resp(StatusCode::NOT_FOUND, "Session not found"))?;

    let cfg = engine.config.snapshot();
    let tok = engine
        .tokenizer
        .tokenize_with_config(&req.content, &session_id, &cfg.dpdp_act, None);

    let msg = engine
        .state_store
        .append_message(
            &format!("MSG-STF-{}", Uuid::new_v4().as_simple().to_string()[..8].to_uppercase()),
            &session_id,
            "staff",
            &tok.sanitized_text,
            &req.content,
            &tok.pii_counts,
            None,
            None,
            Some(&format!("STAFF:{}", req.officer)),
            Some("HUMAN_VERIFIED"),
            0.0,
        )
        .map_err(|e| err_resp(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let _ = engine.state_store.log_staff_action(
        &req.officer,
        "STAFF_CHAT_REPLY",
        &session_id,
        "Sent manual staff reply to customer session.",
    );

    engine.telemetry.record_event(TelemetryEvent {
        event_id: format!("STF-{}", Uuid::new_v4().as_simple().to_string()[..10].to_uppercase()),
        session_id: Some(session_id),
        account_id: Some(session.account_id),
        timestamp: Utc::now().timestamp_millis() as f64 / 1000.0,
        iso_time: Local::now().format("%H:%M:%S").to_string(),
        event_type: "STAFF_OVERRIDE".into(),
        target_model: "HUMAN_OFFICER".into(),
        decision_source: Some(req.officer),
        overhead_ms: 0.0,
        user_prompt_sanitized: tok.sanitized_text,
        pii_masked: tok.pii_counts,
        attack_type: None,
        tool_name: None,
        tool_args: None,
        violation_code: None,
        reason: Some("Manual staff response dispatched.".into()),
        layers: vec![],
    });

    Ok(Json(json!({ "success": true, "message": msg })))
}

pub async fn toggle_quarantine(
    State(engine): State<Arc<AegisMeshEngine>>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
    Json(req): Json<QuarantineReq>,
) -> Result<impl IntoResponse, ApiError> {
    check_admin_auth(&engine, &headers)?;
    let _ = engine
        .state_store
        .get_session(&session_id)
        .map_err(|e| err_resp(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| err_resp(StatusCode::NOT_FOUND, "Session not found"))?;

    let new_status = if req.quarantined {
        SessionStatus::Quarantined
    } else {
        SessionStatus::AiActive
    };

    engine
        .state_store
        .update_session_status(&session_id, new_status, Some(&req.officer), None)
        .map_err(|e| err_resp(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let reason = req.reason.unwrap_or_else(|| "Security review".into());
    let note = if req.quarantined {
        format!("Session QUARANTINED by {}: {}", req.officer, reason)
    } else {
        format!("Quarantine LIFTED by {}: {}", req.officer, reason)
    };

    let _ = engine.state_store.append_message(
        &format!("MSG-Q-{}", Uuid::new_v4().as_simple().to_string()[..8].to_uppercase()),
        &session_id,
        "system",
        &note,
        &note,
        &HashMap::new(),
        None,
        None,
        Some("QUARANTINE_CONTROL"),
        None,
        0.0,
    );

    let _ = engine.state_store.log_staff_action(
        &req.officer,
        if req.quarantined { "QUARANTINE_SESSION" } else { "RELEASE_QUARANTINE" },
        &session_id,
        &reason,
    );

    Ok(Json(json!({
        "success": true,
        "session_id": session_id,
        "status": new_status.as_str()
    })))
}

pub async fn reveal_session_pii(
    State(engine): State<Arc<AegisMeshEngine>>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
    Json(req): Json<RevealPiiReq>,
) -> Result<impl IntoResponse, ApiError> {
    check_admin_auth(&engine, &headers)?;

    let msgs = engine
        .state_store
        .get_session_messages(&session_id, true)
        .map_err(|e| err_resp(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let reason = req
        .reason
        .unwrap_or_else(|| "Staff compliance inspection".into());
    let _ = engine.state_store.log_staff_action(
        &req.officer,
        "REVEAL_MASKED_PII",
        &session_id,
        &format!("Authorized DPDP PII unmasking: {}", reason),
    );

    Ok(Json(json!({
        "success": true,
        "session_id": session_id,
        "audit_logged": true,
        "messages": msgs
    })))
}

pub async fn purge_session_pii(
    State(engine): State<Arc<AegisMeshEngine>>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
    Json(req): Json<RevealPiiReq>,
) -> Result<impl IntoResponse, ApiError> {
    check_admin_auth(&engine, &headers)?;
    let count = engine
        .state_store
        .purge_session_unmasked_pii(&session_id)
        .map_err(|e| err_resp(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let _ = engine.state_store.log_staff_action(
        &req.officer,
        "PURGE_SESSION_PII",
        &session_id,
        &format!("DPDP right-to-erasure purge wiped raw PII across {} message(s).", count),
    );
    Ok(Json(json!({
        "success": true,
        "session_id": session_id,
        "purged_messages": count
    })))
}

pub async fn list_overrides(
    State(engine): State<Arc<AegisMeshEngine>>,
    Query(q): Query<OverridesQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let overrides = engine
        .state_store
        .list_overrides(q.session_id.as_deref(), q.status.as_deref())
        .map_err(|e| err_resp(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(json!({ "overrides": overrides })))
}

pub async fn decide_override(
    State(engine): State<Arc<AegisMeshEngine>>,
    headers: HeaderMap,
    Path(override_id): Path<String>,
    Json(req): Json<OverrideDecisionReq>,
) -> Result<impl IntoResponse, ApiError> {
    check_admin_auth(&engine, &headers)?;

    let ov = engine
        .state_store
        .get_override(&override_id)
        .map_err(|e| err_resp(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| err_resp(StatusCode::NOT_FOUND, "Override record not found"))?;

    let claimed = engine
        .state_store
        .claim_override_for_decision(&override_id)
        .map_err(|e| err_resp(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if !claimed {
        return Err(err_resp(
            StatusCode::CONFLICT,
            format!("Override {} already claimed or resolved (status: {})", override_id, ov.status),
        ));
    }

    let action = req.action.trim().to_uppercase();
    let notes = req.notes.unwrap_or_else(|| "Staff Maker-Checker decision".into());
    let officer = if req.officer.trim().is_empty() {
        "Compliance Officer".into()
    } else {
        req.officer.trim().to_string()
    };

    let (new_status, exec_res) = match action.as_str() {
        "APPROVE" | "MODIFY_APPROVE" => {
            let mut final_args = req.modified_args.unwrap_or_else(|| ov.tool_args.clone());
            // Pin account_id strictly to original override target (prevent account hijacking)
            if let Some(obj) = final_args.as_object_mut() {
                obj.insert("account_id".to_string(), json!(ov.account_id));
                if ov.tool_name == "execute_refund" {
                    let amt = obj.get("amount").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    if !amt.is_finite() || amt < 1.0 {
                        let _ = engine.state_store.resolve_override(&override_id, "PENDING", "", "", None);
                        return Err(err_resp(
                            StatusCode::BAD_REQUEST,
                            "Modified refund amount must be finite and >= ₹1.00.",
                        ));
                    }
                }
            }
            let actor = format!("STAFF_OVERRIDE:{}", officer);
            let out = execute_moneymax_tool(
                &engine.moneymax_db,
                &engine.state_store,
                &ov.tool_name,
                &final_args,
                &actor,
                true, // Staff Maker-Checker explicitly authorizes frozen/limit exceptions
            );
            let status_label = if action == "MODIFY_APPROVE" {
                "MODIFIED_APPROVED"
            } else {
                "APPROVED"
            };
            (status_label, Some(out))
        }
        "REJECT" => ("REJECTED", Some(json!({ "rejected": true, "reason": notes }))),
        other => {
            let _ = engine.state_store.resolve_override(&override_id, "PENDING", "", "", None);
            return Err(err_resp(
                StatusCode::BAD_REQUEST,
                format!("Invalid decision action: {}", other),
            ));
        }
    };

    engine
        .state_store
        .resolve_override(&override_id, new_status, &officer, &notes, exec_res.as_ref())
        .map_err(|e| err_resp(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Check if session still has other pending overrides
    let remaining = engine
        .state_store
        .list_overrides(Some(&ov.session_id), Some("PENDING"))
        .unwrap_or_default();
    if remaining.is_empty() {
        let _ = engine.state_store.update_session_status(
            &ov.session_id,
            SessionStatus::AiActive,
            Some(&officer),
            None,
        );
    }

    let chat_msg = if new_status == "REJECTED" {
        format!(
            "[Maker-Checker Decision by {}]: Held action '{}' was REJECTED. Reason: {}",
            officer, ov.tool_name, notes
        )
    } else {
        format!(
            "[Maker-Checker Approved by {}]: Executed '{}' on account {}. Notes: {}",
            officer, ov.tool_name, ov.account_id, notes
        )
    };

    let _ = engine.state_store.append_message(
        &format!("MSG-OV-{}", Uuid::new_v4().as_simple().to_string()[..8].to_uppercase()),
        &ov.session_id,
        "system",
        &chat_msg,
        &chat_msg,
        &HashMap::new(),
        None,
        exec_res.as_ref(),
        Some("MAKER_CHECKER"),
        Some(new_status),
        0.0,
    );

    let _ = engine.state_store.log_staff_action(
        &officer,
        &format!("OVERRIDE_{}", new_status),
        &override_id,
        &chat_msg,
    );

    engine.telemetry.record_event(TelemetryEvent {
        event_id: format!("OVD-{}", Uuid::new_v4().as_simple().to_string()[..10].to_uppercase()),
        session_id: Some(ov.session_id.clone()),
        account_id: Some(ov.account_id.clone()),
        timestamp: Utc::now().timestamp_millis() as f64 / 1000.0,
        iso_time: Local::now().format("%H:%M:%S").to_string(),
        event_type: "STAFF_OVERRIDE".into(),
        target_model: "MAKER_CHECKER".into(),
        decision_source: Some(officer),
        overhead_ms: 0.0,
        user_prompt_sanitized: chat_msg,
        pii_masked: HashMap::new(),
        attack_type: None,
        tool_name: Some(ov.tool_name),
        tool_args: exec_res.clone(),
        violation_code: Some(ov.violation_code),
        reason: Some(notes),
        layers: vec![],
    });

    let updated = engine.state_store.get_override(&override_id).ok().flatten();
    Ok(Json(json!({
        "success": true,
        "override": updated,
        "execution_result": exec_res
    })))
}
