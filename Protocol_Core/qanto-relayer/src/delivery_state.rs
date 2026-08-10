//! Copyright (C) 2026 Trevor <trvorth@qanto.org>
//!
//! This program is free software: you can redistribute it and/or modify
//! it under the terms of the GNU General Public License as published by
//! the Free Software Foundation, either version 3 of the License, or
//! (at your option) any later version.
//!
//! This program is distributed in the hope that it will be useful,
//! but WITHOUT ANY WARRANTY; without even the implied warranty of
//! MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
//! GNU General Public License for more details.
//!
//! You should have received a copy of the GNU General Public License
//! along with this program. If not, see <https://gnu.org>.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

type DynError = Box<dyn std::error::Error + Send + Sync>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryStatus {
    Pending,
    Delivered,
    Invalid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeliveryEntry {
    pub status: DeliveryStatus,
    pub attempts: u32,
    pub last_attempt_unix_secs: u64,
    pub last_error: Option<String>,
    pub delivered_tx: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct DeliveryStateFile {
    entries: HashMap<String, DeliveryEntry>,
}

#[derive(Debug)]
pub struct DeliveryStateStore {
    path: PathBuf,
    state: DeliveryStateFile,
}

impl DeliveryStateStore {
    pub fn load(path: impl Into<PathBuf>) -> Result<Self, DynError> {
        let path = path.into();
        let state = match fs::read_to_string(&path) {
            Ok(contents) => serde_json::from_str(&contents)?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                DeliveryStateFile::default()
            }
            Err(error) => return Err(error.into()),
        };
        Ok(Self { path, state })
    }

    pub fn save(&self) -> Result<(), DynError> {
        if let Some(parent) = self
            .path
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
        {
            fs::create_dir_all(parent)?;
        }

        let tmp_path = PathBuf::from(format!("{}.tmp", self.path.display()));
        let serialized = serde_json::to_string_pretty(&self.state)?;
        fs::write(&tmp_path, serialized)?;
        fs::rename(tmp_path, &self.path)?;
        Ok(())
    }

    pub fn should_skip(&self, event_key: &str) -> bool {
        matches!(
            self.state.entries.get(event_key).map(|entry| entry.status),
            Some(DeliveryStatus::Delivered | DeliveryStatus::Invalid)
        )
    }

    pub fn should_retry(&self, event_key: &str, now_unix_secs: u64, backoff_secs: u64) -> bool {
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

    pub fn mark_attempt(&mut self, event_key: &str, now_unix_secs: u64) {
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

    pub fn mark_delivered(&mut self, event_key: &str, now_unix_secs: u64, claim_tx: String) {
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

    pub fn mark_invalid(&mut self, event_key: &str, now_unix_secs: u64, reason: String) {
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

    pub fn record_retryable_error(
        &mut self,
        event_key: &str,
        now_unix_secs: u64,
        max_attempts: u32,
        message: String,
    ) {
        let entry = self
            .state
            .entries
            .entry(event_key.to_string())
            .or_insert_with(|| DeliveryEntry {
                status: DeliveryStatus::Pending,
                attempts: 0,
                last_attempt_unix_secs: now_unix_secs,
                last_error: Some(message.clone()),
                delivered_tx: None,
            });
        entry.status = DeliveryStatus::Pending;
        entry.last_attempt_unix_secs = now_unix_secs;
        entry.last_error = Some(message.clone());
        if max_attempts > 0 && entry.attempts >= max_attempts {
            entry.status = DeliveryStatus::Invalid;
            entry.last_error = Some(message);
        }
    }

    pub fn pending_retry_keys(&self, now_unix_secs: u64, backoff_secs: u64) -> Vec<String> {
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

    pub fn prune_completed(&mut self, now_unix_secs: u64, retention_secs: u64) -> bool {
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

pub fn unix_timestamp_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

pub fn parse_event_key(event_key: &str) -> Result<(String, u64), DynError> {
    let (tx_hash, index_str) = event_key
        .split_once(':')
        .ok_or_else(|| format!("invalid event key format: {event_key}"))?;
    let stripped = index_str
        .strip_prefix("0x")
        .or_else(|| index_str.strip_prefix("0X"))
        .unwrap_or(index_str);
    let log_index = if stripped.chars().all(|ch| ch.is_ascii_hexdigit()) {
        u64::from_str_radix(stripped, 16)?
    } else {
        stripped.parse::<u64>()?
    };
    Ok((tx_hash.to_string(), log_index))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn pending_entry_retries_after_backoff() {
        let mut store = DeliveryStateStore {
            path: Path::new("ignored").to_path_buf(),
            state: DeliveryStateFile::default(),
        };
        store.mark_attempt("0xabc:1", 100);
        assert!(!store.should_retry("0xabc:1", 101, 10));
        assert!(store.should_retry("0xabc:1", 110, 10));
    }

    #[test]
    fn delivered_entry_is_skipped() {
        let mut store = DeliveryStateStore {
            path: Path::new("ignored").to_path_buf(),
            state: DeliveryStateFile::default(),
        };
        store.mark_delivered("0xabc:1", 100, "0xdead".to_string());
        assert!(store.should_skip("0xabc:1"));
    }

    #[test]
    fn retryable_error_respects_max_attempts() {
        let mut store = DeliveryStateStore {
            path: Path::new("ignored").to_path_buf(),
            state: DeliveryStateFile::default(),
        };
        store.mark_attempt("0xabc:1", 100);
        store.record_retryable_error("0xabc:1", 100, 1, "nope".to_string());
        assert!(store.should_skip("0xabc:1"));
    }
}
