use std::collections::HashMap;
use std::sync::OnceLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::config::{OutboundPiiMode, PiiConfig};
use crate::layer2_pii::tokenizer::PiiTokenizer;

static LEAK_PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();

fn leak_patterns() -> &'static [Regex] {
    LEAK_PATTERNS.get_or_init(|| {
        vec![
            Regex::new(r"(?i)FINBOT_SYSTEM_PROMPT").unwrap(),
            Regex::new(r"(?i)NEVER\s+disclose\s+this\s+system\s+instruction").unwrap(),
            Regex::new(r"(?i)BEGIN\s+INTERNAL\s+POLICY\s+DIRECTIVE").unwrap(),
        ]
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboundCheckResult {
    pub safe_text: String,
    pub prompt_leak_blocked: bool,
    pub pii_scrubbed_counts: HashMap<String, usize>,
}

#[derive(Clone)]
pub struct OutboundFirewall {
    tokenizer: PiiTokenizer,
}

impl OutboundFirewall {
    pub fn new(tokenizer: PiiTokenizer) -> Self {
        let _ = leak_patterns();
        Self { tokenizer }
    }

    pub fn inspect_and_sanitize(
        &self,
        draft_reply: &str,
        session_id: &str,
        pii_cfg: &PiiConfig,
    ) -> OutboundCheckResult {
        if draft_reply.is_empty() {
            return OutboundCheckResult {
                safe_text: String::new(),
                prompt_leak_blocked: false,
                pii_scrubbed_counts: HashMap::new(),
            };
        }

        for pat in leak_patterns() {
            if pat.is_match(draft_reply) {
                return OutboundCheckResult {
                    safe_text: "I cannot disclose internal system instructions. How else can I assist with your MoneyMax account?".to_string(),
                    prompt_leak_blocked: true,
                    pii_scrubbed_counts: HashMap::new(),
                };
            }
        }

        // First selectively rehydrate session tokens if configured as RehydrateAuthorized,
        // otherwise scrub any raw PII coming from DB tool outputs!
        match pii_cfg.outbound_pii_mode {
            OutboundPiiMode::RehydrateAuthorized => {
                // Temporarily protect authentic vault tokens for THIS session before scrubbing
                // any raw DB-sourced PII or forged <PII:...> markers, then restore originals.
                let valid = self.tokenizer.vault.session_tokens(session_id);
                let mut staged = draft_reply.to_string();
                let mut restore_pairs: Vec<(String, String)> = Vec::new();
                for (idx, (tok, orig)) in valid.into_iter().enumerate() {
                    if staged.contains(&tok) {
                        let sentinel = format!("__AEGIS_REHYDRATE_{}__", idx);
                        staged = staged.replace(&tok, &sentinel);
                        restore_pairs.push((sentinel, orig));
                    }
                }

                let scrub = self
                    .tokenizer
                    .tokenize_with_config(&staged, "outbound_scrub_ephemeral", pii_cfg, None);
                let mut scrubbed_text = scrub.sanitized_text;
                for tok in scrub.tokens_map.keys() {
                    let label = tok
                        .strip_prefix("<PII:")
                        .and_then(|s| s.split(':').next())
                        .unwrap_or("PII");
                    scrubbed_text = scrubbed_text.replace(tok, &format!("[REDACTED:{}]", label));
                }
                for (sentinel, orig) in restore_pairs {
                    scrubbed_text = scrubbed_text.replace(&sentinel, &orig);
                }

                OutboundCheckResult {
                    safe_text: scrubbed_text,
                    prompt_leak_blocked: false,
                    pii_scrubbed_counts: scrub.pii_counts,
                }
            }
            OutboundPiiMode::Last4 => {
                let res = self.tokenizer.tokenize_with_config(draft_reply, session_id, pii_cfg, None);
                let mut text = res.sanitized_text;
                for (tok, orig) in &res.tokens_map {
                    let clean_chars: Vec<char> = orig.chars().filter(|c| !c.is_whitespace()).collect();
                    let masked = if clean_chars.len() > 4 {
                        let tail: String = clean_chars[clean_chars.len() - 4..].iter().collect();
                        format!("XXXX-{}", tail)
                    } else {
                        "[REDACTED]".to_string()
                    };
                    text = text.replace(tok, &masked);
                }
                OutboundCheckResult {
                    safe_text: text,
                    prompt_leak_blocked: false,
                    pii_scrubbed_counts: res.pii_counts,
                }
            }
            OutboundPiiMode::MaskFull => {
                let res = self.tokenizer.tokenize_with_config(draft_reply, session_id, pii_cfg, None);
                let mut text = res.sanitized_text;
                for tok in res.tokens_map.keys() {
                    let label = tok
                        .strip_prefix("<PII:")
                        .and_then(|s| s.split(':').next())
                        .unwrap_or("PII");
                    text = text.replace(tok, &format!("[REDACTED:{}]", label));
                }
                OutboundCheckResult {
                    safe_text: text,
                    prompt_leak_blocked: false,
                    pii_scrubbed_counts: res.pii_counts,
                }
            }
        }
    }
}
