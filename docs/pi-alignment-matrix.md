# pi Alignment Matrix

## Scope

This document compares `opi` against `.repo/pi-0.75.3` by semantic behavior and product workflow. It is not a TypeScript API compatibility checklist.

The intended target is:

- preserve `pi` runtime semantics where users or embedders depend on them;
- keep the Rust crate boundaries native to Rust ownership, traits, and release practice;
- keep workflow-heavy features such as MCP, sub-agents, plan mode, todos, and permission gates outside core.

## Alignment Levels

| Level | Meaning |
|---|---|
| Full | Implemented with equivalent user-visible or library-visible behavior. |
| Partial | Implemented as a substrate or narrower Rust-native equivalent. |
| Deliberate Divergence | Intentionally different because Rust architecture or project scope differs. |
| Missing | Present in `pi` and in scope for `opi`, but not implemented. |
| Out of Scope | Present in `pi`, but excluded from the current `opi` scope. |

## Package Alignment

| pi package | opi crate | Level | Current state | Next action |
|---|---|---|---|---|
| `@earendil-works/pi-ai` | `opi-ai` | Partial | Core provider streaming, provider registry, model metadata, image input, usage/cost, retry, proxy, custom provider/model registration, and config-driven OpenAI-compatible profiles are present. `pi` still has broader first-class provider coverage, OAuth providers, and image-generation surfaces. | Add first-class providers only when the wire protocol or auth model is materially different; keep OAuth as a separate product decision. |
| `@earendil-works/pi-agent-core` | `opi-agent` | Full | Agent loop semantics, hooks, tool batching, queues, sessions, compaction, SDK types, extensions, and streaming proxy primitives are represented. The public surface stays in `lib.rs` while loop internals live in `agent_loop.rs`. | Keep runtime internals deep and focused without introducing a shared types crate. |
| `@earendil-works/pi-coding-agent` | `opi-coding-agent` | Partial | CLI modes, built-in tools, config, sessions, context files, images, JSON/RPC, resources, packages, skills, prompt fragments, themes, custom provider registration, RPC extension commands, `/tree`, `/fork`, `/clone`, `--fork`, same-file active-branch continuation via runtime `parent_id`/`leaf` entries, `opi package add/remove/list/doctor` CLI, manifest V2 with `[adapter]` declarations, `process-jsonl` adapter hosting via `opi-extension-jsonl-v1`, and adapter-to-runtime bridging are present. Product workflow breadth is still narrower than `pi`. | Keep adapter protocol evolving; add broader adapter kinds after API stabilization. |
| `@earendil-works/pi-tui` | `opi-tui` | Deliberate Divergence | `opi-tui` uses `ratatui`/`crossterm` widgets instead of copying `pi`'s TypeScript terminal renderer. It has transcript, markdown/code, diff, pickers, branch picker, themes, keybindings, terminal-image primitives, and CJK display-width snapshot coverage for branch/session pickers. | Keep it scoped to coding-agent needs unless a separate reusable TUI product decision is made. |

## Phase Alignment

