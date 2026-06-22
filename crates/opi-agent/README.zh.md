# opi-agent

[![Crates.io](https://img.shields.io/crates/v/opi-agent.svg)](https://crates.io/crates/opi-agent)
[![Docs.rs](https://docs.rs/opi-agent/badge.svg)](https://docs.rs/opi-agent)

> [opi](https://github.com/OdradekAI/opi) 使用的 Provider 无关 Agent 运行时。

[English](README.md) | [opi workspace](../../README.zh.md)

## 当前状态

当前 crate 版本是 `0.5.3`，继承自 workspace 包版本。

`opi-agent` 负责 Agent 主循环和运行时基础能力：工具契约、JSON Schema 参数校验、
并行/串行工具执行、生命周期 hooks、事件输出、steering/follow-up 队列、会话
JSONL 存储、分支重建、上下文压缩、SDK/RPC 类型、扩展、本地诊断、已脱敏 trace
envelope，以及 streaming proxy。

它依赖 `opi-ai` 的 Provider 和消息类型。它不实现 `opi` CLI、终端 UI 或具体的
文件/ shell 内置工具；这些能力分别位于 `opi-coding-agent` 和 `opi-tui`。

## 核心抽象

| 项 | 作用 |
|----|------|
| `Agent` | 对主循环的有状态封装，提供 prompt、continue、abort、subscribe、steering、follow-up、模型切换和工具注册辅助。 |
| `Tool` | 基于 JSON Schema 的工具契约，支持取消和可选进度更新。 |
| `ExecutionMode` | 控制工具能否进入并行批次，或是否强制串行执行。 |
| `AgentHooks` | 覆盖上下文转换、LLM 转换、工具策略/结果、停止判断和下一轮准备的生命周期 hooks。 |
| `AgentEvent` | 运行时事件流，覆盖生命周期、流式文本、工具调用、队列、重试、压缩和结束状态。 |
| `AgentSessionEvent` | `opi --json` 使用的会话级事件协议。 |
| `AgentLoopConfig` | 主循环限制、重试配置、压缩配置和相关运行时设置。 |

## 主循环形状

主循环按固定的运行时事件顺序执行。`AgentStart` 在首轮之前仅触发一次，
`AgentEnd` 在每条终止路径上仅触发一次（正常停止、Hook 停止、terminate 标志、
取消或错误）。每个 turn（`0..max_turns`）内：

```text
agent_start                              # 仅一次，首轮之前
  对每个 turn：
    cancel check                          # 被取消 -> AgentEnd, AgentError::Cancelled
    turn_start
    transform_context                     # AgentHooks::transform_context
    convert_to_llm                        # AgentHooks::convert_to_llm
    validate request capabilities         # 失败 -> AgentEnd, AgentError::Provider
    provider.stream(Request)
      message_start                       # assistant 流 Start
      message_update                      # 每个文本/思考 delta
      message_end                         # 完整的 assistant 消息
      若存在 tool call：
        validate tool args (jsonschema)
        tool_execution_start              # 每个 tool call
        before_tool_call                  # AgentHooks::before_tool_call（可阻止）
        tool.execute                      # 并行批次；若任一工具为 Sequential 则整批串行
        after_tool_call                   # AgentHooks::after_tool_call（可替换结果）
        tool_execution_end                # 每个 tool call
        turn_end                          # assistant 消息 + tool_results
        若所有结果都 terminate -> AgentEnd, return Ok
        should_stop_after_turn            # true -> AgentEnd, return Ok（压缩停止）
      否则：
        turn_end                          # assistant 消息，无 tool_results
        should_stop_after_turn            # true -> AgentEnd, return Ok
    prepare_next_turn                     # AgentHooks::prepare_next_turn；在终止的
                                         # should_stop_after_turn 之后被跳过；可注入消息
    drain steering queue                  # 非空 -> QueueUpdate，追加，进入下一 turn
    若无待处理工具：
      pop follow-up queue                 # 非空 -> QueueUpdate，追加，进入下一 turn
      否则 -> 停止
agent_end                                 # 仅一次，终止时
```

边界：

- `should_stop_after_turn` 在 `turn_end` 之后、`prepare_next_turn` 及任何队列
  轮询之前执行。压缩协调器在此返回 `true` 以在下一 turn 之前停止；终止停止
  之后不会运行 `prepare_next_turn`，也不会轮询 steering/follow-up。
- `prepare_next_turn` 仅在 `should_stop_after_turn` 允许继续时执行，且早于
  steering/follow-up 轮询；注入的消息会进入下一次 provider 请求。
- Steering 先于 follow-up 被排空。仅当无待处理工具且 steering 队列为空时，
  才弹出 follow-up。
- `CompactionEngine` 只是上下文大小的原语；将压缩与持久化 CLI 会话相连的
  高层协调器位于 `opi-coding-agent`，并通过 `should_stop_after_turn` 停止主循环。

Rate limit 和 timeout 等可重试 Provider 错误可通过 `AgentLoopConfig.retry` 处理。
重试开始/结束会通过 `AgentEvent` 暴露。

## 会话与压缩

会话存储使用 append-only JSONL：

- 第一行：`SessionHeader`。
- 条目：`MessageEntry`、`CompactionEntry` 和 `LeafEntry`。
- Reader 恢复时会跳过损坏条目和末尾截断行。
- `session_branch::SessionTree` 根据 `parent_id` 链接和最新 `LeafEntry` 重建活跃分支。

压缩基础能力包括 threshold/manual/overflow 原因、
`CompactionEngine::should_compact`、`CompactionEngine::compact`，以及用于自定义摘要
生成的 `CompactionHooks`。`opi-coding-agent` 负责把这些基础能力连接到 CLI 会话
持久化。

## SDK、扩展、诊断与 Proxy

- `sdk` 定义 RPC JSONL 模式和嵌入方共享的带 schema version 的命令/响应类型。
  `SDK_SCHEMA_VERSION` 是 `3`。
- `extension` 提供 `Extension` 和 `ExtensionRegistry`，支持生命周期 hooks、自定义
  工具、自定义命令、事件观察器、扩展状态、自定义 Provider 和模型覆盖。
- `diagnostic` 和 `diagnostic_sink` 提供类型化诊断，以及面向公共 JSON/text 边界的
  脱敏辅助。
- `trace` 在调用方显式启用时保存最新运行的本地、已脱敏 trace envelope。
- `streaming_proxy` 可在任意 `BufRead`/`Write` 传输上转发 JSONL 命令/事件，输出
  `proxy_ready` header，提供事件缓冲、取消，并默认脱敏常见密钥模式。

所有 SDK/RPC/proxy 表面都是不稳定的 0.x API。客户端应检查 schema version，并在
需要时固定精确 crate 版本。

## 公共模块

`agent`、`compaction`、`diagnostic`、`diagnostic_sink`、`event`、`extension`、
`hooks`、`loop_types`、`message`、`sdk`、`session`、`session_branch`、
`session_event`、`state`、`streaming_proxy`、`tool`、`trace` 和 `validation`。

crate root 重新导出了常用运行时类型，包括 `Agent`、`Tool`、`ToolResult`、
`ToolError`、`ExecutionMode`、`AgentHooks`、`AgentEvent`、`AgentSessionEvent`、
`AgentLoopConfig`、`SdkCommand`、`SdkResponse` 和 `ToolDef`。

## 测试支持

确定性主循环测试可使用 `opi_ai::test_support::MockProvider` 搭配自定义 `Tool`
实现。涉及会话存储的测试应使用隔离临时目录。

## 许可证

MIT。详见 workspace [LICENSE](../../LICENSE)。
