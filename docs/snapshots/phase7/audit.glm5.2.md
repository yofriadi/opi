# Phase 7 Reliability & Observability — Independent Audit (GLM-5.2)

| | |
|---|---|
| **Auditor** | GLM-5.2 (independent; did not author Phase 7 code) |
| **Date** | 2026-06-19 |
| **Scope** | Phase 7 (tasks 7.1–7.6), commits `4812c94` → `42969ad` (+ archive `4dccbad`) |
| **Inputs** | `docs/snapshots/phase7/opi-impl-state.json`, `docs/superpowers/specs/2026-06-15-phase7-reliability-observability-design.md`, and the Phase 7 source/test tree |
| **Method** | Independent gate run + a 55-agent review workflow (ledger verification, 8 Success Criteria + 9 Non-Goals compliance, adversarial per-surface code review with per-finding verification), plus direct read-through of the security-critical files (`diagnostic.rs`, `trace.rs`, `streaming_proxy.rs` `SecretRedactor`, `doctor.rs`, `diagnostic_bridge.rs`, `harness.rs`, `agent_loop.rs`, `rpc.rs`) to confirm the material findings myself. |

> **Verification-status convention.** Each finding below is tagged:
> **[auditor-verified]** — confirmed by the auditor directly (code read or empirical test).
> **[workflow-verified]** — confirmed by an independent verify subagent (`is_real: true`, high confidence).
> **[unverified-by-skeptic]** — surfaced by a review agent but the rate-limited verify subagent did not run; the auditor independently confirmed the substantive ones (noted inline).
>
> **Workflow rate-limit note.** The review workflow hit a 5-hour API quota ceiling near its end and ~23 per-finding verify subagents plus the `agent-loop-wiring` review subagent failed to run. The auditor closed the resulting gaps by reading the cited code directly. Findings tagged **[unverified-by-skeptic]** were not double-checked by a second model; where the auditor confirmed them they are marked **[auditor-verified]**.

---

## 1. Verdict

Phase 7 is a **structurally sound, well-tested observability substrate** built on the right abstractions (a shared `Diagnostic` shape, a versioned opt-in trace envelope, a network-free `opi doctor`). The ledger's per-task claims are **accurate at the commit level**: all six tasks exist at the claimed commits with real production-code evidence for their Definitions of Done, and the gate suite is green.

However, **"all 8 Success Criteria met" overstates the result**, and the central security control — the secret redactor — has a **credential-disclosure defect** that the test suite does not catch. The audit's bottom line:

- **2 CRITICAL redaction defects.** The redactor matches only synthetic / legacy *no-inner-hyphen* API key forms. Every real current provider key format — Anthropic `sk-ant-api03-…` / `sk-ant-api06-…`, OpenAI `sk-proj-…` / `sk-live-…` / `sk-svcacct-…` — **is not redacted**. Empirically confirmed. The SC6 guard test uses the synthetic forms, so it passes while real keys leak.
- **SC4 and SC5 are `partial`, not `met`.** SC4: the RPC `trace` command is test-only — `run_rpc` hard-codes `trace_sink: None`, so real `opi --rpc` users always get `unsupported_trace_request` despite docs advertising it. SC5: adapter degradation is never typed (`SOURCE_ADAPTER` is a dead constant); startup/adapter failures flow as `Vec<String>` that bypass redaction.
- **`Diagnostic.message` is never redacted** (documented as intentional), which makes it a live leak surface: provider error bodies, package-discovery errors, and credentialed URLs embedded in `message` ship out un-redacted at `doctor --json`/text and any recording-sink/summary boundary.
- **Two 7.2 DoD emission claims are not actually closed in production:** session-recovery diagnostics exist only as a pure function with test callers (dead in the resume path), and compaction reaches the trace envelope but not the in-process `DiagnosticSink` (so run-summary `diagnostic_counts` excludes compaction).

This is **not a "fail."** The architecture is correct, the failure-mode coverage for retry/cancel/provider/tool is genuine, the non-goals are respected, and the gates pass. It is a **"met-with-critical-holes"** result: the redaction control must be fixed and re-tested against real key formats before this surface is relied upon, and the SC4/SC5/emission-wiring gaps should be tracked into Phase 8.

