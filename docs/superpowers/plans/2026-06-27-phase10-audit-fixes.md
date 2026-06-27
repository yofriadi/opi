# Phase 10 Audit Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the confirmed Phase 10 audit issues without broadening Phase 10 scope or rerouting the whole product loop.

**Architecture:** Treat Phase 10 seams as published unstable library surfaces, not fully adopted production paths. First make the new session and provider seams internally safe under resume, sidecar state, and dispatch misuse; then make docs and guard tests accurately state which seams are published, which are production-adopted, and which are deferred.

**Tech Stack:** Rust 2024, Cargo workspace, `thiserror`, `uuid` workspace dependency, `tempfile`, `MockProvider`, JSONL session helpers, existing docs guard tests.

## Global Constraints

- Do not implement OAuth, subscription auth, image generation, broad provider catalog expansion, custom TUI extension protocol, npm/gallery workflows, browser/web UI, `pi` TypeScript API compatibility, `pi` session import, shared `opi-types`, or a whole-loop rewrite.
- Do not move CLI/session directory/package policy into `opi-agent`.
- Keep v1 session files readable.
- Update English and Chinese normative docs together.
- Do not hand-edit `.opi-impl-state.json`; if ledger correction is required, use the implementation-state workflow or leave a reviewed snapshot note.
- After code changes, run `cargo clippy --workspace --all-targets -- -D warnings`.
- If a test file is modified, run that exact test target.
- Do not commit unless explicitly asked.

---

## Verification Summary

Confirmed:

| Area | Status | Evidence |
|---|---|---|
| `SessionFacade` resume append can duplicate IDs and lose parent tip | Confirmed | `SessionFacade::new` initializes `last_entry_id = None`, `id_counter = 0`; `JsonlSessionRepo::open` only counts entries. |
| Extension state can become a content parent | Confirmed | `enqueue_extension_state` advances `last_entry_id`; branch reconstruction ignores `ExtensionState`. |
| Pending-write partial failure is under-tested | Confirmed gap, impact tied to sidecar semantics | Existing tests cover all-or-nothing failure, not first-write-success/second-write-fails. |
| Provider collection dispatch is not product turn dispatch | Confirmed | Product paths build `Box<dyn Provider>` and `agent_loop` calls `provider.stream`; collection dispatch has no production caller. |
| `MetadataProvider` is registered in a dispatch-capable collection but returns an empty stream | Confirmed | `MetadataProvider::stream` returns `stream::empty()`. |
| Bedrock descriptor does not model non-env credential success | Confirmed for collection dispatch misuse | `build_bedrock` can succeed via config/profile; descriptor remains `AWS_ACCESS_KEY_ID`. |
| `dispatch_complete` edge branches lack tests | Confirmed | No tests for terminal `Error`, mid-stream provider error, or empty stream. |
| Docs overclaim `SessionRepo` list/fork and Phase 10 matrix status | Confirmed | `opi-spec` says append/load/list/fork; `SessionRepo` rustdoc excludes list/fork. Matrix still says Phase 10 planned. |
| Exit trace and non-goal guards are too weak | Confirmed | `phase10_exit_trace_completeness` hardcodes all statuses as `met`; non-goal needles are exact phrases. |
| `provider_factory_routes_through_collection` mutates env without serialization | Confirmed | Uses `set_var`/`remove_var` without mutex guard. |
| Whitespace auth status differs from runtime validation | Confirmed | `SecretKey::is_present` and `EnvApiKey` use `is_empty`, while runtime `require_api_key` trims. |
| Error types lack standard error impl | Confirmed | `ProviderBuildError` has manual `Display`; `ListModelsError` has no `Display`/`Error`. |

Not a fix in this plan:

| Area | Decision |
|---|---|
| Full `CodingHarness` turn loop adoption of `AgentHarness` | Defer. This would be a product-loop migration and conflicts with Phase 10 non-goal "no whole-loop rewrite". Update claims instead. |
| `SessionFacade::active_tip` O(n) reload | Defer unless the session fix naturally introduces cached state. It is unused in production and not correctness-critical after hydration. |
| Large flusher deduplication | Defer until correctness tests are green. Avoid mixing refactor with graph fixes. |

