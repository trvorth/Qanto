# Key Management

**Qanto Layer-0 Blockchain**  
**Branch:** main

## Key Types

| Key Type | Purpose | Storage | Required For |
|----------|---------|---------|-------------|
| Node P2P Identity | libp2p Noise handshake | `data/node-identity.key` | Node startup |
| Miner Signer | Block signing | Protected file / OS keyring | Mining |
| User Wallet | Funds control | Encrypted wallet file | Wallet operations |
| Post-Quantum Keypair | ML-KEM operations | Generated per-session | PQC bridge |

## Separation Principle

1. **Node identity key ≠ Wallet key** – P2P identity never controls funds
2. **Miner signer ≠ Wallet key** – Miner signer loaded only when mining enabled
3. **No automatic wallet creation** – Wallet commands are explicit

## Security Requirements

| Requirement | Status |
|-------------|--------|
| No default password | ✓ |
| No hardcoded keys | ✓ |
| No secrets in logs | ✓ |
| Restrictive file permissions (0600) | ✓ for node identity, partial for wallet |
| Atomic writes with sync | ✓ |
| Legacy WALLET_PASSWORD support (mining only) | ✓ with deprecation warning |
| No TTY required for service startup | ✓ |
| No interactive prompts in Docker | ✓ |