| Area | Result |
|---|---|
| Gate suite (fmt/clippy/doc/test/smoke) | **Green** (smoke initially flaked on an orphaned Phase 5 process — diagnosed and resolved; see §2) |
| Ledger verification 7.1–7.6 | **All verified** at claimed commits, with production-code DoD evidence |
| Success Criteria | **6 met, 2 partial** (SC4, SC5). SC6 nominally met but its control is critically defective. |
| Non-Goals (9) | **All respected** |
| Critical findings | **2** (both redaction; auditor-verified) |
| High/Medium findings | ~9 (message leak, startup bypass, session/compaction wiring, RPC trace scope, rubber-stamp test, …) |
| Low/Info findings | ~12 (trace ordering under parallel tools, heuristic path redaction, dead code, bedrock probe, …) |

---

## 2. Independent Gate Results

Run fresh for this audit (`cargo` workspace, Windows/MSVC toolchain):

| Gate | Result | Notes |
|---|---|---|
| `cargo fmt --check --all` | **PASS** (rc 0) | |
| `cargo clippy --workspace --all-targets -- -D warnings` | **PASS** (rc 0) | |
| `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` | **PASS** (rc 0) | |
| `cargo test -p opi-agent --test diagnostics` | **PASS** 17 | ledger said 16 at the 7.1 commit; +1 (`phase7_redacts_sensitive_values`) added in 7.6 |
| `cargo test -p opi-ai --test provider_diagnostics` | **PASS** 8 | |
| `cargo test -p opi-agent --test diagnostics_runtime` | **PASS** 31 | |
| `cargo test -p opi-coding-agent --test diagnostics_runtime` | **PASS** 5 | |
| `cargo test -p opi-agent --test trace_envelope` | **PASS** 29 | ledger 7.3 said 21; grew through 7.5/7.6 |
| `cargo test -p opi-coding-agent --test doctor_cli` | **PASS** 30 | ledger 7.4 said 28; +2 in 7.6 |
| `cargo test -p opi-coding-agent --test json_mode` | **PASS** 16 | |
| `cargo test -p opi-coding-agent --test rpc_jsonl` | **PASS** 69 | |
| `cargo test -p opi-coding-agent --test observability_docs` | **PASS** 4 | |
| `cargo test -p opi-coding-agent --test productized_packages_docs` | **PASS** 35 | |
| `cargo test --workspace --all-targets --no-fail-fast` | **PASS** 0 failures | every binary green |
| `scripts/opi-impl-smoke.ps1` | **PASS (64)** after cleanup | see note below |

**Smoke flake diagnosis (not a Phase 7 defect).** The smoke script's `cargo test` step failed twice with `LNK1104: 无法打开文件 adapter_host_mock-….exe` ("file in use by another process"). Root cause: an **orphaned `adapter_host_mock` test process (PID 34368)** — a Phase 5 adapter-process-host test that spawns children — never exited and held the test binary's `.exe` open, so the linker could not overwrite it. This is a **Phase 5 Windows process-leak**, unrelated to Phase 7. After `taskkill /PID 34368` (and a stray `cargo.exe`), the smoke passed clean (64/64, `smoke PASSED`). The full-workspace test had already compiled and run `adapter_host_mock` successfully; only the back-to-back relink contended on the stale handle. Recommend a follow-up (out of Phase 7 scope) to investigate why the adapter-host test leaks a process on Windows.

**Test-count drift vs ledger.** Current HEAD counts are slightly higher than the per-task ledger snapshots (diagnostics 16→17, trace_envelope 21→29, doctor_cli 28→30) because 7.5/7.6 added tests after the earlier tasks' `verified_at_commit`. This is expected; the ledger counts are accurate at their respective commits.

---

## 3. Compliance with the Phase 7 Design

