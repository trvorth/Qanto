//! # Qanto Core
//!
//! Core native implementations for the Qanto blockchain ecosystem.
//! This crate provides the foundational technologies for cryptography,
//! networking, and serialization that power the entire Qanto network.
//!
//! ## Features
//!
//! - **Native Cryptography**: Ed25519, Post-Quantum, and P256 implementations
//! - **Compatibility Layer**: Drop-in replacements for external crypto libraries
//! - **Native Serialization**: High-performance, zero-copy serialization system
//! - **Native P2P**: Custom networking stack for blockchain communication
//! - **Mining Celebration**: Specialized mining and consensus utilities
//!
//! ## Modules
//!
//! - [`qanto_native_crypto`]: Core cryptographic primitives and implementations
//! - [`qanto_compat`]: Compatibility interfaces for external library replacement
//! - [`qanto_serde`]: Native serialization and deserialization system
//! - [`qanto_p2p`]: Native peer-to-peer networking implementation
//! - [`mining_celebration`]: Mining and consensus celebration utilities

#![allow(missing_docs)]
#![warn(clippy::all)]
#![allow(unsafe_code)]

// Re-export core modules for external use
pub mod qanto_compat;
pub mod qanto_native_crypto;
pub mod qanto_p2p;
pub mod qanto_net;
pub mod qanto_serde;
pub mod qanto_storage;
pub mod storage_adapter;
pub mod storage_traits;
pub mod mining_celebration;

// Re-export specific types to avoid ambiguous glob re-exports
pub use qanto_compat::{QantoNativeCrypto};
pub use qanto_native_crypto::QantoNativeCryptoError as QantoCompatError;
pub use qanto_native_crypto::{QantoPQPublicKey, QantoPQPrivateKey, QantoPQSignature, QantoNativeCrypto as NativeCrypto};
pub use qanto_p2p::{QantoP2P, NetworkConfig};
pub use qanto_net::{QantoNetServer, QantoNetError, NetworkMessage, PeerId};