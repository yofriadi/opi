# Phase 11 Tooling Quality -- Independent Code Audit (Opus 4.6)

## Audit Metadata

| Field | Value |
|-------|-------|
| Auditor model | Opus 4.6 |
| Date | 2026-06-30 |
| Commit range | `8816e7a..3ae3d40` (11 commits) |
| Base version | v0.6.2 (`0a3add8`) |
| Design spec | `docs/superpowers/specs/2026-06-24-phase11-tooling-quality-design.md` |
| Files audited | 63 files, +8591 / -803 lines |
| Audit method | Core source line-by-line, test logic review, documentation sync |
| Severity system | P1 (design/architecture), P2 (correctness/safety), P3 (test coverage), P4 (code quality) |
| Parallel agents | 5 explore agents (WS11.1-11.2, WS11.3-11.4, WS11.5-11.6, WS11.7, WS11.8-11.11) |
| Constraints | Independent review; no existing audit reports read or referenced |

## Audit Scope

Phase 11 covers 11 tasks (all `passing`):

| Task | Title | Crate |
|------|-------|-------|
| 11.1 | Tool result contract and path-metadata normalization | workspace |
| 11.2 | Filesystem error taxonomy and per-cause diagnostic codes | workspace |
| 11.3 | read tool hardening | opi-coding-agent |
| 11.4 | write tool hardening | opi-coding-agent |
| 11.5 | edit tool hardening | opi-coding-agent |
| 11.6 | bash tool hardening | opi-coding-agent |
| 11.7 | Read-only navigation tools (grep/find/ls/glob) consistency | opi-coding-agent |
| 11.8 | Agent diagnostics/trace integration, JSON/RPC shape, Phase 8 termination | workspace |
| 11.9 | Provider tool-result error propagation | opi-ai |
| 11.10 | Tooling policy documentation updates | opi-coding-agent |
| 11.11 | CLI help, docs guards, SC8 non-goal guards | opi-coding-agent |

## Findings (by severity)

### P1 - Design/Architecture

#### P1-1: `opi-spec.md` / `opi-spec.zh.md` not updated for Phase 11

- **File**: `docs/opi-spec.md`, `docs/opi-spec.zh.md`
- **Evidence**: `git diff 0a3add8..3ae3d40 -- docs/opi-spec.md` returns empty; `git diff 0a3add8..3ae3d40 -- docs/opi-spec.zh.md` returns empty
- **Impact**: The normative spec does not document `ToolResult.truncated`, `ToolResult.diagnostics`, `ToolDiagnostic`, `FsToolError` taxonomy, `MaxTurnsExceeded`, per-provider `is_error` wire semantics, the path-metadata contract, the bash operation-metadata key set, or the `AgentEvent::ToolExecutionEnd.diagnostics` field. Section 8.4 still describes `edit` as "exact string replacement or structured patch" without the Phase 11 uniqueness requirement, guardrails, or conflict diagnostics.
- **Cause**: Phase 11 implementation focused on README and guard tests but did not propagate changes to the normative spec.
- **Suggested fix**: Add a subsection to opi-spec.md covering the ToolResult contract (field set, builder requirement), the filesystem error taxonomy, per-provider is_error wire semantics, and the MaxTurnsExceeded termination condition. Update section 8.4 tool descriptions to reflect Phase 11 hardening. Sync `opi-spec.zh.md`.

#### P1-2: `TOOL_ERROR_MARKER` duplicated between two provider files

- **File**: `crates/opi-ai/src/openai_chat.rs` L1076-1082, `crates/opi-ai/src/openai_responses.rs` L22-28
- **Evidence**: `const TOOL_ERROR_MARKER: &str = "[tool_error] ";` declared identically in both files with a comment "Duplicated verbatim ... `tool_result_wire.rs` pins the two byte-identical so future drift is caught."
- **Impact**: A maintenance risk: a change in one file that forgets the other is only caught at test time, not compile time. The value is the wire-visible failure marker, so drift could cause cross-provider inconsistency.
- **Cause**: Deliberate duplication with test-based guard rather than a shared constant.
- **Suggested fix**: Extract `TOOL_ERROR_MARKER` to a shared location in `opi-ai` (e.g., `message.rs` or a new `wire.rs` module) and import it in both providers. Keep the test as a regression gate.

### P2 - Correctness/Safety

#### P2-1: read normalizes CRLF to LF in output, breaking read-then-edit round-trip

