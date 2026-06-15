# opi-coding-agent

[![Crates.io](https://img.shields.io/crates/v/opi-coding-agent.svg)](https://crates.io/crates/opi-coding-agent)
[![Docs.rs](https://docs.rs/opi-coding-agent/badge.svg)](https://docs.rs/opi-coding-agent)

> `opi` 二进制：基于 `opi-ai`、`opi-agent` 和 `opi-tui` 构建的交互式与非交互式终端编程 Agent。

[English](README.md) | [opi workspace](../../README.zh.md)

## 当前状态

当前 crate 版本：`0.5.0`。

本 crate 产出 `opi` CLI，同时也把编程 harness 暴露为 Rust library。当前支持交互式 TUI、位置参数非交互模式、NDJSON 输出、RPC JSONL 模式、9 个内置 Provider 前缀加已配置的 OpenAI-compatible profile、8 个可用内置工具、pi 对齐的交互式默认工具、保守的非交互默认工具、图片附件、模型/会话/分支/会话树选择器、交互式会话 fork/clone、shell 补全生成、上下文文件加载、会话持久化、会话 resume/fork/list/delete、上下文压缩、可配置按键/主题、按 Provider 配置代理、packages/extensions/skills/fragments/themes 的渐进式资源发现、package add/remove/list/doctor 命令、process-jsonl package adapter、retry、token 用量统计，以及尽力而为的费用摘要。

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

# 为第一条提示词附加图片
opi --image screenshot.png "审查这张截图。"

# 在非交互自动化中允许修改性工具
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
| `--allow-mutating` | 在非交互模式中允许 `write`、`edit` 和 `bash` |
| `--json` | 输出 NDJSON 事件到 stdout；同时使用非交互模式 |
| `--list-sessions` | 列出已保存会话并退出 |
| `--resume <ID>` | 按 id 恢复会话 |
| `--fork <ID>` | 按 id fork 已保存会话，生成新会话 |
| `--delete-session <ID>` | 按 id 删除会话并退出 |
| `--generate-completion <SHELL>` | 为 `bash`、`zsh`、`fish`、`powershell` 或 `elvish` 生成 shell 补全 |
| `-v, --verbose` | 启用 debug tracing |
| `--tools <TOOLS>` | 逗号分隔的启用工具 allowlist，例如 `read,grep` |
| `--no-tools` | 禁用所有工具 |
| `--no-builtin-tools` | 禁用内置工具；为扩展/自定义工具预留 |
| `--image <IMAGE>` | 给初始提示词附加一张图片；可重复 |
| `--list-models` | 列出已配置 Provider 可用模型并退出 |
| `--rpc` | RPC JSONL 模式：通过 stdin/stdout 双向命令/事件协议 |
| `package <COMMAND>` | 管理 extension package：`add`、`remove`、`list`、`doctor` |

## Provider

`opi-coding-agent` 会根据配置的模型前缀构建 Provider。

| 前缀 | Provider | 默认凭据/配置 |
|------|----------|---------------|
| `anthropic:` | `AnthropicProvider` | `ANTHROPIC_API_KEY` |
| `openai:` | `OpenAiChatProvider` | `OPENAI_API_KEY` |
| `openai-responses:` | `OpenAiResponsesProvider` | `OPENAI_API_KEY` |
| `openrouter:` | OpenRouter profile | `OPENROUTER_API_KEY` |
| `mistral:` | Mistral profile | `MISTRAL_API_KEY` |
| `gemini:` | `GeminiProvider` | `GEMINI_API_KEY` |
| `bedrock:` | `BedrockProvider` | AWS 环境变量或共享 AWS profile/config |
| `azure:` | `AzureOpenAIProvider` | `AZURE_OPENAI_API_KEY`；endpoint/deployments 在配置中设置 |
| `vertex:` | `VertexProvider` | `VERTEX_ACCESS_TOKEN`；project/location 在配置中设置 |
| 已配置 profile | OpenAI-compatible profile | profile 自己的 `api_key_env`、`base_url` 和模型列表 |

环境变量名、base URL、Provider 专用字段和代理都可以在配置中覆盖。

## 配置

配置层按顺序合并：用户配置、项目配置、显式 `--config` 文件。后面的层覆盖前面的同名字段。

模型优先级：

1. `--model`
2. 未传入 `--config` 时的 `OPI_MODEL`
3. `--config <FILE>` 中的 `model`
4. `<CWD>/.opi/config.toml`
5. 用户配置
6. 内置默认值

常用默认结构：

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

如果没有为 Provider 单独配置代理，HTTP client 会回退到 `HTTP_PROXY`、`HTTPS_PROXY` 和 `NO_PROXY`。

## 内置工具

工具位于 `src/tool/`。

| 工具 | 参数 | 说明 |
|------|------|------|
| `read` | `path`，可选 `offset`、`limit` | 1-based 行偏移；并行 |
| `ls` | `path`，可选 `max_entries`、`max_depth` | 确定性目录列表；遵守 gitignore；并行 |
| `glob` | `pattern` | 遵守 gitignore 的文件发现；并行 |
| `find` | `pattern`，可选 `path` | 遵守 gitignore 的文件发现，可限制到子目录；并行 |
| `grep` | `pattern` | 遵守 gitignore 的正则搜索；并行 |
| `write` | `path`、`content` | 创建父目录；串行；修改性 |
| `edit` | `path`、`old_string`、`new_string` | 替换第一个精确匹配，并记录 before/after details；串行；修改性 |
| `bash` | `command`，可选 `timeout_secs` | 在 workspace 根目录运行；Windows 使用 `cmd /C`，Unix 使用 `sh -c`；串行；修改性 |

可用内置工具包括 `read`、`write`、`edit`、`bash`、`grep`、`find`、`ls` 和 `glob`。

默认启用工具取决于运行模式：

- 交互模式：`read`、`write`、`edit`、`bash`。
- 非交互模式：`read`、`grep`、`find`、`ls`、`glob`。
- 非交互模式带 `--allow-mutating` 或 `defaults.allow_mutating_tools = true`：`read`、`write`、`edit`、`bash`。

`--tools <TOOLS>` 用于显式指定启用工具 allowlist。非交互模式下，如果 allowlist 包含 `write`、`edit` 或 `bash`，必须同时设置 `--allow-mutating` 或 `defaults.allow_mutating_tools = true`。

路径策略按模式区分。写入和编辑默认限制在 harness workspace 根目录内。交互模式的 `read` 可以解析绝对路径和 workspace 外路径；非交互模式的文件工具默认保持 workspace-only。文件工具 details 会记录 `workspace_root`、`resolved_path` 和 `inside_workspace`。

工具选择优先级是 `--no-tools` > `--tools` > `--no-builtin-tools` > 默认。

## 图片

`--image <PATH>` 会把图片附加到交互或非交互模式的第一条提示词。该参数可重复。交互模式也支持 `/image <path>`，用于把图片排队到下一条提示词。

支持 PNG、JPEG、GIF 和 WebP。默认文件大小限制为 20 MiB，可通过 `defaults.max_image_bytes` 修改。

## 会话

会话由 `SessionCoordinator` 自动持久化。

默认存储位置：

- Windows：`%LOCALAPPDATA%\opi\sessions\`
- Unix：`~/.local/share/opi/sessions/`

可以用 `OPI_SESSIONS_DIR` 覆盖。

```sh
opi --list-sessions
opi --resume <session-id> "继续这项工作。"
opi --fork <session-id> "从 fork 继续。"
opi --delete-session <session-id>
```

Resume 会从 session JSONL 条目重建活跃分支。Fork 会创建新的 JSONL 会话，并在 header 中通过 `parent_session` 指回源会话；源会话保持 append-only。如果会话中包含 compaction marker，恢复后的上下文会包含压缩摘要和保留尾部。

## 运行模式

### 交互式

没有提示词参数时，`opi` 启动 ratatui TUI。它使用 `opi-tui` 组件渲染对话记录、输入编辑器、状态、Markdown、工具调用、编辑 diff、主题、按键绑定、模型/会话/分支选择器和终端图片输出。

Slash 命令：

| 命令 | 作用 |
|------|------|
| `/model` | 打开当前 Provider 的模型选择器 |
| `/session` | 打开会话选择器 |
| `/branch` | 打开当前会话的分支选择器 |
| `/tree` | 打开当前会话的会话树选择器 |
| `/fork` | 把当前活跃分支 fork 成新的父子会话 |
| `/clone` | 把当前活跃分支 clone 成新的父子会话 |
| `/image <path>` | 为下一条提示词排队一张图片 |
| `exit` 或 `quit` | 退出 |

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

### RPC JSONL 模式

`--rpc` 启动一个通过 stdin/stdout 进行双向 JSONL 通信的持久会话。这是 IDE、自定义 UI 和外部工具集成的推荐嵌入模式。

**这是一个不稳定的 0.x 协议。** schema 可能在次版本之间变更。客户端必须在 `rpc_ready` 头中检查 `schema_version`。

```sh
opi --rpc
```

启动时，`opi` 会输出 `rpc_ready` 头：

```json
{"type":"rpc_ready","schema_version":2,"mode":"rpc","version":"0.5.0"}
```

命令是以 JSON 对象形式发送到 stdin（每行一个）。响应和事件是以 JSON 对象形式输出到 stdout（每行一个）。诊断信息输出到 stderr。

#### 命令

| 命令 | 说明 |
|------|------|
| `prompt` | 发送用户提示词；agent 事件异步流式输出 |
| `continue` | 用附加文本继续对话 |
| `steer` | 在 agent 运行期间排队转向消息 |
| `follow_up` | 排队后续消息，在 agent 停止后处理 |
| `abort` | 取消当前 agent 操作 |
| `set_model` | 切换 provider:model |
| `set_thinking_level` | 设置推理/思考级别 |
| `compact` | 触发手动压缩 |
| `session_info` | 查询会话元数据 |
| `quit` | 关闭 RPC 会话 |

所有命令都支持可选的 `id` 字段用于请求/响应关联。

对于 `prompt` 和 `continue`，`success: true` 表示命令已被接受。agent 事件（包括接受后的错误）以异步事件行到达。

## 上下文文件

`CodingHarness` 会从 workspace 目录向上查找 `AGENTS.md` 和 `CLAUDE.md`，直到 git root，然后再查找用户配置目录。空文件和超过 128 KiB 的文件会被跳过。

## 资源与 Package

Harness 会从用户、项目、显式和 package 层发现资源元数据，并把它暴露到系统提示词和 RPC/session metadata 中。发现范围包括：

- Extensions：包含 `extension.toml` 的目录。
- Packages：包含 `package.toml` 的目录；package 可以从约定子目录组合 extensions、skills、prompt fragments 和 themes。
- Skills：包含带 YAML frontmatter 的 `SKILL.md` 的目录。
- Prompt fragments：包含带 YAML frontmatter 的 `FRAGMENT.md` 的目录。
- Themes：包含 `theme.toml` 的目录，会在回退到内置主题前解析。

用户级资源位于用户配置目录下（Unix：`~/.config/opi/`；Windows：`%APPDATA%\opi\`）。项目级资源位于 workspace 根目录的 `.opi/` 下。显式 extension 和 package 路径来自配置。高优先级层会覆盖低优先级层；同一层内的重复项会作为 diagnostics 暴露。

Package 命令可以在不构建 Provider 的情况下管理本地和 git package 声明：

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

`add` 和 `remove` 默认写入用户级 package store；传入 `--local` 时写入项目本地 `.opi/packages.toml`。运行时启动会解析已安装声明、校验 lock 状态，并启动有效的 `[adapter]` package；当前支持的 adapter kind 是 `process-jsonl`，协议是 `opi-extension-jsonl-v1`。

## 技能（Skills）

技能通过渐进式发现从项目、用户、显式和包资源中加载。每个技能是一个包含 `SKILL.md` 文件的目录，`SKILL.md` 使用 YAML frontmatter。

**这是一个不稳定的 0.x API。** 技能格式和发现规则可能在次版本之间变更。

### 技能格式

技能目录包含一个 `SKILL.md`：

```markdown
---
name: my-skill
description: 技能功能描述和适用场景。
disable-model-invocation: false
---

完整技能指令写在这里。
```

字段：

| 字段 | 必填 | 说明 |
|------|------|------|
| `name` | 是 | 小写 `a-z`、`0-9`、连字符。最长 64 字符。 |
| `description` | 是 | 最长 1024 字符。 |
| `disable-model-invocation` | 否 | 默认 `false`。设为 `true` 时，技能不会被模型自动调用，但仍可由用户手动使用。 |

### 发现位置

技能从多个层级发现，采用基于优先级的去重（高优先级在名称冲突时覆盖）：

1. **用户级**（Unix: `~/.config/opi/skills/`，Windows: `%APPDATA%\opi\skills\`）— 优先级 0
2. **项目级**（workspace 根目录的 `.opi/skills/`）— 优先级 1
3. **显式资源层**，由嵌入方传入 — 优先级 2
4. **Package 组合资源**，来自已发现的 package，并使用 package 所在层的优先级

每个技能是扫描位置下的一个子目录，包含 `SKILL.md` 文件。

### 渐进式披露

技能元数据（名称、描述）无需加载完整技能正文即可使用。完整指令仅在技能被调用时按需加载。这保持了初始上下文的精简，同时支持丰富的专业化指令。

## 提示词片段（Prompt Fragments）

提示词片段（模板）通过渐进式发现从项目、用户、显式和包资源中加载。每个片段是一个包含 `FRAGMENT.md` 文件的目录，`FRAGMENT.md` 使用 YAML frontmatter。

**这是一个不稳定的 0.x API。** 片段格式和发现规则可能在次版本之间变更。

### 片段格式

片段目录包含一个 `FRAGMENT.md`：

```markdown
---
name: translate
description: 在语言之间翻译文本。
arguments: text, from=en, to=fr
---

将 {{text}} 从 {{from}} 翻译为 {{to}}。
```

字段：

| 字段 | 必填 | 说明 |
|------|------|------|
| `name` | 是 | 小写 `a-z`、`0-9`、连字符。最长 64 字符。 |
| `description` | 是 | 最长 1024 字符。 |
| `arguments` | 否 | 逗号分隔列表。必填参数：`name`。可选参数：`name=default`。 |

### 参数展开

在 frontmatter 中声明的参数在正文中以 `{{name}}` 占位符引用。展开时：

- 必填参数必须提供。
- 可选参数未提供时使用声明的默认值。
- 未声明的占位符保持原样。

### 发现位置

片段使用与技能和扩展相同的基于优先级的发现机制（高优先级在名称冲突时覆盖）：

1. **用户级**（Unix: `~/.config/opi/fragments/`，Windows: `%APPDATA%\opi\fragments\`）— 优先级 0
2. **项目级**（workspace 根目录的 `.opi/fragments/`）— 优先级 1
3. **显式资源层**，由嵌入方传入 — 优先级 2
4. **Package 组合资源**，来自已发现的 package，并使用 package 所在层的优先级

## 主题（Themes）

Themes 从用户、项目、显式和 package 层的 `theme.toml` 文件中发现。主题文件包含元数据和可选颜色 token 覆盖：

```toml
name = "operator"
description = "Operator theme"

[colors]
role_user = "Green"
status_bg = "#1a1a2e"
```

未知 token 和非法颜色会产生 diagnostics。未指定的颜色 token 会继承默认主题。运行时会先解析发现的主题，再回退到内置 `default` 和 `monokai`。

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

嵌入自定义应用时，可以使用 `builder`、`new_with_hooks`、`new_with_hooks_and_resume`、`new_with_selection`、`subscribe`、`cancel`、`queue_images`、`prompt_with_content`、`model_picker_items`、`branch_picker_items`、`set_model`、`resource_metadata`、`resolve_theme` 和 `session`。

## 许可证

MIT。详见 workspace [LICENSE](../../LICENSE)。
