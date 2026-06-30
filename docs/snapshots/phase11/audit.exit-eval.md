# Phase 11 Exit-Evaluation Audit

Independent phase-exit evaluation for Phase 11 (Tooling Quality). Each criterion
was traced to current code and tests by an independent verifier, not from ledger
status. All ten criteria are `met`; there are no `not-met` or
`deferred-by-updated-design` verdicts.

Source spec: `docs/superpowers/specs/2026-06-24-phase11-tooling-quality-design.md`
Final task commit: `3ae3d40` (task 11.11).

## Success Criteria

| ID | Criterion | Verdict | Anchor evidence |
|----|-----------|---------|-----------------|
| SC1 | Built-in tool result details follow a consistent contract | met | `crates/opi-agent/src/tool.rs:54-65` (`ToolResult`, `ToolDiagnostic`); `crates/opi-agent/src/tool/result.rs` builders (`ok`/`err`, `path_metadata`, `bash_operation_metadata`); all 8 tools route through them (`bash.rs:318`, `edit.rs:329/348/373`, `read.rs:195/208`, `write.rs:186/200`, `find.rs:165`, `glob.rs:118`, `grep.rs:139`, `ls.rs:206`); 6 unit tests + 86 integration tests in `tools_read_write_edit_bash.rs`. |
| SC2 | `edit` handles CRLF/LF and conflict cases predictably | met | `crates/opi-coding-agent/src/tool/edit.rs`: binary-mode read (`:187`), UTF-8 decode (`:218`), single exact `replacen` (`:290`), atomic temp-then-rename (`:312/318`); empty/no-op rejected pre-side-effect (`:118/128`); not-found returns `occurrences:0` + `file_bytes`/`line_count` (`:247`); multi-match refused, not silently first-matched (`:264`); 25 `edit_*` tests green. |
| SC3 | `write` reports create/overwrite and avoids silent partial writes | met | `crates/opi-coding-agent/src/tool/write.rs:109-197`: pre-write probe (`exists`+`metadata.len()`), `action` = `created`/`overwritten`, `bytes_written`/`bytes_before`/signed `size_delta`; sibling temp `.{name}.opi-write-tmp-{pid}-{nanos}` (`:163`) + atomic `rename` (`:171`) with temp cleanup on every error path; NUL/binary pre-side-effect guard (`:89-107`); 16 `write_tool_*` tests green. |
| SC4 | `bash` timeout/cancellation/cwd/env/exit-code/truncation documented and tested | met | `crates/opi-coding-agent/src/tool/bash.rs`: `MAX_BASH_OUTPUT_BYTES = 64*1024` (`:26`); 30s default (`:77`); `current_dir(workspace_root)` (`:79/85`); biased 3-way `select!` cancel→timeout→`child.wait` (`:151-167`) with `child.kill()` on cancel/timeout; `exit_code` null on cancel/timeout, `is_error = exit_code != Some(0)` (`:270`); merged 64 KiB cap + `details.full_output` spill; `with_env_policy` injects `details.env={inheritance:"inherited",values_included:false}` (`:372-375`); ~15 bash tests + `phase11_tooling_quality_docs` docs-guard; documented in README/README.zh/spec §8.4. |
| SC5 | Read-only nav tools have consistent ignore/sorting/limit/error behavior | met | `crates/opi-coding-agent/src/tool/mod.rs`: shared `nav_walk_builder` (`:160-169`, `.git_ignore(true)` + hidden defaults), `cap_nav_results` (`:190-210`, `MAX_NAV_RESULTS=200`), `fs_error_result` (`:144-149`); lexicographic relative-path sort (`find.rs:151`, `glob.rs:103`, `ls.rs:162`; grep preserves intra-file line order `grep.rs:120`); typed `FsToolError`→`CODE_TOOL_*` (`diagnostic.rs:246-253`); 56 tests incl. cross-tool nested-`.gitignore` fixtures. |
| SC6 | Tool diagnostics integrate with Phase 7 traces | met | `crates/opi-agent/src/trace.rs:84-116` (`TraceKind::DiagnosticLinked` + shared `source`/`diagnostic_code`/`severity`); `crates/opi-agent/src/diagnostic.rs:221-276` (`CODE_TOOL_*`), `From<&AgentError>` (`:622-679`), `resolve_tool_code` (`:541-556`); `crates/opi-agent/src/agent_loop.rs:840-858` single `observe()` router mirrors every tool failure onto the trace in lockstep; `:937-953` per-cause lift; `trace_envelope.rs` + `diagnostics_runtime.rs` tests (6+2+20 green). |
| SC7 | Tool scheduling respects Phase 8 runtime contracts | met | `Tool::execution_mode()` (`crates/opi-agent/src/tool.rs:30-33`); `batch_is_sequential` (`agent_loop.rs:171-176`) → serial vs parallel `join_all` preserving source order (`:178-321`); mutating=Sequential, read-only=Parallel; `should_stop_after_turn` after every turn (`:337-362`); `AgentError::MaxTurnsExceeded` fall-through (`:523-535`) + Warning diagnostic (`diagnostic.rs:660`); `agent_loop_semantics.rs`/`tool_validation.rs`/`hooks_queues.rs`/`diagnostics_runtime.rs` tests green. |
| SC8 | No permission popup/background bash/remote/sandbox/workflow tool in core | met | Exactly 8 `BUILTIN_TOOL_NAMES` (`policy.rs:19-21`); mutating gating is policy-level `MutatingToolRequiresOptIn` (`policy.rs:60-66`) + `NonInteractiveHooks::before_tool_call` deny (`runner.rs:662-679`); `InteractiveCodingHooks` does not override `before_tool_call` (inherits default `Allow` — no popup, `harness.rs:1949-1953`); bash foreground `status = child.wait()` (`bash.rs:162`) with `child.kill()` on timeout/cancel; no sandbox/remote/LSP/IDE-index/workflow module under `crates/**/src/`; `sc8_non_goals_not_in_core` + `interactive_allows_mutating_tools` green. |