- **File**: `crates/opi-coding-agent/src/tool/read.rs` L165, L180
- **Evidence**: `content.lines().collect()` strips `\r\n` and `\r` line terminators; `selected.join("\n")` re-joins with LF only. A CRLF file read through this tool produces LF-normalized text. If a model then constructs an `edit` `old_string` from this normalized output, the exact match will fail on the CRLF file because the disk content has `\r\n` but the model's string has `\n`.
- **Impact**: The read-then-edit workflow is broken on CRLF files. The edit tool correctly preserves CRLF byte-for-byte (confirmed by tests), but the read output does not reflect the actual byte content. This undermines the Phase 11 design goal of "predictable CRLF/LF handling."
- **Cause**: `str::lines()` is a convenience that normalizes line endings.
- **Suggested fix**: Either (a) report `line_ending` metadata in details so models and callers know the file uses CRLF, or (b) use a split that preserves `\r\n` in the output, or (c) document that read provides a normalized view and edit must account for the difference. Option (a) is the minimum viable fix.

#### P2-2: NDJSON/RPC event path does not redact `ToolExecutionEnd.diagnostics`

- **File**: `crates/opi-agent/src/event.rs` L63-69, `crates/opi-agent/src/sdk.rs`
- **Evidence**: The `ToolExecutionEnd.diagnostics` field comment says "event-path redaction is deferred to a wire-format task." The `agent_event_to_value()` serializer does not apply `RedactionMode::Summary`. Tool diagnostics carry `context` with absolute paths (`user_path`, `resolved_path`, `command`).
- **Impact**: JSON/RPC consumers, logs, and session persistence may expose workspace paths or command context. This is inconsistent with the Phase 7 diagnostic redaction strategy applied to the DiagnosticSink path.
- **Cause**: Deferred work item explicitly noted in the code comment.
- **Suggested fix**: Apply `RedactionMode::Summary` to `ToolExecutionEnd.diagnostics` at the RPC/NDJSON emission boundary, matching the `observe()` redaction path.

#### P2-3: README incorrectly describes edit tool behavior

- **File**: `crates/opi-coding-agent/README.md` L123, `crates/opi-coding-agent/README.zh.md` L119
- **Evidence**: README says "Replaces the first exact match" / "替换第一个精确匹配", but the implementation (edit.rs L264-286) REFUSES when multiple matches exist. The ToolDef description correctly says "Replace a unique exact string in a file." (edit.rs L74).
- **Impact**: Users reading the README expect first-match-wins behavior; the actual behavior rejects non-unique matches. A model relying on this description may supply an old_string that matches multiple times, expecting the first occurrence to be replaced, and get an error instead.
- **Cause**: The README description was not updated when the Phase 11 uniqueness enforcement was added.
- **Suggested fix**: Change to "Replaces the unique exact match" / "替换唯一精确匹配" in both READMEs.

#### P2-4: bash `WaitFailed` branch lacks operation metadata

- **File**: `crates/opi-coding-agent/src/tool/bash.rs` L227-232
- **Evidence**: The `Control::WaitFailed` branch returns a bare `result::err(...)` without calling `bash_operation_metadata`. All other branches (Done, TimedOut, Cancelled) provide the full stable key set (`command`, `cwd`, `shell`, `exit_code`, `timed_out`, `cancelled`, `truncated`).
- **Impact**: Violates the Phase 11.1 stable operation-metadata contract. RPC/NDJSON consumers expecting the key set on every bash result will get `details: None` on wait-failed.
- **Cause**: The WaitFailed branch was not updated when the metadata contract was standardized.
- **Suggested fix**: Construct `bash_operation_metadata(workspace_root, command, cwd, shell, None, false, false, false, None)` and pass through `bash_result` like the other branches.

#### P2-5: bash `StreamCapture::append` error silently breaks drain loop

- **File**: `crates/opi-coding-agent/src/tool/bash.rs` L128-130, L143-145
- **Evidence**: When `out_cap.append()` returns `Err` (e.g., disk full during spill), the drain loop `break`s, stopping all further reads from that pipe. The child process continues writing, and if its output exceeds the pipe buffer, it will block until timeout.
- **Impact**: Disk-full or temp-directory-exhausted scenarios cause the bash tool to degrade to timeout behavior with incomplete/inaccurate truncation metadata.
- **Cause**: The drain loop treats spill IO errors as terminal.
- **Suggested fix**: On append error, continue reading (and discarding) the pipe to prevent child deadlock; push a diagnostic noting the spill failure.

#### P2-6: edit.rs clones entire file content for diff preview

- **File**: `crates/opi-coding-agent/src/tool/edit.rs` L289
- **Evidence**: `let before = content.clone();` clones the entire file string (up to 1 MiB per `MAX_EDIT_FILE_BYTES`) solely for the `truncate_preview` call at L337. The preview is then capped at 64 KiB (`MAX_PREVIEW_BYTES`).
- **Impact**: Temporary memory duplication of up to 1 MiB per edit operation. Not a correctness bug but unnecessary allocation pressure.
- **Cause**: The clone is needed because `content` is consumed by `replacen`, so the original is needed for the "before" preview.
- **Suggested fix**: Compute `before_preview` by calling `truncate_preview(&content)` BEFORE `replacen`, storing only the truncated preview string (max 64 KiB) instead of the full clone. Then apply `replacen` on the original `content`.

