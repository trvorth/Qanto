[![Status](https://img.shields.io/badge/Status-Research%20Prototype-orange?style=for-the-badge)](./Docs/ROADMAP.md)
[![CI](https://img.shields.io/github/actions/workflow/status/trvorth/Qanto/rust.yml?branch=main&label=CI&style=for-the-badge)](https://github.com/trvorth/Qanto/actions/workflows/rust.yml)
[![License](https://img.shields.io/badge/License-MIT-lightgrey?style=for-the-badge)](https://github.com/trvorth/Qanto/blob/main/LICENSE)
[![Docs](https://img.shields.io/badge/Docs-Testnet%20Guide-blue?style=for-the-badge)](https://github.com/trvorth/Qanto/blob/main/Docs/guide/testnet-guide.md)
[![Website](https://img.shields.io/badge/Website-qanto.org-667eea?style=for-the-badge)](https://qanto.org)

---

<div align="center">

# QANTO — Layer-0 Protocol Research Prototype

**Pre-release research code with ML-DSA signing, a multi-parent DAG, and experimental adaptive-control modules. Not ready for a public network or assets of value.**

[Website](https://qanto.org) · [Explorer](https://qanto.org/public/explorer/index.html) · [Whitepaper](./Docs/whitepaper/Qanto-whitepaper.pdf) · [Discord](https://discord.gg/curfp5FKWV) · [X/Twitter](https://x.com/QantoLayer0)

</div>

---

## Overview

Qanto is an experimental Layer-0 codebase. The current canonical block path checks fixed-difficulty Proof-of-Work, but it does not enforce a DPoS committee certificate or Proof-of-Storage/Execution, and automatic finality is disabled. SAGA/ΩMEGA remain experimental and must not control consensus until their inputs and arithmetic are deterministic and committed.

**Key Design Principles:**
- **Post-Quantum direction** — ML-DSA signing is implemented, but sender/validator identity binding, PQ transport, migration, and independent side-channel review remain launch gates.
- **DAG research** — Multi-parent storage and deterministic tie-breaking exist; branch-scoped state, reorganization, and finality are incomplete.
- **Adaptive-control research** — SAGA contains heuristic and neural components; active consensus use is not safe.
- **Sealed allocation constants** — The 21B constants and 9-decimal scale are compile-time checked; live issuance/cap enforcement is not yet correct.

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
| **Initial Block Reward** | Unresolved: code bases use 50 QNTO, but the live SAGA path is not consensus-safe |
| **Target Block Time** | 1.0 second |
| **Halving Period** | ~97.2 days (8,400,000 seconds) |
| **Halving Factor** | 0.999 (0.1% reduction per period) |
| **Chain ID** | `1234` (`0x4D2`) |

### Emission Status

`emission.rs` contains an integer recurrence intended to calculate `50 × 0.999^period`, subject to a remaining-supply clamp. The canonical block producer and validator do not currently use that engine; they use process-local floating SAGA state instead. The shipped genesis also allocates the full 21B while the emission counter starts at zero. Therefore no live emission schedule, security budget, or cap guarantee should be inferred from this README. See the current tokenomics audit under `Docs/audits/2026-08-08-master-a-z/`.

### WQNTO (Wrapped QANTO)

The `WQNTO` ERC-20 contract is a same-chain wrapped-native-token primitive. It is not by itself a cross-chain bridge, and this repository publishes no canonical production deployment or live liquidity pool:

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
| **ML-DSA** | Local signing/verification primitive; identity binding incomplete | FIPS 204 | Parameter-dependent |
| **ML-KEM/Kyber family** | Key material exists; no production session establishment was found | FIPS 203 family | Parameter-dependent |
| **SLH-DSA/SPHINCS+ family** | Optional/fallback code; not canonical authorization | FIPS 205 family | Parameter-dependent |
| **Qanhash32x** | PoW mining algorithm | Custom | N/A |

### Security Layers

```
┌─────────────────────────────────────────────┐
│  Application Layer (Smart Contracts, DApps)  │
├─────────────────────────────────────────────┤
│  Consensus research (active PoW; other gates incomplete) │
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

**Production integration is blocked.** QantoSwap intentionally reverts, bridge claims fail closed, and no production WQNTO address is published. For an explicitly authorized development network, compile and test the current contract before using the artifact-based deployment script.

```bash
# Deploy WQNTO to target chain
cd SmartContracts
RPC_URL=https://your-authorized-rpc PRIVATE_KEY=0x... EXPECTED_CHAIN_ID=1234 node deploy_wqnto.js
```

### For CEX Integrators

`SmartContracts/cex-listing-payload.json` is a draft artifact and is **not approved for exchange integration**. Supply, reward, endpoint, and finality semantics must be corrected and independently verified first.

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
  "chainId": "0x4D2",
  "maxSupply": 21000000000,
  "initialBlockRewardStatus": "unresolved-do-not-integrate",
  "targetBlockTimeSeconds": 1.0
}
```

---

## Performance Status

No reproducible current artifact establishes sustained accepted-and-durable TPS, state-changing TPS, Layer-0 routing throughput, WAN throughput, GPU/CPU QanHash parity, or bridge-proof throughput. The configured transaction and worker counts are not measurements. The available benchmark binary currently exits before useful measurement, and the claimed 10M+/25.447M TPS figures must not be repeated until a multi-node benchmark publishes raw inputs, accepted/finalized counts, bytes, latency distributions, hardware, and failure rates.

---

## Architecture

### Repository Structure

```
qanto/
├── Protocol_Core/
│   ├── qanto-node/src/       # Core node implementation (Rust)
│   │   ├── node.rs           # Main orchestration
│   │   ├── consensus.rs      # Experimental consensus helpers
│   │   ├── emission.rs       # Tokenomics & halving logic
│   │   ├── qantodag.rs       # DAG ledger & sharding
│   │   ├── p2p.rs            # libp2p networking
│   │   ├── wallet.rs         # Encrypted wallet management
│   │   ├── saga.rs           # AI governance engine
│   │   ├── zkp.rs            # Experimental zero-knowledge circuits
│   │   └── post_quantum_crypto.rs  # PQC suite
│   └── qanto-rpc/            # gRPC/JSON-RPC server
├── Frontend/
│   └── website/              # Web frontend
│       ├── index.html
│       ├── assets/css/       # Glassmorphism dark-mode styles
│       └── assets/js/os.js   # SAGA-OS window manager + RPC telemetry
├── SmartContracts/
│   ├── contracts/WQNTO.sol   # Wrapped QANTO (ERC-20, 9 decimals)
│   ├── deploy_wqnto.js       # Deployment script (ethers.js)
│   ├── token-info.json       # Canonical token metadata
│   └── cex-listing-payload.json  # Draft integration metadata
└── Docs/
    ├── whitepaper/           # Academic whitepaper
    └── ROADMAP.md            # Development roadmap
```

### Consensus Model

The repository contains code or specifications for three mechanisms, but only fixed-difficulty PoW is enforced by the canonical inbound block path:

1. **Proof-of-Work (PoW)** — checked, but the custom hash and heterogeneous backends require a versioned repair and parity vectors.
2. **Delegated Proof-of-Stake (DPoS)** — helper code exists; inbound blocks do not prove committee eligibility or quorum.
3. **Proof-of-Storage/Execution (PoSe)** — no canonical verifier was found.

SAGA telemetry and edict code exists, but active values are process-local and unsuitable for consensus. Difficulty is currently fixed.

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
- `is_finalized` reflects the node's stored finalized set. Automatic protocol finality is currently disabled, so clients must not interpret this field as a live quorum certificate or irreversible settlement guarantee.

This endpoint is backed by the node’s existing `address_index` and `tx_index`, avoiding local wallet-side caches and enabling direct decentralized history reads from the QANTO node.

---

## Quick Start

### Prerequisites

- **Rust** via rustup; `rust-toolchain.toml` pins the required nightly
- **System**: Linux / macOS / Windows
- **RAM**: 8–16 GB recommended
- **Storage**: 50 GB+ SSD

### Build & Run

```bash
# Clone
git clone https://github.com/trvorth/Qanto.git
cd Qanto

# Check the locked workspace
cargo check --workspace --locked

# Run the canonical Rust suite
cargo nextest run --workspace --locked
```

The checked-in Docker/Kubernetes/Helm paths are under audit and are not a reproducible public-testnet launch contract. Do not pull or deploy an unpinned `latest` image.

### Connect via RPC

```bash
# Legacy endpoint example only; availability and safety are not asserted
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

The endpoints below are legacy metadata, not evidence of an active or launch-ready public testnet. The audited zero-budget Hugging Face topology is not suitable for a public P2P blockchain.

| Parameter | Value |
|---|---|
| Network Name | QANTO Testnet |
| Chain ID | `1234` (`0x4D2`) |
| RPC | `https://trvorth-qanto-testnet.hf.space/rpc` |
| WebSocket | `wss://trvorth-qanto-testnet.hf.space/ws` |
| Explorer | [qanto.org/public/explorer](https://qanto.org/public/explorer/index.html) |

### Add to MetaMask

```json
{
  "chainId": "0x4D2",
  "chainName": "QANTO Testnet",
  "nativeCurrency": { "name": "QNTO", "symbol": "QNTO", "decimals": 9 },
  "rpcUrls": ["https://trvorth-qanto-testnet.hf.space/rpc"],
  "blockExplorerUrls": ["https://qanto.org/public/explorer/index.html"]
}
```

---

## Security

Report vulnerabilities responsibly:

- Use the private process in [SECURITY.md](SECURITY.md).
- Do not open a public issue containing vulnerability details.
- No formal bug bounty or guaranteed reward is currently active.

---

## Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/my-feature`)
3. Run the applicable locked checks documented in [CONTRIBUTING.md](CONTRIBUTING.md)
4. Submit a Pull Request

See [CONTRIBUTING.md](CONTRIBUTING.md) for detailed guidelines.

---

## License

MIT License — see [LICENSE](LICENSE) for details.

---

<div align="center">

**Qanto** — Layer-0 Protocol Research Prototype

[Website](https://qanto.org) · [Explorer](https://qanto.org/public/explorer/index.html) · [X/Twitter](https://x.com/QantoLayer0) · [Discord](https://discord.gg/curfp5FKWV)

</div>
