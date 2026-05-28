# Phase 3 Codex 审计报告

审计日期：2026-05-27  
审计对象：`docs/snapshots/phase3/opi-impl-state.json` 声明的 Phase 3 实现状态，以及当前工作树源码。  
审计方式：按 `$grill-me` 规则把每个 DoD 拆成质询项；能从仓库回答的问题直接查源码和测试，不把 ledger 的 `passing` 状态当作充分证据。

## 结论

Phase 3 的模块级代码和测试基础已经大量落地，且当前工作树的 `cargo test --workspace --all-targets` 通过。但按快照 DoD 和 `docs/opi-spec.md` 的阶段退出标准衡量，Phase 3 还不能称为 runtime-complete。

主要问题不是测试红，而是多处实现停在“类型、helper、fixture、组件”层，尚未进入真实用户路径：

- `--image` 已解析、`load_image()` 已存在，但运行时 prompt 仍只发送文本。
- `[providers.*.proxy]` 和 env proxy 解析存在，但 provider factory 没有把代理配置传进实际 HTTP client。
- 企业 provider 的 `--list-models` DoD 无法满足，因为 CLI 没有 `--list-models` 分支。
- `SelectList` 与 picker bridge 存在，但没有接入交互式 TUI 的模型/会话选择流程。
- global `AGENTS.md` / `CLAUDE.md` discovery 函数支持参数，但生产 harness 固定传 `None`。
- 终端图像协议存在 escape helper，但 Kitty/Sixel 输出仍不像可用协议实现。
- ledger 缺少 `phase_exit.3`，部分任务证据字段为空或不一致。

建议在发布 0.4.0 或声明 Phase 3 完成前，先做一次 Phase 3 hardening pass，优先关闭 Critical/High 项。

## 新鲜验证

- `cargo test --workspace --all-targets`：通过，退出码 0。
- `cargo run -p opi-coding-agent -- --help`：通过，输出中包含 `--image`、`--generate-completion`、tool selection flags；未包含 `--list-models`。
- `git status --short`：审计前已有未跟踪的其他审计文档和 skill 目录；本报告只新增 `docs/snapshots/phase3/audit.codex.md`。

## Critical

### C1. `--image` 没有进入运行时请求

快照 3.4 要求“user-supplied image inputs round-trip through Agent/UserMessage session JSONL entries and JSON mode events”。当前实现没有做到用户路径级别的 round trip。

证据：

- CLI 确实声明了 `--image`：`crates/opi-coding-agent/src/cli.rs:92`。
- 图像加载 helper 存在：`crates/opi-coding-agent/src/image.rs:20`。
- 但 `main.rs` 没有读取 `cli.image`，`run_non_interactive()` 只把 `prompt_text` 传给 runner。
- `CodingHarness::prompt()` 只接受 `&str`：`crates/opi-coding-agent/src/harness.rs:209`。
- `Agent::prompt()` 固定构造 `InputContent::Text`：`crates/opi-agent/src/agent.rs:109` 和 `crates/opi-agent/src/agent.rs:117`。

影响：用户执行 `opi --image photo.png "describe"` 时，图片不会进入 provider request、session JSONL 或 JSON mode events。现有测试只覆盖 CLI parse、media type detection、手工构造的 image serde/NDJSON，不覆盖真实 runtime。

建议：新增 `prompt_with_content(Vec<InputContent>)` 或等价 API；在 CLI 层加载 `cli.image` 并和文本 prompt 组装成同一个 `UserMessage`；用 `MockProvider` 做 E2E 断言 request.messages、session JSONL、NDJSON 都包含图片。

### C2. Proxy 支持没有接入 provider factory

快照 3.12 要求标准 env vars 和 `[providers.*.proxy]` 配置流入 shared reqwest client。当前代码只解析和测试了配置对象、`HttpClientBuilder` helper，生产路径仍创建无代理 client。

证据：

- `proxy_from_env()` 只有声明：`crates/opi-ai/src/http.rs:210`；源码搜索未发现生产调用。
- Anthropic/OpenAI/OpenRouter/Mistral/OpenAI Responses/Gemini provider 在 `build_provider()` 中使用默认构造器，未传代理 client：例如 `crates/opi-coding-agent/src/main.rs:266`。
- Bedrock/Azure/Vertex 虽然显式 `.with_client(...)`，但传入的是 `HttpClient::new()`：`crates/opi-coding-agent/src/main.rs:397`、`crates/opi-coding-agent/src/main.rs:424`、`crates/opi-coding-agent/src/main.rs:455`。
- TOML 中各 provider 的 `proxy` 字段已解析，但没有被 `build_provider()` 消费。

