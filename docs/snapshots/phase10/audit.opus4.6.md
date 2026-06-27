 # Phase 10 Core Architecture Deepening -- Independent Code Audit

**Auditor:** Opus 4.6 (Cursor Agent)
**Date:** 2026-06-26
**Commit range:** `f0f3e0c..d16c364` (7 commits)
**Scope:** 19 files changed, +4527 / -841 lines

---

## Audit Metadata

| Key | Value |
|-----|-------|
| Audit type | Independent code review (no prior reports read) |
| Codebase state | Post-Phase 10 completion, all 7 tasks passing |
| Methodology | Full-file reads of all new/changed source + tests, git diff analysis, 5 parallel deep-dive agents |
| Constraints | Code/test/config/doc/diff only; no reference to existing review artifacts |

---

## Audit Scope

Phase 10 implements four workstreams across 7 commits:

| Task | Title | Crate | Commit |
|------|-------|-------|--------|
| 10.1 | Provider collection/auth seam | opi-ai | 4a9916b |
| 10.2 | Coding-agent provider factory routing | opi-coding-agent | c069853 |
| 10.3 | Generic AgentHarness core seam | opi-agent | a644adc |
| 10.4 | CodingHarness wrapper integration | opi-coding-agent | d4fbdb7 |
| 10.5 | Session repo/facade boundaries | opi-agent | 8c946a2 |
| 10.6 | Runtime hook boundary hardening | workspace | 4015ffe |
| 10.7 | Docs guards and final Phase 10 gates | workspace | d16c364 |

Key new files:

- `crates/opi-ai/src/provider_collection.rs` (380 lines)
- `crates/opi-agent/src/harness.rs` (873 lines)
- `crates/opi-coding-agent/src/provider_factory.rs` (979 lines)
- 6 new test files (1664 lines total)

---

## Findings

### P0 -- Critical Logic Defects

#### F-01: `last_entry_id` advances ahead of durable state on partial flush failure

**Files:** `crates/opi-agent/src/harness.rs` L396-408, L545-556, L745-773, L834-845

`enqueue_message` and `enqueue_extension_state` update `last_entry_id` at
enqueue time, not after successful flush. `flush_internal` on partial failure
re-queues the unflushed tail but does not roll back `last_entry_id`.

Reproduction scenario:

1. Enqueue `entry-2`, flush fails at `entry-2` -- `entry-2` stays in queue,
   `last_entry_id == "entry-2"`.
2. Enqueue `entry-3` with `parent_id = "entry-2"`.
3. Retry flush: `entry-2` succeeds, `entry-3` fails -- `entry-3` re-queued,
   `last_entry_id == "entry-3"`.
4. Enqueue `entry-4` with `parent_id = "entry-3"`, but `entry-3` was **never
   persisted**.

The `parent_id` chain in the session file becomes broken. `SessionFacade` has
identical logic.

Tests only cover all-or-nothing failure (`FailingHarnessSession`), never
partial success.

**Impact:** Corrupt `parent_id` chains on IO errors; broken branch
reconstruction on resume.

**Suggested fix:** Track `last_durable_entry_id` separately from
`last_enqueued_entry_id`, or update `last_entry_id` only after successful
append in `flush_internal`.

---

#### F-02: `id_counter` / `last_entry_id` not restored on session resume -- ID collisions

**Files:** `crates/opi-agent/src/harness.rs` L348-360, L648-656, L724-740

`AgentHarness::new` and `SessionFacade::new` both initialize `id_counter = 0`
and `last_entry_id = None`. `JsonlSessionRepo::open` restores `count` (number
of entries) but not the ID counter or last entry ID.

If a session file already contains `entry-1` through `entry-N`, appending after
`open` generates new `entry-1`, `entry-2`, etc., causing **ID collisions** and
**broken parent_id links**.

No test covers "open then continue appending".

**Impact:** Session file corruption on resume through `SessionFacade` /
`JsonlSessionRepo`.

**Suggested fix:** `JsonlSessionRepo::open` (or a new `SessionFacade::open`)
should scan existing entries to initialize `id_counter` to `max(existing IDs)`
and `last_entry_id` to the trunk tip.