## File Structure

- Modify: `crates/opi-agent/src/harness.rs`
  - Session graph state: split content tip from sidecar writes, use collision-resistant IDs, hydrate facade state from repo load.
- Modify: `crates/opi-agent/tests/session_facade.rs`
  - Red tests for resume append, extension sidecar parent graph, and partial flush.
- Modify: `crates/opi-agent/tests/harness.rs`
  - Mirror sidecar parent and partial flush coverage for `AgentHarness` where possible.
- Modify: `crates/opi-ai/src/provider_collection.rs`
  - Trim auth values, add a resolved/configured descriptor for providers registered after successful credential resolution, keep dispatch errors explicit.
- Modify: `crates/opi-ai/tests/provider_collection.rs`
  - Red tests for complete-dispatch error branches and whitespace auth.
- Modify: `crates/opi-coding-agent/src/provider_factory.rs`
  - Make metadata-only provider fail explicitly, attach accurate resolved auth metadata, fix compat metadata, derive standard errors.
- Modify: `crates/opi-coding-agent/tests/provider_factory.rs`
  - Serialize env mutation, cover metadata-only dispatch error and compat metadata.
- Modify: `crates/opi-coding-agent/src/doctor.rs`
  - Reuse provider-factory credential descriptor mapping for non-Bedrock providers; keep Bedrock chain probe.
- Modify: `crates/opi-coding-agent/tests/productized_packages_docs.rs`
  - Replace tautological exit trace, strengthen non-goal phrase coverage, keep EN/ZH guard symmetry.
- Modify: `docs/opi-spec.md`
  - Correct Phase 10 seam wording and list/fork ownership.
- Modify: `docs/opi-spec.zh.md`
  - Same corrections as English counterpart.
- Modify: `docs/pi-alignment-matrix.md`
  - Update Phase 10 status and seam adoption language.
- Modify: `docs/pi-alignment-matrix.zh.md`
  - Same corrections as English counterpart.
- Modify if user-facing behavior changes: `CHANGELOG.md`
  - Add `[Unreleased]` entries for library seam correctness fixes.

---

### Task 1: Fix Session Facade Graph Safety

**Files:**
- Modify: `crates/opi-agent/src/harness.rs`
- Modify: `crates/opi-agent/tests/session_facade.rs`
- Modify: `crates/opi-agent/tests/harness.rs`

**Interfaces:**
- Consumes: `SessionEntry::Message`, `SessionEntry::Compaction`, `SessionEntry::ExtensionState`, `SessionTree::from_entries`.
- Produces: `SessionFacade::new(...) -> std::io::Result<SessionFacade>` or an equivalent fallible constructor that hydrates state before append.
- Produces: content entries whose parent chain never points through `ExtensionState`.

- [ ] **Step 1: Write failing resume append test**

Add this test to `crates/opi-agent/tests/session_facade.rs`:

```rust
#[test]
fn session_facade_hydrates_tip_and_uses_unique_ids_after_open() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("resume-append.jsonl");
    {
        let repo = JsonlSessionRepo::create(&path, header("s1")).unwrap();
        let mut facade = SessionFacade::new(Box::new(repo)).unwrap();
        facade.enqueue_message(user_msg("first")).unwrap();
        facade.flush().unwrap();
    }

    let repo = JsonlSessionRepo::open(&path).unwrap();
    let mut facade = SessionFacade::new(Box::new(repo)).unwrap();
    facade.enqueue_message(user_msg("second")).unwrap();
    facade.flush().unwrap();

    let (_header, entries, _recovery) = facade.load().unwrap();
    let messages: Vec<_> = entries
        .iter()
        .filter_map(|entry| match entry {
            SessionEntry::Message(message) => Some(message),
            _ => None,
        })
        .collect();
    assert_eq!(messages.len(), 2);
    assert_ne!(messages[0].id, messages[1].id);
    assert_eq!(messages[1].parent_id.as_deref(), Some(messages[0].id.as_str()));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p opi-agent --test session_facade session_facade_hydrates_tip_and_uses_unique_ids_after_open`

