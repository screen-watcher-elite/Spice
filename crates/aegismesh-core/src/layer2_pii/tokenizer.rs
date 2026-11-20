use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use regex::Regex;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::config::{PiiConfig, TokenFormat};
use super::vault::{PiiRecord, PiiVault};

struct PiiRegexBank {
    email: Regex,
    card: Regex,
    aadhaar: Regex,
    phone: Regex,
    pan: Regex,
    ifsc: Regex,
    upi_vpa: Regex,
    bank_account: Regex,
    secret_code: Regex,
    non_digits: Regex,
}

static PII_BANK: OnceLock<PiiRegexBank> = OnceLock::new();

fn pii_bank() -> &'static PiiRegexBank {
    PII_BANK.get_or_init(|| PiiRegexBank {
        email: Regex::new(r"(?i)\b[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}\b").unwrap(),
        card: Regex::new(r"\b(?:\d[\s\-]?){13,19}\b").unwrap(),
        aadhaar: Regex::new(r"\b[2-9][0-9]{3}[\s\-\./\u{2013}]?[0-9]{4}[\s\-\./\u{2013}]?[0-9]{4}\b").unwrap(),
        phone: Regex::new(r"(?:(?:\+91[\-\s]?)|(?:\b0))?[6-9](?:\d{9}|\d{4}[\-\s]\d{5}|\d{2}[\-\s]\d{3}[\-\s]\d{4})\b").unwrap(),
        pan: Regex::new(r"(?i)\b[A-Z]{5}[0-9]{4}[A-Z]\b").unwrap(),
        ifsc: Regex::new(r"(?i)\b[A-Z]{4}0[A-Z0-9]{6}\b").unwrap(),
        upi_vpa: Regex::new(r"(?i)\b[a-z0-9._-]{2,}@(?:upi|okaxis|okhdfcbank|okicici|oksbi|ybl|paytm|ibl|axl|moneymax|icici|hdfcbank|sbi|axisbank|yesbank|billdesk)\b").unwrap(),
        bank_account: Regex::new(r"(?i)(?:a/c|acct|account\s*(?:no|num|number|#)?)\s*[:#-]?\s*(\d{9,18})\b").unwrap(),
        secret_code: Regex::new(r"(?i)\b(?:otp|cvv|cvc|pin|mpin)\s*(?:is|:|#|-)?\s*(\d{3,6})\b").unwrap(),
        non_digits: Regex::new(r"\D+").unwrap(),
    })
}

const VERHOEFF_D: [[u8; 10]; 10] = [
    [0, 1, 2, 3, 4, 5, 6, 7, 8, 9],
    [1, 2, 3, 4, 0, 6, 7, 8, 9, 5],
    [2, 3, 4, 0, 1, 7, 8, 9, 5, 6],
    [3, 4, 0, 1, 2, 8, 9, 5, 6, 7],
    [4, 0, 1, 2, 3, 9, 5, 6, 7, 8],
    [5, 9, 8, 7, 6, 0, 4, 3, 2, 1],
    [6, 5, 9, 8, 7, 1, 0, 4, 3, 2],
    [7, 6, 5, 9, 8, 2, 1, 0, 4, 3],
    [8, 7, 6, 5, 9, 3, 2, 1, 0, 4],
    [9, 8, 7, 6, 5, 4, 3, 2, 1, 0],
];

const VERHOEFF_P: [[u8; 10]; 8] = [
    [0, 1, 2, 3, 4, 5, 6, 7, 8, 9],
    [1, 5, 7, 6, 2, 8, 3, 0, 9, 4],
    [5, 8, 0, 3, 7, 9, 6, 1, 4, 2],
    [8, 9, 1, 6, 0, 4, 3, 5, 2, 7],
    [9, 4, 5, 3, 1, 2, 6, 8, 7, 0],
    [4, 2, 8, 6, 5, 7, 3, 9, 0, 1],
    [2, 7, 9, 3, 8, 0, 6, 4, 1, 5],
    [7, 0, 4, 6, 9, 1, 3, 2, 5, 8],
];

pub fn verify_verhoeff(digits: &str) -> bool {
    if digits.len() != 12 || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    let mut c: u8 = 0;
    for (i, b) in digits.bytes().rev().enumerate() {
        let num = (b - b'0') as usize;
        c = VERHOEFF_D[c as usize][VERHOEFF_P[i % 8][num] as usize];
    }
    c == 0
}

