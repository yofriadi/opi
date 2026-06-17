# Phase 6 Codex Audit

审计对象：`docs/snapshots/phase6/opi-impl-state.json`、`docs/superpowers/specs/2026-06-15-phase6-alignment-hardening-design.md`，以及当前工作树 `e9e58f40fb87d5715fe7277a16fef376c67acc7e`。

审计方法：按 `grill-me` 方式把 Phase 6 的 10 条成功标准拆成可证伪问题；能从代码、测试、文档回答的问题均直接核验，不向用户追问。Ledger 中的 `passing` 只作为线索，不作为充分证据。

## Verdict

Phase 6 的核心实现大体成立：文档主路径已同步到 `0.5.1`，package add -> lock -> startup -> adapter registration 的生产路径存在，adapter protocol 覆盖面明显强于 Phase 5，RPC startup diagnostics 和 session extension state 也已有 focused tests。

但不能把 Phase 6 标记为“审计面完全干净”。当前有 4 个需要处理的风险，其中 3 个影响可审计性或用户可见行为。

## Findings

### P1. Phase 6 baseline audit is stale and its guard now preserves stale status

`docs/snapshots/phase6/audit-baseline.md` 仍是 Phase 6 早期的 point-in-time baseline：它说 Phase 6 task bucket 是 open tasks (`6.3`-`6.6`)，并明确写着 `None of those tasks is passing yet`。同一仓库的 Phase 6 ledger 又在 `phase_exit.6` 记录所有 6 个任务已经 passing。

Evidence:

- `docs/snapshots/phase6/audit-baseline.md:19`
- `docs/snapshots/phase6/audit-baseline.md:49`
- `docs/snapshots/phase6/audit-baseline.md:50`
- `docs/snapshots/phase6/audit-baseline.md:76`
- `docs/snapshots/phase6/audit-baseline.md:82`
- `docs/snapshots/phase6/opi-impl-state.json:1458`
- `docs/snapshots/phase6/opi-impl-state.json:1497`
- `crates/opi-coding-agent/tests/productized_packages_docs.rs:613`
- `crates/opi-coding-agent/tests/productized_packages_docs.rs:619`

This is not only wording drift. The guard test still requires contested Phase 5 findings to map to open Phase 6 tasks. That freezes an obsolete audit state after 6.3-6.6 have closed many of those same findings.

Concrete stale entries:

- Product loop not wired: baseline says open; `start_installed_package_runtime` is called from non-interactive, RPC, and interactive startup, and `runtime_startup_starts_installed_project_package_adapter` passes.
- `prepare_next_turn` / `transform_context` not implemented: baseline says accepted difference; `ProcessAdapter` implements both and `adapter_prepare_next_turn_can_inject_message` / `adapter_transform_context_can_rewrite_messages` pass.
- Relative adapter command escape: baseline accepts it; `resolve_adapter_command_checked` rejects escape and tests cover `..` and Windows drive-relative forms.
- RPC startup diagnostics absent: baseline says open; `rpc_ready_header_carries_startup_diagnostics` and `rpc_session_info_surfaces_startup_diagnostics` pass.

Recommendation: keep the original baseline as historical context or add a dated "Final Phase 6 reconciliation" section, but update the current disposition table and the guard so it no longer requires "open Phase 6 task" for closed Phase 6 work.

### P1. RPC extension commands can mutate adapter state without persisting it on quit

`CodingHarness::dispatch_extension_command` takes `&self`, dispatches the command, and does not persist extension state. RPC calls it from the `extension_command` branch through `self.harness.as_ref()`, so command-only RPC workflows can mutate adapter state and then quit without writing an `ExtensionState` entry.

Evidence:

- `crates/opi-coding-agent/src/rpc.rs:665`
- `crates/opi-coding-agent/src/rpc.rs:681`
- `crates/opi-coding-agent/src/harness.rs:1115`
- `crates/opi-coding-agent/src/harness.rs:1118`
- `crates/opi-coding-agent/src/harness.rs:1194`
- `crates/opi-coding-agent/tests/session_extension_state.rs:192`
- `crates/opi-coding-agent/tests/session_extension_state.rs:225`
- `crates/opi-coding-agent/tests/rpc_jsonl.rs:3001`
- `crates/opi-coding-agent/tests/rpc_jsonl.rs:3035`

The existing persistence test mutates adapter state through `todo/add`, then runs a prompt turn, and only then asserts state was written. The RPC command consistency test proves `todo/add` followed by `todo/list` works in memory during the same RPC session, but it does not prove that a command-only session survives restart.

Impact: stateful package commands are useful precisely as commands. A user or headless client can issue `extension_command todo/add`, receive success, quit, and lose that state unless a later prompt turn happens.

Recommendation: either persist extension state after successful `extension_command` dispatch when a session exists, or document command-only state as volatile and add a guard test for that contract. The first option better matches the Phase 6 session/RPC hardening goal.

### P1. Graceful adapter shutdown is tested for owned hosts but not wired into production adapter teardown

