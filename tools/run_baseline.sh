#!/usr/bin/env bash
# Reproduce the Qanto baseline toolchain run and write raw logs to audit-results/baseline/.
#
# Usage:   bash tools/run_baseline.sh
# Output:  audit-results/baseline/{fmt,check,clippy,test,audit}.log + status.txt
#
# This script intentionally does not silence output. Exit codes are recorded
# in status.txt and the calling tool must verify each exit code is zero before
# reporting PASS.

set -o pipefail
set -u

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORKSPACE_ROOT="${REPO_ROOT}/Protocol_Core"
OUT_DIR="${REPO_ROOT}/audit-results/baseline"
mkdir -p "${OUT_DIR}"

cd "${WORKSPACE_ROOT}"

cargo fmt --all --check > "${OUT_DIR}/fmt.log" 2>&1;     FMT=$?
cargo check --workspace --all-targets --locked > "${OUT_DIR}/check.log" 2>&1; CHECK=$?
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings \
    > "${OUT_DIR}/clippy.log" 2>&1; CLIPPY=$?
cargo test --workspace --all-targets --locked > "${OUT_DIR}/test.log" 2>&1; TEST=$?

# The workspace Cargo.lock lives at the repository root, not inside Protocol_Core/.
# `cargo audit` resolves lockfiles relative to the current directory; point it explicitly.
( cd "${REPO_ROOT}" && cargo audit --file Cargo.lock > "${OUT_DIR}/audit.log" 2>&1 ); AUDIT=$?

{
    printf 'fmt=%s\ncheck=%s\nclippy=%s\ntest=%s\naudit=%s\n' \
        "${FMT}" "${CHECK}" "${CLIPPY}" "${TEST}" "${AUDIT}"
    printf 'notes: clippy uses --all-features; audit runs from repo root against Cargo.lock\n'
} | tee "${OUT_DIR}/status.txt"

# Do not claim success unless every command actually succeeded.
if [[ "${FMT}" -ne 0 || "${CHECK}" -ne 0 || "${CLIPPY}" -ne 0 || "${TEST}" -ne 0 || "${AUDIT}" -ne 0 ]]; then
    printf 'baseline FAILED — see %s for raw logs\n' "${OUT_DIR}" >&2
    exit 1
fi

printf 'baseline OK\n'
