#!/usr/bin/env bash
set -euo pipefail

echo "=== cargo fmt --check (this workspace only) ==="
# Limit to the workspace crates: `cargo fmt --all` would also traverse the
# ../gitnapse path dependency, coupling this repo's CI to the core's files.
cargo fmt --package gitnapse-protocol --package gitnapse-server --package gitnapse-client -- --check

echo ""
echo "=== cargo clippy --workspace --all-targets -- -D warnings ==="
cargo clippy --workspace --all-targets -- -D warnings

echo ""
echo "=== cargo test --workspace --all-targets ==="
cargo test --workspace --all-targets

echo ""
echo "=== cargo audit ==="
cargo audit --ignore RUSTSEC-2023-0071

echo ""
echo "All CI checks passed."
