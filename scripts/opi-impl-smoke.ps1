#Requires -Version 5.1
Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

Write-Host "=== opi-impl smoke ==="

# opi-implement boot smoke: tier-specific verification lives in Phase D.

# Gate 1: Rust toolchain present
try { rustc --version | Out-Null } catch { Write-Error "FAIL: rustc not found"; exit 1 }
try { cargo --version | Out-Null } catch { Write-Error "FAIL: cargo not found"; exit 1 }

# Gate 2: Workspace compiles
Write-Host "Checking workspace build..."
cargo build --workspace
if ($LASTEXITCODE -ne 0) { Write-Error "FAIL: cargo build --workspace"; exit 1 }

# Gate 3: Format check
Write-Host "Checking format..."
cargo fmt --check --all
if ($LASTEXITCODE -ne 0) { Write-Error "FAIL: cargo fmt --check"; exit 1 }

# Gate 4: Clippy
Write-Host "Checking clippy..."
cargo clippy --workspace --all-targets -- -D warnings
if ($LASTEXITCODE -ne 0) { Write-Error "FAIL: clippy"; exit 1 }

# Gate 5: Tests pass
Write-Host "Running tests..."
cargo test --workspace --all-targets
if ($LASTEXITCODE -ne 0) { Write-Error "FAIL: cargo test"; exit 1 }

Write-Host "=== smoke PASSED ==="