影响：企业网络环境中用户配置的代理不会生效；env proxy 的“deterministic precedence”也只是纯函数语义，未影响真实请求。

建议：提取 `build_http_client(provider_proxy)`，按 explicit config > env proxy > no proxy 的规则构造 `HttpClientBuilder`；所有 provider construction 使用同一 helper；新增 `build_provider` 级测试，验证每个 provider 的 `http_client().proxy_config()`。

### C3. Phase 3 要求的 `--list-models` 不存在

快照 3.1/3.2/3.3 都要求 enterprise providers 的 registry/model metadata 进入 `--list-models`。当前 CLI 没有该 flag。

证据：

- `opi --help` 输出没有 `--list-models`。
- `crates/opi-coding-agent/src/cli.rs` 声明了 model/config/session/completion/tool/image flags，但没有 list-models 字段。
- 源码搜索 `list-model` / `list_models` 没有找到生产实现。

影响：Bedrock/Azure/Vertex 即使 provider 模型元数据存在，也无法通过用户可见 CLI 路径枚举；相关 DoD 未满足。

建议：新增 `--list-models` 早期命令；构造 provider registry 或轻量 model catalog；确保 Azure deployments、Vertex models、Bedrock defaults 都可输出；补 CLI snapshot/subprocess tests。

## High

### H1. Picker 只实现了组件和 bridge，未接入真实 TUI

快照 3.11 要求 fuzzy model/session picker “integrated into real interactive TUI flows”。当前实现只有 `SelectList` widget 和 `picker.rs` 数据转换。

证据：

- bridge 函数在 `crates/opi-coding-agent/src/picker.rs:12` 和 `crates/opi-coding-agent/src/picker.rs:33`。
- 生产 `interactive.rs` 中没有 `SelectList`、`model_picker_items()` 或 `session_picker_items()` 调用；搜索结果显示这些只被测试使用。
- `run_interactive_tui()` 事件循环只处理 prompt 输入、submit、abort、newline，没有模型/会话 picker 状态。

影响：用户无法在交互式 TUI 中打开 Phase 3 声称的模型/会话选择器。当前测试验证的是 widget 和数据转换，不是用户流程。

建议：在 TUI state 中增加 picker mode；绑定明确按键或命令打开模型/会话 picker；确认选择后更新 model 或 resume target；增加交互层集成测试。

### H2. Global context files 没有接入生产 harness

快照 3.7 要求从 cwd ancestors 和 isolated global config dir 发现 `AGENTS.md` / `CLAUDE.md`。函数支持 global 参数，但生产路径固定不传。

证据：

- `discover_context_files(cwd, global_config_dir)` 支持 global dir。
- `CodingHarness` 实际调用为 `discover_context_files(&workspace_root, None)`：`crates/opi-coding-agent/src/harness.rs:129`。
- 测试 `precedence_includes_global_config_last` 直接调用 helper，并未覆盖真实 harness 的 global config 注入。

影响：用户全局 `AGENTS.md` / `CLAUDE.md` 不会进入 system prompt，DoD 的 global context 部分未满足。

建议：定义并实现 global context path，例如 user config dir 下的 context files；在 harness construction 传入该路径；新增 E2E harness 测试。

### H3. Bedrock credential chain 不等于 AWS ambient credential chain

快照 3.1 要求 ambient AWS credential chain。当前实现只覆盖 explicit config、固定 env vars、显式 profile + `~/.aws/credentials`。

证据：

- `resolve_bedrock_env_credentials()` 只读取 `AWS_ACCESS_KEY_ID`、`AWS_SECRET_ACCESS_KEY`、`AWS_SESSION_TOKEN`、`AWS_REGION` / `AWS_DEFAULT_REGION`。
- `default_aws_credentials_path()` 只查 `HOME` / `USERPROFILE` 下的 `.aws/credentials`。
- `resolve_credentials()` 只有 ExplicitConfig、Environment、ProfileFile 三种 source。
- 未看到 `AWS_PROFILE`、`AWS_SHARED_CREDENTIALS_FILE`、`AWS_CONFIG_FILE`、credential_process、SSO、web identity、ECS/EC2 metadata 或 AWS SDK credential provider chain。

