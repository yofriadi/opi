# Opi 技术规范

> Opi 是 [pi](https://github.com/earendil-works/pi) AI 代理工具包的 Rust 重新实现。它保留了 pi 的运行时语义，同时采用 Rust 原生的 API、存储格式和发布实践。

## 0. 文档控制

| 字段 | 值 |
|---|---|
| 状态 | 草案 |
| 规范版本 | 0.5-draft |
| 最后更新 | 2026-06-08 |
| 仓库 | `https://github.com/OdradekAI/opi` |
| 参考上游 | `pi` 0.75.3，位于 `.repo/pi-0.75.3/` |
| 当前实现 | `opi` 0.5.0 workspace，第五阶段产品化 extension/package 生态系统已实现 |
| 下一里程碑 | API 稳定化和更广泛的 adapter 协议支持 |

本文档对当前设计具有规范性。涉及公共 API、事件协议、会话存储、发布行为或阶段边界变更的修改，应在同一变更中更新本文件。

规范性术语：

- **必须（MUST）** 表示合规所要求的。
- **应当（SHOULD）** 表示除非有文档化的理由，否则应遵循。
- **可以（MAY）** 表示可选的扩展行为。

## 1. 概述

Opi 以五个 Rust crate 映射 pi 的包结构：

- `opi-ai`：与供应商无关的 LLM 流式处理。
- `opi-agent`：代理循环、有状态代理、钩子、工具、队列和会话原语。
- `opi-tui`：终端 UI 组件。
- `opi-coding-agent`：`opi` CLI 二进制文件。
- `opi-web-ui`：未发布的可复用 RPC/SDK 事件、状态、组件和 HTML 渲染 crate。

本仓库已经在第三阶段终端编程 Agent 基础上完成第四阶段可扩展性基底：RPC JSONL 模式、共享 SDK 类型、extension hooks/tools/state、资源发现、skills、prompt fragments、themes、packages、自定义 provider/model 注册、会话分支选择、streaming proxy 原语，以及可复用的 Web-facing 组件/状态/渲染代码已经存在。第五阶段添加了产品化 extension/package 生态系统：本地和 git package 来源、`package add/remove/list/doctor` CLI、带 `[adapter]` 声明的 manifest V2、使用 `opi-extension-jsonl-v1` 协议的 `process-jsonl` adapter 托管，以及 adapter 到运行时的工具/命令/hooks/事件/状态/取消桥接。MCP、子 Agent、plan mode、todos、permission gates、动态插件加载和独立浏览器应用应建立在该基底之上，而不是成为核心功能。

核心设计规则：

> 在用户和集成方依赖的地方保留 pi 的行为。默认不保留 pi 的 TypeScript API、npm 扩展 ABI、配置文件或会话文件。

## 2. 设计理念

| 原则 | pi 0.75.3 | opi 设计 |
|---|---|---|
| 最小化核心 | `CONTRIBUTING.md` 和编程代理文档将工作流特定功能保持在核心之外 | 第 1-3 阶段避免 MCP、动态插件、Web UI、子代理、计划模式、待办系统和后台 bash 的范围蔓延 |
| 分层运行时 | `agentLoop` -> `Agent` -> `AgentHarness` / `AgentSession` | `agent_loop` -> `Agent` -> `Harness` / `CodingHarness` |
| 流式优先 | `AssistantMessageEventStream` 和代理事件流 | `Stream<Item = Result<Event, Error>>` 加终端事件 |
| 供应商无关 | API、供应商和模型是独立概念 | `Provider` trait、注册表、供应商适配器 |
| 代理消息 vs LLM 消息 | `AgentMessage[] -> transformContext -> convertToLlm -> Message[]` | 应用消息在 `opi-agent`，供应商消息在 `opi-ai` |
| 工具隔离 | LLM 边界处的 TypeBox schema | 类型化的 Rust 工具输入，在 LLM 边界生成 JSON Schema |
| 错误在流内 | 供应商故障变为 `error` 流事件 | 供应商/运行时故障作为事件呈现，而非 panic |
| 仅追加会话 | 崩溃安全的 JSONL 会话文件 | opi 版本化树状 JSONL，灵感来自 pi |
| 锁步发布 | 所有包共享版本 | 所有 crate 共享 `workspace.package.version` |

### 2.1 非目标

Opi 不是逐行移植。Rust 的枚举、trait、所有权和取消原语应当塑造实现方式。

Opi 与 pi 不是 API 兼容的。TypeScript 的声明合并、`jiti` 扩展加载和 npm 包导出无法干净地映射到 Rust crate 和静态二进制文件。

Opi 在第一阶段不要求读取 pi 配置或 pi 会话文件。迁移命令可以在后续添加，但不假设运行时兼容性。

Opi 的 MVP 不是一个可扩展平台。MCP 不是 pi 设计中的内置核心功能；它可以在扩展 API 稳定后作为扩展或包提供。内置子代理、计划模式、待办系统、后台 bash、永久权限弹窗工作流、WASM 插件、子进程插件运行时和 Web UI 工作都不属于第 1-3 阶段核心范围。

### 2.2 pi 设计边界

Pi 0.75.3 是一个最小化的终端编程 harness。除非后续设计明确选择偏离，Opi 应保留这些边界：

- CLI/TUI 仍然是主要产品表面。
- 核心提供实用默认值，而不是强工作流意见。
- MCP、子代理、计划模式、权限门禁和待办系统属于扩展、包或外部工具，而不是内置核心。
- 工具安全主要通过工具选择、可见性、容器/沙箱和扩展钩子控制。
- RPC 和 SDK 表面支持组合，但不应让终端产品退居次要。

## 3. 与 pi 的关系

Pi 是行为参考。以下行为应被视为继承的设计，而非偶然的实现细节。

### 3.1 Opi 必须保留的语义

| 领域 | 要求的行为 | 上游锚点 |
|---|---|---|
| 代理事件顺序 | `agent_start`、`turn_start`、消息事件、工具事件、`turn_end`、`agent_end` | `packages/agent/README.md` |
| 供应商流生命周期 | `start`、内容增量、内容结束事件，然后 `done` 或 `error` | `packages/ai/src/types.ts` |
| 流内错误 | 请求开始后的失败是流错误和最终的失败助手消息 | `StreamFunction` 合约 |
| 消息转换 | 应用消息在供应商转换前被变换 | `AgentMessage` / `convertToLlm` |
| 工具批处理 | 默认并行；任何顺序工具使整个批次变为顺序执行 | pi agent README |
| 工具结果顺序 | 完成事件可按完成顺序；持久化的工具结果消息遵循助手源顺序 | pi agent README |
| 工具终止 | 仅当批次中每个已完成结果都有 `terminate` 时才提前停止 | pi agent README |
| 工具钩子 | before 钩子可阻塞；after 钩子替换字段而非深度合并 | pi hook result types |
| `shouldStopAfterTurn` | 在 `turn_end` 之后、steering/follow-up 轮询之前运行 | pi agent README |
| 引导队列 | 在当前助手轮次和工具调用之后、下一次供应商调用之前交付 | pi agent README 和 RPC 文档 |
| 后续队列 | 仅在代理即将停止时交付 | pi agent README 和 RPC 文档 |
| 会话持久性 | 仅追加写入和从不完整最后一行恢复 | pi session manager |

### 3.2 Rust 原生重新设计

| pi 机制 | opi 替代方案 | 理由 |
|---|---|---|
| TypeScript 联合类型和声明合并 | Rust 枚举加显式扩展变体 | 穷尽匹配和更安全的演化 |
| TypeBox schemas | `schemars` 生成的 JSON Schema 加 `jsonschema` 验证 | 动态供应商边界，静态工具代码 |
| 动态供应商导入 | feature flag 加显式注册 | 可预测的二进制文件和交叉编译 |
| `jiti` TypeScript 扩展 | 推迟的 Rust 兼容插件方案 | 在 MVP 中避免 Node 依赖和不稳定的 ABI |
| pi `settings.json` / `auth.json` | TOML 配置和显式凭据解析 | Rust 生态系统惯例和注释支持 |
| pi session v3 | opi session v1 树状 JSONL | 保留分支/压缩语义而不锁定 TS 特定条目 |
| 自定义 TUI 渲染器 | `ratatui` + `crossterm` | 活跃的 Rust 终端技术栈 |

### 3.3 功能对等矩阵

| pi 能力 | Opi 阶段 | 兼容目标 |
|---|---:|---|
| 包/crate 布局 | 第 0 阶段已完成 | 结构对等 |
| 二进制 | 第 0 阶段占位符，第 1 阶段可用 | `opi`，非 `pi` |
| 供应商流式处理 | 第 1 阶段 | 语义对等 |
| Anthropic 供应商 | 第 1 阶段 | 语义对等 |
| `agentLoop` / `Agent` | 第 1 阶段 | 语义对等 |
| read/write/edit/bash 加文件搜索/列表工具 | 第 1/3 阶段 | 行为对等；保留 `glob` 作为 opi 原生搜索工具，在声明稳定 CLI 前补齐 `find`/`ls` 对等 |
| 交互式 TUI | 第 1 阶段 | 用户体验对等 |
| OpenAI 兼容/OpenRouter/OpenAI/Gemini/Mistral | 第 2 阶段 | 供应商合约对等 |
| 会话/恢复 | 第 2 阶段 | opi 格式 |
| 压缩 | 第 2 阶段 | 语义对等 |
| JSON 事件模式 | 第 2 阶段 | 版本化的 opi NDJSON |
| 图像支持 | 第 3 阶段 | 语义对等 |
| 工具选择和安全钩子 | 第 3 阶段 | pi 风格 allowlist 和扩展介导确认，而不是核心权限弹窗子系统 |
| RPC/SDK/扩展/技能/包 | 第 4 阶段 | pi 风格组合和定制 |
| MCP 适配器 | 第 4 阶段以后 | 可选扩展/包示例，不内置到核心 |
| Web UI | 第 4 阶段以后 | 推迟的 opi 特定设计 |

持续维护的包级/阶段级偏移账本位于
[`docs/pi-alignment-matrix.zh.md`](pi-alignment-matrix.zh.md)。

## 4. 当前基线

### 4.1 版本 0.5.0

| 领域 | 当前状态 |
|---|---|
| 工作区 | 一个 Cargo 工作区下的五个 crate |
| 版本控制 | 锁步 `0.5.0` |
| 版本（Edition） | Rust 2024 |
| 内部依赖 | `opi-agent -> opi-ai`、`opi-coding-agent -> opi-ai + opi-agent + opi-tui`；`opi-web-ui` 无内部依赖 |
| 外部依赖 | 来自工作区依赖的 Rust 原生异步、HTTP/SSE、schema、配置、TUI、搜索、追踪和测试技术栈 |
| 二进制 | `opi` 支持交互式 TUI、非交互文本模式、`--json`、`--rpc`、会话命令、`--version` 和 `--help` |
| CI | `fmt`、`clippy`、`test`、`doc` |
| 发布 CI | 六平台二进制工作流 |
| 可扩展性 | RPC JSONL、SDK 类型、extension API、资源/package 发现、自定义 provider/model registry、分支选择、streaming proxy、process-JSONL adapter 托管（`opi-extension-jsonl-v1`）、package CLI（`add/remove/list/doctor`），以及可复用 Web UI 组件/状态/渲染表面已经作为不稳定 0.x API 实现 |
| crates.io | 可发布 crate 受质量门控；`opi-web-ui` 保持未发布 |

### 4.2 稳定前 API 说明

第 0 阶段占位符已被替换，但除非另有明确文档说明，0.x 公共 API 仍不稳定。第 3 阶段应加固已有表面，而不是引入宽泛的新平台范围。

| Crate | 当前表面 | 下一目标 |
|---|---|---|
| `opi-ai` | 供应商流式处理、模型注册表、用量/成本、重试/退避、自定义 provider/model 注册 | 尽可能通过注册机制保持 Provider 扩展性 |
| `opi-agent` | 代理循环、钩子、队列、工具、会话、压缩、SDK 类型、extension API、streaming proxy | 保持核心运行时狭窄，并把所有 0.x 公共表面明确标为不稳定 |
| `opi-tui` | ratatui 组件、markdown/代码、diff、主题、键绑定、图像渲染、模糊选择器、分支选择器 | 通过快照测试保持组件可复用和确定性 |
| `opi-coding-agent` | `clap` CLI、TOML 配置、内置工具、会话、JSON/RPC 模式、资源/package 发现、分支选择 | 将可扩展性元数据接入 prompt/RPC，但不声称动态加载 Rust 插件 |
| `opi-web-ui` | 未发布的 RPC/SDK 事件解析器、对话状态、组件模型、HTML renderer | 在单独发布决策前保持 `publish = false`；还没有独立浏览器应用 |

### 4.3 第 0 阶段完成情况

第 0 阶段已完成：

- 五 crate 工作区；
- 锁步版本控制；
- 占位模块和重导出；
- CI 门控；
- 六平台发布工作流；
- `opi --version` 和 `opi --help`；
- 仅 GitHub Release，crates.io 推迟。

## 5. 工作区与依赖

### 5.1 布局

```text
opi/
|-- Cargo.toml
|-- crates/
|   |-- opi-ai/
|   |-- opi-agent/
|   |-- opi-coding-agent/
|   |-- opi-tui/
|   `-- opi-web-ui/
|-- docs/
|-- .github/workflows/
`-- .claude/skills/opi-release/
```

早期草案中的根目录 `config/` 目录不存在。内置主题或语法资源应存放在拥有它们的 crate 中，直到出现真正的共享资源需求。

### 5.2 依赖图

```text
opi-ai           （无内部依赖）
opi-tui          （无内部依赖）
opi-agent        -> opi-ai
opi-web-ui       （无内部依赖）
opi-coding-agent -> opi-ai, opi-agent, opi-tui
```

内部依赖必须在根 `[workspace.dependencies]` 中声明，消费者通过 `{ workspace = true }` 引用。

### 5.3 Crate 角色

| Crate | 类型 | 发布目标 | 角色 |
|---|---|---|---|
| `opi-ai` | 库 | 通过发布门控后发到 crates.io | 供应商协议、模型元数据、面向供应商的消息 |
| `opi-agent` | 库 | 通过发布门控后发到 crates.io | 循环、代理、钩子、工具、队列、会话 |
| `opi-tui` | 库 | 通过发布门控后发到 crates.io | 终端渲染库 |
| `opi-coding-agent` | 二进制 | 通过发布门控后发到 crates.io | `opi` CLI 应用 |
| `opi-web-ui` | 库 | 不发布 | 可复用 RPC/SDK 事件解析、对话状态、组件模型和 HTML renderer |

### 5.4 为何没有 `opi-types`

类型归属于拥有其语义的 crate：

- 面向供应商的 `Message`、`ToolDef`、`ModelInfo` 和 `Usage` 属于 `opi-ai`；
- 运行时的 `AgentMessage`、`AgentEvent`、`Tool` 和 `SessionEntry` 属于 `opi-agent`；
- CLI 配置属于 `opi-coding-agent`；
- 视觉状态属于 `opi-tui`。

共享类型 crate 会成为枢纽依赖。如果某个类型跨越 crate 边界，较低语义层级的拥有者应直接暴露它。预期会增长的公共枚举应在 API 稳定前使用 `#[non_exhaustive]`。

### 5.5 依赖计划

第 1 阶段的依赖应当以能交付 MVP 的最小功能集引入。优先选择显式 feature、可选的重量级功能以及后续阶段添加，而非宽泛的默认值。

| 类别 | Crate | 状态 | 理由 |
|---|---|---|---|
| 异步运行时 | `tokio` | 已有，窄 feature | 网络、进程 IO、信号、定时器；除非有具体需要否则避免 `features = ["full"]` |
| 序列化 | `serde`、`serde_json` | 已有 | 供应商/会话协议 |
| 库错误 | `thiserror` | 已有 | 库 crate 的类型化错误处理 |
| 应用错误 | `anyhow` | 第 1 阶段 | `opi-coding-agent` 中的顶层错误聚合；库 crate 的公共 API 中禁止使用 `anyhow` |
| 异步 trait | `async-trait` | 已有，保持内部使用或在 API 稳定前移除 | 不是目标公共 API 依赖；dyn trait 使用显式的 boxed future/stream 返回值；内部非 dyn trait 可使用原生 async fn |
| HTTP/SSE | `reqwest` 配合 `rustls-tls` | 第 1 阶段，窄 feature | 无需 OpenSSL 的供应商流式处理；使用 `default-features = false` 并仅启用所需的 HTTP/JSON/流 feature |
| SSE 解析 | 手写行解析器或 `eventsource-stream` | 第 1 阶段 | `reqwest-eventsource` 被排除（不支持 POST）；Anthropic 使用基于 POST 的流式处理 |
| 流 | `futures-core`，按需内部流辅助工具 | 第 1 阶段 | 公共流 API 应暴露 `futures-core::Stream`；保持 `futures-util` 等辅助工具为内部使用 |
| 取消 | `tokio-util` | 第 1 阶段 | 协作式取消 |
| CLI | `clap` | 第 1 阶段 | 稳定的选项和补全 |
| 配置 | `toml` | 第 1 阶段 | 人类可编辑的配置 |
| TUI | `ratatui`、`crossterm` | 第 1 阶段 | 跨平台终端 UI |
| Schema | `schemars`、`jsonschema` | 第 1 阶段，工具边界优先 | 类型化的工具 schema 加上在模型/工具边界的运行时验证；在 schema 稳定前避免广泛的协议验证；参见 §5.6 关于草案兼容性 |
| ID/时间 | `uuid`、`time` | 第 1 阶段 | 无需 `chrono` 额外表面的会话 ID 和时间戳 |
| 文件搜索 | `ignore`、`globset`、`regex` | 第 1 阶段 | gitignore 感知的 glob 和 grep 行为 |
| 追踪 | `tracing`、`tracing-subscriber` | 第 1/2 阶段 | 可观测性 |
| Markdown/代码 | `pulldown-cmark`，后续可选 `syntect` | 第 1/2 阶段 | 先做基础 markdown；语法高亮必须是可选的或后续添加的，以免威胁二进制大小目标 |
| Diff | `similar` | 第 2 阶段 | 补丁可视化；在真正的 diff 视图发布前不要添加 |

### 5.6 JSON Schema 草案兼容性

Anthropic 的 Messages API 接受工具 `input_schema` 作为带有顶层 `type: "object"` 约束的 JSON Schema 对象。API 验证错误表明使用了 draft-2020-12 兼容的验证器，而 `schemars` 0.8 默认生成 draft-07。

对于第 1 阶段的工具 schema（简单的 object + properties + required），draft-07 输出应当保持在 Anthropic 接受的通用 JSON Schema 子集内。使用在各草案版本间存在分歧的特性的复杂 schema（数组 `items` vs `prefixItems`、`definitions` vs `$defs`、条件关键字）可能被拒绝。

要求：

- 第 1 阶段必须包含对生成的内置工具 schema 的本地固定测试，包括在反序列化前验证代表性的模型参数。
- 第 1 阶段应当包含一个被忽略的、由环境变量门控的实时 Anthropic schema 验收测试，但默认 CI 禁止要求付费凭据或网络访问。
- 如果发现不兼容，schema 后处理步骤应当将 draft-07 输出规范化为供应商接受的子集（例如，在需要时将 `definitions` 重命名为 `$defs`）。
- `schemars` 1.0（稳定后）可能原生解决此问题；在此之前，将其视为具有已测试缓解路径的已知风险。

## 6. 架构

### 6.1 分层

```text
opi-coding-agent
  CLI、内置工具、配置、提示词、工具选择、应用层会话 UX

CodingHarness / Harness
  会话持久化、压缩、应用钩子、模型/思考状态、队列

Agent
  有状态的运行时包装器、订阅、取消、prompt/continue API

agent_loop
  纯 LLM -> 工具 -> LLM 循环，无持久化或 UI 策略

opi-ai Provider
  供应商 HTTP、SSE 解析、模型元数据、面向供应商的消息
```

`agent_loop` 必须能够使用模拟供应商和模拟工具进行测试，无需磁盘或终端状态。`Agent` 添加状态、取消、队列和事件订阅。`Harness` 组合会话、压缩和应用钩子。

### 6.2 Harness 边界

Pi 0.75.3 同时具有可复用的 `AgentHarness` 代码和编程代理的 `AgentSession`。Opi 应避免意外复制这种拆分。

- `opi-agent` 应当拥有非 CLI 消费者所需的通用 harness 原语。
- `opi-coding-agent` 应当拥有编程特定的行为：内置文件工具、项目上下文、工具 allowlist、CLI 配置和应用层会话命令。
- 如果某个功能同时被库消费者和 CLI 需要，它属于 `opi-agent`；否则留在 `opi-coding-agent`。

### 6.3 运行时流程

```text
用户输入
  -> CLI 解析模式和配置
  -> CodingHarness 加载或创建会话
  -> 从基础提示、工具、项目上下文、摘要构建系统提示
  -> Agent 接收 prompt、steer、follow-up 或 continue 请求
  -> agent_loop 将 AgentMessage 转换为供应商 Message
  -> 供应商流式返回助手事件
  -> 代理发出消息更新
  -> 工具调用被验证并执行
  -> 工具结果消息按助手源顺序追加
  -> should_stop_after_turn 运行
  -> 引导队列被轮询
  -> 后续队列仅在代理即将停止时被轮询
  -> 会话条目被追加
  -> 订阅者在 agent_end 后稳定
```

### 6.4 边界规则

- 供应商适配器禁止执行工具。
- 工具禁止直接调用供应商，除非该工具明确是一个集成。
- TUI 组件必须消费事件和快照；禁止拥有循环策略。
- `agent_loop` 测试禁止要求会话存储。
- CLI 快捷方式禁止泄漏到 `opi-agent`，除非它们描述的是可复用的运行时行为。

## 7. 协议和数据模型

Opi 有四个相关协议。它们必须保持独立。

| 协议 | 所有者 | 用途 |
|---|---|---|
| 供应商流事件 | `opi-ai` | 将供应商分块规范化为助手增量 |
| 代理事件 | `opi-agent` | 循环/消息/工具生命周期，用于 UI 和测试 |
| 代理会话事件 | harness / `opi-coding-agent` | 队列、压缩、重试、会话元数据 |
| 会话条目 | 存储 | 用于重建上下文的持久化记录 |

### 7.1 面向供应商的消息

```rust
#[non_exhaustive]
pub enum Message {
    User(UserMessage),
    Assistant(AssistantMessage),
    ToolResult(ToolResultMessage),
}

pub struct UserMessage {
    pub content: Vec<InputContent>,
    pub timestamp_ms: i64,
}

pub struct AssistantMessage {
    pub content: Vec<AssistantContent>,
    pub api: ApiKind,
    pub provider: String,
    pub model: String,
    pub response_model: Option<String>,
    pub response_id: Option<String>,
    pub usage: Usage,
    pub stop_reason: StopReason,
    pub error_message: Option<String>,
    pub timestamp_ms: i64,
}

pub struct ToolResultMessage {
    pub tool_call_id: String,
    pub tool_name: String,
    pub content: Vec<OutputContent>,
    pub details: Option<serde_json::Value>,
    pub is_error: bool,
    pub timestamp_ms: i64,
}
```

停止原因应当与 pi 保持接近：`stop`、`length`、`tool_use`、`error`、`aborted`。

### 7.2 代理消息

```rust
#[non_exhaustive]
pub enum AgentMessage {
    Llm(opi_ai::Message),
    CompactionSummary(CompactionSummaryMessage),
    BranchSummary(BranchSummaryMessage),
    Custom(CustomAgentMessage),
}
```

每次供应商调用前：

1. `transform_context` 在 `AgentMessage` 层级工作。
2. `convert_to_llm` 转换为 `Vec<opi_ai::Message>` 并过滤仅用于会话/UI 的消息。

未知的自定义消息禁止导致运行时 panic。

### 7.3 供应商流事件

```rust
#[non_exhaustive]
pub enum AssistantStreamEvent {
    Start { partial: AssistantMessage },
    TextStart { content_index: usize, partial: AssistantMessage },
    TextDelta { content_index: usize, delta: String, partial: AssistantMessage },
    TextEnd { content_index: usize, content: String, partial: AssistantMessage },
    ThinkingStart { content_index: usize, partial: AssistantMessage },
    ThinkingDelta { content_index: usize, delta: String, partial: AssistantMessage },
    ThinkingEnd { content_index: usize, content: String, partial: AssistantMessage },
    ToolCallStart { content_index: usize, partial: AssistantMessage },
    ToolCallDelta { content_index: usize, delta: String, partial: AssistantMessage },
    ToolCallEnd { content_index: usize, tool_call: ToolCall, partial: AssistantMessage },
    Done { reason: StopReason, message: AssistantMessage },
    Error { reason: StopReason, message: AssistantMessage },
}
```

每个供应商流必须在增量之前发出 `Start`，并以恰好一个 `Done` 或 `Error` 终止。一旦请求已开始，请求/模型/运行时故障应当成为带有最终助手消息的 `Error` 事件，而不是带外故障。

### 7.4 代理事件

```rust
#[non_exhaustive]
pub enum AgentEvent {
    AgentStart,
    AgentEnd { messages: Vec<AgentMessage> },
    TurnStart,
    TurnEnd { message: AgentMessage, tool_results: Vec<opi_ai::ToolResultMessage> },
    MessageStart { message: AgentMessage },
    MessageUpdate { message: AgentMessage, assistant_event: AssistantStreamEvent },
    MessageEnd { message: AgentMessage },
    ToolExecutionStart { tool_call_id: String, tool_name: String, args: serde_json::Value },
    ToolExecutionUpdate { tool_call_id: String, tool_name: String, args: serde_json::Value, partial_result: serde_json::Value },
    ToolExecutionEnd { tool_call_id: String, tool_name: String, result: serde_json::Value, is_error: bool },
}
```

`MessageUpdate` 仅用于助手消息。`AgentEnd` 表示不再发出循环事件，但等待中的订阅者可能仍在稳定。

### 7.5 会话事件

```rust
#[non_exhaustive]
pub enum AgentSessionEvent {
    Agent(AgentEvent),
    QueueUpdate { steering: Vec<String>, follow_up: Vec<String> },
    CompactionStart { reason: CompactionReason },
    CompactionEnd { reason: CompactionReason, result: Option<CompactionResult>, aborted: bool, will_retry: bool, error_message: Option<String> },
    AutoRetryStart { attempt: u32, max_attempts: u32, delay_ms: u64, error_message: String },
    AutoRetryEnd { success: bool, attempt: u32, final_error: Option<String> },
    SessionInfoChanged { session_id: String, name: Option<String> },
    ThinkingLevelChanged { level: ThinkingLevel },
}
```

当第 2 阶段 JSON 模式实现时，`--json` 每行输出一个 JSON 对象。事件协议必须在下游工具将其视为稳定之前包含 schema 版本。

### 7.6 队列

```rust
pub enum QueueMode {
    All,
    OneAtATime,
}
```

引导消息在当前助手轮次及其工具调用完成后、下一次供应商请求前交付。后续消息仅在代理没有工具调用、没有引导消息且即将停止时交付。如果 `should_stop_after_turn` 返回 true，循环在轮询任一队列之前退出。

## 8. Crate 规范

### 8.1 `opi-ai`

`opi-ai` 拥有面向供应商的消息类型、模型元数据、供应商注册表、凭据辅助工具和流式适配器。

```rust
pub trait Provider: Send + Sync {
    fn id(&self) -> &str;
    fn models(&self) -> &[ModelInfo];
    fn stream(&self, request: Request) -> EventStream;
}

pub type EventStream =
    Pin<Box<dyn Stream<Item = Result<AssistantStreamEvent, ProviderError>> + Send>>;
```

`stream` 返回一个流句柄。取消通过 `Request::cancel` 或等效令牌传播。丢弃流应当取消底层 HTTP 请求。

```rust
pub struct Request {
    pub model: String,
    pub system: Option<String>,
    pub messages: Vec<Message>,
    pub tools: Vec<ToolDef>,
    pub max_tokens: Option<u64>,
    pub temperature: Option<f64>,
    pub thinking: ThinkingConfig,
    pub stop_sequences: Vec<String>,
    pub metadata: Option<serde_json::Value>,
    pub cancel: CancellationToken,
}
```

供应商优先级：

| 供应商 | API 风格 | 阶段 | 原因 |
|---|---|---:|---|
| Anthropic | Messages SSE | 1 | MVP 目标和 pi 的默认模型家族 |
| OpenAI 兼容聊天 | SSE | 2 | 广泛兼容 OpenAI 风格的服务 |
| OpenRouter | OpenAI 兼容路由器 | 2 | 快速模型覆盖扩展和路由诊断 |
| OpenAI Responses | SSE | 2 | 独立的事件映射 |
| Google Gemini | 流式 generateContent | 2 | 主要的非 OpenAI 系列 |
| Mistral | 聊天 SSE | 2 | 供应商矩阵扩展 |
| AWS Bedrock | 响应流 / SigV4 | 3 | 企业认证复杂性 |
| Azure OpenAI | OpenAI 兼容 | 3 | 部署名称差异 |
| Google Vertex | OAuth/服务账号 | 3 | 企业认证复杂性 |

Provider 扩展策略：

- 只有当 wire format、event model 或认证模型存在实质差异时，才增加一等 provider。
- 如果 provider 能用 base URL、API key 环境变量、模型元数据和兼容性 flags 表达，应使用配置化 OpenAI-compatible profile。
- deployment-specific 或 fine-tuned 模型元数据应优先通过 `ProviderRegistry` model overrides 表达。
- 嵌入方和外部 adapter 应通过 extension/SDK provider registration 接入。

OAuth 保持为单独产品决策。Anthropic OAuth、OpenAI Codex OAuth 和 GitHub Copilot OAuth 都会引入登录命令、credential storage、refresh 行为和面向用户的撤销语义；它们不得作为 provider profile 扩展的副作用被静默加入。

凭据优先级：

1. 显式 CLI/配置覆盖；
2. 供应商特定的环境变量；
3. 已实现时的本地认证存储；
4. 环境云凭据链。

密钥禁止被日志记录、持久化到会话或包含在诊断信息中。

### 8.2 `opi-agent`

`opi-agent` 可在不依赖 `opi` 二进制的情况下使用。

```rust
pub trait Tool: Send + Sync {
    fn definition(&self) -> opi_ai::ToolDef;

    fn execute(
        &self,
        call_id: &str,
        arguments: serde_json::Value,
        signal: CancellationToken,
        on_update: Option<UpdateCallback>,
    ) -> Pin<Box<dyn Future<Output = Result<ToolResult, ToolError>> + Send>>;

    fn execution_mode(&self) -> ExecutionMode {
        ExecutionMode::Parallel
    }
}

pub enum ExecutionMode {
    Sequential,
    Parallel,
}

pub struct ToolResult {
    pub content: Vec<opi_ai::OutputContent>,
    pub details: Option<serde_json::Value>,
    pub is_error: bool,
    pub terminate: bool,
}
```

内置工具应当定义类型化的 Rust 参数结构体，派生 `Deserialize` 和 `schemars::JsonSchema`。`ToolDef` 向供应商暴露生成的 JSON Schema，而来自模型的动态输入在反序列化前使用 `jsonschema` 验证。`serde_json::Value` 在协议边界和诊断中可以接受，但工具业务逻辑不应保持基于 Value 的方式。

参数验证发生在 `ToolExecutionStart` 之后和 `before_tool_call` 之前。验证失败成为错误工具结果。

执行规则：

- 全局顺序意味着所有调用顺序运行；
- 全局并行意味着调用并发运行，除非任何目标工具是顺序的；
- 如果批次中任何工具是顺序的，整个批次顺序运行；
- 持久化的工具结果消息按助手源顺序排列。

钩子表面：

```rust
pub trait AgentHooks: Send + Sync {
    fn transform_context(&self, messages: Vec<AgentMessage>, signal: CancellationToken)
        -> Pin<Box<dyn Future<Output = Result<Vec<AgentMessage>, AgentError>> + Send>>;

    fn convert_to_llm(&self, messages: &[AgentMessage])
        -> Result<Vec<opi_ai::Message>, AgentError>;

    fn before_tool_call(&self, ctx: BeforeToolCallContext)
        -> Pin<Box<dyn Future<Output = BeforeToolCallResult> + Send>>;

    fn after_tool_call(&self, ctx: AfterToolCallContext)
        -> Pin<Box<dyn Future<Output = AfterToolCallResult> + Send>>;

    fn should_stop_after_turn(&self, ctx: ShouldStopAfterTurnContext)
        -> Pin<Box<dyn Future<Output = bool> + Send>>;

    fn prepare_next_turn(&self, ctx: PrepareNextTurnContext)
        -> Pin<Box<dyn Future<Output = Option<AgentLoopTurnUpdate>> + Send>>;
}
```

`after_tool_call` 使用字段替换语义，禁止深度合并 `content` 或 `details`。

底层循环：

```rust
pub async fn agent_loop(
    context: AgentLoopContext,
    config: AgentLoopConfig,
    hooks: &dyn AgentHooks,
    events: AgentEventSink,
    cancel: CancellationToken,
) -> Result<Vec<AgentMessage>, AgentError>;
```

`Agent` 用状态、prompt/continue 方法、中止、引导和后续队列、订阅者管理和空闲稳定来包装循环。继续要求最后的上下文消息是用户消息或工具结果。

`opi_agent::Transport` 已在第 4 阶段移除。RPC/proxy 表面现在位于 `opi-coding-agent::rpc`、`opi-agent::sdk` 和 `opi-agent::streaming_proxy`。

### 8.3 `opi-tui`

第 1 阶段组件：

| 组件 | 阶段 | 用途 |
|---|---:|---|
| `MessageList` | 1 | 流式对话显示 |
| `InputEditor` | 1 | 多行提示输入 |
| `StatusBar` | 1 | 模型、状态、token/成本摘要占位符 |
| `ToolCallView` | 1 | 工具调用参数和状态 |
| `MarkdownView` | 1 | 基础 markdown 文本 |
| `CodeBlock` | 1/2 | 语法高亮的代码块 |
| `DiffView` | 2 | 编辑和补丁可视化 |
| `SelectList` | 3 | 会话/模型选择器 |

TUI 的目标是用户可见的行为，而非与 pi 的渲染器兼容：低闪烁、响应式流处理、调整大小安全、Windows 兼容性，以及在小终端上的优雅降级。

第 1 阶段应保持最小可用的 TUI：流式消息、提示输入、状态和工具调用可见性。主题、模糊搜索选择器、丰富的 diff 视图和超出基础围栏代码展示的语法高亮属于后续阶段或可选功能。

### 8.4 `opi-coding-agent`

二进制拥有 CLI 解析、配置加载、供应商注册表构建、内置工具、系统提示构建、会话 UX、工具选择和运行时模式。

| 工具 | 模式 | 阶段 | 范围 |
|---|---|---:|---|
| `read` | 并行 | 1 | 读取文件内容，可选行范围 |
| `write` | 顺序 | 1 | 创建或替换文件 |
| `edit` | 顺序 | 1 | 精确字符串替换或结构化补丁 |
| `bash` | 顺序 | 1 | 带超时和流式输出的子进程命令 |
| `glob` | 并行 | 1 | gitignore 感知的文件发现 |
| `grep` | 并行 | 1 | gitignore 感知的文件内容正则搜索 |
| `find` | 并行 | 3 | pi 兼容的文件发现别名，具备 gitignore 感知行为 |
| `ls` | 并行 | 3 | pi 兼容的目录列表，输出有界 |

第 1 阶段必须包含高风险工具的最小安全边界。`write`、`edit` 和 `bash` 必须在执行前显示提议的路径或命令、有效的工作目录、环境策略、超时以及目标是否在工作区内。非交互模式必须在运行变异文件工具或 shell 命令之前提供显式的选择加入策略。交互式确认可以作为狭窄的起步安全措施存在，但可复用权限配置文件和权限弹窗并不是 pi 的核心对等功能；更丰富的门禁应通过工具 allowlist、钩子、扩展、包、容器或外部 wrapper 构建。

工具选择 flag 应在稳定 CLI 声明前贴近 pi 的形态：`--tools <list>` 用于 allowlist，`--no-tools` 禁用所有工具，`--no-builtin-tools` 在扩展/自定义工具存在后禁用内置工具。

CLI 目标：

```text
opi [选项] [提示]

选项:
  -m, --model <规格>       模型，例如 anthropic:claude-sonnet-4
  -c, --config <路径>      配置文件路径
  -s, --system <路径>      系统提示文件
      --list-models        列出可用模型
      --fork <SESSION_ID>  把已有会话 fork 成新的父子会话
      --non-interactive    单提示模式
  -v, --verbose            启用调试追踪
  -V, --version            打印版本
  -h, --help               打印帮助
```

第 2 阶段在会话存储和 JSON 事件 schema 具有合约测试后添加 `--resume`、`--list-sessions` 和 `--json`。
当前 workspace 还提供 `--fork <SESSION_ID>`，用于从源会话的活跃分支创建新会话，且不改写源 JSONL 文件。

提示层：

1. 基础编程代理指令；
2. 来自 `ToolDef` 的工具描述；
3. 用户系统提示文件；
4. 项目上下文文件，从第 3 阶段开始：来自全局配置和 cwd 祖先目录的 `AGENTS.md` 与 `CLAUDE.md`，匹配 pi；
5. 压缩摘要，从第 2 阶段开始；
6. 技能/提示片段，从第 4 阶段开始。

`OPI.md` 不是默认上下文文件名，因为 pi 和更广泛的编程代理生态已经使用 `AGENTS.md` 与 `CLAUDE.md`。未来可以添加兼容别名，但不得替代这些名称。

### 8.5 `opi-web-ui`

`opi-web-ui` 未发布且保持 `publish = false`，但它不再是占位 crate。它提供 RPC/SDK 事件的类型化解析、对话状态、组件模型和 HTML 渲染辅助。它不是独立浏览器应用，并应继续消费与 JSON/RPC 模式相同的事件 schema，而不是依赖 TUI 内部实现。

## 9. 配置和存储

### 9.1 配置

```toml
[defaults]
model = "anthropic:claude-sonnet-4"
max_iterations = 50
tool_timeout_ms = 30000
theme = "default"

[thinking]
enabled = true
budget_tokens = 10000

[providers.anthropic]
api_key_env = "ANTHROPIC_API_KEY"

[providers.openai_compatible.localai]
api_key_env = "LOCALAI_API_KEY"
base_url = "https://localai.example.com"
max_tokens_field = "max_completion_tokens"

[[providers.openai_compatible.localai.models]]
id = "local-model"
display_name = "Local Model"
context_window = 128000
max_output_tokens = 4096
supports_images = true
supports_streaming = true
supports_thinking = false

[keybindings]
submit = "enter"
abort = "ctrl+c"
new_line = "shift+enter"
```

格式错误的配置文件应当明确失败。对缺失的可选文件允许静默回退，但对无效的用户配置不允许。

### 9.1.1 配置优先级

配置值按以下优先级顺序解析（最高优先）：

1. CLI 参数（`--model`、`--config` 等）
2. 环境变量（`ANTHROPIC_API_KEY`、`OPI_MODEL` 等）
3. 项目配置文件（工作区根目录的 `.opi/config.toml`，实现后）
4. 用户配置文件（`~/.config/opi/config.toml`）
5. 内置默认值

第 1 阶段通过 clap（CLI 参数）+ `std::env`（环境变量）+ `toml` 反序列化（配置文件）+ 结构体默认值实现此功能。第 1 阶段不需要配置框架（figment、config-rs）。如果配置源复杂性超出手动合并能干净处理的范围，后续阶段可以引入框架。

第 1 阶段的配置加载只需要默认值、供应商凭据、模型选择、超时、主题选择和高风险工具策略。压缩、会话和高级键绑定设置可以作为保留字段被接受，但不能暗示这些第 2 阶段功能已激活。

第 2 阶段可以在会话持久化存在后添加 `[compaction]` 表，包含 `enabled`、`reserve_tokens` 和 `keep_recent_tokens` 等字段。

### 9.2 目录布局

```text
~/.config/opi/config.toml
~/.config/opi/themes/
~/.local/share/opi/sessions/
~/.local/share/opi/auth/
```

Windows 上应当使用 `%APPDATA%\opi\` 存放配置类数据，`%LOCALAPPDATA%\opi\` 存放缓存类数据。

### 9.3 会话格式

会话持久化从第 2 阶段开始，而非第 1 阶段。目标格式是仅追加的版本化 JSONL。第一行是头部：

```json
{"type":"session","version":1,"id":"018f...","timestamp":"2026-05-20T12:00:00Z","cwd":"/repo","parent_session":null}
```

后续行是树条目：

```json
{"type":"message","id":"a1b2c3d4","parent_id":null,"timestamp":"2026-05-20T12:00:01Z","message":{"role":"user","content":[{"type":"text","text":"Read src/main.rs"}]}}
{"type":"message","id":"b2c3d4e5","parent_id":"a1b2c3d4","timestamp":"2026-05-20T12:00:02Z","message":{"role":"assistant","content":[{"type":"text","text":"I'll inspect it."}],"stop_reason":"tool_use"}}
{"type":"compaction","id":"c3d4e5f6","parent_id":"b2c3d4e5","timestamp":"2026-05-20T13:00:00Z","summary":"The session inspected CLI scaffolding.","first_kept_entry_id":"b2c3d4e5","tokens_before":45000,"tokens_after":8000}
```

条目类型：

| 类型 | 用途 | LLM 上下文 |
|---|---|---|
| `message` | 用户、助手、工具结果或自定义消息 | 是 |
| `model_change` | 选择的供应商/模型已更改 | 否 |
| `thinking_level_change` | 思考级别已更改 | 否 |
| `compaction` | 摘要加首个保留的条目 | 是 |
| `branch_summary` | 父分支摘要 | 是 |
| `label` | 用户标记 | 否 |
| `session_info` | 名称和元数据 | 否 |
| `custom` | 扩展状态 | 否 |
| `custom_message` | 扩展提供的上下文 | 可配置 |
| `leaf` | 当前分支指针 | 否 |

崩溃恢复可以忽略不完整的最后一行。损坏的中间条目应当被报告；自动跳过中间条目应要求显式恢复模式。

会话 fork 命令会创建新的会话文件。新 header 的 `parent_session` 字段指向源会话 ID，复制的条目来自与 resume 相同的活跃分支重建路径。Fork 绝不能改写源会话文件。

同文件分支创建使用 append-only 树模型：运行时 message 条目使用当前活跃 tip 作为 `parent_id`，compaction 条目链接到前一个活跃 tip 下，turn/compaction 完成后追加 `leaf` 指针来标记活跃分支。选择较早的分支 tip 后继续对话，会在同一个 JSONL session 中创建新的 sibling path，而不会改写旧条目。

### 9.4 为何不使用 pi Session v3

Opi 保留了 pi 的分支和压缩理念，但不使用其文件格式，因为 pi 存储 TypeScript 特定的扩展数据，opi 有独立的配置/插件计划，且意外的部分兼容性会产生误导。未来的迁移命令可以将 pi v3 会话转换为 opi v1 会话。

### 9.5 压缩

压缩从第 2 阶段开始，在会话存储存在之后。

触发条件：

- 手动；
- 基于阈值；
- 上下文溢出恢复。

结果必须记录摘要、`first_kept_entry_id`、压缩前后的 token 数、原因，以及摘要是来自核心还是钩子/扩展。如果在溢出恢复期间压缩失败，代理必须显示可见错误而非静默丢弃历史。

## 10. CLI 和运行时表面

交互模式是 stdin 为 TTY 时的默认模式。它拥有终端初始化、渲染、输入编辑、取消键、工具选择 UX 和任何扩展提供的提示。

非交互模式从 argv 或 stdin 接收一个提示，将助手文本流式输出到 stdout，将诊断信息写入 stderr，并以显式状态退出。

建议的退出码：

| 码 | 含义 |
|---:|---|
| 0 | 成功 |
| 1 | 一般运行时失败 |
| 2 | 无效的 CLI 用法或配置 |
| 3 | 认证失败 |
| 4 | 重试后的供应商/网络故障 |
| 5 | 未恢复的工具故障 |
| 130 | 被用户中断 |

JSON 模式属于第 2 阶段范围。在事件 schema 有合约测试后，它向 stdout 每行输出一个 `AgentSessionEvent` JSON 对象。人类可读的日志发送到 stderr。第 2 阶段 JSON 模式应当接近 pi 的事件模型，但必须包含 opi schema 版本。

RPC 模式是第 4 阶段早期可扩展性表面。它应使用严格的 JSONL 帧：stdin 上每行一个命令，通过可选 `id` 关联响应，stdout 上发送异步事件。RPC 和 SDK 组合应先于动态插件运行时，因为它们贴近 pi 的进程集成模型，同时不会扩张核心策略。

默认的 extension 执行策略是显式注册，而不是动态加载 Rust 库。嵌入方可以通过 `ExtensionRegistry` 注册进程内 Rust extension；外部 package 应通过进程/RPC adapter 暴露可执行行为，把 package 命令转换为 `extension_command` 等 SDK 命令。package/resource discovery 默认只代表元数据和资源组合，除非 adapter 显式注册可执行代码。核心二进制默认不得 `dlopen` 任意 Rust crate，也不得为了保留 pi 的 TypeScript extension 机制而要求 Node/`jiti`。

### 10.1 Package CLI

第五阶段添加了 `opi package` 子命令组，在 provider 构造之前运行：

| 命令 | 用途 |
|---|---|
| `opi package add <source>` | 从本地目录或 git 来源安装 package |
| `opi package remove <name>` | 卸载 package |
| `opi package list` | 列出已安装的 package（支持 `--json`） |
| `opi package doctor` | 诊断 package 问题（支持 `--json`） |

Package 安装到全局（`~/.local/share/opi/packages/`）或项目（`.opi/packages/`）存储中。存储在 `package-lock.toml` 中记录来源路径、可选 git commit、缓存路径和 manifest 哈希。

### 10.2 进程 Adapter

带有 `[adapter]` 声明的 package 以子进程 adapter 方式运行。第五阶段 MVP 支持 `process-jsonl` adapter 类型和 `opi-extension-jsonl-v1` 协议。

Adapter 生命周期：

1. harness 使用配置的命令和参数启动 adapter 子进程。
2. harness 发送 `initialize` 消息；adapter 回复 `capabilities`（工具、命令、hooks）。
3. 运行时，harness 将 adapter 能力桥接到现有 `Extension` trait 方法：`on_command`、`on_before_tool_call`、`on_after_tool_call`、`on_event`、`serialize_state`、`restore_state`。
4. adapter 工具合并到工具集中；adapter hooks 通过 `ExtensionRegistry::wrap_hooks` 与 `CodingAgentHooks` 组合。
5. 关闭时，harness 发送 `shutdown` 消息并回收子进程。

Adapter 协议消息：`initialize`、`capabilities`、`tool`、`command`、`hook`、`event`、`state_serialize`、`state_restore`、`cancel`、`shutdown`。所有消息都是通过 stdin/stdout 的单行 JSON，带有相关联的 `id` 字段。

未被路由到已注册 extension 的 adapter 命令可通过 RPC `extension_command` 分发使用。

## 11. 跨领域运行时关注点

### 11.1 错误处理

| 层 | 方法 |
|---|---|
| `opi-ai` | 类型化 `ProviderError` 加流 `Error` 终端事件 |
| `opi-agent` | 类型化 `AgentError`、`ToolError`、`SessionError` |
| `opi-tui` | 类型化终端/渲染错误 |
| `opi-coding-agent` | 顶层使用 `anyhow::Result` 进行错误聚合；映射退出码；库错误通过 `From` impl 转换 |

库 crate（`opi-ai`、`opi-agent`、`opi-tui`）必须使用 `thiserror` 定义类型化错误，禁止在公共 API 中暴露 `anyhow`。`opi-coding-agent` 可以在类型化错误对最终用户无附加价值时使用 `anyhow`（或 `eyre`）进行顶层错误聚合。

库 crate 必须避免使用 `unwrap` 和 `expect`，除非在测试或可证明安全的静态初始化中。

### 11.2 取消和背压

取消使用 `tokio_util::sync::CancellationToken`，组织为三层树结构：

```text
session_token（程序退出 / 重复 Ctrl+C）
  └── operation_token（当前代理轮次 / 首次 Ctrl+C）
        └── tool_token（单个工具执行 / 工具超时）
```

取消语义：

- 首次 Ctrl+C 取消 `operation_token`：中止活跃的供应商请求和任何正在运行的工具执行。代理返回空闲状态（准备接收新输入）。
- 第二次 Ctrl+C（或空闲时 Ctrl+C）取消 `session_token`：触发优雅关闭（刷新待写入的会话、恢复终端状态、退出）。
- 工具超时仅取消受影响的 `tool_token`。在并行执行模式下，批次中的其他工具继续执行。在顺序模式下，超时工具之后放弃该批次。
- `Agent::abort()` 以编程方式取消 `operation_token`（等同于首次 Ctrl+C）。
- 丢弃供应商流应当通过 `operation_token` 或 `Request::cancel` 字段取消底层 HTTP 请求。

附加规则：

- 供应商流应当使用有界通道传播背压。
- 工具子进程在取消时必须被终止或刻意分离。
- 子令牌按操作和按工具创建；禁止超出其父作用域的生命周期。

### 11.3 可观测性

`tracing` span 应当覆盖供应商调用、SSE 解析、代理轮次、工具执行、会话追加/加载、压缩和重试调度。密钥和原始供应商载荷必须默认被脱敏。

### 11.4 性能目标

| 指标 | 目标 | 验证方式 |
|---|---:|---|
| 启动到首次提示 | 小于 100 ms | 无网络的 CLI 初始化基准测试 |
| 首 token 显示开销 | 供应商增量加小于 50 ms | 模拟流式供应商 |
| TUI 帧率 | 30 FPS 目标 | 终端快照/性能固定 |
| 空闲内存 | 小于 50 MB | release 冒烟测量 |
| release 二进制大小 | 小于 20 MB 目标 | release 产物检查 |

## 12. 测试策略

| 级别 | 所有者 | 要求的覆盖范围 |
|---|---|---|
| 单元 | 每个 crate | 消息转换、schema 验证、配置解析、路径处理 |
| 供应商合约 | `opi-ai` | SSE 固定、终端事件、错误映射 |
| 模拟循环集成 | `opi-agent` | 预设供应商事件和模拟工具 |
| 会话往返 | `opi-agent` | JSONL 追加/加载、树重建、压缩 |
| 工具测试 | `opi-coding-agent` | 临时目录文件工具、命令超时/取消 |
| CLI E2E | `opi-coding-agent` | `--help`、`--version`、非交互模拟运行、退出码 |
| TUI 快照 | `opi-tui` | 固定大小的确定性渲染输出 |
| JSON 合约 | `opi-coding-agent` | NDJSON schema 和行帧 |
| 实时供应商 | `opi-ai` | 由环境变量门控的被忽略测试 |
| 模糊/属性 | 选定 crate | JSONL 加载器、供应商解析器、工具参数 schema |

第 1 阶段必须包含模拟供应商测试框架。实时供应商测试不够充分，因为它们缓慢、付费、不稳定且依赖凭据。会话往返、JSON 合约和会话加载器模糊/属性测试在相应的第 2 阶段功能实现时变为必需。

当前 CI 门控：

- `cargo fmt --all --check`；
- `cargo clippy --workspace --all-targets`；
- `cargo test --workspace --all-targets`；
- `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`。

## 13. 安全和风险

### 13.1 威胁模型

Opi 以用户权限运行本地工具。主要风险是危险的本地命令、密钥泄露、敏感会话文件和凭据处理不当。

### 13.2 要求

- API 密钥禁止被日志记录或写入会话。
- 会话必须被记录为敏感内容。
- `bash` 必须具有超时、工作目录控制、环境策略、取消行为和可见命令文本。
- 文件工具必须有意地解析路径，并记录路径是在工作区内还是外。
- 路径遍历可以被允许，但工具 allowlist 或扩展钩子应当能够限制它。
- 供应商 HTTP 必须默认使用 TLS。
- 第 1 阶段必须包含对 `write`、`edit` 和 `bash` 的可审计性；变异工具和 shell 执行必须可见、有界，并在非交互模式下可被显式控制。
- Opi 核心不应把永久权限弹窗子系统作为第 3 阶段目标。需要环境特定门禁的用户应使用容器、只读工具 allowlist、钩子、扩展或包。

结构化参数降低了 shell 注入风险，但调用 shell 仍然执行模型提供的命令文本。缓解措施是可见性、可审计性、工具 allowlist、超时、工作目录/环境控制、扩展钩子和谨慎的命令构建。

### 13.3 风险登记

| 风险 | 影响 | 可能性 | 缓解措施 |
|---|---|---:|---|
| 供应商 API 漂移 | 高 | 中 | 固定测试和窄适配器 |
| 仅 Anthropic 的 MVP 令对等预期失望 | 中 | 中 | 发布清晰的阶段范围 |
| 会话 schema 过早稳定 | 高 | 中 | 在合约测试通过前保持 v1 不稳定 |
| Bash 工具执行破坏性操作 | 高 | 高 | 顺序模式、可见命令、超时、工具 allowlist、扩展钩子 |
| 密钥泄露到日志/会话 | 高 | 中 | 脱敏测试和密钥类型 |
| Windows TUI 问题 | 中 | 中 | crossterm 测试和 Windows 冒烟检查 |
| 过早发布到 crates.io | 高 | 中 | 门控首次发布于真实实现、文档和合约测试；如果这些门控未在 0.2.0 达标则推迟 crates.io |
| 扩展范围膨胀核心 | 中 | 高 | 最小核心规则 |
| MCP 变成核心范围蔓延 | 中 | 中 | 扩展 API 稳定后将 MCP 保持为扩展/包示例 |
| 重复的会话堆栈 | 高 | 中 | 显式的 Harness vs CodingHarness 所有权 |

## 14. 发布和版本控制

所有 crate 共享一个工作区版本。

| 版本 | 里程碑 | 发布方式 |
|---|---|---|
| 0.1.0 | 脚手架 | 仅 GitHub Release |
| 0.2.0 | 第 1 阶段 MVP | GitHub Release；crates.io 仅在发布门控通过时（`opi-web-ui` 除外） |
| 0.3.0 | 第 2 阶段持久化和供应商 | GitHub + crates.io |
| 0.4.0 | 第 3 阶段生产加固 | GitHub + crates.io |
| 0.5.0 workspace | 第 4 阶段可扩展性基底 | 可发布 crate 走 GitHub + crates.io；`opi-web-ui` 保持未发布 |

首次 crates.io 发布由质量门控，而非仅由版本号决定。如果所有已发布的 crate 暴露真实的、文档化的行为而非占位公共 API，公共文档构建干净，合约测试覆盖已交付的供应商/工具/运行时边界，且发布技能的检查通过，它可以在 0.2.0 发生。如果这些门控未达标，crates.io 发布应当移至后续的 0.2.x 或 0.3.0 版本，同时 GitHub 二进制发布继续进行。因为二进制 crate 依赖内部库 crate，这些库应按依赖顺序一起发布；`opi-web-ui` 在单独发布决策前保持未发布。所有 0.x 公共 API 除非另有明确文档说明，否则为不稳定。

发布流程应当遵循 `.claude/skills/opi-release/skill.md`：预检、版本升级、变更日志、检查、标签/草案发布、crates.io 发布、最终确认。crates.io 发布是不可逆的（只能 yank）；回滚应使用新提交和标签管理，而非强制推送的公开历史。

发布 CI 构建：

- `opi-linux-x64.tar.gz`；
- `opi-linux-arm64.tar.gz`；
- `opi-darwin-x64.tar.gz`；
- `opi-darwin-arm64.tar.gz`；
- `opi-windows-x64.zip`；
- `opi-windows-arm64.zip`。

`SHA256SUMS.txt` 应当与发布产物一起上传。Windows ARM64 是 Tier 2 目标，如果在 Tier 1 目标通过时特定目标的构建不稳定，应将其视为第 1 阶段 MVP 发布的非阻塞项。

## 15. 实施路线图

### 第 0 阶段 - 脚手架基线

状态：在 0.1.0 中完成。

| 任务 | 状态 |
|---|---|
| 五 crate 工作区 | 完成 |
| 锁步版本控制 | 完成 |
| 占位模块和重导出 | 完成 |
| CI 门控 | 完成 |
| 六平台发布工作流 | 完成 |
| `opi --version` 和 `--help` | 完成 |
| 仅 GitHub Release，crates.io 推迟 | 完成 |

### 第 1 阶段 - MVP 基础

目标：0.2.0。

目标：仅 Anthropic 的编程代理，包含核心循环、六个工具、变异工具和 shell 执行的最小安全边界、基础 TUI、TOML 配置和模拟供应商测试。

| # | 任务 | Crate | 完成定义 |
|---|---|---|---|
| 1.0 | 引入第 1 阶段依赖 | 工作区 | 清单包含所需依赖并使用最小 feature，无未使用依赖警告 |
| 1.1 | 消息和流类型 | `opi-ai` | 需要时可序列化；终端流事件已测试 |
| 1.2 | 替换占位供应商 trait | `opi-ai` | `stream(Request)` 替换 `complete` |
| 1.3 | Anthropic SSE 供应商 | `opi-ai` | 固定测试覆盖文本、工具调用、用量、错误 |
| 1.4 | 供应商注册表 | `opi-ai` | 解析 `anthropic:model` 和能力 |
| 1.5 | 工具 trait 和 schema 验证 | `opi-agent` | 无效参数成为错误工具结果 |
| 1.6 | `agent_loop` | `opi-agent` | 模拟测试覆盖无工具和工具使用轮次 |
| 1.7 | `Agent` 包装器 | `opi-agent` | prompt、continue、abort、subscribe 已测试 |
| 1.8 | 钩子和队列 | `opi-agent` | before/after、should-stop、steering、follow-up 已测试 |
| 1.9 | `read`、`write`、`edit`、`bash` | `opi-coding-agent` | 临时目录测试覆盖成功、失败、超时/取消、工作目录/环境报告和最小确认策略 |
| 1.10 | `glob`、`grep` | `opi-coding-agent` | 测试覆盖忽略目录和正则错误 |
| 1.11 | 系统提示构建 | `opi-coding-agent` | 提示包含工具定义和系统层 |
| 1.12 | TUI 外壳 | `opi-tui` | 固定大小渲染快照 |
| 1.13 | markdown/代码渲染 | `opi-tui` | markdown 和围栏代码快照 |
| 1.14 | 交互式 CLI 连接 | `opi-coding-agent` | 可对模拟供应商运行 |
| 1.15 | 非交互模式 | `opi-coding-agent` | stdout/stderr/退出码测试 |
| 1.16 | TOML 配置加载 | `opi-coding-agent` | 缺失默认值和格式错误已测试 |
| 1.17 | 集成测试框架 | 跨 crate | 模拟供应商 E2E 在 CI 中运行 |

退出标准：`opi` 接受提示，流式传输 Claude 输出，在第 1 阶段安全边界后执行 `read/write/edit/bash/glob/grep`，在 TUI 中显示结果，支持带有显式高风险工具策略的非交互模式，并通过模拟供应商 CI 测试。会话、压缩、JSON 模式、MCP、插件、Web UI、丰富 diff 视图和语法高亮代码块不是第 1 阶段退出标准。

### 第 2 阶段 - 多供应商和持久化

目标：0.3.0。

| # | 任务 | Crate |
|---|---|---|
| 2.1 | OpenAI 兼容聊天供应商 | `opi-ai` |
| 2.2 | OpenRouter 供应商配置 | `opi-ai` |
| 2.3 | OpenAI Responses 供应商 | `opi-ai` |
| 2.4 | Google Gemini 供应商 | `opi-ai` |
| 2.5 | Mistral 供应商 | `opi-ai` |
| 2.6 | opi session v1 JSONL 存储和合约测试 | `opi-agent` |
| 2.7 | 会话列表/恢复/删除 | `opi-coding-agent` |
| 2.8 | 压缩 | `opi-agent` / `opi-coding-agent` |
| 2.9 | 思考/推理支持 | `opi-ai` |
| 2.10 | 用量和成本追踪 | `opi-ai` |
| 2.11 | diff 视图 | `opi-tui` |
| 2.12 | 主题 | `opi-tui` |
| 2.13 | 键绑定 | `opi-tui` |
| 2.14 | `--json` NDJSON 模式 | `opi-coding-agent` |
| 2.15 | 重试/退避/速率限制 | `opi-ai` |
| 2.16 | 会话合约测试 | `opi-agent` |

退出标准：会话在重启后存活，多个供应商通过合约固定测试，长对话在溢出前压缩，JSON 模式有 schema 测试。

### 第 3 阶段 - 生产加固

状态：完成于 0.4.0。

| # | 任务 | Crate |
|---|---|---|
| 3.1 | AWS Bedrock 供应商 | `opi-ai` |
| 3.2 | Azure OpenAI 供应商 | `opi-ai` |
| 3.3 | Google Vertex 供应商 | `opi-ai` |
| 3.4 | 图像输入 | `opi-ai` |
| 3.5 | 图像工具结果 | `opi-agent` |
| 3.6 | 终端图像渲染 | `opi-tui` |
| 3.7 | `AGENTS.md` / `CLAUDE.md` 上下文加载 | `opi-coding-agent` |
| 3.8 | pi 风格工具选择和安全钩子 | `opi-coding-agent` |
| 3.9 | `find` / `ls` 内置工具对等 | `opi-coding-agent` |
| 3.10 | shell 补全 | `opi-coding-agent` |
| 3.11 | 模糊模型/会话选择器 | `opi-tui` |
| 3.12 | 网络代理支持 | `opi-ai` |
| 3.13 | 连接池调优 | `opi-ai` |

跨平台二进制发布未在此列出，因为发布 CI 已是第 0 阶段的一部分。

退出标准：企业供应商可用，图像和终端图像流程可用，项目上下文加载匹配 pi，高风险工具可通过 pi 风格工具选择/钩子保持可见且可控，发布产物可重复，交互式 UX 对日常使用足够健壮。

### 第 4 阶段 - 可扩展性

状态：当前 `0.5.0` workspace 中可扩展性基底已实现。

| # | 任务 | Crate |
|---|---|---|
| 4.1 | 带严格 framing、关联响应、异步事件、extension commands，以及 session/model/thinking/compaction 命令的 RPC JSONL 模式 | `opi-coding-agent` |
| 4.2 | 基于同一事件和命令模型的 SDK 嵌入表面 | `opi-coding-agent` / `opi-agent` |
| 4.3 | 处理 `opi-agent::Transport`：实现、隐藏为不稳定 API，或在稳定公共 API 声明前移除 | `opi-agent` |
| 4.4 | extension trait、生命周期 hooks、自定义工具、自定义命令、自定义消息和 extension state | `opi-agent` / `opi-coding-agent` |
| 4.5 | 项目和用户资源的 extension/resource 加载策略 | `opi-coding-agent` |
| 4.6 | 通过 SDK 或 extensions 注册自定义 provider/model | `opi-ai` / `opi-coding-agent` |
| 4.7 | 渐进式发现的 skills、prompt fragments、themes 和 packages | `opi-coding-agent` |
| 4.8 | extension/package 示例：permission gate、protected paths、sub-agent、plan mode、todo、MCP adapter | examples / package template |
| 4.9 | 会话分支 UI | `opi-agent` / `opi-tui` |
| 4.10 | streaming proxy | `opi-agent` 或新 crate |
| 4.11 | 消费 RPC/SDK events 的 Web UI 实现 | `opi-web-ui` |

退出标准：第三方可以通过 RPC、SDK、extension API、发现到的资源、skills、prompt fragments、themes、packages 和自定义 provider/model 注册组合并扩展 opi，而无需修补核心 crate。MCP、子代理、plan mode、todos 和 permission gates 应作为 extensions 或 packages 演示，而不是核心功能。`Transport` 公共表面已经移除；除非有真实实现，否则不得重新作为稳定公共声明引入。

### 第五阶段 - 产品化 Extension/Package 生态系统

状态：当前 `0.5.0` workspace 中已实现。

第五阶段添加了 package 管理和可执行 adapter 托管，使外部 package 可以通过子进程 adapter 提供工具、命令、hooks 和事件，而无需修补核心 crate。

| # | 任务 | Crate |
|---|---|---|
| 5.1 | Package 存储和来源模型 | `opi-coding-agent` |
| 5.2 | Package CLI MVP | `opi-coding-agent` |
| 5.3 | 带有 adapter 和 opi_version 的 Manifest V2 兼容性 | `opi-coding-agent` |
| 5.4 | Adapter JSONL 协议类型 | `opi-coding-agent` |
| 5.5 | Adapter 进程托管 | `opi-coding-agent` |
| 5.6 | Adapter 运行时桥接到 Extension trait | `opi-coding-agent` / `opi-agent` |
| 5.7 | Harness 和启动集成 | `opi-coding-agent` / `opi-agent` |
| 5.8 | 可运行的示例 adapter package | examples / `opi-coding-agent` |
| 5.9 | 文档、对齐和守卫 | workspace |

退出标准：`opi package add/remove/list/doctor` 可用；带有 `[adapter]` 段落的 package 以子进程方式启动并使用 `opi-extension-jsonl-v1`；adapter 工具、命令、hooks、状态和取消桥接到现有 extension API；示例 package（todo、permission-gate、protected-paths）演练完整流水线；文档如实描述，守卫测试拒绝关于 npm、marketplace、热重载、provider 流式 adapter、自定义 TUI adapter 或 package 权限执行的声明。

## 16. 决策日志

| # | 决策 | 选择 | 原因 |
|---|---|---|---|
| ADR-001 | 工作区形状 | 五个 crate 映射 pi 包 | 保留概念边界 |
| ADR-002 | 版本控制 | 锁步工作区版本 | 简化兼容性和发布顺序 |
| ADR-003 | 无共享类型 crate | 类型归属语义拥有者 | 避免枢纽依赖 |
| ADR-004 | pi 兼容性 | 语义对等，非 API/文件对等 | Rust 原生实现 |
| ADR-005 | MVP 供应商 | 仅 Anthropic | 首次发布保持可测试 |
| ADR-006 | 供应商 SDK | 直接 HTTP 适配器 | 流控制和更少的不稳定依赖 |
| ADR-007 | 流协议 | start/delta/end/done/error | 与 pi 对齐并支持 UI 部分状态 |
| ADR-008 | 代理分层 | loop -> Agent -> Harness | 可测试性和关注点分离 |
| ADR-009 | 代理 vs LLM 消息 | 保持分离 | 自定义消息不应泄露到供应商 |
| ADR-010 | 工具边界 | 类型化参数加生成的 JSON Schema | 动态 LLM 边界，类型化内部，运行时验证 |
| ADR-011 | 工具执行 | 默认并行带顺序覆盖 | 匹配 pi 并避免竞态 |
| ADR-012 | 会话格式 | opi 树状 JSONL | 分支语义而不锁定 TS 格式 |
| ADR-013 | 配置格式 | TOML | 注释支持和 Rust 生态系统契合 |
| ADR-014 | TUI | ratatui/crossterm | 跨平台 Rust 终端技术栈 |
| ADR-015 | 扩展策略 | RPC/SDK 和扩展 API 先于协议适配器 | 匹配 pi 的组合模型；MCP 是扩展/包候选，不是第 3 阶段核心功能 |
| ADR-016 | Web UI | 核心稳定前不发布 | 避免过早的 WASM 承诺 |
| ADR-017 | Transport 存根 | 已从公共 API 移除 | 避免未文档化的公共表面 |
| ADR-018 | crates.io 时机 | 质量门控的首次发布 | 仅在占位 API 被隐藏或替换且发布门控通过后发布 |
| ADR-019 | 工具安全 | allowlist、可见性和钩子优先于核心权限配置文件 | pi 明确避免内置权限弹窗；环境特定门禁属于扩展/包或外部沙箱 |
| ADR-020 | 上下文文件 | `AGENTS.md` / `CLAUDE.md` 先于 `OPI.md` | 保留 pi 行为和生态约定 |

## 17. 非功能性需求

Tier 1 目标：

- `x86_64-unknown-linux-gnu`；
- `aarch64-unknown-linux-gnu`；
- `x86_64-apple-darwin`；
- `aarch64-apple-darwin`；
- `x86_64-pc-windows-msvc`。

Tier 2 目标：`aarch64-pc-windows-msvc`。

Rustls 优于 OpenSSL 以构建可移植二进制文件。

可访问性要求：

- 尊重 `NO_COLOR`；
- 在非交互和 JSON 模式下暴露关键状态；
- 不仅依赖颜色来表示错误、工具或 diff；
- 提供适合脚本的退出码。

可维护性要求：

- 实现后为公共 API 编写带示例的文档；
- 在阶段任务标记完成前包含测试；
- 在不可避免时在变更日志或 issue 中跟踪规范/代码漂移；
- 按职责拆分大模块。

## 18. 未来考虑

架构不应排除 MCP 工具、远程工具执行、流式代理服务、编辑器集成、pi 会话迁移、插件运行时或 Web 聊天界面。这些不是第 1-3 阶段核心需求，通常应通过 RPC、SDK、扩展、包或后续审查过的插件运行时进入。

## 19. 术语表

| 术语 | 定义 |
|---|---|
| 供应商（Provider） | LLM 后端，如 Anthropic、OpenAI、Gemini 或 Bedrock |
| API 类型（API kind） | 线路协议家族，如 Anthropic Messages 或 OpenAI Chat Completions |
| 模型（Model） | 具有能力和限制的供应商模型 |
| 代理循环（Agent loop） | 发送上下文、接收助手输出、执行工具并重复的纯循环 |
| 代理（Agent） | 围绕循环的有状态包装器 |
| Harness | 用于会话、压缩和应用钩子的组合层 |
| CodingHarness | 编程代理特定的 harness |
| 代理消息（AgentMessage） | 应用层消息，可能包含自定义/仅会话数据 |
| 消息（Message） | 面向供应商的用户/助手/工具结果消息 |
| 流事件（Stream event） | 供应商级助手增量或终端事件 |
| 代理事件（Agent event） | 运行时生命周期/消息/工具事件 |
| 会话事件（Session event） | 队列/压缩/重试/会话事件 |
| 会话条目（Session entry） | 持久化的 JSONL 树记录 |
| 引导（Steering） | 在代理运行时注入的消息，在下一次供应商调用前 |
| 后续（Follow-up） | 排队直到代理即将停止的消息 |
| 压缩（Compaction） | 在保留近期状态的同时总结较旧的上下文 |
| 工具（Tool） | 模型可调用的、具有 JSON Schema 参数的能力 |

## 20. 参考资料

- [pi 源代码](https://github.com/earendil-works/pi)
- `.repo/pi-0.75.3/CONTRIBUTING.md`
- `.repo/pi-0.75.3/packages/agent/README.md`
- `.repo/pi-0.75.3/packages/agent/src/types.ts`
- `.repo/pi-0.75.3/packages/ai/src/types.ts`
- `.repo/pi-0.75.3/packages/coding-agent/src/core/session-manager.ts`
- `.repo/pi-0.75.3/packages/coding-agent/docs/json.md`
- `.repo/pi-0.75.3/packages/coding-agent/docs/rpc.md`
- `Cargo.toml`
- `CHANGELOG.md`
- `.github/workflows/ci.yml`
- `.github/workflows/release.yml`
- `.claude/skills/opi-release/skill.md`
- [Anthropic Messages API](https://docs.anthropic.com/en/api/messages)
- [ratatui](https://ratatui.rs/)
- [MCP 规范](https://modelcontextprotocol.io/)
