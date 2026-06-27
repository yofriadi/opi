# Phase 10 Independent Code Audit (GLM-5.2)

- Scope: opi Phase 10 "Core Architecture Deepening", commits `f0f3e0c..7cdcfcc`
  (tasks 10.1–10.7), diff `+6186 / −841` across 20 files.
- Method: independent review only. Findings were derived from source, tests,
  diffs, `Cargo.toml`, and the normative spec/design docs — not from any other
  review, audit, or evaluator record in the repo.
- Reviewer: GLM-5.2, assisted by a 7-dimension parallel review workflow
  (27 agents, ~1.63M tokens) with an adversarial refutation stage over every
  high/medium finding (20 findings adjudicated: 14 confirmed, 5 refuted,
  1 partial). The load-bearing claims were additionally verified by direct
  read of `provider_collection.rs`, `harness.rs` (opi-agent), the production
  provider call chain, and the Phase 10 diffs.
- Status: **independent pass only**. Comparison with any other audit/evaluation
  is deliberately deferred until explicitly requested (per review constraints).

## 摘要 (executive summary)

Phase 10 的代码本身是干净的：`SecretKey` 脱敏真实有效，`AgentHarness`
的状态机/快照/pending-write 顺序逻辑正确，provider factory 的 766 行搬迁基本
保持行为等价，没有引入任何被禁用的范围（OAuth / 图片生成 / opi-types 等），
EN/ZH 文档保持同步。**核心问题不在代码正确性，而在"交付物与声称之间的
落差"**：本次引入的三个新接缝（`ProviderCollection` 的 dispatch/auth、
`AgentHarness`、`SessionFacade`/`SessionRepo`）**没有任何一个接入生产运行
路径**——生产流式仍由 `build_provider() -> Box<dyn Provider>` 直接走
`agent_loop.rs:118` 的 `provider.stream()`，完全绕过集合的
`dispatch_stream`/`auth_status` 鉴权门。因此 Workstream 10.1 的"auth seam"、
10.2 的"routes through the seam"、10.4 的"product wrapper over AgentHarness"
基本是**靠文档和守卫测试满足的，而非靠生产接入满足的**。叠加守卫测试的脆弱
性（词法 token 扫描、`exit-trace` 把所有 SC 硬编码为 `met`），这意味着
Phase 10 的验证体系系统性高估了实际交付深度。新增 ~1,250 行库接缝代码中约
550+ 行在生产侧零调用者。建议：要么真正把 dispatch 路由进集合、把 turn
循环路由进 `AgentHarness`，要么把 spec/ledger/模块文档下调为"已发布但未接
入的 unstable 接缝"，并修正守卫测试使其不再暗示已验证生产行为。

## Headline finding — the seams are clean but unwired

This single observation is the through-line of the audit and is confirmed
independently from four angles (dimensions 10.1, 10.2, 10.4, cross-cutting).

**Production dispatch never touches the Phase 10 seams.**

- Run-mode entrypoints build the active provider directly as a raw
  `Box<dyn Provider>`:
  `crates/opi-coding-agent/src/main.rs:253,368,443` →
  `provider_factory::build_provider(config)` (`provider_factory.rs:582-590`) →
  handed to `Agent::new(...)` (`harness.rs:784`). The agent then streams via
  `crates/opi-agent/src/agent_loop.rs:118` `context.provider.stream(request)`.
- The `ProviderCollection` held by `CodingHarness` (`model_registry` field,
  `harness.rs:99`, built by `assemble_harness_collection`
  `provider_factory.rs:944-979` via `from_registry` at `:978` **with no
  `AuthDescriptor`s attached**) is used **only** through the `.registry()`
  accessor — for the model picker (`harness.rs:880`), thinking-config sizing
  (`harness.rs:775`, `:1819`), and `--list-models` (`main.rs:565`).
- Consequently `dispatch_stream` (`provider_collection.rs:313-326`),
  `dispatch_complete` (`:335-342`), `auth_status`/`auth_descriptor`/`compat`
  (`:293-306`), `refresh` (`:350-352`), and `register` on the *harness*
  collection have **zero production callers** (verified by grep across
  `crates/**/src`; the only callers are in `crates/opi-ai/tests/`).
