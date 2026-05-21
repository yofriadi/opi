# opi

[![CI](https://github.com/OdradekAI/opi/actions/workflows/ci.yml/badge.svg)](https://github.com/OdradekAI/opi/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

> Rust 编写的 AI Agent 工具包 —— [earendil-works/pi](https://github.com/earendil-works/pi) 的精简移植，目标是一个最小化的终端编码 Agent。

[English](README.md) · [更新日志](CHANGELOG.md) · [技术规范](docs/opi-spec.zh.md)

---

## 当前状态

Phase 1 MVP（`v0.2.0`）。已完成基于 Anthropic 的可用编码助手，自带六个内置工具、基于 ratatui 的 TUI、TOML 配置体系，以及一套 Mock Provider 测试夹具（覆盖 248 个单测/集成测试）。其他 LLM 提供方、子 Agent、会话持久化、MCP 传输、Web UI 等尚未实现，详见下方 [路线图](#路线图)。

## 工作区布局

Cargo workspace，所有 crate 在 `[workspace.package]` 中**统一版本号**。

| Crate | crates.io | 说明 |
|-------|-----------|------|
| [`opi-ai`](crates/opi-ai) | 已发布 | Provider 抽象 + Anthropic SSE 流式接入 |
| [`opi-agent`](crates/opi-agent) | 已发布 | Agent 运行时：工具调用、Hook、消息队列轮询 |
| [`opi-tui`](crates/opi-tui) | 已发布 | 终端 UI 组件（消息列表、编辑器、Markdown、状态栏、工具视图） |
| [`opi-coding-agent`](crates/opi-coding-agent) | 已发布 | `opi` 二进制 —— 交互式 / 非交互式编码 Agent |
| [`opi-web-ui`](crates/opi-web-ui) | `publish = false` | 占位 crate，尚未实现 |

依赖关系（同时也是 crates.io 发布顺序）：

```
opi-ai      ─┬─→ opi-agent ─┐
             │              ├─→ opi-coding-agent  ──╮
opi-tui ─────┴──────────────┘                       │
opi-web-ui ──→ opi-ai                               │
                                                    └→  opi  二进制
```

## 安装

可执行文件名为 `opi`，由 `opi-coding-agent` crate 构建产出。

```sh
cargo install opi-coding-agent
opi --version
```

每个 [GitHub Release](https://github.com/OdradekAI/opi/releases) 都附带了 Linux、macOS、Windows（x64 + arm64）的预编译二进制。

## 快速上手

```sh
export ANTHROPIC_API_KEY=sk-ant-...

# 交互式 TUI
opi

# 非交互模式（位置参数提示词 → 输出到 stdout → 退出）
opi "列出当前目录下的 Rust 文件。"
```

v0.2.0 只支持 `anthropic:<model>` 形式的 model spec，默认值为 `anthropic:claude-sonnet-4`。可按需覆盖：

```sh
opi -m anthropic:claude-opus-4 "解释一下 src/main.rs"
```

也可以通过 `OPI_MODEL`、`--config`、项目级 `.opi/config.toml`、用户级配置文件来设置。模型优先级：**`--model` > `OPI_MODEL`（未传 `--config` 时）> `--config` 文件 > 项目配置 > 用户配置 > 内置默认**（详见 [`opi-coding-agent`](crates/opi-coding-agent/README.zh.md)）。

## 内置工具

Agent 通过 `opi-agent` 的 `Tool` trait 暴露 6 个工具：

| 工具 | 用途 | 是否修改文件系统 |
|------|------|------------------|
| `read` | 读取文件，支持指定行区间 | 否 |
| `glob` | 按 glob 列出文件（遵循 .gitignore） | 否 |
| `grep` | 在文件内容中搜索（遵循 .gitignore） | 否 |
| `write` | 新建或覆盖文件 | 是 |
| `edit` | 精确字符串替换 | 是 |
| `bash` | 在限定超时内执行 shell 命令 | 是 |

非交互模式下，修改性工具需要显式 `--allow-mutating`（或在配置文件里设置 `defaults.allow_mutating_tools = true`）；交互模式下，TUI 会在调用前弹出确认。

## 源码构建

工作区使用 **Rust edition 2024**，需要 ≥ 1.85 的工具链。

```sh
# 构建全部 crate
cargo build
cargo build --release

# 不安装直接运行 CLI
cargo run -p opi-coding-agent -- --help

# 运行测试套件（共 248 个测试）
cargo test --workspace --all-targets

# 指定单个 crate
cargo test -p opi-ai

# CI 强制执行的检查项
cargo fmt --check --all
cargo clippy --workspace --all-targets -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
```

## 架构

`opi` 二进制启动时根据参数选择路径：

- **非交互**（非空位置参数 `[PROMPT]...`，或 `--non-interactive`）：构造 provider → 运行 `NonInteractiveRunner::run()` → 输出 stdout/stderr → 按退出码退出（`0` 成功，`1` 运行时错误，`2` 配置错误，`3` 鉴权失败，`4` provider 失败，`5` 工具失败，`130` 被中断）。
- **交互**（默认）：构造带 `InteractiveCodingHooks` 的 `CodingHarness`，进入 ratatui TUI。

两种路径都走同一份 `opi-agent::agent_loop`：

```
transform_context → convert_to_llm → provider.stream(Request) → SSE / 工具事件
   → JSON Schema 校验 → before_tool_call → 工具执行（并行 / 串行）
   → after_tool_call → should_stop_after_turn → 拉取 steering / follow-up 队列 → 进入下一轮
```

关键抽象：

- **`opi_ai::Provider`** —— `stream(Request) -> EventStream`，事件类型为 `AssistantStreamEvent`，通过 `tokio_util::sync::CancellationToken` 取消。
- **`opi_agent::Tool`** —— `definition()` 返回 JSON Schema；`execute()` 执行；`execution_mode()` 决定该批工具串行还是并行。
- **`opi_agent::AgentHooks`** —— 六个 Hook 方法：`transform_context` / `convert_to_llm` / `before_tool_call` / `after_tool_call` / `should_stop_after_turn` / `prepare_next_turn`。
- **`opi_agent::Transport`** —— 为 stdio / SSE 工具传输预留的 trait，尚未接入主循环。

完整规范请参考 [`docs/opi-spec.zh.md`](docs/opi-spec.zh.md)。

## 路线图

Phase 1（✅ 已在 0.2.0 发布）：
- Anthropic provider、`Tool` + `AgentHooks` trait、agent 主循环、6 个工具、基础 TUI、TOML 配置。

待实现：
- 其他 provider（OpenAI、Google、Mistral、Bedrock、Azure）—— `ProviderKind` / `ApiKind` 已在注册层与消息类型中预留，但只接入了 Anthropic。
- 持久化会话、分支、上下文压缩。
- 子 Agent、Skills、Prompt 模板、MCP transport。
- `opi-web-ui`（目前是 `publish = false` 的占位 crate）。
- 订阅 / OAuth 登录流程（`/login`）。

## 发布流程

GitHub Release 与 crates.io 的发布由 `opi-release` skill（`.claude/skills/opi-release/skill.md`）统一编排：

- 所有 crate 使用同一版本号，按依赖顺序（由 `cargo metadata` 动态计算）发布。
- 推送 `v*` tag 会触发 [`release.yml`](.github/workflows/release.yml)，自动构建 6 个平台目标并上传到 Release。
- 回滚通过 `git revert` + 删除 tag 完成；**严禁** `git reset --hard` + `git push --force`。

## 参与贡献

项目约定：

- Conventional Commits（`feat:` → Added，`fix:` → Fixed，`feat!:` / `BREAKING CHANGE` → Breaking）。
- 每个 crate 的 `description`、`license`、`repository` 都从 `[workspace.package]` 继承，**不要在 crate 内重复声明**。
- 仓库针对人和 Agent 都有统一的协作规则，详见 [`CLAUDE.md`](CLAUDE.md)。

欢迎在 <https://github.com/OdradekAI/opi/issues> 提交 Issue / PR。

## 许可证

MIT © OdradekAI，详见 [`LICENSE`](LICENSE)。