`AdapterHost::shutdown(mut self, ...)` sends the protocol `shutdown` message and waits up to 5 seconds before killing the child. The production bridge stores the host as `Arc<AdapterHost>` inside `ProcessAdapter`, registers that adapter into `ExtensionRegistry`, and has no production call path that consumes the owned host to call graceful shutdown. When the last `Arc` drops, `Drop for AdapterHost` calls `start_kill()` directly.

Evidence:

- `crates/opi-coding-agent/src/adapter_host.rs:407`
- `crates/opi-coding-agent/src/adapter_host.rs:461`
- `crates/opi-coding-agent/src/adapter_host.rs:506`
- `crates/opi-coding-agent/src/adapter_host.rs:511`
- `crates/opi-coding-agent/src/adapter_extension.rs:208`
- `crates/opi-coding-agent/src/adapter_extension.rs:833`
- `crates/opi-coding-agent/tests/adapter_host.rs:425`

The existing `shutdown_waits_for_child_exit_before_kill` test is valid for the host API, but it does not cover normal harness/RPC/interactive teardown of installed adapter packages.

Impact: Phase 6 claims "shutdown behavior" coverage, but package adapters in production likely observe kill-on-drop rather than protocol shutdown unless another owner calls `AdapterHost::shutdown`, which the current call-site search did not find outside tests.

Recommendation: introduce an explicit async shutdown path for `ProcessAdapter` / `ExtensionRegistry` / `CodingHarness`, or narrow the documented contract to "owned AdapterHost shutdown is graceful; registry teardown is best-effort kill" and add a production-path test.

### P2. Agent context docs still identify the workspace as 0.5.0

The Phase 6 documentation guards cover README, opi-spec, pi-alignment-matrix, and crate READMEs. They do not cover `AGENTS.md` or `CLAUDE.md`, both of which are loaded as agent context and still contain current-state `0.5.0` claims.

Evidence:

- `Cargo.toml:12`
- `README.md:12`
- `AGENTS.md:9`
- `CLAUDE.md:7`
- `crates/opi-coding-agent/tests/productized_packages_docs.rs:438`
- `crates/opi-coding-agent/tests/productized_packages_docs.rs:503`

Impact: future agents receive stale project context even though user-facing docs are correct. This already surfaced in this audit because the active AGENTS instructions said `Current workspace version: 0.5.0` while the workspace and Phase 6 docs say `0.5.1`.

Recommendation: update `AGENTS.md` / `CLAUDE.md` or deliberately mark their `0.5.0` text as historical. Add them to the documentation truth guard if they are intended to be current project context.

## Success Criteria Trace

| SC | Result | Notes |
|---|---|---|
| 1. Current docs identify 0.5.1 | Mostly met | Main docs and crate READMEs pass. Agent context docs still stale. |
| 2. EN/ZH docs synchronized | Met for guarded docs | Guarded README/spec/matrix/crate claims pass. |
| 3. Phase 6 baseline reconciles Phase 5 findings | Partially met | Baseline exists, but now stale after 6.3-6.6 completion. |
| 4. Package startup success/degraded paths | Met | Focused package and harness tests pass. |
| 5. Adapter protocol lifecycle/failure/cancellation/state/shutdown | Partially met | Protocol and host tests pass; production graceful teardown gap remains. |
| 6. Session/RPC extension-state and diagnostics | Partially met | Startup diagnostics and turn-persisted state pass; command-only RPC mutation persistence gap remains. |
| 7. Docs guards prevent overclaiming | Met with caveat | Guards pass, but phrase-based docs guards do not cover AGENTS/CLAUDE. |
| 8. Future Ecosystem backlog non-committal | Met | Baseline backlog is non-committal. |
| 9. No forbidden non-goal implementation added | Met | Guard tests cover no opi-types crate, no JS/TS runtime, provider set unchanged, protocol types stay in opi-coding-agent. |
| 10. Final verification gates | Not fully rerun in this audit | Focused tests passed; full clippy/doc/workspace test were not rerun here. |

## Verification Run

Commands run during this audit:

```text
cargo test -p opi-coding-agent --test phase4_ledger
cargo test -p opi-coding-agent --test productized_packages_docs
cargo test -p opi-coding-agent --test adapter_host --test adapter_runtime
cargo test -p opi-coding-agent --test package_resolver --test package_cli --test package_manifest_v2 --test harness_resource_integration
cargo test -p opi-coding-agent --test session_extension_state --test rpc_jsonl --test session_runtime
```

All commands above passed.

Hash check:

- Phase 6 design hash matches the ledger.
- `docs/opi-spec.md` matches the ledger under the repository's normalized LF hash contract used by `phase4_ledger`.
- Raw checkout bytes differ on Windows due to line endings and should not be treated as snapshot drift.

## Recommended Next Actions

1. Refresh `docs/snapshots/phase6/audit-baseline.md` into a final Phase 6 reconciliation, and update `phase6_baseline_audit_is_complete` so it no longer locks in early open-task language.
2. Add a failing regression test for command-only RPC state persistence across restart; then persist extension state after successful stateful `extension_command` dispatch or document volatility explicitly.
3. Add production teardown coverage for installed adapter packages. Decide whether production shutdown is graceful or kill-only, then align code/docs/tests.
4. Bring `AGENTS.md` and `CLAUDE.md` into the 0.5.1 documentation truth set, or mark their version claims as historical.
