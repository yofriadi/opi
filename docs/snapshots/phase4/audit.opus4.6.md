# Phase 4 Opus 4.6 审计报告

审计日期：2026-06-05
审计对象：`docs/snapshots/phase4/opi-impl-state.json`（19 个任务）与当前工作树
审计方法：对 ledger 中每个 `passing` 任务，独立验证源码实现、测试覆盖、DoD 合规性和文档一致性。不以 `passing` 状态为充分证据，而是追溯到具体文件、行号和行为。

---

## 0. 总结

Phase 4 的机械质量门通过（fmt/clippy/test/doc 全绿），467 个测试函数覆盖全部 19 个任务，0 个测试依赖外部网络。

但 Phase 4 **不建议标记为完整退出**。

核心原因：Phase 4 交付了一个质量不错的**库层基底**（类型、trait、registry、discovery、测试夹具），但这个基底**没有接入产品运行时**。用户无法通过 CLI、配置或 TUI 使用 extension、skill、fragment、theme、package、session branching、custom provider 中的任何一项。RPC 并发模型存在结构性缺陷，文档严重滞后。

| 维度 | 状态 |
|------|------|
| 机械质量门 | 通过 |
| Ledger 任务状态 | 19/19 passing |
| `phase_exit.4` | **缺失**（仅 1-3 已记录） |
| Spec ss15 exit criteria | **代码层面满足，产品层面未满足** |
| 文档同步 | **严重滞后** |

---

## 1. 验证环境

| 检查项 | 结果 |
|--------|------|
| `cargo fmt --check --all` | 通过 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 通过 |
| `cargo test --workspace --all-targets` | 通过 |
| `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` | 通过 |

---

## 2. 逐任务审计

### 2.1 Task 4.1 -- RPC JSONL mode

**Ledger 声明：** 27 RPC tests pass；RPC 模式接受 stdin JSONL 命令，发出 stdout JSONL 响应和异步事件，支持 session/model/thinking/compaction/prompt/continue/abort 命令。

**实现位置：**

| 文件 | 行数 | 职责 |
|------|------|------|
| `crates/opi-coding-agent/src/rpc.rs` | ~432 | RPC runner、协议文档、JSONL I/O |
| `crates/opi-coding-agent/src/cli.rs` | ~106 | `--rpc` flag |
| `crates/opi-coding-agent/src/main.rs` | ~1082 | RPC 早期分支、`run_rpc()` |
| `crates/opi-coding-agent/tests/rpc_jsonl.rs` | ~547 | 单元 + 子进程测试 |

**公共 API：**

- `pub type RpcCommand = SdkCommand`
- `pub const RPC_SCHEMA_VERSION: u32 = SDK_SCHEMA_VERSION`
- `pub struct RpcRunner` -- `new()`, `async fn run() -> i32`

**协议细节确认：**

- 分隔符 `\n`，`\r` 在解析前剥离
- 空行跳过
- Ready header: `rpc_ready` + `schema_version: 2` + `mode: "rpc"` + `version`
- 10 个命令变体: prompt, continue, steer, follow_up, abort, set_model, set_thinking_level, compact, session_info, quit
- 响应通过 `SdkResponse` 统一；解析失败返回 `command: "parse"`, `success: false`
- 模块文档标记 **unstable 0.x**

**P0 -- RPC 主循环是单线程阻塞模型：**

`RpcRunner::run` 在处理 `prompt`/`continue` 时直接 `await self.harness.prompt().await`，在此期间：
1. 不读取 stdin -- `abort`/`steer`/`follow_up` 在运行中不可达
2. 不 drain 事件通道 -- agent 事件在 turn 结束后才批量刷出
3. 文档声称 "agent events stream asynchronously"，但实际上事件在 turn 间歇才写出

这意味着 RPC 协议文档中最核心的实时控制语义（mid-turn abort、运行时 steer、异步事件流）在当前实现中**不成立**。

**P0 -- `run_rpc()` 忽略 CLI 工具选择：**

`main.rs:257` 的 `run_rpc` 参数名为 `_tool_selection`，`rpc.rs:105` 固定使用 `ToolSelection::Default`。`opi --rpc --no-tools` 或 `--tools read,grep` 不生效，这是安全边界漏洞。

**P1 -- `set_model` 返回假成功：**

`rpc.rs:276` 调用 `harness.set_model(model)` -> `agent.set_model` 只设 `self.model = model` 字符串，不重建 provider、不校验 registry。跨 provider family 切换后，下次请求把新 model 字符串发给旧 provider。

**P1 -- `set_thinking_level` 是文档化的 no-op：**

`rpc.rs:284` 注释明确写着 "acknowledge, runtime config update not yet implemented"。

**测试覆盖 vs DoD：**

子进程测试覆盖 ready header、parse error、id correlation、session_info、空行、EOF、顺序元命令。**没有子进程测试发送 `prompt` 或测试 mid-flight abort**。子进程测试在 binary 不存在时 `return` 跳过。

