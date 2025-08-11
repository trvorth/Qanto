<general_rules>
When creating new functions or modules, always first search existing directories (e.g., `src/`) to see if similar functionality exists or if there's an appropriate existing file to extend. If not, create a new file in a logical and relevant directory.

All Rust code must adhere to the standard Rust formatting guidelines. Before submitting any code, ensure it is formatted by running `cargo fmt`. Additionally, run `cargo clippy` to catch common mistakes and improve code quality; all warnings should be addressed.

If you've added new code that requires testing, ensure you write comprehensive tests for it. Before submitting a pull request, verify that the entire test suite passes by running `cargo test`.

Commonly executed commands:
- `cargo build`: Compiles the entire Rust project.
- `cargo run --bin <binary_name>`: Executes a specific binary from `src/bin/`.
- `cargo test`: Runs all unit and integration tests.
- `cargo fmt -- --check`: Checks if the code is properly formatted without modifying files.
- `cargo clippy --workspace -- -D warnings`: Analyzes the code for common errors and stylistic issues, treating warnings as errors.
</general_rules>

<repository_structure>
The Qanto repository is primarily a Rust-based project with a Python Flask web application component.

- **Root Directory**: Contains the main `Cargo.toml` for the Rust workspace, `app.py` for the Flask application, `docker-compose.yml` for containerized deployment, and top-level configuration files.
- **`src/`**: This is the core Rust source code directory for the Qanto node.
    - **`src/bin/`**: Houses various executable binaries, including `qanto` (the main node), `qantowallet` (CLI wallet), `saga_assistant`, `saga_web`, and other utilities like `import_wallet`, `monitor`, and `omega_test`.
    - **`src/node.rs`**: The central orchestrator of the Qanto node, managing other components.
    - **`src/p2p.rs`**: Implements the peer-to-peer networking layer using `libp2p`.
    - **`src/qantodag.rs`**: Defines the Directed Acyclic Graph (DAG) ledger structure.
    - **`src/consensus.rs`**: Contains the logic for the hybrid Proof-of-Work and Delegated Proof-of-Stake (PoW+DPoS) consensus mechanism.
    - **`src/mempool.rs`**: Manages unconfirmed transactions.
    - **`src/wallet.rs`**: Provides core wallet functionalities.
    - **`src/saga.rs`**: Implements the AI-driven governance system.
    - **`src/omega.rs`**: Contains the conscious security layer logic.
- **`myblockchain/`**: A separate Rust workspace member, likely containing foundational blockchain primitives or a distinct blockchain implementation.
- **`qanto_web/`**: Contains the Python Flask web application, including routes, models, and utilities.
- **`docs/`**: Comprehensive documentation, including architectural overviews (`Architecture.md`), contribution guidelines, wallet guides, and testnet information.
- **`.github/workflows/`**: Stores GitHub Actions CI/CD configurations for automated builds and tests.
- **`migrations/`**: Contains Alembic migration scripts for the `qanto_web` Python application's database schema.
</repository_structure>

<dependencies_and_installation>
The project primarily uses Rust, with a Python component for the web interface.

**Rust Dependencies**:
- **Rust Toolchain**: The latest stable Rust toolchain is required, installed via `rustup`.
- **Git**: Necessary for cloning the repository.
- **Build Essentials**: A C++ compiler and the `RocksDB` library are crucial. Specific installation steps vary by operating system:
    - **macOS**: Install `Homebrew`, then `brew install rocksdb`.
    - **Linux (Ubuntu/Debian)**: `sudo apt-get install build-essential clang librocksdb-dev pkg-config libssl-dev`.
    - **Windows**: Install Microsoft C++ Build Tools (via Visual Studio Build Tools installer) and `RocksDB` via `vcpkg`.
- **Cargo**: Rust's package manager, `Cargo`, handles all Rust-specific dependencies listed in `Cargo.toml`. Key dependencies include `libp2p`, `tokio`, `axum` (for web services), various cryptography libraries, and optional features for AI (`tch`), GPU mining (`ocl`), and Zero-Knowledge proofs (`bellman`, `bls12_381`, `ff`).

**Python Dependencies**:
- The `qanto_web` application is a Flask project. While no explicit `requirements.txt` or `pyproject.toml` was found, Python dependencies would typically be managed via `pip` or `poetry`.

**Docker**:
- The `docker-compose.yml` file indicates that Docker can be used to build and run the `qanto-node` within a containerized environment, simplifying dependency management and deployment.
</dependencies_and_installation>

<testing_instructions>
Testing in this repository is primarily conducted using Rust's built-in testing framework.

**Testing Frameworks**:
- **Rust's `test` module**: Used for writing unit and integration tests.
- **`criterion`**: A development dependency used for benchmarking Rust code.
- **`serial_test`**: A development dependency used to serialize tests, preventing conflicts when tests modify shared resources.

**How to Run Tests**:
- To execute all tests across the entire Rust workspace, navigate to the root directory of the repository and run:
    ```bash
    cargo test --workspace --verbose
    ```
- Individual test functions are typically defined within modules and marked with the `#[test]` attribute. Examples of files containing tests include `src/transaction.rs`, `src/saga.rs`, `src/omega.rs`, `src/node.rs`, `src/integration.rs`, and `src/config.rs`.

**Types of Modules Tested**:
Tests cover a wide range of functionalities, including:
- Core blockchain components: Transaction creation and verification, P2P block propagation, state synchronization, and node initialization.
- AI and Security modules: Functionality of the SAGA AI system and the Omega conscious security layer.
- Configuration management: Ensuring configuration loading, saving, and validation work correctly.

When writing new tests, ensure they are placed in a logical location, typically within the same module as the code they are testing, or in a dedicated `tests/` directory for integration tests if applicable.
</testing_instructions>

<pull_request_formatting>
When submitting pull requests, please adhere to the following guidelines for commit messages and pull request descriptions:

**Git Commit Messages**:
- Use the present tense (e.g., "Add feature" instead of "Added feature").
- Use the imperative mood (e.g., "Move file to..." instead of "Moves file to...").
- Limit the first line of the commit message to 72 characters or less.
- Reference relevant issues and pull requests liberally after the first line.

**Pull Request Description**:
The pull request description should provide a clear and concise overview of your changes. Please include the following sections:

- **Description**: A brief summary of the changes.
    - **What does this PR do?**: Explain the purpose and scope of the changes.
    - **Related Issue(s)**: Link to any relevant issues (e.g., `fixes #123`).
- **Changes**: Categorize the type of changes made.
    - `[ ] Bug fix`
    - `[ ] New feature`
    - `[ ] Documentation update`
    - `[ ] Other (please specify)`
- **Testing**: Describe how you tested these changes (e.g., unit tests, manual testing). Include any specific instructions for reviewers to verify the changes.
- **Checklist**: Ensure you have completed the following:
    - `[ ] Code follows Qanto's style guidelines.`
    - `[ ] Changes have been tested locally.`
    - `[ ] Documentation has been updated (if applicable).`
    - `[ ] Security implications have been considered (e.g., IDS integration, cryptography).`
- **Additional Notes**: Add any other context, such as dependencies or potential impacts on the network.
</pull_request_formatting>


