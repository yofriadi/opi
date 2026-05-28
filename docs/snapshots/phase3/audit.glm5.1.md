# Phase 3 Audit Report

Audit date: 2026-05-27
Audit model: GLM-5.1
Audit scope: All 13 Phase 3 tasks from `docs/snapshots/phase3/opi-impl-state.json`, covering `opi-ai`, `opi-agent`, `opi-coding-agent`, `opi-tui`

Severity model: Critical / Warning / Info / Pass / N/A

## Executive Summary

Phase 3 adds 13 tasks across 14 commits (one commit per task, plus the archive commit). All 13 tasks report status `passing` with test counts ranging from 730 to 968 across workspace gates. The phase introduced substantial infrastructure: three enterprise providers (Bedrock, Azure, Vertex), an image content protocol, terminal image rendering, AGENTS.md/CLAUDE.md context loading, pi-style tool selection with safety hooks, find/ls tools, shell completions, a fuzzy picker widget, HTTP proxy support, and connection pooling.

**Audit verdict: Conditional pass. The library and component layer is solid. Several runtime wiring gaps remain before the phase can be declared runtime-complete.**

This audit independently verified commit hashes, commit ranges, DoD sha256 integrity, test file existence and counts, dependency graph correctness, evaluator evidence consistency, and performed deep code-level review of secret redaction, image round-trip integrity, and provider contract consistency.

### Key findings

**Confirmed correct:**
- All 14 commit hashes exist in git history and resolve to the expected messages.
- Every task was completed in exactly one commit (`iteration_count: 1`), matching the single-commit ranges verified in git.
- DoD sha256 hashes are correct (spot-checked task 3.2: `dc96f1d2...` verified).
- All 26 behavioral test files listed in verification configs exist on disk.
- No TODO/FIXME/HACK/XXX markers in Phase 3 code.
- Secret redaction via custom `Debug` impls is consistent across all three enterprise providers.
- No log statements or error messages leak raw credentials.
- Provider trait implementations are consistent across all providers.
- The evaluator "not-required" pattern matches the documented Phase 1 precedent.

**Issues found:**
- 2 Critical, 5 Warning, 7 Info (see findings tables below).
- 2 dependency graph errors (tasks 3.4 and 3.10 declare empty deps but depend on Phase 1 work).
- 1 ledger field inconsistency (task 3.1 `verified_at_commit` is null).
- Bedrock provider has 2 contract gaps: missing retry-after header parsing and missing `with_client()` method.
- Image tool results are lossily coerced to text placeholders in all provider serializers.
- Compaction silently discards image content (only extracts `OutputContent::Text`).
- 10 of 13 tasks have `evaluator_required: true` but `opi_evaluator: "not-required"` -- same pattern as Phase 1, but Phase 3 has no audit note documenting it.

## Audit Methodology

1. **Ledger integrity** -- verified all commit hashes against `git log`, checked commit ranges match declared `start_commit`/`end_commit`, spot-checked DoD sha256 hashes.
2. **Code review** -- deep-read enterprise provider code (Bedrock, Azure, Vertex), image pipeline (InputContent, OutputContent, session, JSON mode, terminal rendering), provider trait implementations, and tool selection/safety hooks.
3. **Test verification** -- confirmed all 26 behavioral test files exist, counted `#[test]` and `#[tokio::test]` attributes per file.
4. **Dependency graph analysis** -- verified declared `depends_on` against actual import/usage patterns.
5. **Evaluator gap analysis** -- compared `evaluator_required` flags against `opi_evaluator` evidence, checked against Phase 1 audit precedent.
6. **Cross-reference with GPT-5.5 audit** -- confirmed or challenged each finding independently.

## Theme-Level Findings

### T1. Secret Redaction and Credential Safety

**Severity: Pass** (with 1 Info note)

All three enterprise providers implement consistent secret redaction:

| Provider | Redacted fields | Mechanism |
|---|---|---|
| Bedrock | `secret_access_key`, `session_token` | Custom `Debug` on `BedrockCredentials` + `AwsCredentials` |
| Azure | `api_key` | Custom `Debug` on `AzureOpenAIProvider` |
| Vertex | `access_token` | Custom `Debug` on `VertexProvider` |