| Criterion | Auditor verdict | Ledger claimed | Note |
|---|---|---|---|
| **SC1** shared diagnostic shape used by new surfaces | **met** (with fidelity gaps) | met | One shared `Diagnostic` crosses every structured boundary. Gaps: `StartupDiagnostics` is `Vec<String>`; `NullSink` is the default production sink (opt-in by design). |
| **SC2** `opi doctor` reports all 6 scopes, no network | **met** | met | Network-free enforced structurally (dispatch before provider construction; credential *presence* only). One depth gap: "corrupt-line recovery" is a static info string, not a scan. |
| **SC3** JSON/RPC startup + run summaries expose structured counts | **met** | met | `SessionDiagnosticCounts` on both surfaces; startup diagnostics precede prompt output; `run_summary` ordered after `AgentEnd`; per-run counts via `RecordingSink::clear`. |
| **SC4** local redacted trace envelope can be requested | **partial** | met | Non-interactive `--trace PATH` works end-to-end. **But the RPC `trace` command is unreachable from production**: `run_rpc` uses `RpcRunner::new_with_runtime_packages`, which hard-codes `trace_sink: None` (`rpc.rs:220`); the only constructor that sets a sink (`new_with_trace`) is test-only. Real `opi --rpc` users issuing `trace` always get `unsupported_trace_request`, contradicting `docs/opi-spec.md` ("the RPC `trace` command"). No config-based toggle exists either. |
| **SC5** retry/cancel/compaction/adapter/provider-failure in diagnostics + trace | **partial** | met | 4 of 5 categories (retry, cancel, compaction, provider failure) are solid in code + tests with trace mirroring. **Adapter degradation is not represented:** `SOURCE_ADAPTER` is declared but never used in any production emit; startup/adapter/package failures route to `metadata.diagnostics: Vec<String>` via raw `format!()`, bypassing the shared model and redaction. |
| **SC6** redaction tests cover API keys / bearer / env / prompt / tool output | **met on paper — control critically defective** | met | Tests exist for each of the five classes. **But the redactor does not match real provider key formats (see §5.1), and the SC6 guard uses synthetic no-hyphen keys.** The criterion is nominally satisfied; the implementation is not. |
| **SC7** docs state observability is local and explicit | **met** | met | EN + ZH, pinned by a substantive guard test. |
| **SC8** no telemetry / ecosystem / OAuth / marketplace / web dashboard | **met** | met | No telemetry crate, no global subscriber, no network egress in Phase 7 files; OAuth/web-ui references are pre-existing (Vertex AI, `opi-web-ui`). Guards are static (grep-based blocklist) — see §5.3 for future-proofing note. |
| **Non-Goals (9)** | **all respected** | met | Independent grep + commit-diff confirms zero forbidden additions. |

---

## 4. Ledger Verification (tasks 7.1–7.6)

All six tasks **verified**: each `verified_at_commit` exists, touches only its task-owned crate, the claimed behavioral tests exist with the claimed (or now-greater) counts, and every Definition-of-Done clause maps to production code, not tests alone.

**One ledger overstatement to flag.** Task 7.2's acceptance scenario `phase7-runtime-failure-diagnostics` was recorded as left OPEN at 7.2 and "closed later in 7.5/7.6." The audit finds that closure is **only partial**:

- **Session recovery** — *not closed.* `CrashRecovery::diagnostics()` exists (`session.rs:138`) and is unit-tested, but a repo-wide search shows **only test callers**. The production resume path never records session-recovery diagnostics into the `DiagnosticSink` or emits a `DiagnosticLinked` trace record. The mapping is dead code from the runtime's perspective. **[auditor-verified]**
- **Compaction** — *partially closed.* `CODE_SESSION_COMPACTED` is emitted to the trace collector only (`harness.rs:1181` → `trace_diagnostic` at `:1320`), never to the in-process `RecordingSink`. Result: `CodingHarness::diagnostic_counts()` (`harness.rs:1333`) never counts compaction, and a non-`--trace` run emits no compaction diagnostic in-process. **[auditor-verified]**
- **Adapter degradation** — *not closed.* Still `Vec<String>` (see SC5 / R1).
- **Config/package at runtime** — *not closed.* `diagnostic_from_config` / `diagnostic_from_package` (`diagnostic_bridge.rs`) are wired only into `opi doctor` (`doctor.rs:282,427`); the runtime/RPC/interactive startup paths do not use them (`main.rs` config-error path is `eprintln!` + `exit(2)`; `rpc.rs` has zero `Diagnostic` references). **[auditor-verified]**

---

## 5. Findings

### 5.1 Critical

#### C1. Anthropic API keys (`sk-ant-api03-…` / `sk-ant-api06-…`) are not redacted **[auditor-verified]**
- **Location:** `crates/opi-agent/src/streaming_proxy.rs:336` (pattern `sk-ant-[a-zA-Z0-9]{20,}`) and the fallback at `:348` (`sk-[a-zA-Z0-9]{20,}`).
- **Defect:** Production Anthropic keys are issued as `sk-ant-api03-<body>` (and historically `sk-ant-api06-`). The `-api03-` segment sits between `sk-ant-` and the body; because `-` is not in `[a-zA-Z0-9]`, the `{20,}` quantifier consumes only `api03` (5 chars) and the match fails. The generic `sk-` fallback fails the same way.
- **Empirical evidence (this audit, ERE with identical class semantics):**

  | Input | `sk-ant-[a-zA-Z0-9]{20,}` | `sk-[a-zA-Z0-9]{20,}` |
  |---|---|---|
  | `sk-ant-api03-1234567890abcdefghijklmnopqrstuv` (real Anthropic) | **NO MATCH** | **NO MATCH** |
  | `sk-ant-api06-…` | **NO MATCH** | **NO MATCH** |
  | `sk-ant-1234567890abcdefghijklmnopqrstuv` (synthetic, no hyphen — **what the test uses**) | MATCH | no match |
  | `sk-1234567890abcdefghijklmnopqrstuv` (legacy bare) | no match | MATCH |