**结论：** 基础 framing 和类型可用，核心交互语义未满足。

---

### 2.2 Task 4.2 -- SDK embedding surface

**Ledger 声明：** 38 SDK tests；Rust embedding API 暴露共享 SDK/RPC 命令和事件类型。

**实现位置：**

| 文件 | 行数 | 职责 |
|------|------|------|
| `crates/opi-agent/src/sdk.rs` | ~252 | 规范命令/响应/事件类型 |
| `crates/opi-agent/tests/sdk_embedding.rs` | ~477 | 解析、Agent+MockProvider 流 |
| `crates/opi-coding-agent/tests/sdk_embedding.rs` | ~75 | RPC<->SDK 类型一致性 |

**公共 API：**

- `SDK_SCHEMA_VERSION = 2`
- `SdkCommand` -- 10 个变体，`#[serde(tag = "type")]`，可选 `id`
- `SdkResponse` -- 仅 Serialize（不可 Deserialize，限制 round-trip 测试）
- `agent_event_to_value(&AgentEvent) -> serde_json::Value` -- **未在 crate root re-export**

**类型共享确认：** `rpc.rs` 中 `pub type RpcCommand = SdkCommand; pub const RPC_SCHEMA_VERSION: u32 = SDK_SCHEMA_VERSION;` -- 无重复。

**P2 -- DoD 偏差：** 测试覆盖 prompt/continue/abort/model/steer/follow_up 流，但 `compact` 和 `set_thinking_level` **只有解析测试**，无行为流测试。DoD 明确列出 compaction 和 thinking 流。

**P2 -- SDK 是类型+辅助函数，不是 turnkey runner：** embedder 需自行组装 `Agent`/`CodingHarness` 或使用 RPC。这是合理的设计选择，但 DoD 中 "Rust embedding API" 可能暗示更高层抽象。

**结论：** 类型层干净且不重复；行为覆盖有小缺口。

---

### 2.3 Task 4.3 -- settle opi-agent::Transport

**Ledger 声明：** Transport 已从公共 API 移除（Option A）。

**验证：**

- `crates/opi-agent/src/lib.rs` 无 `mod transport`、`trait Transport`、`pub use Transport`
- `crates/opi-agent/tests/transport.rs`（72 行）包含 2 个测试：
  - `core_types_accessible_without_transport` -- 编译通过即验证
  - `sdk_docs_do_not_claim_settled_transport` -- 编译时 guard
- `async-trait` 依赖已从 `opi-agent` 移除

**结论：** 完全满足 DoD。Settlement 干净。

---

### 2.4 Task 4.4 -- extension trait, lifecycle hooks, custom tools, custom commands, custom messages, extension state

**Ledger 声明：** 22 opi-agent tests + 4 opi-coding-agent integration tests。

**实现位置：**

| 文件 | 行数 | 职责 |
|------|------|------|
| `crates/opi-agent/src/extension.rs` | ~529 | Extension trait, ExtensionRegistry, CompositeHooks |
| `crates/opi-agent/tests/extensions.rs` | ~872 | 22 单元/行为测试 |
| `crates/opi-coding-agent/tests/extensions.rs` | ~447 | 4 agent-loop 集成测试 |

**公共 API：**

- `Extension` trait: 10 个方法（`name`, `tools`, `providers`, `model_overrides`, `on_before_tool_call`, `on_after_tool_call`, `on_event`, `on_command`, `serialize_state`, `restore_state`）
- `ExtensionRegistry`: 注册、收集、分发、状态、hook/event 包装
- `ExtensionError`: 5 个变体，使用 `thiserror`
- `ExtensionHookResult`: `Continue` / `Block { reason }`，`#[non_exhaustive]`

**生命周期 hook 排序（已验证）：**

1. Before tool: base hook -> 若 Deny 则停止 -> extensions 按注册顺序 -> 首个 Block 变 Deny
2. After tool: base hook (可 Replace) -> extensions 观察有效结果 -> 返回 base 结果
3. Events: 所有 extensions 收到每个 event（sync `on_event`）

**P0 -- 自定义消息 API 缺失：**

DoD 明确要求 "custom agent messages"。当前状态：
- 模块文档第 4 行提到 "custom agent messages"
- `Extension` trait **无方法**注入 `AgentMessage::Custom`
- `AgentMessage::Custom` 存在于 `message.rs`，但注入路径是 `AgentHooks::prepare_next_turn` -> `AgentLoopTurnUpdate.extra_messages`
- `CompositeHooks` **不将 `prepare_next_turn` 委托给 extensions**
- 没有 extension 测试覆盖自定义消息

这是 4.4 DoD 最大的 spec vs implementation 偏差。

**P1 -- `register()` 在 `wrap_hooks()` 后 panic：**

Arc 共享 guard 导致 panic，仅在注释中说明，公共方法签名未反映。

**P2 -- tool 名称碰撞无检测：**

