# Phase 2 系统审计报告

审计日期：2026-05-23  
审计模型：GPT-5.5  
审计范围：`.opi-impl-state.json` 中 Phase 2 全部任务，覆盖 `opi-ai`、`opi-agent`、`opi-coding-agent`、`opi-tui`

## 执行摘要

`.opi-impl-state.json` 记录 Phase 2 共 16 个任务全部为 `passing`，阶段退出状态为 `exit_criteria_met: true`，总测试数为 537，并记录 fmt、clippy、doc、test 通过。任务级证据显示 Phase 2 已完成大量实质性建设：多 provider fixture、session JSONL 存储、session CLI、compaction 引擎、thinking fixture、usage/cost 基础类型、DiffView、主题、快捷键、NDJSON 输出、retry/backoff 与 session contract tests。

本次审计结论为：**有条件通过 / 不建议视为 runtime-complete**。

主要原因是 Phase 2 的库层、fixture 层和组件层覆盖较强，但若按 `docs/opi-spec.md` 的 Phase 2 目标衡量，仍有多处关键运行时路径没有闭环：

- OpenAI Responses 与 Gemini 的 `Provider::stream()` 仍是 HTTP 占位实现，生产 provider trait 路径不可用。
- session JSONL 存储已实现并有 contract tests，但 `CodingHarness`/interactive/non-interactive 运行时未写入 session。
- `--resume` 当前只读取并打印 session 元数据，不会恢复 Agent 上下文后继续对话。
- compaction 引擎已实现，但未接入 harness/agent runtime，未触发 `CompactionStart/End`，也未写入 session entry 或注入 prompt context。
- thinking 配置已解析、Anthropic fixture 已覆盖，但 `[thinking]` 未传入 `Request.thinking`，运行时等价于未启用。
- DiffView、usage/cost 主要停留在组件或库函数层，尚未接入真实交互 UI。

因此，Phase 2 可以记录为“任务级测试通过，核心模块已落地”；但若以“多 provider + session persistence + compaction + JSON mode 全部用户路径可用”为阶段退出标准，仍应保留审计风险项并优先修复 Critical/High 问题。

## 审计依据

### 状态文件

- `.opi-impl-state.json`
  - `current_phase`: `2`
  - Phase 2 任务数量：16
  - 任务状态：16/16 `passing`
  - `phase_exit.2.completed_at`: `2026-05-23T12:00:00Z`
  - `phase_exit.2.exit_criteria_met`: `true`
  - `phase_exit.2.evaluator_summary`: `All 16 Phase 2 tasks passing. 537 tests green. All cross-cutting gates pass: fmt, clippy, doc.`

### 规范与源码范围

- `docs/opi-spec.md`
- `crates/opi-ai`
- `crates/opi-agent`
- `crates/opi-coding-agent`
- `crates/opi-tui`

### 核对的主要测试证据

状态文件记录的主要验证证据包括：