Expected: FAIL before implementation because `SessionFacade::new` does not return `Result` and/or the reopened append uses `entry-1` with `parent_id = None`.

- [ ] **Step 3: Write failing extension sidecar tests**

Add these tests to `crates/opi-agent/tests/session_facade.rs`:

```rust
#[test]
fn extension_state_does_not_become_content_parent_when_enqueued_first() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("extension-first.jsonl");
    let repo = JsonlSessionRepo::create(&path, header("s1")).unwrap();
    let mut facade = SessionFacade::new(Box::new(repo)).unwrap();

    facade.enqueue_extension_state(json!({"n": 1})).unwrap();
    facade.enqueue_message(user_msg("agent-text")).unwrap();
    facade.flush().unwrap();

    let (_header, entries, _recovery) = facade.load().unwrap();
    let message = entries
        .iter()
        .find_map(|entry| match entry {
            SessionEntry::Message(message) => Some(message),
            _ => None,
        })
        .unwrap();
    assert_eq!(message.parent_id, None);
    assert_eq!(facade.active_tip().unwrap().as_deref(), Some(message.id.as_str()));
}

#[test]
fn extension_state_attaches_to_content_tip_without_advancing_it() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("extension-sidecar.jsonl");
    let repo = JsonlSessionRepo::create(&path, header("s1")).unwrap();
    let mut facade = SessionFacade::new(Box::new(repo)).unwrap();

    facade.enqueue_message(user_msg("first")).unwrap();
    facade.enqueue_extension_state(json!({"n": 1})).unwrap();
    facade.enqueue_message(user_msg("second")).unwrap();
    facade.flush().unwrap();

    let (_header, entries, _recovery) = facade.load().unwrap();
    let messages: Vec<_> = entries
        .iter()
        .filter_map(|entry| match entry {
            SessionEntry::Message(message) => Some(message),
            _ => None,
        })
        .collect();
    let state = entries
        .iter()
        .find_map(|entry| match entry {
            SessionEntry::ExtensionState(state) => Some(state),
            _ => None,
        })
        .unwrap();
    assert_eq!(state.parent_id.as_deref(), Some(messages[0].id.as_str()));
    assert_eq!(messages[1].parent_id.as_deref(), Some(messages[0].id.as_str()));
}
```

- [ ] **Step 4: Run sidecar tests to verify they fail**

Run: `cargo test -p opi-agent --test session_facade extension_state`

Expected: FAIL before implementation because a message enqueued after extension state is parented to the extension-state ID.

- [ ] **Step 5: Add partial flush regression test**

Add this helper and test to `crates/opi-agent/tests/session_facade.rs`:

```rust
struct FailOnAppend {
    entries: Vec<SessionEntry>,
    fail_on_call: usize,
    calls: usize,
}

impl FailOnAppend {
    fn new(fail_on_call: usize) -> Self {
        Self {
            entries: Vec::new(),
            fail_on_call,
            calls: 0,
        }
    }
}

impl SessionRepo for FailOnAppend {
    fn append(&mut self, entry: &SessionEntry) -> std::io::Result<()> {
        self.calls += 1;
        if self.calls == self.fail_on_call {
            return Err(std::io::Error::other("simulated partial failure"));
        }
        self.entries.push(entry.clone());
        Ok(())
    }

    fn load(&self) -> std::io::Result<(SessionHeader, Vec<SessionEntry>, CrashRecovery)> {
        Ok((header("partial"), self.entries.clone(), CrashRecovery::Clean))
    }

    fn message_count(&self) -> std::io::Result<usize> {
        Ok(self.entries.len())
    }
}

#[test]
fn partial_flush_never_persists_message_parented_to_extension_state() {
    let mut facade = SessionFacade::new(Box::new(FailOnAppend::new(2))).unwrap();
    facade.enqueue_extension_state(json!({"n": 1})).unwrap();
    facade.enqueue_message(user_msg("message survives first")).unwrap();

    let err = facade.flush().unwrap_err();
    assert!(matches!(err, HarnessError::Write(_)));

    let (_header, entries, _recovery) = facade.load().unwrap();
    assert_eq!(entries.len(), 1);
    match &entries[0] {
        SessionEntry::Message(message) => assert_eq!(message.parent_id, None),
        other => panic!("expected the content message to be written first, got {other:?}"),
    }
}
```

