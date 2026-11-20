use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::Instant;

use base64::{engine::general_purpose, Engine as _};
use parking_lot::Mutex;
use regex::Regex;
use serde::{Deserialize, Serialize};

pub const MAX_PROMPT_CHARS: usize = 16_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyInspectionResult {
    pub is_safe: bool,
    pub attack_type: Option<String>,
    pub confidence: f64,
    pub rule_triggered: Option<String>,
    pub reason: Option<String>,
    pub decoded_payload_matched: bool,
}

struct RegexBank {
    html_comment: Regex,
    base64_token: Regex,
    unicode_escape: Regex,
    override_patterns: Vec<Regex>,
    extraction_patterns: Vec<Regex>,
    jailbreak_patterns: Vec<Regex>,
    delimiter_patterns: Vec<Regex>,
    tool_hijack_patterns: Vec<Regex>,
    sqli_patterns: Vec<Regex>,
}

static REGEX_BANK: OnceLock<RegexBank> = OnceLock::new();

fn bank() -> &'static RegexBank {
    REGEX_BANK.get_or_init(|| RegexBank {
        html_comment: Regex::new(r"<!--[\s\S]*?-->").unwrap(),
        base64_token: Regex::new(r"[A-Za-z0-9+/\-_]{20,}={0,2}").unwrap(),
        unicode_escape: Regex::new(r"\\u([0-9a-fA-F]{4})").unwrap(),
        override_patterns: vec![
            Regex::new(r"(?i)(?:urgent|system|admin|manager|supervisor|rbi|executive|root)\s+(?:override|bypass|escalation|authorization)").unwrap(),
            Regex::new(r"(?i)as\s+(?:an?\s+)?(?:authorized|chief|senior)?\s*(?:branch\s+)?(?:supervisor|manager|admin|officer)").unwrap(),
            Regex::new(r"(?i)(?:bypass|override|disable|skip)\s+(?:all\s+)?(?:security|financial|validation|guardrails?|policies|limits?)").unwrap(),
            Regex::new(r"(?i)override\s+(?:authorization|protocol|code)\s*[:#-]?\s*\w+").unwrap(),
            Regex::new(r"(?i)(?:limit|security|rules?)\s+(?:bypass|override)\s+kar").unwrap(),
        ],
        extraction_patterns: vec![
            Regex::new(r"(?i)ignore\s+(?:all\s+)?(?:previous|prior|above|earlier)\s+(?:instructions?|rules?|directives?)").unwrap(),
            Regex::new(r"(?i)disregard\s+(?:all\s+)?(?:previous|prior|above\s+)?(?:rules?|system|policies|instructions?)").unwrap(),
            Regex::new(r"(?i)(?:print|show|output|leak|reveal|dump|repeat)\s+(?:your\s+)?(?:exact\s+)?(?:system\s+prompt|developer\s+instructions?|internal\s+guidelines?|core\s+instructions?|hidden\s+prompt)").unwrap(),
            Regex::new(r"(?i)what\s+(?:are|were)\s+your\s+(?:exact\s+)?(?:initial|system)\s+instructions?").unwrap(),
            Regex::new(r"(?i)(?:sab|saare|pichle)\s+(?:rules?|instructions?|niyam)\s+(?:bhool|ignore|chhod)").unwrap(),
            Regex::new(r"(?i)(?:system\s+prompt|hidden\s+rules?)\s+(?:dikhao|batao|de\s+do)").unwrap(),
        ],
        jailbreak_patterns: vec![
            Regex::new(r"(?i)(?:developer\s+mode|dan\s+mode|do\s+anything\s+now|jailbreak\s+(?:mode|prompt|enabled))").unwrap(),
            Regex::new(r"(?i)act\s+as\s+(?:an?\s+)?(?:unfiltered|jailbroken|unrestricted)").unwrap(),
            Regex::new(r"(?i)disregard\s+(?:all\s+)?financial\s+policies").unwrap(),
            Regex::new(r"(?i)pretend\s+(?:there\s+are\s+)?no\s+(?:rules|limits|restrictions|guardrails)").unwrap(),
        ],
        delimiter_patterns: vec![
            Regex::new(r"(?i)(?:<\|im_start\|>|<\|im_end\|>|<\|system\|>|<\|user\|>|<\|assistant\|>|\[/?(?:SYSTEM|ADMIN|INST)\]|<<SYS>>|<system(?:\s+[^>]*)?>|</system>|```\s*(?:system|override|admin))").unwrap(),
        ],
        tool_hijack_patterns: vec![
            Regex::new(r#"(?i)(?:force|invoke|call|execute)\s+tool\s+['"]?(?:execute_refund|update_kyc_status|update_credit_limit)['"]?"#).unwrap(),
            Regex::new(r#"(?i)\{"\s*(?:functionCall|tool_call|name)\s*"\s*:\s*"(?:execute_refund|update_kyc_status|update_credit_limit)""#).unwrap(),
        ],
        sqli_patterns: vec![
            Regex::new(r"(?i)(?:\bunion\s+(?:all\s+)?select\b|;\s*(?:drop|delete|truncate|alter)\s+table\b|\bor\s+1\s*=\s*1\b)").unwrap(),
        ],
    })
}

fn fold_confusable(c: char) -> Option<char> {
    match c {
        '\u{00AD}'
        | '\u{200B}'..='\u{200F}'
        | '\u{202A}'..='\u{202E}'
        | '\u{2060}'..='\u{2064}'
        | '\u{FEFF}' => None,
        '\u{00A0}' | '\u{2002}'..='\u{200A}' | '\u{202F}' | '\u{3000}' => Some(' '),
        '\u{FF01}'..='\u{FF5E}' => char::from_u32(c as u32 - 0xFEE0),
        'А' => Some('A'),
        'В' => Some('B'),
        'Е' => Some('E'),
        'К' => Some('K'),
        'М' => Some('M'),
        'Н' => Some('H'),
        'О' => Some('O'),
        'Р' => Some('P'),
        'С' => Some('C'),
        'Т' => Some('T'),
        'Х' => Some('X'),
        'а' => Some('a'),
        'е' => Some('e'),
        'о' => Some('o'),
        'р' => Some('p'),
        'с' => Some('c'),
        'у' => Some('y'),
        'х' => Some('x'),
        'і' => Some('i'),
        'Α' => Some('A'),
        'Β' => Some('B'),
        'Ε' => Some('E'),
        'Ο' => Some('O'),
        'Ρ' => Some('P'),
        'Τ' => Some('T'),
        'α' => Some('a'),
        'ο' => Some('o'),
        'ρ' => Some('p'),
        _ => Some(c),
    }
}

fn decode_percent_and_unicode(raw: &str) -> String {
    // 1. Decode %XX hex sequences
    let bytes = raw.as_bytes();
    let mut pct_out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(hex_str) = std::str::from_utf8(&bytes[i + 1..i + 3]) {
                if let Ok(val) = u8::from_str_radix(hex_str, 16) {
                    pct_out.push(val);
                    i += 3;
                    continue;
                }
            }
        }
        pct_out.push(bytes[i]);
        i += 1;
    }
    let pct_str = String::from_utf8_lossy(&pct_out).into_owned();

    // 2. Decode \uXXXX escapes
    bank()
        .unicode_escape
        .replace_all(&pct_str, |caps: &regex::Captures| {
            u32::from_str_radix(&caps[1], 16)
                .ok()
                .and_then(char::from_u32)
                .map(|c| c.to_string())
                .unwrap_or_else(|| caps[0].to_string())
        })
        .into_owned()
}

