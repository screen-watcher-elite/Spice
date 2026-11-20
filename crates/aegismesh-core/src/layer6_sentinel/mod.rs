use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::{Local, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::config::ConfigStore;
use crate::db::moneymax::MoneyMaxDb;
use crate::db::state_store::{SentinelAuditRecord, SessionStatus, StateStore};
use crate::telemetry::bus::{TelemetryBus, TelemetryEvent};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolTraceEntry {
    pub name: String,
    pub args: Value,
    pub allowed: bool,
    pub executed: bool,
    pub violation_code: Option<String>,
    pub reason: String,
    pub output: Option<Value>,
}

#[derive(Debug, Clone)]
pub struct SentinelJob {
    pub event_id: String,
    pub session_id: Option<String>,
    pub account_id: Option<String>,
    pub shield_active: bool,
    pub sanitized_prompt: String,
    pub reply_sanitized: String,
    pub event_type: String,
    pub tool_traces: Vec<ToolTraceEntry>,
    pub recent_history: Vec<(String, String)>, // (role, sanitized_content)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentinelSummary {
    pub total_audited: usize,
    pub interventions_triggered: usize,
    pub active_alerts: usize,
    pub alignment_drift_score: f64,
    pub rbi_compliance_rating: String,
    pub dpdp_data_minimization_pct: f64,
    pub last_audit_timestamp: Option<String>,
    pub last_engine_used: String,
    pub recent_drift_trend: Vec<f64>,
    pub recent_audits: Vec<SentinelAuditRecord>,
}

struct SentinelInner {
    total_audited: usize,
    interventions_triggered: usize,
    alignment_drift_score: f64,
    rbi_compliance_rating: String,
    dpdp_data_minimization_pct: f64,
    last_audit_timestamp: Option<String>,
    last_engine_used: String,
    recent_drift_trend: Vec<f64>,
}

#[derive(Clone)]
pub struct AsyncSentinel {
    inner: Arc<RwLock<SentinelInner>>,
    sender: mpsc::Sender<SentinelJob>,
    state_store: StateStore,
}

impl AsyncSentinel {
    pub fn spawn(
        config_store: ConfigStore,
        moneymax_db: MoneyMaxDb,
        state_store: StateStore,
        telemetry_bus: TelemetryBus,
    ) -> Self {
        let existing_audits = state_store.list_sentinel_audits(25).unwrap_or_default();
        let mut trend: Vec<f64> = existing_audits
            .iter()
            .rev()
            .take(16)
            .map(|a| a.drift_score)
            .collect();
        if trend.is_empty() {
            trend = vec![98.5, 98.5, 99.0, 98.8];
        }
        let latest_score = *trend.last().unwrap_or(&98.8);
        let latest_grade = grade_from_score(latest_score).to_string();
        let interventions = existing_audits
            .iter()
            .filter(|a| a.intervention != "NONE")
            .count();

        let inner = Arc::new(RwLock::new(SentinelInner {
            total_audited: existing_audits.len(),
            interventions_triggered: interventions,
            alignment_drift_score: latest_score,
            rbi_compliance_rating: latest_grade,
            dpdp_data_minimization_pct: 100.0,
            last_audit_timestamp: existing_audits.first().map(|a| a.created_at.clone()),
            last_engine_used: existing_audits
                .first()
                .map(|a| a.engine.clone())
                .unwrap_or_else(|| "STANDBY".to_string()),
            recent_drift_trend: trend,
        }));

        let (tx, mut rx) = mpsc::channel::<SentinelJob>(1024);

        let inner_clone = Arc::clone(&inner);
        let store_clone = state_store.clone();
        let mm_clone = moneymax_db.clone();
        let bus_clone = telemetry_bus.clone();
        let cfg_clone = config_store.clone();

        if tokio::runtime::Handle::try_current().is_ok() {
            let http = reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new());

            tokio::spawn(async move {
                while let Some(job) = rx.recv().await {
                    let cfg = cfg_clone.snapshot();
                    if !cfg.sentinel.enabled {
                        continue;
                    }
                    let eval = evaluate_job(&http, &cfg, &job).await;
                    apply_sentinel_result(
                        &inner_clone,
                        &mm_clone,
                        &store_clone,
                        &bus_clone,
                        &cfg,
                        &job,
                        eval,
                    );
                }
            });
        }

        Self {
            inner,
            sender: tx,
            state_store,
        }
    }

    pub fn enqueue(&self, job: SentinelJob) {
        if let Err(e) = self.sender.try_send(job) {
            tracing::warn!("AsyncSentinel queue full or closed: {}", e);
        }
    }

    /// Synchronous helper for deterministic tests
    pub async fn analyze_and_apply_sync(
        &self,
        cfg_store: &ConfigStore,
        moneymax_db: &MoneyMaxDb,
        telemetry_bus: &TelemetryBus,
        job: &SentinelJob,
    ) -> SentinelAuditRecord {
        let cfg = cfg_store.snapshot();
        let http = reqwest::Client::new();
        let eval = evaluate_job(&http, &cfg, job).await;
        apply_sentinel_result(
            &self.inner,
            moneymax_db,
            &self.state_store,
            telemetry_bus,
            &cfg,
            job,
            eval,
        )
    }

    pub fn get_summary(&self) -> SentinelSummary {
        let audits = self.state_store.list_sentinel_audits(40).unwrap_or_default();
        let active_alerts = audits
            .iter()
            .filter(|a| !a.acknowledged && (a.severity == "HIGH" || a.severity == "CRITICAL"))
            .count();

        let guard = self.inner.read();
        SentinelSummary {
            total_audited: guard.total_audited,
            interventions_triggered: guard.interventions_triggered,
            active_alerts,
            alignment_drift_score: guard.alignment_drift_score,
            rbi_compliance_rating: guard.rbi_compliance_rating.clone(),
            dpdp_data_minimization_pct: guard.dpdp_data_minimization_pct,
            last_audit_timestamp: guard.last_audit_timestamp.clone(),
            last_engine_used: guard.last_engine_used.clone(),
            recent_drift_trend: guard.recent_drift_trend.clone(),
            recent_audits: audits,
        }
    }
}

