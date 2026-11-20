use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use chrono::{Local, Utc};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::agent::finbot::FinBotAgent;
use crate::config::{ConfigStore, TokenFormat};
use crate::db::moneymax::MoneyMaxDb;
use crate::db::state_store::{PendingOverrideRecord, SessionStatus, StateStore};
use crate::layer1_regex::{Layer1RegexEngine, RateLimiter};
use crate::layer2_pii::tokenizer::PiiTokenizer;
use crate::layer2_pii::vault::PiiVault;
use crate::layer3_guardrail::{DecisionResult, GeminiDecisionClient};
use crate::layer4_breaker::ToolCall;
use crate::layer5_outbound::OutboundFirewall;
use crate::layer6_sentinel::{AsyncSentinel, SentinelJob, ToolTraceEntry};
use crate::telemetry::bus::{TelemetryBus, TelemetryEvent};
use crate::types::{LayerReport, LayerStatus};

#[derive(Debug, Clone, Deserialize)]
pub struct PipelineRequest {
    pub prompt: String,
    pub session_id: Option<String>,
    pub account_id: Option<String>,
    pub client_ip: Option<String>,
    pub explicit_tools: Option<Vec<ToolCall>>,
    pub token_format: Option<TokenFormat>,
    pub shield_override: Option<bool>,
    pub persist_chat: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct PipelineOutcome {
    pub event_id: String,
    pub session_id: String,
    pub status: String, // "PROCESSED" | "MASKED" | "BLOCKED" | "CIRCUIT_BREAKER_TRIPPED" | "RATE_LIMITED" | "UNPROTECTED_RUN"
    pub shield_active: bool,
    pub overhead_ms: f64,
    pub pii_detected: bool,
    pub pii_counts: HashMap<String, usize>,
    pub sanitized_prompt: String,
    pub reply: String,
    pub model_used: String,
    pub decision_source: Option<String>,
    pub gemini_decision: Option<DecisionResult>,
    pub violation_code: Option<String>,
    pub reason: Option<String>,
    pub layers: Vec<LayerReport>,
    pub tool_traces: Vec<ToolTraceEntry>,
    pub created_overrides: Vec<PendingOverrideRecord>,
}

#[derive(Clone)]
pub struct AegisMeshEngine {
    pub config: ConfigStore,
    pub moneymax_db: MoneyMaxDb,
    pub state_store: StateStore,
    pub shield_active: Arc<AtomicBool>,
    pub layer1: Layer1RegexEngine,
    pub rate_limiter: RateLimiter,
    pub vault: PiiVault,
    pub tokenizer: PiiTokenizer,
    pub decision_client: GeminiDecisionClient,
    pub outbound: OutboundFirewall,
    pub agent: FinBotAgent,
    pub telemetry: TelemetryBus,
    pub sentinel: AsyncSentinel,
    pub started_at: String,
    pub auth_failures: Arc<Mutex<HashMap<String, (u32, Instant)>>>,
}

impl AegisMeshEngine {
    pub fn new(
        config: ConfigStore,
        moneymax_db: MoneyMaxDb,
        state_store: StateStore,
    ) -> Self {
        let snap = config.snapshot();
        let shield_active = Arc::new(AtomicBool::new(snap.firewall.default_shield_active));
        let vault = PiiVault::new(snap.dpdp_act.vault_ttl_seconds);
        let tokenizer = PiiTokenizer::new(Some(vault.clone()))
            .with_format(snap.dpdp_act.default_token_format);
        let outbound = OutboundFirewall::new(tokenizer.clone());
        let telemetry = TelemetryBus::new(250, Some(state_store.clone()));
        let sentinel = AsyncSentinel::spawn(
            config.clone(),
            moneymax_db.clone(),
            state_store.clone(),
            telemetry.clone(),
        );

        Self {
            config,
            moneymax_db,
            state_store,
            shield_active,
            layer1: Layer1RegexEngine::new(),
            rate_limiter: RateLimiter::new(),
            vault,
            tokenizer,
            decision_client: GeminiDecisionClient::new(),
            outbound,
            agent: FinBotAgent::new(),
            telemetry,
            sentinel,
            started_at: Utc::now().to_rfc3339(),
            auth_failures: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn is_shield_active(&self) -> bool {
        self.shield_active.load(Ordering::SeqCst)
    }

    pub fn set_shield_active(&self, active: bool) {
        self.shield_active.store(active, Ordering::SeqCst);
    }

    pub async fn process_request(&self, req: PipelineRequest) -> PipelineOutcome {
        let t_total = Instant::now();
        let cfg = self.config.snapshot();
        let event_id = format!("AEGIS-{}", Uuid::new_v4().as_simple().to_string()[..12].to_uppercase());
        let session_id = req
            .session_id
            .clone()
            .unwrap_or_else(|| format!("SESS-{}", Uuid::new_v4().as_simple().to_string()[..8].to_uppercase()));
        let shield_enabled = req.shield_override.unwrap_or_else(|| self.is_shield_active());
        let client_ip = req.client_ip.as_deref().unwrap_or("127.0.0.1");

        let account_id_resolved = req
            .account_id
            .clone()
            .unwrap_or_else(|| "MMX-AC-5001".to_string());
        let customer_name = self
            .moneymax_db
            .get_account_context(&account_id_resolved)
            .ok()
            .flatten()
            .map(|c| c.customer_name)
            .unwrap_or_else(|| "Customer".to_string());

        if req.persist_chat {
            let _ = self.state_store.create_or_get_session(
                &session_id,
                &account_id_resolved,
                &customer_name,
            );
        }

        let recent_history: Vec<(String, String)> = self
            .state_store
            .get_session_messages(&session_id, false)
            .unwrap_or_default()
            .into_iter()
            .map(|m| (m.role, m.content_sanitized))
            .collect();

        // -------------------------------------------------------------
        // UNPROTECTED DEMO PATH (Verified Admin Bypass Only)
        // -------------------------------------------------------------
        if !shield_enabled {
            let sse_safe_tok = self.tokenizer.tokenize_with_config(
                &req.prompt,
                &session_id,
                &cfg.dpdp_act,
                req.token_format,
            );
            let agent_out = self
                .agent
                .run_turn(
                    &cfg,
                    &self.moneymax_db,
                    &self.state_store,
                    &req.prompt,
                    Some(&account_id_resolved),
                    &recent_history,
                    false,
                    req.explicit_tools.as_deref(),
                )
                .await;

            let overhead_ms = round_ms(t_total.elapsed().as_micros());
            let layers = vec![LayerReport {
                layer: 0,
                name: "Shield Bypass".into(),
                status: LayerStatus::Alert,
                latency_us: t_total.elapsed().as_micros() as u64,
                detail: "AegisMesh perimeter bypassed via authorized admin toggle.".into(),
                metadata: None,
            }];

            if req.persist_chat {
                let _ = self.state_store.append_message(
                    &format!("MSG-U-{}", &event_id[6..]),
                    &session_id,
                    "customer",
                    &sse_safe_tok.sanitized_text,
                    &req.prompt,
                    &sse_safe_tok.pii_counts,
                    None,
                    None,
                    None,
                    None,
                    0.0,
                );
                let _ = self.state_store.append_message(
                    &format!("MSG-A-{}", &event_id[6..]),
                    &session_id,
                    "assistant",
                    &agent_out.reply_draft,
                    &agent_out.reply_draft,
                    &HashMap::new(),
                    Some(&json!(layers)),
                    Some(&json!(agent_out.tool_traces)),
                    Some("BYPASSED"),
                    Some("UNPROTECTED_RUN"),
                    overhead_ms,
                );
            }

            self.telemetry.record_event(TelemetryEvent {
                event_id: event_id.clone(),
                session_id: Some(session_id.clone()),
                account_id: Some(account_id_resolved.clone()),
                timestamp: Utc::now().timestamp_millis() as f64 / 1000.0,
                iso_time: Local::now().format("%H:%M:%S").to_string(),
                event_type: "UNPROTECTED_RUN".into(),
                target_model: agent_out.model_used.clone(),
                decision_source: Some("BYPASSED".into()),
                overhead_ms,
                user_prompt_sanitized: sse_safe_tok.sanitized_text.clone(),
                pii_masked: sse_safe_tok.pii_counts.clone(),
                attack_type: None,
                tool_name: agent_out.tool_traces.first().map(|t| t.name.clone()),
                tool_args: agent_out.tool_traces.first().map(|t| t.args.clone()),
                violation_code: None,
                reason: Some("Shield bypassed; executed raw agent run.".into()),
                layers: layers.clone(),
            });

            self.sentinel.enqueue(SentinelJob {
                event_id: event_id.clone(),
                session_id: Some(session_id.clone()),
                account_id: Some(account_id_resolved),
                shield_active: false,
                sanitized_prompt: sse_safe_tok.sanitized_text,
                reply_sanitized: agent_out.reply_draft.clone(),
                event_type: "UNPROTECTED_RUN".into(),
                tool_traces: agent_out.tool_traces.clone(),
                recent_history,
            });

            return PipelineOutcome {
                event_id,
                session_id,
                status: "UNPROTECTED_RUN".into(),
                shield_active: false,
                overhead_ms,
                pii_detected: false,
                pii_counts: HashMap::new(),
                sanitized_prompt: req.prompt,
                reply: agent_out.reply_draft,
                model_used: agent_out.model_used,
                decision_source: Some("BYPASSED".into()),
                gemini_decision: None,
                violation_code: None,
                reason: Some("Shield bypassed via Admin authorization.".into()),
                layers,
                tool_traces: agent_out.tool_traces,
                created_overrides: vec![],
            };
        }

        // -------------------------------------------------------------
        // PROTECTED 6-LAYER PIPELINE
        // -------------------------------------------------------------
        let mut layers: Vec<LayerReport> = Vec::with_capacity(6);

        // LAYER 1: Rate Limit + Regex + Adversarial Normalization
        let t1 = Instant::now();
        let rate_key = format!("{}:{}", client_ip, session_id);
        if !self.rate_limiter.check(&rate_key, cfg.firewall.rate_limit_rpm) {
            let l1_us = t1.elapsed().as_micros() as u64;
            layers.push(LayerReport {
                layer: 1,
                name: "Regex & Velocity Perimeter".into(),
                status: LayerStatus::Blocked,
                latency_us: l1_us,
                detail: "Rate limit exceeded.".into(),
                metadata: None,
            });
            let overhead_ms = round_ms(t_total.elapsed().as_micros());
            return PipelineOutcome {
                event_id,
                session_id,
                status: "RATE_LIMITED".into(),
                shield_active: true,
                overhead_ms,
                pii_detected: false,
                pii_counts: HashMap::new(),
                sanitized_prompt: "[RATE_LIMITED]".into(),
                reply: "Too many requests in a short window. Please wait a moment and try again.".into(),
                model_used: cfg.google_ai.agent_model,
                decision_source: None,
                gemini_decision: None,
                violation_code: Some("ERR_RATE_LIMITED".into()),
                reason: Some("Token-bucket rate limit exceeded.".into()),
                layers,
                tool_traces: vec![],
                created_overrides: vec![],
            };
        }

        let safety = self.layer1.inspect(&req.prompt);
        let l1_us = t1.elapsed().as_micros() as u64;

        // Always sanitize customer prompt before storing/streaming so raw PII never leaks on blocked events!
        let t2 = Instant::now();
        let token_res = self.tokenizer.tokenize_with_config(
            &req.prompt,
            &session_id,
            &cfg.dpdp_act,
            req.token_format,
        );
        let l2_us = t2.elapsed().as_micros() as u64;

        if !safety.is_safe {
            layers.push(LayerReport {
                layer: 1,
                name: "Regex & Adversarial Normalizer".into(),
                status: LayerStatus::Blocked,
                latency_us: l1_us,
                detail: safety.reason.clone().unwrap_or_else(|| "Signature match".into()),
                metadata: Some(json!({
                    "attack_type": safety.attack_type,
                    "confidence": safety.confidence,
                    "decoded_payload": safety.decoded_payload_matched,
                })),
            });
            layers.push(LayerReport {
                layer: 2,
                name: "DPDP PII Tokenizer".into(),
                status: if token_res.has_pii { LayerStatus::Masked } else { LayerStatus::Pass },
                latency_us: l2_us,
                detail: format!("{} PII entities masked for log safety.", token_res.pii_counts.values().sum::<usize>()),
                metadata: Some(json!(token_res.pii_counts)),
            });

            let overhead_ms = round_ms(t_total.elapsed().as_micros());
            let block_reply = "Request blocked by AegisMesh perimeter policy: suspected prompt injection or unauthorized override attempt.";
            let reason = safety.reason.clone().unwrap_or_else(|| "Security policy block".into());

            if req.persist_chat {
                let _ = self.state_store.append_message(
                    &format!("MSG-U-{}", &event_id[6..]),
                    &session_id,
                    "customer",
                    &token_res.sanitized_text,
                    &req.prompt,
                    &token_res.pii_counts,
                    None,
                    None,
                    None,
                    None,
                    0.0,
                );
                let _ = self.state_store.append_message(
                    &format!("MSG-A-{}", &event_id[6..]),
                    &session_id,
                    "assistant",
                    block_reply,
                    block_reply,
                    &HashMap::new(),
                    Some(&json!(layers)),
                    None,
                    Some("LAYER1_REGEX"),
                    Some("BLOCKED"),
                    overhead_ms,
                );
            }

            self.telemetry.record_event(TelemetryEvent {
                event_id: event_id.clone(),
                session_id: Some(session_id.clone()),
                account_id: Some(account_id_resolved.clone()),
                timestamp: Utc::now().timestamp_millis() as f64 / 1000.0,
                iso_time: Local::now().format("%H:%M:%S").to_string(),
                event_type: "BLOCKED".into(),
                target_model: cfg.google_ai.agent_model.clone(),
                decision_source: Some("LAYER1_REGEX".into()),
                overhead_ms,
                user_prompt_sanitized: token_res.sanitized_text.clone(),
                pii_masked: token_res.pii_counts.clone(),
                attack_type: safety.attack_type.clone(),
                tool_name: None,
                tool_args: None,
                violation_code: safety.attack_type.clone(),
                reason: Some(reason.clone()),
                layers: layers.clone(),
            });

            self.sentinel.enqueue(SentinelJob {
                event_id: event_id.clone(),
                session_id: Some(session_id.clone()),
                account_id: Some(account_id_resolved),
                shield_active: true,
                sanitized_prompt: token_res.sanitized_text.clone(),
                reply_sanitized: block_reply.to_string(),
                event_type: "BLOCKED".into(),
                tool_traces: vec![],
                recent_history,
            });

            return PipelineOutcome {
                event_id,
                session_id,
                status: "BLOCKED".into(),
                shield_active: true,
                overhead_ms,
                pii_detected: token_res.has_pii,
                pii_counts: token_res.pii_counts,
                sanitized_prompt: token_res.sanitized_text,
                reply: block_reply.to_string(),
                model_used: cfg.google_ai.agent_model,
                decision_source: Some("LAYER1_REGEX".into()),
                gemini_decision: None,
                violation_code: safety.attack_type,
                reason: Some(reason),
                layers,
                tool_traces: vec![],
                created_overrides: vec![],
            };
        }

        layers.push(LayerReport {
            layer: 1,
            name: "Regex & Adversarial Normalizer".into(),
            status: LayerStatus::Pass,
            latency_us: l1_us,
            detail: "Cleared adversarial normalization, Base64 & injection regex checks.".into(),
            metadata: None,
        });

        let pii_total: usize = token_res.pii_counts.values().sum();
        layers.push(LayerReport {
            layer: 2,
            name: "DPDP PII Tokenizer".into(),
            status: if token_res.has_pii { LayerStatus::Masked } else { LayerStatus::Pass },
            latency_us: l2_us,
            detail: if token_res.has_pii {
                format!("Masked {} PII token(s) into 128-bit UUID vault.", pii_total)
            } else {
                "No raw PII detected in prompt.".into()
            },
            metadata: Some(json!({
                "counts": token_res.pii_counts,
                "aadhaar_verhoeff_verified": token_res.aadhaar_verhoeff_verified,
            })),
        });

        // LAYER 3: AI Guardrails + Google Gemini Decisions API
        let t3 = Instant::now();
        let decision = self
            .decision_client
            .evaluate_prompt(&token_res.sanitized_text, &cfg.google_ai)
            .await;
        let l3_us = t3.elapsed().as_micros() as u64;

        if decision.is_injection {
            layers.push(LayerReport {
                layer: 3,
                name: "Gemini Decisions AI Guardrail".into(),
                status: LayerStatus::Blocked,
                latency_us: l3_us,
                detail: decision.reason.clone(),
                metadata: Some(json!({
                    "source": decision.decision_source,
                    "risk_level": decision.risk_level,
                    "injection_probability": decision.injection_probability,
                    "cached": decision.cached,
                })),
            });

            let overhead_ms = round_ms(t_total.elapsed().as_micros());
            let block_reply = "Request blocked by AI semantic guardrail due to policy violation risk.";

            if req.persist_chat {
                let _ = self.state_store.append_message(
                    &format!("MSG-U-{}", &event_id[6..]),
                    &session_id,
                    "customer",
                    &token_res.sanitized_text,
                    &req.prompt,
                    &token_res.pii_counts,
                    None,
                    None,
                    None,
                    None,
                    0.0,
                );
                let _ = self.state_store.append_message(
                    &format!("MSG-A-{}", &event_id[6..]),
                    &session_id,
                    "assistant",
                    block_reply,
                    block_reply,
                    &HashMap::new(),
                    Some(&json!(layers)),
                    None,
                    Some(&decision.decision_source),
                    Some("BLOCKED"),
                    overhead_ms,
                );
            }

            self.telemetry.record_event(TelemetryEvent {
                event_id: event_id.clone(),
                session_id: Some(session_id.clone()),
                account_id: Some(account_id_resolved.clone()),
                timestamp: Utc::now().timestamp_millis() as f64 / 1000.0,
                iso_time: Local::now().format("%H:%M:%S").to_string(),
                event_type: "BLOCKED".into(),
                target_model: cfg.google_ai.decision_model.clone(),
                decision_source: Some(decision.decision_source.clone()),
                overhead_ms,
                user_prompt_sanitized: token_res.sanitized_text.clone(),
                pii_masked: token_res.pii_counts.clone(),
                attack_type: Some("AI_GUARDRAIL_VIOLATION".into()),
                tool_name: None,
                tool_args: None,
                violation_code: Some("GEMINI_DECISION_BLOCK".into()),
                reason: Some(decision.reason.clone()),
                layers: layers.clone(),
            });

            self.sentinel.enqueue(SentinelJob {
                event_id: event_id.clone(),
                session_id: Some(session_id.clone()),
                account_id: Some(account_id_resolved),
                shield_active: true,
                sanitized_prompt: token_res.sanitized_text.clone(),
                reply_sanitized: block_reply.to_string(),
                event_type: "BLOCKED".into(),
                tool_traces: vec![],
                recent_history,
            });

            return PipelineOutcome {
                event_id,
                session_id,
                status: "BLOCKED".into(),
                shield_active: true,
                overhead_ms,
                pii_detected: token_res.has_pii,
                pii_counts: token_res.pii_counts,
                sanitized_prompt: token_res.sanitized_text,
                reply: block_reply.to_string(),
                model_used: cfg.google_ai.decision_model,
                decision_source: Some(decision.decision_source.clone()),
                gemini_decision: Some(decision.clone()),
                violation_code: Some("GEMINI_DECISION_BLOCK".into()),
                reason: Some(decision.reason),
                layers,
                tool_traces: vec![],
                created_overrides: vec![],
            };
        }

        layers.push(LayerReport {
            layer: 3,
            name: "Gemini Decisions AI Guardrail".into(),
            status: if decision.decision_source == "DISABLED" {
                LayerStatus::Skipped
            } else {
                LayerStatus::Pass
            },
            latency_us: l3_us,
            detail: format!("{} ({})", decision.reason, decision.decision_source),
            metadata: Some(json!({
                "source": decision.decision_source,
                "risk_level": decision.risk_level,
                "injection_probability": decision.injection_probability,
                "cached": decision.cached,
            })),
        });

        // AGENT EXECUTION + LAYER 4 (RBI CIRCUIT BREAKER)
        let t4 = Instant::now();
        let agent_out = self
            .agent
            .run_turn(
                &cfg,
                &self.moneymax_db,
                &self.state_store,
                &token_res.sanitized_text,
                Some(&account_id_resolved),
                &recent_history,
                true,
                req.explicit_tools.as_deref(),
            )
            .await;
        let l4_us = t4.elapsed().as_micros() as u64;

        let tripped_trace = agent_out.tool_traces.iter().find(|t| !t.allowed);
        let circuit_tripped = tripped_trace.is_some();

        layers.push(LayerReport {
            layer: 4,
            name: "RBI Statutory Circuit Breaker".into(),
            status: if circuit_tripped {
                if agent_out.held_overrides.is_empty() {
                    LayerStatus::Blocked
                } else {
                    LayerStatus::Held
                }
            } else {
                LayerStatus::Pass
            },
            latency_us: l4_us,
            detail: if let Some(tr) = tripped_trace {
                tr.reason.clone()
            } else if agent_out.tool_traces.is_empty() {
                "No financial mutation tools invoked.".into()
            } else {
                format!("{} tool call(s) passed live RBI & IDOR checks.", agent_out.tool_traces.len())
            },
            metadata: Some(json!({
                "tools_count": agent_out.tool_traces.len(),
                "held_overrides": agent_out.held_overrides.len(),
            })),
        });

        // LAYER 5: Outbound Response Firewall
        let t5 = Instant::now();
        let out_check = self
            .outbound
            .inspect_and_sanitize(&agent_out.reply_draft, &session_id, &cfg.dpdp_act);
        let l5_us = t5.elapsed().as_micros() as u64;

        let out_pii_count: usize = out_check.pii_scrubbed_counts.values().sum();
        layers.push(LayerReport {
            layer: 5,
            name: "Outbound Response Firewall".into(),
            status: if out_check.prompt_leak_blocked {
                LayerStatus::Blocked
            } else if out_pii_count > 0 {
                LayerStatus::Masked
            } else {
                LayerStatus::Pass
            },
            latency_us: l5_us,
            detail: if out_check.prompt_leak_blocked {
                "Blocked internal prompt leakage in model response.".into()
            } else if out_pii_count > 0 {
                format!("Scrubbed {} sensitive PII fields from outbound reply.", out_pii_count)
            } else {
                "Outbound response verified clean.".into()
            },
            metadata: Some(json!(out_check.pii_scrubbed_counts)),
        });

        // LAYER 6: Async AI Sentinel (queued non-blocking)
        layers.push(LayerReport {
            layer: 6,
            name: "Async AI Sentinel".into(),
            status: if cfg.sentinel.enabled {
                LayerStatus::Pass
            } else {
                LayerStatus::Skipped
            },
            latency_us: 4,
            detail: "Dispatched asynchronously to AI trajectory watcher.".into(),
            metadata: None,
        });

        let mut total_pii = token_res.pii_counts.clone();
        for (k, v) in &out_check.pii_scrubbed_counts {
            *total_pii.entry(k.clone()).or_insert(0) += v;
        }

        // Persist Maker-Checker pending overrides for any held tool calls
        let mut created_overrides = Vec::new();
        if req.persist_chat && !agent_out.held_overrides.is_empty() {
            for prop in &agent_out.held_overrides {
                let ov_id = format!("OV-{}", Uuid::new_v4().as_simple().to_string()[..10].to_uppercase());
                if let Ok(rec) = self.state_store.create_pending_override(
                    &ov_id,
                    &session_id,
                    &account_id_resolved,
                    &prop.tool_name,
                    &prop.tool_args,
                    &prop.violation_code,
                    &prop.reason,
                ) {
                    created_overrides.push(rec);
                }
            }
            let _ = self.state_store.update_session_status(
                &session_id,
                SessionStatus::NeedsOverride,
                None,
                Some(0.75),
            );
        }

        let status = if circuit_tripped {
            "CIRCUIT_BREAKER_TRIPPED"
        } else if !total_pii.is_empty() {
            "MASKED"
        } else {
            "PROCESSED"
        };

        let event_type = if circuit_tripped {
            "CIRCUIT_TRIP"
        } else if !total_pii.is_empty() {
            "MASKED"
        } else {
            "ALLOWED"
        };

        let overhead_ms = round_ms(t_total.elapsed().as_micros());

        if req.persist_chat {
            let _ = self.state_store.append_message(
                &format!("MSG-U-{}", &event_id[6..]),
                &session_id,
                "customer",
                &token_res.sanitized_text,
                &req.prompt,
                &token_res.pii_counts,
                None,
                None,
                None,
                None,
                0.0,
            );
            let _ = self.state_store.append_message(
                &format!("MSG-A-{}", &event_id[6..]),
                &session_id,
                "assistant",
                &out_check.safe_text,
                &agent_out.reply_draft,
                &out_check.pii_scrubbed_counts,
                Some(&json!(layers)),
                Some(&json!(agent_out.tool_traces)),
                Some(&decision.decision_source),
                Some(if circuit_tripped { "CIRCUIT_TRIP" } else { "PASS" }),
                overhead_ms,
            );
        }

        let violation_code = tripped_trace.and_then(|t| t.violation_code.clone());
        let reason = tripped_trace.map(|t| t.reason.clone());

        self.telemetry.record_event(TelemetryEvent {
            event_id: event_id.clone(),
            session_id: Some(session_id.clone()),
            account_id: Some(account_id_resolved.clone()),
            timestamp: Utc::now().timestamp_millis() as f64 / 1000.0,
            iso_time: Local::now().format("%H:%M:%S").to_string(),
            event_type: event_type.into(),
            target_model: agent_out.model_used.clone(),
            decision_source: Some(decision.decision_source.clone()),
            overhead_ms,
            user_prompt_sanitized: token_res.sanitized_text.clone(),
            pii_masked: total_pii.clone(),
            attack_type: None,
            tool_name: agent_out.tool_traces.first().map(|t| t.name.clone()),
            tool_args: agent_out.tool_traces.first().map(|t| t.args.clone()),
            violation_code: violation_code.clone(),
            reason: reason.clone(),
            layers: layers.clone(),
        });

        self.sentinel.enqueue(SentinelJob {
            event_id: event_id.clone(),
            session_id: Some(session_id.clone()),
            account_id: Some(account_id_resolved),
            shield_active: true,
            sanitized_prompt: token_res.sanitized_text.clone(),
            reply_sanitized: out_check.safe_text.clone(),
            event_type: event_type.into(),
            tool_traces: agent_out.tool_traces.clone(),
            recent_history,
        });

        PipelineOutcome {
            event_id,
            session_id,
            status: status.into(),
            shield_active: true,
            overhead_ms,
            pii_detected: !total_pii.is_empty(),
            pii_counts: total_pii,
            sanitized_prompt: token_res.sanitized_text,
            reply: out_check.safe_text,
            model_used: agent_out.model_used,
            decision_source: Some(decision.decision_source.clone()),
            gemini_decision: Some(decision),
            violation_code,
            reason,
            layers,
            tool_traces: agent_out.tool_traces,
            created_overrides,
        }
    }
}

fn round_ms(micros: u128) -> f64 {
    ((micros as f64 / 1000.0) * 1000.0).round() / 1000.0
}
