# Contributing to Qanto

Thank you for your interest in contributing to the Qanto Layer-0 protocol. We welcome developers to help us build a quantum-secure future for decentralized finance.

## Getting Started

### Prerequisites

To build and run Qanto, you will need:

1.  **Rust**: Install [rustup](https://rustup.rs/). The repository's `rust-toolchain.toml` selects the exact pinned nightly toolchain and required components.
    ```bash
    rustup show
    ```
2.  **Node.js and npm**: Use a version supported by the lockfiles and current Hardhat toolchain.
3.  **cargo-nextest**: Recommended for the canonical Rust test profile under `.config/nextest.toml`.

### Environment Setup

1.  Clone the repository:
    ```bash
    git clone https://github.com/trvorth/Qanto.git
    cd Qanto
    ```
2.  Check the Rust workspace:
    ```bash
    cargo check --workspace --locked
    ```
3.  Run the Rust tests:
    ```bash
    cargo nextest run --workspace --locked
    ```
4.  Install and test the Solidity workspace:
    ```bash
    cd SmartContracts
    npm ci
    npm test
    ```
5.  Configure local environment variables from `.env.example`. Never commit `.env`, private keys, seed phrases, or production credentials.

## Development Workflow

### Branching Strategy

We follow a structured branching model for institutional stability:

-   **`main`**: The integration branch. Its presence does not imply production or mainnet readiness.
-   **`release/*`**: Pre-production staging branches for final audit and hardening.
-   **`feature/*`**: Development branches for new features or protocol enhancements.
-   **`fix/*`**: Critical bug fixes or security patches.

### Code Style

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

## Submitting Changes

1.  Create a feature branch.
2.  Write tests for your changes.
3.  Submit a Pull Request with a clear description of the problem and the proposed solution.
4.  Ensure all applicable local and CI checks pass, and document any unavailable platform-specific checks.

## Security

If you discover a security vulnerability, please do NOT open a public issue. Instead, refer to our [Security Policy](SECURITY.md) for reporting instructions.