`collect_tools()` 不检测跨 extension 的重名 tool。

**P0 -- Extension 基底未接入 harness/main.rs：**

生产代码中无 `ExtensionRegistry` 实例化或使用。

**结论：** Extension trait 设计合理，hook 排序正确，但自定义消息 API 缺失且基底未接入运行时。

---

### 2.5 Task 4.5 -- extension/resource loading strategy

**Ledger 声明：** 18 tests，discovery 覆盖项目/用户资源加载、元数据验证、extension 注册集成。

**实现位置：**

| 文件 | 行数 | 职责 |
|------|------|------|
| `crates/opi-coding-agent/src/resource.rs` | ~236 | Discovery 实现 |
| `crates/opi-coding-agent/tests/extension_resources.rs` | ~518 | 18 个测试 |

**优先级模型：** User (0) -> Project (1) -> Explicit (2)，高 precedence 覆盖低 precedence。

**P1 -- 文档与实现不匹配：** 模块文档声称 "Within a single layer, duplicate names produce an error"。实际代码同层 duplicate 静默 first-wins 去重，无测试覆盖同层 duplicate。

**P0 -- 未接入运行时：** `discover_extension_resources` 仅在测试中调用。CLI `--extension` 和 config `extensions.paths` 在文档中提及，但 `config.rs`/`cli.rs`/`harness.rs` 中不存在对应字段或逻辑。

**结论：** Discovery 逻辑本身可用，但是孤立的库代码。

---

### 2.6 Task 4.6 -- custom provider/model registration

**Ledger 声明：** 28 tests (20 opi-ai + 8 opi-coding-agent)。

**实现位置：**

| 文件 | 行数 | 职责 |
|------|------|------|
| `crates/opi-ai/src/registry.rs` | ~305 | Registry 实现 |
| `crates/opi-ai/tests/custom_provider_registration.rs` | ~441 | 20 个测试 |
| `crates/opi-coding-agent/tests/custom_provider_registration.rs` | ~285 | 8 个测试 |

**API 确认：**

- `register_provider()` -- 空 id 拒绝，同 id 静默替换（已文档化）
- `register_model()` -- 空 model id 拒绝，重复 override key 返回 `DuplicateModel`，可 shadow 内置 model
- `all_models()` -- 内置减去被 shadow 的 + 所有 override 条目

**P1 -- `--list-models` 绕过 `all_models()`：**

`main.rs` 中 `list_models` 手写枚举内置 provider builders，不经过 `ProviderRegistry::all_models()`。Extension 注册的 model 对 CLI 不可见。

**P1 -- harness 不使用 extension provider：**

`harness.rs` 接受预构建的 `Box<dyn Provider>`，无 extension provider 注册路径。

**结论：** Registry API 完整且测试充分，运行时集成缺失。

---

### 2.7 Task 4.7.1 -- skills with progressive discovery

**Ledger 声明：** 29 skill discovery tests。

**实现位置：** `crates/opi-coding-agent/src/skill.rs`（~461 行），`tests/skills_discovery.rs`（~549 行）

**P2 -- 朴素 YAML frontmatter 解析器：** 手写 `key: value` 行解析，不支持多行值、嵌套 key 或 `name: "foo: bar"` 含冒号的值。与 `prompt_fragment.rs` 共享 ~150 行近乎相同的代码。

**P2 -- 缺失 `---` 闭合时 body 为空：** `extract_body()` 在 frontmatter 未闭合时返回空字符串，无错误。

**P0 -- 未接入运行时。**

**结论：** Discovery 和 registry 逻辑可用，但解析器脆弱且运行时未连接。

---

### 2.8 Task 4.7.2 -- prompt fragments/templates with progressive discovery

**Ledger 声明：** 40 prompt_fragment tests。

**实现位置：** `crates/opi-coding-agent/src/prompt_fragment.rs`（~636 行），`tests/prompt_fragments.rs`（~703 行）

**API 亮点：** `FragmentArgument`（required/optional with defaults）、`expand_fragment_body()` 做 `{{arg}}` 占位符替换。

**P2 -- 展开中的小 UX 问题：** `MissingArgument` 错误中 fragment name 为空字符串。

**P1 -- DoD 偏差：** Ledger DoD 要求 "exposed as slash-style prompt commands and RPC command metadata"。`format_for_rpc_metadata()` 存在，但 `rpc/` 和 `interactive/` 中无消费代码。Substrate only。

**P0 -- 未接入运行时。**

---

### 2.9 Task 4.7.3 -- themes with progressive discovery

**Ledger 声明：** 36 theme_discovery tests + 21 theme_snapshots。

**实现位置：**

| 文件 | 行数 | 职责 |
|------|------|------|
| `crates/opi-coding-agent/src/theme_discovery.rs` | ~393 | Discovery/registry |
| `crates/opi-tui/src/theme.rs` | ~435 | Palette、parsing、built-ins |
| `crates/opi-coding-agent/tests/theme_discovery.rs` | ~695 | 行为测试 |
| `crates/opi-tui/tests/theme_snapshots.rs` | ~293 | Shell snapshots |