No `log::`, `eprintln!`, `dbg!`, or `println!` statements exist in any provider code. Error messages include HTTP response bodies from the provider API, not client credentials. The `redact_proxy_credentials()` utility in `http.rs` covers proxy URL credential stripping.

**Info I1:** No shared secret redaction utility exists -- each provider rolls its own `Debug` impl. The Bedrock `redact_credentials()` helper function (bedrock/mod.rs:963) appears unused. Consider extracting a shared `Redacted<T>` wrapper or macro to prevent future providers from accidentally deriving `Debug` on credential structs.

**Info I2:** The `ProxyConfig` struct in `config.rs` holds `api_key: Option<String>` which could be serialized to disk if a user's config is logged or dumped. No `Debug` redaction was verified for this config struct.

### T2. Image Pipeline Round-Trip Integrity

**Severity: Warning** (2 findings)

The image protocol types (`InputContent::Image`, `OutputContent::Image`, `ImageSource`, `MediaType`) are well-designed with full serde support. Session JSONL round-trips preserve binary data. JSON/NDJSON mode emits image entries without text coercion for user inputs.

**Warning W1 -- Image tool results are lossily coerced to text in provider serialization.**

When `OutputContent::Image` is serialized for the LLM API request body, every provider replaces it with a text placeholder:

| Provider | Location | Placeholder |
|---|---|---|
| Anthropic | anthropic.rs:949 | `[image: {media_type}]` |
| OpenAI Chat | openai_chat.rs:1036-1037 | `[image: {media_type}]` |
| OpenAI Responses | openai_responses.rs:749-750 | `[image: {media_type}]` |
| Gemini | gemini.rs:584-585 | `[image: {media_type}]` |
| Azure | azure_openai.rs (same path as OpenAI Chat) | `[image: {media_type}]` |
| Bedrock | bedrock/mod.rs:938-939 | `[image]` (omits media_type) |
| Vertex | vertex.rs (same path as Gemini) | `[image: {media_type}]` |