## Non-Goals (all 9)

Permission popup, persistent background bash, remote execution, IDE project
index, language-server integration, automatic formatting on write/edit, package
ecosystem expansion, workflow tools (todo/plan-mode/sub-agents), sandbox
implementation — each asserted documented as a non-goal in both READMEs and
proven absent from core via the SC8 structural positives above.

## Documentation Updates (all 7 bullets, 4 surfaces)

README.md, README.zh.md, `docs/opi-spec.md` §8.4, and CLI `--help` each carry:
read-only vs mutating classification; `--tools`/`--no-tools`/`--no-builtin-tools`/
`--allow-mutating` interaction; bash execution scope; truncation/full-output; and
the permission-prompt rationale ("tool-selection check, not a permission or
sandbox subsystem"). Guarded by `tests/phase11_tooling_quality_docs.rs::
policy_docs_and_help_stay_in_sync` and `tests/non_interactive.rs::
phase11_cli_help_tool_policy`.

## Product-loop reachability

The policy is wired through production startup, not just tests:
`main.rs:109-113` (`resolve_tool_selection`) → `runner.rs:140-153` /
`main.rs:477-499` (`ToolRuntimeConfig::resolve`) → `CodingHarness::builder()
.tool_selection().tool_config()` → `harness.rs:723,1583-1628` (`build_tools`
constructs all 8 tools and filters by `active_tool_names`). All three entry
points (non-interactive, RPC, interactive) carry the policy; the mutating-opt-in
also runs at the runtime hook layer (`runner.rs:662-679`).

## Verdict

Phase 11 exit criteria: **10/10 met**. Archive-eligible. The final task
(`3ae3d40`) and the full task graph are preserved in
`docs/snapshots/phase11/opi-impl-state.json`; the active ledger's
`phase_exit[11].task_summary` holds the compacted 11-task summary.