**P2 -- TUI 仍使用旧路径：** `interactive.rs` 通过 config `[defaults].theme` + `resolve_theme()` 直接加载内置主题，不经过 `ThemeRegistry`。Discovery 发现的自定义主题对用户不可达。

**P2 -- 名为 "monokai" 的自定义主题会 shadow 内置 monokai：** `ThemeRegistry::resolve_theme()` 先查 discovered 再查 built-in，无冲突警告。

**P0 -- 未接入运行时。**

---

### 2.10 Task 4.7.4 -- packages with progressive resource composition

**Ledger 声明：** 36 package_discovery tests。

**实现位置：** `crates/opi-coding-agent/src/package_discovery.rs`（~655 行），`tests/package_discovery.rs`（~938 行）

**双模式：** validated manifest（include lists + missing asset error）和 conventional directory（auto-discover）。

**P2 -- 无 package -> discovery 管线桥接：** `PackageResource::compose()` 产出 path list，但无代码将这些 path 注入 skill/fragment/theme discovery layers。

**P2 -- disabled 列表按名称跨类型匹配：** 一个 disabled 字符串可同时禁用同名的 skill 和 theme。

**P0 -- 未接入运行时。**

---

### 2.11 Task 4.8.1 -- permission gate extension/package example

**Ledger 声明：** 11 tests。

**实现位置：** `examples/permission-gate/`（package.toml + extension.toml）、`crates/opi-coding-agent/tests/permission_gate_example.rs`（~778 行）、`docs/extension-examples/permission-gate.md`

**验证：** PermissionPolicy (AllowAll/DenyAll/DenyList/AllowList)、`on_before_tool_call` gating、audit log、event observation、state serialization。文档明确声明 "example, not core policy"。

**`package.toml` 格式：** 使用 flat top-level keys -- **与 `PackageManifest::from_toml()` 兼容**。

**结论：** 合规。

---

### 2.12 Task 4.8.2 -- protected paths extension/package example

**Ledger 声明：** 14 tests。

**实现位置：** `examples/protected-paths/`、`tests/protected_paths_example.rs`（~1129 行）

**验证：** PathPolicy、path normalization、symlink traversal detection、bash cwd interaction。文档明确声明 example。

**`package.toml` 格式：** flat top-level keys -- **兼容**。

**结论：** 合规。

---

### 2.13 Task 4.8.3 -- sub-agent extension/package example

**Ledger 声明：** 10 tests。

**实现位置：** `examples/sub-agent/`、`tests/sub_agent_example.rs`（~890 行）

**P1 -- `package.toml` 使用 `[package]` 嵌套表：** `PackageManifest::from_toml()` 期望 top-level `name`，该 manifest 会解析失败。

**结论：** 测试行为覆盖充分，manifest schema 不兼容。

---

### 2.14 Task 4.8.4 -- plan mode extension/package example

**Ledger 声明：** 12 tests。

**实现位置：** `examples/plan-mode/`、`tests/plan_mode_example.rs`（~548 行）

**P1 -- `package.toml` 使用 `[package]` 嵌套表：** 同 4.8.3，与 parser 不兼容。

**结论：** 功能行为正确，manifest schema 不兼容。

---

### 2.15 Task 4.8.5 -- todo extension/package example

**Ledger 声明：** 16 tests。

**实现位置：** `examples/todo/`、`tests/todo_example.rs`（~652 行）

**P1 -- `package.toml` 使用 `[package]` 嵌套表：** 与 parser 不兼容。

**结论：** 功能行为正确，manifest schema 不兼容。

---

### 2.16 Task 4.8.6 -- MCP adapter extension/package example

**Ledger 声明：** 20 tests。

**实现位置：** `examples/mcp-adapter/`、`tests/mcp_adapter_example.rs`（~964 行）

**P1 -- `package.toml` 使用 `[package]` 嵌套表：** 与 parser 不兼容。

**结论：** 功能行为最丰富的 example（tool discovery、schema exposure、argument validation、resource metadata、cancellation），但 manifest schema 不兼容。

---

### 2.17 Task 4.9 -- session branching UI

**Ledger 声明：** 22 session_branching tests + 9 branch_picker snapshots。

**实现位置：**

| 文件 | 行数 | 职责 |
|------|------|------|
| `crates/opi-agent/src/session_branch.rs` | ~324 | 树重建 |
| `crates/opi-tui/src/branch_picker.rs` | ~235 | Picker widget/state |
| `crates/opi-agent/tests/session_branching.rs` | ~496 | 行为测试 |
| `crates/opi-tui/tests/session_branching_snapshots.rs` | ~157 | insta snapshots |

**SessionTree 算法确认：** 从 `parent_id` graph 构建；处理 linear、branched、compaction、orphan、cycle、invalid leaf。Active branch 由最后 Leaf entry 决定。