fn fold_leetspeak(normalized: &str) -> String {
    normalized
        .chars()
        .map(|c| match c {
            '0' => 'o',
            '1' => 'i',
            '3' => 'e',
            '4' => 'a',
            '5' => 's',
            '7' => 't',
            '@' => 'a',
            '$' => 's',
            _ => c,
        })
        .collect()
}

fn collapse_spaced_letters(normalized: &str) -> String {
    let tokens: Vec<&str> = normalized.split_whitespace().collect();
    if tokens.len() < 4 {
        return normalized.to_string();
    }
    let mut out = String::with_capacity(normalized.len());
    let mut i = 0;
    while i < tokens.len() {
        if tokens[i].len() == 1 && tokens[i].chars().all(|c| c.is_ascii_alphabetic()) {
            let mut joined = String::new();
            while i < tokens.len()
                && tokens[i].len() == 1
                && tokens[i].chars().all(|c| c.is_ascii_alphabetic())
            {
                joined.push_str(tokens[i]);
                i += 1;
            }
            if !out.is_empty() {
                out.push(' ');
            }
            out.push_str(&joined);
        } else {
            if !out.is_empty() {
                out.push(' ');
            }
            out.push_str(tokens[i]);
            i += 1;
        }
    }
    out
}

#[derive(Clone, Default)]
pub struct Layer1RegexEngine;

impl Layer1RegexEngine {
    pub fn new() -> Self {
        let _ = bank();
        Self
    }