- [ ] **Step 6: Implement content-tip semantics**

In `crates/opi-agent/src/harness.rs`:

```rust
// Rename the field in both AgentHarness and SessionFacade.
content_tip_entry_id: Option<String>,

fn next_id(&mut self) -> String {
    format!("entry-{}", uuid::Uuid::now_v7())
}

fn active_content_tip(entries: &[SessionEntry]) -> Option<String> {
    crate::session_branch::SessionTree::from_entries(entries)
        .active_tip()
        .map(str::to_owned)
}
```

Update message/compaction enqueue paths:

```rust
let id = self.next_id();
let parent_id = self.content_tip_entry_id.clone();
// build MessageEntry or CompactionEntry
self.content_tip_entry_id = Some(id);
```

Update extension-state enqueue paths:

```rust
let id = self.next_id();
let parent_id = self.content_tip_entry_id.clone();
// build ExtensionStateEntry
// Do not update content_tip_entry_id here.
```

Make the facade constructor hydrate from the backing repo:

```rust
pub fn new(repo: Box<dyn SessionRepo>) -> std::io::Result<Self> {
    let (_header, entries, _recovery) = repo.load()?;
    Ok(Self {
        repo,
        queue: PendingWriteQueue::new(),
        content_tip_entry_id: active_content_tip(&entries),
        savepoint_seq: 0,
        last_save_point: None,
    })
}
```

Remove `id_counter` from `SessionFacade`. Remove or stop relying on `id_counter` in `AgentHarness`; UUID/v7 IDs remove collision risk for newly created harness sessions.

- [ ] **Step 7: Update existing tests for fallible constructor**

Replace every `SessionFacade::new(Box::new(...))` call in `crates/opi-agent/tests/session_facade.rs` with:

```rust
SessionFacade::new(Box::new(repo)).unwrap()
```

For direct inline calls, keep the unwrap at construction:

```rust
let mut facade = SessionFacade::new(Box::new(RecordingSessionRepo::default())).unwrap();
```

- [ ] **Step 8: Mirror sidecar coverage in `AgentHarness`**

Add a test in `crates/opi-agent/tests/harness.rs` that enqueues extension state first, then a message, flushes, and asserts the message has no extension parent. Use a `JsonlHarnessSession` and `SessionReader::read_all`.

- [ ] **Step 9: Run session tests**

Run:

```sh
cargo test -p opi-agent --test session_facade
cargo test -p opi-agent --test harness
```

Expected: all tests pass.

---

### Task 2: Harden ProviderCollection Dispatch and Auth Semantics

**Files:**
- Modify: `crates/opi-ai/src/provider_collection.rs`
- Modify: `crates/opi-ai/tests/provider_collection.rs`

**Interfaces:**
- Consumes: `ProviderCollection::dispatch_stream`, `ProviderCollection::dispatch_complete`, `AuthDescriptor::resolve`.
- Produces: explicit behavior for terminal error streams, mid-stream provider errors, empty streams, and whitespace credentials.

- [ ] **Step 1: Add complete-dispatch edge tests**

Add helper providers to `crates/opi-ai/tests/provider_collection.rs`:

```rust
struct StreamProvider {
    id: &'static str,
    events: Vec<Result<opi_ai::AssistantStreamEvent, opi_ai::provider::ProviderError>>,
}

impl Provider for StreamProvider {
    fn id(&self) -> &str {
        self.id
    }

    fn models(&self) -> &[opi_ai::provider::ModelInfo] {
        static MODELS: std::sync::OnceLock<Vec<opi_ai::provider::ModelInfo>> =
            std::sync::OnceLock::new();
        MODELS.get_or_init(|| vec![opi_ai::provider::ModelInfo {
            id: "mock-model".into(),
            display_name: "Mock Model".into(),
            context_window: 128_000,
            max_output_tokens: 4096,
            supports_images: false,
            supports_streaming: true,
            supports_thinking: false,
        }])
    }

    fn stream(&self, _request: Request) -> opi_ai::provider::EventStream {
        Box::pin(futures_util::stream::iter(self.events.clone()))
    }
}
```

Then add these tests:

```rust
#[tokio::test]
async fn dispatch_complete_returns_terminal_error_event() {
    let message = AssistantMessage {
        content: vec![AssistantContent::Text {
            text: "terminal failure".into(),
        }],
        api: opi_ai::ApiKind::OpenAi,
        provider: "mock".into(),
        model: "mock-model".into(),
        response_model: None,
        response_id: None,
        usage: opi_ai::stream::Usage::default(),
        stop_reason: opi_ai::stream::StopReason::Error,
        error_message: Some("terminal failure".into()),
        timestamp_ms: 0,
    };
    let mut collection = ProviderCollection::new();
    collection
        .register(
            Box::new(StreamProvider {
                id: "mock",
                events: vec![Ok(opi_ai::AssistantStreamEvent::Error {
                    reason: opi_ai::stream::StopReason::Error,
                    message,
                })],
            }),
            AuthDescriptor::StaticApiKey {
                value: SecretKey::new("configured"),
            },
            CompatMetadata::default(),
        )
        .unwrap();

    let completed = collection
        .dispatch_complete("mock:mock-model", minimal_request("mock:mock-model"))
        .await
        .unwrap();
    assert!(matches!(completed, CompletedRequest::Error { .. }));
}

#[tokio::test]
async fn dispatch_complete_propagates_mid_stream_provider_error() {
    let mut collection = ProviderCollection::new();
    collection
        .register(
            Box::new(StreamProvider {
                id: "mock",
                events: vec![Err(opi_ai::provider::ProviderError::StreamError(
                    "mid-stream failure".into(),
                ))],
            }),
            AuthDescriptor::StaticApiKey {
                value: SecretKey::new("configured"),
            },
            CompatMetadata::default(),
        )
        .unwrap();

    let err = collection
        .dispatch_complete("mock:mock-model", minimal_request("mock:mock-model"))
        .await
        .unwrap_err();
    assert!(matches!(err, CollectionError::Provider(_)));
    assert!(err.to_string().contains("mid-stream failure"));
}

#[tokio::test]
async fn dispatch_complete_rejects_empty_stream() {
    let mut collection = ProviderCollection::new();
    collection
        .register(
            Box::new(StreamProvider {
                id: "mock",
                events: Vec::new(),
            }),
            AuthDescriptor::StaticApiKey {
                value: SecretKey::new("configured"),
            },
            CompatMetadata::default(),
        )
        .unwrap();

    let err = collection
        .dispatch_complete("mock:mock-model", minimal_request("mock:mock-model"))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("stream ended without a terminal event"));
}
```

Use `AuthDescriptor::StaticApiKey { value: SecretKey::new("configured") }` for each registered provider.

- [ ] **Step 2: Add whitespace auth tests**

Add:

```rust
#[test]
fn auth_descriptor_treats_whitespace_as_missing() {
    let missing = AuthDescriptor::StaticApiKey {
        value: SecretKey::new("   "),
    };
    assert!(matches!(missing.resolve(), AuthStatus::Missing { .. }));
}
```

Add an env-var version using a serialized env helper if the test mutates process env.

- [ ] **Step 3: Run tests to verify failures**

Run: `cargo test -p opi-ai --test provider_collection`

