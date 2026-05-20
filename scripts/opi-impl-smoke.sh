#!/usr/bin/env bash
set -euo pipefail

# opi-implement boot smoke — runs at Phase A.3 of every invocation.
# Tier-specific verification lives in Phase D; this catches broken workspace health early.

echo "=== opi-impl smoke ==="

# Gate 1: Rust toolchain present
rustc --version >/dev/null 2>&1 || { echo "FAIL: rustc not found"; exit 1; }
cargo --version >/dev/null 2>&1 || { echo "FAIL: cargo not found"; exit 1; }

# Gate 2: Workspace compiles
echo "Checking workspace build..."
cargo build --workspace 2>&1 || { echo "FAIL: cargo build --workspace"; exit 1; }

# Gate 3: Format check
echo "Checking format..."
cargo fmt --check --all 2>&1 || { echo "FAIL: cargo fmt --check"; exit 1; }

# Gate 4: Clippy
echo "Checking clippy..."
cargo clippy --workspace --all-targets -- -D warnings 2>&1 || { echo "FAIL: clippy"; exit 1; }

# Gate 5: Tests pass
echo "Running tests..."
cargo test --workspace --all-targets 2>&1 || { echo "FAIL: cargo test"; exit 1; }

echo "=== smoke PASSED ==="