**P1 -- DoD 要求 "interactive TUI exposes session branch selection"：** `BranchPicker` widget 存在但 `interactive.rs` 中无引用。`BranchPickerOutcome` 未连接到任何事件循环。Widget 在产品中不可达。

**P2 -- branch 顺序非确定性：** roots 从 `HashSet` 迭代收集，多 root session 的 branch 顺序可能不稳定。

**P2 -- snapshot 使用手建 `BranchItem`：** 不经过 `SessionTree` 输出，widget 与数据层之间无集成测试。

**结论：** 数据层和 widget 层均可用，但未构成可达 workflow。

---

### 2.18 Task 4.10 -- streaming proxy

**Ledger 声明：** 25 streaming_proxy tests。

**实现位置：**

| 文件 | 行数 | 职责 |
|------|------|------|
| `crates/opi-agent/src/streaming_proxy.rs` | ~462 | Proxy engine、redactor、error |
| `crates/opi-agent/tests/streaming_proxy.rs` | ~692 | Mock handler 测试 |

**公共 API：** `ProxyEvent`、`ProxyHandler`（sync trait）、`ProxyConfig`（channel capacity + redaction toggle）、`StreamingProxy<H>`、`SecretRedactor`、`StreamingProxyError`

**framing 确认：** Strict JSONL、`\n`、每行 flush、header `proxy_ready` + `schema_version`

**backpressure：** bounded `sync_channel`，满时 drop + `tracing::warn`（RPC 用 unbounded tokio channel -- 策略不一致）

**P1 -- `StreamingProxyError::Cancelled` 变体是死代码：** 无代码路径构造它。

**P1 -- 文档声称 handler 收到 cancellation token，但 `ProxyHandler` trait 无 token 参数。**

**P1 -- `read_line` 是同步调用在 `async fn run` 中：** 阻塞 Tokio worker，cancel 不在阻塞读期间检查。

**P1 -- RPC vs proxy parse-error wire format 不一致：** RPC 返回 `{"type":"response", "command":"parse", ...}`，proxy 返回 `{"type":"proxy_error", "line_number":..., "raw":...}`。embedder 需要两个解析器。

**P2 -- `SecretRedactor` `eyJ` 前缀匹配可能 false-positive：** 不含长度检查，任何以 `eyJ` 开头的值都会被 redact。

**结论：** framing、backpressure、disconnect handling 可用且测试覆盖好；cancellation 和文档有偏差。

---

### 2.19 Task 4.11 -- web UI implementation

**Ledger 声明：** 53 web-ui tests + 9 RPC integration tests。

**实现位置：**

| 文件 | 行数 | 职责 |
|------|------|------|
| `crates/opi-web-ui/src/event.rs` | event parsing | WebUiEvent from RPC JSONL |
| `crates/opi-web-ui/src/state.rs` | state machine | ConversationState |
| `crates/opi-web-ui/src/components.rs` | component model | ChatMessage, ToolCallView, ThinkingBlock, StatusBar, ConversationView |
| `crates/opi-web-ui/src/render.rs` | HTML rendering | Render trait + XSS escaping |
| `crates/opi-web-ui/tests/web_ui.rs` | ~53 tests | parse/state/render/SDK round-trip |
| `crates/opi-coding-agent/tests/web_ui_rpc.rs` | ~9 tests | subprocess + mock JSON |

**`publish = false` 确认：** 保持。

**依赖：** `opi-ai`（**声明但未在 src/ 中使用** -- dead weight）、`opi-agent`（测试中使用）、serde/serde_json/thiserror。

**XSS 转义确认：** `escape_html()` 处理 `&`, `<`, `>`, `"`, `'`。已转义：message text、thinking content、tool name/result、status bar。

**P1 -- `SessionInfo` 和 `ModelChanged` 无 parse arm：** `event.rs` 中 `parse()` 对这两个变体无对应 wire type 匹配。只能手动构造。

**P1 -- `tool_call_*` stream events 映射到 `TextDelta`：** `MessageUpdate` 中的 `tool_call_start|delta|end` 被折叠为 `TextDelta`，可能把 tool-call JSON fragment 追加到 assistant text 而非结构化 tool UI。

**P1 -- `RpcResponse.data` 被丢弃：** `parse_rpc_response` 不读 `data` 字段。`session_info` response 中的 model/session data 不进入 UI state。`to_status_bar()` 中的 `self.model` 只由 `ModelChanged`/`SessionInfo` 设置 -- 永远读不到 RPC response 中的值。

**P2 -- Windows 子进程测试不可用：** `web_ui_rpc.rs` 固定查找 `target/debug/opi`，Windows 下是 `opi.exe`，测试静默跳过。

**P2 -- XSS 测试覆盖不足：** 仅 1 个显式 XSS 测试（message text）。thinking blocks、tool results、status bar、attribute injection 无 XSS 测试。

**P2 -- `opi-ai` 依赖未使用：** 建议移除或在 typed event 转换中使用。

