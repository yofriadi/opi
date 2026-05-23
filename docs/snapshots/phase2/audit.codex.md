# Phase 2 系统审计报告

审计日期：2026-05-23  
审计对象：`.opi-impl-state.json` 中 Phase 2 全部 16 个任务  
审计范围：`docs/opi-spec.md`、`crates/opi-ai`、`crates/opi-agent`、`crates/opi-coding-agent`、`crates/opi-tui`  
审计提交：`43a64dcfba38d05996d51c078da0e7c31a19fec7`

## 结论

Phase 2 的机械验证是通过的：ledger 标记 16/16 `passing`，本次重新运行的工作区测试、格式、clippy 和文档门也全部通过。

但按 `docs/opi-spec.md` 的 Phase 2 exit criteria 衡量，当前状态不应描述为 **runtime complete**。主要原因是多项功能已在库层、fixture 层或组件层完成，但没有完整接入 `opi` 二进制的真实用户路径：

- 多 provider 没有接入 CLI provider 构建路径。
- OpenAI Responses 和 Gemini 的 `Provider::stream()` 仍是 HTTP stub。
- session JSONL 存储存在，但 harness/runtime 不写 session，`--resume` 只打印 metadata 后退出。
- compaction engine 存在，但不会在长对话中触发、持久化或注入上下文。
- thinking、usage/cost、DiffView 主要停在 provider fixture 或 TUI component 层。

建议将 Phase 2 标记为：**任务级测试通过，审计条件通过；不建议宣称 Phase 2 runtime exit criteria 已完全满足**。

## 重新验证

| Gate | Command | Result |
| --- | --- | --- |
| Test | `cargo test --workspace --all-targets` | PASS，537 tests |
| Format | `cargo fmt --check --all` | PASS |
| Clippy | `cargo clippy --workspace --all-targets -- -D warnings` | PASS |
| Docs | `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` | PASS |

本次验证说明代码可以通过当前 CI 型机械门；以下 findings 是 runtime 完成度和 specification drift 层面的风险。

## Phase 2 任务状态核对

| Task | Ledger 状态 | 审计状态 | 说明 |
| --- | --- | --- | --- |
| 2.1 OpenAI-compatible chat provider | passing | Partial | `OpenAiChatProvider::stream()` 有真实 HTTP path，但 `opi` CLI 无法构建 OpenAI provider。 |
| 2.2 OpenRouter provider profile | passing | Partial | profile 复用 OpenAI adapter，但 CLI 不支持 `openrouter:*`；HTTP header/routing diagnostics 缺少 lifecycle 级测试。 |
| 2.3 OpenAI Responses provider | passing | Blocked | mapper/fixtures 完成，但 `Provider::stream()` 返回 HTTP streaming 未实现。 |
| 2.4 Google Gemini provider | passing | Blocked | mapper/fixtures 完成，但 `Provider::stream()` 返回 HTTP streaming 未实现。 |
| 2.5 Mistral provider | passing | Partial | profile 可复用 OpenAI adapter，但 CLI 不支持 `mistral:*`。 |
| 2.6 session v1 JSONL storage | passing | Library complete | 存储层和 contract 基础扎实；runtime 未写入 session。 |
| 2.7 session list/resume/delete | passing | Partial | list/delete 基本可用；`--resume` 不恢复上下文继续对话。 |
| 2.8 compaction | passing | Library only | engine 和 tests 存在；runtime 未触发、未持久化、未注入 prompt。 |
| 2.9 thinking/reasoning support | passing | Partial | Anthropic request body/fixtures 覆盖；agent loop 未传入 config，且 runtime 会丢弃 thinking content。 |
| 2.10 usage and cost tracking | passing | Library only | `Usage`/`CumulativeUsage`/`Pricing` 存在；没有 model pricing table、runtime accumulation 或 TUI 展示。 |
| 2.11 diff view | passing | Component only | `DiffView` widget 和 snapshots 存在；未接入 edit/patch tool path。 |
| 2.12 themes | passing | Mostly complete | 内置 default/monokai 和 config wiring 已接入；用户 theme directory 未实现。 |
| 2.13 keybindings | passing | Mostly complete | TOML parsing 和 interactive input handler 已接入。 |
| 2.14 `--json` NDJSON mode | passing | Partial | runner 层 NDJSON 有测试；受 session/compaction 未接入影响，无法覆盖完整 session event 语义。 |
| 2.15 retry/backoff/rate limits | passing | Mostly complete | agent loop retry 和 AutoRetry events 有测试；provider HTTP coverage 不均衡。 |
| 2.16 session contract tests | passing | Complete for storage contract | JSONL/tree/compaction recovery/property tests 覆盖较好。 |

