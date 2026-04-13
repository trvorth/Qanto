# Contributing to Qanto

Thank you for your interest in contributing to the Qanto Layer-0 protocol. We welcome developers to help us build a quantum-secure future for decentralized finance.

## Getting Started

### Prerequisites

To build and run Qanto, you will need:

1.  **Rust**: Install the latest stable version using [rustup](https://rustup.rs/). Nightly is recommended for some advanced ZK optimizations.
    ```bash
    rustup default nightly
    ```
2.  **Node.js**: Version 18.x or higher for the frontend build pipeline.
3.  **Vite**: The enterprise website and explorer use Vite for lightning-fast development and optimized production builds.

### Environment Setup

1.  Clone the repository:
    ```bash
    git clone https://github.com/trvorth/Qanto.git
    cd Qanto
    ```
2.  Install frontend dependencies:
    ```bash
    cd website && npm install
    ```
3.  Configure environment variables:
    Create a `.env` file in the root directory and add the necessary configuration (see `.env.example`).

## Development Workflow

### Branching Strategy

We follow a structured branching model for institutional stability:

-   **`main`**: The production-ready branch. Only merges from `release` are permitted.
-   **`release/*`**: Pre-production staging branches for final audit and hardening.
-   **`feature/*`**: Development branches for new features or protocol enhancements.
-   **`fix/*`**: Critical bug fixes or security patches.

### Code Style

-   **Rust**: Use `cargo fmt` and ensure all code passes `cargo clippy` with zero warnings.
-   **JavaScript/CSS**: Use Prettier for consistent formatting across the frontend.

## Adding New ZK-Circuits

Qanto uses a modular ZK-proof system based on **arkworks**. To add a new circuit:

1.  **Define the Circuit Structure**: Create a new struct in `src/zkp.rs` implementing `ConstraintSynthesizer<ConstraintF>`.
2.  **Define Constraints**: Implement the `generate_constraints` method using R1CS gadgets.
3.  **Integrate with ZKManager**: 
    - Add the new variant to `ZKProofType`.
    - Update `ZKProofSystem::generate_proof` to handle the new type.
4.  **Add Unit Tests**: Every circuit must have a corresponding `#[test]` verifying successful proof/verification cycles.

## Submitting Changes

1.  Create a feature branch.
2.  Write tests for your changes.
3.  Submit a Pull Request with a clear description of the problem and the proposed solution.
4.  Ensure all CI/CD pipelines pass.

## Security

If you discover a security vulnerability, please do NOT open a public issue. Instead, refer to our [Security Policy](SECURITY.md) for reporting instructions.