- The auth gate (`provider_collection.rs:319-324`) is therefore unreachable
  from any production dispatch: even if it were reached, `assemble_harness_collection`
  attaches no descriptors, so `auth_status()` returns `None` and the gate is a
  no-op. The DoD's "redacted missing/invalid auth diagnostics" is exercised only
  in tests and on the throwaway *listing* collection
  (`build_collection_for_listing`, `provider_factory.rs:890-933`).
- `AgentHarness`, `SessionFacade`, `SessionRepo`, `JsonlSessionRepo`,
  `HarnessSession`, `JsonlHarnessSession` have **zero** production construction
  sites (every non-test occurrence in `opi-coding-agent/src` is a doc-comment).
  The production session path is still `SessionCoordinator`
  (`session_coordinator.rs`, constructed in `harness.rs:818,829,1009,1052,1108`).

Net effect: ~550 of the 873 new lines in `opi-agent/src/harness.rs` and the
entire dispatch/auth half of `provider_collection.rs` are public library surface
with no production consumer. The design doc *permits* "seam-first / thin
adapters" (Implementation Notes; SC3 allows "a documented seam **or** an
approved design"), so shipping unwired seams is within the letter of the spec.
The defect is the **gap between claim and reality**:

- Ledger `production_call_sites` list `opi_agent::harness::AgentHarness`,
  `SessionFacade`, `ProviderCollection` with `status: "met"` — these types are
  not production call sites.
- Spec/module docs say provider construction "routes through the seam" (SC2) and
  `CodingHarness` is a "product wrapper over `AgentHarness`" (SC4). In substance
  both hold only at the documentation level.

**Severity:** medium (systemic). No runtime/security defect today (the dead gate
simply never runs), but it misdirects future contributors and embedders, and the
verification apparatus (below) reinforces the overclaim.

---

## Severity-graded findings

`V` = verification status: **confirmed** (adversarially confirmed), **direct**
(personally verified by direct source read), **reported** (single-reviewer, not
refuted; low/info).

### Medium

**F-01 — `AgentHarness` cannot drive a turn: it exposes no execution method.**
`V: direct`. `crates/opi-agent/src/harness.rs:340-599`. The public methods are
`new/phase/snapshot/runtime_config/enqueue_*/flush/begin_*/end_*/abort/messages_snapshot/control_handle/cancel_token`
— there is **no** `prompt`/`step`/`run`/`continue`. The module doc admits this
(`harness.rs:20-22`). This is a sharper blocker than the ledger's stated
"by-value wall" (`harness.rs:30-36`, commit `d4fbdb7`): even if the `Agent`
ownership problem were solved (e.g. `Arc<Agent>`), `AgentHarness` still could
not run a provider turn without new API. So it is structurally a phase/queue/
snapshot sidecar, not a turn-driving harness. *Impact:* the generic seam cannot
replace the product turn loop without further design work; "incremental
migration" is further away than the docs imply. *Fix:* either add a turn-driving
method (and resolve agent ownership) and route `CodingHarness::prompt` through
it, or explicitly document `AgentHarness` as a non-driving discipline sidecar
and re-scope WS 10.2 SC2/SC4 accordingly.

**F-02 — WS 10.2 SC2/SC4 are met via documentation, not substance.**
`V: confirmed`. `crates/opi-coding-agent/src/harness.rs:13-19,84-93,1123-1127`;
design doc `2026-06-24-phase10-...md:120-125,143-144`. The design target assigns
generic turn lifecycle / phase guards / save points / pending writes to a new
`opi-agent::harness`, and SC2 requires `CodingHarness` to be "no longer the only
owner of generic orchestration semantics." `CodingHarness` still owns all of
that inline (it holds an `Agent`, not an `AgentHarness`, and `prompt` delegates
to `self.agent`). *Impact:* the architectural goal of factoring generic
orchestration into a product-agnostic seam is unmet in substance. *Fix:* either
reroute `CodingHarness` through `AgentHarness` (depends on F-01), or mark
SC2/SC4 as "seam published, product adoption deferred" in the design doc and
ledger rather than `met`.

