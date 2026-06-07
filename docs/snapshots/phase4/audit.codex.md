# Phase 4 Codex 审计报告

审计日期：2026-06-05
审计对象：`docs/snapshots/phase4/opi-impl-state.json` 与当前工作树
审计方法：按 `$grill-me` 的方式把 Phase 4 的交付声明拆成可证伪问题，并用源码、测试和少量真实命令验证。`opi-impl-state.json` 中的 `passing` 只作为线索，不作为充分证据。

## 总结

Phase 4 的机械质量门通过，但不建议把 Phase 4 标记为完整退出。

主要原因是：RPC/SDK/扩展/包/Web UI 的不少能力已经有类型、测试夹具或组件层代码，但关键路径没有接到真实运行时入口，部分 RPC 命令返回成功但没有执行承诺语义，示例包并非真实可加载包，Web UI 对真实 RPC 响应的消费也不完整。

## 验证结果

| 检查项 | 命令 | 结果 |
| --- | --- | --- |
| 格式 | `cargo fmt --check --all` | 通过 |
| 全量测试 | `cargo test --workspace --all-targets` | 通过 |
| Clippy | `cargo clippy --workspace --all-targets -- -D warnings` | 通过 |
| 文档 | `$env:RUSTDOCFLAGS='-D warnings'; cargo doc --workspace --no-deps` | 通过 |
| Web UI RPC 子进程测试 | `cargo test -p opi-coding-agent --test web_ui_rpc web_ui_conversation_renders_from_rpc_output -- --nocapture` | 测试进程返回通过，但实际跳过：Windows 下查找 `target/debug/opi`，而真实二进制是 `target/debug/opi.exe` |
| RPC 空闲命令 | `opi.exe --rpc --model anthropic:claude-sonnet-4` + `session_info` / `set_model` / `quit` | 可返回 ready、response 和 session_info data |

额外确认：`opi-impl-state.json` 中 Phase 4 任务均为 `passing`，但 `phase_exit` 只有 `1`、`2`、`3`，没有 `4`。

## P0 阻断项

### P0.1 RPC 主循环不是运行中双向协议

Phase 4 声明 RPC 支持 prompt、continue、abort、steer、follow_up 等命令，并作为 Web UI/SDK 的双向控制面。但当前 `RpcRunner::run` 在读到 `prompt` 或 `continue` 后直接 `await self.harness.prompt(...)` / `await self.harness.continue_(...)`，这期间主循环不再读取 stdin。

证据：

- `crates/opi-coding-agent/src/rpc.rs:156` 在循环前克隆一次 `cancel_token`。
- `crates/opi-coding-agent/src/rpc.rs:219` 处理 `prompt`。
- `crates/opi-coding-agent/src/rpc.rs:228` 直接等待 `self.harness.prompt(&message).await`。
- `crates/opi-coding-agent/src/rpc.rs:233` 处理 `continue`。
- `crates/opi-coding-agent/src/rpc.rs:242` 直接等待 `self.harness.continue_(&message).await`。
- `crates/opi-coding-agent/src/rpc.rs:257`、`363`、`371` 才处理 `abort`、`steer`、`follow_up`，但这些分支只有在前一个 agent 调用结束后才会被读到。

影响：

- `abort` 不能在 provider streaming 或工具执行期间实时生效。
- `steer` / `follow_up` 不能在 agent 运行期间排队，只能在当前 prompt 结束后处理。
- `running` 分支中的 “agent is already running” 基本不可达，因为循环被同步占住。
- Web UI 或外部 SDK 无法构建可靠的实时控制面。

结论：4.1 RPC JSONL mode 和 4.2 SDK embedding surface 的核心交互语义未满足。

### P0.2 扩展、资源、skills、fragments、themes、packages 没有接入产品运行时

当前代码包含 discovery/registry 层，但 CLI、配置加载、provider 构建和 TUI/非交互运行时没有实际加载第三方包或扩展。

证据：

- `crates/opi-coding-agent/src/config.rs:22` 的 `OpiConfig` 只包含 defaults、thinking、providers、keybindings、retry、compaction，没有 extensions/packages 配置。
- `crates/opi-coding-agent/src/main.rs:446` 的 `build_provider` 仍按内置 provider id 手写 `match`。
- `crates/opi-coding-agent/src/main.rs:766` 的 `list_models` 也手写枚举内置 provider。
- `crates/opi-coding-agent/src/package_discovery.rs:493`、`skill.rs:325`、`prompt_fragment.rs:410`、`theme_discovery.rs:271`、`resource.rs:175` 定义 discovery 函数，但在 `crates/opi-coding-agent/src` 的生产路径没有调用。
- `crates/opi-coding-agent/README.md:389`、`:439` 文档提到 `extensions.paths`，但配置结构不存在该字段。

影响：

- 第三方无法通过配置或 CLI 加载 extension/package。
- 自定义 provider/model registry 不能影响 `opi --list-models` 或实际 agent provider。
- 4.4 到 4.7 的 substrate 更像库级能力和测试夹具，不是终端产品能力。

结论：Phase 4 exit criteria 中 “third parties can compose and extend opi ... without patching core crates” 未满足。