#### P2-7: bash merged-preview allocation is 2x the cap

- **File**: `crates/opi-coding-agent/src/tool/bash.rs` L241-246
- **Evidence**: `out_cap.preview` and `err_cap.preview` can each be up to `MAX_BASH_OUTPUT_BYTES` (64 KiB). The merged vector allocates up to 128 KiB before being re-capped to 64 KiB at L245-246.
- **Impact**: Temporary allocation of 2x the cap (128 KiB) per bash execution that produces output on both streams. Not a correctness bug but avoidable.
- **Cause**: The merged preview is constructed by concatenating both full previews, then truncating.
- **Suggested fix**: Track remaining cap budget during merge and stop copying from `err_cap.preview` once the 64 KiB budget is exhausted.

### P3 - Test Coverage Gaps

#### P3-1: grep does not track or report non-UTF-8 filenames

- **File**: `crates/opi-coding-agent/src/tool/grep.rs` L108-112
- **Evidence**: grep uses `to_string_lossy()` for path formatting, which silently replaces non-UTF-8 bytes with U+FFFD. In contrast, find.rs L142-147, glob.rs L94-98, and ls.rs L145-151 all check `relative.to_str()` and track `non_utf8` count, emitting a `FsToolError::UnsupportedEncoding` diagnostic.
- **Impact**: grep silently produces U+FFFD-contaminated path strings instead of skipping and reporting the count. Cross-tool consistency gap: find/glob/ls all report non-UTF-8 entries; grep does not.
- **Cause**: grep was not updated to match the find/glob/ls pattern.
- **Suggested fix**: Add a `non_utf8` counter to grep, check `relative.to_str()` instead of `to_string_lossy()`, and push `FsToolError::UnsupportedEncoding` when non-zero (matching find/glob/ls exactly).

#### P3-2: No cancellation test for grep/find/glob/ls

- **File**: `crates/opi-coding-agent/tests/tools_glob_grep.rs`, `find_tool.rs`, `ls_tool.rs`
- **Evidence**: All four nav tools implement cooperative cancellation via `signal.is_cancelled()`, but no test verifies the cancellation path produces partial results and sets `cancelled: true` in details.
- **Impact**: The cancellation code paths are untested; a regression could silently break cooperative cancel.
- **Suggested fix**: Add a parametric cancellation test that cancels mid-walk and asserts `cancelled: true` in details and partial results.

#### P3-3: read/write/edit do not honor CancellationToken

- **File**: `crates/opi-coding-agent/src/tool/read.rs` L75 (`_signal`), `write.rs` L57 (`_signal`), `edit.rs` L83 (`_signal`)
- **Evidence**: All three tools accept a `CancellationToken` but ignore it (parameter named `_signal`). Large file reads (up to `DEFAULT_READ_LINES` = 2000 lines) and atomic writes (up to 1 MiB) are not cancellable.
- **Impact**: Minor for typical file sizes but inconsistent with bash/grep/find/glob/ls which all honor the token.
- **Cause**: Single file I/O operations are fast enough that cancellation was not deemed necessary.
- **Suggested fix**: Document the intentional non-cancellation. Optionally, add a cancellation check between the read and the response assembly for read, and between the temp write and rename for write/edit.

#### P3-4: No test for read tool CRLF behavior

- **File**: `crates/opi-coding-agent/tests/tools_read_write_edit_bash.rs`
- **Evidence**: Write has `write_tool_preserves_line_endings`, edit has `edit_crlf_preservation`, but read has no CRLF test. Given P2-1 (read normalizes CRLF to LF), the absence of a test means the normalization behavior is unspecified and unguarded.
- **Impact**: If read is changed to preserve CRLF, the change would not be caught by tests. If read is intended to normalize, the behavior is undocumented.
- **Suggested fix**: Add a test that creates a CRLF file, reads it, and asserts the expected behavior (either normalized LF output or preserved CRLF), documenting the design choice.

#### P3-5: No test for edit on file with CRLF line endings

- **File**: `crates/opi-coding-agent/tests/tools_read_write_edit_bash.rs`
- **Evidence**: grep for "crlf" or "\\r\\n" in the test file shows no edit-specific CRLF test. The edit tool doc (edit.rs L47-49) claims "CRLF/LF and final-newline state of the file are preserved byte-for-byte", but this is untested.
- **Impact**: The CRLF preservation claim is unverified by tests; a regression could silently corrupt line endings.
- **Suggested fix**: Add a test that creates a file with CRLF line endings, edits a substring, and asserts the output file preserves CRLF byte-for-byte.

#### P3-6: `MaxTurnsExceeded` trace mirror not tested