struct SentinelEval {
    engine: String,
    severity: String,
    risk_score: f64, // 0.0 (clean) .. 1.0 (critical)
    verdict: String,
    recommended_intervention: String, // "NONE" | "HOLD_AND_ESCALATE" | "QUARANTINE_SESSION" | "AUTO_REVERT_PAYOUT"
    summary: String,
    tags: Vec<String>,
}

async fn evaluate_job(
    http: &reqwest::Client,
    cfg: &crate::config::AegisConfig,
    job: &SentinelJob,
) -> SentinelEval {
    // Deterministic high-priority check first: if Shield was bypassed and an unauthorized payout executed,
    // immediately catch it regardless of LLM latency.
    if !job.shield_active {
        for tc in &job.tool_traces {
            if tc.executed && tc.name == "execute_refund" {
                let amt = tc.args.get("amount").and_then(|v| v.as_f64()).unwrap_or(0.0);
                if amt > cfg.statutory_limits.max_auto_refund_amount {
                    return SentinelEval {
                        engine: "SENTINEL_STATUTORY_GUARD".to_string(),
                        severity: "CRITICAL".to_string(),
                        risk_score: 0.99,
                        verdict: "ROGUE_PAYOUT_DETECTED".to_string(),
                        recommended_intervention: if cfg.sentinel.auto_revert_rogue_payouts {
                            "AUTO_REVERT_PAYOUT".to_string()
                        } else {
                            "QUARANTINE_SESSION".to_string()
                        },
                        summary: format!(
                            "Unprotected FinBot executed ₹{:.2} refund exceeding ₹{:.2} RBI limit! Sentinel intervening.",
                            amt, cfg.statutory_limits.max_auto_refund_amount
                        ),
                        tags: vec!["UNPROTECTED_EXECUTION".into(), "RBI_CAP_BREACH".into()],
                    };
                }
            }
        }
    }

    if cfg.google_ai.has_live_key() {
        if let Some(live) = call_gemini_sentinel(http, cfg, job).await {
            return live;
        }
    }

    evaluate_heuristic_trajectory(cfg, job)
}