- `cargo fmt --check --all`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace --all-targets`
- `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`
- 任务级测试：
  - `crates/opi-ai/tests/openai_chat_fixtures.rs`
  - `crates/opi-ai/tests/openrouter_fixtures.rs`
  - `crates/opi-ai/tests/openai_responses_fixtures.rs`
  - `crates/opi-ai/tests/gemini_fixtures.rs`
  - `crates/opi-ai/tests/mistral_fixtures.rs`
  - `crates/opi-ai/tests/usage_cost.rs`
  - `crates/opi-ai/tests/retry_backoff.rs`
  - `crates/opi-agent/tests/session_storage.rs`
  - `crates/opi-agent/tests/compaction.rs`
  - `crates/opi-agent/tests/retry_agent.rs`
  - `crates/opi-agent/tests/session_contract.rs`
  - `crates/opi-coding-agent/tests/session_cli.rs`
  - `crates/opi-coding-agent/tests/json_mode.rs`
  - `crates/opi-tui/tests/diff_view_snapshots.rs`
  - `crates/opi-tui/tests/theme_snapshots.rs`
  - `crates/opi-tui/tests/keybindings.rs`
  - `crates/opi-coding-agent/tests/keybindings_config.rs`

本次审计以只读源码审查和状态文件证据核对为主，未重新执行完整测试套件。

## Phase 2 状态核对

| 任务范围 | 状态文件结论 | 审计结论 |
| --- | --- | --- |
| 2.1 OpenAI-compatible chat provider | `passing` | 基本通过。SSE fixture 与 compat config 覆盖较好，HTTP stream 路径已实现；缺少同 Anthropic 级别的 provider lifecycle wiremock 覆盖。 |
| 2.2 OpenRouter provider profile | `passing` | 条件通过。profile 复用 OpenAI chat adapter，但 routing diagnostics 和 headers 的运行时断言不足。 |
| 2.3 OpenAI Responses provider | `passing` | 应降级为 partial。fixture mapper 完成，但 `Provider::stream()` 仍返回 HTTP 未实现错误。 |
| 2.4 Google Gemini provider | `passing` | 应降级为 partial。fixture mapper 完成，但 `Provider::stream()` 仍返回 HTTP 未实现错误。 |
| 2.5 Mistral provider | `passing` | 条件通过。复用 OpenAI chat adapter 可运行；DoD 中 “Mistral-specific headers/auth” 与实现的标准 Bearer auth 不完全一致。 |
| 2.6 session v1 JSONL storage | `passing` | 存储层通过。JSONL、serde、恢复、append 测试扎实，但未接入运行时 session 写入。 |
| 2.7 session list/resume/delete | `passing` | list/delete 基本通过；resume 只读元数据后退出，不满足真实恢复对话语义。 |
| 2.8 compaction | `passing` | 引擎层通过；workspace 运行时未接入，未触发、未持久化、未注入 prompt。 |
| 2.9 thinking/reasoning support | `passing` | Anthropic provider fixture 通过；config 到 runtime 未贯通。 |
| 2.10 usage and cost tracking | `passing` | 库层通过；无内置 pricing table，无 agent/TUI 累积和展示路径。 |
| 2.11 diff view | `passing` | 组件和 snapshot 通过；未接入 edit/patch 用户路径。 |
| 2.12 themes | `passing` | 基本通过。内置 default/monokai、组件迁移、config wiring 已完成；用户主题目录未实现。 |
| 2.13 keybindings | `passing` | 基本通过。TOML parsing、key combo、interactive handler 已接线；默认值与 spec 示例存在漂移。 |
| 2.14 `--json` NDJSON mode | `passing` | runner 层通过；缺少 CLI subprocess E2E 和 AutoRetry NDJSON 场景。 |
| 2.15 retry/backoff/rate limits | `passing` | agent loop 和 retry 原语通过；header 覆盖和 JSON runner 层测试不足。 |
| 2.16 session contract tests | `passing` | 通过。存储契约、树重建和 property tests 覆盖较好。 |

## 分域审计

### `opi-ai`

`opi-ai` 是 Phase 2 增量最大的 crate。OpenAI-compatible chat adapter 已具备较完整的 request body 构建、SSE parse、tool call delta、usage、error、compat config 与 HTTP stream 路径。OpenRouter 和 Mistral profiles 复用该 adapter，整体方向合理，避免了重复实现。

Anthropic thinking fixture、usage/cost 基础类型、retry 原语也已落地：

- `Usage` 增加 `cache_read_tokens` 和 `cache_write_tokens`。
- `CumulativeUsage`、`Pricing`、`CostBreakdown`、`calculate_cost()` 提供了计算基础。
- `RetryConfig`、`ProviderError::is_retryable()`、`parse_retry_after()` 和 exponential backoff 原语已实现。
- Anthropic provider 支持 `ThinkingStart/Delta/End` 与 `budget_tokens` request body。

主要风险集中在 provider trait 的真实生产路径与 DoD 文字覆盖：

- `crates/opi-ai/src/openai_responses.rs` 的 `Provider::stream()` 仍是 “HTTP streaming not implemented”。
- `crates/opi-ai/src/gemini.rs` 的 `Provider::stream()` 同样未实现 HTTP streaming。
- OpenRouter/Mistral 虽然可通过 OpenAI chat adapter 运行，但对应测试大量使用 `stream_from_sse()`，缺少 headers、HTTP status、cancel、backpressure 等 wiremock lifecycle 级验证。
- 2.10 声称来自 model pricing table 的 cost calculation，但当前只有 `Pricing` 结构体和计算函数，没有 model -> pricing lookup。
- retry DoD 提到 `x-ratelimit-*`，当前只覆盖 `x-ratelimit-reset` 与 seconds 形式 `Retry-After`，未覆盖 HTTP-date 或更多 provider header。

审计结论：`opi-ai` 的 fixture 契约层完成度高，但 provider runtime 可用性不均衡。OpenAI chat、OpenRouter、Mistral 接近可用；Gemini 与 Responses 仍不能算生产 provider 完成。

### `opi-agent`

`opi-agent` 在 Phase 2 中新增了 session、compaction、session event、retry 和 contract test 能力：

- `SessionHeader`、`SessionEntry`、`SessionWriter`、`SessionReader` 已实现 append-only JSONL。
- `AgentMessage` 扩展了 `CompactionSummary`、`BranchSummary`、`Custom`。
- `AgentSessionEvent` 定义了 agent/session 层事件，包括 compaction、retry、queue、thinking level 等。
- `CompactionEngine`、`CompactionHooks`、trigger 判断、summary 输出均有集中测试。
- agent loop 已加入 retry loop，并能发射 `AutoRetryStart/End`。
- `session_contract.rs` 使用 deterministic tests 和 proptest 覆盖 JSONL round-trip、tree reconstruction、compaction recovery 与不变量。

这些都是扎实的库层工作。但与 `docs/opi-spec.md` 的 harness 流程相比，运行时集成仍缺关键闭环：

- `SessionWriter` 没有被 `CodingHarness`、interactive runner 或 non-interactive runner 调用。
- `CompactionEngine` 没有被 agent turn、overflow 或 manual trigger 调用。
- `CompactionSummary` 在 hooks 的 `convert_to_llm` 路径中会被过滤，无法成为 prompt layer。
- `AgentSessionEvent` 类型虽然存在，但多数 session-level 事件没有真实发射路径。

审计结论：`opi-agent` 的 Phase 2 存储和引擎原语可以作为后续集成基础，但 session persistence 与 compaction 还不是端到端运行时功能。

### `opi-coding-agent`

Phase 2 的 CLI/runtime 工作包括 session commands、JSON mode、config wiring、retry config 和 interactive keybinding/theme wiring。

完成度较好的部分：

- `--list-sessions`、`--delete-session` 已有 path traversal 防护和 subprocess 测试。
- `--json` runner 会输出 schema header，并将 agent event 包装为 NDJSON。
- `--json` 隐含 non-interactive 的修复已记录在 ledger。
- `[keybindings]` 和 `[defaults].theme` 已从 config 传入 interactive path。
- `[retry]` config 已传入 `AgentLoopConfig`。

主要缺口：

- `--resume` 当前不恢复上下文、不启动 agent，只打印 session metadata 后退出。
- session storage 没有由 harness 创建或追加 entry。
- compaction config 表未接入，compaction 引擎没有 runtime coordinator。
- `[thinking]` 已解析但没有传入 `Request.thinking`。
- `--json` 缺少子进程级测试来证明 stdout 纯 NDJSON、stderr 仅人类日志、exit code 符合 CLI wiring。

审计结论：CLI command surface 部分可用，但 Phase 2 最核心的“sessions survive restart”尚未形成真实用户路径。

### `opi-tui`

`opi-tui` 的 Phase 2 组件工作质量较好：

- `DiffView` 实现 LCS diff、unified hunk、context 行和 snapshot 覆盖。
- `Theme` 提供 27 个语义色字段，内置 default 和 monokai。
- Shell、MessageList、StatusBar、InputEditor、ToolCallView、MarkdownView、CodeBlock、DiffView 均可接收 theme。
- `KeyCombo` 和 `Keybindings` 已有 parsing、默认值和配置映射测试。

主要问题是组件没有充分接入真实交互状态：

- `DiffView` 没有被 edit/patch 工具路径、`ToolCallView` 或 `interactive.rs` 调用。
- `StatusBar` 有 token count 占位，但 interactive path 没有从 provider `Usage` 累积 token 或 cost。
- thinking stream event 没有在 TUI 中展示或折叠。
- 自定义主题目录 `~/.config/opi/themes/` 尚未实现，当前只有内置主题。

审计结论：TUI Phase 2 的组件库增量基本通过，但与用户可见 runtime 的连接仍偏薄。

## 严重问题清单

### Critical

| 编号 | 位置 | 问题 | 影响 | 建议 |
| --- | --- | --- | --- | --- |
| C1 | `crates/opi-ai/src/openai_responses.rs` | `Provider::stream()` 仍返回 HTTP streaming 未实现。 | OpenAI Responses provider fixture 可通过，但生产 trait 路径不可用。 | 实现 `/v1/responses` HTTP SSE stream、status mapping、cancel/backpressure，并补 wiremock lifecycle tests。 |
| C2 | `crates/opi-ai/src/gemini.rs` | `Provider::stream()` 仍返回 HTTP streaming 未实现。 | Gemini provider 无法作为真实 provider 使用。 | 实现 `streamGenerateContent` HTTP stream、错误映射、取消，并补生命周期测试。 |
| C3 | `crates/opi-coding-agent/src/harness.rs` / `runner.rs` / `interactive.rs` | session JSONL storage 未接入运行时。 | 对话不会持久化，Phase 2 “sessions survive restart” 不成立。 | 在 harness 层创建 session，按 turn/tool/compaction append entry，并补 E2E。 |
| C4 | `crates/opi-coding-agent/src/session_cli.rs` / `main.rs` | `--resume` 只读取并打印 metadata 后退出。 | 用户无法恢复对话继续运行。 | resume 应重建 active branch 的 `AgentMessage` 链，并进入 interactive 或 non-interactive flow。 |
| C5 | `crates/opi-agent/src/compaction.rs` / `crates/opi-coding-agent/src/harness.rs` | compaction 只有引擎，没有 runtime trigger、session event、session entry 或 prompt 注入。 | 长对话不会在 overflow 前压缩，compaction summary 不影响上下文。 | 增加 `CompactionCoordinator` 或 harness 集成层，发射 `CompactionStart/End`，写入 JSONL 并注入 prompt。 |
| C6 | `crates/opi-agent/src/lib.rs` / `crates/opi-coding-agent/src/harness.rs` | `[thinking]` config 未传入 `Request.thinking`。 | 用户配置 `budget_tokens` 无效，2.9 只在 provider fixture 层成立。 | 在 `AgentLoopConfig` 增加 `ThinkingConfig`，由 harness 从 `OpiConfig` 映射。 |

### High

| 编号 | 位置 | 问题 | 影响 | 建议 |
| --- | --- | --- | --- | --- |
| H1 | `crates/opi-ai/tests/provider_lifecycle.rs` | provider lifecycle wiremock 主要覆盖 Anthropic，缺少 OpenAI chat/OpenRouter/Mistral。 | HTTP 错误映射、headers、cancel/backpressure 回归风险高。 | 为 OpenAI chat adapter 增加 lifecycle suite，并覆盖 OpenRouter profile headers。 |
| H2 | `crates/opi-ai/tests/openrouter_fixtures.rs` | OpenRouter routing diagnostics 和 `HTTP-Referer`/`X-Title` headers 缺少发送断言。 | DoD 中 routing diagnostics tested 证据不足。 | 使用 wiremock 断言 outgoing headers 和 provider-specific error diagnostics。 |
| H3 | `crates/opi-ai/src/stream.rs` / `usage_cost.rs` | 只有 `Pricing` 和计算函数，无 model pricing table。 | 2.10 DoD “from model pricing table” 未满足，TUI cost 无数据源。 | 在 model metadata 或 registry 中增加 pricing lookup API。 |
| H4 | `crates/opi-coding-agent/src/interactive.rs` / `crates/opi-tui/src/status_bar.rs` | usage/cost 未接入 StatusBar 或 interactive state。 | 用户看不到 token/cost summary。 | 在 `MessageEnd`/`Done` 中累积 usage，并传给 TUI state。 |
| H5 | `crates/opi-tui/src/diff_view.rs` / `crates/opi-coding-agent/src` | DiffView 未接入 edit/patch 工具可视化路径。 | 2.11 仅组件可用，用户路径不可达。 | 将 edit 前后内容或 patch result 映射为 `DiffView`，补 snapshot/integration test。 |
| H6 | `crates/opi-coding-agent/tests/json_mode.rs` | `--json` 缺少 AutoRetry NDJSON framing 测试。 | JSON 消费者可能无法可靠处理 retry events。 | 用 MockProvider 注入 rate limit -> success，断言 top-level `AutoRetryStart/End`。 |
| H7 | `crates/opi-coding-agent/tests/json_mode.rs` | JSON mode 无 subprocess E2E。 | CLI wiring 与 runner 单元测试可能脱节。 | 增加 `opi --json "prompt"` 子进程测试，验证 stdout/stderr/exit code。 |
| H8 | `crates/opi-agent/src/message.rs` / `crates/opi-coding-agent/src/harness.rs` | `CompactionSummary` 在 LLM 转换路径中被过滤。 | 即使 future runtime 写入 compaction，也不会进入后续 provider context。 | 将 compaction summary 转换为 system/user context message，补测试。 |

### Medium

| 编号 | 位置 | 问题 | 影响 | 建议 |
| --- | --- | --- | --- | --- |
| M1 | `crates/opi-ai/src/retry.rs` | rate-limit header 解析只覆盖 `Retry-After` 秒值和 `x-ratelimit-reset`。 | 部分 provider 的 HTTP-date 或其他 `x-ratelimit-*` 信息会丢失。 | 扩展 header parser 并补 provider-specific tests。 |
| M2 | `crates/opi-ai/src/mistral.rs` | DoD 写 Mistral-specific headers/auth，但实现为标准 Bearer auth。 | DoD 与实现描述不一致。 | 确认 Mistral 是否需要特殊 header；若不需要，更新 ledger notes。 |
| M3 | `crates/opi-agent/src/session.rs` | 有 `LeafEntry`，但缺少生产级 active branch/leaf 解析 API。 | resume 很难正确重建当前会话分支。 | 暴露 `resolve_active_branch` 或 session tree helper。 |
| M4 | `crates/opi-agent/src/session.rs` / `crates/opi-coding-agent/src/session_cli.rs` | corrupt middle entries 在 resume/list 中可能被静默跳过。 | 用户不知道历史丢失或部分损坏。 | resume 使用 recovery metadata 并向 stderr 报告。 |
| M5 | `crates/opi-coding-agent/src/config.rs` | 无 `[compaction]` config 表。 | threshold/reserve/keep_recent 等无法配置。 | 增加 config schema 并传入 compaction coordinator。 |
| M6 | `crates/opi-agent/src/event.rs` / `session_event.rs` | queue/retry 等事件在 AgentEvent 与 AgentSessionEvent 之间分层不完全一致。 | JSON wire format 可能与 spec 伪代码不同。 | 文档化当前 wire format，或在 runner 统一提升 session-level events。 |
| M7 | `crates/opi-tui/src/keybindings.rs` / `docs/opi-spec.md` | 默认 abort/new_line 与 spec 示例不同。 | 用户预期可能不一致。 | 对齐默认值或更新 spec 示例。 |
| M8 | `crates/opi-ai/tests/*` | 多个 provider fixture 只测 `stream_from_sse()`，未覆盖 `Provider::stream()`。 | fixture-ready 与 runtime-ready 混淆。 | 每个 provider 至少加一个 wiremock `stream()` 冒烟测试。 |

### Low

| 编号 | 位置 | 问题 | 影响 | 建议 |
| --- | --- | --- | --- | --- |
| L1 | `.opi-impl-state.json` | 多个任务记录短 hash，少数任务 `last_attempt` 为 `null`。 | 审计可追溯性不一致。 | 未来 ledger 统一 40 字符 SHA 和字段完整性。 |
| L2 | `.opi-impl-state.json` | 2.9 `behavioral_tests` 指向不存在的 `thinking_fixtures.rs`。 | 证据路径误导。 | 改为 `crates/opi-ai/tests/anthropic_fixtures.rs`。 |
| L3 | `.opi-impl-state.json` | 多个测试计数与当前源码函数数量不完全一致。 | 不影响功能，但降低 ledger 精度。 | reinit 或 audit pass 时刷新测试计数。 |
| L4 | `crates/opi-ai/src/stream.rs` | legacy `StreamEvent` 仍公开。 | API 面可能混淆。 | 标记 deprecated 或移除公开导出。 |
| L5 | `crates/opi-tui/tests/diff_view_snapshots.rs` | 120x40 快照覆盖少于 80x24。 | resize 回归保护有限。 | 对 multi-hunk、add/remove-only 增补 120x40 snapshot。 |
| L6 | `crates/opi-tui/src/theme.rs` | 只支持内置主题，不读取用户 theme directory。 | 自定义主题不可用。 | 若 Phase 3 需要主题扩展，再实现 theme file loading。 |

## Ledger 与源码不一致

| 任务 | Ledger 记录 | 审计核对 |
| --- | --- | --- |
| 2.1 | `openai_chat_fixtures.rs` 33 tests | 当前测试函数数量更多；ledger 计数偏旧。 |
| 2.2 | 12 tests，routing diagnostics tested | 当前测试数量更多，但 headers/diagnostics 的 runtime 断言不足。 |
| 2.3 | fixtures cover text/tool/usage/error | fixture 覆盖成立，但 `Provider::stream()` 未实现。 |
| 2.4 | fixtures cover text/tool/usage/error | fixture 覆盖成立，但 `Provider::stream()` 未实现。 |
| 2.5 | Mistral-specific headers/auth | 实现采用标准 Bearer auth，无额外 Mistral-specific headers。 |
| 2.6 | 21 session storage tests | 当前测试数量略有偏差；存储层通过，但无 runtime 写入。 |
| 2.8 | tier=workspace，涉及 `opi-agent` / `opi-coding-agent` | 实际主要是 `opi-agent` 引擎与测试，`opi-coding-agent` 运行时未接入。 |
| 2.9 | `thinking_fixtures.rs` | 该文件不存在，实际 thinking 测试在 `anthropic_fixtures.rs`。 |
| 2.10 | model pricing table | 未发现内置 model pricing table，仅有 `Pricing` struct 与计算函数。 |
| 2.11 | edit/patch visualization | `DiffView` widget 与 snapshot 存在，但 edit/patch runtime 不可达。 |
| 2.13 | `last_attempt: null` | passing 任务字段格式与其他任务不一致。 |
| 2.14 | NDJSON smoke | runner 层测试存在，缺 subprocess E2E。 |
| 2.15 | `x-ratelimit-*` | 只实现部分 header 解析。 |
| Phase exit | 537 tests green | 机械门通过可作为任务级证据，但不足以证明所有 runtime paths 已闭环。 |

## 覆盖良好的方面

- OpenAI-compatible chat adapter 的协议映射、compat config、request body、tool call delta、usage、error fixture 覆盖较全面。
- OpenRouter 与 Mistral 复用 OpenAI chat adapter，减少重复协议实现，结构上合理。
- session JSONL 存储层具备 header、message、compaction、leaf、append、recovery 与 serde 往返测试。
- `session_contract.rs` 增加 property-based tests，对 JSONL round-trip、tree roots、header schema、compaction reference 等不变量形成了较好保护。
- `--json` runner 层已有 schema header、framing、agent event 包装、tool call、provider error 测试。
- agent retry loop 已覆盖 retry success、auth no-retry、exhausted、timeout success、disabled retry、AutoRetryStart 字段。
- `DiffView` 的 LCS diff 与 snapshot 基础扎实，可作为后续 edit/patch UI 的组件基线。
- Theme 迁移范围较完整，主要 TUI widgets 已支持 semantic theme colors。
- Keybindings 从 TOML 到 interactive handler 已基本接线，配置失败时有 fallback。
- Phase 2 的测试数量、fixture 数量和 property-based coverage 明显高于 Phase 1，说明实现过程重视验证。

## Phase exit 建议

### 建议结论

建议将 Phase 2 当前状态表述为：

- 任务级状态：`16/16 passing`，机械门与任务级测试证据成立。
- 审计状态：**有条件通过**。
- 发布/里程碑状态：不建议称为 “Phase 2 runtime complete”，除非补齐 provider HTTP、session runtime、resume、compaction 和 thinking config 贯通。

如果只以“库层与 fixture 层完成”为阶段目标，当前状态可接受；如果以 `docs/opi-spec.md` 中 Phase 2 exit criteria 衡量，仍应保留 Critical blockers。

### 优先修复顺序

1. 实现 `OpenAiResponsesProvider::stream()` 和 `GeminiProvider::stream()` 的真实 HTTP streaming，并补 wiremock lifecycle tests。
2. 在 `CodingHarness` 接入 session create/append，使 interactive 和 non-interactive 都能产生 JSONL session。
3. 将 `--resume` 改为重建 active branch context 并继续运行 agent。
4. 将 compaction engine 接入 harness runtime，发射 `CompactionStart/End`、写入 compaction entry，并将 summary 注入 prompt。
5. 将 `[thinking]` config 映射进 `AgentLoopConfig` 和 `Request.thinking`，补端到端 request body 测试。
6. 增加 usage/cost 的 runtime accumulation 与 StatusBar 展示，补 pricing table 或明确 cost 仍为后续范围。
7. 将 DiffView 接入 edit/patch 用户路径。
8. 补 `--json` subprocess E2E 与 AutoRetry NDJSON framing 测试。
9. 刷新 `.opi-impl-state.json` 中证据路径、测试计数、hash 格式和 DoD 备注。

### 建议验证命令

修复上述阻塞项后，建议至少运行：

```sh
cargo fmt --check --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
```

并新增或强化以下目标测试：

- `cargo test -p opi-ai --test provider_lifecycle`
- `cargo test -p opi-ai --test openai_responses_fixtures`
- `cargo test -p opi-ai --test gemini_fixtures`
- `cargo test -p opi-coding-agent --test session_cli`
- `cargo test -p opi-coding-agent --test json_mode`
- `cargo test -p opi-coding-agent --test interactive_mock`
- `cargo test -p opi-agent --test compaction`
- `cargo test -p opi-agent --test session_contract`

## 最终结论

Phase 2 已经完成了大量必要基础设施，并显著扩展了项目能力边界：多 provider adapter、session 存储格式、compaction 原语、JSON event mode、retry/backoff、TUI 主题和 keybindings 都已具备可测试基础。

但当前最大的风险不是缺少模块，而是若干模块尚未进入真实运行路径。provider、session、compaction、thinking、usage/cost、DiffView 都存在不同程度的 “fixture/component ready, runtime incomplete” 状态。

建议短期内不要再扩展 Phase 3 功能面，而是先做一次 Phase 2 hardening pass，将上述 Critical/High 项补齐后再更新阶段快照。届时 Phase 2 才能更稳妥地宣称达到 “multi-provider and persistence” 里程碑。
