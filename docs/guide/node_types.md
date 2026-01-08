# Qanto Node Types and Startup Configuration

This guide documents the available Qanto node types, how to select them, and what features and resource defaults they enable. It complements the in-code startup banner emitted by `QantoDAG::new` which summarizes the effective configuration and progress during initialization.

## Selecting a Node Type

- Set the environment variable `QANTO_NODE_TYPE` before starting the node.
- Accepted values: `light`, `light_client`, `full`, `rpc`, `archive`.
- If not set, the node defaults to `full`.

Example (Unix shells):

```bash
export QANTO_NODE_TYPE=light
./target/release/qanto start --config config.toml
```

Example (Windows PowerShell):

```powershell
$env:QANTO_NODE_TYPE = "rpc"
qanto.exe start --config config.toml
```

## Node Type Matrix

- `light`
  - Consensus: enabled
  - JSON-RPC: enabled
  - Fast sync: enabled
  - Mining: enabled
  - Full validation: disabled
  - Compression, Indexing: disabled
  - Resource guards: `soft_memory_mb=512`, `hard_memory_mb=1024`, `soft_cpu_pct=50`, `hard_cpu_pct=85`
  - Default log level: `warn`

- `full` (default)
  - Consensus: enabled
  - JSON-RPC: enabled
  - Fast sync: enabled
  - Mining: enabled
  - Full validation: enabled
  - Compression: enabled
  - Indexing: enabled
  - Resource guards: `soft_memory_mb=2048`, `hard_memory_mb=8192`, `soft_cpu_pct=70`, `hard_cpu_pct=90`
  - Default log level: `info`

- `rpc`
  - Consensus: disabled (API-only mode)
  - JSON-RPC: enabled
  - Fast sync: enabled
  - Mining: enabled
  - Full validation: enabled
  - Compression: disabled
  - Indexing: disabled
  - Request cache size: `10_000` (for API performance)
  - Resource guards: `soft_memory_mb=1024`, `hard_memory_mb=4096`, `soft_cpu_pct=60`, `hard_cpu_pct=90`
  - Default log level: `debug`

- `archive`
  - Consensus: enabled
  - JSON-RPC: enabled
  - Fast sync: disabled
  - Mining: enabled
  - Full validation: enabled
  - Compression: enabled
  - Indexing: enabled (build and maintain global indexes)
  - Resource guards: `soft_memory_mb=8192`, `hard_memory_mb=32768`, `soft_cpu_pct=70`, `hard_cpu_pct=95`
  - Default log level: `trace`

## Startup Banner

On startup, the node emits a structured banner containing:

- Node type and software version
- Build timestamp, commit hash, Rust toolchain
- Initialization phase (`Booting`, `FigureBlocks`, `Ready`)
- Progress and ETA
- Figure-blocks statistics (loaded blocks, height, throughput)
- Storage bytes loaded
- System resources (memory usage, CPU cores)
- Logging level

This helps operators quickly assess the node’s initialization status and resource posture.

## Validation Rules

The initializer enforces configuration consistency:

- `rpc` node type can enable consensus or mining.
- `light` node type must NOT enable full validation.

Violations result in startup errors surfaced via `QantoDAGError`.

## Operational Notes

- For production deployments, prefer `full` or `archive` based on storage and indexing requirements.
- `light` is suited for resource-constrained clients; use `fast sync` to minimize validation workload.
- `rpc` is intended for API gateways and read-only services.
- Adjust `logging` in `config.toml` if you need more or less verbosity than the node-type defaults.

## Troubleshooting

- If initialization appears slow, check the `FigureBlocks` throughput and storage bytes loaded.
- Ensure system memory isn’t exceeding soft/hard limits; warnings are emitted when guards are crossed.
- Verify `QANTO_NODE_TYPE` spelling; unrecognized values default to `full`.