Expected: new edge tests fail or do not compile until helper imports and auth trimming are implemented.

- [ ] **Step 4: Implement auth trimming**

In `SecretKey::is_present`:

```rust
pub fn is_present(&self) -> bool {
    !self.0.trim().is_empty()
}
```

In `AuthDescriptor::EnvApiKey` resolution:

```rust
Ok(value) if !value.trim().is_empty() => AuthStatus::Configured,
Ok(_) => AuthStatus::Missing {
    source: format!("env var {env_var} is set but empty"),
},
Err(std::env::VarError::NotPresent) => AuthStatus::Missing {
    source: format!("env var {env_var} is not set"),
},
Err(std::env::VarError::NotUnicode(_)) => AuthStatus::Missing {
    source: format!("env var {env_var} is not valid unicode"),
},
```

- [ ] **Step 5: Add resolved credential descriptor**

If provider factory registration needs to represent "provider was successfully built from config/profile/credential chain", add:

```rust
Resolved {
    source: String,
}
```

to `AuthDescriptor`, and resolve it as `AuthStatus::Configured` when `source.trim()` is non-empty. Do not include secret values in `source`.

- [ ] **Step 6: Run provider collection tests**

Run: `cargo test -p opi-ai --test provider_collection`

Expected: all tests pass.

---

### Task 3: Fix Provider Factory Footguns and Test Isolation

**Files:**
- Modify: `crates/opi-coding-agent/src/provider_factory.rs`
- Modify: `crates/opi-coding-agent/tests/provider_factory.rs`
- Modify: `crates/opi-coding-agent/src/doctor.rs`

**Interfaces:**
- Consumes: `build_collection_for_listing`, `assemble_harness_collection`, `auth_descriptor_for`, `compat_metadata_for`.
- Produces: explicit metadata-only dispatch error, serialized env mutation, standard error types, accurate compat metadata.

- [ ] **Step 1: Serialize env mutation in provider factory tests**

At top of `crates/opi-coding-agent/tests/provider_factory.rs`:

```rust
use std::sync::Mutex;

static ENV_MUTEX: Mutex<()> = Mutex::new(());

fn with_env_var<F, R>(key: &str, value: &str, f: F) -> R
where
    F: FnOnce() -> R,
{
    let _lock = ENV_MUTEX.lock().unwrap();
    let original = std::env::var(key).ok();
    unsafe { std::env::set_var(key, value) };
    let result = f();
    match original {
        Some(value) => unsafe { std::env::set_var(key, value) },
        None => unsafe { std::env::remove_var(key) },
    }
    result
}
```

Wrap the body that needs `OPI_TEST_FACTORY_ROUTE_9F2A7C11`:

```rust
let collection = with_env_var(env_var, "test-key", || {
    build_collection_for_listing(&config).expect("listing collection builds through the factory")
});
```

- [ ] **Step 2: Add metadata-only dispatch regression**

Add a test using `assemble_harness_collection` and a `MockProvider`, then call `dispatch_complete("mock:mock-model", minimal_request(...))`.

Expected after the implementation: the error message contains `metadata-only provider` and the active provider ID.

- [ ] **Step 3: Add compat metadata regression**

Add assertions:

```rust
use opi_coding_agent::provider_factory::compat_metadata_for;

#[test]
fn built_in_openai_compatible_metadata_is_set() {
    for provider in ["openai", "openrouter", "mistral"] {
        assert!(
            compat_metadata_for(provider).openai_compatible,
            "{provider} should advertise OpenAI-compatible chat metadata"
        );
    }
    assert!(!compat_metadata_for("anthropic").openai_compatible);
}
```

- [ ] **Step 4: Implement explicit metadata provider error**

In `MetadataProvider::stream`:

```rust
fn stream(&self, _request: Request) -> EventStream {
    let id = self.id.clone();
    Box::pin(futures_util::stream::once(async move {
        Err(ProviderError::StreamError(format!(
            "provider '{id}' is metadata-only in the harness model registry and cannot dispatch"
        )))
    }))
}
```