| Phase | Feature family | opi crate | pi manifestation | Level | Current state | Next action |
|---:|---|---|---|---|---|---|
| 1 | Provider trait, stream events, Anthropic provider | `opi-ai` | `pi-ai` provider and stream contracts | Full | Provider-neutral stream API and Anthropic SSE are implemented. | Maintain fixture coverage for stream lifecycle and in-band errors. |
| 1 | Provider registry | `opi-ai` | `pi-ai` API/model/provider registry concepts | Full | `provider:model` resolution and capabilities exist. | Route more model listing and profile behavior through the registry. |
| 1 | Agent loop, `Agent`, hooks, queues | `opi-agent` | `pi-agent-core` `agentLoop`, hooks, steering/follow-up | Full | Runtime semantics are represented and the loop implementation is now isolated in a focused internal module. | Preserve the public `opi_agent::agent_loop` export while keeping private helpers internal. |
| 1 | Tool trait and schema validation | `opi-agent` | TypeBox tool schemas and runtime validation | Full | Rust tool trait plus JSON Schema validation are present. | Keep validation at model/tool boundary. |
| 1 | Coding tools | `opi-coding-agent` | `read`, `write`, `edit`, `bash`, plus read-only navigation | Partial | Interactive defaults match `pi`; `grep`, `find`, `ls`, and extra `glob` exist. | Keep `glob` as extra convenience and avoid making it a core dependency. |
| 1 | TUI shell and markdown/code rendering | `opi-tui` | `pi-tui` terminal UI components | Partial | Ratatui shell, transcript, markdown, code, and snapshots exist. Picker snapshots now include CJK-width labels. | Add only primitives needed by coding-agent workflows. |
| 1 | Config and non-interactive mode | `opi-coding-agent` | `pi` print mode and settings | Full | TOML config and text mode exist with Rust-native storage. | Keep TOML; do not chase `pi` JSON config compatibility by default. |
| 2 | OpenAI-compatible, OpenAI Responses, OpenRouter, Gemini, Mistral | `opi-ai` | `pi-ai` provider families | Partial | Core Phase 2 provider set and config-driven OpenAI-compatible profiles exist. | Use profile configuration for provider breadth instead of hardcoding every compatible provider. |
| 2 | Sessions and resume/delete/list/fork | `opi-agent`, `opi-coding-agent` | `pi` session manager | Partial | Append-only JSONL sessions, resume, list/delete, `--fork`, interactive `/fork`/`/clone` new-session paths, and same-file active-branch continuation with runtime `parent_id`/`leaf` entries are implemented. | Improve package-manager workflow and richer tree metadata display. |
| 2 | Compaction | `opi-agent`, `opi-coding-agent` | `pi` compaction and summarization flow | Full | Threshold/manual/overflow compaction primitives and session events exist. | Keep branch-aware compaction tests current. |
| 2 | NDJSON event mode | `opi-coding-agent` | `pi` JSON event mode | Full | `--json` emits versioned session and agent events. | Keep schema/event contract tests. |
| 2 | Thinking, usage, cost, retry | `opi-ai`, `opi-coding-agent` | `pi` model options and accounting | Partial | Thinking, usage accumulation, best-effort cost, and retry/backoff exist. | Keep provider capability checks conservative. |
| 2 | Diff, themes, keybindings | `opi-tui`, `opi-coding-agent` | `pi-tui` and coding-agent settings | Partial | Diff rendering, themes, and keybindings exist. | Avoid broad TUI-framework expansion unless required by commands. |
| 3 | Bedrock, Azure OpenAI, Vertex AI | `opi-ai` | `pi-ai` enterprise providers | Partial | Wire adapters exist. | Decide OAuth/ADC/profile scope separately; do not silently add credential stores. |
| 3 | Image input and image tool results | `opi-ai`, `opi-agent`, `opi-coding-agent` | `pi` attachments and multimodal messages | Partial | Image attachments and image result serialization exist. | Add broader attachment types only through explicit product plans. |
| 3 | Terminal image rendering | `opi-tui` | `pi-tui` image support | Full | Terminal image protocol detection/rendering exists. | Maintain cross-terminal snapshots/smoke checks. |
| 3 | Context files | `opi-coding-agent` | `AGENTS.md` / `CLAUDE.md` context loading | Full | Workspace-ancestor and user-config context loading exists. | Keep `OPI.md` intentionally excluded unless a later migration plan changes it. |
| 3 | Tool selection and safety hooks | `opi-coding-agent`, `opi-agent` | `pi` tool allowlists and extension-mediated safety | Full | Tool flags and mutating-tool opt-in policy exist. | Keep permission popups outside core. |
| 3 | `find` / `ls`, completions, model/session picker | `opi-coding-agent`, `opi-tui` | `pi` CLI tools and interactive UX | Partial | Commands/tools/pickers exist. | Improve session tree UX rather than adding unrelated widgets. |
| 3 | Proxy and HTTP pooling | `opi-ai` | `pi-ai` proxy/provider HTTP support | Full | Per-provider proxy and standard proxy env fallback exist. | Keep secret redaction and no-proxy behavior covered. |
| 4 | RPC JSONL and SDK event/command model | `opi-agent`, `opi-coding-agent` | `pi` RPC/SDK modes | Full | Strict JSONL, correlated responses, async events, shared SDK types, and `extension_command` dispatch exist. | Keep protocol versioned and reject unsupported runtime mutations honestly. |
| 4 | Extension hooks, tools, commands, messages, state | `opi-agent`, `opi-coding-agent` | `pi` TypeScript extensions | Partial | In-process Rust extension API, RPC/SDK command dispatch, and process-JSONL adapter bridging (tools, commands, hooks, events, state, cancellation) exist for embedders and external packages. | Keep adapter protocol evolving; add gRPC or other adapter kinds after API stabilization. |
| 4 | Resource discovery | `opi-coding-agent` | `pi` extension/resource loading | Partial | User/project/explicit resource metadata loading exists. | Ensure metadata is wired into interactive, non-interactive, and RPC paths consistently. |
| 4 | Skills and prompt fragments | `opi-coding-agent` | `pi` skills and prompt templates | Partial | Progressive discovery exists. | Add invocation and metadata paths without making prompt fragments implicit core commands. |
| 4 | Themes | `opi-coding-agent`, `opi-tui` | `pi` themes | Partial | Theme discovery and built-in fallback exist. | Add tests for precedence and missing theme diagnostics. |
| 4 | Packages | `opi-coding-agent` | `pi` packages and package manager | Partial | `package.toml` discovery, composition, `opi package add/remove/list/doctor` CLI, manifest V2 with adapter declarations, and process-JSONL adapter hosting via `opi-extension-jsonl-v1` are present. | Keep adapter kinds extensible; do not claim marketplace/registry support unless a later product plan adds it. |
| 4 | Custom provider/model registration | `opi-ai`, `opi-coding-agent` | `pi` custom provider extension points | Partial | Registry registration exists; configured profiles feed runtime provider construction and `--list-models`. | Feed extension-provided providers into end-user runtime paths when an extension/package adapter is productized. |
| 4 | Branch selection | `opi-agent`, `opi-coding-agent`, `opi-tui` | `pi` session tree, fork, clone, branch selection | Partial | `/branch` and `/tree` open the branch/tree picker; `/fork`, `/clone`, and `--fork` create new parented sessions from the active branch; continuing from a selected branch tip writes a new same-file sibling path. | Improve richer branch metadata display and package-level workflows. |
| 4 | Streaming proxy | `opi-agent` | `pi` process integration/proxy surfaces | Partial | Streaming proxy primitives exist. | Clarify sync/async I/O semantics and production wiring. |
| 4 | MCP, sub-agent, plan mode, todo, permission gate examples | examples/packages | `pi` keeps workflow-heavy features outside core | Full | Examples/package scaffolds exist outside core. | Keep them outside built-in CLI unless routed through extension/package registration. |
| 5 | Package store, CLI, manifest V2, adapter protocol, adapter host, adapter bridge, example adapters | `opi-coding-agent`, `opi-agent` | `pi` package manager and extension adapters | Partial | `opi package add/remove/list/doctor`, local/git sources, manifest V2 with `[adapter]`, `process-jsonl` adapter hosting via `opi-extension-jsonl-v1`, adapter-to-runtime bridging for tools/commands/hooks/events/state/cancellation, and runnable example adapter packages (todo, permission-gate, protected-paths) exist. | Stabilize adapter protocol before claiming broad package ecosystem; keep npm/marketplace out of scope. |
| 7 | Reliability and observability (shared diagnostics, local trace envelope, `opi doctor`) | `opi-agent`, `opi-coding-agent` | `pi` error/diagnostics surfacing | Partial | A shared diagnostic model, redaction core, provider/runtime error classification, an opt-in unstable 0.x local trace envelope, and a network-free top-level `opi doctor` are present as a local and explicit surface. | Keep observability local and explicit; do not add telemetry, analytics, automatic session sharing, or a stable 1.0 observability protocol. |
| 8 | Agent runtime stabilization (event order, hook/tool/cancellation/SDK-RPC contracts, API surface classification) | `opi-agent`, `opi-coding-agent` | `pi-agent-core` runtime contracts | Partial | Runtime event order, hook semantics, tool scheduling/termination, cancellation, SDK/RPC command-state, diagnostics/trace wire, and a public API surface classification (supported 0.x / unstable internal / candidate removal) are documented and contract-tested. | Keep the public surface 0.x; do not add a stable 1.0 API promise, TypeScript extension API compatibility, package ecosystem expansion, a new adapter kind, web UI, provider OAuth login, in-core workflow tools, an MCP runtime, a shared opi-types crate, or a whole-loop rewrite. |

