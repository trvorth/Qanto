# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## graphify

This project has a knowledge graph at graphify-out/ with god nodes, community structure, and cross-file relationships.

Rules:
- For codebase questions, first run `graphify query "<question>"` when graphify-out/graph.json exists. Use `graphify path "<A>" "<B>"` for relationships and `graphify explain "<concept>"` for focused concepts. These return a scoped subgraph, usually much smaller than GRAPH_REPORT.md or raw grep output.
- If graphify-out/wiki/index.md exists, use it for broad navigation instead of raw source browsing.
- Read graphify-out/GRAPH_REPORT.md only for broad architecture review or when query/path/explain do not surface enough context.
- After modifying code, run `graphify update .` to keep the graph current (AST-only, no API cost).

## Development Commands

### Building the Project
```bash
# Check the workspace (fast, no emissions)
cargo check --workspace --locked

# Build in release mode with infinite-strata feature
cargo build --release --features infinite-strata

# Build specific binaries
cargo build --release --bin qanto        # Main node binary
cargo build --release --bin qantowallet  # Wallet generation tool
cargo build --release --bin omega_test   # Conscious security layer simulation
```

### Running Tests
```bash
# Run all workspace tests with nextest (recommended)
cargo nextest run --workspace --locked

# Run tests for specific crates
cargo nextest run --package qanto-node --locked
cargo nextest run --package qanto-rpc --locked

# Run tests with standard cargo test (slower but shows all output)
cargo test --workspace --locked

# Run a single test function
cargo nextest run --workspace --locked -- test_function_name
```

### Smart Contracts Development
```bash
# Install and test Solidity contracts
cd SmartContracts
npm ci
npm test

# Deploy WQNTO to authorized development network
cd SmartContracts
RPC_URL=https://your-authorized-rpc PRIVATE_KEY=0x... EXPECTED_CHAIN_ID=1234 node deploy_wqnto.js
```

### Desktop Wallet Development
```bash
cd qanto-desktop-wallet

# Install frontend dependencies
npm install

# Start in development mode
npm run tauri dev

# Production frontend build
npm run build
```

### Code Quality
```bash
# Rust formatting
cargo fmt --all -- --check

# Rust linting
cargo clippy --workspace --all-targets --locked -- -D warnings

# JavaScript/TypeScript formatting (if applicable)
# Uses Prettier configured in respective package.json files
```

## Code Architecture

### Core Components
The Qanto Protocol consists of several key components organized in the Protocol_Core directory:

1. **qanto-node** - Main node implementation handling networking, consensus, and block processing
2. **qanto-core** - Core data structures, types, and fundamental protocol logic
3. **qanto-consensus-primitives** - Consensus mechanisms including Proof-of-Work implementations
4. **qanto-rpc** - gRPC API service exposing node functionality to external clients
5. **qanto-relayer** - Transaction relaying and mempool management components
6. **qanto-zk-sdk** - Zero-knowledge proof tools and utilities for privacy features
7. **myblockchain** - Experimental blockchain implementations and research prototypes

### Key Research Areas
The codebase implements/explores three consensus mechanisms (only PoW is currently enforced):
- **Proof-of-Work (PoW)** - Custom hash implementation with heterogeneous backends
- **Delegated Proof-of-Stake (DPoS)** - Helper code exists but not actively enforced
- **Proof-of-Storage/Execution (PoSe)** - No canonical verifier implemented

Additional experimental modules:
- **SAGA** - Adaptive control system with heuristic and neural components (not consensus-safe)
- **ΛΣ-ΩMEGA** - Conscious security layer simulation (accessible via `omega_test` binary)
- **Multi-parent DAG** - Branch-scoped state storage research (incomplete finality)

### Frontend Applications
- **qanto-dapp** - Decentralized application interface
- **qantoscan** - Blockchain explorer and analytics
- **website** - Official project documentation and resources
- **qanto-desktop-wallet** - Native desktop client built with Tauri

### Configuration and Deployment
- Configuration files stored in `config/` directory
- Docker/Kubernetes/Helm deployment configurations in `DevOps/`
- Network-specific configurations managed through environment variables and `.env` files
- Local testnet deployment guided by `Docs/guide/testnet-guide.md`

## RTK (Rust Token Killer) - Token-Optimized Commands

## Golden Rule

**Always prefix commands with `rtk`**. If RTK has a dedicated filter, it uses it. If not, it passes through unchanged. This means RTK is always safe to use.

**Important**: Even in command chains with `&&`, use `rtk`:
```bash
# �� ❌ Wrong
git add . && git commit -m "msg" && git push

# � ✅ Correct
rtk git add . && rtk git commit -m "msg" && rtk git push
```

## RTK Commands by Workflow

### Build & Compile (80-90% savings)
```bash
rtk cargo build         # Cargo build output
rtk cargo check         # Cargo check output
rtk cargo clippy        # Clippy warnings grouped by file (80%)
rtk tsc                 # TypeScript errors grouped by file/code (83%)
rtk lint                # ESLint/Biome violations grouped (84%)
rtk prettier --check    # Files needing format only (70%)
rtk next build          # Next.js build with route metrics (87%)
```

