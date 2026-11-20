use std::env;
use std::fs;
use std::path::Path;
use std::sync::Arc;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TokenFormat {
    #[default]
    AngleUuid,
    BracketIndexed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum OutboundPiiMode {
    #[default]
    MaskFull,
    Last4,
    RehydrateAuthorized,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum DecisionsMode {
    #[default]
    Smart,
    Strict,
    Off,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub auto_open_browser: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8080,
            auto_open_browser: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct FirewallConfig {
    pub default_shield_active: bool,
    pub policy_preset: String,
    pub admin_key: Option<String>,
    pub rate_limit_rpm: u32,
}

impl Default for FirewallConfig {
    fn default() -> Self {
        Self {
            default_shield_active: true,
            policy_preset: "RBI_DPDP_FINTECH_V1".to_string(),
            admin_key: None,
            rate_limit_rpm: 120,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PolicyConfig {
    pub max_auto_refund_amount: f64,
    pub max_24h_refund_total: f64,
    pub prohibit_frozen_accounts: bool,
    pub enforce_kyc_transition_rules: bool,
    pub max_credit_limit_multiplier: f64,
    pub allowed_tools: Vec<String>,
}

impl Default for PolicyConfig {
    fn default() -> Self {
        Self::rbi_dpdp_default()
    }
}

impl PolicyConfig {
    pub fn rbi_dpdp_default() -> Self {
        Self {
            max_auto_refund_amount: 5000.0,
            max_24h_refund_total: 5000.0,
            prohibit_frozen_accounts: true,
            enforce_kyc_transition_rules: true,
            max_credit_limit_multiplier: 1.25,
            allowed_tools: vec![
                "get_customer_profile".to_string(),
                "get_transaction_history".to_string(),
                "get_disputes".to_string(),
                "execute_refund".to_string(),
                "update_kyc_status".to_string(),
                "update_credit_limit".to_string(),
            ],
        }
    }

    pub fn strict_banking() -> Self {
        Self {
            max_auto_refund_amount: 2000.0,
            max_24h_refund_total: 2000.0,
            prohibit_frozen_accounts: true,
            enforce_kyc_transition_rules: true,
            max_credit_limit_multiplier: 1.10,
            allowed_tools: vec![
                "get_customer_profile".to_string(),
                "get_transaction_history".to_string(),
                "get_disputes".to_string(),
                "execute_refund".to_string(),
            ],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PiiConfig {
    pub vault_ttl_seconds: u64,
    pub default_token_format: TokenFormat,
    pub strict_aadhaar_verhoeff: bool,
    pub outbound_pii_mode: OutboundPiiMode,
    pub mask_pan: bool,
    pub mask_aadhaar: bool,
    pub mask_phone: bool,
    pub mask_email: bool,
    pub mask_card: bool,
    pub mask_ifsc: bool,
    pub mask_upi: bool,
    pub mask_secrets: bool,
}

impl Default for PiiConfig {
    fn default() -> Self {
        Self {
            vault_ttl_seconds: 300,
            default_token_format: TokenFormat::AngleUuid,
            strict_aadhaar_verhoeff: false,
            outbound_pii_mode: OutboundPiiMode::MaskFull,
            mask_pan: true,
            mask_aadhaar: true,
            mask_phone: true,
            mask_email: true,
            mask_card: true,
            mask_ifsc: true,
            mask_upi: true,
            mask_secrets: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GeminiConfig {
    pub api_key: Option<String>,
    pub agent_model: String,
    pub decisions_endpoint: String,
    pub decision_model: String,
    pub decisions_mode: DecisionsMode,
    pub decision_timeout_secs: u64,
    pub cache_ttl_secs: u64,
}

impl Default for GeminiConfig {
    fn default() -> Self {
        Self {
            api_key: None,
            agent_model: "gemini-3.1-flash-lite".to_string(),
            decisions_endpoint: "https://generativelanguage.googleapis.com/v1beta/decisions"
                .to_string(),
            decision_model: "gemini-decision-1".to_string(),
            decisions_mode: DecisionsMode::Smart,
            decision_timeout_secs: 6,
            cache_ttl_secs: 300,
        }
    }
}

impl GeminiConfig {
    pub fn has_live_key(&self) -> bool {
        self.api_key
            .as_deref()
            .map(|k| {
                let t = k.trim();
                t.len() > 10 && !t.starts_with("YOUR_")
            })
            .unwrap_or(false)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SentinelConfig {
    pub enabled: bool,
    pub model: String,
    pub auto_quarantine_threshold: f64,
    pub auto_revert_rogue_payouts: bool,
}

impl Default for SentinelConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            model: "gemini-3.1-flash-lite".to_string(),
            auto_quarantine_threshold: 0.82,
            auto_revert_rogue_payouts: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AegisConfig {
    pub server: ServerConfig,
    pub firewall: FirewallConfig,
    pub statutory_limits: PolicyConfig,
    pub dpdp_act: PiiConfig,
    pub google_ai: GeminiConfig,
    pub sentinel: SentinelConfig,
}

impl AegisConfig {
    pub fn load_from_dir(base_dir: &Path) -> Self {
        let mut cfg = Self::default();

        for filename in ["aegismesh.toml", "aegismesh.local.toml"] {
            let path = base_dir.join(filename);
            if path.exists() {
                if let Ok(raw) = fs::read_to_string(&path) {
                    if let Ok(parsed) = toml::from_str::<AegisConfig>(&raw) {
                        cfg = parsed;
                    }
                }
            }
        }

        // Load optional .env file if present locally (never committed)
        let env_path = base_dir.join(".env");
        if env_path.exists() {
            if let Ok(raw) = fs::read_to_string(&env_path) {
                for line in raw.lines() {
                    let line = line.trim();
                    if line.is_empty() || line.starts_with('#') {
                        continue;
                    }
                    if let Some((k, v)) = line.split_once('=') {
                        let k = k.trim();
                        let v = v.trim().trim_matches('"').trim_matches('\'');
                        match k {
                            "GEMINI_API_KEY" if !v.is_empty() => {
                                cfg.google_ai.api_key = Some(v.to_string());
                            }
                            "GEMINI_MODEL" if !v.is_empty() => {
                                cfg.google_ai.agent_model = v.to_string();
                                cfg.sentinel.model = v.to_string();
                            }
                            "AEGIS_ADMIN_KEY" if !v.is_empty() => {
                                cfg.firewall.admin_key = Some(v.to_string());
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        if let Ok(k) = env::var("GEMINI_API_KEY") {
            if !k.trim().is_empty() {
                cfg.google_ai.api_key = Some(k.trim().to_string());
            }
        }
        if let Ok(m) = env::var("GEMINI_MODEL") {
            if !m.trim().is_empty() {
                cfg.google_ai.agent_model = m.trim().to_string();
                cfg.sentinel.model = m.trim().to_string();
            }
        }
        if let Ok(ak) = env::var("AEGIS_ADMIN_KEY") {
            if !ak.trim().is_empty() {
                cfg.firewall.admin_key = Some(ak.trim().to_string());
            }
        }
        if let Ok(h) = env::var("AEGIS_HOST") {
            if !h.trim().is_empty() {
                cfg.server.host = h.trim().to_string();
            }
        }
        if let Ok(p) = env::var("AEGIS_PORT") {
            if let Ok(port) = p.trim().parse::<u16>() {
                cfg.server.port = port;
            }
        }

        cfg
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PublicSettingsView {
    pub policy_preset: String,
    pub rate_limit_rpm: u32,
    pub admin_key_configured: bool,
    pub statutory_limits: PolicyConfig,
    pub dpdp_act: PiiConfig,
    pub gemini_api_key_configured: bool,
    pub gemini_api_key_masked: Option<String>,
    pub agent_model: String,
    pub decision_model: String,
    pub decisions_endpoint: String,
    pub decisions_mode: DecisionsMode,
    pub sentinel_enabled: bool,
    pub sentinel_model: String,
    pub auto_quarantine_threshold: f64,
    pub auto_revert_rogue_payouts: bool,
}

fn mask_secret(opt: &Option<String>) -> (bool, Option<String>) {
    match opt.as_deref().map(str::trim) {
        Some(s) if s.len() > 8 && !s.starts_with("YOUR_") => {
            let tail = &s[s.len() - 4..];
            (true, Some(format!("••••••••{}", tail)))
        }
        Some(s) if !s.is_empty() && !s.starts_with("YOUR_") => {
            (true, Some("••••".to_string()))
        }
        _ => (false, None),
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct SettingsPatch {
    pub policy_preset: Option<String>,
    pub rate_limit_rpm: Option<u32>,
    pub admin_key: Option<String>,
    pub clear_admin_key: Option<bool>,
    pub gemini_api_key: Option<String>,
    pub clear_gemini_api_key: Option<bool>,
    pub agent_model: Option<String>,
    pub decisions_mode: Option<DecisionsMode>,
    pub sentinel_enabled: Option<bool>,
    pub sentinel_model: Option<String>,
    pub auto_quarantine_threshold: Option<f64>,
    pub auto_revert_rogue_payouts: Option<bool>,
    pub dpdp_act: Option<PiiConfig>,
    pub statutory_limits: Option<PolicyConfig>,
}

#[derive(Clone)]
pub struct ConfigStore {
    inner: Arc<RwLock<AegisConfig>>,
}

impl ConfigStore {
    pub fn new(cfg: AegisConfig) -> Self {
        Self {
            inner: Arc::new(RwLock::new(cfg)),
        }
    }

    pub fn snapshot(&self) -> AegisConfig {
        self.inner.read().clone()
    }

    pub fn replace(&self, cfg: AegisConfig) {
        *self.inner.write() = cfg;
    }

    pub fn update_policy(&self, preset: String, policy: PolicyConfig) {
        let mut guard = self.inner.write();
        guard.firewall.policy_preset = preset;
        guard.statutory_limits = policy;
    }

    pub fn apply_patch(&self, patch: SettingsPatch) -> AegisConfig {
        let mut guard = self.inner.write();
        if let Some(p) = patch.policy_preset {
            guard.firewall.policy_preset = p;
        }
        if let Some(rpm) = patch.rate_limit_rpm {
            guard.firewall.rate_limit_rpm = rpm.max(10);
        }
        if patch.clear_admin_key == Some(true) {
            guard.firewall.admin_key = None;
        } else if let Some(k) = patch.admin_key {
            let trimmed = k.trim();
            if !trimmed.is_empty() {
                guard.firewall.admin_key = Some(trimmed.to_string());
            }
        }
        if patch.clear_gemini_api_key == Some(true) {
            guard.google_ai.api_key = None;
        } else if let Some(k) = patch.gemini_api_key {
            let trimmed = k.trim();
            if !trimmed.is_empty() {
                guard.google_ai.api_key = Some(trimmed.to_string());
            }
        }
        if let Some(m) = patch.agent_model {
            if !m.trim().is_empty() {
                guard.google_ai.agent_model = m.trim().to_string();
            }
        }
        if let Some(dm) = patch.decisions_mode {
            guard.google_ai.decisions_mode = dm;
        }
        if let Some(se) = patch.sentinel_enabled {
            guard.sentinel.enabled = se;
        }
        if let Some(sm) = patch.sentinel_model {
            if !sm.trim().is_empty() {
                guard.sentinel.model = sm.trim().to_string();
            }
        }
        if let Some(th) = patch.auto_quarantine_threshold {
            guard.sentinel.auto_quarantine_threshold = th.clamp(0.3, 0.99);
        }
        if let Some(ar) = patch.auto_revert_rogue_payouts {
            guard.sentinel.auto_revert_rogue_payouts = ar;
        }
        if let Some(dpdp) = patch.dpdp_act {
            guard.dpdp_act = dpdp;
        }
        if let Some(pol) = patch.statutory_limits {
            guard.statutory_limits = pol;
        }
        guard.clone()
    }

    pub fn verify_admin_key(&self, supplied: Option<&str>) -> bool {
        let guard = self.inner.read();
        match guard.firewall.admin_key.as_deref().map(str::trim) {
            Some(expected) if !expected.is_empty() => {
                let Some(sup) = supplied.map(str::trim) else {
                    return false;
                };
                let h_exp = Sha256::digest(expected.as_bytes());
                let h_sup = Sha256::digest(sup.as_bytes());
                let mut diff = 0u8;
                for (a, b) in h_exp.iter().zip(h_sup.iter()) {
                    diff |= a ^ b;
                }
                diff == 0
            }
            _ => true,
        }
    }

    pub fn has_admin_key(&self) -> bool {
        self.inner
            .read()
            .firewall
            .admin_key
            .as_deref()
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false)
    }

    pub fn public_view(&self) -> PublicSettingsView {
        let guard = self.inner.read();
        let (admin_ok, _) = mask_secret(&guard.firewall.admin_key);
        let (gemini_ok, gemini_masked) = mask_secret(&guard.google_ai.api_key);
        PublicSettingsView {
            policy_preset: guard.firewall.policy_preset.clone(),
            rate_limit_rpm: guard.firewall.rate_limit_rpm,
            admin_key_configured: admin_ok,
            statutory_limits: guard.statutory_limits.clone(),
            dpdp_act: guard.dpdp_act.clone(),
            gemini_api_key_configured: gemini_ok,
            gemini_api_key_masked: gemini_masked,
            agent_model: guard.google_ai.agent_model.clone(),
            decision_model: guard.google_ai.decision_model.clone(),
            decisions_endpoint: guard.google_ai.decisions_endpoint.clone(),
            decisions_mode: guard.google_ai.decisions_mode,
            sentinel_enabled: guard.sentinel.enabled,
            sentinel_model: guard.sentinel.model.clone(),
            auto_quarantine_threshold: guard.sentinel.auto_quarantine_threshold,
            auto_revert_rogue_payouts: guard.sentinel.auto_revert_rogue_payouts,
        }
    }
}
