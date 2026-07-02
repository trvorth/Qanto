[![Status](https://img.shields.io/badge/Status-Testnet%20Alpha-00e5ff?style=for-the-badge)](./Docs/ROADMAP.md)
[![CI](https://img.shields.io/github/actions/workflow/status/trvorth/Qanto/rust.yml?branch=main&label=CI&style=for-the-badge)](https://github.com/trvorth/Qanto/actions/workflows/rust.yml)
[![License](https://img.shields.io/badge/License-MIT-lightgrey?style=for-the-badge)](https://github.com/trvorth/Qanto/blob/main/LICENSE)
[![Docs](https://img.shields.io/badge/Docs-Testnet%20Guide-blue?style=for-the-badge)](https://github.com/trvorth/Qanto/blob/main/Docs/guide/testnet-guide.md)
[![Website](https://img.shields.io/badge/Website-qanto.org-667eea?style=for-the-badge)](https://qanto.org)

---

<div align="center">

# QANTO — Quantum-Resistant Layer-0 Blockchain

**The world's first post-quantum, AI-governed blockchain infrastructure.**

[Website](https://qanto.org) · [Explorer](https://qanto.org/public/explorer/index.html) · [Whitepaper](./Docs/whitepaper/Qanto-whitepaper.pdf) · [Discord](https://discord.gg/curfp5FKWV) · [X/Twitter](https://x.com/QantoLayer0)

</div>

---

## Overview

Qanto is a production-grade Layer-0 blockchain engineered for extreme throughput, post-quantum security, and AI-driven governance. Its hybrid **PoW + DPoS** consensus model uses Proof-of-Work for permissionless leader election and Delegated Proof-of-Stake for sub-second block finality.

**Key Design Principles:**
- **Post-Quantum Security** — CRYSTALS-Dilithium signatures, Kyber KEM, SPHINCS+ hash-based auth
- **DAG-Based Parallel Execution** — 100,000+ TPS capacity with sub-second finality
- **AI Governance (SAGA)** — Neural network-driven parameter tuning and threat response
- **Deflationary Tokenomics** — Asymptotic halving curve with 21B hard cap

**Author**: trvorth (trvorth@qanto.org)

---

## Tokenomics

### Core Parameters

| Parameter | Value |
|---|---|
| **Token Symbol** | QNTO |
| **Native Decimals** | 9 |
| **Max Supply** | 21,000,000,000 QNTO |
| **Smallest Unit per QNTO** | 1,000,000,000 base units |
| **Initial Block Reward** | 2.5 QNTO |
| **Target Block Time** | 1.0 second |
| **Halving Period** | ~97.2 days (8,400,000 seconds) |
| **Halving Factor** | 0.999 (0.1% reduction per period) |
| **Chain ID** | `0x1234` (4660 decimal) |

### Deflationary Emission Curve

Qanto employs an **asymptotic halving model** where the block reward is multiplied by a `0.999` factor every ~97.2 days. Unlike Bitcoin's aggressive 50% halvings, this creates a smooth, predictable deflation curve:

```
Reward(period) = 2.5 × 0.999^period
```

| Period | Approximate Date | Block Reward | Cumulative % Emitted |
|--------|-----------------|--------------|---------------------|
| 0 | Genesis | 2.5000 QNTO | 0% |
| 100 | ~26.7 years | 2.2618 QNTO | ~4.8% |
| 500 | ~133.3 years | 1.5186 QNTO | ~21.1% |
| 1000 | ~266.6 years | 0.9215 QNTO | ~36.8% |

This model ensures **long-term incentive alignment** — validators and miners receive meaningful rewards for centuries, eliminating the "security budget crisis" that plagues aggressive-halving chains.

### WQNTO (Wrapped QANTO)

The `WQNTO` ERC-20 compatible wrapper enables DEX liquidity provision on EVM chains:

| Property | Value |
|---|---|
| Contract Standard | ERC-20 (Solidity ^0.8.20) |
| Decimals | 9 (matches native QNTO) |
| Functions | `deposit()`, `withdraw()`, `transfer()`, `approve()` |
| Overflow Protection | Native Solidity 0.8.x checked arithmetic |
| Source | [`SmartContracts/contracts/WQNTO.sol`](./SmartContracts/contracts/WQNTO.sol) |

---

## Post-Quantum Security Architecture

### Cryptographic Primitives

| Algorithm | Usage | NIST Status | Key Size |
|---|---|---|---|
| **CRYSTALS-Dilithium** | Transaction signatures | FIPS 204 (Approved) | 2528 bytes (pk) |
| **Kyber KEM** | Secure key exchange | FIPS 203 (Approved) | 1568 bytes (pk) |
| **SPHINCS+** | Hash-based fallback signatures | FIPS 205 (Approved) | 32-64 bytes (pk) |
| **Qanhash32x** | PoW mining algorithm | Custom | N/A |

### Security Layers

```
┌─────────────────────────────────────────────┐
│  Application Layer (Smart Contracts, DApps)  │
├─────────────────────────────────────────────┤
│  Consensus Layer (PoW + DPoS + PoSe + SAGA) │
├─────────────────────────────────────────────┤
│  Network Layer (libp2p, WSS, P2P Mesh)       │
├─────────────────────────────────────────────┤
│  Cryptographic Layer (PQC Suite)             │
│  • Dilithium Signatures                     │
│  • Kyber Key Encapsulation                   │
│  • SPHINCS+ Hash-Based Auth                  │
│  • Qanhash32x PoW                            │
└─────────────────────────────────────────────┘
```

---

## DEX & CEX Integration

### For DEX Integrators

1. Deploy `WQNTO.sol` to your target EVM chain using [`SmartContracts/deploy_wqnto.js`](./SmartContracts/deploy_wqnto.js)
2. Pair WQNTO with your desired liquidity token (USDT, USDC, ETH)
3. Use the standard ERC-20 ABI — available at [`Frontend/website/assets/json/WQNTO_ABI.json`](./Frontend/website/assets/json/WQNTO_ABI.json)

```bash
# Deploy WQNTO to target chain
cd SmartContracts
RPC_URL=https://your-chain-rpc PRIVATE_KEY=0x... node deploy_wqnto.js
```

### For CEX Integrators

The complete CEX listing payload is available at [`SmartContracts/cex-listing-payload.json`](./SmartContracts/cex-listing-payload.json).

**Required API Endpoints (CoinMarketCap / Binance Compatible):**

| Endpoint | Method | Response |
|---|---|---|
| `/api/v1/supply/circulating` | GET | Plain text — circulating supply |
| `/api/v1/supply/total` | GET | Plain text — `21000000000` |
| `/api/v1/supply/max` | GET | Plain text — `21000000000` |
| `/rpc` (eth_blockNumber) | POST | JSON-RPC hex block height |

**Token Metadata:**
```json
{
  "name": "QNTO",
  "symbol": "QNTO",
  "decimals": 9,
  "chainId": "0x1234",
  "maxSupply": 21000000000,
  "initialBlockReward": 2.5,
  "targetBlockTimeSeconds": 1.0
}
```

---

## Performance Benchmarks

Benchmarks conducted on Apple M-series CPU with integrated GPU.

| Benchmark | Time | Throughput | Notes |
|---|---|---|---|
| CPU Hashrate (1k) | ~281 µs | **3.55 MHash/s** | Single-core Qanhash |
| GPU Hashrate (1 batch) | ~3.67 ms | **17.85 MHash/s** | 65,536 parallel hashes |
| Execution Layer (16k txs) | ~79.7 ms | **200,580 TPS** | Full block payload |
| Hyperscale (1.6M txs) | ~62.8 ms | **25,447,000 TPS** | Sharded execution |
| PQ Signatures | ~1.2 ms | **833 sigs/sec** | Dilithium verification |
| Cross-Chain Verify | ~15 ms | **66 proofs/sec** | Light client proofs |

---

## Architecture

### Repository Structure

```
qanto/
├── Protocol_Core/
│   ├── qanto-node/src/       # Core node implementation (Rust)
│   │   ├── node.rs           # Main orchestration
│   │   ├── consensus.rs      # PoW + DPoS + PoSe
│   │   ├── emission.rs       # Tokenomics & halving logic
│   │   ├── qantodag.rs       # DAG ledger & sharding
│   │   ├── p2p.rs            # libp2p networking
│   │   ├── wallet.rs         # Encrypted wallet management
│   │   ├── saga.rs           # AI governance engine
│   │   ├── zk.rs             # Zero-knowledge circuits
│   │   └── post_quantum_crypto.rs  # PQC suite
│   └── qanto-rpc/            # gRPC/JSON-RPC server
├── Frontend/
│   └── website/              # Production website (Cloudflare Pages)
│       ├── index.html
│       ├── assets/css/       # Glassmorphism dark-mode styles
│       └── assets/js/os.js   # SAGA-OS window manager + RPC telemetry
├── SmartContracts/
│   ├── contracts/WQNTO.sol   # Wrapped QANTO (ERC-20, 9 decimals)
│   ├── deploy_wqnto.js       # Deployment script (ethers.js)
│   ├── token-info.json       # Canonical token metadata
│   └── cex-listing-payload.json  # CEX integration payload
└── docs/
    ├── whitepaper/           # Academic whitepaper
    └── ROADMAP.md            # Development roadmap
```

### Consensus Model

Qanto's hybrid consensus combines three mechanisms:

1. **Proof-of-Work (PoW)** — Qanhash algorithm for permissionless leader election with ASIC resistance
2. **Delegated Proof-of-Stake (DPoS)** — Validator selection for fast finality (sub-second)
3. **Proof-of-Storage/Execution (PoSe)** — Resource utilization verification

SAGA (AI Governance) continuously optimizes parameters, detects anomalies, and adjusts difficulty in real-time.

---

## QANTO Desktop Wallet

### Architecture

The desktop wallet under `qanto-desktop-wallet/` is a native desktop client built on:

- **Tauri + React + TypeScript** for a lightweight cross-platform shell and responsive UI
- **Rust IPC commands** for wallet creation, unlock, signing, balance queries, and on-chain transaction history
- **Argon2id + AES-256-GCM keystore** for local encrypted private-key storage in the OS standard app-data directory
- **Ed25519 + Dilithium3 dual signatures** for hybrid classical + post-quantum transaction authorization
- **gRPC integration with `qanto-rpc`** for live balance reads, transaction submission, and address-scoped history queries

### Local Build And Run

```bash
cd qanto-desktop-wallet

# Install frontend dependencies
npm install

# Start the desktop wallet in development mode
npm run tauri dev

# Production frontend build check
npm run build
```

The wallet connects to the local QANTO RPC node by default at `127.0.0.1:50051`. Override with:

```bash
QANTO_RPC_ADDR="127.0.0.1:50051" npm run tauri dev
```

---

## RPC API Updates

### GetTransactionsByAddress

The `QantoRpc` gRPC service now exposes a native address-scoped transaction history endpoint:

```proto
rpc GetTransactionsByAddress(GetTransactionsByAddressRequest)
  returns (GetTransactionsByAddressResponse);
```

#### Request

```proto
message GetTransactionsByAddressRequest {
  string address = 1;
  uint32 page = 2;       // 1-based
  uint32 page_size = 3;  // server-clamped
}
```

#### Response

```proto
message AddressTransactionEntry {
  Transaction transaction = 1;
  string block_id = 2;
  uint64 block_height = 3;
  uint64 block_timestamp = 4;
  uint64 confirmations = 5;
  bool is_finalized = 6;
}

message GetTransactionsByAddressResponse {
  repeated AddressTransactionEntry transactions = 1;
  uint64 total = 2;
  uint32 current_page = 3;
  uint32 page_size = 4;
}
```

#### Semantics

- `transaction.amount`, `transaction.fee`, `transaction.gas_price`, `transaction.priority_fee`, and output amounts remain **string-encoded base units** to preserve full `u128` precision
- `block_timestamp` is the canonical on-chain block time for the transaction inclusion block
- `confirmations` is computed from `latest_height - block_height + 1`
- `is_finalized` is derived from the DAG finality path and indicates stronger settlement assurance than a simple inclusion confirmation

This endpoint is backed by the node’s existing `address_index` and `tx_index`, avoiding local wallet-side caches and enabling direct decentralized history reads from the QANTO node.

---

## Quick Start

### Prerequisites

- **Rust** 1.75+ with Cargo
- **System**: Linux / macOS / Windows
- **RAM**: 8–16 GB recommended
- **Storage**: 50 GB+ SSD

### Build & Run

```bash
# Clone
git clone https://github.com/trvorth/Qanto.git
cd Qanto

# Build
cargo build --release

# Run node
cargo run --release --features infinite-strata --bin qanto -- \
  start --config config.toml --wallet wallet.key --clean

# Run tests
cargo test --release
```

### Docker

```bash
docker pull qanto/node:latest
docker run -d --name qanto-node \
  -p 8081:8081 -p 30303:30303 -p 50051:50051 \
  -v qanto-data:/data qanto/node:latest
```

### Connect via RPC

```bash
# Check block height
curl -s -X POST https://trvorth-qanto-testnet.hf.space/rpc \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}'
```

---

## Configuration

```toml
p2p_address = "/ip4/0.0.0.0/tcp/30303/ws"
api_address = "0.0.0.0:8081"
network_id = "qanto-testnet"
chain_id = 1234
target_block_time = 1000
difficulty = 1
max_amount = 21000000000
use_gpu = false
zk_enabled = true
mining_threads = 4
mining_enabled = true
num_chains = 1
mining_chain_id = 0

[rpc]
address = "127.0.0.1:50051"

[logging]
level = "info"
```

---

## Testnet

| Parameter | Value |
|---|---|
| Network Name | QANTO Testnet |
| Chain ID | `0x1234` (4660) |
| RPC | `https://trvorth-qanto-testnet.hf.space/rpc` |
| WebSocket | `wss://trvorth-qanto-testnet.hf.space/ws` |
| Explorer | [qanto.org/public/explorer](https://qanto.org/public/explorer/index.html) |

### Add to MetaMask

```json
{
  "chainId": "0x1234",
  "chainName": "QANTO Testnet",
  "nativeCurrency": { "name": "QNTO", "symbol": "QNTO", "decimals": 9 },
  "rpcUrls": ["https://trvorth-qanto-testnet.hf.space/rpc"],
  "blockExplorerUrls": ["https://qanto.org/public/explorer/index.html"]
}
```

---

## Security

Report vulnerabilities responsibly:

- **Email**: security@qanto.org
- **Response Time**: 24–48 hours
- **Bug Bounty**: Up to $1,000 for critical vulnerabilities

---

## Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/my-feature`)
3. Run `cargo test --all && cargo fmt && cargo clippy`
4. Submit a Pull Request

See [CONTRIBUTING.md](CONTRIBUTING.md) for detailed guidelines.

---

## License

MIT License — see [LICENSE](LICENSE) for details.

---

<div align="center">

**Qanto** — Quantum-Resistant Layer-0 Blockchain

[Website](https://qanto.org) · [Explorer](https://qanto.org/public/explorer/index.html) · [X/Twitter](https://x.com/QantoLayer0) · [Discord](https://discord.gg/curfp5FKWV)

</div>