- **File**: `crates/opi-agent/tests/diagnostics_runtime.rs`
- **Evidence**: `max_turns_exhaustion_emits_warning_diagnostic_and_error` passes `trace: None`. The implementation calls `observe()` which writes to both DiagnosticSink and trace, but the trace integration is untested.
- **Impact**: The CHANGELOG claims "warning diagnostic + trace" but the trace half is unverified.
- **Suggested fix**: Extend the test to mount a `RecordingTraceSink` and assert a `DiagnosticLinked` entry with `CODE_AGENT_MAX_TURNS_EXCEEDED`.

#### P3-7: `resolve_tool_code` has no unit test

- **File**: `crates/opi-agent/src/diagnostic.rs` L541-555
- **Evidence**: The function bridges dynamic `ToolDiagnostic.code` strings to `&'static str` identifiers and is critical to the 11.8 lift, but has no test.
- **Impact**: A mismatched code string or a missing match arm would not be caught.
- **Suggested fix**: Add a table-driven test covering all 8 filesystem codes, the generic `tool_execution_failed`, and an unknown/forward-compat code.

### P4 - Code Quality

#### P4-1: `WorkspaceRelation::Unresolved` is defined but never populated

- **File**: `crates/opi-agent/src/tool/result.rs` L105-107
- **Evidence**: The `Unresolved` variant has a comment "Reserved: not populated by 11.1 tools (`resolve_tool_path` returns `Err` instead); 11.2 may relax that." No tool in Phase 11 constructs this variant.
- **Impact**: Dead code. The variant is serializable (`serde(rename_all = "snake_case")`) so it occupies space in the enum, but it is never constructed. Not harmful but may confuse readers.
- **Cause**: Forward-compatible reservation.
- **Suggested fix**: Leave as-is if Phase 12+ plans to use it; otherwise remove and add when needed.

#### P4-2: `nav_walk_builder` adds `.gitignore` as both git-ignore and custom-ignore

- **File**: `crates/opi-coding-agent/src/tool/mod.rs` L161-168
- **Evidence**: `git_ignore(true)` already enables `.gitignore` processing; `add_custom_ignore_filename(".gitignore")` adds it again as a custom ignore file. Both apply `.gitignore` patterns, causing redundant processing in git repos.
- **Impact**: No functional impact (ignore is idempotent). In non-git directories, the custom-ignore mechanism ensures `.gitignore` files are still honored — this may be the intended purpose.
- **Cause**: Likely intentional to handle non-git workspaces with `.gitignore` files.
- **Suggested fix**: Add a comment explaining the dual registration purpose. If non-git `.gitignore` support is not needed, remove the `add_custom_ignore_filename` call.

#### P4-3: `clippy::too_many_arguments` allow on `bash_result`

- **File**: `crates/opi-coding-agent/src/tool/bash.rs` L307
- **Evidence**: `#[allow(clippy::too_many_arguments)]` on `bash_result` (8 parameters) and on `bash_operation_metadata` in `result.rs` L70.
- **Impact**: Code readability. The parameters thread the four failure discriminators alongside the result builder inputs.
- **Cause**: The function assembles a result from multiple independent dimensions (content, details, is_error, truncated, command, exit_code, cancelled, timed_out).
- **Suggested fix**: Consider a `BashOutcome` enum (`Success`, `NonZeroExit(i32)`, `TimedOut`, `Cancelled`) to collapse the four boolean-like discriminators. Low priority.

#### P4-4: `edit_semantic_error` uses generic `CODE_TOOL_EXECUTION_FAILED`

- **File**: `crates/opi-coding-agent/src/tool/edit.rs` L372-382
- **Evidence**: All edit-semantic errors (not-found, multiple-match, empty old_string, no-op, oversized) use the same generic `CODE_TOOL_EXECUTION_FAILED` code. These causes are distinct from filesystem errors and from each other.
- **Impact**: Headless consumers cannot programmatically distinguish between "old_string not found" and "old_string not unique" via the diagnostic code alone — they must parse the message string.
- **Cause**: The comment (L362-371) explains this is intentional: "edit-semantic causes (the file itself is fine), so they do not map to an `FsToolError` variant."
- **Suggested fix**: If programmatic disambiguation is desired in the future, add edit-specific codes like `CODE_TOOL_EDIT_NOT_FOUND`, `CODE_TOOL_EDIT_NOT_UNIQUE`. Low priority given the intentional design.

#### P4-5: `opi-spec.md` / `opi-spec.zh.md` `ToolExecutionEnd` struct outdated

- **File**: `docs/opi-spec.md` L499, `docs/opi-spec.zh.md` L447
- **Evidence**: Spec still shows `ToolExecutionEnd { ..., is_error: bool }`. Implementation has additional fields: `details`, `truncated`, `diagnostics`.
- **Impact**: Embedders/SDK developers following the spec will produce or consume wrong wire shapes.
- **Suggested fix**: Update section 7.4 enum definition with current fields and note the provider-facing vs agent-facing field split.