- **Impact:** A real Anthropic key embedded in any diagnostic `details` string value, or under a non-sensitive field name, ships un-redacted through `doctor --json`, JSON mode, RPC responses, and the trace envelope. Field-by-name redaction only saves keys sitting in a field literally named `api_key`/`authorization`/`token`/etc.
- **Why the test missed it:** `phase7_redacts_sensitive_values` (`crates/opi-agent/tests/diagnostics.rs:265`) uses the synthetic `sk-ant-1234567890…` (no inner hyphen) and embeds it in `tool_output` (redacted by content-sensitivity regardless of the secret pattern). A grep across `crates/` confirms `streaming_proxy.rs:336` is the only Anthropic value pattern — no compensating regex.

#### C2. OpenAI API keys (`sk-proj-…` / `sk-live-…` / `sk-svcacct-…`) are not redacted **[auditor-verified]**
- **Location:** `crates/opi-agent/src/streaming_proxy.rs:348` (`sk-[a-zA-Z0-9]{20,}`).
- **Defect:** The pattern matches only legacy bare keys (`sk-<48 base62>`). Current OpenAI formats (`sk-proj-`, `sk-live-`, `sk-svcacct-`) all contain a hyphen within the first 20 characters after `sk-`, breaking the class. Empirically confirmed **NO MATCH** for all three current formats.
- **Impact:** Same as C1 for OpenAI keys. The doc comments (`streaming_proxy.rs:42-43`, `:313`, `:347`) claim `sk-*` covers OpenAI API keys — that claim has not held since OpenAI shipped the `sk-proj-` family.
- **Root cause for C1+C2:** the character class must include `-` (e.g. `sk-ant-[a-zA-Z0-9-]{20,}`, `sk-[a-zA-Z0-9-]{20,}`). Adding `-` was empirically confirmed to restore correct matching.

### 5.2 High / Medium

#### H1. `Diagnostic.message` is never redacted **[auditor-verified]**
- **Location:** `crates/opi-agent/src/diagnostic.rs:124-127` (doc: "Redaction never touches the severity, code, source, message, or action fields") and the `From<&ProviderError>` / `From<&AgentError>` bridges at `:301-399` which clone provider/tool error strings straight into `message`.
- **Defect:** Redaction runs only on `details`. Provider response bodies (`AuthFailed(format!("authentication failed: {body}"))`, `RequestFailed(format!("HTTP {code}: {body}"))` — see `anthropic.rs`/`azure_openai.rs`/`gemini.rs`), tool error strings, and credentialed URLs in discovery errors that land in `message` flow verbatim to every output boundary.
- **Impact:** Latent but real. Bounded today because (a) the default production sink is `NullSink`, (b) `TraceRecord` has no `message` field, and (c) 401/403 bodies usually omit the key. But any recording sink, run-summary renderer, or `doctor` output that surfaces `message` emits it un-scrubbed. This is the root design gap behind H2/H3 below.
- **Severity rationale:** Medium. It is *documented as intentional*, but it contradicts the design's "API keys, bearer tokens, … should not be emitted unless the user explicitly asks for verbose debug output."

#### H2. `opi doctor --json` leaks absolute paths (OS username + package path) via `message` **[auditor-verified]**
- **Location:** `crates/opi-coding-agent/src/doctor.rs:261-273` (`format_json` redacts `details` only) ← `crates/opi-coding-agent/src/diagnostic_bridge.rs:36-41` (`diagnostic_from_package` copies `pd.message` verbatim) ← `crates/opi-coding-agent/src/package_resolver.rs:172,329-330,455-456` (`manifest_sha256` failure produces `PackageResolverError::Failed(format!("read {}: {e}", path.display()))`, surfaced as the `manifest_hash_failed` diagnostic).
- **Defect:** Package diagnostics embed the **canonicalized package root** (e.g. `/Users/<username>/.config/opi/packages/<name>/package.toml` or `C:\Users\<username>\…`) directly in `message`. `doctor --json` never redacts `message`, so the absolute path — and the OS username inside it — ships out. A credentialed URL or token in a discovery-error `message` would leak the same way.
- **Contradicts** the 7.4 fix's stated intent ("details routed through `redacted_details(Summary)` at the --json boundary so absolute config paths are not leaked"). The fix covered `details.package_source` and `details.path`, but `message` is a parallel, un-redacted leak surface. The 7.6 guard test (`phase7_doctor_redacts_sensitive_values`) does not construct a path-bearing `message`, so it passes while the leak ships. **[workflow finding, auditor-confirmed]**

