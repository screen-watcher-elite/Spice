use std::convert::Infallible;
use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Json, Response,
    },
};
use futures_util::Stream;
use serde::Deserialize;
use serde_json::json;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;

use aegismesh_core::pipeline::AegisMeshEngine;

use super::{check_admin_auth, err_resp, ApiError};

#[derive(Debug, Deserialize)]
pub struct ExportQuery {
    pub format: Option<String>,
}

fn csv_safe(cell: &str) -> String {
    let cleaned = cell.replace(['\r', '\n'], " ");
    let trimmed = cleaned.trim_start();
    let prefixed = if matches!(trimmed.chars().next(), Some('=' | '+' | '-' | '@' | '\t')) {
        format!("'{}", cleaned)
    } else {
        cleaned
    };
    format!("\"{}\"", prefixed.replace('"', "\"\""))
}

pub async fn get_stats(State(engine): State<Arc<AegisMeshEngine>>) -> impl IntoResponse {
    Json(engine.telemetry.get_stats())
}

pub async fn get_history(State(engine): State<Arc<AegisMeshEngine>>) -> impl IntoResponse {
    let history = engine.telemetry.get_history(120);
    Json(json!({ "history": history }))
}

pub async fn stream_events(
    State(engine): State<Arc<AegisMeshEngine>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = engine.telemetry.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|msg| match msg {
        Ok(event) => {
            let json_str = serde_json::to_string(&event).unwrap_or_default();
            Some(Ok(Event::default().data(json_str)))
        }
        Err(_) => None,
    });
    Sse::new(stream).keep_alive(KeepAlive::default())
}

pub async fn get_sentinel_state(State(engine): State<Arc<AegisMeshEngine>>) -> impl IntoResponse {
    Json(engine.sentinel.get_summary())
}

pub async fn ack_sentinel_alert(
    State(engine): State<Arc<AegisMeshEngine>>,
    headers: HeaderMap,
    Path(audit_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    check_admin_auth(&engine, &headers)?;
    let ok = engine
        .state_store
        .acknowledge_sentinel_audit(&audit_id)
        .map_err(|e| err_resp(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if !ok {
        return Err(err_resp(StatusCode::NOT_FOUND, "Sentinel audit ID not found"));
    }
    Ok(Json(json!({ "success": true, "audit_id": audit_id })))
}

pub async fn get_staff_logs(
    State(engine): State<Arc<AegisMeshEngine>>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, ApiError> {
    check_admin_auth(&engine, &headers)?;
    let logs = engine
        .state_store
        .list_staff_audit_logs(100)
        .map_err(|e| err_resp(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(json!({ "logs": logs })))
}

pub async fn export_audit_report(
    State(engine): State<Arc<AegisMeshEngine>>,
    headers: HeaderMap,
    Query(q): Query<ExportQuery>,
) -> Result<Response, ApiError> {
    check_admin_auth(&engine, &headers)?;
    let fmt = q.format.as_deref().unwrap_or("json").to_lowercase();
    let stats = engine.telemetry.get_stats();
    let sentinel = engine.sentinel.get_summary();
    let history = engine.telemetry.get_history(200);
    let overrides = engine.state_store.list_overrides(None, None).unwrap_or_default();
    let staff_logs = engine.state_store.list_staff_audit_logs(200).unwrap_or_default();

    if fmt == "csv" {
        let mut csv = String::from(
            "event_id,iso_time,event_type,account_id,overhead_ms,decision_source,violation_code,reason\n",
        );
        for ev in &history {
            csv.push_str(&format!(
                "{},{},{},{},{:.3},{},{},{}\n",
                csv_safe(&ev.event_id),
                csv_safe(&ev.iso_time),
                csv_safe(&ev.event_type),
                csv_safe(ev.account_id.as_deref().unwrap_or("")),
                ev.overhead_ms,
                csv_safe(ev.decision_source.as_deref().unwrap_or("")),
                csv_safe(ev.violation_code.as_deref().unwrap_or("")),
                csv_safe(ev.reason.as_deref().unwrap_or("")),
            ));
        }
        return Ok((
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, "text/csv; charset=utf-8".to_string()),
                (
                    header::CONTENT_DISPOSITION,
                    "attachment; filename=\"aegismesh_compliance_audit.csv\"".to_string(),
                ),
            ],
            csv,
        )
            .into_response());
    }

    let report = json!({
        "report_type": "AegisMesh RBI & DPDP Compliance Audit",
        "generated_at": chrono::Utc::now().to_rfc3339(),
        "stats": stats,
        "sentinel": sentinel,
        "overrides": overrides,
        "staff_audit_logs": staff_logs,
        "telemetry_events": history,
    });

    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/json; charset=utf-8".to_string()),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"aegismesh_compliance_audit.json\"".to_string(),
            ),
        ],
        serde_json::to_string_pretty(&report).unwrap_or_default(),
    )
        .into_response())
}