影响：许多真实 AWS 环境无法认证 Bedrock；“ambient AWS credential chain”声明过宽。

建议：要么引入 AWS SDK credential provider chain 并保持 no-live-call tests，要么把 DoD 和文档降级为“limited offline credential resolution”。

### H4. Enterprise provider HTTP lifecycle 测试缺口

Bedrock/Azure/Vertex 的测试集中在 request body、URL、fixture SSE parsing、少量 status mapping；没有看到 wiremock 级 `Provider::stream()` lifecycle 测试。

证据：

- Bedrock 测试使用 `stream_from_fixture()`。
- Azure/Vertex 测试使用 `stream_from_sse()`。
- 对这三家 provider 搜索 `wiremock` / `MockServer` 无结果。

影响：真实 HTTP header、auth header、URL path、status body、Retry-After、cancellation、terminal event 行为没有通过 provider runtime 测试锁住。企业 provider 适配风险高。

建议：为三家 provider 增加离线 wiremock tests，直接调用 `Provider::stream()`，覆盖成功、401/403、429 + Retry-After、5xx、cancel、no-terminal-event。

### H5. Bedrock 429 没有解析 Retry-After

Phase 2 已有 retry/backoff 语义，Azure/Vertex 也解析 Retry-After；Bedrock 429 固定 `retry_after_ms: None`。

证据：

- `map_bedrock_status()` 中 429 返回 `retry_after_ms: None`：`crates/opi-ai/src/bedrock/mod.rs:94`。
- 实际 HTTP path 中 429 也返回 `retry_after_ms: None`：`crates/opi-ai/src/bedrock/mod.rs:315`。

影响：Bedrock rate limit 会退回指数退避，无法尊重服务端明确等待时间。

建议：和 Azure/Vertex 一样传入 response headers 并调用 `crate::retry::parse_retry_after()`。

### H6. 终端图像协议实现仍像占位

快照 3.6 要求 Kitty/iTerm/Sixel escape generation。当前 fallback 文本层可用，但 Kitty/Sixel 更像结构测试桩。

证据：

- `kitty_escape()` 把 base64 payload 拼进参数列表里的 `t=d,{b64}`，并硬编码 `f=24`：`crates/opi-tui/src/terminal_image.rs:104`。
- `sixel_escape()` 完全不使用 `data.bytes`，只输出尺寸包装：`crates/opi-tui/src/terminal_image.rs:142`。
- `MessageList` 把 escape sequence 作为 ratatui `Span` 写入 buffer：`crates/opi-tui/src/message_list.rs:112` 到 `crates/opi-tui/src/message_list.rs:115`，这不是经过验证的 raw terminal graphics output path。

影响：snapshot/fallback 测试会通过，但支持 Kitty/Sixel 的真实终端很可能无法显示图像。

建议：先把 Phase 3 声明收敛为 fallback rendering，或补真实协议编码和可验证的 terminal output path；Sixel 至少需要实际像素编码，不应忽略图片字节。

## Medium

### M1. Azure endpoint 缺失时使用占位 URL 而不是配置错误

`AzureOpenAIProvider::new()` 和 `from_config()` 在 endpoint 缺失时默认使用 `https://YOUR_RESOURCE.openai.azure.com`：`crates/opi-ai/src/azure_openai.rs:60`、`crates/opi-ai/src/azure_openai.rs:88`。

影响：用户只设置 `AZURE_OPENAI_API_KEY` 和 `--model azure:<deployment>` 时，会发起到占位域名的请求，而不是得到清晰配置错误。

建议：provider factory 对 Azure endpoint 做必填校验；测试缺 endpoint 返回 config error。

### M2. Bedrock 对 URL 图片静默发送空 bytes

Bedrock Converse serialization 遇到 `ImageSource::Url` 返回 `String::new()`：`crates/opi-ai/src/bedrock/mod.rs:888`。

影响：URL 图片不会被拒绝，也不会正确发送，可能变成难诊断的 provider error。

建议：在 request build 前返回 clear unsupported-capability error，或实现下载/转换策略。