**结论：** 有实质性实现（不再是 placeholder），但 event 消费不完整、XSS 覆盖不足、Windows CI 不可靠。

---

## 3. 跨领域分析

### 3.1 架构：已实现 vs 未连接

```
已实现（库层 + 测试）                     未连接到 opi 二进制
------------------------------          ---------------------------
Extension trait                   --->  CLI / config
ExtensionRegistry                 --->  Harness / Agent startup
CompositeHooks                    --->  Session JSONL extension state
discover_extension_resources      --->  Config `extensions.paths`
discover_skills                   --->  Interactive / RPC prompt context
discover_fragments                --->  Slash commands / RPC metadata
discover_themes                   --->  TUI theme resolution
discover_packages                 --->  Compose -> discovery pipeline
ProviderRegistry.register_*       --->  --list-models / build_provider
SessionTree + BranchPicker        --->  TUI overlay / session resume
StreamingProxy                    --->  External entry point
opi-web-ui event/state/render     --->  Browser app / RPC transport
```

Phase 4 exit criteria 要求 "third parties can compose and extend opi through RPC, SDK, extensions, skills, prompt fragments, themes, packages, and custom provider/model registration **without patching core crates**"。当前状态是第三方可以 fork 测试代码来使用这些能力，但不能通过 config/CLI/TUI 使用。

### 3.2 测试质量评估

| 指标 | 值 |
|------|-----|
| Phase 4 测试函数总数 | 467 |
| `#[test]` (sync) | 353 |
| `#[tokio::test]` (async) | 114 |
| 使用 MockProvider 的文件 | 9 |
| 使用 tempdir 的文件 | 9 |
| 依赖外部网络的文件 | 0 |
| 子进程测试（条件性跳过） | ~14 tests across rpc_jsonl + web_ui_rpc |

**优点：**

- 完全隔离，无网络依赖
- MockProvider 覆盖充分
- 每个 task 都有专属测试文件
- tempdir 用于文件系统 fixture

**风险：**

- 子进程测试在 binary 未构建时静默跳过 -- CI 必须先 `cargo build` 否则覆盖虚假
- Windows 下 `web_ui_rpc.rs` 固定路径导致测试不运行
- Snapshot 测试使用合成数据，不经过生产数据路径

### 3.3 安全审查

| 领域 | 状态 |
|------|------|
| 路径限制 | 工具层路径约束到 workspace root -- Phase 1-3 已有 |
| RPC 工具选择 | **漏洞**：`--rpc` 模式忽略 `--no-tools` / `--tools` |
| Secret redaction | StreamingProxy 有 SecretRedactor；RPC 无 redaction |
| XSS | `opi-web-ui` 有 `escape_html()`，但 coverage 不足 |
| Extension state | JSON 序列化，无加密或签名 |
| Symlink | protected-paths example 有 detection；resource discovery 无 |
| Package security | `canonicalize()` + `starts_with()` 验证；无集成 symlink-escape 测试 |

### 3.4 Spec ss15 Exit Criteria 合规矩阵

| Exit 条件 | 代码层 | 产品层 | 文档层 |
|-----------|--------|--------|--------|
| RPC JSONL with session/model/thinking/compaction commands | 部分（framing 可用，并发不成立） | `--rpc` flag 存在 | unstable 0.x in module docs |
| SDK over shared command/event model | 满足 | N/A（库 API） | unstable 0.x |
| Transport real/unstable/absent | 满足 | N/A | 更新 |
| Extensions (hooks, tools, commands, messages, state) | 部分（消息 API 缺失） | 未接入 | unstable 0.x |
| Resource loading (project/user) | 满足 | 未接入 | doc/code mismatch |
| Custom provider/model registration | 满足 | 未接入 --list-models / harness | unstable 0.x |
| Skills, prompt fragments, themes, packages | 满足 | 未接入 | crate READMEs 有，root docs 滞后 |
| Workflow features as extensions, not core | 满足 | 示例在测试中运行 | 文档明确 "not core" |
| Web UI consumes RPC/SDK events | 部分（tool_call/SessionInfo/data 消费不完整） | 无 browser app | crate README 准确，root stale |
| Session branching UI in TUI | 数据层+widget 满足 | Widget 不可达 | 无 |
| Streaming proxy | 满足（有 doc 偏差） | 无外部入口 | unstable 0.x |
| Agent-facing docs reflect Phase 4 | N/A | N/A | **未满足** |

### 3.5 文档状态

