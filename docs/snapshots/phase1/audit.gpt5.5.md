# Phase 1 系统审计报告

审计日期：2026-05-21  
审计模型：GPT-5.5  
审计范围：Phase 1 全部任务，重点覆盖 `opi-ai`、`opi-agent`、`opi-coding-agent`、`opi-tui`

## 执行摘要

Phase 1 的任务级状态显示为全部 `passing`：`.opi-impl-state.json` 中 18 个 Phase 1 任务均有测试证据、通过记录和 DoD 说明。但阶段退出状态仍未完成：`phase_exit.1.exit_criteria_met` 为 `false`，`completed_at` 为 `null`，并且阶段退出 evaluator 尚未运行。

本次审计结论为：**有条件通过 / 不可 Phase exit**。

主要原因是当前实现已经具备较完整的库层骨架和测试夹具，但距离 `docs/opi-spec.md` 中 Phase 1 MVP 的真实运行语义仍有明显缺口：

- `opi-ai` 已完成 message/stream 类型、Provider trait、Anthropic SSE fixture 映射和 registry，但真实 `AnthropicProvider::stream` HTTP 路径仍未就绪，属于 **fixture-ready, live-not-ready**。
- `opi-agent` 已完成 Tool trait、基础 `agent_loop`、Agent wrapper、hooks/queues 骨架，但存在 assistant 文本丢失、工具批处理、`terminate`、`transform_context` 等关键语义偏差。
- `opi-coding-agent` 与 `opi-tui` 的工具、配置、非交互 runner、TUI 组件快照和 mock harness 测试较完整，但 `opi` 二进制的交互式 CLI 尚未接入 TUI，`--config`、`--system`、认证退出码等 CLI 行为仍有缺口。

因此，任务级 `passing` 可以作为“当前单元/集成测试通过”的证据，但不能等同于 Phase 1 退出通过。建议保持 `.opi-impl-state.json` 中 Phase exit 未通过状态，直到 Critical/High 项修复并运行阶段退出 evaluator。

## 审计依据

### 状态文件

- `.opi-impl-state.json`
  - `current_phase`: `1`
  - Phase 1 任务数量：18
  - 任务状态：18/18 `passing`
  - `phase_exit.1.completed_at`: `null`
  - `phase_exit.1.exit_criteria_met`: `false`
  - `phase_exit.1.evaluator_summary`: `All 18 tasks passing. Phase-exit evaluator not run (requires explicit invocation).`

### 规范与源码范围

- `docs/opi-spec.md`
- `Cargo.toml`
- `crates/opi-ai`
- `crates/opi-agent`
- `crates/opi-coding-agent`
- `crates/opi-tui`

### 核对的测试证据

状态文件记录的主要验证证据包括：

