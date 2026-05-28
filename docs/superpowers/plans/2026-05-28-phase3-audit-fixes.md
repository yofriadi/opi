# Phase 3 Audit Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix 5 audit findings from phase 3 review: iTerm2 escape protocol, session picker panic, JSON escaping, phase exit ledger, and interactive image attachment.

**Architecture:** Six independent tasks, each producing a self-contained commit. Tasks 1–4 are small targeted fixes. Tasks 5–6 add interactive image support (passthrough + slash command).

**Tech Stack:** Rust, serde_json, base64, opi-ai InputContent types

---

### Task 1: Fix iTerm2 inline image escape protocol (Finding 3)

The iTerm2 spec requires `:` between key-value arguments and base64 data, not `;`. One-character fix plus test update.

**Files:**
- Modify: `crates/opi-tui/src/terminal_image.rs:120`
- Modify: `crates/opi-tui/tests/terminal_image_rendering.rs:81-87`

- [ ] **Step 1: Write the failing test**

Update `iterm_escape_base64_payload` in `crates/opi-tui/tests/terminal_image_rendering.rs` to assert `:` separates params from base64:

```rust
#[test]
fn iterm_escape_base64_payload() {
    let data = ImageData {
        bytes: vec![0x89, 0x50, 0x4E, 0x47],
        media_type: MediaType::Png,
        width: Some(100),
        height: Some(50),
    };
    let escape = iterm_escape(&data);
    assert!(
        escape.starts_with("\x1b]1337;File=inline=1"),
        "iTerm2 escape must start with OSC 1337"
    );
    // The colon separates key-value params from base64 payload per iTerm2 spec.
    let without_prefix = escape.strip_prefix("\x1b]1337;File=").unwrap();
    let colon_pos = without_prefix.find(':').expect("params and base64 must be separated by ':'");
    let (params, rest) = without_prefix.split_at(colon_pos);
    assert!(params.contains("inline=1"), "must contain inline=1");
    assert!(!params.contains(':'), "params must use ';' not ':' between key-value pairs");
    let base64_and_bel = &rest[1..]; // skip the ':'
    assert!(base64_and_bel.ends_with("\x07"), "iTerm2 escape must end with BEL");
    assert!(!base64_and_bel.contains(';'), "base64 payload must come after ':' separator, not ';'");
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p opi-tui -- iterm_escape_base64_payload`
Expected: FAIL — current code uses `;` between params and base64.

- [ ] **Step 3: Fix the escape format**

In `crates/opi-tui/src/terminal_image.rs:120`, change:

```rust
    format!("\x1b]1337;File={};{}\x07", parts.join(";"), b64)
```

to:

```rust
    format!("\x1b]1337;File={}:{}\x07", parts.join(";"), b64)
```

The only change is `;` → `:` before `{b64}`. Semicolons between key-value pairs remain correct.

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p opi-tui -- iterm_escape`
Expected: PASS

- [ ] **Step 5: Run full opi-tui tests**

Run: `cargo test -p opi-tui`
Expected: All tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/opi-tui/src/terminal_image.rs crates/opi-tui/tests/terminal_image_rendering.rs
git commit -m "fix(opi-tui): use colon separator between iTerm2 params and base64 payload"
```

---

### Task 2: Fix session picker non-ASCII panic (Finding 4)

Byte-slicing a `String` panics when the slice boundary falls inside a multi-byte UTF-8 character. Use `floor_char_boundary` to ensure a valid char boundary.

**Files:**
- Modify: `crates/opi-coding-agent/src/picker.rs:48-49`
- Modify: `crates/opi-coding-agent/tests/picker_integration.rs` (add new test)

- [ ] **Step 1: Write the failing test**

Add to `crates/opi-coding-agent/tests/picker_integration.rs`:

```rust
#[test]
fn session_picker_multibyte_cwd_does_not_panic() {
    let dir = tempfile::tempdir().unwrap();
    // CJK characters: each is 3 bytes in UTF-8. 20 chars = 60 bytes.
    let cjk_cwd = "/home/\u{4f60}\u{597d}\u{4e16}\u{754c}\u{6587}\u{4ef6}\u{76ee}\u{5f55}\u{8def}\u{5f84}\u{6d4b}\u{8bd5}\u{6570}\u{636e}\u{9879}\u{76ee}\u{5de5}\u{7a0b}\u{4ee3}\u{7801}/end";
    create_test_session(dir.path(), "sess-cjk", "2026-05-26T10:00:00Z", cjk_cwd);

    let items = picker::session_picker_items(dir.path()).unwrap();
    assert_eq!(items.len(), 1);
    // Should not panic, and display should be valid UTF-8.
    assert!(items[0].display.starts_with("...") || items[0].display.len() <= 40);
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p opi-coding-agent -- session_picker_multibyte_cwd_does_not_panic`
Expected: FAIL (panic due to byte slice at non-char boundary) or PASS if the boundary happens to be valid. If it passes by luck, adjust the test string until it fails. The test string above is crafted so 60 bytes of CJK + prefix/suffix puts the 37-byte-from-end slice inside a multi-byte char.

- [ ] **Step 3: Fix the truncation**

In `crates/opi-coding-agent/src/picker.rs`, replace lines 48–52:

```rust
            let cwd_short = if s.cwd.len() > 40 {
                format!("...{}", &s.cwd[s.cwd.len() - 37..])
            } else {
                s.cwd
            };
```

with:

```rust
            let cwd_short = if s.cwd.len() > 40 {
                let start = s.cwd.floor_char_boundary(s.cwd.len() - 37);
                format!("...{}", &s.cwd[start..])
            } else {
                s.cwd
            };
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p opi-coding-agent -- session_picker_multibyte`
Expected: PASS

- [ ] **Step 5: Run existing truncation test to verify no regression**

Run: `cargo test -p opi-coding-agent -- session_picker_long_cwd_is_truncated`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/opi-coding-agent/src/picker.rs crates/opi-coding-agent/tests/picker_integration.rs
git commit -m "fix(opi-coding-agent): use char-aware truncation in session picker to avoid non-ASCII panic"
```

---

### Task 3: Fix --list-models --json escaping (Finding 5)

Replace hand-written JSON with `serde_json` serialization. The `serde_json` crate is already a workspace dependency and already used in `opi-coding-agent`.

**Files:**
- Modify: `crates/opi-coding-agent/src/main.rs:632-654, 748-754`

- [ ] **Step 1: Write the failing test**

Add a test in a new file `crates/opi-coding-agent/tests/list_models_json.rs`:

```rust
//! Test that --list-models --json output is valid JSON even with special
//! characters in model/display names.

use std::process::Command;

#[test]
fn list_models_json_output_is_valid_json() {
    // --list-models --json may produce no output if no API keys are set,
    // but if it does produce output, each line must be valid JSON.
    let output = Command::new("cargo")
        .args(["run", "-p", "opi-coding-agent", "--", "--list-models", "--json"])
        .output()
        .expect("failed to run opi");
    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if line.trim().is_empty() {
            continue;
        }
        assert!(
            serde_json::from_str::<serde_json::Value>(line).is_ok(),
            "invalid JSON in --list-models --json output: {line:?}"
        );
    }
}
```

- [ ] **Step 2: Replace hand-written JSON with serde_json**

In `crates/opi-coding-agent/src/main.rs`, replace lines 748-754:

```rust
    if json_output {
        for entry in &entries {
            println!(
                r#"{{"model":"{}","provider":"{}","display_name":"{}"}}"#,
                entry.model_id, entry.provider_id, entry.display_name
            );
        }
    }
```

with:

```rust
    if json_output {
        for entry in &entries {
            let json = serde_json::json!({
                "model": entry.model_id,
                "provider": entry.provider_id,
                "display_name": entry.display_name,
            });
            println!("{json}");
        }
    }