#### P4-6: `opi-spec.zh.md` section 8.4 not synced with English version

- **File**: `docs/opi-spec.zh.md` L671-673
- **Evidence**: English 8.4 describes interactive/non-interactive default tool sets, `--allow-mutating`, and "permission popups are not core." Chinese version uses older phrasing without `--allow-mutating` or non-goal text.
- **Impact**: Chinese readers may misunderstand tool policy. Violates CLAUDE.md documentation sync requirement.
- **Suggested fix**: Align `opi-spec.zh.md` 8.4 with English 8.4 and Phase 11 README updates.

#### P4-7: Phase 11 still marked "planned" in spec roadmap

- **File**: `docs/opi-spec.md` L1454-1461, `docs/opi-spec.zh.md` corresponding section
- **Evidence**: Phase 11 status is `planned` despite all 11 tasks being implemented and passing.
- **Impact**: Misleading roadmap information.
- **Suggested fix**: Update to `completed` with list of delivered sub-items.

#### P4-8: `resolve_tool_code` folds unknown codes into generic bucket

- **File**: `crates/opi-agent/src/diagnostic.rs` L541-555
- **Evidence**: Unrecognized `ToolDiagnostic.code` strings are mapped to `CODE_TOOL_EXECUTION_FAILED`. Extension-contributed codes or future taxonomy additions lose granularity.
- **Impact**: Low; current 8 codes are exhaustive for built-in tools. Becomes relevant with extension tool diagnostics.
- **Suggested fix**: Preserve raw code in `Diagnostic.details` context when folding to generic.

## Per-Task Assessment

### 11.1: Tool result contract and path-metadata normalization

**Files**: `crates/opi-agent/src/tool.rs`, `crates/opi-agent/src/tool/result.rs`

The ToolResult struct gains `truncated: bool` and `diagnostics: Vec<ToolDiagnostic>`. The `result::ok` and `result::err` builders provide a consistent construction API. `path_metadata` and `bash_operation_metadata` define uniform metadata shapes. All 8 built-in tools use the builders exclusively (no hand-written `ToolResult { ... }` literals outside the builder). `ToolResultMessage` in `opi-ai/src/message.rs` carries `truncated` with `#[serde(default)]`.

Assessment: **Pass**. The contract is clean and consistently adopted. Note: `after_tool_call` Replace can silently drop `truncated`/`diagnostics` by constructing a new `ToolResult` — acceptable since hooks are extension points with known trade-offs.

### 11.2: Filesystem error taxonomy and per-cause diagnostic codes

**Files**: `crates/opi-agent/src/diagnostic.rs`

`FsToolError` defines 8 variants (NotFound, NotAFile, NotADirectory, PermissionDenied, BinaryFile, UnsupportedEncoding, OutsideWorkspace, UnresolvedWorkspaceRoot), each mapped to a distinct `CODE_TOOL_*` constant. `resolve_tool_code` bridges dynamic `ToolDiagnostic.code` strings to `&'static str` identifiers. Classification bridges exist for `ProviderError` and `AgentError` including the new `MaxTurnsExceeded` variant.

Assessment: **Pass**. The taxonomy is comprehensive, non-overlapping, and well-tested.

### 11.3: read tool hardening

**File**: `crates/opi-coding-agent/src/tool/read.rs`

Line-range behavior is correct: 1-based offset with floor-to-1, take_n from `limit` or `DEFAULT_READ_LINES`, explicit `limit` honored exactly (no re-cap). Binary detection via NUL byte before UTF-8 check. Path metadata via `result::path_metadata`. Details include `line_count`, `offset`, `limit`, `truncated`, `omitted`. Tests cover: default truncation, explicit limit, limit 0, offset past end, binary file, non-UTF-8, directory, not-found, permission-denied, outside-workspace.

Assessment: **Conditional pass**. Minor: does not honor `CancellationToken` (P3-3). P2-1 (CRLF normalization) is a material correctness issue for the read-then-edit workflow; P3-4 (no CRLF test).

### 11.4: write tool hardening

**File**: `crates/opi-coding-agent/src/tool/write.rs`

Create-vs-overwrite reported via `action` field. Pre-write existence probe + bytes_before. NUL rejection before any filesystem side effect. Parent-dir creation with `first_file_ancestor` for deterministic NotADirectory classification. Atomic write via temp-file + rename with best-effort cleanup. Audit details include `action`, `bytes_written`, `bytes_before`, `size_delta`.

Assessment: **Pass**. Clean atomic-write implementation. Minor: does not honor `CancellationToken` (P3-3).

### 11.5: edit tool hardening

**File**: `crates/opi-coding-agent/src/tool/edit.rs`