### P0.3 RPC `set_model` 和 `set_thinking_level` 返回成功但没有实现承诺语义

`set_model` 只更新 agent 内部 model 字符串，不重建 provider，也不走 provider registry 校验。实测在 Anthropic-backed runner 中发送 `set_model openai:gpt-4o` 会返回成功，并且 `session_info` 随后显示 model 为 `openai:gpt-4o`。

证据：

- `crates/opi-coding-agent/src/rpc.rs:276` 调用 `self.harness.set_model(model)`。
- `crates/opi-coding-agent/src/harness.rs:344` 继续委托到 `agent.set_model`。
- `crates/opi-agent/src/agent.rs:180` 的 `set_model` 只是 `self.model = model`。
- `crates/opi-coding-agent/src/rpc.rs:284` 的 `set_thinking_level` 注释明确写着目前只是 acknowledge，完整 runtime config 更新未实现。

影响：

- 切换 provider family 后，下一个请求会把 OpenAI model 字符串发给 Anthropic provider。
- Web UI/SDK 收到的是假成功，无法知道能力未生效。
- thinking level 不会改变后续 agent loop 配置。

结论：4.1 的 model/thinking commands 和 4.2 的 SDK 控制能力未满足。

### P0.4 RPC 模式忽略 CLI 工具选择

RPC 入口接收了 tool selection，但参数名为 `_tool_selection`，随后没有传给 `RpcRunner`。

证据：

- `crates/opi-coding-agent/src/main.rs:257` 的 `run_rpc` 接收 `_tool_selection: ToolSelection`。
- `crates/opi-coding-agent/src/rpc.rs:105` 固定使用 `ToolSelection::Default`。

影响：

- `opi --rpc --no-tools` 仍会按默认非交互工具集创建 runtime。
- `opi --rpc --tools read,grep` 不能收窄工具暴露面。
- 外部 RPC client 无法依赖 CLI policy 作为安全边界。

结论：RPC 模式没有继承现有工具选择语义，属于安全和集成阻断项。

## P1 高风险问题

### P1.1 示例包不是可直接加载的真实包

部分 `examples/*/package.toml` 使用 `[package]` 和 `[package.extensions]` 表结构，而 parser 期望 top-level `name`、`description`、`extensions` 等字段。

证据：

- `crates/opi-coding-agent/src/package_discovery.rs:135` 定义 `TomlPackageFile`。
- `crates/opi-coding-agent/src/package_discovery.rs:136`、`:139` 期望 top-level `name` 和 `extensions`。
- `examples/mcp-adapter/package.toml:1` 使用 `[package]`。
- `examples/mcp-adapter/package.toml:6` 使用 `[package.extensions]`。
- `examples/plan-mode/package.toml:1`、`examples/sub-agent/package.toml:1`、`examples/todo/package.toml:1` 有同类结构。
- `examples/mcp-adapter/README.md:40`、`examples/plan-mode/README.md:41`、`examples/sub-agent/README.md:41`、`examples/todo/README.md:41` 说明实际 Rust 实现位于测试文件。

影响：

- 用户不能把这些示例当作真实 package 安装或加载。
- 4.8 系列 “demonstrable as extensions/packages” 主要由测试文件证明，不是产品级示例。

### P1.2 Session branching UI 只是 widget，未接入交互式 TUI

`opi-tui` 中有 `BranchPicker`，但 `opi-coding-agent` 的交互式路径没有引用它。

证据：

- `crates/opi-tui/src/branch_picker.rs:112` 定义 `BranchPicker` widget。
- `crates/opi-coding-agent/src/interactive.rs:460` 只处理 `/session`。
- `crates/opi-coding-agent/src/interactive.rs:462` 进入的是通用 `session_picker_items`。
- 搜索 `BranchPicker` 在 `crates/opi-coding-agent/src` 中无生产引用。

影响：

- 用户不能在 TUI 中选择 session branch tip。
- 4.9 更接近 TUI 组件交付，不是可达 UI workflow。

### P1.3 Web UI 对真实 RPC 响应只做部分消费

`opi-web-ui` 有状态机和组件，但真实 RPC `response` 中的 `data` 被 parser 丢弃，因此 session/model/compaction 的真实响应路径没有进入 Web UI state。

证据：

- `crates/opi-web-ui/src/event.rs:23` 的 `RpcResponse` 只有 command、success、id、error。
- `crates/opi-web-ui/src/event.rs:93`、`:99` 定义了 `SessionInfo` 和 `ModelChanged` 事件。
- `crates/opi-web-ui/src/event.rs:125` 把真实 RPC `"response"` 交给 `parse_rpc_response`。
- `crates/opi-web-ui/src/event.rs:163` 的 `parse_rpc_response` 不读取 `data`。
- `crates/opi-coding-agent/src/rpc.rs:346` 的 `session_info` 使用 `success_with_data` 返回 model/session data。
- `crates/opi-coding-agent/tests/web_ui_rpc.rs:17` 手写二进制路径，`tests/web_ui_rpc.rs:21` 固定追加 `opi`，Windows 下不会找到 `opi.exe`。