### M3. 图像能力没有模型级 gating

快照 3.4 要求 provider capability gating explicit。当前 `ModelInfo` 只有 streaming/thinking capability，没有 image capability；OpenRouter/Mistral 通过 OpenAI-compatible adapter 直接序列化图片。

影响：文本模型或不支持 vision 的兼容 provider 会在服务端失败，而不是在本地给出清晰错误。

建议：扩展 capability model，例如 `supports_images`；在 provider request build 前校验；OpenRouter/Mistral 对非 vision profile 返回 unsupported。

### M4. `ls` 截断元数据和提示不准确

`ls` 先计算 `truncated`，再 `entries.truncate(max_entries)`，之后用截断后的 `entries.len()` 生成 omitted count 和 `entry_count`：`crates/opi-coding-agent/src/tool/ls.rs:124`、`crates/opi-coding-agent/src/tool/ls.rs:148`。

影响：输出会把“已显示数量”当作“总数/遗漏数量”的基础，用户看到的 truncation 信息不可靠。

建议：保留 `total_entries`，用 `total_entries - max_entries` 计算 omitted；details 同时记录 total/displayed。

## Ledger / governance

### L1. 缺少 `phase_exit.3`

`opi-impl-state.json` 的 `phase_exit` 只有 Phase 1 和 Phase 2：`docs/snapshots/phase3/opi-impl-state.json:1247`。

影响：快照记录所有 Phase 3 tasks passing，但没有阶段级 exit criteria 结论、审计摘要、已知例外和最终验证状态。

建议：不要在修复 Critical/High 前补 `phase_exit.3` 为完成；应先记录本审计中的 blockers，再在 hardening 后生成新的 phase exit。

### L2. 任务证据字段不一致

示例：

- 3.1 `verified_at_commit` 为 `null`，但有 `end_commit: 99b263d`：`docs/snapshots/phase3/opi-impl-state.json:94`。
- 多个任务 `evaluator_required=true`，但 evidence 记录 `opi_evaluator: not-required`。这可能沿用 Phase 1 的保守 gate 逻辑，但 Phase 3 snapshot 没有对应 audit note。
- 3.4 到 3.13 多数 `end_commit` 为空，仅有 `verified_at_commit`。

影响：ledger 仍可读，但不能作为严格审计账本直接证明阶段关闭。

建议：统一 `verified_at_commit` / `end_commit` 语义；为 evaluator_required 和 not-required 的差异补 Phase 3 audit note。

## 覆盖良好的方面

- `cargo test --workspace --all-targets` 当前通过，说明现有测试集稳定。
- Bedrock SigV4 有确定性 signing tests，降低签名算法回归风险。
- Azure/Vertex 对 URL/body/auth header 的构造测试较清楚。
- `InputContent::Image` / `OutputContent::Image` 的 serde、session JSONL、NDJSON 事件层测试覆盖较好。
- `find` 使用 `ignore::WalkBuilder`，整体比 `ls` 的手写 gitignore 逻辑更稳。
- `SelectList` widget 本身有较完整的状态和 snapshot coverage。
- shell completion subprocess tests 覆盖了 bash/zsh/fish/powershell/elvish。

## 建议修复顺序

1. 修复 `--image` runtime path，并补 request/session/NDJSON E2E。
2. 修复 proxy runtime wiring，并补每个 provider 的 factory-level tests。
3. 实现 `--list-models`，明确 provider registry/catalog 归属。
4. 接入 TUI picker 真实流程，或下调 3.11 完成声明。
5. 接入 global context files 的生产路径。
6. 补 Bedrock/Azure/Vertex wiremock lifecycle tests，并修 Bedrock Retry-After。
7. 明确 Azure endpoint 必填、Bedrock URL image unsupported、image capability gating。
8. 修 terminal image protocol，或把非 fallback graphics 标记为 experimental。
9. 修 ledger：补 Phase 3 audit note，hardening 后再写 `phase_exit.3`。

## 最终判定

当前状态适合作为“Phase 3 implementation snapshot with passing tests”，不适合作为“Phase 3 exit complete”。

阻塞发布级声明的核心原因是：多个 DoD 依赖真实用户路径，但实现和测试仍停留在 helper/component/fixture 层。建议先完成一次窄范围 hardening，而不是继续扩展 Phase 4 功能面。