- `cargo fmt --check --all`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`
- `cargo test --workspace --all-targets`
- 各任务专属测试：
  - `crates/opi-ai/tests/stream_events.rs`
  - `crates/opi-ai/tests/provider_trait.rs`
  - `crates/opi-ai/tests/anthropic_fixtures.rs`
  - `crates/opi-ai/tests/registry.rs`
  - `crates/opi-agent/tests/tool_validation.rs`
  - `crates/opi-agent/tests/agent_loop_mock.rs`
  - `crates/opi-agent/tests/agent_wrapper.rs`
  - `crates/opi-agent/tests/hooks_queues.rs`
  - `crates/opi-coding-agent/tests/tools_read_write_edit_bash.rs`
  - `crates/opi-coding-agent/tests/tools_glob_grep.rs`
  - `crates/opi-coding-agent/tests/system_prompt.rs`
  - `crates/opi-coding-agent/tests/interactive_mock.rs`
  - `crates/opi-coding-agent/tests/non_interactive.rs`
  - `crates/opi-coding-agent/tests/non_interactive_policy.rs`
  - `crates/opi-coding-agent/tests/config_loading.rs`
  - `crates/opi-coding-agent/tests/config_precedence.rs`
  - `crates/opi-coding-agent/tests/mock_e2e.rs`
  - `crates/opi-tui/tests/tui_snapshots.rs`
  - `crates/opi-tui/tests/markdown_snapshots.rs`

本次审计主要基于源码和既有证据做系统性只读审查，未重新运行完整测试套件。

## Phase 1 状态核对

| 任务范围 | 状态文件结论 | 审计结论 |
| --- | --- | --- |
| 1.0 依赖引入 | `passing` | 基本通过。依赖约束记录清晰，但部分依赖尚未实际接入生产路径。 |
| 1.1-1.4 `opi-ai` | `passing` | 条件通过。协议层和 fixture 通过，真实 Anthropic HTTP provider 未就绪。 |
| 1.5-1.8 `opi-agent` | `passing` | 条件通过。核心骨架完成，但 agent loop 语义存在 Critical/High 偏差。 |
| 1.9-1.13 工具、prompt、TUI 组件 | `passing` | 库层基本通过。工具和快照覆盖较好，但 TUI 尚未接入 CLI。 |
| 1.14 interactive CLI wiring | `passing` | 应降级为 partial。`CodingHarness` 库层可用，但二进制交互路径仍为 stub。 |
| 1.15 non-interactive mode | `passing` | 条件通过。runner 与 policy 通过，但 stdin、auth exit code、进程级 E2E 仍缺。 |
| 1.16 config loading | `passing` | 条件通过。解析与 precedence 覆盖较好，但 `--config` path 未生效。 |
| 1.17 integration harness | `passing` | 基本通过。MockProvider 与跨 crate E2E 基础设施可用。 |
| Phase exit | `exit_criteria_met: false` | 审计同意维持未通过。 |

## 分域审计

### `opi-ai`

`opi-ai` 已完成 Phase 1 需要的公开类型和测试夹具主体：

- `crates/opi-ai/src/message.rs` 定义 provider-facing message、tool def、tool call/result 类型。
- `crates/opi-ai/src/stream.rs` 定义 `AssistantStreamEvent`、`StopReason`、`Usage` 等流协议类型。
- `crates/opi-ai/src/provider.rs` 已将 placeholder `complete` 替换为 `Provider::stream(Request) -> EventStream`。
- `crates/opi-ai/src/anthropic.rs` 已实现 Anthropic SSE 事件解析和 mapper，并通过文本、tool call、usage、error、mixed fixtures 测试。
- `crates/opi-ai/src/registry.rs` 已覆盖 `anthropic:model` resolution、capabilities、未知 provider/model、非法 spec 和重复注册。

风险集中在生产 provider 路径：

- `AnthropicProvider::stream` 当前并未真正发起 `reqwest` HTTP SSE 请求，而是基于空 SSE 字符串构造 stream。真实 CLI 配置 Anthropic provider 时可能得到空流，没有 `Start`/`Done`/`Error` 终端事件。
- `serialize_messages` 中 assistant tool call 的 `input` 可能以 JSON 字符串而非对象写入 Anthropic 请求体，多轮 tool 历史下存在真实 API 请求失败风险。
- SSE parse 错误目前容易被静默丢弃，畸形流可能导致无终端事件或截断流。
- `Request::cancel` 在 Anthropic stream 路径尚未真正接入 HTTP cancellation/drop cancellation。
- 缺少跨 provider 的生命周期契约测试：`Start -> deltas -> exactly one Done/Error`。

审计结论：`opi-ai` 的协议层与 fixture 层完成度较高，但不应宣称真实 Anthropic provider 已可用于 Phase 1 MVP。建议标注为 **fixture-ready, live-not-ready**。

### `opi-agent`

`opi-agent` 已完成 Agent 核心 API 的骨架：

- `crates/opi-agent/src/tool.rs` 定义 `Tool`、`ToolResult`、`ToolError`、`ExecutionMode`。
- `crates/opi-agent/src/validation.rs` 使用 `jsonschema` 验证工具参数。
- `crates/opi-agent/src/lib.rs` 实现 `agent_loop`、工具执行、hooks、steering/follow-up queue 轮询。
- `crates/opi-agent/src/agent.rs` 提供 `Agent::prompt`、`continue_`、`abort`、`subscribe` 等 wrapper。
- `crates/opi-agent/tests` 覆盖 tool validation、mock loop、Agent wrapper、hooks/queues。

但与 `docs/opi-spec.md` 中继承自 pi 的运行语义相比，存在关键偏差：

- 纯文本 assistant turn 的 content 可能被清空。`TextDelta` 只触发 `MessageUpdate`，未累积到 `assistant_content`；终端 `Done.message.content` 随后又被 `assistant_content.clone()` 覆盖。
- 工具批处理语义未实现。规范要求默认并行，任一 sequential 工具使整批顺序执行；当前始终按 `for tc in &tool_calls` 串行执行，并忽略 `Tool::execution_mode()`。
- `ToolResult.terminate` 字段存在但未参与 loop 决策。规范要求只有 batch 内所有 finalized results 都 `terminate` 才 early stop。
- `transform_context`/`prepare_next_turn` 等 hook 面存在或被规范提及，但主循环尚未接线。
- `should_stop_after_turn` 的 `tool_results` 当前从所有历史 messages 过滤，而非仅当前 turn 的 tool results。
- Provider stream 若无 `Done`/`Error` 就结束，当前不会产生完整 `TurnEnd`，事件顺序契约不够强。
- `state.rs`、`transport.rs` 仍保留公开 placeholder API，与 Phase 1 后的稳定 API 边界不完全一致。

审计结论：`opi-agent` 是 **implementation scaffold complete; semantic parity incomplete**。修复 assistant 文本丢失和工具批处理/terminate/transform_context 前，不建议将 1.6-1.8 视为 audit-clean。

### `opi-coding-agent`

`opi-coding-agent` 的库层实现较充分：

- `crates/opi-coding-agent/src/tool` 中 read/write/edit/bash/glob/grep 六个内置工具已实现。
- `crates/opi-coding-agent/src/prompt.rs` 实现 system prompt 三层构造：base、tools、user system。
- `crates/opi-coding-agent/src/config.rs` 实现 TOML config、defaults、env/project/user/CLI precedence 合并。
- `crates/opi-coding-agent/src/harness.rs` 负责组装 provider、tools、prompt、hooks、Agent。
- `crates/opi-coding-agent/src/runner.rs` 实现 non-interactive runner、stdout/stderr、exit code 和 mutating policy。
- `crates/opi-coding-agent/tests` 覆盖工具行为、schema、config、interactive mock、non-interactive、mock E2E。

主要风险集中在二进制运行时和 CLI 行为：

- `crates/opi-coding-agent/src/main.rs` 的交互式路径仍输出 `"(interactive mode not yet wired to TUI)"`，未启动 TUI，也未运行完整 interactive harness。
- `opi-tui` 虽作为依赖存在，但 `opi-coding-agent` 未实际使用 `opi_tui`。
- `--config` 参数传入 `ConfigSource`，但 `resolve_config` 未读取该 path，导致 CLI 指定配置文件不生效。
- `--system` 参数存在，但未读取系统 prompt 文件并传入 `SystemPromptBuilder::user_system()`。
- 认证失败当前更接近 config error，和规范中 auth failure exit code 3 的要求不一致。
- 没有进程级 CLI E2E 测试覆盖 `--help`、`--version`、mock non-interactive run、exit code matrix。
- interactive 模式对 write/edit/bash 缺少确认或 before hook policy；non-interactive 有 deny policy，但 interactive 当前没有对应安全边界。
- `tool_timeout_ms`、`thinking` 等配置存在但运行时未完整接线。
- Windows 下 `BashTool` 使用 `sh` 存在可移植性风险。

审计结论：1.9-1.13、1.15-1.17 的库层和测试层证据较强；1.14 interactive CLI wiring 应视为 **partial / blocked-on-TUI-wiring**。

### `opi-tui`

`opi-tui` 已完成 Phase 1 组件和快照测试：

- `MessageList`
- `InputEditor`
- `StatusBar`
- `ToolCallView`
- `Shell`
- `MarkdownView`
- `CodeBlock`

快照覆盖 80x24 和 120x40 尺寸，渲染输出稳定，适合作为 TUI 组件基础。

主要缺口是集成层：

- 没有 event loop。
- 没有 `AgentEvent -> TUI state` 桥接。
- `MarkdownView`/`CodeBlock` 未接入 `MessageList` 或 `Shell` 的真实对话渲染路径。
- crate 文档提到 differential rendering，但当前实现仍是静态组件渲染。

审计结论：TUI 组件库可算通过，但 Phase 1 “interactive CLI displays results in TUI” 尚未满足。

## 严重问题清单

### Critical

| 编号 | 位置 | 问题 | 影响 | 建议 |
| --- | --- | --- | --- | --- |
| C1 | `crates/opi-ai/src/anthropic.rs` | `AnthropicProvider::stream` 未实现真实 HTTP SSE，当前生产路径可能返回空流。 | 真实 Anthropic CLI 无 assistant 输出，违反 provider lifecycle。 | 实现 `reqwest` streaming POST、SSE decode、HTTP/error mapping、cancel/drop cancellation，并补 `Provider::stream` E2E。 |
| C2 | `crates/opi-agent/src/lib.rs` | 纯文本 assistant content 可能在 turn 结束时被清空。 | Agent prompt/non-interactive/TUI 可能丢失模型文本。 | 在 `TextDelta`/`TextEnd` 累积文本，或不要用空 `assistant_content` 覆盖 `Done.message.content`；补 assistant content 断言测试。 |
| C3 | `crates/opi-coding-agent/src/main.rs` | 交互式 CLI 仍是 stub，未接入 TUI。 | Phase 1 exit criteria 中 interactive TUI 不满足。 | 接线 `main -> TUI event loop -> CodingHarness -> Agent events -> opi-tui`，补进程级/mock interactive E2E。 |

### High

| 编号 | 位置 | 问题 | 影响 | 建议 |
| --- | --- | --- | --- | --- |
| H1 | `crates/opi-ai/src/anthropic.rs` | `serialize_messages` 的 tool call input 可能序列化为字符串。 | 含 tool 历史的真实 Anthropic 请求可能失败。 | 将 `tool_call.arguments` 解析为 JSON object 写入 `input`，并补 request body 测试。 |
| H2 | `crates/opi-ai/src/anthropic.rs` | 缺少 provider lifecycle 契约测试，畸形 SSE 可静默丢弃。 | 流协议可能无终端事件或错误不可见。 | 增加 `Start -> deltas -> exactly one Done/Error` 契约测试；parse 错误映射为 `ProviderError` 或终端 `Error`。 |
| H3 | `crates/opi-agent/src/lib.rs` | 工具 batch execution mode 未实现，始终串行。 | 与 pi 语义和规范不一致，影响并发工具吞吐和顺序保证。 | 按 batch 中工具 `ExecutionMode` 决定并行/串行执行，补多工具测试。 |
| H4 | `crates/opi-agent/src/lib.rs` | `ToolResult.terminate` 未参与 loop 决策。 | 早停语义缺失。 | 实现“全部 finalized results terminate 才 early stop”，补 batch terminate 测试。 |
| H5 | `crates/opi-agent/src/lib.rs` | `transform_context`/`prepare_next_turn` 未接线。 | 上下文转换与 turn 更新 hook 语义缺失。 | 在 provider request 前接入 context transform，在 queue/下一轮前接入 next turn hook。 |
| H6 | `crates/opi-coding-agent/src/config.rs` | `--config` path 未生效。 | 用户指定配置文件不会影响运行。 | 让 `ConfigSource.config_path` 参与读取并拥有 CLI precedence。 |
| H7 | `crates/opi-coding-agent/src/main.rs` / `src/harness.rs` | `--system` 未读取和注入 prompt。 | 用户系统层配置无效。 | 读取系统 prompt 文件并传入 `SystemPromptBuilder::user_system()`。 |
| H8 | `crates/opi-coding-agent/src/harness.rs` | interactive 模式缺少 mutating 工具确认/拦截。 | write/edit/bash 可被模型直接执行，安全边界不足。 | 在 interactive hooks 中实现确认策略或至少 deny/allow policy，并补测试。 |
| H9 | `crates/opi-coding-agent/src/main.rs` | 认证失败 exit code 与规范不一致。 | 自动化调用无法可靠区分 config/auth/provider failure。 | 将缺 API key 等认证失败映射到 exit code 3。 |

### Medium

| 编号 | 位置 | 问题 | 影响 | 建议 |
| --- | --- | --- | --- | --- |
| M1 | `crates/opi-agent/src/lib.rs` | `should_stop_after_turn` 使用历史全部 tool results。 | hook 看到的上下文可能大于当前 turn。 | 传递当前 turn tool results。 |
| M2 | `crates/opi-agent/src/lib.rs` | provider stream 无 terminal event 时缺少明确错误。 | 事件顺序不完整，agent 可能静默结束。 | 将无终端事件视为 provider protocol error。 |
| M3 | `crates/opi-agent/src/agent.rs` | `continue_` 未校验最后消息类型。 | 多轮语义可能偏离规范。 | 按规范约束最后消息为 user 或 tool result。 |
| M4 | `crates/opi-agent/src/loop_types.rs` | `MaxTurnsExceeded` 变体未使用。 | max turns 耗尽语义不清。 | 达到上限时返回或发出明确错误/事件。 |
| M5 | `crates/opi-coding-agent/src/tool/read.rs` | `inside_workspace` 硬编码为 true。 | 审计细节不可靠。 | 使用 canonicalize/starts_with 与 write/edit 一致计算。 |
| M6 | `crates/opi-coding-agent/src/tool/*` | 路径穿越策略无测试。 | workspace 边界策略不透明。 | 增加 `../` path 测试并记录允许/拒绝决策。 |
| M7 | `crates/opi-coding-agent/tests` | read/write/edit/bash 缺 schema fixture 测试。 | 工具 schema 与模型参数兼容性证据不足。 | 补齐四个工具的 schema fixture 测试。 |
| M8 | `crates/opi-coding-agent/src/main.rs` | non-interactive stdin prompt 未实现。 | 不满足“argv or stdin”行为。 | 无 positional prompt 时从 stdin 读取一次 prompt。 |
| M9 | `crates/opi-coding-agent/src/runner.rs` | exit code matrix 覆盖不完整。 | 自动化稳定性证据不足。 | 补 1/3/5/130 等路径测试。 |
| M10 | `crates/opi-tui/src/*` | TUI 无 event loop 和 AgentEvent bridge。 | 组件无法支撑真实交互模式。 | 增加 TUI runtime adapter，将 AgentEvent 映射到 view state。 |
| M11 | `crates/opi-coding-agent/src/tool/bash.rs` | Windows 使用 `sh` 风险。 | Tier 1 Windows 目标可能失败。 | Windows 使用 `cmd`/PowerShell 或显式文档化 shell requirement。 |

### Low

| 编号 | 位置 | 问题 | 影响 | 建议 |
| --- | --- | --- | --- | --- |
| L1 | `crates/opi-ai/src/stream.rs` | legacy `StreamEvent` 仍在生产模块中。 | API 清洁度不足。 | 删除或标记 deprecated/hidden。 |
| L2 | crate README | 多个 README 仍描述为 stubs。 | 文档误导。 | 更新 README 状态。 |
| L3 | `crates/opi-agent/src/state.rs` / `transport.rs` | placeholder API 仍公开。 | 稳定 API 面可能过早暴露。 | Phase 4 前 hidden 或移除公开 re-export。 |
| L4 | `crates/opi-coding-agent/src/runner.rs` 等 | `Mutex::lock().unwrap()` 出现在生产代码。 | poison 时 panic，与“无 unwrap”证据存在字面冲突。 | 改为错误处理或 poison recovery。 |
| L5 | `crates/opi-tui/src/lib.rs` | 文档称 differential rendering，但实现为静态 widget render。 | 文档与实现不一致。 | 调整文档或补真实 differential renderer。 |

## 覆盖良好的方面

- Workspace 依赖约束整体清晰，`reqwest` 使用 `default-features = false` 与 `rustls-tls`，`tokio` 未使用 `full`。
- `opi-ai` 的 message/stream 类型、Provider trait 和 registry 测试覆盖较好。
- Anthropic SSE mapper 的 fixture 覆盖文本、tool call、usage、error、mixed 响应。
- `opi-agent` 的 Tool trait、jsonschema validation、基础 hooks/queues 和 Agent wrapper 已形成可扩展骨架。
- `opi-coding-agent` 六工具的库层行为覆盖较扎实，尤其 read/write/edit/bash 的 temp-dir 行为与 glob/grep 的 gitignore/regex 错误处理。
- non-interactive mutating policy 设计明确：默认拒绝 write/edit/bash，允许 read/glob/grep，支持 opt-in。
- TOML config 的 missing、malformed、partial、unknown fields 和 precedence 测试较充分。
- `opi-tui` 快照测试稳定覆盖两种终端尺寸，组件拆分清楚。
- `MockProvider` 跨 crate 测试基础设施有助于后续补齐 CLI/TUI E2E。

## Phase exit 建议

### 必须完成后再 Phase exit

1. 实现真实 `AnthropicProvider::stream` HTTP SSE 路径，并补生命周期契约测试。
2. 修复 `agent_loop` assistant 文本丢失问题，并补纯文本 turn 内容断言。
3. 实现工具 batch execution mode、`ToolResult.terminate`、`transform_context` 等核心语义。
4. 将 `opi` 二进制交互模式接入 TUI 与 `CodingHarness`，移除 interactive stub。
5. 让 `--config`、`--system`、auth exit code、stdin prompt 等 CLI 行为符合规范。
6. 为 interactive write/edit/bash 增加确认或 before hook policy。
7. 运行 phase-exit evaluator，并将结果写回 `.opi-impl-state.json`。

### 建议补充的验证命令

修复上述阻塞项后，建议至少运行：

```sh
cargo fmt --check --all
cargo clippy --workspace --all-targets -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
cargo test --workspace --all-targets
cargo run -p opi-coding-agent -- --version
```

此外建议新增并运行：

- `cargo test -p opi-ai --test anthropic_provider_stream`
- `cargo test -p opi-agent --test agent_loop_semantics`
- `cargo test -p opi-coding-agent --test cli_e2e`
- `cargo test -p opi-coding-agent --test interactive_tui_mock`

测试文件名可按仓库实际命名调整，重点是覆盖真实 provider stream、agent loop 语义、二进制 CLI、interactive TUI wiring。

## 建议的任务状态调整

- 1.1、1.2、1.4：维持 `passing`，标注协议/registry 通过。
- 1.3：改为 `partial` 或在 notes 中标注 `fixture-ready, live-provider pending`。
- 1.5：维持 `passing`，补充 agent loop 集成验证 invalid args。
- 1.6：改为 `partial`，直到 assistant content、batch、terminate、transform_context 修复。
- 1.7：条件维持 `passing`，补 `continue_` 前置条件和 max turns 语义。
- 1.8：改为 `partial` 或补 before hook、prepare_next_turn、当前 turn tool_results 测试。
- 1.9、1.10：维持 `passing`，补路径策略和 schema fixtures。
- 1.11：维持 `passing`，但接入 `--system` 前仍属库层完成。
- 1.12、1.13：维持 `passing`，标注 TUI component-only。
- 1.14：改为 `partial / blocked-on-TUI-wiring`。
- 1.15：维持 `passing`，标注 binary stdin/auth exit gaps。
- 1.16：条件维持 `passing`，修复 `--config` path。
- 1.17：维持 `passing`，建议扩展到进程级 CLI E2E。

## 最终结论

Phase 1 当前已经完成大量基础设施和库层实现，足以作为后续迭代的稳定基线；但它尚未达到 Phase 1 MVP 的真实运行标准。最关键的差距不是测试数量，而是测试覆盖面与用户路径之间存在断层：Anthropic live provider、agent loop 关键语义、interactive CLI/TUI、CLI flag/exit 行为仍需补齐。

建议当前阶段保持：

- 任务级状态：可记录为“多数任务测试通过，若干任务需审计备注”。
- 阶段退出状态：继续保持 **未通过**。
- 发布判断：不宜以“Phase 1 MVP 完成”或 `0.2.0` 功能完成名义发布；可以作为内部 snapshot 或 pre-exit audit baseline。