**F-03 — `phase10_forbidden_current_scope_claims_rejected` is a fixed-needle
lexical guard with a synonym-verb bypass.**
`V: confirmed`. `crates/opi-coding-agent/tests/productized_packages_docs.rs:1773-1852`
(needles at `:1796-1815`). The 6 EN needles are exact claim phrases
("supports provider OAuth login", "supports subscription auth", …). A future
overclaim reworded with a synonym verb ("implements"/"provides"/"enables"
subscription auth) passes the guard. *Impact:* SC8/non-goal guard is a partial
net — reliable for today's phrasing, fragile against rewording. *Fix:* broaden
each non-goal to a small synonym set, and/or add a positive-counterpart
assertion that the spec still contains the negated non-goal phrasing.

**F-04 — `phase10_exit_trace_completeness` asserts only spec-string presence and
hardcodes every SC status to `"met"`.**
`V: confirmed`. `crates/opi-coding-agent/tests/productized_packages_docs.rs:1854-1973`
(SC string checks `:1867-1905`; hardcoded trace array `:1957-1966`). Every SC
assertion is `spec_en.contains("...")`; the 8-entry trace table supplies both
the criterion names and the statuses, so `status == "met"` cannot fail unless a
human edits the literal. *Impact:* high assurance the spec *mentions* each SC,
**zero** assurance the claims are *true of the code* — deleting
`crates/opi-agent/src/harness.rs` entirely would leave this test green.
*Fix:* either retitle to `phase10_exit_trace_doc_attestation` (honest doc-presence
gate) and drop the tautological status array, or add minimal code-truth anchors
(e.g. the seam types exist and are constructed somewhere, named regression tests
pass).

**F-05 — `coding_harness_wrapper_keeps_product_policy_out_of_opi_agent` is
comment-blind and token-list-bound.**
`V: confirmed`. `crates/opi-coding-agent/tests/harness_resource_integration.rs:974-1049`
(`strip_rust_comments`), `:1158-1246` (guard). `strip_rust_comments` removes all
comments before scanning, so a real leak placed in an explanatory comment is
invisible to the guard; and only 22 enumerated tokens are checked. The guard is
mutation-non-vacuous for those 22 tokens but is not a general "no product policy
in opi-agent" invariant. *Impact:* over-trusting this test as a boundary proof.
*Fix:* narrow the claim to "these 22 tokens do not appear in opi-agent code,"
or make the guard less token-dependent (e.g. fail on references to types whose
defining crate is `opi-coding-agent`).

