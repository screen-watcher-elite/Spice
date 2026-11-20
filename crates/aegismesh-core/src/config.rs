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
            agent_model: "gemini-3.5-flash-lite".to_string(),
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
            model: "gemini-3-flash-preview".to_string(),
            auto_quarantine_threshold: 0.82,
            auto_revert_rogue_payouts: true,
        }
    }
}

fn load_dotenv_file(cfg: &mut AegisConfig, path: &Path, key_only: bool) {
    let Ok(raw) = fs::read_to_string(path) else {
        return;
    };
    let mut legacy_gemini_model: Option<String> = None;
    let mut explicit_agent_model = false;

    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            let k = k.trim();
            let v = v.trim().trim_matches('"').trim_matches('\'');
            if v.is_empty() {
                continue;
            }
            if key_only {
                if k == "GEMINI_API_KEY" {
                    cfg.google_ai.api_key = Some(v.to_string());
                }
                continue;
            }
            match k {
                "GEMINI_API_KEY" => {
                    cfg.google_ai.api_key = Some(v.to_string());
                }
                "GEMINI_AGENT_MODEL" => {
                    cfg.google_ai.agent_model = v.to_string();
                    explicit_agent_model = true;
                }
                "GEMINI_MODEL" => {
                    legacy_gemini_model = Some(v.to_string());
                }
                "GEMINI_DECISION_MODEL" => {
                    cfg.google_ai.decision_model = v.to_string();
                }
                "GEMINI_SENTINEL_MODEL" => {
                    cfg.sentinel.model = v.to_string();
                }
                "AEGIS_ADMIN_KEY" => {
                    cfg.firewall.admin_key = Some(v.to_string());
                }
                _ => {}
            }
        }
    }

    if !key_only && !explicit_agent_model {
        if let Some(m) = legacy_gemini_model {
            cfg.google_ai.agent_model = m;
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
            load_dotenv_file(&mut cfg, &env_path, false);
        }

        // Fallback API key lookup in sibling ../client_infra/.env (key-only so legacy GEMINI_MODEL does not downgrade 3-model defaults)
        if cfg.google_ai.api_key.is_none() {
            if let Some(sibling_env) = base_dir.parent().map(|p| p.join("client_infra").join(".env"))
            {
                if sibling_env.exists() {
                    load_dotenv_file(&mut cfg, &sibling_env, true);
                }
            }
        }

        if let Ok(k) = env::var("GEMINI_API_KEY") {
            if !k.trim().is_empty() {
                cfg.google_ai.api_key = Some(k.trim().to_string());
            }
        }
        let mut env_explicit_agent = false;
        if let Ok(am) = env::var("GEMINI_AGENT_MODEL") {
            if !am.trim().is_empty() {
                cfg.google_ai.agent_model = am.trim().to_string();
                env_explicit_agent = true;
            }
        }
        if !env_explicit_agent {
            if let Ok(m) = env::var("GEMINI_MODEL") {
                if !m.trim().is_empty() {
                    cfg.google_ai.agent_model = m.trim().to_string();
                }
            }
        }
        if let Ok(dm) = env::var("GEMINI_DECISION_MODEL") {
            if !dm.trim().is_empty() {
                cfg.google_ai.decision_model = dm.trim().to_string();
            }
        }
        if let Ok(sm) = env::var("GEMINI_SENTINEL_MODEL") {
            if !sm.trim().is_empty() {
                cfg.sentinel.model = sm.trim().to_string();
            }
        }
        if let Ok(ak) = env::var("AEGIS_ADMIN_KEY") {
            if !ak.trim().is_empty() {
                cfg.firewall.admin_key = Some(ak.trim().to_string());
            }
        }
        Self::apply_cloud_env(
            &mut cfg,
            env::var("AEGIS_HOST").ok().as_deref(),
            env::var("AEGIS_PORT").ok().as_deref(),
            env::var("PORT").ok().as_deref(),
            env::var("K_SERVICE").ok().as_deref(),
        );

        cfg
    }

    /// Apply container / Google Cloud Run runtime networking overrides (`PORT`, `K_SERVICE`, `AEGIS_HOST`, `AEGIS_PORT`).
    pub fn apply_cloud_env(
        cfg: &mut AegisConfig,
        aegis_host: Option<&str>,
        aegis_port: Option<&str>,
        cloud_port: Option<&str>,
        k_service: Option<&str>,
    ) {
        let has_cloud_port = cloud_port.and_then(|p| p.trim().parse::<u16>().ok()).is_some();
        let has_k_service = k_service.map(|s| !s.trim().is_empty()).unwrap_or(false);

        if let Some(h) = aegis_host.map(str::trim).filter(|s| !s.is_empty()) {
            cfg.server.host = h.to_string();
        } else if has_k_service || has_cloud_port {
            cfg.server.host = "0.0.0.0".to_string();
            cfg.server.auto_open_browser = false;
        }

        if let Some(p) = aegis_port.and_then(|v| v.trim().parse::<u16>().ok()) {
            cfg.server.port = p;
        } else if let Some(p) = cloud_port.and_then(|v| v.trim().parse::<u16>().ok()) {
            cfg.server.port = p;
        }
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
    pub decision_model: Option<String>,
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
        if let Some(dm) = patch.decision_model {
            if !dm.trim().is_empty() {
                guard.google_ai.decision_model = dm.trim().to_string();
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
