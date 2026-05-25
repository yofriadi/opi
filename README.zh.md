# opi

[![CI](https://github.com/OdradekAI/opi/actions/workflows/ci.yml/badge.svg)](https://github.com/OdradekAI/opi/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

> Rust 编写的 AI Agent 工具包，将 [earendil-works/pi](https://github.com/earendil-works/pi) 的思路重新实现为终端优先的编程 Agent 与可复用 Agent crate。

[English](README.md) | [更新日志](CHANGELOG.md) | [技术规范](docs/opi-spec.zh.md)

## 当前状态

当前 workspace 版本：`0.3.0`。

`opi` 现在包含可用的编程 Agent 二进制、6 个内置工具、ratatui TUI、非交互 stdout 模式、NDJSON 模式、TOML 配置、多 Provider 流式接入、会话持久化、上下文压缩、retry/backoff、可配置按键与主题、用量累计，以及尽力而为的费用估算。

`opi-web-ui` 仍是预留占位 crate，不会发布到 crates.io。

## 工作区

Cargo workspace 采用锁步版本：所有 crate 都从 `[workspace.package]` 继承 `version`、`edition`、`license`、`repository` 和 `authors`。

| Crate | 是否发布 | 说明 |
|-------|----------|------|
| [`opi-ai`](crates/opi-ai) | 是 | 多 Provider LLM API、流式事件、注册表、重试、用量与费用工具 |
| [`opi-agent`](crates/opi-agent) | 是 | Agent 主循环、工具执行、hooks、事件、会话、压缩、transport trait |
| [`opi-tui`](crates/opi-tui) | 是 | Ratatui 组件、diff 视图、主题、按键绑定 |
| [`opi-coding-agent`](crates/opi-coding-agent) | 是 | `opi` 二进制与可嵌入的编程 harness |
| [`opi-web-ui`](crates/opi-web-ui) | 否（`publish = false`） | 预留的 Web 聊天组件 crate |

内部依赖关系：

```text
opi-ai
  -> opi-agent
  -> opi-web-ui

opi-tui

opi-ai + opi-agent + opi-tui
  -> opi-coding-agent
     -> opi binary
```

## 安装

可执行文件名是 `opi`，由 `opi-coding-agent` crate 产出。

```sh
cargo install opi-coding-agent
opi --version
```

每个 [GitHub Release](https://github.com/OdradekAI/opi/releases) 都附带 Linux、macOS、Windows 的 x64 与 arm64 预编译二进制。

## 快速开始

先设置要使用的 Provider API key：

```sh
export ANTHROPIC_API_KEY=sk-ant-...
# 或 OPENAI_API_KEY、OPENROUTER_API_KEY、MISTRAL_API_KEY、GEMINI_API_KEY
```

启动交互式 TUI：

```sh
opi
```

运行单次提示词，并把助手文本输出到 stdout：

```sh
opi "列出这个 workspace 里的 Rust crate。"
```

为自动化流程输出 newline-delimited JSON 事件：

```sh
opi --json "总结最新会话状态。"
```

使用 `provider:model` 语法指定模型：

```sh
opi -m anthropic:claude-sonnet-4-5-20250514 "解释 crates/opi-agent/src/lib.rs"
opi -m openai:gpt-4o "检查公共 API 形态。"
opi -m openai-responses:gpt-4o-mini "找出小的文档缺口。"
opi -m openrouter:openai/gpt-4o-mini "列出 TODO 注释。"
opi -m mistral:codestral-latest "解释工具模块。"
opi -m gemini:gemini-2.5-flash "总结 README 文件。"
```

## 支持的 Provider

Provider 在 `opi-ai` 中实现，并已接入 `opi-coding-agent`。

| Provider spec 前缀 | 默认 API key 环境变量 | 说明 |
|--------------------|-----------------------|------|
| `anthropic:` | `ANTHROPIC_API_KEY` | Anthropic Messages API，支持 thinking |
| `openai:` | `OPENAI_API_KEY` | OpenAI Chat Completions 兼容流式接口 |
| `openai-responses:` | `OPENAI_API_KEY` | OpenAI Responses API 流式接口 |
| `openrouter:` | `OPENROUTER_API_KEY` | OpenAI-compatible OpenRouter profile |
| `mistral:` | `MISTRAL_API_KEY` | OpenAI-compatible Mistral profile |
| `gemini:` | `GEMINI_API_KEY` | Gemini `streamGenerateContent` SSE |

## 内置工具

工具由 `opi-coding-agent` 实现，并通过 `opi-agent::Tool` trait 暴露。

| 工具 | 参数 | 执行模式 | 是否修改 |
|------|------|----------|----------|
| `read` | `path`，可选 `offset`、`limit` | 并行 | 否 |
| `glob` | `pattern` | 并行 | 否 |
| `grep` | `pattern` | 并行 | 否 |
| `write` | `path`、`content` | 串行 | 是 |
| `edit` | `path`、`old_string`、`new_string` | 串行 | 是 |
| `bash` | `command`，可选 `timeout_secs` | 串行 | 是 |

所有路径都被限制在 harness 的 workspace 根目录下。修改性工具需要 `--allow-mutating`，或配置 `defaults.allow_mutating_tools = true`。

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
model = "anthropic:claude-sonnet-4-5-20250514"
max_iterations = 50
tool_timeout_ms = 30000
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

[providers.openai_responses]
api_key_env = "OPENAI_API_KEY"

[providers.openrouter]
api_key_env = "OPENROUTER_API_KEY"
# referer = "https://example.com"

[providers.mistral]
api_key_env = "MISTRAL_API_KEY"

[providers.gemini]
api_key_env = "GEMINI_API_KEY"
```

## 会话

编程 harness 会自动把会话写成 JSONL 文件。

默认位置：

- Windows：`%LOCALAPPDATA%\opi\sessions\`
- Unix：`~/.local/share/opi/sessions/`

可以用 `OPI_SESSIONS_DIR` 覆盖。

```sh
opi --list-sessions
opi --resume <session-id> "从这个会话继续。"
opi --delete-session <session-id>
```

会话文件保存 header，以及 message、compaction、leaf 条目。Resume 会重建活跃分支，并保留压缩摘要语义。`--json` 会输出 session 事件、retry 事件、compaction 事件，以及包含 token 总量和可选费用总量的最终 session summary。

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

- 非交互：位置参数提示词、`--non-interactive` 或 `--json`；构建 Provider 并运行 `NonInteractiveRunner`。
- 交互式：无提示词时的默认模式；构建带交互 hooks 的 `CodingHarness` 并启动 ratatui TUI。
- 会话命令：`--list-sessions`、`--resume`、`--delete-session` 会在 Provider 构建之前处理。

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
- `opi_agent::Transport`：为外部工具服务器预留的 stdio/SSE transport 抽象；尚未接入主循环。

## 尚未实现

- 子 Agent 与 skills。
- Prompt template registry。
- 通过 `Transport` 接入 MCP 工具服务器。
- OAuth 或订阅登录流程。
- `opi-web-ui` 中真实的 Web UI 组件。

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