Uniqueness enforcement replaces the prior first-match behavior. Empty old_string and old==new are rejected. `MAX_EDIT_FILE_BYTES` (1 MiB) guardrail with pre-read stat. Binary/encoding checks match read.rs. Multiple-match refusal with `sample_offsets`. Diff preview with `MAX_PREVIEW_BYTES` (64 KiB) and `before_truncated`/`after_truncated` flags. Atomic write matches write.rs pattern.

Findings: P2-3 (README incorrectly describes behavior), P2-6 (unnecessary full clone), P3-5 (no CRLF test), P4-4 (generic diagnostic code).

Assessment: **Conditional pass** pending P2-3 README fix.

### 11.6: bash tool hardening

**File**: `crates/opi-coding-agent/src/tool/bash.rs`

Concurrent stdout/stderr drain via `tokio::join!` (three-way: drain_out, drain_err, control). `StreamCapture` bounds memory with spill-to-disk for overflow. Merged preview capped at `MAX_BASH_OUTPUT_BYTES` (64 KiB). Full output merged to one temp file. Operation metadata via `result::bash_operation_metadata` with stable key set. Environment policy via `with_env_policy` (no values dumped). `bash_operation_diagnostic` for error results.

`StreamCapture` is well-tested with 6 unit tests covering small stream, single-huge-chunk overflow, mid-chunk overflow, exact-boundary, cap+1, and exact-fit-then-overflow.

Findings: P2-4 (WaitFailed lacks metadata), P2-5 (StreamCapture append error breaks drain), P2-7 (2x cap allocation).

Assessment: **Conditional pass** pending P2-4 metadata contract fix.

### 11.7: Read-only navigation tools consistency

**Files**: `grep.rs`, `find.rs`, `ls.rs`, `glob.rs`, `mod.rs`

All four tools use `nav_walk_builder` for shared ignore/hidden/symlink config. All sort results lexicographically. grep/find/glob use `cap_nav_results` for the shared result-count cap (`MAX_NAV_RESULTS = 200`); ls uses its own `max_entries` with a matching default. All implement cooperative cancellation via `signal.is_cancelled()`. find/glob/ls track and report non-UTF-8 entries.

Finding: P3-1 (grep uses `to_string_lossy` instead of `to_str` for paths, inconsistent with find/glob/ls).

Assessment: **Conditional pass** pending P3-1 fix for full cross-tool consistency.

### 11.8: Agent diagnostics/trace integration, JSON/RPC shape, Phase 8 termination

**File**: `crates/opi-agent/src/agent_loop.rs`, `crates/opi-agent/src/event.rs`

The agent loop lifts `ToolResult.diagnostics` into Phase 7 Diagnostics via `tool_owned_diagnostic`. `AgentEvent::ToolExecutionEnd` gains `truncated` and `diagnostics` fields (additive, `skip_serializing_if` empty, `#[serde(default)]`). `MaxTurnsExceeded` fires when the for-loop exhausts `config.max_turns` with `has_tools_pending == true`, emitting a warning diagnostic + trace before returning `Err`.

The `has_tools_pending` initialization was changed from uninitialized to `false` (L62), which is correct: the variable is set inside the loop but is read AFTER the loop exits, so the initial value matters for zero-turn runs.

Assessment: **Pass**. Clean integration.

### 11.9: Provider tool-result error propagation

**Files**: `crates/opi-ai/src/anthropic.rs`, `openai_chat.rs`, `openai_responses.rs`, `gemini.rs`

Each provider now propagates `is_error`:
- Anthropic: native `is_error: true` on the `tool_result` content block.
- OpenAI Chat / Azure / OpenRouter / Mistral: `[tool_error] ` prefix marker.
- OpenAI Responses: same `[tool_error] ` prefix marker.
- Gemini / Vertex: `error: true` inside `functionResponse.response`.
- Bedrock: already used native `toolResult.status` (pre-Phase 11).

Success bodies are byte-identical to pre-fix shape on every provider (only the failure case adds the signal). Tests in `tool_result_wire.rs` (5 providers, success + failure) and `tool_result_no_leak.rs` (4 providers, truncated field) provide strong coverage.

Findings: P1-2 (duplicated `TOOL_ERROR_MARKER`).

Assessment: **Pass**. All providers covered with good test coverage.

### 11.10: Tooling policy documentation updates

**Files**: `crates/opi-coding-agent/README.md`, `README.zh.md`

Both READMEs gain comprehensive sections on tool policy, bash execution, output truncation, non-goals, flag precedence, and exit codes. The English and Chinese versions are structurally parallel. Both carry all 9 Phase 11 non-goals.

Finding: P2-3 (edit tool description inaccuracy in both READMEs).

Assessment: **Conditional pass** pending P2-3 fix.

### 11.11: CLI help, docs guards, SC8 non-goal guards

**File**: `crates/opi-coding-agent/tests/phase11_tooling_quality_docs.rs`