- [ ] **Step 5: Use resolved auth after successful listing construction**

When `build_collection_for_listing` successfully constructs a provider, register it with a descriptor that reflects the successful construction:

```rust
let auth = resolved_auth_descriptor_for(config, provider_id);
```

For Bedrock, use a source such as `"aws credential chain"` rather than `AWS_ACCESS_KEY_ID`. For regular env providers, `"env {VAR}"` is fine. Keep `auth_descriptor_for` as the pure mapping used by tests and doctor diagnostics.

- [ ] **Step 6: Fix compat metadata**

Implement:

```rust
pub fn compat_metadata_for(provider_id: &str) -> CompatMetadata {
    match provider_id {
        "openai" | "openrouter" | "mistral" => CompatMetadata {
            openai_compatible: true,
            profile: Some(provider_id.to_owned()),
        },
        _ => CompatMetadata::default(),
    }
}
```

Only add `azure` here if a direct check confirms its provider is intended to be advertised as OpenAI-compatible at the collection metadata layer.

- [ ] **Step 7: Derive standard errors**

Replace manual `ProviderBuildError` display with `thiserror`:

```rust
#[derive(Debug, thiserror::Error)]
pub enum ProviderBuildError {
    #[error("{0}")]
    Auth(String),
    #[error("{0}")]
    Config(String),
    #[error(transparent)]
    Provider(#[from] ProviderError),
}

#[derive(Debug, thiserror::Error)]
pub enum ListModelsError {
    #[error("missing credentials")]
    MissingCredentials,
    #[error("{0}")]
    Config(String),
}
```

Remove the manual `impl From<ProviderError>` if the derive covers it.

- [ ] **Step 8: Reuse factory credential mapping in doctor**

In `doctor.rs`, keep the Bedrock-specific `bedrock_credential_probe` because it already models profile/config/env. For non-Bedrock providers, call `provider_factory::auth_descriptor_for` or `auth_descriptor_for_profile` to obtain the env var name instead of keeping a parallel match table.

- [ ] **Step 9: Run provider factory and doctor tests**

Run:

```sh
cargo test -p opi-coding-agent --test provider_factory
cargo test -p opi-coding-agent --test doctor_cli
```

Expected: all tests pass.

---

### Task 4: Correct Phase 10 Claims and Strengthen Guard Tests

**Files:**
- Modify: `docs/opi-spec.md`
- Modify: `docs/opi-spec.zh.md`
- Modify: `docs/pi-alignment-matrix.md`
- Modify: `docs/pi-alignment-matrix.zh.md`
- Modify: `crates/opi-coding-agent/tests/productized_packages_docs.rs`
- Modify: `crates/opi-coding-agent/tests/provider_factory.rs`
- Modify: `CHANGELOG.md` when the implemented fix changes user-visible CLI output or public library behavior.

**Interfaces:**
- Consumes: Phase 10 design and current code facts.
- Produces: docs that distinguish "published seam" from "product-adopted dispatch/turn loop".

- [ ] **Step 1: Update session seam wording**

In `docs/opi-spec.md`, replace:

```markdown
stable durable append/load/list/fork traits and ordered read/write facade for Phase 13
```

with:

```markdown
stable durable append/load/entry-count traits and ordered read/write facade for Phase 13; list/fork stay product-owned in `opi-coding-agent`
```

Make the equivalent correction in `docs/opi-spec.zh.md`.

- [ ] **Step 2: Update provider seam wording**

In `docs/opi-spec.md`, change the Phase 10 paragraph so it says:

```markdown
`opi-ai` exposes the provider collection/auth seam. `opi-coding-agent` routes
model listing and model-registry construction through that seam; active
runtime provider dispatch still uses the existing `Box<dyn Provider>` path, and
collection-level dispatch adoption is deferred until a reviewed product-loop
migration.
```

Make the equivalent correction in `docs/opi-spec.zh.md`.

- [ ] **Step 3: Update alignment matrix**

In `docs/pi-alignment-matrix.md`:

- Change Phase 10 current level from `Planned` to `Partial`.
- Change the `opi-agent` package gap from "No generic `AgentHarness`/session facade equivalent yet" to "Generic `AgentHarness`/session facade seams exist, but product turn/session adoption is still partial."
- Change `Pending session write ordering` from `Missing` to `Partial`.

Make the equivalent corrections in `docs/pi-alignment-matrix.zh.md`.

- [ ] **Step 4: Replace tautological exit trace test**

In `phase10_exit_trace_completeness`, remove the hardcoded array that asserts every status string equals `"met"`.

Replace it with assertions over honest status phrases:

```rust
for phrase in [
    "published provider collection/auth seam",
    "runtime provider dispatch still uses",
    "published generic `AgentHarness`",
    "product turn loop adoption is deferred",
    "list/fork stay product-owned",
] {
    assert!(
        spec_en.contains(phrase),
        "Phase 10 exit trace must honestly state `{phrase}`"
    );
}
```

Use exact phrases after the doc wording is finalized. Add ZH counterparts for the session seam and deferred adoption claims.

- [ ] **Step 5: Strengthen forbidden-scope guard**

Replace the single exact-phrase list with grouped claim verbs:

```rust
let claims = [
    ("provider OAuth login", ["supports", "implements", "provides", "ships"]),
    ("subscription auth", ["supports", "implements", "provides", "ships"]),
    ("shared opi-types crate", ["introduced", "adds", "ships", "provides"]),
    ("whole agent loop", ["rewrote", "replaces", "migrates", "routes entirely through"]),
];
```

For each `(feature, verbs)`, assert no positive line contains both a verb and the feature unless it is clearly negated by `no_positive_claim`.

- [ ] **Step 6: Strip comments in provider centralization guard**

Move or copy the existing `strip_rust_comments` helper into `crates/opi-coding-agent/tests/provider_factory.rs` and scan comment-stripped source in `provider_policy_is_centralized`.

- [ ] **Step 7: Run docs guard tests**

Run:

```sh
cargo test -p opi-coding-agent --test productized_packages_docs phase10
cargo test -p opi-coding-agent --test provider_factory provider_policy_is_centralized
```

Expected: all tests pass.

---

### Task 5: Final Verification

**Files:**
- No new files beyond the task modifications.

**Interfaces:**
- Consumes: completed Tasks 1-4.
- Produces: verified workspace state ready for review.

- [ ] **Step 1: Format**

Run:

```sh
cargo fmt --all
cargo fmt --check --all
```

Expected: both commands complete cleanly.

- [ ] **Step 2: Run targeted test suite**

Run:

```sh
cargo test -p opi-agent --test session_facade
cargo test -p opi-agent --test harness
cargo test -p opi-ai --test provider_collection
cargo test -p opi-coding-agent --test provider_factory
cargo test -p opi-coding-agent --test doctor_cli
cargo test -p opi-coding-agent --test productized_packages_docs phase10
```

Expected: all pass.

- [ ] **Step 3: Run required clippy gate**

Run:

```sh
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: no warnings.

- [ ] **Step 4: Run broader workspace tests**

Run:

```sh
cargo test --workspace --all-targets
```

Expected: all tests pass. If this command cannot complete because of an environmental blocker, record the blocker and the targeted tests plus clippy evidence explicitly.

- [ ] **Step 5: Inspect git status**

Run:

```sh
git status --short
```

Expected: only files touched by these tasks appear as modified, plus the existing untracked audit documents if they remain untracked.

## Execution Notes

- Keep Task 1 as the first implementation batch. It protects future session adoption from corrupt parent graphs.
- Keep Task 4 after code truth is settled. The docs should describe the final behavior, not an intermediate hypothesis.
- Do not route `CodingHarness::prompt` through `AgentHarness` in this fix set. That is a separate migration requiring a turn-driving harness API and broader regression coverage.
- Do not update released changelog sections. If changelog is needed, add only under `## [Unreleased]`.