| 文件 | 状态 |
|------|------|
| `CHANGELOG.md [Unreleased]` | **空** -- Phase 4 无条目 |
| `CLAUDE.md` | **Phase 3 状态** -- 无 RPC/SDK/extension/discovery 描述 |
| `AGENTS.md` | **Phase 3 状态** -- 同上 |
| `README.md` | **`opi-web-ui` 仍称 placeholder**；"Still Not Implemented" 列出 skills/sub-agents/MCP/web UI |
| `README.zh.md` | **同 README.md** |
| `docs/opi-spec.md` ss0/ss4.1 | **仍称 Phase 3 complete**、`opi-web-ui -> opi-ai` only（实际还依赖 `opi-agent`） |
| `docs/opi-spec.zh.md` Phase 4 表 | **与英文版不同步**（中文版只列到 4.9） |
| crate-level READMEs | 基本准确（`opi-coding-agent` 有 RPC/skills/fragments 文档；`opi-web-ui` 准确） |

---

## 4. 问题汇总

### P0 -- 阻断项

| # | 问题 | 影响任务 | 影响 |
|---|------|----------|------|
| P0.1 | RPC 主循环单线程阻塞，mid-turn abort/steer/follow_up 不可达，事件不实时流出 | 4.1, 4.2, 4.11 | 核心交互语义不成立 |
| P0.2 | Extension/resource/skill/fragment/theme/package discovery 未接入产品运行时 | 4.4-4.7 | 第三方无法通过 config/CLI 使用 |
| P0.3 | RPC 忽略 CLI 工具选择（`_tool_selection`） | 4.1 | 安全边界漏洞 |
| P0.4 | Extension trait 缺少 custom messages API | 4.4 | DoD 明确要求但未实现 |

### P1 -- 高风险

| # | 问题 | 影响任务 |
|---|------|----------|
| P1.1 | `set_model` 返回成功但不重建 provider | 4.1 |
| P1.2 | `set_thinking_level` 是 documented no-op | 4.1 |
| P1.3 | 4 个 example `package.toml` 用 `[package]` 嵌套表，与 parser 不兼容 | 4.8.3-4.8.6 |
| P1.4 | `--list-models` 绕过 `ProviderRegistry::all_models()` | 4.6 |
| P1.5 | Session branching widget 未接入 TUI | 4.9 |
| P1.6 | Web UI `tool_call_*` 映射到 TextDelta | 4.11 |
| P1.7 | Web UI 缺少 SessionInfo/ModelChanged parse arm | 4.11 |
| P1.8 | Web UI 丢弃 RpcResponse.data | 4.11 |
| P1.9 | Prompt fragments 未作为 slash commands / RPC metadata 暴露 | 4.7.2 |
| P1.10 | `StreamingProxyError::Cancelled` 死代码 | 4.10 |
| P1.11 | Proxy 文档声称 handler 收到 cancel token，实际无 | 4.10 |
| P1.12 | resource.rs 文档声称同层 duplicate 报错，实际静默去重 | 4.5 |
| P1.13 | `register()` 在 `wrap_hooks()` 后 panic 无签名反映 | 4.4 |

### P2 -- 中风险

| # | 问题 | 影响任务 |
|---|------|----------|
| P2.1 | `phase_exit.4` 缺失 | 全局 |
| P2.2 | Windows 子进程测试路径不含 `.exe` | 4.1, 4.11 |
| P2.3 | XSS 测试仅覆盖 message text（1 个测试） | 4.11 |
| P2.4 | RPC vs proxy parse-error wire format 不一致 | 4.1, 4.10 |
| P2.5 | `SecretRedactor` eyJ prefix false-positive 风险 | 4.10 |
| P2.6 | 阻塞 I/O 在 Tokio runtime 中（RPC stdin + proxy read_line） | 4.1, 4.10 |
| P2.7 | TUI 不使用 ThemeRegistry，仍走旧 resolve_theme 路径 | 4.7.3 |
| P2.8 | package -> skill/fragment/theme discovery 管线未桥接 | 4.7.4 |
| P2.9 | `collect_tools()` 无跨 extension tool 名称碰撞检测 | 4.4 |
| P2.10 | SDK compact/thinking 仅有 parse 测试，无行为流测试 | 4.2 |
| P2.11 | `opi-ai` 依赖在 `opi-web-ui` src 中未使用 | 4.11 |
| P2.12 | Snapshot 测试用合成 BranchItem，不经 SessionTree 输出 | 4.9 |
| P2.13 | Branch 顺序在多 root 时非确定性（HashSet 迭代） | 4.9 |
| P2.14 | disabled 列表按名称跨 ResourceKind 匹配 | 4.7.4 |
| P2.15 | skills/fragments ~150 行重复 YAML frontmatter 解析代码 | 4.7.1, 4.7.2 |

### P3 -- 低风险

| # | 问题 |
|---|------|
| P3.1 | `streaming_proxy.rs` 用手写 `Display+Error` 而非 `thiserror` |
| P3.2 | `agent_event_to_value` 未在 `opi_agent` crate root re-export |
| P3.3 | `SdkResponse` 不可 Deserialize（限制 round-trip 测试） |
| P3.4 | CJK/wide char 在 BranchPicker 中每字符按 width 1 计算 |
| P3.5 | Skill `extract_body()` 在 frontmatter 未闭合时返回空字符串无错误 |
| P3.6 | `expand_fragment_body` 中 MissingArgument 的 fragment name 为空字符串 |
| P3.7 | `ThinkingBlock` 未在 `opi-web-ui` crate root re-export |