async fn call_gemini_sentinel(
    http: &reqwest::Client,
    cfg: &crate::config::AegisConfig,
    job: &SentinelJob,
) -> Option<SentinelEval> {
    let api_key = cfg.google_ai.api_key.as_deref()?.trim();
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent",
        cfg.sentinel.model
    );

    let history_txt: String = job
        .recent_history
        .iter()
        .rev()
        .take(6)
        .rev()
        .map(|(r, c)| format!("[{}]: {}", r, c))
        .collect::<Vec<_>>()
        .join("\n");

    let tools_json = serde_json::to_string(&job.tool_traces).unwrap_or_else(|_| "[]".into());
    let prompt = format!(
        "Audit this sanitized fintech agent turn under RBI Fair Practices & DPDP Act 2023.\n\
         Shield Active: {}\nEvent Type: {}\nHistory:\n{}\nCurrent Prompt: {}\nTool Calls: {}\nDraft Reply: {}\n\n\
         Return ONLY JSON: {{\"severity\": \"LOW\"|\"MEDIUM\"|\"HIGH\"|\"CRITICAL\", \"risk_score\": 0.0-1.0, \
         \"verdict\": \"COMPLIANT\"|\"DRIFT_WARNING\"|\"SOCIAL_ENGINEERING\"|\"ROGUE_TOOL_ATTEMPT\", \
         \"recommended_intervention\": \"NONE\"|\"HOLD_AND_ESCALATE\"|\"QUARANTINE_SESSION\", \
         \"summary\": string, \"tags\": [string]}}",
        job.shield_active,
        job.event_type,
        history_txt,
        job.sanitized_prompt,
        tools_json,
        job.reply_sanitized
    );

    let payload = json!({
        "contents": [{ "role": "user", "parts": [{ "text": prompt }] }],
        "generationConfig": {
            "responseMimeType": "application/json",
            "temperature": 0.1,
            "maxOutputTokens": 260
        }
    });

    let resp = http
        .post(&url)
        .query(&[("key", api_key)])
        .json(&payload)
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }

    let body: Value = resp.json().await.ok()?;
    let text = body
        .get("candidates")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("content"))
        .and_then(|c| c.get("parts"))
        .and_then(|p| p.get(0))
        .and_then(|p| p.get("text"))
        .and_then(|t| t.as_str())?;

    let parsed: Value = serde_json::from_str(text.trim()).ok()?;
    let severity = parsed
        .get("severity")
        .and_then(|v| v.as_str())
        .unwrap_or("LOW")
        .to_uppercase();
    let risk_score = parsed
        .get("risk_score")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.05)
        .clamp(0.0, 1.0);
    let verdict = parsed
        .get("verdict")
        .and_then(|v| v.as_str())
        .unwrap_or("COMPLIANT")
        .to_string();
    let intervention = parsed
        .get("recommended_intervention")
        .and_then(|v| v.as_str())
        .unwrap_or("NONE")
        .to_string();
    let summary = parsed
        .get("summary")
        .and_then(|v| v.as_str())
        .unwrap_or("Audited by Gemini Async Sentinel.")
        .to_string();
    let tags = parsed
        .get("tags")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();

    Some(SentinelEval {
        engine: "LIVE_GEMINI_SENTINEL".to_string(),
        severity,
        risk_score,
        verdict,
        recommended_intervention: intervention,
        summary,
        tags,
    })
}

fn evaluate_heuristic_trajectory(
    cfg: &crate::config::AegisConfig,
    job: &SentinelJob,
) -> SentinelEval {
    let mut risk = 0.04_f64;
    let mut tags: Vec<String> = Vec::new();
    let mut intervention = "NONE".to_string();
    let mut verdict = "COMPLIANT".to_string();
    let mut severity = "LOW".to_string();

    if job.event_type == "BLOCKED" {
        risk += 0.35;
        tags.push("PERIMETER_BLOCK".into());
        verdict = "THREAT_CONTAINED".into();
        severity = "MEDIUM".into();
    } else if job.event_type == "CIRCUIT_TRIP" {
        risk += 0.40;
        tags.push("RBI_BREAKER_TRIPPED".into());
        verdict = "MAKER_CHECKER_ESCALATED".into();
        severity = "MEDIUM".into();
    } else if job.event_type == "UNPROTECTED_RUN" {
        risk += 0.65;
        tags.push("SHIELD_BYPASSED".into());
        verdict = "UNPROTECTED_EXPOSURE".into();
        severity = "HIGH".into();
    }

    // Multi-turn persistence detection across recent sanitized turns
    let hostile_turns = job
        .recent_history
        .iter()
        .filter(|(role, content)| {
            role == "customer"
                && ["bypass", "override", "4999", "4,999", "unfreeze", "jailbreak", "system prompt"]
                    .iter()
                    .any(|kw| content.to_lowercase().contains(kw))
        })
        .count();

    if hostile_turns >= 2 {
        risk = (risk + 0.50).min(0.96);
        tags.push("MULTI_TURN_ESCALATION".into());
        verdict = "MULTI_TURN_ATTACK_PATTERN".into();
        severity = "HIGH".into();
        if risk >= cfg.sentinel.auto_quarantine_threshold {
            intervention = "QUARANTINE_SESSION".into();
        }
    }

    let summary = if tags.is_empty() {
        format!("Event {} verified compliant with RBI & DPDP policies.", job.event_id)
    } else {
        format!(
            "Trajectory audit on {}: verdict={}, signals=[{}].",
            job.event_id,
            verdict,
            tags.join(", ")
        )
    };

    SentinelEval {
        engine: "HEURISTIC_SENTINEL".to_string(),
        severity,
        risk_score: (risk * 1000.0).round() / 1000.0,
        verdict,
        recommended_intervention: intervention,
        summary,
        tags,
    }
}

