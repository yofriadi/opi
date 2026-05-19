# opi-release Skill Design

## Overview

A Claude Code skill that orchestrates the full release process for the opi Rust workspace, publishing to both GitHub Releases and crates.io with phased gates for safety.

## Trigger

- `/opi-release <version>` — run full release flow
- `/opi-release <version> --fix` — auto-fix fmt/clippy issues during pre-flight
- `/opi-release <version> --skip-cross` — skip cross-compilation (source-only release)

## Release Readiness Prerequisites

One-time setup tasks that must be completed before the release skill can be used for the first time. The skill checks these and reports missing items as blockers.

### Required Files
- `LICENSE` (MIT) at workspace root — crates.io packaging requires it
- `README.md` at workspace root
- Per-crate `README.md` or `readme = "../README.md"` in each crate's `Cargo.toml`

### Workspace Dependency Configuration
Internal workspace dependencies MUST include both `path` and `version` for crates.io publishing:
```toml
[workspace.dependencies]
opi-ai = { path = "crates/opi-ai", version = "=0.1.0" }
opi-agent = { path = "crates/opi-agent", version = "=0.1.0" }
opi-tui = { path = "crates/opi-tui", version = "=0.1.0" }
opi-web-ui = { path = "crates/opi-web-ui", version = "=0.1.0" }
opi-coding-agent = { path = "crates/opi-coding-agent", version = "=0.1.0" }
```
Without `version`, `cargo publish` fails during dependency resolution. The version MUST be updated alongside `workspace.package.version` during Phase 2.

### CLI Version Command
`opi --version` must output the current version string. This is a hard prerequisite — the artifact self-check and post-release verification depend on it. Current `main.rs` only prints a static string; it must be updated to use `env!("CARGO_PKG_VERSION")` or `clap`'s version derivation.

### .gitignore Coverage
`.gitignore` must include:
- `/target`
- `.env*`
- `*.key`, `*.pem`, `*.p12`, `*.pfx`
- `credentials.toml`
- `.opi-release-state.json`

### Authentication
- `cargo login` completed (credentials in `~/.cargo/credentials.toml`) or `CARGO_REGISTRY_TOKEN` set
- `gh auth status` passes
- User is an owner of all workspace crates on crates.io (first publish: user must `cargo publish` manually or skill handles initial publish)

## Architecture: Seven Phased Gates

Seven phases, each reports status and requires confirmation before proceeding. Irreversible operations (crates.io) are last. GitHub Release uses draft mode until crates.io completes.

```
Phase 1: Pre-flight checks (code quality, security, metadata)
Phase 2: Version bump + dry-run validation
Phase 3: Changelog generation
Phase 4: Build, cross-compile & artifact self-check
Phase 5: Commit, tag, push & GitHub Draft Release (PARTIALLY reversible)
Phase 6: Publish to crates.io (IRREVERSIBLE)
Phase 7: Finalize — publish draft release, post-release verification
```

### Irreversibility Boundaries
- **Phase 1–4**: Fully reversible. No side effects outside local filesystem.
- **Phase 5**: Push to main and tag are PUBLIC immediately. Draft release is not visible, but the commit and tag are. Rollback requires `git revert` (adds a new commit) — history cannot be erased without force push.
- **Phase 6**: crates.io publishes are permanent. Yanking hides a version from dependency resolution but does not delete it. Version numbers are consumed forever.
- **Phase 7**: Publishing the draft release is trivially reversible (can re-draft or delete).

## Phase 1: Pre-flight Checks

### File Presence (critical)
- `LICENSE` file exists at workspace root (crates.io packaging requires it)
- `README.md` exists at workspace root
- Each crate directory has `README.md` or root README is referenced via `readme` field
- `.gitignore` exists and covers `/target`, `.env*`, `*.key`, `*.pem`, `.opi-release-state.json`
- `Cargo.lock` is committed (required for reproducible binary builds)

