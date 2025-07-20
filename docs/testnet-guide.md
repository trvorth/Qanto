# Qanto Public Testnet Guide (Codename: Ignition)

Welcome to the Qanto Public Testnet! This guide provides all the necessary steps to compile the source code, configure your node, and connect to the network.

## 1. Prerequisites

Before you begin, ensure you have the following installed:

* **Rust Toolchain**: Install via `rustup`.
* **Git**: For cloning the repository.
* **Build Essentials**: A C++ compiler and the RocksDB library.
    * **Ubuntu/Debian**: `sudo apt-get install build-essential clang librocksdb-dev`
    * **macOS (Homebrew)**: `brew install rocksdb`

## 2. Compile the Node

First, clone the official repository and compile the project in release mode.

```bash
git clone [https://github.com/trvorth/Qanto.git](https://github.com/trvorth/Qanto.git)
cd Qanto
cargo build --release --features infinite-strata
