# opi-coding-agent

[![Crates.io](https://img.shields.io/crates/v/opi-coding-agent.svg)](https://crates.io/crates/opi-coding-agent)
[![Docs.rs](https://docs.rs/opi-coding-agent/badge.svg)](https://docs.rs/opi-coding-agent)

> `opi` 二进制：基于 `opi-ai`、`opi-agent` 和 `opi-tui` 构建的交互式与非交互式终端编程 Agent。

[English](README.md) | [opi workspace](../../README.zh.md)

## 当前状态

当前 crate 版本：`0.3.0`。

本 crate 产出 `opi` CLI，同时也把编程 harness 暴露为 Rust library。当前支持交互式 TUI、位置参数非交互模式、NDJSON 输出模式、多 Provider 构建、内置文件/命令工具、会话持久化、会话 resume/list/delete、上下文压缩、可配置按键/主题、retry、token 用量统计，以及尽力而为的费用摘要。

## 安装

```sh
cargo install opi-coding-agent
opi --version
```

也可以从 [GitHub Release](https://github.com/OdradekAI/opi/releases) 下载预编译二进制。

## 快速开始

```sh
export ANTHROPIC_API_KEY=sk-ant-...

# 交互式 TUI
opi

# 单次提示词，助手文本输出到 stdout
opi "找出这个仓库里的所有 TODO 注释。"

# 为自动化输出 NDJSON 事件流
opi --json "总结这个 workspace。"

# 指定 provider/model
opi -m openai:gpt-4o "解释 crates/opi-coding-agent/src/main.rs"

# 允许 write/edit/bash 这类修改性工具
opi --allow-mutating "更新 README。"
```

## CLI 参数

| 参数 | 说明 |
|------|------|
| `[PROMPT]...` | 位置参数提示词；非空时进入非交互模式 |
| `-m, --model <SPEC>` | 模型 spec，例如 `anthropic:claude-sonnet-4-5-20250514` |
| `-c, --config <FILE>` | 显式 TOML 配置文件；必须存在 |
| `-s, --system <FILE>` | 用户系统提示词文件，会追加到内置编程提示词 |
| `--non-interactive` | 强制非交互模式；仍需提示词文本 |
| `--json` | 输出 NDJSON 事件到 stdout；同时使用非交互模式 |
| `--allow-mutating` | 允许 `write`、`edit`、`bash` |
| `--list-sessions` | 列出已保存会话并退出 |
| `--resume <ID>` | 按 id 恢复会话 |
| `--delete-session <ID>` | 按 id 删除会话并退出 |
| `-v, --verbose` | 启用 debug tracing |

## Provider

`opi-coding-agent` 会根据配置的模型前缀构建 Provider。

| 前缀 | Provider | 默认 API key 环境变量 |
|------|----------|-----------------------|
| `anthropic:` | `AnthropicProvider` | `ANTHROPIC_API_KEY` |
| `openai:` | `OpenAiChatProvider` | `OPENAI_API_KEY` |
| `openai-responses:` | `OpenAiResponsesProvider` | `OPENAI_API_KEY` |
| `openrouter:` | OpenRouter profile | `OPENROUTER_API_KEY` |
| `mistral:` | Mistral profile | `MISTRAL_API_KEY` |
| `gemini:` | `GeminiProvider` | `GEMINI_API_KEY` |

环境变量名和 base URL 都可以在配置中覆盖。

## 配置

配置层按顺序合并：用户配置、项目配置、显式 `--config` 文件。后面的层覆盖前面的同名字段。

模型优先级：

1. `--model`
2. 未传入 `--config` 时的 `OPI_MODEL`
3. `--config <FILE>` 中的 `model`
4. `<CWD>/.opi/config.toml`
5. 用户配置
6. 内置默认值

完整结构与默认值：

```toml
[defaults]
model = "anthropic:claude-sonnet-4"
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
```

## 内置工具

工具位于 `src/tool/`。

| 工具 | 参数 | 说明 |
|------|------|------|
| `read` | `path`，可选 `offset`、`limit` | 1-based 行偏移；并行 |
| `glob` | `pattern` | 遵守 gitignore 的文件发现；并行 |
| `grep` | `pattern` | 遵守 gitignore 的正则搜索；并行 |
| `write` | `path`、`content` | 创建父目录；串行；修改性 |
| `edit` | `path`、`old_string`、`new_string` | 替换第一个精确匹配，并记录 before/after details；串行；修改性 |
| `bash` | `command`，可选 `timeout_secs` | 在 workspace 根目录运行；Windows 使用 `cmd /C`，Unix 使用 `sh -c`；串行；修改性 |

所有文件路径都会被校验，不能越出 harness 的 workspace 根目录。除非设置 `--allow-mutating` 或 `defaults.allow_mutating_tools = true`，否则修改性工具会被拒绝。

## 会话

会话由 `SessionCoordinator` 自动持久化。

默认存储位置：

- Windows：`%LOCALAPPDATA%\opi\sessions\`
- Unix：`~/.local/share/opi/sessions/`

可以用 `OPI_SESSIONS_DIR` 覆盖。

```sh
opi --list-sessions
opi --resume <session-id> "继续这项工作。"
opi --delete-session <session-id>
```

Resume 会从 session JSONL 条目重建活跃分支。如果会话中包含 compaction marker，恢复后的上下文会包含压缩摘要和保留尾部。

## 运行模式

### 交互式

没有提示词参数时，`opi` 启动 ratatui TUI。它使用 `opi-tui` 组件渲染对话记录、输入编辑器、状态、Markdown、工具调用、编辑 diff、主题与按键绑定。

### 文本非交互

带提示词参数或 `--non-interactive` 时，`NonInteractiveRunner::run()` 把助手文本写到 stdout，把诊断信息写到 stderr。

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

### JSON 非交互

`--json` 会把 NDJSON 输出到 stdout。第一行是 schema header，随后是序列化的 session/agent 事件，最后输出带 token 总量和可选费用总量的 `session_summary`。

## 作为库使用

```rust
use opi_coding_agent::config::OpiConfig;
use opi_coding_agent::harness::CodingHarness;

# async fn example(provider: Box<dyn opi_ai::Provider>) -> anyhow::Result<()> {
let config = OpiConfig::default();
let mut harness = CodingHarness::new(
    provider,
    config.defaults.model.clone(),
    config,
    std::env::current_dir()?,
);
let _messages = harness.prompt("你好").await?;
# Ok(()) }
```

嵌入自定义应用时，可以使用 `new_with_hooks`、`new_with_hooks_and_resume`、`subscribe`、`cancel` 和 `session`。

## 许可证

MIT。详见 workspace [LICENSE](../../LICENSE)。