影响：

- Web UI state 中的 `SessionInfo`、`ModelChanged` 主要可由 synthetic event 驱动，不是由真实 RPC response 驱动。
- Windows 上 Web UI 子进程集成测试会跳过，不能证明 4.11。
- `compact` response data、`set_model` 后模型状态和 `session_info` data 没有被统一消费。

### P1.4 文档与实现状态漂移

根 README 和 spec 仍把 `opi-web-ui` 描述为 placeholder；另一部分 README 又描述了 Phase 4 extension discovery 能力。

证据：

- `README.md:16` 和 `README.zh.md:16` 仍说 `opi-web-ui` 是 placeholder。
- `README.md:42` 和 `README.zh.md:42` 仍说 `opi-web-ui` 是 reserved web chat component crate。
- `docs/opi-spec.md:33`、`:723` 仍描述 `opi-web-ui` deferred/placeholder。
- `docs/opi-spec.md:1108` 之后的英文 Phase 4 表包含 4.9 session branching UI、4.10 streaming proxy、4.11 web UI。
- `docs/opi-spec.zh.md:1044` 的中文 Phase 4 表仍只有 `4.9 Web UI 实现`，与英文版本不同步。

影响：

- 用户无法从文档判断 Phase 4 到底已交付、部分交付，还是仍处于未来规划。
- 中文文档与英文文档的 Phase 4 范围不一致。

## P2 中风险问题

### P2.1 `phase_exit.4` 缺失

`opi-impl-state.json` 记录了 Phase 4 各 task 的 `passing`，但 `phase_exit` 下没有 `4`。这会让后续自动化无法区分 “任务测试已过” 和 “阶段审计退出已过”。

### P2.2 Web UI 是 Rust HTML 组件层，不是可运行浏览器应用

`opi-web-ui` 当前更像 embeddable component/state/parser crate。若 Phase 4 的解释是 “可嵌入组件层消费 RPC/SDK event”，这个范围可以接受；若解释是 “真实可运行 Web UI”，则还缺少 app shell、RPC transport wiring、browser verification 和用户交互路径。

### P2.3 Streaming proxy 需要独立运行验证

Phase 4 snapshot 将 streaming proxy 标为 passing，但本轮审计没有看到它被 CLI/Web UI 作为真实外部入口使用。建议补一个端到端验证：外部 client 经 proxy 发 prompt、接收 stream、发送 abort/steer，并确认事件顺序。

## 已完成且质量较好的部分

- 全量 `fmt`、test、clippy 和 docs gate 通过。
- RPC JSONL 基础 framing、ready header、response correlation 和 session_info 输出已存在。
- SDK command/response/event 类型和 RPC 协议有统一基础。
- `Transport` stub 已不再作为虚假稳定 API 暴露。
- extension、resource、skill、prompt fragment、theme、package 的 discovery/registry 层有明显测试覆盖。
- provider registry 类型已存在，可作为修复 custom provider/model runtime wiring 的基础。
- `opi-web-ui` 的 HTML escaping、状态机和组件层可以作为后续真实 Web UI 的基础。

## 建议修复顺序

1. 重写 RPC 主循环为真正并发：agent run task、stdin command task、event drain task 用 `select` 协调；运行中可处理 abort/steer/follow_up；每次 run 使用当前 cancel token；补慢 provider/mock tool 的实时 abort/queue 测试。
2. 把 `ToolSelection` 传入 `RpcRunner`，补 `--rpc --no-tools`、`--rpc --tools read,grep` 的 subprocess 测试。
3. 实现 `set_model` 的 provider registry 校验和 provider rebuild，或明确限制为同 provider model 切换；失败时返回 error，不返回假成功。
4. 实现 `set_thinking_level` 的 runtime config 更新，或从协议中移除/标记 unsupported。
5. 增加 extension/package 配置和 CLI 入口，把 discovery 结果注册进 harness：tools、hooks、slash commands、messages、state、providers、themes、skills、fragments 都要有真实运行时路径。
6. 统一 `examples/*/package.toml` schema，把示例实现从测试文件迁出到可加载示例 crate/package，保留测试只做验证。
7. 把 `BranchPicker` 接入 TUI，例如 `/branch` 或 session resume flow；补可达 workflow 测试或 snapshot。
8. 修复 `opi-web-ui` 对真实 RPC `response.data` 的解析，覆盖 session_info、set_model、compact/thinking 事件；Windows 下使用 `CARGO_BIN_EXE_opi` 或给路径补 `.exe`。
9. 同步 `README.md`、`README.zh.md`、`docs/opi-spec.md`、`docs/opi-spec.zh.md`，明确 Phase 4 的真实状态。
10. 以上阻断项关闭后，再在 `opi-impl-state.json` 写入 `phase_exit.4`。

## 审计结论

当前状态可以描述为：Phase 4 的多个底层模块和测试夹具已经落地，机械质量门通过；但 Phase 4 的产品级退出条件未满足。建议把 Phase 4 状态调整为 “implementation complete, audit failed / exit blocked”，并优先修复 P0 项后再进行最终验收。