**F-06 — `dispatch_complete` edge cases are untested.**
`V: confirmed`. `crates/opi-ai/src/provider_collection.rs:364-380`
(`drain_to_completion`); tests `crates/opi-ai/tests/provider_collection.rs:131-305`.
`drain_to_completion` has four branches: `Ok(Done)` (tested), `Ok(Error)` (the
`CompletedRequest::Error` variant — never exercised), mid-stream `Err` (never
tested), and non-terminal end → `StreamError` (`:377-379`, never tested).
*Impact:* the complete-dispatch decision (the seam's second headline capability)
is validated only on the happy path. *Fix:* add tests for the Error-terminal
stream, a mid-stream `ProviderError`, an empty stream, and tool-use
accumulation.

**F-07 — SC2 "routes provider construction through the seam" only partially met.**
`V: confirmed` (folded into the headline; restated for the SC scorecard).
`crates/opi-coding-agent/src/provider_factory.rs:10-24,944-978`;
`agent_loop.rs:118`. *Fix:* downgrade the spec SC2 claim to "model-metadata
lookup routes through the seam; auth-gated dispatch is wired in a later phase,"
or actually attach the active provider's `AuthDescriptor` in
`assemble_harness_collection` and route dispatch through `dispatch_stream`.

### Low

**F-08 — `parent_id` is assigned at enqueue time but the queue is reordered at
flush → non-monotonic parent references.**
`V: confirmed` (severity corrected medium→low). `crates/opi-agent/src/harness.rs:396-427`
(`enqueue_message`/`enqueue_extension_state` set `parent_id = last_entry_id` at
enqueue), `:155-161` (`drain_ordered` reorders by `(kind_rank, order)`).
Trace: enqueue `ExtensionState` then `Message` → insertion order assigns
`msg.parent = ext.id`, but flush reorders to `[msg, ext]`, so the durable log
contains a forward reference (`msg` parents a later `ext`). *Impact:*
`SessionTree::from_entries` keys by id in a `HashMap` and tolerates this for
Message/Compaction, and the seam is unused in production, so impact is low — but
the invariant is fragile and undocumented. *Fix:* compute `parent_id` at flush
time in drain order, or enforce/document that extension-state writes must follow
their companion message in the same batch.

**F-09 — `abort()` leaves the wrapped `Agent`'s `CancellationToken` permanently
cancelled; "reusable" doc overstates.**
`V: confirmed`. `crates/opi-agent/src/harness.rs:502-516` (abort, doc `:506`
"reset to Idle (reusable)"); `crates/opi-agent/src/agent.rs:209-215`
(`Agent::abort` → `self.cancel.cancel()`). `tokio_util::sync::CancellationToken`
is one-shot with no un-cancel. *Impact:* post-abort the harness is structurally
Idle but the underlying agent cannot run another turn; "reusable" is true only
for the phase machine. Latent in Phase 10 (unused). *Fix:* re-create the
agent's token after abort (needs an `Agent` API or reconstruction —
`AgentHarness::new` takes `Agent` by value so it cannot today), or correct the
doc to "structurally reset to Idle; the wrapped agent's cancellation is
permanent."

**F-10 — `AgentHarness` and `SessionFacade` are `!Send`/`!Sync`.**
`V: direct`. `crates/opi-agent/src/harness.rs:289-294` (`pub trait
HarnessSession` with no bounds), `:329` (`session: Box<dyn HarnessSession>`),
`:616-625` (`pub trait SessionRepo`, no bounds), `:721`
(`repo: Box<dyn SessionRepo>`). Plain trait objects without `Send + Sync`
supertraits make both types non-`Send`. *Impact:* an embedder cannot hold the
harness across `await` points in a multi-task runtime, wrap it in `Arc<Mutex>`,
or spawn a task owning it — another concrete obstacle to production adoption in
tokio. *Fix:* add `: Send + Sync` to both traits (matching opi-agent's other
trait-object conventions), or document the single-threaded constraint
explicitly.

**F-11 — `MetadataProvider::stream()` returns an empty stream — latent dispatch
footgun.**
`V: confirmed`. `crates/opi-coding-agent/src/provider_factory.rs:200-226`.
`MetadataProvider` implements `Provider` only to contribute `id()`/`models()`
metadata; `stream()` returns `stream::empty()`. *Impact:* safe today (dispatch
goes through the held `Agent`, not the collection), but the type system invites
a future misuse the compiler will not catch. *Fix:* have `stream()` return an
error stream, or restrict construction so it cannot be registered as a
dispatchable provider.

**F-12 — `SessionFacade::active_tip` is O(n) per call, uncached, full re-read.**
`V: partial` (mechanism confirmed; impact low — no production caller).
`crates/opi-agent/src/harness.rs:807-812`; `session_branch.rs:60-161`.
`active_tip` calls `repo.load()` (full JSONL re-read) and rebuilds
`SessionTree::from_entries` (two `HashMap`s + sort) on every call. *Fix:* cache
the tree/tip on `SessionFacade`, invalidate on successful flush.