### Git State
- Working tree clean (no uncommitted changes)
- On `main` branch
- Local in sync with remote (no unpushed commits, no upstream changes)
- Git tag `v<version>` does not exist locally or remotely

### CI Status (HEAD-bound)
Bind CI check to the exact current HEAD, not just "latest on main":
```bash
HEAD_SHA=$(git rev-parse HEAD)
gh api repos/OdradekAI/opi/commits/$HEAD_SHA/check-runs \
  --jq '.check_runs[] | {name, conclusion}'
```
- ALL required checks must be `success` for the current HEAD SHA
- If any check is `failure` or `pending`, **BLOCKED**
- If no CI configured (no check runs found), warn but allow to continue
- Fallback: `gh run list --commit $HEAD_SHA --json conclusion` if check-runs API unavailable

### Code Quality
- `cargo fmt --check --all` passes
- `cargo clippy --workspace --all-targets -- -D warnings` passes
- `cargo test --workspace --all-targets` passes
- `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` passes (warnings are errors)

**If `--fix`:** Run `cargo fmt --all` and `cargo clippy --fix --workspace --allow-dirty` to auto-fix. Then re-verify.

### Documentation Currency
- `README.md` last modified within 90 days or matches current crate list
- Each crate's `description` in Cargo.toml is non-empty and meaningful (not placeholder text)
- If `CHANGELOG.md` exists, verify it's not stale (last entry matches previous release)

### Security & Dependencies
- `cargo audit` passes (if installed; warn if not available)
- No `[patch]` sections in Cargo.toml (crates.io rejects these)
- No git dependencies (crates.io rejects these)
- No path dependencies outside workspace (crates.io rejects these)
- All internal dependencies use `workspace = true` AND have `version` field
- No secrets tracked in git: `git ls-files '*.env' '*.key' '*.pem' '*.p12' '*.pfx'` returns empty
- No hardcoded tokens/keys in source: grep for common patterns (`AKIA`, `sk-`, `ghp_`, `glpat-`)

### MSRV (Minimum Supported Rust Version)
- If any crate specifies `rust-version` in Cargo.toml, verify build passes with that toolchain
- If no `rust-version` specified, warn: external users may hit implicit version requirements
- Recommend: set explicit `rust-version` in `[workspace.package]` for all crates

### Release Metadata & Permissions
- Each crate has complete metadata: `description`, `license`, `repository`
- Verify crates.io authentication: check `~/.cargo/credentials.toml` exists or `CARGO_REGISTRY_TOKEN` env is set
- Verify crate ownership: `cargo owner --list <crate>` for each crate (confirms publish permission)
- Target version does not exist on crates.io: `cargo search <crate> --limit 1` check
- New version > current version (semver ordering)

Note: `cargo publish --dry-run` is intentionally NOT run here — it validates the current version, not the target version. Dry-run validation happens in Phase 2 after the version bump.

### Package Content Check
For each crate, run `cargo package -p <crate> --list` and verify:
- No large files (>1MB) accidentally included (test fixtures, binaries, images)
- No private configuration files (`.env`, `*.local`, IDE configs)
- No build artifacts or generated files
- `include` / `exclude` fields in Cargo.toml are reasonable
- Total package size is within expectations (warn if >5MB)

### Version Semantics Check
- If commits contain `BREAKING CHANGE` or `feat!:`, version should be major bump
- If commits contain `feat:`, version should be at least minor bump
- If only `fix:`, `docs:`, etc., patch bump is appropriate
- Warn if version bump doesn't match commit types

### CLI Version Command Check
- Verify `opi --version` (or `cargo run -p opi-coding-agent -- --version`) outputs a version string
- If the binary has no `--version` flag, **BLOCKED** — this is a prerequisite (see Release Readiness Prerequisites)

