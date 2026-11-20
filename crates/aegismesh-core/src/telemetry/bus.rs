use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::broadcast;

use crate::db::state_store::StateStore;
use crate::types::LayerReport;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryEvent {
    pub event_id: String,
    pub session_id: Option<String>,
    pub account_id: Option<String>,
    pub timestamp: f64,
    pub iso_time: String,
    pub event_type: String, // "ALLOWED", "MASKED", "BLOCKED", "CIRCUIT_TRIP", "SENTINEL_ALERT", "STAFF_OVERRIDE", "UNPROTECTED_RUN"
    pub target_model: String,
    pub decision_source: Option<String>,
    pub overhead_ms: f64,
    pub user_prompt_sanitized: String,
    pub pii_masked: HashMap<String, usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attack_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_args: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub violation_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default)]
    pub layers: Vec<LayerReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryStats {
    pub total_requests: usize,
    pub blocked_attacks: usize,
    pub pii_masked_count: usize,
    pub circuit_trips: usize,
    pub sentinel_interventions: usize,
    pub staff_overrides: usize,
    pub avg_overhead_ms: f64,
    pub p50_overhead_ms: f64,
    pub p95_overhead_ms: f64,
    pub p99_overhead_ms: f64,
    pub active_connections: usize,
    pub recent_latencies_ms: Vec<f64>,
}

struct BusInner {
    max_history: usize,
    history: VecDeque<TelemetryEvent>,
    total_requests: usize,
    blocked_attacks: usize,
    pii_masked_count: usize,
    circuit_trips: usize,
    sentinel_interventions: usize,
    staff_overrides: usize,
    latencies: VecDeque<f64>,
}

#[derive(Clone)]
pub struct TelemetryBus {
    inner: Arc<RwLock<BusInner>>,
    broadcast_tx: broadcast::Sender<TelemetryEvent>,
    state_store: Option<StateStore>,
}

impl TelemetryBus {
    pub fn new(max_history: usize, state_store: Option<StateStore>) -> Self {
        let (tx, _rx) = broadcast::channel(512);
        let mut history = VecDeque::with_capacity(max_history);
        let mut total_requests = 0usize;
        let mut blocked_attacks = 0usize;
        let mut pii_masked_count = 0usize;
        let mut circuit_trips = 0usize;
        let mut sentinel_interventions = 0usize;
        let mut staff_overrides = 0usize;
        let mut latencies = VecDeque::with_capacity(200);

        if let Some(ref store) = state_store {
            if let Ok(mut saved) = store.load_recent_telemetry(max_history) {
                saved.reverse(); // chronological
                for v in saved {
                    if let Ok(ev) = serde_json::from_value::<TelemetryEvent>(v) {
                        total_requests += 1;
                        match ev.event_type.as_str() {
                            "BLOCKED" => blocked_attacks += 1,
                            "CIRCUIT_TRIP" => circuit_trips += 1,
                            "SENTINEL_ALERT" => sentinel_interventions += 1,
                            "STAFF_OVERRIDE" => staff_overrides += 1,
                            _ => {}
                        }
                        pii_masked_count += ev.pii_masked.values().sum::<usize>();
                        if ev.overhead_ms > 0.0 {
                            if latencies.len() >= 200 {
                                latencies.pop_front();
                            }
                            latencies.push_back(ev.overhead_ms);
                        }
                        if history.len() >= max_history {
                            history.pop_front();
                        }
                        history.push_back(ev);
                    }
                }
            }
        }

        let inner = BusInner {
            max_history,
            history,
            total_requests,
            blocked_attacks,
            pii_masked_count,
            circuit_trips,
            sentinel_interventions,
            staff_overrides,
            latencies,
        };

        Self {
            inner: Arc::new(RwLock::new(inner)),
            broadcast_tx: tx,
            state_store,
        }
    }

    pub fn record_event(&self, event: TelemetryEvent) {
        {
            let mut bus = self.inner.write();
            bus.total_requests += 1;
            match event.event_type.as_str() {
                "BLOCKED" => bus.blocked_attacks += 1,
                "CIRCUIT_TRIP" => bus.circuit_trips += 1,
                "SENTINEL_ALERT" => bus.sentinel_interventions += 1,
                "STAFF_OVERRIDE" => bus.staff_overrides += 1,
                _ => {}
            }
            let pii_sum: usize = event.pii_masked.values().sum();
            bus.pii_masked_count += pii_sum;

            if event.overhead_ms >= 0.0 {
                if bus.latencies.len() >= 200 {
                    bus.latencies.pop_front();
                }
                bus.latencies.push_back(event.overhead_ms);
            }

            if bus.history.len() >= bus.max_history {
                bus.history.pop_front();
            }
            bus.history.push_back(event.clone());
        }

        if let Some(ref store) = self.state_store {
            if let Ok(val) = serde_json::to_value(&event) {
                let _ = store.persist_telemetry_event(&event.event_id, &val);
            }
        }

        let _ = self.broadcast_tx.send(event);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<TelemetryEvent> {
        self.broadcast_tx.subscribe()
    }

    pub fn get_history(&self, limit: usize) -> Vec<TelemetryEvent> {
        let bus = self.inner.read();
        bus.history.iter().rev().take(limit).cloned().collect()
    }

    pub fn get_stats(&self) -> TelemetryStats {
        let bus = self.inner.read();
        let mut sorted: Vec<f64> = bus.latencies.iter().copied().collect();
        let (avg, p50, p95, p99) = if sorted.is_empty() {
            (0.0, 0.0, 0.0, 0.0)
        } else {
            let sum: f64 = sorted.iter().sum();
            let avg = ((sum / sorted.len() as f64) * 100.0).round() / 100.0;
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let pct = |q: f64| -> f64 {
                let idx = ((sorted.len() - 1) as f64 * q).round() as usize;
                ((sorted[idx] * 100.0).round()) / 100.0
            };
            (avg, pct(0.50), pct(0.95), pct(0.99))
        };

        TelemetryStats {
            total_requests: bus.total_requests,
            blocked_attacks: bus.blocked_attacks,
            pii_masked_count: bus.pii_masked_count,
            circuit_trips: bus.circuit_trips,
            sentinel_interventions: bus.sentinel_interventions,
            staff_overrides: bus.staff_overrides,
            avg_overhead_ms: avg,
            p50_overhead_ms: p50,
            p95_overhead_ms: p95,
            p99_overhead_ms: p99,
            active_connections: self.broadcast_tx.receiver_count(),
            recent_latencies_ms: bus.latencies.iter().rev().take(24).rev().copied().collect(),
        }
    }
}
