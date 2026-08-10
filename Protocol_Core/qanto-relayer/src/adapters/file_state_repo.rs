//! File-based delivery state repository adapter.

use async_trait::async_trait;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use crate::error::{RelayerResult, StateError};
use crate::ports::DeliveryStateRepository;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryStatus {
    Pending,
    Delivered,
    Invalid,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DeliveryEntry {
    pub status: DeliveryStatus,
    pub attempts: u32,
    pub last_attempt_unix_secs: u64,
    pub last_error: Option<String>,
    pub delivered_tx: Option<String>,
}

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
struct DeliveryStateFile {
    entries: HashMap<String, DeliveryEntry>,
}

pub struct FileDeliveryStateRepo {
    path: PathBuf,
    state: DeliveryStateFile,
}

impl FileDeliveryStateRepo {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            state: DeliveryStateFile::default(),
        }
    }
}

#[async_trait]
impl DeliveryStateRepository for FileDeliveryStateRepo {
    async fn load(&mut self) -> RelayerResult<()> {
        self.state = match fs::read_to_string(&self.path) {
            Ok(contents) => serde_json::from_str(&contents).map_err(StateError::Serialization)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => DeliveryStateFile::default(),
            Err(e) => return Err(StateError::Io(e).into()),
        };
        Ok(())
    }

    async fn save(&self) -> RelayerResult<()> {
        if let Some(parent) = self.path.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent).map_err(StateError::Io)?;
        }
        let tmp_path = PathBuf::from(format!("{}.tmp", self.path.display()));
        let serialized =
            serde_json::to_string_pretty(&self.state).map_err(StateError::Serialization)?;
        fs::write(&tmp_path, serialized).map_err(StateError::Io)?;
        fs::rename(tmp_path, &self.path).map_err(StateError::Io)?;
        Ok(())
    }

    fn should_skip(&self, event_key: &str) -> bool {
        matches!(
            self.state.entries.get(event_key).map(|e| &e.status),
            Some(DeliveryStatus::Delivered | DeliveryStatus::Invalid)
        )
    }

    fn should_retry(&self, event_key: &str, now_unix_secs: u64, backoff_secs: u64) -> bool {
        self.state
            .entries
            .get(event_key)
            .is_none_or(|entry| match entry.status {
                DeliveryStatus::Pending => {
                    now_unix_secs.saturating_sub(entry.last_attempt_unix_secs) >= backoff_secs
                }
                DeliveryStatus::Delivered | DeliveryStatus::Invalid => false,
            })
    }

    fn mark_attempt(&mut self, event_key: &str, now_unix_secs: u64) {
        let entry = self
            .state
            .entries
            .entry(event_key.to_string())
            .or_insert_with(|| DeliveryEntry {
                status: DeliveryStatus::Pending,
                attempts: 0,
                last_attempt_unix_secs: now_unix_secs,
                last_error: None,
                delivered_tx: None,
            });
        entry.status = DeliveryStatus::Pending;
        entry.attempts = entry.attempts.saturating_add(1);
        entry.last_attempt_unix_secs = now_unix_secs;
    }

    fn mark_delivered(&mut self, event_key: &str, now_unix_secs: u64, claim_tx: String) {
        let entry = self
            .state
            .entries
            .entry(event_key.to_string())
            .or_insert_with(|| DeliveryEntry {
                status: DeliveryStatus::Delivered,
                attempts: 0,
                last_attempt_unix_secs: now_unix_secs,
                last_error: None,
                delivered_tx: Some(claim_tx.clone()),
            });
        entry.status = DeliveryStatus::Delivered;
        entry.last_attempt_unix_secs = now_unix_secs;
        entry.last_error = None;
        entry.delivered_tx = Some(claim_tx);
    }

    fn mark_invalid(&mut self, event_key: &str, now_unix_secs: u64, reason: String) {
        let entry = self
            .state
            .entries
            .entry(event_key.to_string())
            .or_insert_with(|| DeliveryEntry {
                status: DeliveryStatus::Invalid,
                attempts: 0,
                last_attempt_unix_secs: now_unix_secs,
                last_error: Some(reason.clone()),
                delivered_tx: None,
            });
        entry.status = DeliveryStatus::Invalid;
        entry.last_attempt_unix_secs = now_unix_secs;
        entry.last_error = Some(reason);
    }

    fn mark_retryable(
        &mut self,
        event_key: &str,
        now_unix_secs: u64,
        max_attempts: u32,
        error: String,
    ) {
        let entry = self
            .state
            .entries
            .entry(event_key.to_string())
            .or_insert_with(|| DeliveryEntry {
                status: DeliveryStatus::Pending,
                attempts: 0,
                last_attempt_unix_secs: now_unix_secs,
                last_error: Some(error.clone()),
                delivered_tx: None,
            });
        entry.status = DeliveryStatus::Pending;
        entry.last_attempt_unix_secs = now_unix_secs;
        entry.last_error = Some(error.clone());
        if max_attempts > 0 && entry.attempts >= max_attempts {
            entry.status = DeliveryStatus::Invalid;
            entry.last_error = Some(error);
        }
    }

    fn pending_retry_keys(&self, now_unix_secs: u64, backoff_secs: u64) -> Vec<String> {
        self.state
            .entries
            .iter()
            .filter_map(|(key, entry)| match entry.status {
                DeliveryStatus::Pending => {
                    if now_unix_secs.saturating_sub(entry.last_attempt_unix_secs) >= backoff_secs {
                        Some(key.clone())
                    } else {
                        None
                    }
                }
                DeliveryStatus::Delivered | DeliveryStatus::Invalid => None,
            })
            .collect()
    }

    fn prune_completed(&mut self, now_unix_secs: u64, retention_secs: u64) -> bool {
        if retention_secs == 0 {
            return false;
        }
        let before = self.state.entries.len();
        self.state.entries.retain(|_, entry| match entry.status {
            DeliveryStatus::Pending => true,
            DeliveryStatus::Delivered | DeliveryStatus::Invalid => {
                now_unix_secs.saturating_sub(entry.last_attempt_unix_secs) <= retention_secs
            }
        });
        before != self.state.entries.len()
    }
}
