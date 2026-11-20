use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LayerStatus {
    Pass,
    Masked,
    Blocked,
    Held,
    Skipped,
    Alert,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerReport {
    pub layer: u8,
    pub name: String,
    pub status: LayerStatus,
    pub latency_us: u64,
    pub detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
}