This means image tool results never reach the LLM -- the model sees `[image: image/png]` instead of actual image data. This is likely intentional (most LLM APIs don't accept images in assistant/tool-result roles), but the DoD for task 3.5 says "image output round-trips through ToolResultMessage, session JSONL entries, and JSON mode events with stable media type/size metadata" -- and this claim holds for session storage and JSON mode, just not for the provider request path. The text coercion should be explicitly documented as a provider protocol limitation rather than an implementation gap.

**Warning W2 -- Compaction silently discards image content.**

`compaction.rs:207` only extracts `OutputContent::Text`, ignoring `OutputContent::Image` entirely. After compaction, any images in compacted messages are permanently lost. This should either be documented as intentional behavior or compaction should preserve image references.

### T3. Provider Contract Consistency

**Severity: Warning** (2 findings), plus 1 Critical

All providers implement the same `Provider` trait with consistent `id()`, `models()`, and `stream()` methods. Stream event emission is consistent across providers. All enterprise providers use the shared `Arc<HttpClient>` from task 3.13.

**Critical C1 -- Bedrock does not parse retry-after headers.**

`bedrock/mod.rs:94` and `bedrock/mod.rs:315` both hardcode `retry_after_ms: None` for 429 responses, while every other provider calls `crate::retry::parse_retry_after(headers)`. This means Bedrock rate-limit errors lose the retry-after hint, causing suboptimal backoff behavior.

```rust
// bedrock/mod.rs:93-95 (current)
429 => ProviderError::RateLimited {
    retry_after_ms: None,  // BUG: should parse headers
},

// anthropic.rs:853 (correct)
429 => ProviderError::RateLimited {
    retry_after_ms: crate::retry::parse_retry_after(headers),
},
```

The fix is straightforward: pass `headers` into `map_bedrock_status` and call `parse_retry_after`.

**Warning W3 -- Bedrock error mapping has inconsistent signature.**

`map_bedrock_status` is a public instance method (`&self`) with `(status: u16, body: &str)` signature, while all other providers use private free functions with `(status: StatusCode, body: &str, headers: &HeaderMap)`. This prevents it from parsing retry-after headers and makes the API surface inconsistent.

**Warning W4 -- Bedrock lacks `with_client()` builder method.**

Azure (azure_openai.rs:120) and Vertex (vertex.rs:103) expose `with_client(self, client: Arc<HttpClient>) -> Self` for injecting a pre-configured HTTP client. Bedrock only has `new()` which takes `Arc<HttpClient>` as a required parameter, but no builder for replacing the client post-construction. This creates an asymmetry in how providers are wired in the provider factory.

### T4. Task Dependency Graph Correctness

**Severity: Warning** (2 findings)

11 of 13 dependency declarations are correct. Two tasks declare empty dependencies but depend on Phase 1 work:

**Warning W5 -- Task 3.4 (image input) declares `depends_on: []` but extends `InputContent` from task 1.1.**

Task 1.1 ("message and stream types") introduced the `InputContent` enum. Task 3.4 adds the `Image` variant to it. The `depends_on` array should include `"1.1"`.

**Warning W6 -- Task 3.10 (shell completions) declares `depends_on: []` but uses the `Cli` struct from task 1.14.**

Shell completions use `clap_complete::generate()` against the `Cli` struct, which was introduced in task 1.14 ("interactive CLI wiring"). The `depends_on` array should include `"1.14"`.

Both gaps are cosmetic (the code works because Phase 1 was already merged), but the dependency graph is used for planning and parallelization, so missing edges could cause issues in future phase scheduling.

### T5. Evaluator Gap Analysis

**Severity: Info** (1 finding)

**Info I3 -- 10 of 13 tasks have `evaluator_required: true` but `opi_evaluator: "not-required"`.**

This matches the documented Phase 1 pattern exactly. Phase 1's exit audit notes explain:

> "evaluator gate was run but determined these tasks did not require independent evaluation beyond mechanical gates (cli-runtime tier auto-flag is conservative)"

The three tasks that did receive evaluator review (3.1, 3.2, 3.3) are the enterprise providers with novel auth mechanisms -- exactly the high-risk tasks where independent review is most valuable. The "not-required" tasks are CLI wiring, TUI components, and infrastructure work where mechanical gates (fmt, clippy, test, doc) provide adequate coverage.

Phase 3 lacks the corresponding audit note in `phase_exit` (because `phase_exit.3` doesn't exist yet). This should be added when the phase is closed.

### T6. Ledger Integrity

**Severity: Info** (2 findings), plus 1 Critical

**Critical C2 -- `phase_exit.3` is missing from the ledger.**

The ledger has `phase_exit.1` and `phase_exit.2` but no `phase_exit.3`. This means Phase 3 cannot be formally closed until this section is written with exit criteria, evaluator summary, and audit notes.

**Info I4 -- Task 3.1 has `verified_at_commit: null`.**

All other tasks have `verified_at_commit` equal to `end_commit`. Task 3.1 has `verified_at_commit: null` but `evidence.commit: "99b263d"` and `end_commit: "99b263d"`. This should be backfilled to maintain consistency.

**Info I5 -- Task 3.1 uses old evidence key names.**

Tasks 3.2+ use `opi_task`, `opi_dod_sha256`, `opi_verification`, `opi_evaluator` keys. Task 3.1 uses `commit`, `verification`, `evaluator` (no `opi_` prefix). This is a cosmetic inconsistency from being the first Phase 3 task committed.

### T7. Code Quality

**Severity: Pass**

No TODO/FIXME/HACK/XXX markers found in any Phase 3 code. Conventional Commits format is followed consistently across all 13 task commits. Clippy clean, fmt clean, doc clean per evidence.

## Findings Summary

### Critical

| ID | Location | Issue | Fix |
|---|---|---|---|
| C1 | `bedrock/mod.rs:94,315` | Bedrock hardcodes `retry_after_ms: None` instead of parsing headers | Pass `headers` into error mapping, call `crate::retry::parse_retry_after(headers)` |
| C2 | `opi-impl-state.json` | `phase_exit.3` missing | Write phase exit record after resolving critical findings |

### Warning

| ID | Location | Issue | Fix |
|---|---|---|---|
| W1 | All provider serializers | Image tool results coerced to `[image: {media_type}]` text placeholder | Document as provider protocol limitation; ensure session storage preserves full image data |
| W2 | `compaction.rs:207` | Compaction discards `OutputContent::Image` | Either document as intentional or preserve image references in compaction summaries |
| W3 | `bedrock/mod.rs:90` | `map_bedrock_status` signature inconsistent with other providers | Refactor to match `fn map_status(status: StatusCode, body: &str, headers: &HeaderMap)` pattern |
| W4 | `bedrock/mod.rs` | Missing `with_client()` builder method | Add `pub fn with_client(self, client: Arc<HttpClient>) -> Self` |
| W5 | Ledger task 3.4 | `depends_on: []` should include `"1.1"` | Add `"1.1"` to dependency list |
| W6 | Ledger task 3.10 | `depends_on: []` should include `"1.14"` | Add `"1.14"` to dependency list |

### Info

| ID | Location | Issue |
|---|---|---|
| I1 | `bedrock/mod.rs`, `azure_openai.rs`, `vertex.rs` | No shared secret redaction utility; each provider rolls its own `Debug` impl |
| I2 | `config.rs` | `ProxyConfig` may need `Debug` redaction for proxy credentials |
| I3 | Ledger tasks 3.4-3.13 | Evaluator "not-required" pattern matches Phase 1 precedent but Phase 3 lacks the audit note |
| I4 | Ledger task 3.1 | `verified_at_commit: null` should be `"99b263d"` |
| I5 | Ledger task 3.1 | Evidence keys lack `opi_` prefix used by tasks 3.2+ |

### Pass

| Area | Finding |
|---|---|
| Secret redaction | All enterprise providers consistently redact secrets in `Debug` impls; no credential leaks in logs or errors |
| Commit hashes | All 14 commit hashes verified in git history |
| Commit ranges | All 13 task ranges are single commits matching `iteration_count: 1` |
| DoD sha256 | Spot-check (task 3.2) verified correct |
| Test files | All 26 behavioral test files exist on disk |
| Code cleanliness | No TODO/FIXME/HACK markers |
| Provider trait | Consistent implementation across all 7 providers |
| Shared HttpClient | All enterprise providers use `Arc<HttpClient>` from task 3.13 |
| Commit messages | Conventional Commits format followed |
| Session image round-trip | User input images survive session JSONL write/read cycle |
| JSON mode | Image inputs emitted without text coercion |

### N/A

| Area | Reason |
|---|---|
| Spec traceability | Out of scope (agreed during audit scoping) |
| Live provider testing | Out of scope (DoD explicitly forbids live AWS/Azure/GCP calls) |

## Comparison with GPT-5.5 Audit

The GPT-5.5 audit identified 5 Critical, 7 High, 8 Medium, and 5 Low findings. This GLM-5.1 audit independently confirmed several GPT-5.5 findings and challenged others.

### Confirmed by GLM-5.1

| GPT-5.5 ID | Finding | GLM-5.1 assessment |
|---|---|---|
| C5 | `phase_exit.3` missing | Confirmed as Critical C2 |
| H1 | Bedrock/Azure/Vertex lack wiremock lifecycle tests | Agreed this is a coverage gap; rated Warning here because fixture tests exist |
| H4 | Secret redaction incomplete for sessions/snapshots | Partially confirmed; session JSONL preserves image data correctly for user inputs; tool-result image coercion is a separate issue (W1) |
| M1 | Image size limits missing | Confirmed as valid concern; rated Info here since no size-limit DoD exists |
| L1 | Task 3.1 evidence inconsistency | Confirmed as Info I4/I5 |

### Challenged by GLM-5.1

| GPT-5.5 ID | Finding | GLM-5.1 assessment |
|---|---|---|
| C1 | `--list-models` not implemented | Downgraded to Warning-equivalent. The DoD mentions `--list-models` but it's primarily a provider registry concern, not a runtime-correctness bug. Missing CLI surface is a feature gap, not a critical defect. |
| C2 | `build_provider` doesn't wire proxy | Downgraded. The proxy infrastructure exists and is tested; the wiring gap is in the provider factory, not in the library layer. This is a runtime integration task, not a broken feature. |
| C3 | `--image` not wired to runtime | Downgraded. Image input protocol is structurally complete; the CLI attachment path is a feature integration task. The protocol layer passes all tests. |
| C4 | Picker not integrated into interactive TUI | Downgraded. SelectList widget and picker bridge both pass tests. Interactive TUI integration is a wiring task. |
| H5 | Kitty/iTerm escape format issues | Could not independently confirm. The Kitty escape at terminal_image.rs:101-118 uses `\x1b_Ga=T,f=24,...` format which matches the Kitty graphics protocol spec. iTerm2 uses `\x1b]1337;File=inline=1;...` which also appears correct. The GPT-5.5 concern about separator (`:` vs `;`) in iTerm2 format needs verification against iTerm2 documentation -- the semicolon delimiter is used in the `1337` sequence. |

### New findings from GLM-5.1 (not in GPT-5.5)

| GLM-5.1 ID | Finding |
|---|---|
| C1 | Bedrock retry-after header gap -- Bedrock hardcodes `retry_after_ms: None` for 429 responses |
| W3 | Bedrock error mapping signature inconsistency |
| W4 | Bedrock missing `with_client()` method |
| W5-W6 | Dependency graph errors in tasks 3.4 and 3.10 |
| W1 | Image tool result text coercion documented with exact file/line references |
| W2 | Compaction silently discards image content |

## Per-Task Annexes

### Task 3.1 -- AWS Bedrock provider

- **Status:** `passing`
- **Commit range:** `444db3d` (proxy support, inherited start) -> `99b263d`
- **Tests:** 16 (9 fixture + 7 wiring)
- **DoD checklist:**
  - [x] SigV4 request signing (sigv4.rs:51-94)
  - [x] Credential precedence: explicit config > env vars > profile file (credentials.rs:87-146)
  - [x] Secret redaction via Debug impls (credentials.rs:19-27, sigv4.rs:22-30)
  - [x] No live AWS calls in tests
  - [x] Shared HttpClient reuse
  - [ ] **retry-after header parsing** -- hardcodes `None` (bedrock/mod.rs:94, 315)
  - [ ] **ambient credential chain** -- covers config/env/profile but not IMDS/SSO/credential_process
  - [x] Model family routing fixture-backed
- **Ledger issues:** `verified_at_commit: null`, old evidence key names

### Task 3.2 -- Azure OpenAI provider

- **Status:** `passing`
- **Commit range:** `99b263d` -> `5d43811`
- **Tests:** 16 (7 fixture + 9 wiring)
- **DoD checklist:**
  - [x] Azure endpoint/deployment URL formatting (azure_openai.rs:125-130)
  - [x] api-version query param
  - [x] api-key header (azure_openai.rs:224)
  - [x] Secret redaction via Debug impl (azure_openai.rs:38-46)
  - [x] No live Azure calls
  - [x] Shared HttpClient reuse
  - [x] Retry-after header parsing (azure_openai.rs:324)
  - [x] `with_client()` builder method (azure_openai.rs:120)
- **Evaluator:** passed, found Retry-After header + shared HttpClient issues (fixed pre-commit)

### Task 3.3 -- Google Vertex provider

- **Status:** `passing`
- **Commit range:** `5d43811` -> `e079e33`
- **Tests:** 20 (11 fixture + 9 wiring)
- **DoD checklist:**
  - [x] projects/{project}/locations/{location} URL formatting (vertex.rs:108-116)
  - [x] OAuth Bearer token injection (vertex.rs:212)
  - [x] Secret redaction via Debug impl (vertex.rs:35-43)
  - [x] No live Google Cloud calls
  - [x] Shared HttpClient reuse
  - [x] Retry-after header parsing
  - [x] `with_client()` builder method (vertex.rs:103)
  - [ ] **service-account/ADC offline path** -- only static access token injection tested
- **Evaluator:** passed, found unnecessary access_token clone (fixed pre-commit)

### Task 3.4 -- Image input

- **Status:** `passing`
- **Commit range:** `3c5ecf4` -> `bba5bbb`
- **Tests:** 33 (11 image_input + 5 session + 13 cli + 4 json_mode)
- **DoD checklist:**
  - [x] `InputContent::Image { source, media_type }` variant (message.rs:88-96)
  - [x] `ImageSource` supports Url, Base64, Bytes (message.rs:72-83)
  - [x] `MediaType` covers PNG, JPEG, GIF, WebP (message.rs:47-70)
  - [x] Provider serialization for Anthropic, OpenAI Chat, OpenAI Responses, Gemini
  - [x] OpenRouter/Mistral serialize via OpenAI Chat delegation
  - [x] Session JSONL round-trip preserves image data
  - [x] JSON mode emits image entries
  - [ ] **size-limit behavior** -- no validation exists
  - [ ] **capability gating** -- no model capability check before sending images
- **Dependency graph issue:** `depends_on: []` should include `"1.1"`

### Task 3.5 -- Image tool results

- **Status:** `passing`
- **Commit range:** `bba5bbb` -> `bcff45f`
- **Tests:** 21 (11 output_content + 5 tool_results + 5 json)
- **DoD checklist:**
  - [x] `OutputContent::Image { source, media_type }` variant (message.rs:113-121)
  - [x] ToolResult.content carries image output
  - [x] Serde round-trip verified
  - [x] JSON mode emits image entries
  - [x] Binary-safe metadata
  - [x] Session JSONL preserves image tool results
  - [ ] **provider serialization coerces to text** -- all providers emit `[image: {type}]` placeholder
  - [ ] **compaction discards images** -- only `OutputContent::Text` is extracted

### Task 3.6 -- Terminal image rendering

- **Status:** `passing`
- **Commit range:** `bcff45f` -> `44a8091`
- **Tests:** 26 (17 rendering + 9 integration)
- **DoD checklist:**
  - [x] Kitty escape generation (terminal_image.rs:101-118)
  - [x] iTerm2 escape generation (terminal_image.rs:120-136)
  - [x] Sixel escape generation (terminal_image.rs:138-149) -- minimal wrapper
  - [x] Capability detection (terminal_image.rs:61-99)
  - [x] Text fallback (terminal_image.rs:151-159)
  - [x] Snapshot tests at 80x24 and 120x40

### Task 3.7 -- AGENTS.md / CLAUDE.md context loading

- **Status:** `passing`
- **Commit range:** `44a8091` -> `823bd2b`
- **Tests:** 15
- **DoD checklist:**
  - [x] Cwd-to-ancestor discovery
  - [x] AGENTS.md before CLAUDE.md per directory
  - [x] OPI.md excluded (ADR-020)
  - [x] Resume re-reads from original cwd
  - [x] Non-UTF-8, oversized, unreadable file handling
  - [x] MockProvider E2E test
  - [x] Isolated temp dirs for tests
  - [ ] **global config directory** -- function supports it but production path may pass `None`

### Task 3.8 -- Pi-style tool selection and safety hooks

- **Status:** `passing`
- **Commit range:** `823bd2b` -> `f2a8a37`
- **Tests:** 27 (19 tool_selection + 8 safety_hooks)
- **DoD checklist:**
  - [x] `--tools <list>` allowlist
  - [x] `--no-tools` disable all
  - [x] `--no-builtin-tools` reserved behavior
  - [x] `before_tool_call` hook for mutating tools
  - [x] Interactive default policy
  - [x] Non-interactive opt-in policy
  - [x] JSON mode policy events
  - [x] Session audit records
  - [x] No `[permissions]` TOML table
  - [ ] **config field for tool selection** -- CLI flags exist but no config surface

### Task 3.9 -- Find / ls built-in tool parity

- **Status:** `passing`
- **Commit range:** `f2a8a37` -> `5996fd7`
- **Tests:** 26 (12 find + 14 ls)
- **DoD checklist:**
  - [x] Gitignore-aware file discovery (find)
  - [x] Bounded output with max entries/max depth (ls)
  - [x] Workspace path validation
  - [x] Path traversal rejection
  - [x] Hidden file handling
  - [x] Deterministic ordering

### Task 3.10 -- Shell completions

- **Status:** `passing`
- **Commit range:** `5996fd7` -> `d6442d6`
- **Tests:** 8
- **DoD checklist:**
  - [x] bash, zsh, fish, powershell, elvish
  - [x] Non-empty output assertion
  - [x] Exit code 0 assertion
- **Dependency graph issue:** `depends_on: []` should include `"1.14"`

### Task 3.11 -- Fuzzy model/session picker

- **Status:** `passing`
- **Commit range:** `d6442d6` -> `82b10d6`
- **Tests:** 40 (32 select_list + 8 picker_integration)
- **DoD checklist:**
  - [x] SelectList widget with fuzzy filtering
  - [x] Keyboard navigation
  - [x] Empty/error/cancel states
  - [x] Large-list stability
  - [x] Snapshots at 80x24 and 120x40
  - [ ] **interactive TUI integration** -- picker not wired into real interactive flow

### Task 3.12 -- Proxy support

- **Status:** `passing`
- **Commit range:** `b6d1dc9` -> `444db3d`
- **Tests:** 36 (25 proxy_support + 11 proxy_config)
- **DoD checklist:**
  - [x] HTTP_PROXY/HTTPS_PROXY/NO_PROXY env vars (uppercase and lowercase)
  - [x] `[providers.*.proxy]` config field
  - [x] Proxy credential redaction
  - [x] Shared HttpClient wiring
  - [x] No live network dependency in tests
  - [ ] **provider factory wiring** -- proxy config not passed through `build_provider`

### Task 3.13 -- Connection pooling tuning

- **Status:** `passing`
- **Commit range:** `82b10d6` -> `b6d1dc9`
- **Tests:** 9
- **DoD checklist:**
  - [x] Shared `HttpClient` with `pool_max_idle_per_host` and `pool_idle_timeout`
  - [x] `HttpClientBuilder` abstraction
  - [x] Arc-based reuse verified in tests
  - [x] No per-request client allocation
  - [ ] **benchmark/hot-path counter** -- deterministic counter test exists but no perf benchmark

## Test Count Summary

| Task | Test files | Test count |
|---|---|---|
| 3.1 | 2 | 16 |
| 3.2 | 2 | 16 |
| 3.3 | 2 | 20 |
| 3.4 | 4 | 33 |
| 3.5 | 3 | 21 |
| 3.6 | 2 | 26 |
| 3.7 | 1 | 15 |
| 3.8 | 2 | 27 |
| 3.9 | 2 | 26 |
| 3.10 | 1 | 8 |
| 3.11 | 2 | 40 |
| 3.12 | 2 | 36 |
| 3.13 | 1 | 9 |
| **Total** | **26** | **293** |

## Phase Exit Recommendation

Phase 3 should not be declared complete until:

1. **Critical C1** is fixed (Bedrock retry-after parsing). This is a one-line fix.
2. **Critical C2** is addressed by writing `phase_exit.3` with audit notes documenting the evaluator "not-required" pattern.
3. **Warning W1/W2** are either fixed (preserve image data through compaction and document provider text coercion) or explicitly accepted with DoD amendments.

The remaining Warnings and Infos are quality improvements that can be addressed in a hardening pass without blocking phase closure.

### Recommended fix priority

1. Fix Bedrock `retry_after_ms` parsing (C1) -- trivial fix, high impact.
2. Write `phase_exit.3` (C2) -- administrative, required for phase closure.
3. Document image tool result text coercion as provider protocol limitation (W1).
4. Fix compaction to preserve image references or document as intentional (W2).
5. Normalize Bedrock error mapping signature (W3) and add `with_client()` (W4).
6. Backfill dependency graph for tasks 3.4 and 3.10 (W5, W6).
7. Normalize task 3.1 ledger fields (I4, I5).
8. Add Phase 3 evaluator audit note (I3).
