//! Error types and outcome handling for the relayer.

use thiserror::Error;

/// Result type alias for relayer operations.
pub type RelayerResult<T> = Result<T, RelayerError>;

/// Unified error type for the relayer.
#[derive(Error, Debug)]
pub enum RelayerError {
    #[error("configuration error: {0}")]
    Config(#[from] ConfigError),

    #[error("source chain RPC error: {0}")]
    SourceChain(String),

    #[error("source chain log decode error: {0}")]
    LogDecode(String),

    #[error("claim construction error: {0}")]
    ClaimConstruction(String),

    #[error("claim dispatch error: {0}")]
    ClaimDispatch(String),

    #[error("state persistence error: {0}")]
    StatePersistence(#[from] StateError),

    #[error("signing error: {0}")]
    Signing(String),

    #[error("timeout: {0}")]
    Timeout(String),

    #[error("internal error: {0}")]
    Internal(String),
}

/// Configuration-related errors.
#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("missing required environment variable: {0}")]
    MissingEnv(String),

    #[error("invalid value for {var}: {reason}")]
    InvalidValue { var: String, reason: String },

    #[error("invalid address format: {0}")]
    InvalidAddress(String),

    #[error("invalid private key: {0}")]
    InvalidPrivateKey(String),

    #[error("parse error: {0}")]
    Parse(String),
}

/// State persistence errors.
#[derive(Error, Debug)]
pub enum StateError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

/// Outcome of a single delivery attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeliveryOutcome {
    /// Successfully delivered to Qanto.
    Delivered { claim_tx_hash: String },

    /// Permanently invalid (bad log, rejected by Qanto, max retries exceeded).
    Invalid { reason: String },

    /// Transient failure, eligible for retry.
    Retryable { reason: String },

    /// Qanto reported duplicate - treat as delivered.
    Duplicate { claim_tx_hash: String },

    /// Max retries exhausted - mark invalid.
    Exhausted { error: String },
}

impl DeliveryOutcome {
    /// Check if this outcome represents a final state (no more retries).
    pub fn is_final(&self) -> bool {
        matches!(
            self,
            DeliveryOutcome::Delivered { .. }
                | DeliveryOutcome::Invalid { .. }
                | DeliveryOutcome::Exhausted { .. }
        )
    }

    /// Check if this is a successful delivery.
    pub fn is_delivered(&self) -> bool {
        matches!(
            self,
            DeliveryOutcome::Delivered { .. } | DeliveryOutcome::Duplicate { .. }
        )
    }

    /// Get the claim transaction hash if delivered.
    pub fn claim_tx_hash(&self) -> Option<&str> {
        match self {
            DeliveryOutcome::Delivered { claim_tx_hash } => Some(claim_tx_hash),
            DeliveryOutcome::Duplicate { claim_tx_hash } => Some(claim_tx_hash),
            _ => None,
        }
    }
}

/// Classify an error message from Qanto RPC to determine if it's a duplicate/rejected claim.
pub fn classify_qanto_error(message: &str) -> DeliveryOutcome {
    let lowered = message.to_ascii_lowercase();

    // Duplicate / already claimed -> treat as delivered
    if lowered.contains("already")
        || lowered.contains("duplicate")
        || lowered.contains("exists")
        || lowered.contains("already claimed")
        || lowered.contains("already minted")
    {
        return DeliveryOutcome::Delivered {
            claim_tx_hash: "duplicate".to_string(),
        };
    }

    // Explicit rejection -> invalid
    if lowered.contains("rejected")
        || lowered.contains("invalid")
        || lowered.contains("failed verification")
    {
        return DeliveryOutcome::Invalid {
            reason: message.to_string(),
        };
    }

    // Everything else -> retryable
    DeliveryOutcome::Retryable {
        reason: message.to_string(),
    }
}

/// Convert any RelayerError into a DeliveryOutcome for unified handling.
#[allow(dead_code)]
pub fn error_to_outcome(error: &RelayerError) -> DeliveryOutcome {
    match error {
        RelayerError::LogDecode(msg)
        | RelayerError::ClaimConstruction(msg)
        | RelayerError::Signing(msg) => DeliveryOutcome::Invalid {
            reason: msg.clone(),
        },
        RelayerError::ClaimDispatch(msg) => classify_qanto_error(msg),
        RelayerError::SourceChain(msg) => DeliveryOutcome::Retryable {
            reason: msg.clone(),
        },
        RelayerError::StatePersistence(_) => DeliveryOutcome::Retryable {
            reason: error.to_string(),
        },
        RelayerError::Config(_) => DeliveryOutcome::Invalid {
            reason: error.to_string(),
        },
        RelayerError::Timeout(msg) => DeliveryOutcome::Retryable {
            reason: msg.clone(),
        },
        RelayerError::Internal(msg) => DeliveryOutcome::Retryable {
            reason: msg.clone(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_duplicate() {
        let outcome = classify_qanto_error("claim already exists");
        assert!(outcome.is_delivered());
    }

    #[test]
    fn test_classify_rejected() {
        let outcome = classify_qanto_error("claim rejected: invalid proof");
        assert!(matches!(outcome, DeliveryOutcome::Invalid { .. }));
    }

    #[test]
    fn test_classify_retryable() {
        let outcome = classify_qanto_error("network timeout");
        assert!(matches!(outcome, DeliveryOutcome::Retryable { .. }));
    }
}