## Current Remediation Priorities

| Priority | Area | Reason | Target outcome |
|---:|---|---|---|
| P0 | Documentation truth | Version and phase status must match `Cargo.toml` and `CHANGELOG.md`. | Current docs describe the `0.5.4` workspace and keep historical `0.5.2` and `0.5.3` rows historical. |
| P1 | Session tree | Same-file branch continuation now has runtime `parent_id` and `leaf` coverage, but `pi` still has richer tree product workflows. | Improve branch metadata display and higher-level package/workflow integration. |
| P1 | Extension/package execution | Process-JSONL adapters via `opi-extension-jsonl-v1` bridge package commands, tools, hooks, events, state, and cancellation into the runtime. Adapter hosting and example packages are present. | Stabilize adapter protocol; add broader adapter kinds (gRPC, etc.) after API stabilization. |
| P1 | Provider profiles | OpenAI-compatible profiles and model metadata are config-driven and registry-backed. | Keep profile expansion policy documented; track OAuth providers separately. |
| P2 | Rust module depth | `opi-agent` crate boundary remains correct; the loop internals have been moved out of `lib.rs`. | Continue deepening large runtime areas only where it improves locality without changing crate boundaries. |

## Maintenance Rules

- Update this matrix when a phase milestone, public extension surface, package workflow, provider family, or session command changes.
- If an English doc update has a localized counterpart, update the localized counterpart in the same change.
- Do not use this matrix to justify copying TypeScript module structure into Rust.
- Do not mark `Full` unless there is a working user path or library path plus tests.
- Keep historical release rows historical; do not rewrite released version sections to match the current workspace.