`policy_docs_and_help_stay_in_sync` cross-checks README, README.zh, opi-spec section 8.4, and CLI help against `policy.rs` for tool classification, flag precedence, allow_mutating, bash policy, truncation, and permission-prompt rationale. `sc8_non_goals_not_in_core` pins all 9 non-goals in both READMEs and verifies SC8 structural positives (8 built-in tools, no workflow tool names registered, policy-level gating, foreground child await).

Note: guard does not cover `opi-spec.zh.md`, meaning Chinese spec drift is undetected by CI.

Assessment: **Pass**. Strong structural guards, with minor coverage gap on zh spec.

## Cross-Workstream Consistency Checks

### ToolResult builder consistency

All 8 built-in tools construct results exclusively through `result::ok` and `result::err` (or `fs_error_result` / `edit_semantic_error` / `bash_result` which delegate to the builders). No hand-written `ToolResult { ... }` literals exist in tool source files.

The 5 `ToolResult` literals in `agent_loop.rs` (malformed-args, cancelled, unknown-tool, hook-skipped results) correctly include the new `truncated: false` and `diagnostics: vec![]` fields.

**Result**: Consistent.

### Path-metadata consistency

- `read`, `write`, `edit`: use `result::path_metadata` (uniform 4-key shape: `workspace_root`, `path`, `resolved_path`, `workspace_relation`).
- `bash`: uses `result::bash_operation_metadata` (operation shape: `workspace_root`, `command`, `cwd`, `shell`, `exit_code`, `timed_out`, `cancelled`, `truncated`).
- `grep`, `glob`: hardcode `workspace_relation: WorkspaceRelation::Inside` (correct: always walk workspace root).
- `find`, `ls`: resolve `workspace_relation` dynamically via `resolve_tool_path` (correct: support scope/target paths).

**Result**: Consistent with documented semantics.

### Non-UTF-8 path handling

- `find`, `glob`, `ls`: check `to_str()`, skip entries, track `non_utf8` count, push `FsToolError::UnsupportedEncoding` diagnostic.
- `grep`: uses `to_string_lossy()`, does NOT track or report non-UTF-8 entries.

**Result**: Inconsistent (P3-1).

### CancellationToken usage

- `bash`, `grep`, `find`, `ls`, `glob`: honor `signal` (via `is_cancelled()` or `signal.cancelled()`).
- `read`, `write`, `edit`: ignore `signal` (`_signal`).

**Result**: Documented difference (file I/O tools considered fast enough). Acceptable but inconsistent (P3-3).

### Windows path and CRLF handling

