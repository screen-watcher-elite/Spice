use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PiiRecord {
    pub token: String,
    pub original: String,
    pub pii_type: String,
    pub session_id: String,
    pub expires_at: u64,
}

#[derive(Clone)]
pub struct PiiVault {
    ttl_seconds: u64,
    store: Arc<RwLock<HashMap<String, HashMap<String, PiiRecord>>>>,
}

impl PiiVault {
    pub fn new(ttl_seconds: u64) -> Self {
        let vault = Self {
            ttl_seconds,
            store: Arc::new(RwLock::new(HashMap::new())),
        };
        if tokio::runtime::Handle::try_current().is_ok() {
            vault.spawn_background_cleaner(Duration::from_secs(30));
        }
        vault
    }

    fn current_timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    pub fn ttl_seconds(&self) -> u64 {
        self.ttl_seconds
    }

    pub fn store(&self, token: &str, original: &str, pii_type: &str, session_id: &str) {
        let expires_at = Self::current_timestamp() + self.ttl_seconds;
        let rec = PiiRecord {
            token: token.to_string(),
            original: original.to_string(),
            pii_type: pii_type.to_string(),
            session_id: session_id.to_string(),
            expires_at,
        };
        let mut lock = self.store.write();
        lock.entry(session_id.to_string())
            .or_default()
            .insert(token.to_string(), rec);
    }

    pub fn store_batch(&self, records: Vec<PiiRecord>) {
        if records.is_empty() {
            return;
        }
        let mut lock = self.store.write();
        for rec in records {
            lock.entry(rec.session_id.clone())
                .or_default()
                .insert(rec.token.clone(), rec);
        }
    }

    pub fn get(&self, token: &str, session_id: &str) -> Option<String> {
        let now = Self::current_timestamp();
        let mut lock = self.store.write();
        if let Some(session_map) = lock.get_mut(session_id) {
            if let Some(rec) = session_map.get(token) {
                if now <= rec.expires_at {
                    return Some(rec.original.clone());
                } else {
                    session_map.remove(token);
                }
            }
        }
        None
    }

    pub fn session_tokens(&self, session_id: &str) -> Vec<(String, String)> {
        let now = Self::current_timestamp();
        let lock = self.store.read();
        let mut out: Vec<(String, String)> = match lock.get(session_id) {
            Some(session_map) => session_map
                .values()
                .filter(|rec| now <= rec.expires_at)
                .map(|rec| (rec.token.clone(), rec.original.clone()))
                .collect(),
            None => Vec::new(),
        };
        out.sort_by_key(|b| std::cmp::Reverse(b.0.len()));
        out
    }

    pub fn rehydrate(&self, text: &str, session_id: &str) -> String {
        let now = Self::current_timestamp();
        let mut valid_records: Vec<(String, String)> = {
            let lock = self.store.read();
            match lock.get(session_id) {
                Some(session_map) => session_map
                    .values()
                    .filter(|rec| now <= rec.expires_at)
                    .map(|rec| (rec.token.clone(), rec.original.clone()))
                    .collect(),
                None => return text.to_string(),
            }
        };

        if valid_records.is_empty() {
            return text.to_string();
        }

        // Longest tokens first so short keys never corrupt composite tokens
        valid_records.sort_by_key(|b| std::cmp::Reverse(b.0.len()));

        let mut rehydrated = text.to_string();
        for (token, original) in valid_records {
            if rehydrated.contains(&token) {
                rehydrated = rehydrated.replace(&token, &original);
            }
        }
        rehydrated
    }

    pub fn cleanup_expired(&self) -> usize {
        let now = Self::current_timestamp();
        let mut lock = self.store.write();
        let mut removed = 0;
        lock.retain(|_, session_map| {
            let before = session_map.len();
            session_map.retain(|_, rec| now <= rec.expires_at);
            removed += before - session_map.len();
            !session_map.is_empty()
        });
        removed
    }

    pub fn spawn_background_cleaner(&self, interval: Duration) {
        let weak = Arc::downgrade(&self.store);
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            loop {
                ticker.tick().await;
                if let Some(arc) = weak.upgrade() {
                    let now = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    let mut lock = arc.write();
                    lock.retain(|_, session_map| {
                        session_map.retain(|_, rec| now <= rec.expires_at);
                        !session_map.is_empty()
                    });
                } else {
                    break;
                }
            }
        });
    }

    pub fn total_records(&self) -> usize {
        self.store.read().values().map(|m| m.len()).sum()
    }
}
