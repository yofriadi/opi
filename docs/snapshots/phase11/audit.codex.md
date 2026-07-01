# Phase 11 Independent Code Audit (Codex)

Date: 2026-06-30

Scope:

- Inputs: `docs/snapshots/phase11/opi-impl-state.json` and `docs/superpowers/specs/2026-06-24-phase11-tooling-quality-design.md`.
- Implementation range used for intent: `0a3add8..3ae3d40`; code inspected on current working tree.
- Existing audit/review reports were not opened, searched, summarized, or used as evidence.
- The implementation ledger's self-reported pass/fail status was treated only as task boundary metadata, not as audit evidence.

## Summary

Phase 11 materially improves tool result structure, filesystem taxonomy, bash execution handling, provider `is_error` propagation, and documentation. I do not consider the phase fully accepted against the design as written. The main blockers are public event-path leakage of raw bash diagnostics and a mismatch between the claimed CLI help acceptance scenario and the actual help/test coverage. There are also correctness gaps in `write`, `grep`, and navigation-tool performance guardrails.

## Findings

### P1: Bash failure diagnostics leak raw command text over NDJSON/RPC events

Locations:

- `crates/opi-coding-agent/src/tool/bash.rs:321-324`
- `crates/opi-coding-agent/src/tool/bash.rs:349-358`
- `crates/opi-agent/src/agent_loop.rs:213-225`
- `crates/opi-agent/src/agent_loop.rs:295-307`
- `crates/opi-agent/src/event.rs:63-70`
- `crates/opi-coding-agent/src/runner.rs:246-248`

Problem:

`bash_result` pushes a `ToolDiagnostic` for every bash error result. That diagnostic context includes `"command": command`. The agent loop then clones `result.diagnostics` directly into `AgentEvent::ToolExecutionEnd`. `AgentEvent` explicitly serializes these diagnostics verbatim, and non-interactive JSON mode wraps the cloned event into `AgentSessionEvent::Agent`.

The comment in `bash.rs` says `command` is content-sensitive and scrubbed by the diagnostic sink in Summary mode, but the NDJSON/RPC event path does not use `Diagnostic::redacted_payload`; it serializes the tool-owned diagnostic as-is.

Impact:

A failed, timed-out, or cancelled bash command that embeds a token, password, signed URL, or private path in the command line can be emitted in `ToolExecutionEnd.diagnostics[].context.command` to JSON/RPC clients. This violates the Phase 11 bash goal of no secret leakage in diagnostics. The existing no-leak test only covers inherited environment values and successful commands, so it does not catch this event-path leak.

Recommended fix:

Do not put raw command text in `ToolDiagnostic.context`, or convert tool diagnostics to redacted `DiagnosticPayload` before serializing public `AgentEvent` / `AgentSessionEvent` / RPC events. Add a regression test where a failing bash command contains a canary secret and assert the canary is absent from NDJSON/RPC `ToolExecutionEnd.diagnostics`.

### P1: `opi --help` does not expose the Phase 11 bash/truncation policy claimed by the acceptance scenario

Locations:

- `crates/opi-coding-agent/src/cli.rs:46-60`
- `crates/opi-coding-agent/src/cli.rs:50-52`
- `crates/opi-coding-agent/tests/non_interactive.rs:405-430`
- `crates/opi-coding-agent/tests/phase11_tooling_quality_docs.rs:271-286`
- `crates/opi-coding-agent/tests/phase11_tooling_quality_docs.rs:225-239`

Problem:

The Phase 11 design says tool docs and help output should clarify what bash can execute and how truncation/full-output paths work. The Phase 11 state acceptance scenario also says public `opi --help` documents tool-selection flags, mutating opt-in, and bash execution/truncation policy.

The actual clap help text only exposes brief flag descriptions such as `Allow mutating tools (write, edit, bash) in non-interactive mode.` It does not mention `cmd /C` vs `sh -c`, workspace-root cwd, 30s timeout, `timeout_secs`, `64 KiB`, or `details.full_output`. The tests only assert the four flag names and the word `mutating`, while README assertions cover bash/truncation separately.

Impact:

The public CLI boundary does not satisfy the documented acceptance scenario. A user invoking `opi --help` cannot discover the bash execution policy or truncation/full-output behavior, and the current guard tests would pass even if those help details remain absent.

Recommended fix:

Add a `long_about` or `after_help` section to the clap command that summarizes tool selection precedence, read-only vs mutating sets, bash shell/cwd/timeout policy, output cap/full-output spill, and the no-permission-popup rationale. Strengthen `phase11_cli_help_tool_policy` and `policy_docs_and_help_stay_in_sync` to assert those help strings, not only flag presence.

### P2: `write` to an existing directory falls back to a generic write failure instead of the filesystem taxonomy

Locations:

- `crates/opi-coding-agent/src/tool/write.rs:109-118`
- `crates/opi-coding-agent/src/tool/write.rs:145-174`

Problem:

`WriteTool` probes existence and prior size but does not reject an existing directory at the target path before staging the temp file. It then attempts `tokio::fs::rename(&temp_path, &file_path)`. If `file_path` is a directory, the rename fails and returns a generic `result::err` with no `FsToolError` diagnostic.