### Test (60-99% savings)
```bash
rtk cargo test          # Cargo test failures only (90%)
rtk go test             # Go test failures only (90%)
rtk jest                # Jest failures only (99.5%)
rtk vitest              # Vitest failures only (99.5%)
rtk playwright test     # Playwright failures only (94%)
rtk pytest              # Python test failures only (90%)
rtk rake test           # Ruby test failures only (90%)
rtk rspec               # RSpec test failures only (60%)
rtk test <cmd>          # Generic test wrapper - failures only
```

### Git (59-80% savings)
```bash
rtk git status          # Compact status
rtk git log             # Compact log (works with all git flags)
rtk git diff            # Compact diff (80%)
rtk git show            # Compact show (80%)
rtk git add             # Ultra-compact confirmations (59%)
rtk git commit          # Ultra-compact confirmations (59%)
rtk git push            # Ultra-compact confirmations
rtk git pull            # Ultra-compact confirmations
rtk git branch          # Compact branch list
rtk git fetch           # Compact fetch
rtk git stash           # Compact stash
rtk git worktree        # Compact worktree
```

Note: Git passthrough works for ALL subcommands, even those not explicitly listed.

### GitHub (26-87% savings)
```bash
rtk gh pr view <num>    # Compact PR view (87%)
rtk gh pr checks        # Compact PR checks (79%)
rtk gh run list         # Compact workflow runs (82%)
rtk gh issue list       # Compact issue list (80%)
rtk gh api              # Compact API responses (26%)
```

### JavaScript/TypeScript Tooling (70-90% savings)
```bash
rtk pnpm list           # Compact dependency tree (70%)
rtk pnpm outdated       # Compact outdated packages (80%)
rtk pnpm install        # Compact install output (90%)
rtk npm run <script>    # Compact npm script output
rtk npx <cmd>           # Compact npx command output
rtk prisma              # Prisma without ASCII art (88%)
rtk uv run <cmd>        # Compact uv project command output
```

### Files & Search (60-75% savings)
```bash
rtk ls <path>           # Tree format, compact (65%)
rtk read <file>         # Code reading with filtering (60%)
rtk grep <pattern>      # Search grouped by file (75%). Format flags (-c, -l, -L, -o, -Z) run raw.
rtk find <pattern>      # Find grouped by directory (70%)
```

### Analysis & Debug (70-90% savings)
```bash
rtk err <cmd>           # Filter errors only from any command
rtk log <file>          # Deduplicated logs with counts
rtk json <file>         # JSON structure without values
rtk deps                # Dependency overview
rtk env                 # Environment variables compact
rtk summary <cmd>       # Smart summary of command output
rtk diff                # Ultra-compact diffs
```

### Infrastructure (85% savings)
```bash
rtk docker ps           # Compact container list
rtk docker images       # Compact image list
rtk docker logs <c>     # Deduplicated logs
rtk kubectl get         # Compact resource list
rtk kubectl logs        # Deduplicated pod logs
```

### Network (65-70% savings)
```bash
rtk curl <url>          # Compact HTTP responses (70%)
rtk wget <url>          # Compact download output (65%)
```

### Meta Commands
```bash
rtk gain                # View token savings statistics
rtk gain --history      # View command history with savings
rtk discover            # Analyze Claude Code sessions for missed RTK usage
rtk proxy <cmd>         # Run command without filtering (for debugging)
rtk init                # Add RTK instructions to CLAUDE.md
rtk init --global       # Add RTK to ~/.claude/CLAUDE.md
```

## Token Savings Overview

| Category | Commands | Typical Savings |
|----------|----------|-----------------|
| Tests | vitest, playwright, cargo test | 90-99% |
| Build | next, tsc, lint, prettier | 70-87% |
| Git | status, log, diff, add, commit | 59-80% |
| GitHub | gh pr, gh run, gh issue | 26-87% |
| Package Managers | pnpm, npm, npx | 70-90% |
| Files | ls, read, grep, find | 60-75% |
| Infrastructure | docker, kubectl | 85% |
| Network | curl, wget | 65-70% |

Overall average: **60-90% token reduction** on common development operations.

## Branching Strategy

We follow a structured branching model for institutional stability:

-   **`main`**: The integration branch. Its presence does not imply production or mainnet readiness.
-   **`release/*`**: Pre-production staging branches for final audit and hardening.
-   **`feature/*`**: Development branches for new features or protocol enhancements.
-   **`fix/*`**: Critical bug fixes or security patches.

## Code Style

-   **Rust**: Run `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets --locked -- -D warnings`.
-   **JavaScript/CSS**: Use Prettier for consistent formatting across the frontend.

## Adding New ZK-Circuits

Qanto uses a modular ZK-proof system based on **arkworks**. To add a new circuit:

1.  **Define the Circuit Structure**: Create a new struct in `Protocol_Core/qanto-node/src/zkp.rs` implementing `ConstraintSynthesizer<ConstraintF>`.
2.  **Define Constraints**: Implement the `generate_constraints` method using R1CS gadgets.
3.  **Integrate with ZKManager**: 
    - Add the new variant to `ZKProofType`.
    - Update `ZKProofSystem::generate_proof` to handle the new type.
4.  **Add Tests and Setup Review**: Every circuit must include positive and negative proof tests, deterministic public-input encoding, versioned parameters, and a documented trusted-setup or transparent-setup process. Mock or per-process verifier keys are not acceptable for a production bridge.