### Post-Fix Re-verification
**IMPORTANT:** If `--fix` was used and any auto-fixes were applied:
1. Re-run `cargo fmt --check --all` and `cargo clippy` to confirm fixes resolved all issues
2. Commit fixes: `git add -A && git commit -m "chore: pre-release auto-fix"`
3. Re-verify git state is clean: `git status --porcelain` must return empty
4. If tree is still dirty after commit, abort and report remaining issues

### Phase 1 Report
Output a summary table after all checks complete:

```
Check               | Status   | Details
--------------------|----------|--------
File Presence       | PASS     | LICENSE, README.md, Cargo.lock present
Git State           | PASS     | Clean tree, on main, synced
CI Status           | PASS     | All checks passed for abc1234
Code Quality        | FAIL     | 2 clippy warnings
Tests               | PASS     | 47/47 passing
Documentation       | WARN     | README last modified 120 days ago
Security            | PASS     | No vulnerabilities, no secrets
Dependencies        | PASS     | No git/patch deps, version fields present
MSRV                | WARN     | No rust-version specified
Package Content     | PASS     | All packages <1MB, no large files
Release Metadata    | PASS     | All crates have complete metadata
Crate Ownership     | PASS     | User is owner of all crates
Version Semantics   | WARN     | feat commits found, but patch bump specified
CLI --version       | PASS     | Outputs "0.1.0"
```

If any critical check fails: **BLOCKED — fix issues before proceeding.**
If all critical pass (warnings are advisory): **READY — proceed to Phase 2?**

## Phase 2: Version Bump + Dry-Run Validation

### Version Update
- Update `workspace.package.version` in root `Cargo.toml`
- Update `version` field in ALL internal `[workspace.dependencies]` entries to match (e.g., `version = "=0.2.0"`)
- Run `cargo check --workspace` to verify update is valid
- Show diff of changed lines

### Publish Dry-Run Gate
After version bump, validate that each crate can be packaged and published:
```bash
cargo publish --dry-run -p <crate>
```
Run for each publishable crate in dependency order. This validates:
- Package metadata is complete for the NEW version
- Dependencies resolve correctly with the new version numbers
- No files are missing from the package

If dry-run fails, **BLOCKED** — fix issues before proceeding. The version bump can be reverted with `git checkout -- Cargo.toml`.

## Phase 3: Changelog Generation

### Commit Collection
- Find last release tag: `git describe --tags --abbrev=0 --match='v*'`
- Collect commits: `git log <last-tag>..HEAD --pretty=format:'%H|%s|%an'`

### Parsing & Categorization
Parse conventional commits and group by type:

- `feat:` → **Added**
- `fix:` → **Fixed**
- `perf:` → **Performance**
- `docs:` → **Documentation**
- `refactor:` → **Changed**
- `test:` → **Testing** (internal, usually omitted from user-facing changelog)
- `chore:` → (omit from changelog)
- `BREAKING CHANGE` or `feat!:` → **Breaking Changes**
- Commits with `Removed` or `remove:` → **Removed**

### GitHub Issue/PR Linking
- Extract `#<number>` from commit messages
- Format as `([#123](https://github.com/OdradekAI/opi/issues/123))`

### Contributor Attribution
- Extract author from git log
- Format as `by [@username](https://github.com/username)` if GitHub username available
- Fall back to git author name if no GitHub mapping

### Output Format
Generate two formats:

1. **CHANGELOG.md** (Keep a Changelog style):
```markdown
## [0.2.0] - 2026-05-19

### Breaking Changes
- Description ([#123](link)) by [@user](link)

### Added
- Description ([#124](link))

### Fixed
- Description ([#125](link)) by [@user](link)

### Changed
- Description

### Removed
- Description
```

2. **GitHub Release Notes** (same content, optimized for GitHub UI)