## Critical Findings

### C1. Phase 2 providers are not reachable from the `opi` binary

`opi-coding-agent` 的 provider 构建路径仍只接受 `anthropic:*`：

- `crates/opi-coding-agent/src/main.rs:194-208` 只匹配 `"anthropic"`，其他 provider 全部返回 `unknown provider`。
- `crates/opi-coding-agent/src/config.rs:65-69` 的 `[providers]` resolved config 也只有 `anthropic` 字段。

影响：Phase 2 新增的 OpenAI、OpenRouter、OpenAI Responses、Gemini、Mistral 即使在 `opi-ai` 中有 crate-level tests，也不能通过实际 CLI 使用。`docs/opi-spec.md:1037` 的 “multiple providers pass contract fixtures” 只能解释为库层 fixture，而不是二进制 runtime 可用。

建议：在 `build_provider()` 中接入 `ProviderRegistry` 或显式 provider factory；扩展 config 以支持各 provider 的 API key env/base_url；增加 CLI/subprocess 级 tests 验证 `--model openai:*`、`openrouter:*`、`gemini:*`、`mistral:*` 至少能构建 mock/wiremock provider。

### C2. OpenAI Responses and Gemini provider trait paths are stubs

两个 Phase 2 provider 的真实 `Provider::stream()` 仍未实现：

- `crates/opi-ai/src/openai_responses.rs:787-793`
- `crates/opi-ai/src/gemini.rs:625-630`

当前 tests 主要调用 `stream_from_sse()` 验证 mapper 和 fixture，而不是验证真实 HTTP provider path。影响是 `ProviderRegistry` 即使能解析这些 provider，调用 trait path 仍会产生 `RequestFailed("HTTP streaming not implemented")`。

建议：实现 `/v1/responses` 和 Gemini `streamGenerateContent?alt=sse` 的 HTTP streaming、status/error mapping、cancellation 和 no-terminal-event handling；增加 wiremock lifecycle tests，覆盖 headers、request body、stream framing、429/401/500、cancellation。

### C3. Session storage is not connected to runtime persistence

`SessionWriter` 和 `SessionEntry` 只出现在 `opi-agent` storage 模块和 `session_cli` 读取/测试路径；`CodingHarness`、interactive runner、non-interactive runner 没有创建或 append JSONL session。

相关证据：

- `crates/opi-agent/src/session.rs:97-121` 定义 `SessionWriter`。
- `crates/opi-coding-agent/src/harness.rs:63-75` 创建 `AgentLoopConfig` 和 `Agent`，没有 session writer/coordinator。
- `crates/opi-coding-agent/src/main.rs:16-22` 在运行前先处理 session CLI 命令并直接返回。

影响：正常 prompt 不会产生 session 文件，因此 “sessions survive restart” 不成立。

建议：在 harness 层加入 session coordinator：创建 header、按 user/assistant/tool/compaction append entry、flush on cancellation/shutdown；增加 interactive/non-interactive E2E 验证 prompt 后生成 JSONL，重新加载后可恢复 active branch context。

### C4. `--resume` does not resume a conversation

`--resume` 当前只读取 session header/entries 并打印一行 metadata，然后返回 handled：

- `crates/opi-coding-agent/src/session_cli.rs:182-191`
- `crates/opi-coding-agent/src/main.rs:16-22`

影响：用户无法从历史 session 重建 context 并继续对话。当前实现更接近 `inspect session`，不是 resume。

