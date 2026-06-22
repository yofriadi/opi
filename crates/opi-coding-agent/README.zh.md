# opi-coding-agent

[![Crates.io](https://img.shields.io/crates/v/opi-coding-agent.svg)](https://crates.io/crates/opi-coding-agent)
[![Docs.rs](https://docs.rs/opi-coding-agent/badge.svg)](https://docs.rs/opi-coding-agent)

> `opi` 二进制与可嵌入编程 harness。

[English](README.md) | [opi workspace](../../README.zh.md)

## 当前状态

当前 crate 版本是 `0.5.3`，继承自 workspace 包版本。

本 crate 把 `opi-ai`、`opi-agent` 和 `opi-tui` 连接成终端编程 Agent。它提供：

- `opi` CLI 二进制；
- 交互式 ratatui TUI 模式；
- 单次文本模式和 `--json` NDJSON 模式；
- `--rpc` JSONL 命令/事件模式；
- 模型、会话、分支和会话树选择器；
- 通过 `--image` 和 `/image` 附加图片；
- 会话 list/resume/fork/delete 命令；
- 8 个内置工具；
- 配置、上下文文件加载、会话持久化、压缩、重试、用量、费用摘要、package/资源发现、
  诊断和可选 trace。

本 crate 也可以通过 `CodingHarness` 作为库使用，但多数用户应先从 CLI 开始。

## 安装

```sh
cargo install opi-coding-agent
opi --version
```

预编译二进制附在 [GitHub Releases](https://github.com/OdradekAI/opi/releases)。

## 快速开始

```sh
export ANTHROPIC_API_KEY=sk-ant-...

# 交互式 TUI
opi

# 单次提示词，助手文本输出到 stdout
opi "找出这个仓库中的 TODO 注释。"

# NDJSON 事件流
opi --json "总结这个 workspace。"

# 指定 provider/model
opi -m openai:gpt-4o "解释 crates/opi-coding-agent/src/main.rs"

# 给第一条提示词附加图片
opi --image screenshot.png "审查这张截图。"

# 在非交互自动化中允许 write/edit/bash
opi --allow-mutating "更新 README。"
```

## CLI 命令与参数

运行 `opi --help` 可查看当前精确表面。重要命令和参数：

| 命令 / 参数 | 作用 |
|-------------|------|
| `[PROMPT]...` | 非空位置参数会进入单次文本模式。 |
| `-m, --model <SPEC>` | 模型 spec，例如 `anthropic:claude-sonnet-4-5-20250514`。 |
| `-c, --config <FILE>` | 显式 TOML 配置文件；必须存在。 |
| `-s, --system <FILE>` | 把用户系统提示词文件追加到内置编程提示词。 |
| `--non-interactive` | 强制单次文本模式；仍然需要提示词文本。 |
| `--json` | 向 stdout 输出 NDJSON session/agent 事件。 |
| `--rpc` | 通过 stdin/stdout 启动双向 JSONL 命令/事件模式。 |
| `--allow-mutating` | 在交互模式之外允许 `write`、`edit` 和 `bash`。 |
| `--tools <TOOLS>` | 逗号分隔的内置工具 allowlist。 |
| `--no-tools` | 禁用所有工具。 |
| `--no-builtin-tools` | 禁用内置工具，同时保留 extension/custom 工具可用性。 |
| `--image <PATH>` | 给初始提示词附加一张图片；可重复。 |
| `--list-models` | 列出已配置 Provider 暴露的模型并退出。 |
| `--list-sessions` | 列出已保存会话并退出。 |
| `--resume <ID>` | 恢复已保存会话。 |
| `--fork <ID>` | fork 已保存会话为新会话。 |
| `--delete-session <ID>` | 删除已保存会话并退出。 |
| `--generate-completion <SHELL>` | 为 `bash`、`zsh`、`fish`、`powershell` 或 `elvish` 生成补全。 |
| `--trace <PATH>` | 为非交互/JSON 运行写入可选的、已脱敏本地 trace envelope。 |
| `doctor [--json] [--scope ...]` | 本地、无网络健康检查。 |
| `package <add|remove|list|doctor>` | 管理本地/git extension package。 |

## Provider

| 前缀 | 后端 | 默认凭据/配置 |
|------|------|---------------|
| `anthropic:` | `AnthropicProvider` | `ANTHROPIC_API_KEY` |
| `openai:` | `OpenAiChatProvider` | `OPENAI_API_KEY` |
| `openai-responses:` | `OpenAiResponsesProvider` | `OPENAI_API_KEY` |
| `openrouter:` | OpenRouter profile | `OPENROUTER_API_KEY` |
| `mistral:` | Mistral profile | `MISTRAL_API_KEY` |
| `gemini:` | `GeminiProvider` | `GEMINI_API_KEY` |
| `bedrock:` | `BedrockProvider` | AWS 环境变量或共享 AWS profile/config |
| `azure:` | `AzureOpenAIProvider` | `AZURE_OPENAI_API_KEY`；endpoint/deployments 在配置中 |
| `vertex:` | `VertexProvider` | `VERTEX_ACCESS_TOKEN`；project/location 在配置中 |
| 已配置 profile | OpenAI-compatible profile | profile 自己的 `api_key_env`、`base_url` 和模型列表 |

Provider 凭据环境变量名、base URL、模型列表和代理都可以在配置中覆盖。

## 内置工具

工具位于 `src/tool/`。

| 工具 | 参数 | 说明 |
|------|------|------|
| `read` | `path`，可选 `offset`、`limit` | 1-based 行偏移；并行。 |
| `ls` | `path`，可选 `max_entries`、`max_depth` | 确定性目录列表；遵守 gitignore；并行。 |
| `glob` | `pattern` | 遵守 gitignore 的文件发现；并行。 |
| `find` | `pattern`，可选 `path` | 遵守 gitignore 的文件发现，可限制到子目录；并行。 |
| `grep` | `pattern` | 遵守 gitignore 的正则搜索；并行。 |
| `write` | `path`、`content` | 创建父目录；串行；修改性。 |
| `edit` | `path`、`old_string`、`new_string` | 替换第一个精确匹配，并记录 before/after details；串行；修改性。 |
| `bash` | `command`，可选 `timeout_secs` | 在 workspace 根目录运行；Windows 使用 `cmd /C`，Unix 使用 `sh -c`；串行；修改性。 |

默认启用工具：

| 模式 | 工具 |
|------|------|
| 交互式 | `read`、`write`、`edit`、`bash` |
| 非交互 / RPC | `read`、`grep`、`find`、`ls`、`glob` |
| 非交互 / RPC 且显式允许修改 | `read`、`write`、`edit`、`bash` |

非交互/RPC 模式下，显式 allowlist 如果包含 `write`、`edit` 或 `bash`，必须同时设置
`--allow-mutating` 或 `defaults.allow_mutating_tools = true`。

## 运行模式

### 交互式

没有提示词参数时，`opi` 启动 ratatui TUI。Slash 命令包括：

| 命令 | 作用 |
|------|------|
| `/model` | 打开当前 Provider 的模型选择器。 |
| `/session` | 打开会话选择器。 |
| `/branch` | 打开分支选择器。 |
| `/tree` | 打开会话树选择器。 |
| `/fork` | 把当前活跃分支 fork 成新的父子会话。 |
| `/clone` | 把当前活跃分支 clone 成新的父子会话。 |
| `/image <path>` | 为下一条提示词排队一张图片。 |
| `exit` / `quit` | 退出。 |

### 非交互与 JSON

文本模式把助手文本写到 stdout，把诊断写到 stderr。`--json` 会输出 schema header、
序列化 session/agent 事件，以及最终 `session_summary`，格式为 NDJSON。

退出码：

| Code | 含义 |
|------|------|
| `0` | 成功 |
| `1` | 运行时失败 |
| `2` | 配置错误 |
| `3` | 鉴权失败 |
| `4` | Provider 失败 |
| `5` | 工具失败 |
| `130` | 被中断 |

### RPC JSONL

`--rpc` 为 IDE、自定义 UI 和其他嵌入方启动持久双向 JSONL 协议。这是不稳定的 0.x
协议；客户端必须检查 `rpc_ready` header 中的 `schema_version`。当前 SDK/RPC
schema version 是 `3`。启动诊断会通过该 ready header 的 `startup_diagnostics`
字段暴露。

命令包括 `prompt`、`continue`、`steer`、`follow_up`、`abort`、`set_model`、
`set_thinking_level`、`compact`、`session_info`、`extension_command`、`trace` 和
`quit`。

## 配置、会话与上下文文件

配置会合并用户配置、项目配置和显式 `--config` 文件。模型优先级依次为
`--model`、未传入 `--config` 时的 `OPI_MODEL`、显式配置、项目 `.opi/config.toml`、
用户配置和内置默认值。

用户配置路径：

- Windows: `%APPDATA%\opi\config.toml`
- Unix: `~/.config/opi/config.toml`

会话是 append-only JSONL 文件。默认位置是 Windows 的
`%LOCALAPPDATA%\opi\sessions\` 和 Unix 的 `~/.local/share/opi/sessions/`，可用
`OPI_SESSIONS_DIR` 覆盖。

`CodingHarness` 会从 workspace 祖先目录向上到 git root 加载 `AGENTS.md` 和
`CLAUDE.md`，然后加载用户配置目录中的同名文件。空文件和超过 128 KiB 的文件会被
忽略。`OPI.md` 有意不加载。

## 资源与 Package

资源发现覆盖来自用户、项目、显式和 package 层的 extensions、packages、skills、
prompt fragments 和 themes。高优先级层覆盖低优先级层；同一层内的重复名称会作为
diagnostics 暴露。

Package 命令：

```sh
opi package add ./vendor/todo
opi package add --local ./vendor/todo
opi package add git:github.com/user/pkg@v1
opi package list
opi package list --json
opi package doctor
opi package doctor --json
opi package remove todo
```

Package 可以启动使用 `opi-extension-jsonl-v1` 协议的 `process-jsonl` adapter。该
adapter 协议是不稳定的 0.x 契约。Package 是受信任代码，不会被 package manager
sandbox。

## 作为库使用

`CodingHarness` 是嵌入入口。它可以直接构建，也可以通过 `CodingHarness::builder`
构建，并可配置自定义 hooks、会话恢复数据、工具选择、运行时 package 状态、资源
metadata 和启动诊断。

常用方法包括 `prompt`、`prompt_with_content`、`queue_images`、`subscribe`、
`cancel`、`set_model`、`model_picker_items`、`branch_picker_items`、
`resource_metadata`、`resolve_theme` 和 `session`。

## 边界

- `opi` 不收集 telemetry 或 analytics，也不会自动分享会话。
- `opi doctor` 默认不发起付费模型调用，也不做网络检查。
- 修改性工具策略不是操作系统级 sandbox。
- 生产级子 Agent、permission gate、plan/todo 和 MCP 工作流是 examples/package
  模式，不是内置核心工作流。
- OAuth 或订阅登录流程尚未实现。

## 许可证

MIT。详见 workspace [LICENSE](../../LICENSE)。