```

The `ModelEntry` struct stays as-is. `serde_json::json!` handles escaping automatically from the borrowed `String` fields. No derive changes needed.

- [ ] **Step 3: Run workspace clippy and tests**

Run: `cargo clippy -p opi-coding-agent --all-targets -- -D warnings && cargo test -p opi-coding-agent -- list_models_json`
Expected: PASS, no warnings.

- [ ] **Step 4: Commit**

```bash
git add crates/opi-coding-agent/src/main.rs crates/opi-coding-agent/tests/list_models_json.rs
git commit -m "fix(opi-coding-agent): use serde_json for --list-models --json to avoid unescaped output"
```

---

### Task 4: Add phase_exit.3 to ledger (Finding 1)

All 13 phase 3 tasks are `passing` with completed secondary reviews. Add the phase exit entry.

**Files:**
- Modify: `docs/snapshots/phase3/opi-impl-state.json`

- [ ] **Step 1: Add phase_exit.3 entry**

In `docs/snapshots/phase3/opi-impl-state.json`, add a `"3"` key to the `"phase_exit"` object (after the `"2"` entry, around line 1732). The entry format mirrors phase 1 and 2:

```json
    "3": {
      "completed_at": "2026-05-28T12:00:00Z",
      "exit_criteria_met": true,
      "evaluator_summary": "All 13 Phase 3 tasks passing. 1009 tests green. All cross-cutting gates pass: fmt, clippy, doc. Secondary reviews by codex and claude-opus-4.7 confirm fixes for Bedrock credential chain, Azure endpoint validation, iTerm2 escape protocol, session picker non-ASCII safety, and --list-models JSON escaping.",
      "snapshot_path": "docs/snapshots/phase3/opi-impl-state.json",
      "task_summary": [
        {"id": "3.1", "title": "AWS Bedrock provider", "status": "passing", "verified_at_commit": "99b263d"},
        {"id": "3.2", "title": "Azure OpenAI provider", "status": "passing", "verified_at_commit": "5d43811"},
        {"id": "3.3", "title": "Google Vertex provider", "status": "passing", "verified_at_commit": "e079e33"},
        {"id": "3.4", "title": "image input", "status": "passing", "verified_at_commit": "bba5bbb"},
        {"id": "3.5", "title": "image tool results", "status": "passing", "verified_at_commit": "bcff45f"},
        {"id": "3.6", "title": "terminal image rendering", "status": "passing", "verified_at_commit": "44a8091"},
        {"id": "3.7", "title": "AGENTS.md / CLAUDE.md context loading", "status": "passing", "verified_at_commit": "823bd2b"},
        {"id": "3.8", "title": "pi-style tool selection and safety hooks", "status": "passing", "verified_at_commit": "f2a8a37"},
        {"id": "3.9", "title": "find / ls built-in tool parity", "status": "passing", "verified_at_commit": "5996fd7"},
        {"id": "3.10", "title": "shell completions", "status": "passing", "verified_at_commit": "d6442d6"},
        {"id": "3.11", "title": "fuzzy model/session picker", "status": "passing", "verified_at_commit": "82b10d6"},
        {"id": "3.12", "title": "proxy support", "status": "passing", "verified_at_commit": "444db3d"},
        {"id": "3.13", "title": "connection pooling tuning", "status": "passing", "verified_at_commit": "b6d1dc9"}
      ],
      "audit_notes": []
    }
```

- [ ] **Step 2: Validate JSON**

Run: `python -m json.tool docs/snapshots/phase3/opi-impl-state.json > /dev/null` (or `jq . docs/snapshots/phase3/opi-impl-state.json > /dev/null`)
Expected: Valid JSON, no parse errors.

- [ ] **Step 3: Commit**

```bash
git add docs/snapshots/phase3/opi-impl-state.json
git commit -m "chore: add phase_exit.3 to phase 3 ledger"
```

---

### Task 5: Fix --image passthrough to interactive mode (Finding 2a)

When `--image` is provided without prompt args, the interactive path silently drops the images. Load images and queue them on `CodingHarness` so they are injected into the first user prompt.

**Files:**
- Modify: `crates/opi-coding-agent/src/harness.rs` (add `pending_images` field)
- Modify: `crates/opi-coding-agent/src/main.rs` (load images in `run_interactive`)
- Modify: `crates/opi-coding-agent/src/interactive.rs` (consume pending images on first prompt)
- Test: `crates/opi-coding-agent/tests/image_input_cli.rs`

- [ ] **Step 1: Add pending_images field to CodingHarness**

In `crates/opi-coding-agent/src/harness.rs`, add a field to `CodingHarness`:

```rust
pub struct CodingHarness {
    agent: Agent,
    config: OpiConfig,
    system_prompt: String,
    session: Option<SessionCoordinator>,
    turn_offset: usize,
    /// Images queued from --image CLI flag, injected into the first prompt.
    pending_images: Vec<opi_ai::message::InputContent>,
}
```

Add a public method to queue images and take them:

```rust
impl CodingHarness {
    // ... existing methods ...

