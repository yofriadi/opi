# Phase 10 Independent Code Audit - Codex

Date: 2026-06-26

Scope: independent review of Phase 10 against `docs/snapshots/phase10/opi-impl-state.json`, `docs/superpowers/specs/2026-06-24-phase10-core-architecture-deepening-design.md`, and the actual diff from `f0f3e0c58fe66f9fcc98a33fb41a0ea8b500b6d3` to `HEAD` (`7cdcfcc` at review time).

Independence note: I did not open or read existing review reports. This audit is based on code, tests, docs, config, the Phase 10 state file requested by the user, and the implementation diff.

Verification note: this was a static review. I did not run the full workspace test suite.

## Findings

### P1 - `SessionFacade` loses resume state and can append duplicate/root entries

Files:

- `crates/opi-agent/src/harness.rs:648`
- `crates/opi-agent/src/harness.rs:731`
- `crates/opi-agent/src/harness.rs:745`
- `crates/opi-agent/src/harness.rs:862`
- comparison point: `crates/opi-coding-agent/src/session_coordinator.rs:124`

`JsonlSessionRepo::open()` counts existing entries, but `SessionFacade::new()` always initializes `last_entry_id: None` and `id_counter: 0`. The next `enqueue_message()` therefore writes a new entry with `parent_id: None` and `next_id()` emits `entry-1`, even if the backing JSONL session already contains entries or an existing `entry-1`.

Cause: the facade has a load path (`active_tip()` can compute the durable tip) but its constructor does not hydrate append state from the repo. The existing product `SessionCoordinator::open_existing()` already does this correctly by selecting ordered entries and seeding `active_tip_entry_id`.

Impact: a generic library caller that opens an existing session through the new Phase 10 seam and then appends will fork a detached root and may duplicate entry IDs. That breaks deterministic branch reconstruction and undermines the Phase 13 session seam this workstream is intended to provide.

Suggested fix: split fresh/open constructors or hydrate inside `SessionFacade::new()` by loading the repo, seeding `last_entry_id` from the active content tip, and advancing the ID counter past existing facade-generated IDs. Prefer collision-resistant IDs (for example UUID/v7) if the facade is intended to append to files created by other writers. Add a regression that creates a facade session, flushes entries, reopens it with `JsonlSessionRepo::open()`, appends again, and asserts unique IDs plus a parent link to the previous active tip.

### P1 - Pending write ordering can corrupt parent chains around extension state

Files:

- `crates/opi-agent/src/harness.rs:155`
- `crates/opi-agent/src/harness.rs:414`
- `crates/opi-agent/src/harness.rs:745`
- `crates/opi-agent/src/harness.rs:762`
- `crates/opi-agent/src/session_branch.rs:65`
- `crates/opi-coding-agent/src/session_cli.rs:531`
- insufficient test assertion: `crates/opi-agent/tests/session_facade.rs:215`

`PendingWriteQueue::drain_ordered()` always flushes `AgentMessage` before `ExtensionState`. However, both `AgentHarness::enqueue_extension_state()` and `SessionFacade::enqueue_extension_state()` advance `last_entry_id` to the extension-state entry. If extension state is enqueued first and a message second, the message's `parent_id` points at the extension-state ID, while the flush order writes the message before the extension state.

That is not just a file-order oddity. Branch reconstruction ignores `SessionEntry::ExtensionState` in both `session_branch.rs` and the coding-agent active-branch walker. A message parented to an extension-state entry is therefore treated as having a missing parent. The existing test intentionally enqueues extension state first, then a message, but only asserts durable order; it never checks the resulting `parent_id` graph.

Impact: any caller using the new generic pending-write seam can produce a session where active-branch reconstruction stops at the first message parented through extension state. With a leaf pointing to that message, resume can drop earlier context from the active branch.

Suggested fix: treat extension state as a sidecar attached to the current content tip, not as the next content tip. In practice, `enqueue_extension_state()` should use `last_entry_id` as its `parent_id` but should not update `last_entry_id`. Alternatively, include extension states in branch reconstruction and make flush order topologically consistent, but that is a larger semantic change. Add tests that assert parent IDs and active-branch reconstruction when extension state is enqueued before and after content messages.

### P2 - Bedrock auth diagnostics and dispatch disagree for non-env credentials

Files:

- `crates/opi-coding-agent/src/provider_factory.rs:364`
- `crates/opi-coding-agent/src/provider_factory.rs:837`
- `crates/opi-coding-agent/src/provider_factory.rs:900`
- `crates/opi-ai/src/provider_collection.rs:313`

`build_bedrock()` accepts credentials from config, environment, AWS profile, and shared credentials/config files. But `auth_descriptor_for("bedrock")` always returns `EnvApiKey { env_var: "AWS_ACCESS_KEY_ID" }`. `build_collection_for_listing()` registers the successfully built Bedrock provider with that descriptor, and `ProviderCollection::dispatch_stream()` gates dispatch on `auth_status()`.

