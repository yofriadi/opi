# Phase 3 系统审计报告

审计日期：2026-05-27  
审计模型：GPT-5.5  
审计范围：`docs/snapshots/phase3/opi-impl-state.json` 中 Phase 3 全部任务，覆盖 `opi-ai`、`opi-agent`、`opi-coding-agent`、`opi-tui`

## 执行摘要

`docs/snapshots/phase3/opi-impl-state.json` 记录 Phase 3 共 13 个任务全部为 `passing`。任务级证据显示 Phase 3 完成了大量实质性建设：AWS Bedrock、Azure OpenAI、Google Vertex provider，图像输入和图像工具结果协议，终端图像渲染组件，AGENTS.md/CLAUDE.md context loading，pi-style 工具选择与安全 hook，find/ls 工具，shell completions，SelectList 模糊选择器，proxy 原语和共享 HTTP client/pooling。

本次审计结论为：**有条件通过 / 不建议视为 Phase 3 runtime-complete**。

主要原因不是缺少模块，而是若干 Phase 3 DoD 明确要求的用户路径和运行时接线尚未闭环：

- `--list-models` 在 spec 和 3.1/3.2/3.3 DoD 中被要求，但 CLI 未实现。
- proxy 支持已有库层和配置解析测试，但 `build_provider` 没有把 `[providers.*.proxy]` 或 env proxy 接入 provider HTTP client。
- `--image`、multimodal prompt API、TUI 用户图像展示路径未贯通，图像输入主要停留在协议/serde/fixture 层。
- Kitty/iTerm/Sixel 终端图像 escape generation 存在格式或桩实现风险。
- SelectList 组件和 picker bridge 已实现，但未接入真实 interactive TUI 的 model/session picker 流程。
- `phase_exit.3` 缺失，ledger evidence 字段、commit hash、测试计数和 evaluator 记录不一致。

因此，Phase 3 可以记录为“任务级实现与验证证据大体成立，核心基础设施已落地”；但若以“企业 provider、图像输入/输出、proxy、picker 等用户路径全部可用”为阶段退出标准，仍应保留 Critical/High 风险项，并在发布 0.4.0 或宣称 Phase 3 完整前修复。

## 审计依据

### 状态文件

- `docs/snapshots/phase3/opi-impl-state.json`
  - `current_phase`: `3`
  - Phase 3 任务数量：13
  - 任务状态：13/13 `passing`
  - `phase_exit.3`: 缺失
  - 阶段末任务提交：`3.3` 记录 `verified_at_commit: e079e33`

### 规范与源码范围

- `docs/opi-spec.md`
- `CHANGELOG.md`
- `crates/opi-ai`
- `crates/opi-agent`
- `crates/opi-coding-agent`
- `crates/opi-tui`

### 核对的主要测试证据

状态文件记录的主要验证证据包括：