Prepend new entry to `CHANGELOG.md` (create if doesn't exist).

## Phase 4: Build & Cross-Compile

### Release Build
- `cargo build --release --workspace`
- `cargo test --release --workspace`

### Host Capability Matrix
Cross-compilation feasibility depends on the current host. The skill MUST detect the host platform and determine which targets are buildable:

| Target | Native on | Requires |
|--------|-----------|----------|
| `x86_64-unknown-linux-gnu` | Linux x64 | `cross` or Docker on other hosts |
| `aarch64-unknown-linux-gnu` | Linux arm64 | `cross` on any host |
| `x86_64-apple-darwin` | macOS x64 | macOS host only (no cross from Linux/Windows) |
| `aarch64-apple-darwin` | macOS arm64 | macOS host only |
| `x86_64-pc-windows-msvc` | Windows x64 | Windows host or `cargo-xwin` on Linux |
| `aarch64-pc-windows-msvc` | Windows arm64 | Windows arm64 host or CI |

**Detection logic:**
1. Detect current host triple: `rustc -vV | grep host`
2. Check available tools: `cross --version`, `cargo-xwin --version`
3. For each target, classify as: `native`, `cross-available`, or `unavailable`
4. Report which targets will be built and which are skipped (with reason)
5. If fewer than 2 targets are buildable, warn user and suggest using CI release workflow instead

**Recommendation:** For full 6-platform coverage, use a CI-based release workflow (GitHub Actions matrix) rather than local cross-compilation. The skill should support both modes:
- Local mode: builds whatever the host can produce
- CI mode: triggers a release workflow and waits for artifacts

### Cross-Compilation Targets
Build `opi` binary for available platforms:

| Target | Output Name |
|--------|-------------|
| `x86_64-unknown-linux-gnu` | `opi-linux-x64.tar.gz` |
| `aarch64-unknown-linux-gnu` | `opi-linux-arm64.tar.gz` |
| `x86_64-apple-darwin` | `opi-darwin-x64.tar.gz` |
| `aarch64-apple-darwin` | `opi-darwin-arm64.tar.gz` |
| `x86_64-pc-windows-msvc` | `opi-windows-x64.zip` |
| `aarch64-pc-windows-msvc` | `opi-windows-arm64.zip` |

Use `cross` for targets classified as `cross-available` in the capability matrix.
If `--skip-cross` is set, only build for the native platform.
Skip targets classified as `unavailable` — do not fail the release for unbuildable targets.

### Asset Packaging
- Each archive contains: `opi` (or `opi.exe`) + `README.md` + `LICENSE`
- Generate `SHA256SUMS.txt` with checksums for all archives

### Artifact Self-Check
After packaging, verify each archive:
1. Unpack to temp directory and confirm expected files exist
2. Verify binary name: `opi` (Unix) or `opi.exe` (Windows)
3. For native platform: run `./opi --version` and confirm output matches target version
4. For cross-compiled: verify file type with `file` command (ELF/Mach-O/PE as expected)
5. Verify `SHA256SUMS.txt` covers all archives (count matches)
6. Verify no archive exceeds reasonable size threshold (warn if >50MB)

## Phase 5: Commit, Tag, Push & GitHub Draft Release (PARTIALLY Reversible)

### Irreversibility Warning
This phase pushes to the remote. Once executed:
- The release commit and tag are **publicly visible** on GitHub (even though the Release is draft)
- Anyone watching the repo will see the tag in their feed
- Other developers may base work on this commit
- Rollback requires `git revert` which adds a new commit to history — it does NOT erase the original

The skill MUST present this warning before proceeding:
> "Phase 5 will push a release commit and tag to origin/main. This is publicly visible immediately. The draft release itself is private, but the commit and tag are not. Proceed?"

### Commit & Tag
- Stage changes: `git add Cargo.toml Cargo.lock CHANGELOG.md`
- Commit: `git commit -m "chore: release v<version>"`
- Create tag: `git tag -a v<version> -m "Release v<version>"`
- Push: `git push origin main --follow-tags`

### Create Draft Release
Use `gh release create --draft`:
```bash
gh release create v<version> \
  --draft \
  --title "v<version>" \
  --notes-file <generated-notes> \
  opi-linux-x64.tar.gz \
  opi-linux-arm64.tar.gz \
  opi-darwin-x64.tar.gz \
  opi-darwin-arm64.tar.gz \
  opi-windows-x64.zip \
  opi-windows-arm64.zip \
  SHA256SUMS.txt
```

Draft release is NOT visible to the public. Only published after crates.io succeeds.

### User Confirmation Gate
Show user the draft release URL and ask:
> "Draft GitHub Release v<version> created at <url>. Please review. If correct, I'll proceed with the irreversible crates.io publish. Continue?"

Only proceed to Phase 6 after explicit user approval.

## Phase 6: Publish to crates.io (Irreversible)

### Dynamic Dependency-Ordered Publishing
Do NOT hardcode publish order. Compute it dynamically:

```bash
cargo metadata --format-version 1 --no-deps
```

Parse the output to:
1. Build dependency graph of workspace crates
2. Exclude crates with `publish = false`
3. Topological sort into batches (crates with no internal deps first)
4. Publish each batch in order, with delays for index propagation

**Current expected order** (for reference, but skill must compute dynamically):
- Batch 1: `opi-ai`, `opi-tui` (no internal deps)
- Batch 2: `opi-agent`, `opi-web-ui` (depend on Batch 1)
- Batch 3: `opi-coding-agent` (depends on Batch 1 & 2)

Wait 30s between batches for crates.io index update.

### Verification
After each publish, verify availability:
```bash
cargo search <crate> --limit 1 | grep <version>
```

### Error Classification & Retry Policy
Distinguish between recoverable and non-recoverable errors:

**Auto-retryable (up to 3 attempts with exponential backoff):**
- Network timeout / connection reset
- HTTP 5xx from crates.io (server error)
- "crate index not updated yet" (propagation delay)

**NOT auto-retryable (require user decision):**
- HTTP 4xx (auth failure, validation error, version conflict)
- `cargo publish` exits with explicit error message (missing field, dependency not found)
- Any error after `cargo publish` returned success but verification fails (partial state)

The skill MUST NOT automatically retry when `cargo publish` returns an explicit error code — this risks double-publishing or masking real issues.

### Failure Decision Gate
If a publish fails mid-batch, present user with explicit options:

> "❌ `cargo publish -p opi-agent` failed: <error>. Already published: opi-ai, opi-tui.
> Options:
> 1. **Retry** — fix the issue and retry this crate
> 2. **Wait & retry** — wait for index propagation (60s) then retry
> 3. **Yank & abort** — yank all already-published crates for this version, abort release
> 4. **Continue later** — keep already-published crates, record progress, exit skill
>
> Choose an option:"

The skill must NOT automatically retry without user input when real money (version numbers) is at stake.

## Phase 7: Finalize & Post-Release Verification

### Publish Draft Release
After all crates are published successfully:
```bash
gh release edit v<version> --draft=false
```

The release is now public.

### Post-Release Verification
Verify the release is functional end-to-end:

1. **crates.io install test:**
   ```bash
   cargo install opi-coding-agent --version <version>
   opi --version  # should output target version
   ```

2. **GitHub Release asset check:**
   - Download one asset from the release URL
   - Verify checksum matches `SHA256SUMS.txt`
   - If native platform: run the downloaded binary `opi --version`

3. **docs.rs build status (all crates):**
   For each published crate, check docs.rs build:
   ```bash
   curl -s -o /dev/null -w "%{http_code}" https://docs.rs/<crate>/<version>
   ```
   - 200: docs built successfully
   - 404: not yet built (may take minutes, non-blocking)
   - Report status for each crate; warn if any show build failure after 5 minutes

4. **Final report:**
   ```
   ✅ Release v<version> complete!
   
   Published crates:
     - opi-ai v<version>       https://crates.io/crates/opi-ai
     - opi-tui v<version>      https://crates.io/crates/opi-tui
     - opi-agent v<version>    https://crates.io/crates/opi-agent
     - opi-web-ui v<version>   https://crates.io/crates/opi-web-ui
     - opi-coding-agent v<version> https://crates.io/crates/opi-coding-agent
   
   GitHub Release: https://github.com/OdradekAI/opi/releases/tag/v<version>
   Install: cargo install opi-coding-agent
   ```

## Failure Recovery

### Phase 1-3 Failures
No side effects. Fix issue and re-run skill.

### Phase 4 Failures
Clean build artifacts: `cargo clean`. Fix issue and retry.

### Phase 5 Failures
- If commit/tag pushed but draft release creation failed:
  - Retry `gh release create --draft` (idempotent with same tag)
- If user rejects the draft release:
  - Delete draft: `gh release delete v<version> --yes`
  - Delete remote tag: `git push origin :refs/tags/v<version>`
  - Delete local tag: `git tag -d v<version>`
  - Revert release commit: `git revert HEAD --no-edit && git push origin main`
  - **NEVER** use `git reset --hard` + `git push --force` on main automatically
  - Force push is only allowed if user explicitly requests it AND confirms no one else has based work on the commit

### Phase 6 Failures
- Already-published crates cannot be unpublished (only yanked via `cargo yank`)
- Skill presents user with explicit decision options (see Failure Decision Gate above)
- If user chooses "Continue later":
  - Record progress to `.opi-release-state.json` (which crates published, target version)
  - On next invocation with same version, detect state file and resume from failure point
  - Skip already-published crates automatically
- If user chooses "Yank & abort":
  - Run `cargo yank -p <crate> --version <version>` for each published crate
  - Delete draft release and tag (Phase 5 rollback)
  - Revert release commit

## Implementation Notes

### Tools Required
- `cargo` (Rust toolchain)
- `cross` (for cross-compilation, install via `cargo install cross`)
- `gh` (GitHub CLI)
- `git`
- `cargo-audit` (optional, for security checks)

### Configuration
Skill should check for:
- `CARGO_REGISTRY_TOKEN` env var or `~/.cargo/credentials.toml`
- `gh auth status` for GitHub authentication

### Skill Structure
The skill should:
1. Use `TaskCreate` to track each phase
2. Use `AskUserQuestion` for the Phase 5→6 gate
3. Use `Bash` tool for all command execution
4. Parse command output to detect failures
5. Provide clear status updates after each phase

### Edge Cases
- **Concurrent releases**: Check if tag already exists before starting
- **Network failures**: Handled by Error Classification & Retry Policy in Phase 6 (auto-retry for transient, user decision for explicit errors)
- **Index lag**: Wait times between publishes may need adjustment (default 30s, extend to 60s on retry)
- **Cross-compilation failures**: Report which targets failed and which succeeded; allow partial release with available targets
- **First-time publish**: If crate has never been published, `cargo owner --list` will fail — detect this and handle as "new crate" flow
- **Workspace version desync**: If internal dependency `version` fields don't match `workspace.package.version`, Phase 2 must fix all of them atomically

## Success Criteria

A successful release means:
1. All pre-flight checks passed (including CI green)
2. Version bumped in workspace
3. CHANGELOG.md updated with categorized, linked entries
4. 6 platform binaries built, checksummed, and self-checked
5. GitHub Release published (not draft) with all assets
6. All workspace crates published to crates.io in correct dependency order
7. Each crate visible on crates.io at new version
8. `cargo install opi-coding-agent --version <version>` succeeds
9. `opi --version` outputs the correct version

## Future Enhancements

- Support for pre-release versions (alpha, beta, rc)
- Automated GitHub username mapping from git email
- Slack/Discord notification on successful release
- Rollback command for yanking published versions
- Support for release branches (not just main)