- `strip_verbatim_prefix` removes `\\?\` from canonical paths on Windows (mod.rs L113-123). Tested.
- `paths_diverge_indicating_traversal` uses case-insensitive comparison on Windows (mod.rs L133-134). Tested.
- `read` tool: reads as raw bytes, line-splits with `.lines()` which strips `\r\n` and `\r`. **Does NOT round-trip CRLF** — output is LF-normalized (P2-1).
- `write` tool: writes `content` bytes verbatim (Rust binary mode). CRLF preserved.
- `edit` tool: reads as UTF-8 string, applies `replacen`, writes bytes verbatim. CRLF preserved in theory but untested (P3-4).
- `bash` tool: selects `cmd /C` on Windows, `sh -c` on Unix. Tested structurally in docs guard.

**Result**: Good coverage. Edit CRLF preservation is claimed but untested.

## Success Criteria Verification

### SC1: Built-in tool result details follow a consistent contract

**Status**: Met. All 8 tools use builders. ToolResult carries truncated/diagnostics. Path-metadata and operation-metadata shapes are uniform. Parametric contract tests exist.

### SC2: edit handles CRLF/LF and conflict cases predictably

**Status**: Partially met. edit preserves CRLF (binary-mode read/write) but is not tested with CRLF files. Conflict cases (not-found, multiple-match) are well-tested. However, read normalizes CRLF to LF (P2-1), breaking the read-then-edit round-trip on CRLF files.

### SC3: write reports create/overwrite behavior and avoids silent partial writes

**Status**: Met. `action` field distinguishes "created" vs "overwritten". `bytes_before`/`size_delta` on overwrite. Atomic write via temp+rename. NUL rejection before any side effect.

### SC4: bash timeout, cancellation, cwd, env, exit code, and truncation documented and tested

**Status**: Met. All behaviors documented in README. Operation metadata has stable key set. StreamCapture bounds memory. Tests cover timeout, cancellation, exit codes, truncation, and env policy.

### SC5: Read-only nav tools have consistent ignore, sorting, limit, and error behavior

**Status**: Mostly met. All four use `nav_walk_builder`. All sort lexicographically. All use caps. grep is inconsistent on non-UTF-8 path handling (P3-1).

### SC6: Tool diagnostics integrate with Phase 7 traces

**Status**: Partially met. `tool_owned_diagnostic` lifts each `ToolDiagnostic` into a `Diagnostic` at the agent-loop boundary. `resolve_tool_code` bridges dynamic codes to static identifiers. `observe()` routes to both diagnostic sink and trace. However, the NDJSON/RPC event path does not redact diagnostics (P2-2), and the trace emit only carries code/severity, not structured context.

### SC7: Tool scheduling respects Phase 8 runtime contracts

**Status**: Met. `MaxTurnsExceeded` now fires correctly. The agent loop's tool execution, hook invocation, and terminate-flag handling are unchanged from Phase 8.

### SC8: No permission popup, background bash, remote execution, sandbox, or workflow tool added to core

**Status**: Met. Guard test `sc8_non_goals_not_in_core` pins all 9 non-goals. Structural positives verify 8 built-in tools, no workflow names registered, policy-level gating (not popup), and foreground child await (not background).

## Non-Goal Compliance

All 9 design non-goals are verified absent from core:

| Non-Goal | Status |
|----------|--------|
| Built-in permission popup | Absent (policy-level `--allow-mutating` check) |
| Persistent background bash | Absent (foreground `child.wait()` per call) |
| Remote execution | Absent |
| IDE project index | Absent |
| Language server integration | Absent |
| Automatic formatting on write/edit | Absent |
| Package ecosystem expansion | Absent |
| Workflow tools (todo, plan, sub-agents) | Absent |
| Sandbox implementation | Absent |

## CHANGELOG Verification

The Phase 11 CHANGELOG entries under `[Unreleased]` have three sections:

- **Added**: Describes the diagnostic lift (tool-owned `ToolDiagnostic` -> Phase 7 `Diagnostic`) and `AgentEvent::ToolExecutionEnd.diagnostics`. Accurate.
- **Changed**: Describes `MaxTurnsExceeded` behavior change (exit `1` instead of silent `Ok`). Accurate.
- **Fixed**: Describes per-provider `is_error` propagation with correct per-provider wire detail. Describes `MaxTurnsExceeded` classification fix and per-cause filesystem code surfacing. Accurate.

The CHANGELOG does NOT mention:
- The `ToolResult.truncated` field addition (arguably part of the diagnostic lift, but a separate structural change).
- The `ToolResultMessage.truncated` field addition in `opi-ai`.
- The tool-specific hardening (read line-range, write atomicity, edit uniqueness, bash StreamCapture, nav tool consistency).

**Assessment**: The CHANGELOG accurately describes the cross-crate behavioral changes but underreports the breadth of tool-specific improvements. This is acceptable for a CHANGELOG (which focuses on user-visible behavior changes) but worth noting.

## Documentation Sync Verification

### README.md / README.zh.md

The two READMEs are structurally parallel (same sections, same tables, same ordering). Content is semantically equivalent. Both carry all 9 non-goals. Both carry the same tool policy, bash execution, and truncation documentation.

**Status**: Synchronized, modulo the shared inaccuracy in edit tool description (P2-3).

### opi-spec.md / opi-spec.zh.md

Neither file was updated in Phase 11.

**Status**: Both are stale relative to Phase 11 implementation (P1-1). The Chinese spec also has internal drift from the English version in section 8.4 (P4-6).

## Overall Assessment

Phase 11 delivers a substantial and well-structured improvement to built-in tool quality. The ToolResult contract, filesystem error taxonomy, per-provider is_error propagation, diagnostic lift, and MaxTurnsExceeded termination are all cleanly implemented and well-tested. The tool hardening (read, write, edit, bash) and nav tool consistency are thorough.

The implementation quality is high: all 8 tools use builders consistently, path metadata is uniform, the StreamCapture bounded-memory design is correct, and the docs guard tests provide strong regression protection.

**Key items requiring attention** (in priority order):

1. **P1-1**: `opi-spec.md` must be updated to reflect Phase 11 changes (normative spec drift).
2. **P2-1**: read CRLF-to-LF normalization breaks the read-then-edit round-trip.
3. **P2-2**: NDJSON/RPC diagnostics not redacted (security/consistency).
4. **P2-3**: README edit tool description must be corrected (user-facing inaccuracy).
5. **P2-4**: bash `WaitFailed` branch violates operation-metadata contract.
6. **P2-5**: bash `StreamCapture` append error can deadlock child process.
7. **P3-1**: grep non-UTF-8 path handling must be aligned with find/glob/ls.
8. **P3-4**: read CRLF behavior should be tested and design choice documented.
9. **P3-5**: edit CRLF preservation should be tested.
10. **P3-6**: `MaxTurnsExceeded` trace integration untested.

The remaining findings (P1-2, P2-6, P2-7, P3-2, P3-3, P3-7, P4-1 through P4-8) are improvement opportunities that do not block the phase.

**Finding summary**: 2 P1, 7 P2, 7 P3, 8 P4 = 24 findings total.