pub fn verify_luhn(digits: &str) -> bool {
    if !(13..=19).contains(&digits.len()) || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    let mut sum = 0u32;
    let mut double = false;
    for b in digits.bytes().rev() {
        let mut d = (b - b'0') as u32;
        if double {
            d *= 2;
            if d > 9 {
                d -= 9;
            }
        }
        sum += d;
        double = !double;
    }
    sum.is_multiple_of(10)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenizeResult {
    pub original_text: String,
    pub sanitized_text: String,
    pub has_pii: bool,
    pub pii_counts: HashMap<String, usize>,
    pub tokens_map: HashMap<String, String>,
    pub aadhaar_verhoeff_verified: usize,
}

#[derive(Clone)]
pub struct PiiTokenizer {
    pub vault: PiiVault,
    pub default_format: TokenFormat,
}

impl PiiTokenizer {
    pub fn new(vault: Option<PiiVault>) -> Self {
        let _ = pii_bank();
        Self {
            vault: vault.unwrap_or_else(|| PiiVault::new(300)),
            default_format: TokenFormat::AngleUuid,
        }
    }

    pub fn with_format(mut self, format: TokenFormat) -> Self {
        self.default_format = format;
        self
    }

    pub fn normalize_text(raw: &str) -> String {
        raw.chars()
            .filter_map(|c| match c {
                '\u{00AD}'
                | '\u{200B}'..='\u{200F}'
                | '\u{202A}'..='\u{202E}'
                | '\u{2060}'..='\u{2064}'
                | '\u{FEFF}' => None,
                '\u{00A0}' | '\u{2002}'..='\u{200A}' | '\u{202F}' | '\u{3000}' => Some(' '),
                '\u{FF21}'..='\u{FF3A}' => char::from_u32(c as u32 - 0xFEE0),
                '\u{FF41}'..='\u{FF5A}' => char::from_u32(c as u32 - 0xFEE0),
                '\u{FF10}'..='\u{FF19}' => char::from_u32(c as u32 - 0xFEE0),
                _ => Some(c),
            })
            .collect()
    }

    pub fn tokenize(&self, text: &str, session_id: &str) -> TokenizeResult {
        self.tokenize_with_config(text, session_id, &PiiConfig::default(), Some(self.default_format))
    }

    pub fn tokenize_with_format(
        &self,
        text: &str,
        session_id: &str,
        format: TokenFormat,
    ) -> TokenizeResult {
        self.tokenize_with_config(text, session_id, &PiiConfig::default(), Some(format))
    }

    pub fn tokenize_with_config(
        &self,
        text: &str,
        session_id: &str,
        cfg: &PiiConfig,
        format_override: Option<TokenFormat>,
    ) -> TokenizeResult {
        if text.is_empty() {
            return TokenizeResult {
                original_text: String::new(),
                sanitized_text: String::new(),
                has_pii: false,
                pii_counts: HashMap::new(),
                tokens_map: HashMap::new(),
                aadhaar_verhoeff_verified: 0,
            };
        }

        let format = format_override.unwrap_or(cfg.default_token_format);
        // Defeat user-supplied fake <PII:...> token injection
        let preprocessed = text.replace("<PII:", "&lt;PII:");
        let normalized = Self::normalize_text(&preprocessed);
        let b = pii_bank();

        #[derive(Debug)]
        struct PiiSpan<'a> {
            start: usize,
            end: usize,
            pii_type: &'static str,
            original: &'a str,
            priority: u8,
            verhoeff_ok: bool,
        }

        let mut candidates: Vec<PiiSpan> = Vec::new();

        if cfg.mask_upi {
            for m in b.upi_vpa.find_iter(&normalized) {
                candidates.push(PiiSpan {
                    start: m.start(),
                    end: m.end(),
                    pii_type: "UPI_VPA",
                    original: m.as_str(),
                    priority: 8,
                    verhoeff_ok: false,
                });
            }
        }

        if cfg.mask_email {
            for m in b.email.find_iter(&normalized) {
                candidates.push(PiiSpan {
                    start: m.start(),
                    end: m.end(),
                    pii_type: "EMAIL",
                    original: m.as_str(),
                    priority: 7,
                    verhoeff_ok: false,
                });
            }
        }

        if cfg.mask_card {
            for m in b.card.find_iter(&normalized) {
                let raw_match = m.as_str().trim();
                let digits = b.non_digits.replace_all(raw_match, "");
                if verify_luhn(&digits) {
                    candidates.push(PiiSpan {
                        start: m.start(),
                        end: m.end(),
                        pii_type: "CARD",
                        original: m.as_str(),
                        priority: 6,
                        verhoeff_ok: false,
                    });
                }
            }
        }

        if cfg.mask_aadhaar {
            for m in b.aadhaar.find_iter(&normalized) {
                let digits = b.non_digits.replace_all(m.as_str(), "");
                if digits.len() == 12 {
                    let v_ok = verify_verhoeff(&digits);
                    if !cfg.strict_aadhaar_verhoeff || v_ok {
                        candidates.push(PiiSpan {
                            start: m.start(),
                            end: m.end(),
                            pii_type: "AADHAAR",
                            original: m.as_str(),
                            priority: 5,
                            verhoeff_ok: v_ok,
                        });
                    }
                }
            }
        }

        if cfg.mask_phone {
            for m in b.phone.find_iter(&normalized) {
                let s = m.as_str();
                if !s.contains('[') && !s.contains(']') && !s.contains('<') && !s.contains('>') {
                    candidates.push(PiiSpan {
                        start: m.start(),
                        end: m.end(),
                        pii_type: "PHONE",
                        original: s,
                        priority: 4,
                        verhoeff_ok: false,
                    });
                }
            }
        }

        if cfg.mask_pan {
            for m in b.pan.find_iter(&normalized) {
                candidates.push(PiiSpan {
                    start: m.start(),
                    end: m.end(),
                    pii_type: "PAN",
                    original: m.as_str(),
                    priority: 3,
                    verhoeff_ok: false,
                });
            }
        }

        if cfg.mask_ifsc {
            for m in b.ifsc.find_iter(&normalized) {
                candidates.push(PiiSpan {
                    start: m.start(),
                    end: m.end(),
                    pii_type: "IFSC",
                    original: m.as_str(),
                    priority: 3,
                    verhoeff_ok: false,
                });
            }
        }

        if cfg.mask_secrets {
            for caps in b.bank_account.captures_iter(&normalized) {
                if let Some(m) = caps.get(1) {
                    candidates.push(PiiSpan {
                        start: m.start(),
                        end: m.end(),
                        pii_type: "BANK_ACCOUNT",
                        original: m.as_str(),
                        priority: 2,
                        verhoeff_ok: false,
                    });
                }
            }
            for caps in b.secret_code.captures_iter(&normalized) {
                if let Some(m) = caps.get(1) {
                    candidates.push(PiiSpan {
                        start: m.start(),
                        end: m.end(),
                        pii_type: "SECRET_CODE",
                        original: m.as_str(),
                        priority: 2,
                        verhoeff_ok: false,
                    });
                }
            }
        }

        // Enclosing-span precedence, then priority, then left-most start
        candidates.sort_by(|a, c| {
            (c.end - c.start)
                .cmp(&(a.end - a.start))
                .then_with(|| c.priority.cmp(&a.priority))
                .then_with(|| a.start.cmp(&c.start))
        });

        let mut accepted: Vec<PiiSpan> = Vec::new();
        for span in candidates {
            let overlaps = accepted
                .iter()
                .any(|acc| span.start < acc.end && acc.start < span.end);
            if !overlaps {
                accepted.push(span);
            }
        }
        accepted.sort_by_key(|s| s.start);

        let mut counts: HashMap<String, usize> = HashMap::new();
        let mut token_map: HashMap<String, String> = HashMap::new();
        let mut vault_records: Vec<PiiRecord> = Vec::new();
        let mut verhoeff_verified = 0usize;

        let now_ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let expires_at = now_ts + self.vault.ttl_seconds();

        let mut sanitized = String::with_capacity(normalized.len() + accepted.len() * 24);
        let mut last_idx = 0;

        for (counter, span) in (1..).zip(accepted) {
            if span.start > last_idx {
                sanitized.push_str(&normalized[last_idx..span.start]);
            }
            let token = match format {
                TokenFormat::AngleUuid => {
                    format!("<PII:{}:{}>", span.pii_type, Uuid::new_v4().as_simple())
                }
                TokenFormat::BracketIndexed => format!("[{}_TOKEN_{:02}]", span.pii_type, counter),
            };
            sanitized.push_str(&token);

            vault_records.push(PiiRecord {
                token: token.clone(),
                original: span.original.to_string(),
                pii_type: span.pii_type.to_string(),
                session_id: session_id.to_string(),
                expires_at,
            });
            token_map.insert(token, span.original.to_string());
            *counts.entry(span.pii_type.to_string()).or_insert(0) += 1;
            if span.verhoeff_ok {
                verhoeff_verified += 1;
            }
            last_idx = span.end;
        }

        if last_idx < normalized.len() {
            sanitized.push_str(&normalized[last_idx..]);
        }

        self.vault.store_batch(vault_records);
        let total_pii: usize = counts.values().sum();

        TokenizeResult {
            original_text: text.to_string(),
            sanitized_text: sanitized,
            has_pii: total_pii > 0,
            pii_counts: counts,
            tokens_map: token_map,
            aadhaar_verhoeff_verified: verhoeff_verified,
        }
    }
}