---

### P1 -- Design / Architecture Issues

#### F-03: Auth descriptor is decoupled from provider credentials

**File:** `crates/opi-ai/src/provider_collection.rs` L117-136, L313-326

`AuthDescriptor` checks its own state (env var set, static key non-empty) but
does **not** inject credentials into `Provider::stream`. The provider typically
reads credentials at construction time, independently. This means:

- Descriptor can report `Configured` while the provider has no valid key.
- Descriptor can report `Missing` while the provider has a hardcoded key.
- `from_registry` path (used by `assemble_harness_collection`) creates
  collections with **no** auth descriptors, so `dispatch_stream` skips auth
  gating entirely.

The design is documented, but "provider-owned auth contract" is misleading when
auth only gates diagnostics, not actual credential injection.

**Impact:** Auth status in diagnostics may not reflect actual provider auth
state.

---

#### F-04: `doctor.rs` duplicates credential env-var mapping outside centralization guard

**File:** `crates/opi-coding-agent/src/doctor.rs` L686-712

`provider_credential_env_name` implements a parallel mapping of provider ID to
env var name, duplicating `auth_descriptor_for` and `profile_api_key_env_default`
in `provider_factory.rs`. The `provider_policy_is_centralized` guard only scans
for factory function **definitions** (e.g., `fn auth_descriptor_for`), not
**credential env-var resolution logic**.

If these two mappings drift, `opi doctor` will report incorrect credential
status.

**Impact:** Credential policy not truly centralized; drift risk between factory
and doctor.

**Suggested fix:** Have `doctor.rs` call `auth_descriptor_for` /
`auth_descriptor_for_profile` instead of maintaining its own mapping.

---

#### F-05: Spec claims SessionRepo provides list/fork, but code explicitly excludes them

**File:** `docs/opi-spec.md` L1395, `crates/opi-agent/src/harness.rs` L609-614

The spec's session facade boundaries paragraph states the trait provides
"stable durable append/load/list/fork traits". The `SessionRepo` trait
rustdoc explicitly says:

> `list`/`fork` and directory policy are intentionally NOT part of this trait:
> they are coding-agent product policy.

This is a direct contradiction between normative documentation and code.
`docs/opi-spec.zh.md` L1235 has the same error.

**Impact:** Misleading spec; Phase 13 embedders may expect list/fork on the
trait.

**Suggested fix:** Remove "list/fork" from the spec description, or qualify
it as Phase 13 planned.

---

#### F-06: `pi-alignment-matrix.md` contradicts `opi-spec.md` on Phase 10 status

**File:** `docs/pi-alignment-matrix.md` L184, L209-211, L250

The alignment matrix still describes Phase 10 as "Planned", AgentHarness as
"No generic AgentHarness/session facade equivalent yet", and pending write
ordering as "Missing". Meanwhile `opi-spec.md` L1387 says "in progress;
initial seams landed".

Phase 10 guard tests do not check the alignment matrix.

**Impact:** Normative documents contradict each other.

---

#### F-07: `begin_turn` rustdoc has stale forward-pointer

**File:** `crates/opi-agent/src/harness.rs` L443-446

```rust
/// Begin an agent turn: [`Phase::Idle`] -> [`Phase::Turn`]. Freezes a
/// runtime-config snapshot for the turn. (State-machine guard only in
/// Phase 10.3; the loop itself is wired in task 10.4.)
```

Task 10.4 explicitly did NOT wire the loop (by-value wall). The module-level
doc was corrected (L18-36), but this method-level doc still promises loop
wiring in 10.4.

**Impact:** Misleading API documentation.

---

#### F-08: AgentHarness and SessionFacade have significant code duplication

**File:** `crates/opi-agent/src/harness.rs`

The following functions are near-identical between `AgentHarness` and
`SessionFacade`:

| Function | AgentHarness lines | SessionFacade lines |
|----------|-------------------|-------------------|
| `flush_internal` | 545-557 | 834-846 |
| `next_id` | 571-574 | 862-865 |
| `next_timestamp` | 576-581 | 867-872 |
| `record_save_point` | 559-568 | 848-860 |
| `enqueue_message` | 396-408 | 745-756 |
| `enqueue_extension_state` | 414-426 | 762-773 |

