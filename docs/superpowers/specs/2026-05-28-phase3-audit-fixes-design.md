# Phase 3 Audit Fixes

Date: 2026-05-28

## Scope

Five findings from post-audit review of phase 3 implementation. Four are targeted bug fixes; one is a small feature addition.

## Finding 1 (Critical): phase_exit.3 missing

**Problem**: `opi-impl-state.json` has `phase_exit` entries for phases 1 and 2 but not phase 3. The phase cannot formally close.

**Fix**: Add `phase_exit.3` entry to the ledger. All 13 tasks (3.1–3.13) are `passing` with completed secondary reviews from codex and claude-opus-4.7. The entry mirrors the phase 1/2 format: `completed_at`, `exit_criteria_met: true`, `evaluator_summary`, `snapshot_path`, `task_summary` list.

**Files**: `docs/snapshots/phase3/opi-impl-state.json`

## Finding 2 (High): Interactive TUI image attachment

### 2a: --image passthrough to interactive mode

**Problem**: `opi --image file.png` with no prompt args enters interactive mode. The `--image` flag is only read in `run_non_interactive()`. The interactive code path ignores `cli.image` entirely.

**Fix**: In `main.rs`, when entering interactive mode with `cli.image` non-empty, load images via `image::load_image_with_limit()` and store them on the `CodingHarness`. When the user submits their first prompt, inject the queued images alongside the text. After injection, clear the queue so subsequent prompts are text-only.

**Files**: `crates/opi-coding-agent/src/main.rs`, `crates/opi-coding-agent/src/harness.rs`, `crates/opi-coding-agent/src/interactive.rs`

### 2b: /image slash command

**Problem**: No way to attach images mid-conversation in the TUI.

**Fix**: Add a `/image <path>` slash command to the interactive TUI. The command loads the image via `image::load_image_with_limit()`, queues it on the harness, and confirms to the user. The next user message submission includes the queued image. After attachment, the queue is cleared.

**Files**: `crates/opi-coding-agent/src/interactive.rs`, `crates/opi-coding-agent/src/harness.rs`

## Finding 3 (High): iTerm2 image escape protocol

**Problem**: `terminal_image.rs:120` uses `;` to separate parameters from base64 data. The iTerm2 protocol requires `:` as the separator between key-value arguments and the base64 payload.

Current: `\x1b]1337;File=inline=1;width=100;<base64>\x07`
Correct: `\x1b]1337;File=inline=1;width=100:<base64>\x07`

**Fix**: Change the format string from `";{}"` to `":{}"`. Update the snapshot test that asserts the escape format.

**Files**: `crates/opi-tui/src/terminal_image.rs`, `crates/opi-tui/tests/terminal_image_rendering.rs`, snapshot files under `crates/opi-tui/tests/snapshots/`

## Finding 4 (Medium): Session picker non-ASCII panic

**Problem**: `picker.rs:48-49` truncates long paths with `&s.cwd[s.cwd.len() - 37..]`. Byte-slicing a UTF-8 `String` panics if the boundary falls inside a multi-byte character (e.g., CJK paths).

**Fix**: Replace byte-slice with char-aware truncation. Use `.floor_char_boundary()` on the byte offset to ensure the slice always falls on a char boundary. Alternative: truncate by char count directly.

```rust
let cwd_short = if s.cwd.len() > 40 {
    let start = s.cwd.floor_char_boundary(s.cwd.len() - 37);
    format!("...{}", &s.cwd[start..])
} else {
    s.cwd.clone()
};
```

**Files**: `crates/opi-coding-agent/src/picker.rs`

## Finding 5 (Medium): --list-models --json escaping

**Problem**: `main.rs:748-754` hand-writes JSON with `format!`. User-configured model IDs, provider names, or display names containing `"`, `\`, or control characters produce invalid JSON.

**Fix**: Replace the hand-written JSON with `serde_json` serialization (already a workspace dependency). Define a `ModelEntry` struct or use `serde_json::json!` to build the objects.

**Files**: `crates/opi-coding-agent/src/main.rs`

## Dependency order

These fixes are independent and can be implemented in any order. Suggested order by severity:

1. Finding 3 (one-character fix + snapshot update)
2. Finding 4 (one-line fix)
3. Finding 5 (small refactor)
4. Finding 1 (ledger update)
5. Finding 2a (small wiring change)
6. Finding 2b (new slash command)

## Testing

- Finding 3: Update existing snapshot test, add test for iTerm2 escape with multi-param
- Finding 4: Add test with CJK/emoji path strings
- Finding 5: Add test with model names containing quotes/backslashes
- Finding 2a: Add test that `--image` data survives into interactive harness first message
- Finding 2b: Add test for `/image` command parsing and queue behavior
- Finding 1: N/A (ledger metadata)
- After all fixes: `cargo test --workspace --all-targets` + `cargo clippy --workspace --all-targets -- -D warnings`