- `cargo fmt --check --all`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace --all-targets`
- `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`
- 任务级测试：
  - `crates/opi-ai/tests/bedrock_fixtures.rs`
  - `crates/opi-coding-agent/tests/bedrock_provider_wiring.rs`
  - `crates/opi-ai/tests/azure_openai_fixtures.rs`
  - `crates/opi-coding-agent/tests/azure_openai_provider_wiring.rs`
  - `crates/opi-ai/tests/vertex_fixtures.rs`
  - `crates/opi-coding-agent/tests/vertex_provider_wiring.rs`
  - `crates/opi-ai/tests/image_input.rs`
  - `crates/opi-agent/tests/image_input_session.rs`
  - `crates/opi-coding-agent/tests/image_input_cli.rs`
  - `crates/opi-coding-agent/tests/image_input_json_mode.rs`
  - `crates/opi-ai/tests/output_content_image.rs`
  - `crates/opi-agent/tests/image_tool_results.rs`
  - `crates/opi-coding-agent/tests/image_tool_results_json.rs`
  - `crates/opi-tui/tests/terminal_image_rendering.rs`
  - `crates/opi-coding-agent/tests/terminal_image_integration.rs`
  - `crates/opi-coding-agent/tests/agents_md_context.rs`
  - `crates/opi-coding-agent/tests/tool_selection.rs`
  - `crates/opi-coding-agent/tests/safety_hooks.rs`
  - `crates/opi-coding-agent/tests/find_tool.rs`
  - `crates/opi-coding-agent/tests/ls_tool.rs`
  - `crates/opi-coding-agent/tests/shell_completions.rs`
  - `crates/opi-tui/tests/select_list.rs`
  - `crates/opi-coding-agent/tests/picker_integration.rs`
  - `crates/opi-ai/tests/proxy_support.rs`
  - `crates/opi-coding-agent/tests/proxy_config.rs`
  - `crates/opi-ai/tests/connection_pooling.rs`

本次审计以只读源码审查、状态文件证据核对和测试路径核对为主，未重新执行完整测试套件。

## Phase 3 状态核对

| 任务范围 | 状态文件结论 | 审计结论 |
| --- | --- | --- |
| 3.1 AWS Bedrock provider | `passing` | 条件通过。SigV4、fixture、模型族路由和凭据解析基础扎实；但 `verified_at_commit` 为空，缺 `--list-models`、`build_provider` 端到端、完整 ambient credential chain、runtime proxy 接线和 session/snapshot 脱敏证据。 |
| 3.2 Azure OpenAI provider | `passing` | 条件通过。deployment URL、api-version、api-key header、fixture 与 wiring 测试较完整；缺 `--list-models`、runtime proxy 接线和 `Provider::stream` wiremock lifecycle 覆盖。 |
| 3.3 Google Vertex provider | `passing` | 条件通过。Vertex URL、Bearer token、Gemini SSE 复用和 registry 测试存在；缺 service-account/ADC offline 路径、`--list-models`、runtime proxy 接线和 HTTP lifecycle 覆盖。 |
| 3.4 image input | `passing` | 应降级为 partial。公共协议、provider serialization、session/JSON mode serde 覆盖较好；但 `--image` 未进入 runtime，Agent prompt API 仍是纯文本，TUI 用户图像输入不可达，size limit 和 capability gating 不完整。 |
| 3.5 image tool results | `passing` | 条件通过。`OutputContent::Image`、ToolResult、JSON mode 和 serde 基础成立；tool-result image 的完整 session writer/replay/tree reconstruction 和 unsupported fallback 测试仍偏薄。 |
| 3.6 terminal image rendering | `passing` | 应降级为 partial。fallback 占位和 snapshot 覆盖存在；但 Kitty/iTerm escape 格式可疑，Sixel 更接近桩实现，用户图像输入 TUI 展示路径未闭环，集成测试没有充分覆盖生产 interactive 路径。 |
| 3.7 AGENTS.md / CLAUDE.md context loading | `passing` | 条件通过。cwd/ancestor 优先级、AGENTS before CLAUDE、resume reread 和 MockProvider E2E 证据较好；但全局 config directory 在生产 harness 路径中未接线，非 git 工作区向上遍历边界需收紧。 |
| 3.8 pi-style tool selection and safety hooks | `passing` | 条件通过。CLI flags、allowlist、`--no-tools`、`--no-builtin-tools`、hook deny、JSON/session audit 覆盖较好；但无真正交互确认 UI，config precedence/conflict 仅在 DoD 中存在，生产配置未实现对应字段。 |
| 3.9 find / ls built-in tool parity | `passing` | 基本通过。find/ls 工具、workspace validation、gitignore、hidden files、bounded output、traversal rejection 测试较好；存在输出风格和截断提示的小问题。 |
| 3.10 shell completions | `passing` | 基本通过。`--generate-completion` 覆盖 bash/zsh/fish/powershell/elvish；测试依赖 `target/release/opi`，与普通 `cargo test` debug 二进制路径不完全一致。 |
| 3.11 fuzzy model/session picker | `passing` | 不满足 DoD。`opi-tui` SelectList、fuzzy state 和 snapshot 成立，`picker.rs` bridge 有测试；但 `interactive.rs` 未集成 picker overlay/keybinding，真实 TUI 里不能选择模型或 session。 |
| 3.12 proxy support | `passing` | 不满足 runtime DoD。`HttpClientBuilder`、proxy env/config 解析和脱敏函数存在；但 `build_provider` 未把 proxy 配置或 env proxy 接入实际 provider HTTP client。 |
| 3.13 connection pooling tuning | `passing` | 条件通过。共享 `HttpClient`、pool 参数和 Arc 复用基础存在；缺热路径计数器/benchmark 证明，企业 provider 复用覆盖不完整，CLI 仍按 provider 各自创建 client。 |

## 分域审计

### `opi-ai`

`opi-ai` 是 Phase 3 变更最密集的 crate。企业 provider 方向正确：Bedrock 有模型族路由、SigV4 signing、Converse Stream fixture；Azure 通过 OpenAI-compatible adapter 处理 deployment URL、api-version 和 `api-key`；Vertex 复用 Gemini wire protocol 并构造 `projects/{project}/locations/{location}` endpoint。`HttpClient` 抽象、pooling 参数、proxy redaction、image input/output 协议和 provider serialization 测试也已经落地。

覆盖良好的方面包括：

- Bedrock SigV4 有确定性 signing 单测。
- Bedrock/Azure/Vertex fixture 覆盖 text、tool call、usage、provider error 和错误映射。
- `InputContent::Image`、`OutputContent::Image`、`ImageSource`、`MediaType` 的 serde 形状稳定。
- `image_provider_serialization.rs` 覆盖多个 provider 的图像 request body 序列化。
- `HttpClientBuilder` 集中设置连接池参数，避免每个 provider 分散 HTTP 配置。

主要风险集中在 runtime 接线和 DoD 范围：

- `--list-models` 未实现，使企业 provider 的 registry/model metadata 用户路径缺失。
- Bedrock/Azure/Vertex 的真实 `Provider::stream()` HTTP 路径缺少 wiremock lifecycle 测试，现有覆盖主要是 fixture mapper 或 SSE parser。
- proxy 只在库层和配置解析层存在，未进入 `build_provider` 创建 provider 的路径。
- Vertex DoD 要求 service-account/offline OAuth，但当前证据主要是静态 access token 注入。
- Bedrock DoD 要求 ambient AWS credential chain，但实现证据更接近 config/env/profile 文件链，未覆盖 IMDS/SSO/`credential_process`。
- provider secret redaction 测试多集中在 `Debug` 或 redaction 函数，缺 session JSONL、NDJSON、snapshot、错误路径层面的系统覆盖。

审计结论：`opi-ai` 的协议和 fixture 层完成度高，但企业 provider 与 proxy/pooling 的 CLI runtime 闭环还不足。

### `opi-agent`

Phase 3 在 `opi-agent` 的重点是图像工具结果能作为结构化内容进入 `ToolResultMessage`、session event 和 JSON mode。`OutputContent::Image` 的 serde、ToolResult content 保留和 JSON event 承载方向正确，避免了把图像结果永久压成纯文本。

主要缺口是 3.5 DoD 中 “session replay / tree reconstruction / unsupported fallback” 的证据仍偏薄：

- 用户图像输入已有 writer/read round-trip 级测试，tool-result image 更多停留在 `SessionEntry` serde 和事件层。
- provider 不支持图像工具结果时的 fallback 字符串行为存在实现，但缺专门测试锁定。
- 二进制安全和 metadata 稳定性测试存在，但与完整 session replay 的组合覆盖不足。

审计结论：`opi-agent` 对图像工具结果的结构化承载基本方向正确，建议补齐 session writer/replay 与 fallback 合约测试后再称为完整通过。

### `opi-coding-agent`

Phase 3 在 CLI/runtime 层新增了 AGENTS.md/CLAUDE.md context loading、tool selection、安全 hook、find/ls、shell completions、图像 CLI、provider wiring 和 picker bridge。这里的测试数量明显增加，但很多问题也集中在这个 crate 的生产路径接线。

完成度较好的部分：

- `context_files.rs` 支持 cwd 到 ancestor 的 discovery、AGENTS before CLAUDE、非 UTF-8/oversized 跳过和 OPI.md 排除。
- `agents_md_context.rs` 通过 MockProvider 证明 context 内容能进入 system prompt。
- `tool_selection.rs` 覆盖 `--tools`、`--no-tools`、`--no-builtin-tools`、allowlist 过滤和未注册工具处理。
- `safety_hooks.rs` 覆盖 mutating tool deny、JSON mode policy event 和 session audit record。
- find/ls 工具有路径边界、gitignore、hidden-file、truncation、glob/regex 错误测试。
- shell completion 在 provider/config 构造前早退，避免需要 API key。

主要缺口：

- `build_provider` 没有接入 proxy config/env，导致 3.12 的 runtime DoD 不成立，也削弱 3.1/3.2/3.3 “reuse 3.12/3.13 shared client/proxy” 的证据。
- `--list-models` 未实现，enterprise provider 的 model metadata 不能通过 CLI 暴露。
- `--image` 定义和 `load_image()` 测试存在，但 non-interactive/interactive runtime 没有读取 `cli.image` 并构造 multimodal user message。
- `Agent::prompt()` 仍硬编码纯文本 `InputContent::Text`，缺 public multimodal prompt API。
- 全局 AGENTS.md/CLAUDE.md config directory 支持存在于函数参数，但生产 harness 调用传入 `None`。
- 3.8 DoD 写到 config precedence/conflict，但生产 config 没有 tool selection 字段。
- 3.11 的 picker 只到 bridge/state 测试，未接入 `interactive.rs` 的真实 TUI。

审计结论：`opi-coding-agent` 的 Phase 3 单元/集成测试基础较强，但多个用户可见功能停在解析、helper 或组件层，尚未完全进入 interactive/non-interactive 主路径。

### `opi-tui`

`opi-tui` 在 Phase 3 增加了终端图像渲染相关类型、fallback view、terminal capability detection、SelectList 组件和 snapshot。

覆盖良好的方面：

- image fallback snapshot 覆盖 80x24 和 120x40。
- terminal capability detection 覆盖 Kitty/iTerm/Sixel 环境变量优先级。
- `SelectListState` 覆盖 fuzzy filtering、empty state、large list、selection stability。
- SelectList snapshot 覆盖多个终端尺寸。

主要风险：

- Kitty escape 把 base64 放进参数而不是分号后的 payload，可能不符合协议。
- iTerm2 inline image payload 分隔符疑似应为 `:` 而不是 `;`。
- Sixel 生成只输出 minimal wrapper，没有真实像素 sixel 数据。
- SelectList 没有进入 production interactive TUI 的 model/session picker。
- TUI 对用户侧 image input 的展示路径未闭环；tool result image 部分可达，但 URL image 和无效 base64 的 fallback 行为偏弱。

审计结论：`opi-tui` 的组件层和 fallback 层通过度较高，但 terminal graphics protocol 和 real interactive integration 仍是 Phase 3 的主要风险区。

## 严重问题清单

### Critical

| 编号 | 位置 | 问题 | 影响 | 建议 |
| --- | --- | --- | --- | --- |
| C1 | `crates/opi-coding-agent/src/cli.rs` / `crates/opi-coding-agent/src/main.rs` | `--list-models` 未实现。 | 3.1/3.2/3.3 DoD 和 spec CLI surface 不满足，provider registry/model metadata 无用户路径。 | 增加 CLI flag，在 provider/config 构造后列出 registry/config 中可用模型或部署；补 subprocess 测试。 |
| C2 | `crates/opi-coding-agent/src/main.rs` | `build_provider` 未把 `[providers.*.proxy]` 或 env proxy 接入 `HttpClientBuilder`。 | 3.12 仅库层通过，真实 provider 请求不走 proxy；企业 provider 也没有复用 3.12 proxy。 | 统一 provider HTTP client 构造，明确 config > env > none 优先级，并覆盖所有 provider。 |
| C3 | `crates/opi-coding-agent/src/main.rs` / `crates/opi-agent/src/agent.rs` | `--image` 未接入 runtime，Agent prompt API 仅支持纯文本。 | 用户无法实际发送图像输入，3.4 的 CLI/TUI attachment contract 不成立。 | 增加 multimodal prompt API，non-interactive 和 interactive 首条 prompt 都从 `cli.image` 构造 `InputContent::Image`。 |
| C4 | `crates/opi-coding-agent/src/interactive.rs` / `crates/opi-coding-agent/src/picker.rs` | SelectList picker 未接入真实 interactive TUI。 | 3.11 核心 DoD 不满足，用户不能通过 TUI 选择 model/session。 | 增加 picker overlay/state machine、keybinding、cancel/confirm 流程，并补 production-path integration test。 |
| C5 | `docs/snapshots/phase3/opi-impl-state.json` | 缺少 `phase_exit.3`，根 ledger 与快照状态也存在漂移。 | 阶段无法审计关闭，不能可靠声明 Phase 3 exit criteria met。 | 写入 `phase_exit.3` 前先修复 Critical runtime 项并统一 evidence。 |

### High

| 编号 | 位置 | 问题 | 影响 | 建议 |
| --- | --- | --- | --- | --- |
| H1 | `crates/opi-ai/tests/*` | Bedrock/Azure/Vertex 缺真实 `Provider::stream()` wiremock lifecycle 测试。 | fixture-ready 与 runtime-ready 容易混淆，HTTP status、headers、cancel、error body 回归风险高。 | 为三家 provider 增加离线 wiremock stream tests。 |
| H2 | `crates/opi-ai/src/vertex.rs` / `crates/opi-coding-agent/src/config.rs` | Vertex service-account/ADC offline 路径未覆盖。 | 企业 GCP 默认凭据路径不满足 DoD。 | 明确缩窄 DoD 或实现 service account JSON/ADC token provider，并用 offline fixture 测试。 |
| H3 | `crates/opi-ai/src/bedrock/credentials.rs` | Bedrock ambient credential chain 覆盖不足。 | EC2/EKS/SSO/credential_process 等常见 AWS 环境可能不可用。 | 接入更完整 AWS credential chain 或更新文档和 DoD。 |
| H4 | `crates/opi-ai` / `crates/opi-agent` / `crates/opi-coding-agent` | 企业 provider secret redaction 未覆盖 logs/errors/sessions/snapshots 全范围。 | 凭据可能在错误、session 或快照路径泄漏，违反 provider-contract 风险模型。 | 增加 session JSONL、NDJSON、snapshot/error body redaction 测试。 |
| H5 | `crates/opi-tui/src/terminal_image.rs` | Kitty/iTerm escape 格式可疑，Sixel 为 minimal wrapper。 | 支持图形协议的终端可能无法显示图像，3.6 DoD 过度乐观。 | 对照协议修正 Kitty/iTerm；Sixel 要么实现真实编码，要么下调 DoD 为 unsupported fallback。 |
| H6 | `crates/opi-coding-agent/src/harness.rs` / `context_files.rs` | 全局 AGENTS.md/CLAUDE.md 目录在生产路径未接线。 | 用户全局 context 永远不会进入 system prompt。 | 将用户 config 目录传给 context discovery，并补生产路径测试。 |
| H7 | `docs/opi-spec.md` / `CHANGELOG.md` | spec Document Control 和 changelog 未反映 Phase 3 完成态。 | release/phase 状态对外不一致。 | phase exit 前更新 `[Unreleased]` 和 spec 状态，或明确 Phase 3 仍未关闭。 |

### Medium

| 编号 | 位置 | 问题 | 影响 | 建议 |
| --- | --- | --- | --- | --- |
| M1 | `crates/opi-coding-agent/src/image.rs` | 图像加载缺 size limit。 | 大图可能撑爆 session JSONL、provider payload 或 TUI 内存。 | 增加配置化 `max_image_bytes` 并在加载前校验。 |
| M2 | workspace | 缺 provider/model capability gating。 | 不支持 vision 的模型会在 provider 侧失败，而不是 opi 给出清晰错误。 | 在 `ModelInfo` 增加能力字段，发送前检查 image support。 |
| M3 | `crates/opi-agent/tests/image_tool_results.rs` | image tool result 缺完整 SessionWriter replay/tree reconstruction 测试。 | 3.5 的 session replay DoD 证据不足。 | 仿照 image input session tests 增加 writer/read/branch reconstruction。 |
| M4 | `crates/opi-coding-agent/src/policy.rs` / `config.rs` | tool selection 与 config precedence/conflict 未实现。 | DoD 文本与真实配置 surface 不一致。 | 增加 config 字段并定义 CLI 覆盖规则，或调整 DoD。 |
| M5 | `crates/opi-coding-agent/tests/safety_hooks.rs` | 部分 safety hook 测试断言过宽或未执行完整 agent loop。 | hook allowlist/deny 交互回归可能漏检。 | 解析 NDJSON 事件并断言具体事件类型；增加 allowlist + hook deny E2E。 |
| M6 | `crates/opi-coding-agent/tests/shell_completions.rs` | completion 测试依赖 release 二进制。 | 普通 debug `cargo test` 环境可能失败或跳过真实路径。 | 使用 `CARGO_BIN_EXE_opi` 或显式构建策略。 |
| M7 | `crates/opi-coding-agent/src/tool/ls.rs` | `ls` 截断提示计数疑似使用截断后的 entries 长度。 | 用户看到的 omitted count 可能不准确。 | 截断前保存 total count，输出 `total - max_entries`。 |
| M8 | `crates/opi-coding-agent/src/tool/find.rs` | `find` 输出绝对路径。 | 与 `glob` 相对路径风格和 pi parity 可能不一致。 | 根据 spec 决定是否统一为 workspace-relative。 |

### Low

| 编号 | 位置 | 问题 | 影响 | 建议 |
| --- | --- | --- | --- | --- |
| L1 | `docs/snapshots/phase3/opi-impl-state.json` | 3.1 evidence 使用旧键名，`verified_at_commit` 为空。 | 任务追溯性弱。 | 统一为 `opi_task`、`opi_dod_sha256`、`opi_verification`、`opi_evaluator`。 |
| L2 | `docs/snapshots/phase3/opi-impl-state.json` | 3.4-3.13 缺 `end_commit`，hash 长短混用。 | 阶段审计和发布追溯不一致。 | 统一使用 40 字符 commit hash 或明确短 hash 策略。 |
| L3 | `docs/snapshots/phase3/opi-impl-state.json` | 多个 `evaluator_required: true` 任务记录 `opi_evaluator: not-required`。 | 与 Phase 1 类似，需要解释 evaluator gate 的实际语义。 | 在 `phase_exit.3.audit_notes` 记录原因，或补 evaluator evidence。 |
| L4 | `docs/snapshots/phase3/opi-impl-state.json` | `last_attempt` 为日期字符串，与 schema 预期对象结构不一致。 | 机器可读 ledger 质量下降。 | 后续 reinit/cleanup 时规范化字段。 |
| L5 | `docs/snapshots/phase3/opi-impl-state.json` | 测试计数和 behavioral_tests 列表不完全同步。 | 审计证据容易误导。 | 刷新测试路径：如 3.4 加 `image_provider_serialization.rs`，3.6 加 `image_view_snapshots.rs`。 |

## Ledger 与源码不一致

| 任务/范围 | Ledger 记录 | 审计核对 |
| --- | --- | --- |
| Phase exit | 13/13 tasks `passing` | 缺 `phase_exit.3`，`current_phase` 仍为 3，不应视为阶段关闭。 |
| 3.1 | `status: passing`，`verified_at_commit: null` | evidence 和 commit 追溯不完整。 |
| 3.1-3.3 | registry/model metadata and `--list-models` | 未发现 `--list-models` CLI flag 或主流程分支。 |
| 3.1-3.3 | shared reqwest client/proxy from 3.12/3.13 | provider 构造使用 client 基础设施，但 proxy 未接入 production `build_provider`。 |
| 3.1 | ambient AWS credential chain | 实现证据主要覆盖 config/env/profile 文件链，未覆盖完整 AWS ambient chain。 |
| 3.3 | offline OAuth/service-account token injection | 实现证据主要是 access token env 注入，缺 service-account/ADC。 |
| 3.4 | CLI/TUI attachment contract | `--image` 和 load tests 存在，但 runtime prompt/TUI 用户路径不可达。 |
| 3.5 | session replay/tree reconstruction | image tool result serde/JSON 证据存在，完整 writer/replay 覆盖不足。 |
| 3.6 | Kitty/iTerm/Sixel escape generation | fallback 和 detection 测试存在，但 escape 格式和 Sixel 数据生成有风险。 |
| 3.7 | global config directory context loading | discovery 函数支持参数，但生产 harness 传入 `None`。 |
| 3.8 | config precedence/conflicts | CLI flag precedence 存在，config tool selection 字段不存在。 |
| 3.8 | confirmation prompts | 实现为 allow/deny hook 和 mutating policy，无交互式确认 UI。 |
| 3.9 | find/ls parity | 基本成立；`ls` 截断提示和 `find` 输出路径风格需核对。 |
| 3.10 | subprocess tests | completion 测试存在，但依赖 release binary。 |
| 3.11 | real interactive TUI picker | `SelectList` 和 `picker.rs` 存在，`interactive.rs` 未集成。 |
| 3.12 | provider proxy wiring | proxy 原语和配置测试存在，真实 provider construction 未接线。 |
| 3.13 | no per-request client allocation in hot path | Arc/client 复用测试存在，缺热路径计数器或 benchmark；企业 provider 覆盖不完整。 |

## 覆盖良好的方面

- Bedrock/Azure/Vertex 的协议适配方向正确，fixture 覆盖 text、tool call、usage、provider error 和错误映射。
- Bedrock SigV4 有确定性 signing 测试，降低企业 provider 鉴权回归风险。
- 图像协议在 `opi-ai` 公共类型中结构化落地，serde 对 base64/bytes/url 形态有基础覆盖。
- provider image serialization 测试覆盖 Anthropic、OpenAI Chat、OpenAI Responses、Gemini、OpenRouter、Mistral 等路径。
- image input 的 session JSONL 和 JSON mode 事件覆盖比 Phase 2 时更完整。
- `ToolResult` image content 能以结构化 JSON 进入 agent/session event，而不是只保留文本。
- AGENTS.md/CLAUDE.md 的目录优先级、文件顺序、OPI.md 排除和 resume reread 逻辑有集中测试。
- tool selection 和 safety hooks 覆盖 CLI flags、allowlist、deny policy、JSON policy event 和 session audit。
- find/ls 工具补齐了 pi-style file discovery/list parity 的主要工具面。
- shell completions 早退设计合理，不需要 provider API key。
- SelectList 组件本身的 fuzzy filtering、selection stability、empty state 和 snapshot 覆盖较好。
- `HttpClient`/pooling/proxy 原语为后续统一 provider HTTP construction 提供了正确基础。

## Phase exit 建议

### 建议结论

建议将 Phase 3 当前状态表述为：

- 任务级状态：`13/13 passing`，库层、fixture 层、组件层和部分 CLI 测试证据成立。
- 审计状态：**有条件通过**。
- 发布/里程碑状态：不建议称为 “Phase 3 runtime-complete”，也不建议在未修复 Critical 项前发布 0.4.0。

如果只以“功能模块和测试基础已合并”为阶段目标，当前状态可以作为 Phase 3 hardening 前快照；如果以 `docs/opi-spec.md` 和 ledger DoD 衡量，仍应保留 Critical blockers。

### 优先修复顺序

1. 实现 `--list-models` CLI，并补 provider registry/model metadata 的 subprocess 或 integration 测试。
2. 在 `build_provider` 接入 config/env proxy，统一 `HttpClientBuilder` 构造和优先级，覆盖所有 provider。
3. 将 `--image` 接入 non-interactive/interactive runtime，增加 multimodal prompt API，并让 TUI 展示用户图像输入。
4. 将 SelectList picker 接入真实 interactive TUI 的 model/session 流程，补 cancel/confirm/keybinding 测试。
5. 修正 Kitty/iTerm escape，明确 Sixel 是真实实现还是 fallback-only；按结论更新 DoD。
6. 为 Bedrock/Azure/Vertex 增加 `Provider::stream()` wiremock lifecycle tests。
7. 补 Vertex service-account/ADC、Bedrock ambient chain 或同步下调 DoD。
8. 补企业 provider secret redaction 的 session/NDJSON/snapshot/error 覆盖。
9. 写入 `phase_exit.3`，统一 evidence 字段、commit hash、`end_commit`、测试路径和 evaluator notes。
10. 更新 `CHANGELOG.md` `[Unreleased]` 和 `docs/opi-spec.md` Document Control，避免 release 状态漂移。

### 建议验证命令

修复上述阻塞项后，建议至少运行：

```sh
cargo fmt --check --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
```

并新增或强化以下目标测试：

- `cargo test -p opi-ai --test bedrock_fixtures`
- `cargo test -p opi-ai --test azure_openai_fixtures`
- `cargo test -p opi-ai --test vertex_fixtures`
- `cargo test -p opi-ai --test proxy_support`
- `cargo test -p opi-ai --test connection_pooling`
- `cargo test -p opi-coding-agent --test bedrock_provider_wiring`
- `cargo test -p opi-coding-agent --test azure_openai_provider_wiring`
- `cargo test -p opi-coding-agent --test vertex_provider_wiring`
- `cargo test -p opi-coding-agent --test image_input_cli`
- `cargo test -p opi-coding-agent --test image_input_json_mode`
- `cargo test -p opi-coding-agent --test terminal_image_integration`
- `cargo test -p opi-coding-agent --test agents_md_context`
- `cargo test -p opi-coding-agent --test tool_selection`
- `cargo test -p opi-coding-agent --test safety_hooks`
- `cargo test -p opi-coding-agent --test picker_integration`
- `cargo test -p opi-tui --test terminal_image_rendering`
- `cargo test -p opi-tui --test select_list`

## 最终结论

Phase 3 已经显著扩展了项目能力边界：企业 provider、图像协议、终端图像组件、context loading、tool selection、find/ls、completions、fuzzy picker、proxy 和 pooling 都具备了可测试基础。与 Phase 2 审计时相比，许多前期 “fixture/component ready” 的模式在 Phase 3 中继续推进，并形成了更完整的跨 crate 类型和测试资产。

但当前最大的风险仍是若干功能尚未进入真实用户路径。`--list-models`、proxy runtime、图像输入端到端、interactive picker、terminal graphics protocol 和 phase exit ledger 治理，是 Phase 3 从“任务级 passing”走向“runtime-complete”的主要差距。

建议短期内不要继续扩展 Phase 4 功能面，而是先做一次 Phase 3 hardening pass，修复 Critical/High 项后重新生成阶段快照并写入 `phase_exit.3`。届时 Phase 3 才能更稳妥地宣称达到 “enterprise provider + multimodal + pi parity hardening” 里程碑。