---

## 5. 质量较好的部分

- **全量机械质量门通过**（fmt/clippy/test/doc 零警告）
- **SDK 类型不重复** -- RPC 通过 type alias 共享 `SdkCommand`/`SdkResponse`
- **Transport settlement 干净** -- 移除而非留 stub，测试文档化决策理由
- **Extension trait 设计合理** -- hook 排序、`#[non_exhaustive]`、`thiserror` error types、state serialization 均遵循项目约定
- **Progressive discovery 一致性** -- 5 种资源类型共享 `DiscoveryLayer` + precedence collision 模式
- **467 个完全隔离的测试** -- 零网络依赖，MockProvider/tempdir 使用正确
- **Example extensions 清晰地声明 "not core"** -- 文档和代码均无歧义
- **opi-web-ui 不再是 placeholder** -- 有实质性 event parser、state machine、component model、HTML renderer（尽管仍标 `publish = false`）

---

## 6. 建议修复顺序

### 第一优先级 -- 解除阻断

1. **RPC 并发重写**：将 `RpcRunner::run` 改为 `tokio::select!` 协调 agent task + stdin command task + event drain task。运行中可处理 abort/steer/follow_up；事件实时流出。补 MockProvider 慢响应 + mid-flight abort/steer 的子进程测试。
2. **传递 `ToolSelection`**：在 `run_rpc` 和 `RpcRunner::new` 中使用 CLI 传入的 `tool_selection`。补 `--rpc --no-tools` 和 `--rpc --tools read,grep` 的子进程测试。
3. **Extension runtime wiring**：在 config 中增加 `[extensions]`/`[packages]` 段；在 harness startup 调用 discovery -> registry -> `wrap_hooks()`/`collect_tools()`/`collect_providers()`。至少让一个 example extension 能通过 config 加载到真实 agent loop 中。
4. **Custom messages API**：在 `Extension` trait 增加 `prepare_messages()` 或让 `CompositeHooks` 委托 `prepare_next_turn` 到 extensions。

### 第二优先级 -- 修复高风险

5. `set_model` 做 provider registry 校验 + rebuild（或限制为同 provider 内切换并在跨 provider 时返回 error）。
6. `set_thinking_level` 实现 runtime config 更新或从协议中标记 unsupported。
7. 统一 4 个 example `package.toml` schema 为 flat top-level keys。
8. `--list-models` 使用 `ProviderRegistry::all_models()`。
9. `BranchPicker` 接入 TUI（如 `/branch` 命令或 session resume flow）。
10. `opi-web-ui`: 修复 tool_call mapping、增加 SessionInfo/ModelChanged parse arm、消费 RpcResponse.data。

### 第三优先级 -- 文档同步

11. `CHANGELOG.md [Unreleased]` 补全 Phase 4 条目。
12. 更新 `CLAUDE.md`、`AGENTS.md` -- RPC 模式、SDK、extension、discovery、web-ui 不再是 placeholder。
13. 更新 root `README.md`/`README.zh.md` -- 移除 "Still Not Implemented" 中已实现项、更新 workspace dependency 图、更新 `opi-web-ui` 描述。
14. 同步 `docs/opi-spec.md` ss0/ss4.1 和 `docs/opi-spec.zh.md` Phase 4 表。

### 第四优先级 -- 关闭阶段

15. 修复 Windows 子进程测试路径（使用 `CARGO_BIN_EXE_opi` 或追加 `.exe`）。
16. 补 XSS 测试（thinking、tool results、status bar、attribute injection）。
17. 统一 RPC/proxy parse-error wire format。
18. 删除 `StreamingProxyError::Cancelled` 死代码或实际构造它。
19. 修正 Proxy 文档关于 handler cancellation token 的描述。
20. 修正 resource.rs 同层 duplicate 的文档。
21. 当以上项关闭后，在 `opi-impl-state.json` 写入 `phase_exit.4`。

---

## 7. 审计结论

Phase 4 交付了一个设计合理、测试覆盖充分的**可扩展性库基底**。Extension trait、progressive discovery、SDK 类型、streaming proxy 的 API 设计体现了对 pi 0.75.3 语义和 opi spec 的深入理解。467 个完全隔离的测试在机械层面是高质量的。

但 Phase 4 的 **产品级 exit criteria 未满足**：

- 第三方无法通过 config/CLI 使用任何新能力（除 `--rpc` 的基础 framing）
- RPC 核心交互语义（mid-turn abort、实时事件流）在当前并发模型下不成立
- Extension trait 缺少 DoD 要求的 custom messages API
- 文档严重滞后，与实际状态存在多处矛盾

建议将 Phase 4 状态调整为 **"substrate complete, runtime integration and documentation pending"**，优先修复 P0 阻断项，再进行最终验收并写入 `phase_exit.4`。