**F-13 — Code duplication between `AgentHarness` and `SessionFacade`; two
overlapping trait pairs.**
`V: confirmed` (severity corrected medium→low). `crates/opi-agent/src/harness.rs`:
`enqueue_message`/`enqueue_extension_state` `:396-427` vs `:745-774`;
`flush_internal` `:545-557` vs `:834-846`; `record_save_point` `:559-569` vs
`:848-860`; `next_id` `:571-574` vs `:862-865`; `next_timestamp` `:576-581` vs
`:867-872`. Separately, `HarnessSession`+`JsonlHarnessSession` (`:289-320`)
overlaps `SessionRepo`+`JsonlSessionRepo` (`:616-678`) — both define
`append`+`message_count` with parallel JSONL impls. *Impact:* two implementations
of the flush contract that can drift; a flush-semantics fix must be applied in
both. *Fix:* extract a shared flusher the two types delegate to; collapse
`HarnessSession` into `SessionRepo` (or retire one) once `AgentHarness` is
rewired.

**F-14 — `JsonlHarnessSession` has no `open`/resume constructor (asymmetric with
`JsonlSessionRepo`).**
`V: confirmed`. `crates/opi-agent/src/harness.rs:297-320` (only `create`) vs
`:648-656` (`JsonlSessionRepo::open` counts existing entries). *Impact:* an
`AgentHarness` cannot be constructed over an existing/resumed session — blocking
`--resume`/`--fork` from ever routing through it. *Fix:* add
`JsonlHarnessSession::open` mirroring the repo, or document `HarnessSession` as
create-only for Phase 10.

**F-15 — `phase10_process_adapter_stays_out_of_opi_agent` is a lexical token
scan; opi-agent already carries adapter vocabulary it silently allows.**
`V: confirmed` (severity corrected medium→low).
`crates/opi-coding-agent/tests/productized_packages_docs.rs:1697-1770`
(tokens at `:1712-1720`). opi-agent already defines `SOURCE_ADAPTER`
(`diagnostic.rs:205`), six `CODE_ADAPTER_*` constants (`:254-259`),
`"adapter_error"` (`:293`), and `HookSkipped` (`trace.rs:79-83`) — none in the
token list. *Impact:* the guard enforces "no adapter under these 7 names," not
"no adapter hosting"; a synonym-named bridge would slip through. *Fix:* make the
allowed adapter vocabulary explicit (documented allowlist) and narrow the
test's claim to "fixed-token lexical regression."

**F-16 — `provider_policy_is_centralized` scans raw (non-comment-stripped) text,
unlike its sibling guards.**
`V: confirmed`. `crates/opi-coding-agent/tests/provider_factory.rs:442-454`.
Sibling tests (`session_facade.rs:332-381`,
`harness_resource_integration.rs:1159-1246`) strip comments first; this one does
`text.contains(token)` on raw `fs::read_to_string`. *Impact:* a doc-comment
name-drop in another `src/` file would register as a false policy leak; the guard
is fragile. *Fix:* apply the same `strip_rust_comments` helper before scanning.

**F-17 — Agent-before-extension durable-order test proves only the easy
direction.**
`V: reported`. `crates/opi-agent/tests/harness.rs:134-159,357-370`. Both
assertions enqueue message-then-extension, where insertion order already equals
flush order. *Fix:* add an extension-then-message variant asserting durable
order is `[Message, ExtensionState]` and the `parent_id` chain is consistent
(cross-refs F-08).

