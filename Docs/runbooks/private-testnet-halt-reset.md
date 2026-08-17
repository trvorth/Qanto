# Private Testnet Halt and Reset Procedure

This document outlines the procedure for halting and resetting the private engineering testnet.

## Safety Requirements

**NEVER** run these procedures against production or value-bearing data.

## Prerequisites

1. You must have set the environment variable `$QANTO_DISPOSABLE_DATA_DIR` to a directory that:
   - Is not the root directory (`/`)
   - Is not your home directory (`$HOME`)
   - Is not the repository root
   - Is not a symlink
   - Contains a file named `.qanto-private-disposable` (a sentinel file)

2. The sentinel file `.qanto-private-disposable` must exist in `$QANTO_DISPOSABLE_DATA_DIR` to confirm this is a disposable data directory.

## Halt Procedure

1. Stop the Qanto node process gracefully (e.g., SIGTERM).
2. Verify the node has stopped by checking for the absence of the process.
3. Do not delete any data yet; the halt is only for stopping the node.

## Reset Procedure

1. Ensure the node is halted (see Halt procedure).
2. Verify `$QANTO_DISPOSABLE_DATA_DIR` meets all safety requirements:
   - Not `/`, `$HOME`, or repository root
   - Not a symlink
   - Contains `.qanto-private-disposable` sentinel file
3. Delete the contents of `$QANTO_DISPOSABLE_DATA_DIR`:
   ```bash
   rm -rf "$QANTO_DISPOSABLE_DATA_DIR"/*
   ```
4. Verify the directory is empty except potentially for the sentinel file (which may have been deleted; if so, recreate it).
5. Optionally, recreate the sentinel file:
   ```bash
   touch "$QANTO_DISPOSABLE_DATA_DIR"/.qanto-private-disposable
   ```
6. Restart the Qanto node with the private engineering configuration pointing to `$QANTO_DISPOSABLE_DATA_DIR` for data storage.

## Important Notes

- This procedure results in **complete and irreversible loss of all blockchain state**, including transaction history, accounts, and smart contract deployments.
- The genesis block will be reinitialized from the configured genesis file.
- All keys and accounts must be re-provisioned after reset.
- Never use real keys or non-disposable data with this procedure.
- Always verify the sentinel file and directory constraints before deletion to prevent accidental data loss.

## Example Usage

```bash
export QANTO_DISPOSABLE_DATA_DIR="/tmp/qanto-private-testnet"
# Ensure the directory contains the sentinel file
touch "$QANTO_DISPOSABLE_DATA_DIR"/.qanto-private-disposable
# Halt the node (send SIGTERM to the Qanto process)
# Verify halt
# Reset
rm -rf "$QANTO_DISPOSABLE_DATA_DIR"/*
touch "$QANTO_DISPOSABLE_DATA_DIR"/.qanto-private-disposable
# Restart node with --data-dir "$QANTO_DISPOSABLE_DATA_DIR"
```