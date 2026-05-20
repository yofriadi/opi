# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

`opi` is a Rust reimplementation of [earendil-works/pi](https://github.com/earendil-works/pi) — an AI agent toolkit. Currently in scaffolding phase: every crate exposes its module structure but contains stub implementations. New work fills in those stubs rather than redesigning the layout.

Repository: https://github.com/OdradekAI/opi

## Workspace layout

Cargo workspace with **lockstep versioning** (all crates share `version.workspace = true`). Internal dependencies flow through `[workspace.dependencies]` in the root `Cargo.toml`:

```
opi-ai      (no internal deps)        — unified multi-provider LLM API
opi-tui     (no internal deps)        — terminal UI with differential rendering
opi-agent   → opi-ai                  — agent runtime, tool calling, transport
opi-web-ui  → opi-ai                  — web chat components
opi-coding-agent → opi-ai, opi-agent, opi-tui  — produces the `opi` binary
```

The dependency order above is also the **crates.io publish order** (see the `opi-release` skill). Adding a new internal dependency means updating `[workspace.dependencies]` in root `Cargo.toml`, then referencing it as `foo = { workspace = true }` in the consumer crate's `Cargo.toml`.

When publishing internal crates, the `path` dependencies MUST also carry a `version` field — bare path deps cannot be published to crates.io. The release skill manages this.

## Edition

Workspace is on **Rust edition 2024**, so a recent stable toolchain is required (the edition gained stable support in Rust 1.85+).

## Common commands

```sh
# Build everything
cargo build
cargo build --release

# Run the CLI binary
cargo run -p opi-coding-agent             # → prints "opi - AI coding agent"
cargo run -p opi-coding-agent -- --version

# Tests
cargo test --workspace --all-targets
cargo test -p opi-ai                      # single crate
cargo test -p opi-ai -- some_test_name    # single test

# Lint & format (these are the gates the release skill enforces)
cargo fmt --all
cargo fmt --check --all
cargo clippy --workspace --all-targets -- -D warnings

# Docs with warnings-as-errors (release gate)
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
```

## Releasing

Releases go to both **GitHub Releases** and **crates.io** via the `opi-release` skill at `.claude/skills/opi-release/skill.md`. Invoke with a target semver version (e.g. `0.2.0`). Critical properties of that flow:

- **Phases 1–4 are reversible**; **Phase 5 pushes a tag (publicly visible)**; **Phase 6 publishes to crates.io and is irreversible** (crates can only be yanked, never deleted).
- All crates publish at the same version, in dependency order, computed dynamically via `cargo metadata`.
- Never use `git reset --hard` + `git push --force` for rollback. Use `git revert` + tag deletion (this is enforced in the skill).
- Interrupted releases can resume from `.opi-release-state.json` at the repo root.
- **CI-driven builds** (recommended): Push the tag → `release.yml` builds all 6 platform targets and uploads to the GitHub Release. No local `cross` needed.

The design rationale is in `docs/superpowers/specs/2026-05-19-opi-release-skill-design.md`.

## CI

Two GitHub Actions workflows in `.github/workflows/`:

- **ci.yml** — Runs on push/PR to `main`. Jobs: `fmt`, `clippy`, `test`, `doc`. These are the gates that Phase 1.3 of the release skill checks.
- **release.yml** — Triggered by `v*` tags or manual `workflow_dispatch`. Builds `opi` binary for 6 targets (linux-x64, linux-arm64, darwin-x64, darwin-arm64, windows-x64, windows-arm64) using native runners + `cross` for linux-arm64. Uploads artifacts to the GitHub Release.

## Conventions

- Conventional Commits drive changelog categorization (`feat:` → Added, `fix:` → Fixed, `feat!:`/`BREAKING CHANGE` → Breaking Changes).
- Each crate's `description`, `license`, and `repository` come from the workspace — don't duplicate them per crate.
- The CLI binary is named `opi` (defined by `[[bin]]` in `crates/opi-coding-agent/Cargo.toml`), not `opi-coding-agent`.