**F-18 — Module docs overstate where generic semantics live.**
`V: confirmed`. `crates/opi-coding-agent/src/harness.rs:13-19,84-93` ("generic
turn lifecycle … live in `opi_agent::harness`"); `provider_factory.rs:10-24`
("routes … through the collection"). At runtime those concerns still live in
`CodingHarness`; the collection only provides metadata. *Fix:* use present tense
for what `AgentHarness` *exists-as* (a published, contract-tested seam) and
conditional/future tense for what it *owns-at-runtime*; tighten the factory doc
to "model metadata routes through the collection; auth-gated dispatch is future."

**F-19 — `compat_metadata_for` returns default for OpenAI-compatible built-ins.**
`V: confirmed`. `crates/opi-coding-agent/src/provider_factory.rs:871-873`.
`compat_metadata_for(_provider_id)` returns `CompatMetadata::default()` for all
nine built-ins, including `openai`/`openrouter`/`mistral`/`openai-responses`,
which *are* OpenAI-compatible. *Impact:* a consumer querying
`collection.compat("openrouter").openai_compatible` gets `false` — misleading
(SC3 "compatibility flags have a clear home" half-met). No `--list-models`
impact (listing ignores compat). *Fix:* set `openai_compatible = true` for the
known-compatible built-ins, or document that `CompatMetadata` applies only to
user-declared profiles.

**F-20 — Behavior delta: listing-path proxied-HTTP-client error message changed.**
`V: confirmed`. `crates/opi-coding-agent/src/provider_factory.rs:479-487`
(`build_proxied_client_for_listing`) vs the old per-provider builders which used
`e.to_string()`. On `opi --list-models` with a broken proxy the message is now
`"failed to build HTTP client with proxy config: <e>"` instead of the bare
`"<e>"`. *Impact:* cosmetic, low — but it is an observable change in a 766-line
extraction that claimed behavior preservation. *Fix:* restore the bare message,
or record it under `CHANGELOG.md` `[Unreleased] -> Changed`.

**F-21 — SC4 "OAuth can be added later without redesigning provider construction"
is half-true.**
`V: reported`. `crates/opi-ai/src/provider_collection.rs:25-29,96-110,344-352`.
`AuthDescriptor` is `#[non_exhaustive]` so a new variant fits without API
breakage — that part holds. But there is no auth-context object, no
credential-store abstraction, no token-rotation hook (`refresh()` is a hard
no-op), and dispatch never touches production. *Fix:* soften the doc to "the
variant enum is non_exhaustive; token refresh and production dispatch wiring
remain future work."

### Info

| ID | File:line | Note |
|----|-----------|------|
| F-22 | `harness.rs:78-82,455-458` | `end_*` called from `Idle` returns `Busy(Idle)` → renders "busy in phase Idle". Misleading; functionally correct. Add a `NotInPhase` variant. |
| F-23 | `crates/opi-ai/tests/registry.rs:187-279` | `registry_resolves_all_builtin_providers` is a *meaningful* regression (constructs all 9 providers, asserts sorted ids), not a tautology. Positive: no action. |
| F-24 | `provider_collection.rs` re-exports at `lib.rs:31-34` | 7 new pub types at crate root; only `ProviderCollection.{resolve,registry}` reached by production. Flag as unstable-and-unadopted (module doc already says 0.x). |
| F-25 | `docs/snapshots/phase4/opi-impl-state.json` | phase4-ledger hash-resync claim in commits `4015ffe`/`d16c364` is taken on trust — contamination control barred re-reading the ledger snapshots. Independent re-check deferred. |

---

## Success-criteria scorecard (independent, code-grounded)

| SC | Spec wording (paraphrase) | Independent status |
|----|---------------------------|--------------------|
| SC1 | opi-ai provider collection/auth seam exists | **Met** — seam exists, is contract-tested, redaction is real. |
| SC2 | provider construction routes through the seam | **Partially met** — metadata routes through it; auth-gated dispatch does not (F-07/headline). |
| SC3 | opi-agent owns generic harness (phase/snapshot/save-point/pending-write) | **Met as a published seam** (spec permits "seam or approved design"); not product-adopted (F-01). |
| SC4 | `CodingHarness` documented as a product wrapper | **Met via documentation only** — does not compose over `AgentHarness` (F-02). |
| SC5 | session repo/facade boundaries defined for Phase 13 | **Met as a seam** — types exist and are contract-tested; no production consumer (headline). |
| SC6 | runtime hook boundaries distinguish current/future | **Met** — boundaries documented and guard-tested (guard is lexical, F-15). |
| SC7 | existing behavior covered by regression tests | **Met** — factory extraction behavior-neutral for the paths tested (minor F-20). |
| SC8 | no ecosystem breadth feature added | **Met** — no forbidden scope leaked (verified by grep: no `opi-types`, no OAuth). |

Six of eight SCs hold up to code inspection. SC2 and SC4 are the substantive
gaps; SC3 and SC5 are met only at the "published seam" level the spec permits,
which is the source of the claim/reality tension.

## What was done well

- `SecretKey` redaction is genuine: custom `Debug`/`Display` both emit
  `<redacted>` (`provider_collection.rs:74-84`); `AuthDescriptor::resolve`
  (`:117-135`) and `CollectionError::AuthNotConfigured` (`:198-206`) never carry
  a credential value. This is the correct security posture for the auth surface.
- The complete-dispatch decision (drain the streaming `Provider` trait rather
  than add a second trait method) is sound: real providers carry the accumulated
  `partial` in the terminal `Done`/`Error` event, so `drain_to_completion`
  needs no separate accumulator.
- The `AgentHarness` phase machine, snapshot-freeze discipline, and
  pending-write `drain_ordered` (stable `sort_by_key`) are correct on inspection
  and the happy paths; abort genuinely never discards accepted writes
  (`AbortLeftPending(n)`, queue retained).
- The 766-line factory extraction is behavior-neutral for the model listing,
  picker, thinking-config, and run-mode paths that are tested
  (`model_registry: ProviderCollection` is a clean type-widening).
- No forbidden scope leaked (no OAuth, image generation, `opi-types`, etc.);
  EN/ZH doc parity maintained; the registry regression is meaningful.

## Recommended actions (prioritized)

1. **Reconcile claims with reality** (highest leverage, lowest cost). In the
   design doc, spec SC2/SC4, module docs (`harness.rs`, `provider_factory.rs`),
   and the ledger, change `production_call_sites`/`status:met` for
   `AgentHarness`/`SessionFacade`/collection-dispatch to "published unstable
   seam, not yet adopted by the product." This removes the headline defect
   without code risk.
2. **Strengthen the verification apparatus.** (a) `phase10_exit_trace_completeness`:
   drop the hardcoded `met` array or add code-truth anchors (F-04). (b)
   Broaden non-goal needles to synonym sets (F-03). (c) Strip comments in
   `provider_policy_is_centralized` for consistency (F-16). (d) Add the
   `dispatch_complete` edge-case tests (F-06).
3. **Decide the seam's fate.** Either commit to wiring (route dispatch through
   `ProviderCollection::dispatch_stream` with real `AuthDescriptor`s; give
   `AgentHarness` a turn-execution method and route `CodingHarness::prompt`
   through it; add `Send + Sync` bounds — F-01, F-10, F-14), or explicitly gate
   the unwired surface behind a feature flag / `#[doc(hidden)]` until adoption.