Cause: the descriptor collapses a multi-source credential chain to one env var. The comment says the Bedrock descriptor "does not gate dispatch", but the collection gates every registered descriptor in `dispatch_stream()`.

Impact: a Bedrock provider can be successfully constructed from profile/config credentials, then be rejected by collection dispatch as `AuthNotConfigured` when `AWS_ACCESS_KEY_ID` is absent. This directly weakens the Phase 10 provider collection/auth seam and the Phase 12 fixture target that should test providers through collection-level auth and dispatch.

Suggested fix: make Bedrock auth status reflect the same credential-resolution result used to build the provider. Options: add an auth descriptor variant for AWS credential chains, attach a configured/static descriptor after successful resolution, or skip collection auth gating for Bedrock until a real multi-source descriptor exists. Add a test where Bedrock resolves from a profile/config path with no `AWS_ACCESS_KEY_ID`, then `auth_status()` and `dispatch_stream()` agree.

### P2 - `assemble_harness_collection()` returns a dispatch-capable type with a non-dispatchable active provider

Files:

- `crates/opi-coding-agent/src/provider_factory.rs:196`
- `crates/opi-coding-agent/src/provider_factory.rs:223`
- `crates/opi-coding-agent/src/provider_factory.rs:935`
- `crates/opi-coding-agent/src/provider_factory.rs:961`
- `crates/opi-ai/src/provider_collection.rs:335`
- `crates/opi-ai/src/provider_collection.rs:364`

`assemble_harness_collection()` wraps the active runtime provider in `MetadataProvider`, whose `stream()` returns an empty stream. The function then returns `ProviderCollection::from_registry(registry)`. To callers, that looks like a normal collection: `resolve()` succeeds for the active provider, but `dispatch_complete()` drains the empty stream and returns `stream ended without a terminal event`.

Cause: the same `ProviderCollection` type is used for a real dispatch seam and for a metadata-only model registry. The distinction is documented in comments, but not represented in the type system.

Impact: SDK/test callers can receive a `ProviderCollection` that satisfies lookup but fails dispatch for the active provider. This contradicts the collection's stream/complete dispatch contract and can mislead future provider correctness fixtures.

Suggested fix: either return a metadata-only registry/view from `assemble_harness_collection()`, or make the active provider shareable so the collection contains the real dispatchable provider. If a dummy provider remains necessary, make dispatch fail with an explicit unsupported error and do not expose it as the same collection used for provider dispatch fixtures.

### P2 - Session repo/facade contract and docs disagree about list/fork ownership

Files:

- design requirement: `docs/superpowers/specs/2026-06-24-phase10-core-architecture-deepening-design.md:162`
- normative doc claim: `docs/opi-spec.md:1395`
- implementation: `crates/opi-agent/src/harness.rs:608`
- guard: `crates/opi-agent/tests/session_facade.rs:341`

The Phase 10 design and `opi-spec.md` say the `Session repo/facade` workstream provides stable durable append/load/list/fork traits. The implementation explicitly says `list`/`fork` are not part of `SessionRepo`, and the guard test treats `fork_session`, `list_sessions`, and related tokens as product-policy leaks into `opi-agent`.

Cause: the implementation made a narrower ownership decision than the Phase 10 design/docs/DoD still claim.

Impact: this can mislead Phase 13 work. A future implementer may assume a generic list/fork seam exists in `opi-agent` when it does not, while the tests currently enforce its absence. It also makes the Phase 10 exit trace overstate what the session seam provides.

Suggested fix: choose one contract and make all artifacts agree. If list/fork are generic, add them to an `opi-agent` repo/facade trait with product directory policy supplied by `opi-coding-agent`. If they are deferred/product-owned, update the design trace and `opi-spec.md` to say Phase 10 delivered append/load/read-write ordering only, with generic list/fork explicitly deferred or excluded.

### P3 - New env-mutating provider factory test is not serialized

Files:

- `crates/opi-coding-agent/tests/provider_factory.rs:346`
- `crates/opi-coding-agent/tests/provider_factory.rs:374`
- local precedent: `crates/opi-ai/tests/proxy_support.rs:18`

`provider_factory_routes_through_collection()` mutates the process environment with `std::env::set_var()` and `remove_var()` but has no serialization guard. The repository instructions require tests that mutate process environment variables to be serialized, and existing proxy tests use a static mutex for this reason.

Impact: this is mostly a test-isolation issue because the env var is unique, but it still violates the repository rule and can leak the variable if the test panics before cleanup.

Suggested fix: wrap the mutation in an env guard protected by a static mutex, following `proxy_support.rs`, or refactor the factory test to avoid process-global env mutation by injecting an env lookup function.

## Summary

The Phase 10 implementation establishes the intended seams, but the session facade is not yet safe for resumed append workflows, and the pending-write ordering contract can create invalid parent graphs. The provider collection seam also has two type/diagnostic mismatches: Bedrock auth descriptors do not match its credential chain, and a metadata-only provider is exposed through a dispatch-capable collection type. These should be addressed before Phase 13 or Phase 12 builds on the new seams.