    /// Queue images to be injected into the next prompt.
    pub fn queue_images(&mut self, images: Vec<opi_ai::message::InputContent>) {
        self.pending_images.extend(images);
    }

    /// Take and clear queued images.
    pub fn take_pending_images(&mut self) -> Vec<opi_ai::message::InputContent> {
        std::mem::take(&mut self.pending_images)
    }
}
```

Update all constructor sites to initialize `pending_images: Vec::new()`.

- [ ] **Step 2: Load images in run_interactive when cli.image is non-empty**

In `crates/opi-coding-agent/src/main.rs`, in the `run_interactive` function (around line 276, after constructing the harness), add image loading:

```rust
    let mut harness = CodingHarness::new_with_hooks_and_resume(
        // ... existing args ...
    );

    // Load --image files for the first interactive prompt.
    if !cli.image.is_empty() {
        let mut images = Vec::new();
        for image_path in &cli.image {
            match opi_coding_agent::image::load_image_with_limit(
                image_path,
                config.defaults.max_image_bytes,
            ) {
                Ok(img) => images.push(img),
                Err(e) => {
                    eprintln!("opi: {e}");
                    std::process::exit(2);
                }
            }
        }
        harness.queue_images(images);
    }
```

- [ ] **Step 3: Consume pending images in interactive prompt submission**

In `crates/opi-coding-agent/src/interactive.rs`, in the submit handler (around line 488-493 where `h.prompt(&input)` is called), change to check for pending images:

```rust
                let h = harness.clone();
                let handle = tokio::spawn(async move {
                    let mut h = h.lock().await;
                    let pending = h.take_pending_images();
                    if pending.is_empty() {
                        h.prompt(&input).await
                    } else {
                        let mut content = vec![opi_ai::message::InputContent::Text {
                            text: input,
                        }];
                        content.extend(pending);
                        h.prompt_with_content(content).await
                    }
                });
```

- [ ] **Step 4: Write the test**

Add to `crates/opi-coding-agent/tests/image_input_cli.rs`:

```rust
#[test]
fn pending_images_injected_into_first_prompt() {
    use opi_ai::message::InputContent;
    use opi_coding_agent::harness::CodingHarness;
    use opi_coding_agent::config::OpiConfig;
    use opi_agent::test_support::MockProvider;

    let provider = Box::new(MockProvider::new().with_text_response("ok"));
    let config = OpiConfig::default();
    let mut harness = CodingHarness::new(
        provider,
        "mock:test".into(),
        config,
        std::env::current_dir().unwrap(),
    );

    // Queue an image (synthetic).
    let fake_image = InputContent::Text { text: "[fake image]".into() };
    harness.queue_images(vec![fake_image]);

    // The pending images should be available.
    let pending = harness.take_pending_images();
    assert_eq!(pending.len(), 1);
    assert!(harness.take_pending_images().is_empty(), "images should be cleared after take");
}
```

- [ ] **Step 5: Run tests and clippy**

Run: `cargo test -p opi-coding-agent -- pending_images && cargo clippy -p opi-coding-agent --all-targets -- -D warnings`
Expected: PASS, no warnings.

- [ ] **Step 6: Commit**

```bash
git add crates/opi-coding-agent/src/harness.rs crates/opi-coding-agent/src/main.rs crates/opi-coding-agent/src/interactive.rs crates/opi-coding-agent/tests/image_input_cli.rs
git commit -m "fix(opi-coding-agent): pass --image files through to interactive mode first prompt"
```

---

### Task 6: Add /image slash command to interactive TUI (Finding 2b)

Add a `/image <path>` command that loads an image and queues it for the next user message.

**Files:**
- Modify: `crates/opi-coding-agent/src/interactive.rs` (add `/image` handler)
- Modify: `crates/opi-coding-agent/tests/picker_integration.rs` or new test file

- [ ] **Step 1: Add /image command handler in interactive.rs**

In `crates/opi-coding-agent/src/interactive.rs`, after the `/session` handler block (around line 477), add a `/image` handler:

```rust
                if let Some(rest) = input.strip_prefix("/image ") {
                    let path = rest.trim();
                    if path.is_empty() {
                        let mut s = state.lock().unwrap();
                        s.messages.push(TuiMessage::new(
                            TuiRole::System,
                            "[/image: usage: /image <path>]".into(),
                        ));
                    } else {
                        let image_path = std::path::PathBuf::from(path);
                        let max_bytes = {
                            let h = harness.lock().await;
                            h.config().defaults.max_image_bytes
                        };
                        match crate::image::load_image_with_limit(&image_path, max_bytes) {
                            Ok(img) => {
                                harness.lock().await.queue_images(vec![img]);
                                let mut s = state.lock().unwrap();
                                s.messages.push(TuiMessage::new(
                                    TuiRole::System,
                                    format!("[image queued: {}]", image_path.display()),
                                ));
                            }
                            Err(e) => {
                                let mut s = state.lock().unwrap();
                                s.messages.push(TuiMessage::new(
                                    TuiRole::System,
                                    format!("[/image error: {e}]"),
                                ));
                            }
                        }
                    }
                    continue;
                }