建议：把 `--resume <id>` 从 early-return command 改为 runtime input：读取 JSONL、重建 active branch 的 `AgentMessage` 序列、构造 `CodingHarness` 初始状态，然后进入 interactive 或 non-interactive flow。

### C5. Compaction is not wired into the agent/harness runtime

`CompactionEngine`、`CompactionStart`、`CompactionEnd` 已定义并有 tests，但在 `opi-coding-agent` runtime 中没有引用；agent loop 也没有 token threshold/overflow trigger。

影响：长对话不会在 overflow 前压缩，compaction summary 不会写入 session，也不会进入后续 provider context。`docs/opi-spec.md:1037` 的 “long conversations compact before overflow” 未满足。

建议：增加 compaction coordinator，基于 usage/token estimate 触发 threshold/overflow/manual compaction；发出 `CompactionStart/End`，写入 `SessionEntry::Compaction`，并将 `CompactionSummary` 转换为后续 provider-visible context。

## High Findings

### H1. Thinking config is parsed but not used by agent loop, and thinking content is dropped

`[thinking]` config 已存在，默认 enabled：

- `crates/opi-coding-agent/src/config.rs:50-60`

但 `AgentLoopConfig` 没有 thinking 字段，harness 没有映射 thinking config：

- `crates/opi-coding-agent/src/harness.rs:63-67`

最终 provider request 总是使用默认 disabled thinking：

- `crates/opi-agent/src/lib.rs:94-105`

此外，agent loop 只累积 `TextDelta` 和 `ToolCallEnd`；`ThinkingStart/Delta/End` 被 `_ => None` 忽略，并且 terminal message 的 provider content 会被 `assistant_content` 覆盖：

- `crates/opi-agent/src/lib.rs:123-128`
- `crates/opi-agent/src/lib.rs:440-473`

影响：2.9 在 provider fixture 层通过，但真实 agent runtime 不会启用 thinking，也不会保留 thinking content。

建议：在 `AgentLoopConfig` 中加入 `opi_ai::provider::ThinkingConfig`，从 `OpiConfig.thinking` 映射到 provider request；更新 `process_stream_event()` 保留 `Thinking*` content，并增加 agent-loop integration test。

### H2. Usage/cost tracking is not accumulated or displayed in runtime

`opi-ai` 暴露 `Usage`、`CumulativeUsage`、`Pricing`、`calculate_cost()`，TUI `StatusBar` 也支持 `token_count`，但 interactive state 没有从 `Done`/`MessageEnd` 累积 usage，也没有调用 `Shell::token_count()`.

证据：

- `crates/opi-ai/src/stream.rs:70-145`
- `crates/opi-tui/src/render.rs:59-60`
- `crates/opi-tui/src/render.rs:108`
- `crates/opi-coding-agent/src/interactive.rs` 没有 usage/cost accumulation。

影响：2.10 只是库层计算能力；用户看不到 token/cost，JSON/session 也没有累计 cost summary。

建议：在 runner/harness 层累计 `AssistantMessage.usage`，提供 pricing lookup API 或明确 cost 仍为外部输入；把 token/cost summary 接入 TUI status bar 和 JSON/session events。

### H3. DiffView is not used by edit/patch tool paths

`DiffView` 只在 `opi-tui` 中定义和测试：

- `crates/opi-tui/src/diff_view.rs:40-47`
- `crates/opi-tui/src/diff_view.rs:193`

`opi-coding-agent/src` 没有引用 `DiffView`。影响：2.11 的 widget/snapshot 已完成，但 “edit/patch visualization” 对用户不可达。

建议：让 `edit` tool result 带上 before/after 或 patch details，并在 interactive TUI 的 tool result view 中渲染 `DiffView`；补集成或 snapshot test。

### H4. Provider lifecycle coverage is uneven

`crates/opi-ai/tests/provider_lifecycle.rs` 明确只覆盖 `AnthropicProvider::stream()`。OpenAI-compatible adapter 有真实 HTTP path，但 OpenAI/OpenRouter/Mistral 缺少同级 wiremock tests；OpenAI Responses/Gemini 则因 `stream()` stub 更严重。