#### H3. `opi doctor` (text) is entirely unredacted **[workflow-verified, is_real:true]**
- `doctor.rs:233-253` `format_text()` prints `message`/`action`/`code` with zero redaction. Documented-by-omission (the module scopes redaction to the `--json` boundary), so it is a contract-narrowing rather than a breach — but `opi doctor` with no flags emits the same paths H2 describes.

#### H4. Startup diagnostics bypass redaction on three live boundaries (R1, worse than ledger) **[auditor-verified]**
- **Location:** `RuntimePackageStartup.diagnostics: Vec<String>` / `DiscoveredResourceMetadata.diagnostics: Vec<String>` are pre-formatted free text including the adapter `command` string and package-discovery errors. They are emitted **verbatim, with no `redact()`** on: (1) `rpc.rs:351-362` `rpc_ready` header `startup_diagnostics`; (2) `runner.rs:194-203 / 446-456` NDJSON `AgentSessionEvent::StartupDiagnostics`; (3) `harness.rs:158-167 / 710-719` `format_for_system_prompt` injecting them into the system prompt sent to the provider. (`main.rs:488` and `runner.rs:164` thread `runtime_startup.diagnostics` straight through.)
- **Defect:** An adapter `command` that is an absolute path (`/Users/alice/bin/adapter`), or a package-source/discovery error containing a credentialed git URL or inline token (`https://alice:s3cr3t@host/…`, `… -token ghp_xxx`), ships out un-redacted. `doctor.rs:format_json` *does* redact because it rebuilds structured `Diagnostic` details; the rpc/runner/prompt paths operate on already-flattened strings and never touch the redaction core.
- **Severity:** Medium-High. Bounded (requires a secret-bearing adapter command or package-discovery string to appear — not reachable from a normal user prompt), but a real un-redacted leak surface, contradicting the design's "absolute paths … should not be emitted" and the secret-redaction intent. The ledger frames R1 as a typing cosmetic ("metadata strings, not typed Diagnostic+observe()"); the production consequence is a leak path.

#### H5. SC6 guard test does not exercise the `StartupDiagnostics` path it claims to guard **[workflow-verified, is_real:true]**
- `crates/opi-coding-agent/tests/json_mode.rs:648-705` (`phase7_json_trace_redacts_sensitive_values`): the match arm at `:691-695` explicitly includes `"StartupDiagnostics"`, but the test seeds secrets **only into the user prompt** (`:658-661`), which is the conversation stream the test deliberately skips. No secret ever reaches a `StartupDiagnostics` line, so `!serialized.contains(secret)` passes vacuously for that event type. It would still pass with H4's leak present. (Same shape in the sibling trace test at `:602-640`.) Medium.

#### H6. RPC `trace` command returns a cumulative session envelope, not a per-run one **[auditor-verified]**
- **Location:** `harness.rs:1281-1298` `prepare_trace_run` clears the diagnostic `RecordingSink` (`:1285-1287`) but **not** the trace sink; `RecordingTraceSink` is constructed once per `RpcRunner` and reused; `rpc.rs:768-776` serializes `sink.snapshot()`.
- **Defect:** A `trace` request after run N returns the merged records of runs `1..=N`. Contradicts the documented "trace envelope for a run" (`rpc.rs:130-132`, `sdk.rs:118-120`). Two consequences: unbounded memory growth + re-serialization of all prior runs in a long-lived RPC session; and a client cannot scope to one run except by filtering on `run_id`. The supported-path test only runs once. Medium.

#### H7. Bedrock credential-presence check ignores config-file / profile sources **[workflow-verified, is_real:true → low]**
- `doctor.rs:605-610` hard-codes the Bedrock probe to `AWS_ACCESS_KEY_ID` only, while `main.rs` Bedrock resolution also accepts `config.providers.bedrock.access_key_id`, an AWS shared-credentials profile, and config-driven env names. A user whose Bedrock provider works via those paths sees a misleading `bedrock credentials not set` Warning. Presence-only, exit 0, no runtime breakage. Low.