4. **Minor correctness hardening** (low risk): compute `parent_id` at flush time
   (F-08); fix or document the abort/cancellation reuse gap (F-09); cache
   `active_tip` (F-12); deduplicate the flusher (F-13); make `MetadataProvider`
  non-dispatchable (F-11).

## Methodology and limits

- Coverage: 7 review dimensions (10.1 provider collection, 10.2 factory,
  10.3 `AgentHarness`, 10.5 session facade, 10.4 `CodingHarness` wrapper,
  10.6/10.7 guards+docs, cross-cutting) each reading full source + tests +
  Phase 10 diffs, plus an adversarial refutation stage over all 20 high/medium
  findings.
- Adversarial stage results: **14 confirmed, 5 refuted, 1 partial**. The 5
  refuted findings (e.g. "SC4 not met" misread the doc; "D1 drops unknown
  entries" overstated; "by-value wall is a rationalization" misframed the
  deferral) were dropped or downgraded rather than relayed.
- Load-bearing claims (`production_wiring` for all three seam families, the
  `AgentHarness`/`SessionFacade` source, the provider call chain) were
  additionally verified by direct read, independent of the workflow.
- Limits: the phase4-ledger hash-resync (F-25) could not be independently
  re-checked because contamination control barred reading the ledger snapshots.
  Low-severity single-reviewer findings (F-17, F-21, F-24) were not separately
  re-verified but are consistent with the surrounding code.