Approximately 100 lines of duplicated logic. A shared `PendingWriteManager`
or trait-default implementation would eliminate this.

**Impact:** Maintenance burden; bugs fixed in one copy may not be fixed in
the other.

---

#### F-09: `ProviderBuildError` does not implement `std::error::Error`

**File:** `crates/opi-coding-agent/src/provider_factory.rs` L54-88

Has `Display` and `From<ProviderError>` but no `impl std::error::Error`. This
is inconsistent with the project convention (`thiserror` for library errors)
and prevents `?`-chaining into `anyhow::Error`.

`ListModelsError` also lacks both `Display` and `Error`.

**Impact:** Error ergonomics; cannot use standard error composition.

---

#### F-10: `compat_metadata_for` returns default for all built-in providers

**File:** `crates/opi-coding-agent/src/provider_factory.rs` L871-873

```rust
pub fn compat_metadata_for(_provider_id: &str) -> CompatMetadata {
    CompatMetadata::default()
}
```

OpenRouter, Mistral, and OpenAI are OpenAI-compatible providers, but their
`CompatMetadata` has `openai_compatible: false`. Only user-declared
`openai_compatible` profiles get `true`.

Currently harmless (listing and harness don't read `compat`), but will be
incorrect if future code uses `compat.openai_compatible` for routing or
diagnostics.

**Impact:** Metadata inaccuracy; latent bug for future consumers.

---

### P2 -- Correctness / Safety Issues

#### F-11: `end_turn` / `end_compaction` set phase to Idle before flush -- settlement failure leaves turn "ended"

**File:** `crates/opi-agent/src/harness.rs` L455-462, L473-481

```rust
pub fn end_turn(&mut self) -> HarnessResult<SavePoint> {
    // ...
    self.phase = Phase::Idle;
    self.turn_snapshot = None;
    self.flush()
}
```

If `flush()` returns `HarnessError::Write`, the phase is already Idle and the
turn snapshot is cleared, but pending writes remain queued. The turn has
semantically "ended" without successful settlement.

Recoverable (caller can retry `flush()`), but no test covers this path.

---

#### F-12: `active_tip()` / `load()` do not see pending queue entries

**File:** `crates/opi-agent/src/harness.rs` L807-811

`active_tip` reads only from the repo (disk), not from the pending write
queue. Calling `active_tip` after `enqueue` but before `flush` returns a
stale tip. No documentation warns about this semantic.

---

#### F-13: `MetadataProvider::stream` returns empty stream instead of error

**File:** `crates/opi-coding-agent/src/provider_factory.rs` L223-225

If accidentally dispatched through (e.g., via `ProviderCollection`), the
caller gets a silent empty stream that ends without a terminal event,
resulting in `ProviderError::StreamError("stream ended without a terminal
event")` -- a confusing error far from the root cause.

---

#### F-14: `end_turn` at `Phase::Idle` returns `Busy(Idle)` -- misleading error

**File:** `crates/opi-agent/src/harness.rs` L456-458

"operation rejected: harness is busy in phase Idle" does not communicate
"no active turn to end". A dedicated `NoActiveTurn` or `InvalidTransition`
variant would be clearer.

---

#### F-15: Whitespace-only API keys pass auth checks

**File:** `crates/opi-ai/src/provider_collection.rs` L69-71, L128-132

`SecretKey::is_present()` checks `!is_empty()` but does not trim. A key
consisting of only spaces `"   "` is reported as `Configured`.
`AuthDescriptor::EnvApiKey` has the same issue. The runtime
`require_api_key` in `provider_factory.rs` does trim, creating an
inconsistency between auth status and actual provider validation.

---

#### F-16: `EnvApiKey` error message conflates "unset" with "empty" and "invalid UTF-8"

**File:** `crates/opi-ai/src/provider_collection.rs` L128-132

All three cases produce `"env var {env_var} is not set"`, which is inaccurate
for empty or non-UTF-8 values. The DoD mentions "missing/invalid auth
diagnostics", but there is no `AuthStatus::Invalid` variant.

---

#### F-17: `snapshot()` silently swallows IO errors on `message_count`

**File:** `crates/opi-agent/src/harness.rs` L381

```rust
message_count: self.session.message_count().unwrap_or(0),
```

If the session backend fails, the snapshot silently reports 0 messages, which
could mislead compaction threshold checks or monitoring.

---

#### F-18: `abort` discards the original IO error

**File:** `crates/opi-agent/src/harness.rs` L512-515

```rust
Err(_) => Err(HarnessError::AbortLeftPending(self.queue.len())),
```

The root cause IO error is dropped, making debugging difficult.
`SessionFacade::abort` (L792-795) has the same issue.

---

#### F-19: `SavePoint.at_phase` is always `Phase::Idle` in practice

**File:** `crates/opi-agent/src/harness.rs` L559-565

All flush paths either require Idle or set phase to Idle before
`record_save_point`. The field has no discriminating power between turn
settlement and idle flush. `SessionFacade` hardcodes `Phase::Idle` (L852-854).

---

### P3 -- Test Coverage Gaps

#### F-20: Partial flush failure untested

Neither `tests/harness.rs` nor `tests/session_facade.rs` tests partial flush
(first entry succeeds, second fails). This is the scenario that triggers F-01.

---

#### F-21: Session resume append untested

No test covers `JsonlSessionRepo::open` followed by further `append` calls.
This is the scenario that triggers F-02.

---

#### F-22: `BranchSummary` phase busy rejections untested

`tests/harness.rs` tests Turn and Compaction busy rejections but not
BranchSummary. Missing tests:

- `enqueue_*` during BranchSummary
- `begin_turn` during BranchSummary
- `end_branch_summary` during Idle
- `end_compaction` during Idle/Turn
- `abort` during Compaction/BranchSummary

---

#### F-23: `provider_factory.rs` tests do not test factory functions

**File:** `crates/opi-coding-agent/tests/provider_factory.rs`

File header says "all 6 providers" but tests directly call `opi_ai::*::new`,
not `build_provider` or `build_collection_for_listing`. Only
`provider_factory_routes_through_collection` exercises a factory function, and
only for `openai_compatible` profiles.

Missing: per-provider `build_provider` tests, `parse_model_spec` edge cases,
`require_api_key` whitespace behavior, `auth_descriptor_for` coverage for all
9 built-in IDs.

---

#### F-24: `drain_to_completion` branches untested

**File:** `crates/opi-ai/src/provider_collection.rs` L364-379

No test covers:

- `CompletedRequest::Error` (from `AssistantStreamEvent::Error`)
- Stream ending without terminal event (`StreamError`)
- Stream `Item = Err(ProviderError)` propagation
- Multiple terminal events (only first is returned)

---

#### F-25: `from_registry` dispatch path untested

`collection_wraps_existing_registry_via_from_registry` tests lookup but not
`dispatch_stream` or `dispatch_complete`. No test proves that dispatch through
`from_registry` skips auth gating.

---

#### F-26: ZH guard coverage is asymmetric with EN

**File:** `crates/opi-coding-agent/tests/productized_packages_docs.rs` L1648-1657

`phase10_runtime_hook_boundaries` checks all 6 surfaces in EN but only 4 in
ZH. "Generic harness events/results" and "Coding-agent extension registry"
are not enforced in ZH. ZH can silently regress.

---

#### F-27: Exit trace completeness test is tautological

**File:** `crates/opi-coding-agent/tests/productized_packages_docs.rs` L1955-1971

```rust
let trace: [(&str, &str); 8] = [
    ("SC1 provider collection/auth seam", "met"),
    // ... all hardcoded "met"
];
for (criterion, status) in trace {
    assert_eq!(status, "met", ...);
}
```

This loop **always passes**. It does not read `.opi-impl-state.json`, does
not run smoke tests, and does not cross-reference contract tests. The test
name "completeness" and "exit trace" is misleading -- it is a documentation
presence check with a cosmetic status attestation.

---

### P4 -- Code Quality

#### F-28: `strip_rust_comments` duplicated 3 times

Three nearly-identical implementations exist:

- `crates/opi-coding-agent/tests/productized_packages_docs.rs` L1536-1590
- `crates/opi-agent/tests/session_facade.rs` L258-314
- `crates/opi-coding-agent/tests/harness_resource_integration.rs`

None handle raw strings (`r#"..."#`) or byte strings (`b"..."`).

---

#### F-29: `HarnessSession` and `SessionRepo` traits lack `Send` bound

Neither trait requires `Send` or `Sync`, limiting future async/multi-threaded
usage. No documentation states the thread-safety contract.

---

#### F-30: `SecretKey` missing `PartialEq` derive

Tests must use `assert_eq!(key.as_str(), ...)` instead of comparing keys
directly. Also missing: `Eq`, `Hash`. No `Zeroize` on drop.

---

#### F-31: Listing vs runtime API key whitespace handling inconsistent

**File:** `crates/opi-coding-agent/src/provider_factory.rs`

- Runtime: `require_api_key` trims and rejects whitespace-only keys.
- Listing: `std::env::var` success is sufficient, no trim.

A whitespace-only key passes listing but fails at runtime.

---

#### F-32: `build_collection_for_listing` `MissingCredentials` return is unreachable in `main.rs`

**File:** `crates/opi-coding-agent/src/main.rs` L554

`build_collection_for_listing` silently skips providers with missing
credentials and returns `Ok(empty_collection)`. The `MissingCredentials`
match arm in `main.rs` is dead code.

---

#### F-33: Bedrock credential resolution duplicated between listing and runtime

**File:** `crates/opi-coding-agent/src/provider_factory.rs`

`build_bedrock` (L364-400) and the bedrock arm of `build_runtime_provider`
(L704-754) share ~50 lines of nearly-identical credential resolution logic
with different error types.

---

#### F-34: `use serde::Serialize` unused in `crates/opi-coding-agent/src/harness.rs`

**File:** `crates/opi-coding-agent/src/harness.rs` L42

The import is present but all `#[derive(Serialize)]` use the full path. Not
introduced by Phase 10 but persists in the working tree.

---

## Per-Workstream Assessment

### WS10.1: Provider Collection / Auth Seam

**Verdict: Structurally sound; auth model is diagnostic-only, not credential-injecting.**

The `ProviderCollection` wrapping `ProviderRegistry` is well-designed.
`SecretKey` redaction works correctly at Debug/Display time. The
`#[non_exhaustive]` `AuthDescriptor` properly leaves room for future OAuth.

Key concerns:

- Auth descriptors are decoupled from actual provider credentials (F-03).
- `from_registry` bypasses auth gating entirely (documented but significant).
- Whitespace-only keys pass as Configured (F-15).
- `drain_to_completion` and error-path branches lack test coverage (F-24).

Test quality: Good for happy paths and redaction; missing error paths and
edge cases.

### WS10.2: Provider Factory + CodingHarness Wrapper

**Verdict: Successful extraction; no runtime regression detected.**

The ~726-line extraction from `main.rs` to `provider_factory.rs` preserves
all existing behavior. The centralization guard is a useful regression net.
`CodingHarness` documentation clearly establishes the product/generic
boundary.

Key concerns:

- `doctor.rs` maintains a parallel credential mapping outside the guard (F-04).
- `ProviderBuildError` lacks `std::error::Error` (F-09).
- `compat_metadata_for` returns default for OpenAI-compatible built-ins (F-10).
- Factory tests do not test factory functions (F-23).
- `begin_turn` rustdoc has stale forward-pointer (F-07).

Test quality: Centralization guard and routes-through-collection test are
effective; per-provider factory function coverage is absent.

### WS10.3: Generic AgentHarness + Session Repo/Facade

**Verdict: State machine is correct for tested paths; two critical bugs in
ID/parent-chain management.**

The phase state machine (Idle/Turn/Compaction/BranchSummary) correctly rejects
structural operations while busy. Turn snapshot discipline (freeze at begin,
unfreeze at end) works as documented. Pending-write ordering (agent before
extension) is correct and tested.

Critical concerns:

- `last_entry_id` advances ahead of durable state on partial flush (F-01).
- `id_counter` not restored on session resume (F-02).

Additional:

- Significant code duplication between AgentHarness and SessionFacade (F-08).
- BranchSummary phase guards untested (F-22).
- `active_tip` does not see pending queue (F-12).

Test quality: Strong for main happy path and all-or-nothing failure; missing
partial failure, resume, and BranchSummary phase tests.

### WS10.4: Runtime Hook Boundaries

**Verdict: Documentation and guard tests establish the boundary model;
enforcement is medium-strength.**

The 6-surface boundary model table in the spec accurately reflects the
current code architecture (with minor inaccuracies around `ExtensionRegistry`
ownership description). Guard tests prevent common structural regressions.

Key concerns:

- Spec claims SessionRepo has list/fork, code explicitly excludes them (F-05).
- `pi-alignment-matrix` contradicts spec on Phase 10 status (F-06).
- ZH guard coverage is asymmetric (F-26).
- Exit trace completeness test is tautological (F-27).
- Guard tests can be bypassed by token renaming or semantic leakage.

Test quality: Adequate for doc regression prevention; not a formal
architectural enforcement mechanism.

---

## Success Criteria Verification

| SC | Description | Assessment |
|----|-------------|------------|
| SC1 | `opi-ai` has documented provider collection/auth seam | **Met.** `ProviderCollection` with auth/compat/dispatch. Auth model is diagnostic-only (F-03). |
| SC2 | Provider construction in `opi-coding-agent` routes through the seam | **Met.** `provider_factory.rs` centralizes construction. `doctor.rs` has parallel mapping (F-04). |
| SC3 | `opi-agent` owns generic harness with phase/snapshot/save-point/pending-write | **Met.** `AgentHarness` with full state machine. Two critical ID bugs (F-01, F-02). |
| SC4 | `CodingHarness` documented as product wrapper | **Met.** Module and struct docs establish boundary clearly. |
| SC5 | Session repo/facade boundaries defined for Phase 13 | **Met.** `SessionRepo` + `SessionFacade` with 4 documented decisions. Spec inaccuracy (F-05). |
| SC6 | Runtime hook boundaries distinguish current/future surfaces | **Met.** 6-surface table in spec; guard tests enforce EN. ZH gap (F-26). |
| SC7 | Existing behavior covered by regression tests | **Partially met.** Existing startup/session/RPC tests pass. New seam tests have coverage gaps (F-20 through F-27). |
| SC8 | No ecosystem breadth feature added | **Met.** No OAuth, image gen, custom UI, npm, or browser work. |

---

## Non-Goal Compliance

All 11 Phase 10 non-goals verified absent:

| Non-goal | Status |
|----------|--------|
| Provider OAuth login | Not implemented |
| Anthropic/OpenAI Codex/GitHub Copilot subscription auth | Not implemented |
| Broad provider catalog expansion | Not implemented |
| Image generation | Not implemented |
| Custom TUI extension protocol | Not implemented |
| npm/package marketplace | Not implemented |
| Browser/web UI | Not implemented |
| `pi` TypeScript extension API compatibility | Not implemented |
| `pi` session file compatibility | Not implemented |
| Shared `opi-types` crate | Not created |
| Whole-loop rewrite | Not performed |

---

## Overall Assessment

Phase 10 **successfully establishes three core architectural seams** (provider
collection/auth, generic harness, session repo/facade) and **centralizes
provider construction** without runtime regression. The documentation clearly
establishes the product/generic boundary, and guard tests provide structural
protection against common regressions.

**Two P0 bugs** (F-01, F-02) affect session durability under error conditions
and on resume. These are in the new `harness.rs` code and have not yet
been exercised by production paths (CodingHarness still uses
`SessionCoordinator`), but will become critical when the product wrapper
adopts `AgentHarness` / `SessionFacade`.

**Key technical debt:**

1. Code duplication between `AgentHarness` and `SessionFacade` (~100 lines).
2. Credential mapping duplication between `provider_factory.rs` and `doctor.rs`.
3. Listing/runtime provider builder duplication (~400+ lines).
4. `strip_rust_comments` helper duplicated 3 times across test files.
5. Test coverage gaps in error paths, resume scenarios, and factory functions.

**Documentation issues:**

1. Spec/code contradiction on SessionRepo list/fork (F-05).
2. Spec/alignment-matrix contradiction on Phase 10 status (F-06).
3. Stale `begin_turn` rustdoc forward-pointer (F-07).
4. Exit trace completeness test does not verify implementation (F-27).

**Recommendations (priority order):**

1. Fix F-01 and F-02 before any production adoption of `AgentHarness` /
   `SessionFacade`.
2. Add partial-flush-failure and resume-append tests.
3. Correct spec to remove list/fork from SessionRepo description.
4. Update `pi-alignment-matrix.md` Phase 10 row.
5. Have `doctor.rs` reuse `auth_descriptor_for` to eliminate credential
   mapping duplication.

---

## Finding Index

| ID | Severity | Title | File(s) |
|----|----------|-------|---------|
| F-01 | P0 | `last_entry_id` advances ahead of durable state | `harness.rs` L396-408, L545-556 |
| F-02 | P0 | `id_counter`/`last_entry_id` not restored on resume | `harness.rs` L348-360, L648-656 |
| F-03 | P1 | Auth descriptor decoupled from provider credentials | `provider_collection.rs` L117-136 |
| F-04 | P1 | `doctor.rs` duplicates credential mapping | `doctor.rs` L686-712 |
| F-05 | P1 | Spec claims SessionRepo has list/fork | `opi-spec.md` L1395 |
| F-06 | P1 | Alignment matrix contradicts spec | `pi-alignment-matrix.md` L184 |
| F-07 | P1 | `begin_turn` stale forward-pointer | `harness.rs` L443-446 |
| F-08 | P1 | AgentHarness/SessionFacade code duplication | `harness.rs` |
| F-09 | P1 | `ProviderBuildError` missing `Error` trait | `provider_factory.rs` L54-88 |
| F-10 | P1 | `compat_metadata_for` returns default for compat providers | `provider_factory.rs` L871-873 |
| F-11 | P2 | `end_turn` sets Idle before flush | `harness.rs` L455-462 |
| F-12 | P2 | `active_tip` ignores pending queue | `harness.rs` L807-811 |
| F-13 | P2 | `MetadataProvider::stream` returns empty stream | `provider_factory.rs` L223-225 |
| F-14 | P2 | `end_turn` at Idle returns misleading `Busy(Idle)` | `harness.rs` L456-458 |
| F-15 | P2 | Whitespace-only keys pass auth | `provider_collection.rs` L69-71 |
| F-16 | P2 | `EnvApiKey` conflates unset/empty/invalid-UTF8 | `provider_collection.rs` L128-132 |
| F-17 | P2 | `snapshot()` swallows IO errors | `harness.rs` L381 |
| F-18 | P2 | `abort` drops original IO error | `harness.rs` L512-515 |
| F-19 | P2 | `SavePoint.at_phase` always Idle | `harness.rs` L559-565 |
| F-20 | P3 | Partial flush failure untested | tests |
| F-21 | P3 | Session resume append untested | tests |
| F-22 | P3 | BranchSummary busy rejections untested | tests |
| F-23 | P3 | Factory tests don't test factory functions | `provider_factory.rs` tests |
| F-24 | P3 | `drain_to_completion` branches untested | `provider_collection.rs` tests |
| F-25 | P3 | `from_registry` dispatch path untested | `provider_collection.rs` tests |
| F-26 | P3 | ZH guard coverage asymmetric with EN | `productized_packages_docs.rs` |
| F-27 | P3 | Exit trace completeness test tautological | `productized_packages_docs.rs` |
| F-28 | P4 | `strip_rust_comments` duplicated 3 times | test files |
| F-29 | P4 | Traits lack `Send` bound | `harness.rs` |
| F-30 | P4 | `SecretKey` missing `PartialEq` | `provider_collection.rs` |
| F-31 | P4 | Listing/runtime whitespace handling inconsistent | `provider_factory.rs` |
| F-32 | P4 | Dead `MissingCredentials` match arm in `main.rs` | `main.rs` L554 |
| F-33 | P4 | Bedrock credential resolution duplicated | `provider_factory.rs` |
| F-34 | P4 | Unused `use serde::Serialize` | `harness.rs` L42 |
