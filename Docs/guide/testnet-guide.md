# Qanto Public Testnet Guide (Codename: Ignition)

Welcome to the Qanto Public Testnet! This guide provides all the necessary steps to compile the source code, configure your node, and connect to the network across all major operating systems (Windows, macOS, and Linux).

## 1. Prerequisites

Before you begin, ensure you have the following installed on your system:

* **Rust Toolchain**: The latest stable version of Rust, installed via `rustup`. If you don't have it, open your terminal and run:
    ```bash
    curl --proto '=https' --tlsv1.2 -sSf [https://sh.rustup.rs](https://sh.rustup.rs) | sh
    ```
* **Git**: Required for cloning the source code repository.
* **Build Essentials**: A C++ compiler and the RocksDB library are essential dependencies. Follow the instructions for your specific operating system below.

---

## 2. System-Specific Setup

### For macOS

1.  **Install Homebrew**: If you don't have it, open your terminal and run:
    ```bash
    /bin/bash -c "$(curl -fsSL [https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh](https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh))"
    ```
2.  **Install Dependencies**:
    ```bash
    brew install rocksdb
    ```

### For Linux (Ubuntu/Debian)

1.  **Install Dependencies**: Open your terminal and run the following command:
    ```bash
    sudo apt-get update && sudo apt-get install build-essential clang librocksdb-dev pkg-config libssl-dev
    ```

### For Windows

Building on Windows requires the MSVC C++ build tools and installing RocksDB via `vcpkg`.

1.  **Install Microsoft C++ Build Tools**:
    * Download and run the [Visual Studio Build Tools installer](https://visualstudio.microsoft.com/visual-cpp-build-tools/).
    * In the installer, select the **"C++ build tools"** workload. Make sure the latest Windows SDK and the English language pack are included.

2.  **Install and Configure `vcpkg`**:
    * Open PowerShell and clone the `vcpkg` repository:
        ```powershell
        git clone [https://github.com/Microsoft/vcpkg.git](https://github.com/Microsoft/vcpkg.git)
        cd vcpkg
        ./bootstrap-vcpkg.bat
        ./vcpkg integrate install
        ```

3.  **Install RocksDB using `vcpkg`**:
    * This command will install the 64-bit version of RocksDB. It may take some time.
        ```powershell
        ./vcpkg.exe install rocksdb:x64-windows
        ```
4.  **Configuration of Environment Variables**:
    * An environment variable must be established to inform the Cargo build system of the location of the RocksDB library files. A PowerShell terminal with administrative privileges must be utilized to execute the following command, with the file path adjusted to correspond to the `vcpkg` installation directory:
        ```powershell
        [System.Environment]::SetEnvironmentVariable('ROCKSDB_LIB_DIR', 'C:\path\to\vcpkg\installed\x64-windows\lib', [System.EnvironmentVariableTarget]::Machine)
        ```
    * **Note Bene**: A restart of the terminal or Integrated Development Environment (IDE) is mandatory for this environment variable modification to take effect.

---

## 3. Compile the Qanto Node

Once the prerequisites are installed, you can clone the repository and compile the project.

1.  **Clone the Repository**:
    ```bash
    git clone [https://github.com/trvorth/Qanto.git](https://github.com/trvorth/Qanto.git)
    cd Qanto
    ```

2.  **Compile the Node**:
    * This command builds the project in `release` mode for optimal performance and enables the `infinite-strata` feature.
    ```bash
    cargo build --release --features infinite-strata
    ```
    The compiled binaries will be located in the `target/release/` directory.

---

## 4. Configure and Run Your Node

Now that the software is compiled, you can generate a wallet and configure your node to join the testnet.

1.  **Generate Your Wallet**:
    * The `qantowallet` tool creates your encrypted `wallet.key` file. You will be prompted to create a secure password.
    ```bash
    cargo run --release --bin qantowallet -- gen
    ```
    * **CRITICAL**: After creating the wallet, run `cargo run --release --bin qantowallet -- show-keys` to view your public address and mnemonic phrase. **Write down the mnemonic phrase and store it in a secure, offline location.**

2.  **Configure Your Node**:
    * Copy the example configuration file:
        ```bash
        cp config.toml.example config.toml
        ```
    * Open the new `config.toml` file in a text editor.
    * Find the `genesis_validator` field and replace the placeholder with the public address you just generated.
    * (Optional) Find the `peers` array and add the multiaddresses of other testnet nodes you want to connect to. A list of official bootnodes will be provided by the project team.

3.  **Start the Node**:
    * Run the node with the `--clean` flag on the first start to ensure no old database files cause issues. You will be prompted for the wallet password you created.
    ```bash
    cargo run --release --features infinite-strata --bin qanto -- start --config config.toml --wallet wallet.key --clean
    ```

Your node will now start, initialize its services using the **X-PHYRUS™** pre-boot sequence, and attempt to connect to the Qanto network.

## Cloud Deployment

For cloud deployment, use the provided infrastructure templates and deployment scripts.

4.  **Demonstrating the ΛΣ-ΩMEGA Module**

To see a simulation of the conscious security layer, run the `omega_test` binary:

```bash
cargo run --bin omega_test
```
