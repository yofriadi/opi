# opi

[![CI](https://github.com/OdradekAI/opi/actions/workflows/ci.yml/badge.svg)](https://github.com/OdradekAI/opi/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

> Rust 编写的 AI Agent 工具包，将 [earendil-works/pi](https://github.com/earendil-works/pi) 的思路重新实现为终端优先的编程 Agent 与可复用 Agent crate。

[English](README.md) | [更新日志](CHANGELOG.md) | [技术规范](docs/opi-spec.zh.md)

## 当前状态

当前 workspace 版本：`0.5.0`。

`opi` 已经是可用的终端编程 Agent。它包含交互式 ratatui TUI、文本与 NDJSON 非交互模式、RPC JSONL 模式、8 个内置工具、图片附件、模型/会话/分支选择器、shell 补全生成、分层 TOML 配置、按 Provider 配置代理、多 Provider 流式接入、JSONL 会话持久化、上下文压缩、retry/backoff、可配置按键与主题、token 用量累计，以及尽力而为的费用摘要。

可扩展性表面已经存在且仍是不稳定 0.x API：共享 SDK/RPC 命令类型、面向嵌入方的 extension hooks/tools/state、按层发现 extensions、packages、skills、prompt fragments 和 themes 资源、自定义 provider/model 注册，以及 streaming proxy。`opi-web-ui` 仍是 `publish = false`；它不是独立浏览器应用，但已经提供可复用的 RPC/SDK 事件解析、对话状态、组件模型和 HTML 渲染。

## 与 pi 的关系

`opi` 借鉴 pi 的思想和设计边界，但不与 pi API 兼容，也不默认读取 pi 的配置文件或会话文件。

| 领域 | pi 的方向 | opi 的处理 |
|------|-----------|------------|
| 产品表面 | 最小终端编程 harness | 终端优先的 Rust 编程 Agent 与可复用 Rust crate |
| 核心编程工具 | 默认 `read`、`write`、`edit`、`bash` | 交互模式保留同一组默认工具 |
| 只读导航 | `read`、`grep`、`find`、`ls` | 保留同一组核心只读工具；`glob` 是额外便利能力，核心流程不应依赖它 |
| 扩展能力 | Extensions、skills、prompt templates、themes、packages | RPC/SDK、extension API、资源发现、skills、prompt fragments、themes、packages 和自定义 provider/model 注册已经作为不稳定 0.x 表面实现 |
| 工作流重功能 | MCP、子 Agent、plan mode、todos、permission gates 不进入核心 | 作为 extension/package 示例，而不是内置核心策略 |
| 配置与会话 | `.pi` JSON 设置与 pi 会话文件 | TOML 配置与 opi JSONL 会话 |
| Web UI | pi 的包集合中已有实现 | 未发布的可复用组件/状态/渲染 crate；还没有独立浏览器应用 |

## 工作区

Cargo workspace 采用锁步版本。所有 crate 都从 `[workspace.package]` 继承 `version`、`edition`、`license`、`repository` 和 `authors`。

| Crate | 是否发布 | 说明 |
|-------|----------|------|
| [`opi-ai`](crates/opi-ai) | 是 | Provider 无关 LLM API、流式事件、图片内容、注册表、重试、HTTP 连接池/代理、用量与费用工具 |
| [`opi-agent`](crates/opi-agent) | 是 | Agent 主循环、工具执行、hooks、事件、队列、会话、压缩、SDK 类型、extension API 和 streaming proxy 原语 |
| [`opi-tui`](crates/opi-tui) | 是 | Ratatui 组件、对话渲染、diff 视图、选择/分支列表、终端图片、主题、按键绑定 |
| [`opi-coding-agent`](crates/opi-coding-agent) | 是 | `opi` 二进制与可嵌入的编程 harness |
| [`opi-web-ui`](crates/opi-web-ui) | 否（`publish = false`） | RPC/SDK 事件解析、对话状态、组件模型和 HTML 渲染辅助 |

内部依赖关系：

```text
opi-ai（无内部依赖）
opi-tui（无内部依赖）
opi-agent -> opi-ai
opi-web-ui（无内部依赖，publish = false）
opi-coding-agent -> opi-ai + opi-agent + opi-tui -> opi binary
```

## 安装

可执行文件名是 `opi`，由 `opi-coding-agent` crate 产出。

```sh
cargo install opi-coding-agent
opi --version
```

Linux、macOS 和 Windows 的 x64/arm64 预编译二进制附在 [GitHub Releases](https://github.com/OdradekAI/opi/releases)。

## 快速开始

先设置要使用的 Provider 凭据：

```sh
export ANTHROPIC_API_KEY=sk-ant-...
# 或 OPENAI_API_KEY、OPENROUTER_API_KEY、MISTRAL_API_KEY、GEMINI_API_KEY
# 或 Bedrock 的 AWS 凭据、AZURE_OPENAI_API_KEY、VERTEX_ACCESS_TOKEN
```

启动交互式 TUI：

```sh
opi
```

运行单次提示词，并把助手文本输出到 stdout：

```sh
opi "列出这个 workspace 中的 Rust crate。"
```

为自动化输出 NDJSON 事件：

```sh
opi --json "总结最新会话状态。"
```

给初始提示词附加图片：

```sh
opi --image screenshot.png "审查这个 UI。"
opi --image before.png --image after.png "对比这两张图片。"
```

用 `provider:model` 语法选择模型：

```sh
opi -m anthropic:claude-sonnet-4-5-20250514 "解释 crates/opi-agent/src/lib.rs"
opi -m openai:gpt-4o "审查公共 API 形态。"
opi -m openai-responses:gpt-4o-mini "找出文档缺口。"
opi -m openrouter:openai/gpt-4o-mini "列出 TODO 注释。"
opi -m mistral:codestral-latest "解释工具模块。"
opi -m gemini:gemini-2.5-flash "总结 README 文件。"
opi -m bedrock:anthropic.claude-sonnet-4-20250514-v2:0 "总结这个仓库。"
opi -m azure:my-deployment "使用我的 Azure OpenAI deployment。"
opi -m vertex:gemini-2.5-flash "使用 Vertex AI。"
```

## 支持的 Provider

Provider 支持在 `opi-ai` 中实现，并已接入 `opi-coding-agent`。

| 模型前缀 | 后端 | 默认凭据 |
|----------|------|----------|
| `anthropic:` | Anthropic Messages SSE | `ANTHROPIC_API_KEY` |
| `openai:` | OpenAI Chat Completions SSE | `OPENAI_API_KEY` |
| `openai-responses:` | OpenAI Responses SSE | `OPENAI_API_KEY` |
| `openrouter:` | OpenAI-compatible OpenRouter profile | `OPENROUTER_API_KEY` |
| `mistral:` | OpenAI-compatible Mistral profile | `MISTRAL_API_KEY` |
| `gemini:` | Gemini `streamGenerateContent` SSE | `GEMINI_API_KEY` |
| `bedrock:` | AWS Bedrock Converse streaming，使用 SigV4 | AWS 环境变量或共享 AWS config/credentials |
| `azure:` | Azure OpenAI Chat Completions deployment | `AZURE_OPENAI_API_KEY` 加配置中的 endpoint |
| `vertex:` | Google Vertex AI Gemini streaming | `VERTEX_ACCESS_TOKEN` 加配置中的 project/location |
| 已配置 profile | OpenAI-compatible Chat Completions profile | profile 自己的 `api_key_env` |

使用 `opi --list-models` 可列出已配置 Provider 暴露的模型；加 `--json` 可输出机器可读格式。

## 内置工具

工具由 `opi-coding-agent` 实现，并通过 `opi-agent::Tool` trait 暴露。

| 工具 | 参数 | 执行模式 | 是否修改 |
|------|------|----------|----------|
| `read` | `path`，可选 `offset`、`limit` | 并行 | 否 |
| `ls` | `path`，可选 `max_entries`、`max_depth` | 并行 | 否 |
| `glob` | `pattern` | 并行 | 否 |
| `find` | `pattern`，可选 `path` | 并行 | 否 |
| `grep` | `pattern` | 并行 | 否 |
| `write` | `path`、`content` | 串行 | 是 |
| `edit` | `path`、`old_string`、`new_string` | 串行 | 是 |
| `bash` | `command`，可选 `timeout_secs` | 串行 | 是 |

路径策略会按运行模式区分。写入、编辑和非交互文件工具默认限制在 harness 的 workspace 根目录内。交互模式的 `read` 可以解析绝对路径和 workspace 外路径用于检查，文件工具 details 会记录 `workspace_root`、`resolved_path` 和 `inside_workspace`。非交互/RPC 运行中，修改性工具需要 `--allow-mutating`，或配置 `defaults.allow_mutating_tools = true`；因此无人值守和边缘设备运行默认保持只读，除非调用方显式允许写入或 shell 执行。

工具选择参数：

```sh
opi --tools read,grep "只读检查代码。"
opi --no-tools "只根据对话上下文回答。"
opi --no-builtin-tools "不加载内置工具运行。"
```

## 配置

配置会合并用户配置、项目配置和显式配置文件。模型优先级如下：

1. `--model`
2. 未传入 `--config` 时的 `OPI_MODEL`
3. `--config <FILE>` 中的 `model`
4. `<CWD>/.opi/config.toml`
5. 用户配置（Windows：`%APPDATA%\opi\config.toml`；Unix：`~/.config/opi/config.toml`）
6. 内置默认值

示例：

```toml
[defaults]
model = "anthropic:claude-sonnet-4"
max_iterations = 50
tool_timeout_ms = 30000
max_image_bytes = 20971520
theme = "default"
allow_mutating_tools = false

[thinking]
enabled = true
budget_tokens = 10000

[retry]
max_attempts = 3
initial_delay_ms = 1000
max_delay_ms = 60000

[compaction]
enabled = true
threshold_tokens = 100000

[keybindings]
submit = "enter"
abort = "escape"
new_line = "alt+enter"

[providers.anthropic]
api_key_env = "ANTHROPIC_API_KEY"
# base_url = "https://api.anthropic.com"

[providers.openai]
api_key_env = "OPENAI_API_KEY"
# base_url = "https://api.openai.com"

[providers.openai_responses]
api_key_env = "OPENAI_API_KEY"
# base_url = "https://api.openai.com"

[providers.openrouter]
api_key_env = "OPENROUTER_API_KEY"
# base_url = "https://openrouter.ai/api"
# referer = "https://example.com"

[providers.mistral]
api_key_env = "MISTRAL_API_KEY"
# base_url = "https://api.mistral.ai"

[providers.gemini]
api_key_env = "GEMINI_API_KEY"
# base_url = "https://generativelanguage.googleapis.com"

[providers.bedrock]
region = "us-east-1"
# profile = "default"
# base_url = "https://bedrock-runtime.us-east-1.amazonaws.com"
# secret_access_key_env = "AWS_SECRET_ACCESS_KEY"
# session_token_env = "AWS_SESSION_TOKEN"

[providers.azure]
api_key_env = "AZURE_OPENAI_API_KEY"
endpoint = "https://my-resource.openai.azure.com"
api_version = "2024-06-01"
deployments = ["my-deployment"]

[providers.vertex]
access_token_env = "VERTEX_ACCESS_TOKEN"
project = "my-gcp-project"
location = "us-central1"
models = ["gemini-2.5-flash", "gemini-2.5-pro"]

[providers.openai_compatible.localai]
api_key_env = "LOCALAI_API_KEY"
base_url = "https://localai.example.com"
system_role_override = "developer"
max_tokens_field = "max_completion_tokens"
tool_result_name_field = true
usage_in_stream = true

[[providers.openai_compatible.localai.models]]
id = "local-model"
display_name = "Local Model"
context_window = 128000
max_output_tokens = 4096
supports_images = true
supports_streaming = true
supports_thinking = false

[providers.openai.proxy]
url = "http://proxy.example.com:8080"
no_proxy = "localhost,127.0.0.1"

[extensions]
paths = ["vendor/my-extension"]

[packages]
paths = ["vendor/my-package"]
```

如果没有为某个 Provider 配置代理，`opi` 会回退到标准的 `HTTP_PROXY`、`HTTPS_PROXY` 和 `NO_PROXY` 环境变量。

## 交互模式

没有提示词参数时，`opi` 会启动 ratatui TUI。输入框中可用的命令：

| 命令 | 作用 |
|------|------|
| `/model` | 打开当前 Provider 的模型选择器 |
| `/session` | 打开会话选择器并恢复已有会话 |
| `/branch` | 打开当前会话的分支选择器 |
| `/tree` | 打开当前会话的会话树选择器 |
| `/fork` | 把当前活跃分支 fork 成新的父子会话 |
| `/clone` | 把当前活跃分支 clone 成新的父子会话 |
| `/image <path>` | 为下一条提示词排队一张图片 |
| `exit` 或 `quit` | 退出 TUI |

默认按键是 `enter` 提交、`escape` 中止/退出、`alt+enter` 换行。可在 `[keybindings]` 中修改。

## 会话

编程 harness 会自动把会话写成 JSONL 文件。

默认位置：

- Windows：`%LOCALAPPDATA%\opi\sessions\`
- Unix：`~/.local/share/opi/sessions/`

可以用 `OPI_SESSIONS_DIR` 覆盖。

```sh
opi --list-sessions
opi --resume <session-id> "从这个会话继续。"
opi --fork <session-id> "从这个会话的 fork 继续。"
opi --delete-session <session-id>
```

会话文件保存 header，以及 message、compaction、leaf 条目。Resume 会重建活跃分支并保留压缩摘要语义。Fork 命令会创建新的 JSONL 会话，并让新 header 的 `parent_session` 指向源会话；源文件保持 append-only，不会被改写。`--json` 会输出 session 事件、retry 事件、compaction 事件、thinking-level 事件，以及带 token 总量和可选费用总量的最终 session summary。

## 上下文文件

编程 harness 会从 workspace 目录向上查找 `AGENTS.md` 和 `CLAUDE.md`，直到 git root，然后再查找用户配置目录。超过 128 KiB 的文件和空文件会被忽略。`OPI.md` 有意不会加载。

## RPC、SDK 与扩展

`opi --rpc` 会通过 stdin/stdout 启动一个持久 JSONL 命令/事件会话。启动时会输出 `schema_version = 2` 的 `rpc_ready` 头；命令包括 `prompt`、`continue`、`abort`、`steer`、`follow_up`、`set_model`、`set_thinking_level`、`compact`、`session_info`、`extension_command` 和 `quit`。响应可用可选的 `id` 关联；已接受的 prompt 输出会作为异步 agent 事件流式返回。

共享 SDK 类型位于 `opi_agent::sdk`。`opi-agent` 的 extension API 面向嵌入方支持生命周期 hook、自定义工具、自定义命令、自定义 agent message/state，以及自定义 provider/model 注册。CLI 会从用户、项目、package 和显式路径发现已配置的资源元数据，并把它暴露到 prompt/RPC metadata 中。它不会从磁盘动态加载任意 Rust 代码。

Package 可以通过扁平 `package.toml` manifest 组合 extensions、skills、prompt fragments 和 themes。`opi package` CLI 管理本地和 git 来源：

```sh
opi package add ./vendor/todo          # 本地目录
opi package add git:github.com/user/pkg@v1  # git 来源
opi package list                       # 列出已安装的 package
opi package doctor                     # 诊断 package 问题
opi package remove todo                # 卸载 package
```

带有 `[adapter]` 声明的 package 会以子进程 adapter 的方式运行，使用 `opi-extension-jsonl-v1` 协议。Adapter 进程通过 JSONL stdin/stdout 通信，可以通过现有 extension API 暴露自定义工具、命令、hooks、事件观察者、会话作用域状态和取消桥接——无需 Node、npm 或在线 provider。`process-jsonl` 是第五阶段 MVP 中唯一支持的 adapter 类型。

Skills 与 prompt fragments 采用渐进式披露：先发现元数据，只有需要时才加载正文。Themes 可以从 `theme.toml` 资源发现，并在回退到内置 `default` 和 `monokai` 前优先解析。同一发现层内的重复资源名会报错；高优先级层会覆盖低优先级层。

## 从源码构建

Workspace 使用 Rust edition 2024，需要 Rust 1.85 或更新版本。

```sh
cargo build
cargo build --release

cargo run -p opi-coding-agent -- --help

cargo test --workspace --all-targets
cargo test -p opi-ai

cargo fmt --check --all
cargo clippy --workspace --all-targets -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
```

## 架构

`opi` 启动时会选择运行模式：

- 会话、模型列表和补全生成命令会尽早处理并退出。
- 非交互模式由提示词参数、`--non-interactive` 或 `--json` 触发；它构建 Provider 并运行 `NonInteractiveRunner`。
- 交互模式是没有提示词参数时的默认模式；它构建带交互 hooks 的 `CodingHarness` 并启动 ratatui TUI。

交互和非交互模式共用同一个 Agent 主循环：

```text
transform_context
  -> convert_to_llm
  -> provider.stream(Request)
  -> 累积 assistant stream events
  -> 检测工具调用
  -> 校验 JSON Schema 参数
  -> before_tool_call
  -> 并行或串行批量执行工具
  -> after_tool_call
  -> should_stop_after_turn
  -> prepare_next_turn
  -> 轮询 steering/follow-up 队列
```

关键抽象：

- `opi_ai::Provider`：流式 LLM 后端接口。
- `opi_ai::AssistantStreamEvent`：Provider 无关的流式事件模型，覆盖文本、thinking、工具调用、完成与错误。
- `opi_agent::Tool`：基于 JSON Schema 的工具契约，支持并行或串行执行模式。
- `opi_agent::AgentHooks`：围绕消息转换、工具策略、工具结果、停止条件、下一轮准备的生命周期 hooks。
- `opi_agent::SessionWriter` / `SessionReader`：append-only JSONL 会话存储，带崩溃恢复。
- `opi_agent::CompactionEngine`：支持阈值、手动、溢出触发的上下文压缩。
- `opi_agent::sdk`：共享的 SDK/RPC 命令与事件类型，用于程序化嵌入。

## 未内置到核心

- 生产级子 Agent、permission gate、plan/todo 和 MCP 工作流。仓库包含 package/example 脚手架，但它们不是内置核心产品工作流。
- 在交互式 slash command 中运行时展开 prompt fragments。
- 从任意 extension 路径动态加载 Rust 插件。
- OAuth 或订阅登录流程。
- 独立的浏览器托管 Web 应用。

## 发布

发布由 `opi-release` skill（`.claude/skills/opi-release/skill.md`）统一编排到 GitHub Releases 与 crates.io。

- 所有可发布 crate 使用同一个版本号。
- 发布顺序遵循内部依赖关系。
- 推送 `v*` tag 会触发 `.github/workflows/release.yml`。
- 回滚使用 `git revert` 与删除 tag，不改写历史。

## 参与贡献

- 使用 Conventional Commits。
- crate 元数据保持从 `[workspace.package]` 继承。
- 按变更范围运行 `cargo fmt --check --all`、`cargo clippy --workspace --all-targets -- -D warnings`、测试和文档检查。
- 仓库的人类与 Agent 协作规则见 `AGENTS.md`。

欢迎在 <https://github.com/OdradekAI/opi/issues> 提交 Issue / PR。

## 许可证

MIT (c) OdradekAI。详见 [LICENSE](LICENSE)。