Impact:

The failure collapses to generic `tool_execution_failed` at the agent-loop boundary instead of a distinct `tool_not_a_file` / `tool_not_a_directory` diagnostic. This weakens Phase 11's filesystem taxonomy and makes JSON/RPC/trace consumers unable to distinguish "target is a directory" from an arbitrary write failure.

Recommended fix:

After resolving the path and before creating the temp file, call `metadata` for existing targets. If the target exists and is not a regular file, return a typed filesystem error (`NotAFile` is the closest existing variant for a file-writing target). Also classify rename errors by `ErrorKind` where possible, especially permission and directory/type errors, instead of always returning a generic `result::err`.

### P2: Navigation caps limit output after full traversal and full collection, not the work performed

Locations:

- `crates/opi-coding-agent/src/tool/grep.rs:77-127`
- `crates/opi-coding-agent/src/tool/find.rs:121-154`
- `crates/opi-coding-agent/src/tool/glob.rs:74-106`
- `crates/opi-coding-agent/src/tool/ls.rs:130-170`
- `crates/opi-coding-agent/src/tool/mod.rs:190-209`

Problem:

`grep`, `find`, `glob`, and `ls` all push every matching result into a vector, sort the full vector, and only then call `cap_nav_results` or truncate. For `grep`, every matching line is stored before the 200-result output cap is applied. For `find`/`glob`/`ls`, every matching path/entry is stored before truncation.

Impact:

The cap protects the final payload size but not memory, CPU, or traversal time. A large tree, or a 1 MiB file with hundreds of thousands of matching short lines, can still force large allocations and long scans. This falls short of the Phase 11 navigation-tool performance guardrail goal.

Recommended fix:

Introduce an execution guard separate from output formatting. Options include stopping after `MAX_NAV_RESULTS + 1` candidates when exact totals are not required, using a bounded top-N structure if lexicographic prefix must be preserved, and adding a visited-entry/read-byte hard cap with explicit `truncated`/`cancelled`/`omitted_count` semantics. Add tests that prove the tools stop work early rather than merely trimming the final string.

### P2: `grep` silently skips unreadable or non-UTF-8 files without diagnostics

Locations:

- `crates/opi-coding-agent/src/tool/grep.rs:94-107`

Problem:

`grep` ignores metadata failures and `read_to_string` failures with `continue`. Unlike `find`, `glob`, and `ls`, it does not count unsupported encodings or attach a `tool_unsupported_encoding` diagnostic. Permission/read failures are also not surfaced.

Impact:

Search results can be false negatives with no indication that files were skipped. This conflicts with Phase 11's filesystem tool policy to distinguish permission denied and unsupported encoding, and with the navigation-tool goal of stable error behavior.

Recommended fix:

Track skipped read failures by cause. For invalid UTF-8, count omitted files and attach a `CODE_TOOL_UNSUPPORTED_ENCODING` diagnostic. For permission denied, attach a typed permission diagnostic or include a structured skipped count in `details` with a warning diagnostic. Add tests for a non-UTF-8 text file and, where the platform supports it, an unreadable file.

### P3: Path-addressed failures do not carry uniform path metadata in `details`

Locations:

- `crates/opi-agent/src/tool/result.rs:32-39`
- `crates/opi-coding-agent/src/tool/mod.rs:142-148`
- `crates/opi-coding-agent/src/tool/read.rs:88-123`
- `crates/opi-coding-agent/src/tool/read.rs:195-209`
- `crates/opi-coding-agent/src/tool/edit.rs:372-381`
- `crates/opi-coding-agent/tests/tools_read_write_edit_bash.rs:3118`

Problem:

Success paths build `details` with `path_metadata`, but filesystem and edit-semantic failures use `result::err`, which forces `details: None`; `fs_error_result` only adds diagnostics. Tests explicitly assert that an error result must not carry details.

Impact:

Consumers of JSON/RPC/UI details see `workspace_root`, `path`, `resolved_path`, and `workspace_relation` for success but not for failures such as not found, not a file, binary file, outside workspace, or edit conflict. The Phase 11 design asks path-addressed tools to emit normalized path metadata when a concrete path applies, so the implementation and tests encode a narrower contract than the design text.

Recommended fix:

Decide the contract explicitly. If diagnostics are intended to be the sole failure metadata channel, update the Phase 11 design/DoD and docs to say so. If the design text is normative, extend `fs_error_result` or add a path-aware error builder that includes safe path metadata in `details` for concrete path failures, then update tests that currently require `details: None`.

## Verification Performed

- `cargo test -p opi-coding-agent --test non_interactive phase11_cli_help_tool_policy` passed. This confirms the current help guard is weak, not that the acceptance scenario is met.
- `cargo test -p opi-coding-agent --test tools_read_write_edit_bash bash_tool_no_secret_leakage_in_diagnostics_and_env_reporting` passed. This covers inherited env values and successful bash commands, but not failing-command diagnostics over NDJSON/RPC.

I did not run full workspace clippy/test/doc gates for this audit because no production code was modified.
