# opi-coding-agent

[![Crates.io](https://img.shields.io/crates/v/opi-coding-agent.svg)](https://crates.io/crates/opi-coding-agent)
[![Docs.rs](https://docs.rs/opi-coding-agent/badge.svg)](https://docs.rs/opi-coding-agent)

> `opi` 二进制 —— 一个最小化的终端编码 Agent。由本 crate 产出，依赖 [`opi-ai`](https://crates.io/crates/opi-ai)、[`opi-agent`](https://crates.io/crates/opi-agent) 与 [`opi-tui`](https://crates.io/crates/opi-tui)。

[English](README.md) · [← opi](../../README.zh.md)

---

## 当前状态（v0.2.0）

Phase 1 MVP。交互式 TUI 与非交互模式（位置参数提示词或 `--non-interactive`）都已在 Anthropic 上端到端可用，自带 6 个内置工具、TOML 配置、清晰的退出码以及一套高风险工具的安全策略。

尚未实现：其他 provider、持久化会话、子 Agent、Skills、斜杠命令、`/login` / OAuth。

## 安装

```sh
cargo install opi-coding-agent
opi --version
```

或从 [GitHub Release](https://github.com/OdradekAI/opi/releases) 下载对应平台的预编译二进制。

## 快速上手

```sh
export ANTHROPIC_API_KEY=sk-ant-...

# 交互式（ratatui TUI）
opi

# 非交互：位置参数提示词 → 输出到 stdout → 退出
opi "找出当前仓库里所有的 TODO。"

# 指定其他模型
opi -m anthropic:claude-opus-4 "解释一下 src/main.rs"

# 非交互模式下允许 write/edit/bash
opi "为最新一次 commit 追加 CHANGELOG 条目。" --allow-mutating
```

## 命令行参数

| 参数 | 说明 |
|------|------|
| `[PROMPT]...` | 位置参数提示词；非空时进入非交互模式 |
| `-m, --model <SPEC>` | 模型 spec（如 `anthropic:claude-sonnet-4`） |
| `-c, --config <FILE>` | TOML 配置文件路径（必须存在） |
| `-s, --system <FILE>` | 系统提示词文件（追加到内置提示词之前） |
| `--non-interactive` | 强制非交互模式（仍需提供提示词文本） |
| `--allow-mutating` | 在非交互模式下放开 `write` / `edit` / `bash` |
| `-v, --verbose` | 打开 debug tracing |

## 配置

TOML 文件按 **用户 → 项目 → `--config`** 逐层 merge（后者覆盖前者同名字段）。

**模型**优先级（从高到低）：

1. `--model`（CLI）
2. `OPI_MODEL` —— 仅当**未**传入 `--config` 时生效
3. `--config <file>` 中的 `model`
4. 项目配置：`<CWD>/.opi/config.toml`
5. 用户配置：Windows `%APPDATA%\opi\config.toml`，Unix `~/.config/opi/config.toml`
6. 内置默认值

`.opi/config.toml` 结构（所有字段都是可选的，下方值即默认值）：

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

[providers.anthropic]
api_key_env = "ANTHROPIC_API_KEY"
# base_url = "https://api.anthropic.com"  # 仅在需要时覆盖
```

把 `defaults.allow_mutating_tools = true` 写进配置，就可以省去每次非交互调用都加 `--allow-mutating`。

## 内置工具

工具实现位于 [`src/tool/`](src/tool)：

| 工具 | 参数 | 说明 |
|------|------|------|
| `read` | `path`，可选 `offset` + `limit` | 1-based 行号区间 |
| `glob` | `pattern`，可选 `path` | 遵循 .gitignore |
| `grep` | `pattern`，可选 `glob` / `path` | 遵循 .gitignore，支持正则 |
| `write` | `path`、`content` | 修改性，非交互模式需要 `--allow-mutating` |
| `edit` | `path`、`old_string`、`new_string` | 精确字符串替换；修改性 |
| `bash` | `command`，可选 `timeout_secs` | Windows 下用 `cmd.exe`，Unix 下用 `sh` |

所有路径都相对于（并被限制在）传给 `CodingHarness::new` 的 workspace 根目录（CLI 使用当前工作目录）。

## 运行模式

### 非交互

`NonInteractiveRunner::run()` 把助手输出写到 stdout、把诊断信息写到 stderr，最终返回如下退出码：

| 退出码 | 含义 |
|--------|------|
| `0` | 成功 |
| `1` | 运行时失败 |
| `2` | 配置错误 |
| `3` | 鉴权失败（缺少或无效的 API key） |
| `4` | Provider 调用失败 |
| `5` | 工具执行失败 |
| `130` | 被中断（Ctrl+C） |

### 交互

`CodingHarness` + `InteractiveCodingHooks` 驱动一个由 `opi-tui` 组件搭起来的 ratatui TUI。流式增量会实时更新对话记录，工具调用以带状态的形式呈现，修改性工具会弹出确认。

## 作为库使用

`opi-coding-agent` 同时也是一个普通的 library crate。`harness` 与 `runner` 模块可以让你把同一套主循环嵌入到自己的 Rust 应用中：

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
let _messages = harness.prompt("你好！").await?;
# Ok(()) }
```

## 许可证

MIT —— 见 workspace 根目录 [`LICENSE`](../../LICENSE)。