### 5.3 Low / Info

- **L1. Parallel tool branch violates the trace's "single-threaded emission" assumption; file write order can diverge from sequence order.** `trace.rs:31-33` documents single-threaded emission, but `agent_loop.rs:224-270` runs parallel (`ExecutionMode::Parallel`) tools via `join_all`, so concurrent `emit_inner` calls can acquire the sink lock out of sequence order. Sequence stays monotonic-by-allocation (`fetch_add` SeqCst) and sinks are `Mutex`-guarded (no torn lines), so a post-mortem reader can re-sort on `sequence`. The `file_sink_writes_versioned_jsonl_in_order` test uses a single sequential tool. **[workflow-verified, is_real:true → low]**
- **L2. `CONTENT_SENSITIVE_KEYS` omits common sensitive field spellings.** `diagnostic.rs:214-224` covers `prompt/tool_output/tool_result/env/command/args/cwd` but not `body/response/headers/request/text/content/output/stdout/stderr`. Opaque non-prefix credentials under those keys survive summary mode. **[workflow-verified, is_real:true → low]** (The redactor's value patterns still catch prefix-bearing secrets anywhere; this is about opaque values.)
- **L3. `ABSOLUTE_PATH_RE` is a fixed allowlist of POSIX roots.** `diagnostic.rs:231-236` matches `Users|home|root|tmp|var|etc|opt|mnt|private|proc|sys|dev|srv|lib|run|app|data|usr|bin|sbin`, Windows drive letters, and UNC. Roots like `/Applications`, `/Library`, `/System` (macOS), `/workspace`, `/builds`, `/code`, `/secret` are **not** matched; the `path` key is also not in `CONTENT_SENSITIVE_KEYS`, so a config path under an unlisted root survives summary redaction. Heuristic by design and documented as such, but no test pins the false-negative set. **[auditor-verified]**
- **L4. Dead code: `let _ = turn_idx;`** at the end of the agent-loop turn body (`agent_loop.rs:494`) — `turn_idx` is already used earlier in the body; the trailing no-op serves no purpose. **[workflow-verified, is_real:true → low]**
- **L5. `finish()` flushes the sink before disabling.** `trace.rs:204-207`. A concurrent emit *could* write after `finish()` under a violated single-threaded model — but the verify subagent **refuted** the race premise for the actual wiring (`is_real:false → info`). Swap to `disable()` before `sink.finish()` to tighten the invariant. **[refuted as a defect; info]**
- **L6. `doctor --json` silently drops entries on serialization failure** (`filter_map … .ok()`, `doctor.rs:265-270`) and the RPC `trace` command maps per-record failure to JSON `null` (`rpc.rs:775`). Both benign today (types always serialize) but lossy-by-design rather than failing. **[unverified-by-skeptic]**
- **L7. Abort-shutdown timeout event is short-circuited.** `rpc.rs:514-525`: on timeout, `ok && emit(&timeout_event)` drops the informative "did not stop before shutdown timeout" event when `ok` is false. Operator sees only the generic "task failed". No crash. **[unverified-by-skeptic]**
- **L8. `--scope ""` is treated as "all scopes" silently** (`doctor.rs:95-100,200-204`); a stray space (`--scope " "`) runs everything instead of erroring. Documented. **[unverified-by-skeptic]**
- **L9. RPC/JSON conversation event stream is unredacted by design** (`rpc.rs:371` → `sdk.rs:288-296`). Prompt text, provider error strings, and tool output flow verbatim to stdout. Pre-existing; the test suite explicitly carves the conversation stream out of redaction assertions. Flagged only so the trace-redaction claims are not over-read as covering the event stream. Info.
- **L10. Trace redaction tests assert fields the production loop never traces.** `trace_envelope.rs:250/281/313` attach `prompt`/`tool_output`/`workspace`/`api_key` to trace details, but the agent loop only ever attaches `tool_name`/`phase`/retry counters. These exercise the shared `redact()` in isolation, not the trace surface. (Tool arguments — the most secret-prone data — are never traced at all; safety is by omission, not redaction, and is untested.) **[workflow-verified, is_real:true → low]**
- **L11. Runtime-emission tests don't drive the real tool-failure paths.** `CODE_TOOL_VALIDATION_FAILED`/`_EXECUTION_FAILED`/`_HOOK_FAILED`/`_PROVIDER_CAPABILITY_INVALID` are pinned by classification tests, not by driving a schema-invalid call, a failing `Tool::execute`, or a `Deny` hook through `agent_loop`. Call sites exist; emission is only indirectly verified. Info.
- **L12. Whole-second trace timestamp.** `trace.rs:231` (`unix_timestamp()`). No intra-second ordering signal beyond `sequence`. Info.
- **L13. SC8 guard is static.** `observability_docs.rs:283-293` greps `Cargo.toml` for a hand-maintained telemetry-crate blocklist and `main.rs` for subscriber-install calls. A hand-rolled egress path using a non-blocklisted crate would evade it. Mitigated for Phase 7 by a direct grep for network-egress APIs in Phase 7 files (none found), but the guard does not enforce that invariant. **[workflow finding]**

---

## 6. The `Diagnostic.message` redaction gap (cross-cutting theme)

C1, C2, H1, H2, H3, H4 all share a root: **the redaction contract applies to `details` only, and `message` — which routinely carries provider bodies, package paths, and adapter/discovery strings — is passed through verbatim everywhere.** The design says diagnostics "must avoid secrets … and absolute paths outside the relevant workspace," but the implementation's redaction boundary excludes the field most likely to carry them. Phase 7 treated this as "human-readable, formatted near the CLI," which is the right *layering* instinct but the wrong *safety* outcome. Any Phase 8 hardening should either (a) route `message` through the redactor at every public boundary, or (b) stop putting dynamic/path/secret-prone content into `message` (build structured `details` instead, as `diagnostic_from_config` already does for the config path).

---

## 7. Residuals (ledger R1/R2/R3)

The ledger records three non-blocking residuals. The audit's read:

- **R1 — "harness startup surfaces adapter degradation as metadata strings, not typed Diagnostic+observe()."** **Confirmed, and understated.** It is not merely a typing concern; it is a live un-redacted leak surface on three boundaries (H4). Recommend treating as a real defect, not a cosmetic.
- **R2 — "aggregate rollups (`StartupDiagnostics Vec<String>`, `SessionDiagnosticCounts`) remain pre-0.x wire shapes pending Phase 8."** Confirmed. `StartupDiagnostics` as `Vec<String>` is also what enables H4/H5; converting it to `Vec<Diagnostic>` in Phase 8 closes both at once.
- **R3 — "schema versions stay unstable-0.x (`TRACE_SCHEMA_VERSION=1`, `SDK_SCHEMA_VERSION=2`)."** Confirmed (`trace.rs:54`, `sdk.rs:42`), consistent with the design.

The audit adds one residual the ledger does **not** record: **session-recovery diagnostics are dead code in production** (§4), and compaction does not reach the in-process `DiagnosticSink`. These should join the Phase 8 list.

---

## 8. Recommended Fixes (prioritized)

1. **(Critical) Fix the secret-redaction regexes for real key formats** — add `-` to the character classes in `streaming_proxy.rs:336,348` (e.g. `sk-ant-[a-zA-Z0-9-]{20,}`, `sk-[a-zA-Z0-9-]{20,}`). Consider anchoring on the prefix + body separately to avoid over-matching. **Re-test with real-format fixtures** (`sk-ant-api03-…`, `sk-proj-…`, `sk-live-…`, `sk-svcacct-…`) — the current synthetic fixtures must be replaced or augmented.
2. **(High) Stop leaking via `Diagnostic.message`.** Either route `message` through the redactor at `doctor --json`/text and other public boundaries, or stop embedding paths/bodies/URLs in `message` (move to structured `details`). At minimum, scrub `package_resolver` path-bearing messages and discovery-error strings.
3. **(High) Make the RPC `trace` command reachable** from `opi --rpc` (thread an optional recording sink / `--trace-for-rpc` into `run_rpc` → `new_with_runtime_packages`), and add an integration test driving the real RPC path. Restore SC4 to `met`. (Either that, or remove the `docs/opi-spec.md` claim of an RPC `trace` command.)
4. **(Medium) Wire session-recovery and compaction into the in-process `DiagnosticSink`**, not just trace, so `diagnostic_counts` and non-traced runs see them. Closes the 7.2 acceptance scenario for real.
5. **(Medium) Convert `StartupDiagnostics`/adapter degradation from `Vec<String>` to `Vec<Diagnostic>`** routed through `observe()`/`redact()` (R1/H4/H5). Closes SC5.
6. **(Medium) Clear the `RecordingTraceSink` per run** (or scope the RPC `trace` response to a run_id) so the envelope is per-run and memory is bounded (H6).
7. **(Low) Test hygiene.** Replace rubber-stamp redaction fixtures with real-format keys and path-bearing messages (H5, L10); add the real tool-failure-path emission tests (L11); pin the `ABSOLUTE_PATH_RE` false-negative set (L3).
8. **(Low)** Remove `let _ = turn_idx;` (L4); swap `finish()`/`disable()` order (L5); broaden `CONTENT_SENSITIVE_KEYS` (L2); fix the Bedrock probe (H7) and the abort-shutdown timeout event (L7).

---

## 9. Phase 8 Handoff Notes

- The shared `Diagnostic` shape and trace envelope are sound foundations; Phase 8 should **stabilize the wire shapes** (`StartupDiagnostics`, `SessionDiagnosticCounts`, `run_summary`) and in doing so fold the `Vec<String>` startup channel into typed `Diagnostic`s — that single change closes R1, R2, H4, and H5 together.
- **Do not treat the current SC6 test suite as evidence that redaction works.** It proves redaction of synthetic inputs; Phase 8 must prove redaction of real-format credentials (C1/C2) before any "stable" claim.
- The trace's single-threaded-emission invariant (L1) is false under the parallel-tool path; if Phase 8 freezes the envelope ordering, either document "sort by `sequence`, not file order" or serialize trace emission.
- The static non-goal guards (L13) should grow a network-egress grep if observability is ever broadened.

---

## 10. 中文摘要

本次由 GLM-5.2 对 Phase 7（可观测性/可靠性）进行独立审计：独立重跑全部质量门禁、并以 55 个子代理的工作流逐任务核验台账、逐条比对设计文档的 8 条成功标准与 9 条非目标、对每个表面做对抗式代码审查与逐项复核，再由审计者亲自通读安全关键文件复核重要结论。

**总体结论：架构正确、测试充分、台账在提交级别准确，但"8 条成功标准全部满足"被高估，核心脱敏控件存在凭据泄露缺陷。**

- **门禁全部通过**（fmt/clippy/doc/全工作区测试 0 失败）。smoke 首次失败是 Phase 5 的 `adapter_host_mock` 测试在 Windows 上泄露了孤儿进程占用 exe 导致链接器 `LNK1104`，与 Phase 7 无关；清理后 smoke 干净通过（64 项）。
- **2 个严重（Critical）缺陷**：脱敏正则只匹配无内部连字符的合成/旧式密钥，对真实密钥格式——Anthropic `sk-ant-api03-`/`sk-ant-api06-`、OpenAI `sk-proj-`/`sk-live-`/`sk-svcacct-`——**完全不脱敏**（已用等价正则实测确认）。而 SC6 守护测试恰好用的是合成形式，因此"测试通过"但真实密钥会泄露。
- **SC4 仅部分满足**：RPC `trace` 命令在生产中不可达（`run_rpc` 硬编码 `trace_sink: None`），真实 `opi --rpc` 用户永远收到 `unsupported_trace_request`，与文档承诺矛盾。
- **SC5 仅部分满足**：适配器降级从未类型化（`SOURCE_ADAPTER` 是死常量），启动/适配器失败以 `Vec<String>` 形式绕过脱敏，在 rpc_ready 头、NDJSON `StartupDiagnostics`、系统提示词三条路径上原样外发（残留 R1，比台账描述更严重）。
- **`Diagnostic.message` 从不脱敏**（文档标注为有意），是 H1/H2/H3/H4 的共同根源：provider 错误体、包路径（含系统用户名）、带凭据的 URL 经 message 原样输出。
- **两处 7.2 DoD 在生产中并未真正闭环**：会话恢复诊断 `CrashRecovery::diagnostics()` 仅有测试调用者（生产恢复路径从不发射）；压缩只写入 trace，不写入进程内 `DiagnosticSink`，因此运行摘要的 `diagnostic_counts` 不含压缩事件。
- **非目标（9 条）全部遵守**；台账 7.1–7.6 在所声称的提交上均可核验。

**优先修复**：(1) 修正密钥脱敏正则以匹配真实格式并用真实格式重测；(2) 堵住 `Diagnostic.message` 泄露（在公共边界对 message 脱敏，或改为结构化 `details`）；(3) 让 RPC `trace` 命令可达；(4) 将会话恢复/压缩接入进程内 sink；(5) Phase 8 在稳定线协议时把 `StartupDiagnostics` 由 `Vec<String>` 改为 `Vec<Diagnostic>`，可一并关闭 R1/R2/H4/H5。

完整发现（含文件:行号、证据、复核状态）见英文正文第 5 节。