影响：headers、status mapping、request path、body serialization、cancellation、no-terminal-event 等风险主要靠 fixture tests 间接覆盖。

建议：为 OpenAI Chat adapter 增加 lifecycle suite，并用 profile tests 验证 OpenRouter headers (`HTTP-Referer`, `X-Title`) 和 Mistral base URL/path。

## Medium Findings

### M1. Ledger 证据有轻微漂移

- `.opi-impl-state.json` task 2.9 的 `behavioral_tests` 指向 `crates/opi-ai/tests/thinking_fixtures.rs`，但该文件不存在；实际 thinking tests 在 `crates/opi-ai/tests/anthropic_fixtures.rs`。
- 多个 `verified_at_commit` 使用短 hash，而 Phase 1 大多使用 40 字符 hash。
- task 2.13 `last_attempt` 为 `null`，格式与其他 passing tasks 不一致。

影响：不影响代码运行，但降低审计追踪精度。

建议：刷新 `.opi-impl-state.json` 的 evidence path、hash 格式和 nullable 字段约定。

### M2. Config schema does not expose provider-specific sections beyond Anthropic

`OpiConfig.providers` 只有 `anthropic`。即使 `build_provider()` 扩展 provider matching，也缺少 OpenAI/OpenRouter/Gemini/Mistral 的 API key env/base_url 配置入口。

建议：新增 provider-specific config sections，或引入 generic `[providers.<id>]` map；保持 API key 只从 env 解析，避免写入 session/log。

### M3. JSON mode contract is valid but not full session contract

`--json` runner 输出 schema header 并把 `AgentEvent` 包装为 `AgentSessionEvent`。这满足当前 tests，但由于 session persistence 和 compaction runtime 未接入，JSON mode 不能真实覆盖 session lifecycle、compaction lifecycle 或 resume lifecycle。

建议：在 C3/C5 修复后增加 JSON mode tests，覆盖 `SessionInfoChanged`、`CompactionStart/End`、resume 后事件序列，以及 CLI subprocess stdout/stderr/exit-code。

## 覆盖良好的部分

- OpenAI-compatible chat mapper/request body/compat config fixture 覆盖较扎实。
- OpenRouter 和 Mistral 复用 OpenAI chat adapter，结构上避免了重复协议实现。
- Session JSONL storage、tree reconstruction、compaction recovery 和 property tests 质量较好。
- Retry/backoff 在 `opi-ai` primitive 和 `opi-agent` loop 两层都有测试。
- TUI theme/keybinding/DiffView snapshot 覆盖充足，组件层设计清晰。
- 当前工作区无 `unsafe` 使用，workspace dependency 和 lockstep versioning 保持一致。

## 建议修复顺序

1. 接入 CLI provider factory/config，让 Phase 2 provider 可以从 `opi --model ...` 实际使用。
2. 实现 OpenAI Responses 和 Gemini 的真实 HTTP `Provider::stream()`，补 lifecycle tests。
3. 接入 session writer，确保 prompt runtime 会创建和 append JSONL。
4. 实现真正的 resume：重建 active branch context 并继续 agent flow。
5. 接入 compaction coordinator，发 event、写 session、注入后续 prompt context。
6. 贯通 thinking config，并修复 agent loop 丢弃 thinking content 的问题。
7. 接入 usage/cost accumulation 和 TUI/JSON/session 展示。
8. 接入 DiffView 到 edit/patch runtime。
9. 刷新 ledger evidence，修正 2.9 路径和 hash 字段一致性。

## Phase Exit 建议

如果 Phase 2 的目标定义为“库层 provider/parser、session storage primitives、TUI components、retry primitives 和 JSON framing 已具备测试基础”，当前可以接受。

如果目标按 `docs/opi-spec.md:1037` 的 exit criteria 执行：sessions survive restart、multiple providers pass contract fixtures、long conversations compact before overflow、JSON mode has schema tests，则当前仍有 blockers。建议在进入 Phase 3 前安排一次 Phase 2 hardening pass，至少关闭 C1-C5。
