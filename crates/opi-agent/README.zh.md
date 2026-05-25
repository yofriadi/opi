# opi-agent

[![Crates.io](https://img.shields.io/crates/v/opi-agent.svg)](https://crates.io/crates/opi-agent)
[![Docs.rs](https://docs.rs/opi-agent/badge.svg)](https://docs.rs/opi-agent)

> [opi](https://github.com/OdradekAI/opi) 的通用 Agent 运行时：流式 turn、工具调用、hooks、事件、会话与上下文压缩。

[English](README.md) | [opi workspace](../../README.zh.md)

## 当前状态

当前 crate 版本：`0.3.0`。

`opi-agent` 提供 `opi` 二进制使用的 Provider 无关运行时。它负责 turn 主循环、工具参数 JSON Schema 校验、并行/串行工具执行、支持 retry 的 Provider streaming、事件订阅、steering/follow-up 队列、JSONL 会话存储，以及阈值/手动/溢出触发的上下文压缩基础能力。

`Transport` trait 已作为 stdio/SSE 工具传输抽象存在，但基于外部 transport 的工具服务器尚未接入主循环。

## 核心抽象

```rust
pub trait Tool: Send + Sync {
    fn definition(&self) -> ToolDef;
    fn execute(&self, call_id: &str, args: serde_json::Value,
               signal: CancellationToken,
               on_update: Option<UpdateCallback>) -> ...;
    fn execution_mode(&self) -> ExecutionMode { ExecutionMode::Parallel }
}

pub trait AgentHooks: Send + Sync {
    async fn transform_context(...) -> Result<Vec<AgentMessage>, AgentError>;
    fn convert_to_llm(...) -> Result<Vec<Message>, AgentError>;
    async fn before_tool_call(...) -> BeforeToolCallResult;
    async fn after_tool_call(...) -> AfterToolCallResult;
    async fn should_stop_after_turn(...) -> bool;
    async fn prepare_next_turn(...) -> Option<PrepareNextTurnUpdate>;
}
```

`Agent` 在主循环外提供 `prompt`、`continue_`、`abort`、`subscribe`、`add_tool` 与消息缓冲区辅助方法。

## Agent 主循环

```text
agent_loop
  -> transform_context
  -> convert_to_llm
  -> provider.stream(Request)
  -> 发出并累积 AssistantStreamEvent
  -> 检测工具调用
  -> 用 jsonschema 校验参数
  -> before_tool_call hook
  -> 执行工具
     -> 全部是 parallel 工具时一起执行
     -> 只要有 sequential 工具，整批串行执行
  -> after_tool_call hook
  -> 所有工具结果 terminate 时停止
  -> should_stop_after_turn hook
  -> prepare_next_turn hook
  -> drain steering 队列
  -> 无待执行工具时弹出一条 follow-up 消息
```

可重试的 Provider 错误（`RateLimited`、`Timeout`）可以通过 `AgentLoopConfig.retry` 自动重试。retry 开始/结束会通过 `AgentEvent` 发出。

## 会话与压缩

会话存储采用 append-only JSONL：

- 第一行：`SessionHeader`。
- 条目：`MessageEntry`、`CompactionEntry`、`LeafEntry`。
- Reader 支持崩溃恢复，会跳过损坏条目和末尾截断行。

上下文压缩能力包括：

- `CompactionConfig { enabled, threshold_tokens }`。
- `CompactionReason::{Manual, Threshold, Overflow}`。
- `CompactionEngine::should_compact`。
- `CompactionEngine::compact`。
- `CompactionHooks` 自定义摘要生成，缺省时使用 core fallback summary。

`opi-coding-agent` 负责把这些基础能力连接到运行时持久化。

## 事件

`AgentEvent` 覆盖 Agent 生命周期、turn 生命周期、消息流式更新、工具执行、队列、自动 retry、压缩、会话持久化错误和 Agent 结束。

`AgentSessionEvent` 是 JSON 输出使用的会话级 wire protocol。它包装 agent events，并增加 compaction、retry、thinking level、session info、session summary 等事件。

## 简短示例

```rust
use opi_agent::{ExecutionMode, Tool, ToolError, ToolResult};
use opi_ai::message::{OutputContent, ToolDef};

struct EchoTool;

impl Tool for EchoTool {
    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "echo".into(),
            description: "原样返回输入。".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": { "text": { "type": "string" } },
                "required": ["text"],
            }),
        }
    }

    fn execute(&self, _id: &str, args: serde_json::Value,
               _signal: tokio_util::sync::CancellationToken,
               _on_update: Option<opi_agent::tool::UpdateCallback>)
        -> std::pin::Pin<Box<dyn std::future::Future<
            Output = Result<ToolResult, ToolError>> + Send>>
    {
        let text = args.get("text").and_then(|v| v.as_str())
            .unwrap_or("").to_owned();
        Box::pin(async move {
            Ok(ToolResult {
                content: vec![OutputContent::Text { text }],
                details: None,
                is_error: false,
                terminate: false,
            })
        })
    }

    fn execution_mode(&self) -> ExecutionMode { ExecutionMode::Parallel }
}
```

创建 `Agent` 时传入 boxed `opi_ai::Provider`、工具列表、模型、可选系统提示词、`AgentLoopConfig` 和 `AgentHooks` 实现即可。

## 模块速查

| 模块 | 作用 |
|------|------|
| `agent` | `Agent` 封装与消息缓冲区管理 |
| 根模块 `agent_loop` | Provider/tool turn 主循环 |
| `tool` | `Tool`、`ToolResult`、`ToolError`、`ExecutionMode` |
| `hooks` | Hook trait 及其上下文/结果类型 |
| `event` | 运行时事件协议 |
| `session_event` | JSON 模式使用的会话级事件协议 |
| `session` | JSONL 会话 header、条目、writer、reader、恢复 |
| `compaction` | 上下文压缩引擎与 hooks |
| `state` | 对话状态容器 |
| `message` | Agent 层消息变体 |
| `loop_types` | 主循环上下文、配置和错误 |
| `validation` | JSON Schema 参数校验 |
| `transport` | 外部工具服务器 transport trait |

## 许可证

MIT。详见 workspace [LICENSE](../../LICENSE)。