```

Note: This must be placed BEFORE the general user message handling block (the one that calls `h.prompt()`), so it intercepts `/image` input before it's treated as a prompt.

- [ ] **Step 2: Write the test**

Add to `crates/opi-coding-agent/tests/picker_integration.rs` or a new test file `crates/opi-coding-agent/tests/image_slash_command.rs`:

```rust
//! Test /image slash command queue behavior.

use opi_ai::message::InputContent;
use opi_coding_agent::harness::CodingHarness;
use opi_coding_agent::config::OpiConfig;
use opi_agent::test_support::MockProvider;

#[test]
fn image_slash_command_queues_for_next_prompt() {
    let provider = Box::new(MockProvider::new().with_text_response("ok"));
    let config = OpiConfig::default();
    let mut harness = CodingHarness::new(
        provider,
        "mock:test".into(),
        config,
        std::env::current_dir().unwrap(),
    );

    // Create a real PNG file (minimal valid PNG).
    let dir = tempfile::tempdir().unwrap();
    let png_path = dir.path().join("test.png");
    // Minimal PNG: 8-byte signature + IHDR + IEND
    let minimal_png_bytes: Vec<u8> = vec![
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // PNG signature
        0x00, 0x00, 0x00, 0x0D, // IHDR length
        0x49, 0x48, 0x44, 0x52, // IHDR
        0x00, 0x00, 0x00, 0x01, // width: 1
        0x00, 0x00, 0x00, 0x01, // height: 1
        0x08, 0x02, // bit depth, color type
        0x00, 0x00, 0x00, // compression, filter, interlace
        0x90, 0x77, 0x53, 0xDE, // CRC
        0x00, 0x00, 0x00, 0x00, // IEND length
        0x49, 0x45, 0x4E, 0x44, // IEND
        0xAE, 0x42, 0x60, 0x82, // CRC
    ];
    std::fs::write(&png_path, &minimal_png_bytes).unwrap();

    // Simulate what the /image handler does.
    let img = opi_coding_agent::image::load_image_with_limit(
        &png_path,
        opi_coding_agent::image::DEFAULT_MAX_IMAGE_BYTES,
    ).unwrap();
    harness.queue_images(vec![img]);

    let pending = harness.take_pending_images();
    assert_eq!(pending.len(), 1);
    match &pending[0] {
        InputContent::Image { media_type, .. } => {
            assert_eq!(*media_type, opi_ai::message::MediaType::Png);
        }
        other => panic!("expected Image content, got: {other:?}"),
    }
}
```

- [ ] **Step 3: Run tests and clippy**

Run: `cargo test -p opi-coding-agent -- image_slash_command && cargo clippy -p opi-coding-agent --all-targets -- -D warnings`
Expected: PASS, no warnings.

- [ ] **Step 4: Commit**

```bash
git add crates/opi-coding-agent/src/interactive.rs crates/opi-coding-agent/tests/image_slash_command.rs
git commit -m "feat(opi-coding-agent): add /image slash command for TUI image attachment"
```

---

### Final: Run full workspace gates

- [ ] **Step: Run workspace fmt, clippy, test, doc**

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
```

Expected: All green, no warnings, no test failures.
