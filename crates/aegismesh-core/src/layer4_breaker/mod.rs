use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::PolicyConfig;
use crate::db::moneymax::MoneyMaxDb;
use crate::db::state_store::StateStore;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub name: String,
    pub args: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AccountContext {
    pub account_id: Option<String>,
    pub is_frozen: Option<bool>,
    pub kyc_status: Option<String>,
    pub credit_limit: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerDecision {
    pub allowed: bool,
    pub escalate_to_staff: bool,
    pub reason: String,
    pub violation_code: Option<String>,
    pub tool_name: String,
    pub sanitized_args: Option<Value>,
}

#[derive(Clone, Default)]
pub struct CircuitBreakerEvaluator;

impl CircuitBreakerEvaluator {
    pub fn new() -> Self {
        Self
    }

    pub fn evaluate(
        &self,
        tool_call: &ToolCall,
        policy: &PolicyConfig,
        session_account_ctx: Option<&AccountContext>,
        moneymax_db: Option<&MoneyMaxDb>,
        state_store: Option<&StateStore>,
    ) -> CircuitBreakerDecision {
        let tool_name = tool_call.name.trim().to_string();

        // 1. Strict JSON object args validation
        let args = match tool_call.args.as_ref() {
            Some(Value::Object(map)) => Value::Object(map.clone()),
            None => Value::Object(serde_json::Map::new()),
            Some(_) => {
                return CircuitBreakerDecision {
                    allowed: false,
                    escalate_to_staff: false,
                    reason: "Invalid arguments format: tool arguments must be a structured JSON object.".into(),
                    violation_code: Some("ERR_INVALID_PARAMETERS".into()),
                    tool_name,
                    sanitized_args: None,
                };
            }
        };

        // 2. Authorized tool capability check
        if !policy.allowed_tools.iter().any(|t| t == &tool_name) {
            return CircuitBreakerDecision {
                allowed: false,
                escalate_to_staff: false,
                reason: format!("Tool '{}' is not in the authorized fintech capability registry.", tool_name),
                violation_code: Some("ERR_UNAUTHORIZED_TOOL".into()),
                tool_name,
                sanitized_args: None,
            };
        }

        // 3. IDOR / Confused Deputy protection against authenticated session account_id
        if let Some(ctx) = session_account_ctx {
            if let Some(ctx_acc) = ctx.account_id.as_deref() {
                if let Some(arg_acc) = args.get("account_id").and_then(|v| v.as_str()) {
                    if !arg_acc.trim().eq_ignore_ascii_case(ctx_acc.trim()) {
                        return CircuitBreakerDecision {
                            allowed: false,
                            escalate_to_staff: false,
                            reason: format!(
                                "Authorization mismatch (IDOR blocked): target account '{}' does not match session account '{}'.",
                                arg_acc, ctx_acc
                            ),
                            violation_code: Some("ERR_ACCOUNT_IDOR_MISMATCH".into()),
                            tool_name,
                            sanitized_args: None,
                        };
                    }
                }
                if tool_name == "get_customer_profile" {
                    if let Some(ident) = args.get("identifier").and_then(|v| v.as_str()) {
                        let up = ident.trim().to_uppercase();
                        if up.starts_with("MMX-AC-") && !up.eq_ignore_ascii_case(ctx_acc.trim()) {
                            return CircuitBreakerDecision {
                                allowed: false,
                                escalate_to_staff: false,
                                reason: format!(
                                    "Authorization mismatch (IDOR blocked): profile lookup '{}' does not match session account '{}'.",
                                    ident, ctx_acc
                                ),
                                violation_code: Some("ERR_ACCOUNT_IDOR_MISMATCH".into()),
                                tool_name,
                                sanitized_args: None,
                            };
                        }
                        if let Some(db) = moneymax_db {
                            if let Ok(Some(matched)) = db.get_customer_profile(ident) {
                                if !matched.account_id.eq_ignore_ascii_case(ctx_acc.trim()) {
                                    return CircuitBreakerDecision {
                                        allowed: false,
                                        escalate_to_staff: false,
                                        reason: format!(
                                            "Authorization mismatch (IDOR blocked): identifier '{}' belongs to '{}', not session account '{}'.",
                                            ident, matched.account_id, ctx_acc
                                        ),
                                        violation_code: Some("ERR_ACCOUNT_IDOR_MISMATCH".into()),
                                        tool_name,
                                        sanitized_args: None,
                                    };
                                }
                            }
                        }
                    }
                }
            }
        }

        // Resolve live account state directly from moneymax.db (never trust client headers alone)
        let target_account_id = args
            .get("account_id")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .or_else(|| session_account_ctx.and_then(|c| c.account_id.clone()));

        let live_ctx = target_account_id
            .as_deref()
            .and_then(|acc_id| moneymax_db.and_then(|db| db.get_account_context(acc_id).ok().flatten()));

        let effective_frozen = live_ctx
            .as_ref()
            .map(|c| c.is_frozen)
            .or_else(|| session_account_ctx.and_then(|c| c.is_frozen))
            .unwrap_or(false);

        let effective_kyc = live_ctx
            .as_ref()
            .map(|c| c.kyc_status.clone())
            .or_else(|| session_account_ctx.and_then(|c| c.kyc_status.clone()))
            .unwrap_or_default()
            .trim()
            .to_uppercase();

        let effective_credit_limit = live_ctx
            .as_ref()
            .map(|c| c.credit_limit)
            .or_else(|| session_account_ctx.and_then(|c| c.credit_limit))
            .or_else(|| args.get("current_limit").and_then(parse_number));

        // 4. Frozen account hold on state-changing or financial tools
        if policy.prohibit_frozen_accounts && effective_frozen {
            let acc = target_account_id.as_deref().unwrap_or("unknown");
            return CircuitBreakerDecision {
                allowed: false,
                escalate_to_staff: true,
                reason: format!(
                    "Account {} is FROZEN under compliance hold. Requires staff compliance officer review.",
                    acc
                ),
                violation_code: Some("ERR_ACCOUNT_FROZEN".into()),
                tool_name,
                sanitized_args: Some(args),
            };
        }

        // 5. execute_refund: RBI single cap + rolling 24h smurfing velocity breaker
        if tool_name == "execute_refund" {
            let amount = match args.get("amount").and_then(parse_number) {
                Some(a) if a.is_finite() && a >= 1.0 => a,
                Some(_) => {
                    return CircuitBreakerDecision {
                        allowed: false,
                        escalate_to_staff: false,
                        reason: "Invalid refund amount: must be a finite number >= ₹1.00.".into(),
                        violation_code: Some("ERR_INVALID_PARAMETERS".into()),
                        tool_name,
                        sanitized_args: None,
                    };
                }
                None => {
                    return CircuitBreakerDecision {
                        allowed: false,
                        escalate_to_staff: false,
                        reason: "Missing or unparseable refund amount.".into(),
                        violation_code: Some("ERR_INVALID_PARAMETERS".into()),
                        tool_name,
                        sanitized_args: None,
                    };
                }
            };

            if amount > policy.max_auto_refund_amount {
                return CircuitBreakerDecision {
                    allowed: false,
                    escalate_to_staff: true,
                    reason: format!(
                        "RBI Fair Practices Code hold: Refund ₹{:.2} exceeds autonomous ₹{:.2} cap. Escalated for Maker-Checker staff approval.",
                        amount, policy.max_auto_refund_amount
                    ),
                    violation_code: Some("ERR_REFUND_LIMIT_EXCEEDED".into()),
                    tool_name,
                    sanitized_args: Some(args),
                };
            }

            if let (Some(acc_id), Some(store)) = (target_account_id.as_deref(), state_store) {
                let prior_24h = store.sum_refunds_last_24h(acc_id).unwrap_or(0.0);
                if prior_24h + amount > policy.max_24h_refund_total {
                    return CircuitBreakerDecision {
                        allowed: false,
                        escalate_to_staff: true,
                        reason: format!(
                            "Smurfing Velocity Breaker tripped: 24h prior refunds ₹{:.2} + ₹{:.2} = ₹{:.2} exceeds ₹{:.2} rolling 24h limit.",
                            prior_24h,
                            amount,
                            prior_24h + amount,
                            policy.max_24h_refund_total
                        ),
                        violation_code: Some("ERR_SMURFING_VELOCITY_EXCEEDED".into()),
                        tool_name,
                        sanitized_args: Some(args),
                    };
                }
            }
        }

        // 6. update_credit_limit: multiplier check against live credit_limit
        if tool_name == "update_credit_limit" {
            let new_limit = match args
                .get("new_limit")
                .or_else(|| args.get("credit_limit"))
                .and_then(parse_number)
            {
                Some(n) if n.is_finite() && n > 0.0 => n,
                _ => {
                    return CircuitBreakerDecision {
                        allowed: false,
                        escalate_to_staff: false,
                        reason: "Missing or invalid positive finite new credit limit.".into(),
                        violation_code: Some("ERR_INVALID_PARAMETERS".into()),
                        tool_name,
                        sanitized_args: None,
                    };
                }
            };

            if let Some(curr) = effective_credit_limit {
                if curr > 0.0 {
                    let max_allowed = curr * policy.max_credit_limit_multiplier;
                    if new_limit > max_allowed {
                        return CircuitBreakerDecision {
                            allowed: false,
                            escalate_to_staff: true,
                            reason: format!(
                                "Requested credit limit ₹{:.2} exceeds max allowed ₹{:.2} ({:.2}x current limit ₹{:.2}). Escalated for Maker-Checker review.",
                                new_limit, max_allowed, policy.max_credit_limit_multiplier, curr
                            ),
                            violation_code: Some("ERR_CREDIT_LIMIT_EXCEEDED".into()),
                            tool_name,
                            sanitized_args: Some(args),
                        };
                    }
                }
            }
        }

        // 7. update_kyc_status: block illegal autonomous escalation to VERIFIED
        if tool_name == "update_kyc_status" && policy.enforce_kyc_transition_rules {
            let new_status = args
                .get("new_status")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_uppercase();

            if new_status == "VERIFIED" && effective_kyc != "VERIFIED" {
                return CircuitBreakerDecision {
                    allowed: false,
                    escalate_to_staff: true,
                    reason: format!(
                        "KYC state-machine lockdown (fail-closed): autonomous AI cannot transition '{}' directly to VERIFIED without manual compliance audit.",
                        if effective_kyc.is_empty() { "UNKNOWN" } else { effective_kyc.as_str() }
                    ),
                    violation_code: Some("ERR_ILLEGAL_KYC_ESCALATION".into()),
                    tool_name,
                    sanitized_args: Some(args),
                };
            }
        }

        CircuitBreakerDecision {
            allowed: true,
            escalate_to_staff: false,
            reason: "Tool call verified against live RBI & DPDP policy.".into(),
            violation_code: None,
            tool_name,
            sanitized_args: Some(args),
        }
    }
}

fn parse_number(v: &Value) -> Option<f64> {
    if let Some(n) = v.as_f64() {
        Some(n)
    } else if let Some(s) = v.as_str() {
        let cleaned = s.replace([',', '₹', ' '], "");
        cleaned.parse::<f64>().ok()
    } else {
        None
    }
}