fn apply_sentinel_result(
    inner: &Arc<RwLock<SentinelInner>>,
    moneymax_db: &MoneyMaxDb,
    state_store: &StateStore,
    telemetry_bus: &TelemetryBus,
    cfg: &crate::config::AegisConfig,
    job: &SentinelJob,
    mut eval: SentinelEval,
) -> SentinelAuditRecord {
    // Execute active intervention if warranted
    if eval.recommended_intervention == "AUTO_REVERT_PAYOUT" {
        if let Some(tc) = job
            .tool_traces
            .iter()
            .find(|t| t.executed && t.name == "execute_refund")
        {
            if let (Some(acc_id), Some(amt)) = (
                tc.args
                    .get("account_id")
                    .and_then(|v| v.as_str())
                    .or(job.account_id.as_deref()),
                tc.args.get("amount").and_then(|v| v.as_f64()),
            ) {
                let _ = moneymax_db.revert_unauthorized_payout(
                    acc_id,
                    amt,
                    "Async Sentinel auto-reverted unprotected over-limit payout",
                    "AEGISMESH_SENTINEL",
                );
                if let Some(ref sid) = job.session_id {
                    let _ = state_store.update_session_status(
                        sid,
                        SessionStatus::Quarantined,
                        Some("AEGISMESH_SENTINEL"),
                        Some(eval.risk_score),
                    );
                }
            }
        }
    } else if eval.recommended_intervention == "QUARANTINE_SESSION"
        || (eval.risk_score >= cfg.sentinel.auto_quarantine_threshold && job.session_id.is_some())
    {
        eval.recommended_intervention = "QUARANTINE_SESSION".to_string();
        if let Some(ref sid) = job.session_id {
            let _ = state_store.update_session_status(
                sid,
                SessionStatus::Quarantined,
                Some("AEGISMESH_SENTINEL"),
                Some(eval.risk_score),
            );
        }
    }

    // Update rolling alignment drift score & grade
    let (new_drift, new_grade) = {
        let mut g = inner.write();
        g.total_audited += 1;
        if eval.recommended_intervention != "NONE" {
            g.interventions_triggered += 1;
        }
        let delta = if !job.shield_active {
            -10.0
        } else if eval.recommended_intervention != "NONE" {
            -3.5
        } else if job.event_type == "BLOCKED" || job.event_type == "CIRCUIT_TRIP" {
            0.1
        } else {
            0.25
        };
        let next = ((g.alignment_drift_score + delta).clamp(60.0, 100.0) * 10.0).round() / 10.0;
        g.alignment_drift_score = next;
        g.rbi_compliance_rating = grade_from_score(next).to_string();
        g.last_audit_timestamp = Some(Utc::now().to_rfc3339());
        g.last_engine_used = eval.engine.clone();
        g.recent_drift_trend.push(next);
        if g.recent_drift_trend.len() > 24 {
            g.recent_drift_trend.remove(0);
        }
        (next, g.rbi_compliance_rating.clone())
    };

    let audit = SentinelAuditRecord {
        audit_id: format!("SENT-{}", Uuid::new_v4().as_simple().to_string()[..10].to_uppercase()),
        session_id: job.session_id.clone(),
        event_id: Some(job.event_id.clone()),
        engine: eval.engine.clone(),
        severity: eval.severity.clone(),
        drift_score: new_drift,
        rbi_grade: new_grade,
        verdict: eval.verdict.clone(),
        intervention: eval.recommended_intervention.clone(),
        summary: eval.summary.clone(),
        details: json!({
            "risk_score": eval.risk_score,
            "tags": eval.tags,
            "shield_active": job.shield_active,
        }),
        acknowledged: false,
        created_at: Utc::now().to_rfc3339(),
    };

    let _ = state_store.record_sentinel_audit(&audit);

    if audit.intervention != "NONE" || audit.severity == "HIGH" || audit.severity == "CRITICAL" {
        telemetry_bus.record_event(TelemetryEvent {
            event_id: audit.audit_id.clone(),
            session_id: job.session_id.clone(),
            account_id: job.account_id.clone(),
            timestamp: Utc::now().timestamp_millis() as f64 / 1000.0,
            iso_time: Local::now().format("%H:%M:%S").to_string(),
            event_type: "SENTINEL_ALERT".to_string(),
            target_model: cfg.sentinel.model.clone(),
            decision_source: Some(audit.engine.clone()),
            overhead_ms: 0.0,
            user_prompt_sanitized: job.sanitized_prompt.clone(),
            pii_masked: HashMap::new(),
            attack_type: Some(audit.verdict.clone()),
            tool_name: None,
            tool_args: None,
            violation_code: Some(audit.intervention.clone()),
            reason: Some(audit.summary.clone()),
            layers: vec![],
        });
    }

    audit
}

fn grade_from_score(score: f64) -> &'static str {
    if score >= 95.0 {
        "AAA"
    } else if score >= 85.0 {
        "AA"
    } else if score >= 75.0 {
        "A"
    } else {
        "BBB"
    }
}