    pub fn normalize_prompt(raw: &str) -> String {
        let decoded = decode_percent_and_unicode(raw);
        let stripped_comments = bank().html_comment.replace_all(&decoded, " ");
        let cleaned: String = stripped_comments
            .chars()
            .filter_map(fold_confusable)
            .collect();

        let unmarkdowned = cleaned.replace(['*', '_', '~', '`', '#', '|'], " ");
        unmarkdowned.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    fn extract_decoded_base64(text: &str) -> Vec<String> {
        let mut decoded_segments = Vec::new();
        for m in bank().base64_token.find_iter(text) {
            let tok = m.as_str();
            let decoded_bytes = general_purpose::STANDARD
                .decode(tok)
                .or_else(|_| general_purpose::STANDARD_NO_PAD.decode(tok))
                .or_else(|_| general_purpose::URL_SAFE.decode(tok))
                .or_else(|_| general_purpose::URL_SAFE_NO_PAD.decode(tok));
            if let Ok(bytes) = decoded_bytes {
                if let Ok(s) = String::from_utf8(bytes) {
                    if s.chars().filter(|c| c.is_ascii_graphic() || c.is_whitespace()).count() >= 10 {
                        decoded_segments.push(s);
                    }
                }
            }
        }
        decoded_segments
    }

    fn scan_single(&self, raw: &str, normalized: &str, from_decoded: bool) -> Option<SafetyInspectionResult> {
        let leet = fold_leetspeak(normalized);
        let collapsed = collapse_spaced_letters(normalized);

        let checks: &[(&[Regex], &str, f64, &str)] = &[
            (
                &bank().delimiter_patterns,
                "DELIMITER_ESCAPE",
                0.96,
                "Frontier model delimiter escape or fake system tag detected.",
            ),
            (
                &bank().override_patterns,
                "SUPERVISORY_OVERRIDE_INJECTION",
                0.95,
                "Adversarial supervisory override or authorization escalation detected.",
            ),
            (
                &bank().extraction_patterns,
                "SYSTEM_PROMPT_EXTRACTION",
                0.98,
                "System prompt exfiltration or instruction disregard attempt detected.",
            ),
            (
                &bank().jailbreak_patterns,
                "JAILBREAK_PERSONA",
                0.93,
                "Unfiltered persona or jailbreak mode attempt detected.",
            ),
            (
                &bank().tool_hijack_patterns,
                "TOOL_HIJACK_INJECTION",
                0.94,
                "Direct tool invocation spoofing attempt detected.",
            ),
            (
                &bank().sqli_patterns,
                "SQL_INJECTION_PAYLOAD",
                0.97,
                "SQL command injection pattern detected in prompt.",
            ),
        ];

        for (patterns, attack_type, confidence, reason) in checks {
            for pat in *patterns {
                if pat.is_match(raw)
                    || pat.is_match(normalized)
                    || pat.is_match(&leet)
                    || pat.is_match(&collapsed)
                {
                    return Some(SafetyInspectionResult {
                        is_safe: false,
                        attack_type: Some((*attack_type).to_string()),
                        confidence: *confidence,
                        rule_triggered: Some(pat.as_str().to_string()),
                        reason: Some(if from_decoded {
                            format!("{} (discovered inside encoded payload)", reason)
                        } else {
                            (*reason).to_string()
                        }),
                        decoded_payload_matched: from_decoded,
                    });
                }
            }
        }
        None
    }

    pub fn inspect(&self, prompt: &str) -> SafetyInspectionResult {
        if prompt.chars().count() > MAX_PROMPT_CHARS {
            return SafetyInspectionResult {
                is_safe: false,
                attack_type: Some("PAYLOAD_TOO_LARGE_DOS".to_string()),
                confidence: 1.0,
                rule_triggered: Some("max_prompt_chars".to_string()),
                reason: Some(format!(
                    "Prompt exceeds maximum permitted length ({} chars).",
                    MAX_PROMPT_CHARS
                )),
                decoded_payload_matched: false,
            };
        }

        let clean_text = prompt.trim();
        if clean_text.is_empty() {
            return SafetyInspectionResult {
                is_safe: true,
                attack_type: None,
                confidence: 0.0,
                rule_triggered: None,
                reason: Some("Empty prompt cleared.".to_string()),
                decoded_payload_matched: false,
            };
        }

        let normalized = Self::normalize_prompt(clean_text);
        if let Some(hit) = self.scan_single(clean_text, &normalized, false) {
            return hit;
        }

        for decoded in Self::extract_decoded_base64(clean_text) {
            let norm_dec = Self::normalize_prompt(&decoded);
            if let Some(hit) = self.scan_single(&decoded, &norm_dec, true) {
                return hit;
            }
        }

        SafetyInspectionResult {
            is_safe: true,
            attack_type: None,
            confidence: 0.0,
            rule_triggered: None,
            reason: Some("Layer 1 regex & normalization cleared.".to_string()),
            decoded_payload_matched: false,
        }
    }
}

struct Bucket {
    tokens: f64,
    last_refill: Instant,
}

#[derive(Clone)]
pub struct RateLimiter {
    buckets: std::sync::Arc<Mutex<HashMap<String, Bucket>>>,
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

impl RateLimiter {
    pub fn new() -> Self {
        Self {
            buckets: std::sync::Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn check(&self, key: &str, rpm_limit: u32) -> bool {
        let limit = rpm_limit.max(10) as f64;
        let refill_per_sec = limit / 60.0;
        let now = Instant::now();

        let mut map = self.buckets.lock();
        if map.len() > 4096 {
            map.retain(|_, b| now.duration_since(b.last_refill).as_secs() < 300);
        }

        let bucket = map.entry(key.to_string()).or_insert_with(|| Bucket {
            tokens: limit,
            last_refill: now,
        });

        let elapsed = now.duration_since(bucket.last_refill).as_secs_f64();
        bucket.tokens = (bucket.tokens + elapsed * refill_per_sec).min(limit);
        bucket.last_refill = now;

        if bucket.tokens >= 1.0 {
            bucket.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}
