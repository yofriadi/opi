# pi 对齐矩阵

## 范围

本文按语义行为和产品工作流对比 `opi` 与 `.repo/pi-0.75.3`。它不是 TypeScript API 兼容性清单。

目标是：

- 保留用户和嵌入方依赖的 `pi` 运行时语义；
- 让 Rust crate 边界遵循 Rust 的所有权、trait 和发布实践；
- 将 MCP、子 Agent、plan mode、todo、permission gate 等工作流重功能保持在核心之外。

## 对齐等级

| 等级 | 含义 |
|---|---|
| 完整 | 已具备等价的用户可见或库可见行为。 |
| 部分 | 已实现底座或更窄的 Rust-native 等价能力。 |
| 有意偏离 | 因 Rust 架构或项目范围而刻意不同。 |
| 缺失 | `pi` 中存在、且属于 `opi` 范围，但尚未实现。 |
| 范围外 | `pi` 中存在，但不属于当前 `opi` 范围。 |

## 包级对齐

| pi package | opi crate | 等级 | 当前状态 | 下一步 |
|---|---|---|---|---|
| `@earendil-works/pi-ai` | `opi-ai` | 部分 | 已有核心 provider streaming、provider registry、model metadata、image input、usage/cost、retry、proxy、custom provider/model registration，以及 config-driven OpenAI-compatible profiles。`pi` 仍有更广的一等 provider、OAuth provider 和 image generation 表面。 | 只有 wire protocol 或 auth model 明显不同时才增加一等 provider；OAuth 保持为单独产品决策。 |
| `@earendil-works/pi-agent-core` | `opi-agent` | 完整 | agent loop 语义、hooks、tool batching、queues、sessions、compaction、SDK types、extensions 和 streaming proxy primitives 均有体现。public surface 保留在 `lib.rs`，loop internals 已移入 `agent_loop.rs`。 | 保持 runtime internals 聚焦且更深，但不引入 shared types crate。 |
| `@earendil-works/pi-coding-agent` | `opi-coding-agent` | 部分 | CLI modes、built-in tools、config、sessions、context files、images、JSON/RPC、resources、packages、skills、prompt fragments、themes、custom provider registration、RPC extension commands、`/tree`、`/fork`、`/clone`、`--fork`、同文件活跃分支 continuation、`opi package add/remove/list/doctor` CLI、带 `[adapter]` 声明的 manifest V2、通过 `opi-extension-jsonl-v1` 运行的 `process-jsonl` adapter hosting，以及 adapter-to-runtime bridge 均已存在。产品工作流广度仍窄于 `pi`。 | 保持 adapter protocol 演进；API 稳定后再增加更广的 adapter kind。 |
| `@earendil-works/pi-tui` | `opi-tui` | 有意偏离 | `opi-tui` 使用 `ratatui`/`crossterm` widgets，没有复制 `pi` 的 TypeScript terminal renderer。已有 transcript、markdown/code、diff、pickers、branch picker、themes、keybindings、terminal-image primitives，以及 branch/session picker 的 CJK display-width snapshot 覆盖。 | 除非单独决定做可复用 TUI 产品，否则保持 coding-agent 所需范围。 |
| `@earendil-works/pi-web-ui` | `opi-web-ui` | 有意偏离 | `opi-web-ui` 是未发布的 Rust event/state/component/rendering crate，不是 `pi-web-ui` 那种独立 browser component package。 | 保持 `publish = false`，在单独 web-app 计划前只描述为 RPC/SDK consumer surface。 |

## Phase 对齐

| Phase | 功能族 | opi crate | pi 体现 | 等级 | 当前状态 | 下一步 |
|---:|---|---|---|---|---|---|
| 1 | Provider trait、stream events、Anthropic provider | `opi-ai` | `pi-ai` provider 和 stream contracts | 完整 | Provider-neutral stream API 和 Anthropic SSE 已实现。 | 保持 stream lifecycle 和 in-band errors fixture 覆盖。 |
| 1 | Provider registry | `opi-ai` | `pi-ai` API/model/provider registry concepts | 完整 | 已有 `provider:model` 解析和 capabilities。 | 让更多 model listing 和 profile 行为走 registry。 |
| 1 | Agent loop、`Agent`、hooks、queues | `opi-agent` | `pi-agent-core` `agentLoop`、hooks、steering/follow-up | 完整 | Runtime 语义已体现，loop implementation 现在位于聚焦的 internal module。 | 保持 public `opi_agent::agent_loop` export，同时让 private helpers 留在内部。 |
| 1 | Tool trait 和 schema validation | `opi-agent` | TypeBox tool schemas 和 runtime validation | 完整 | Rust tool trait + JSON Schema validation 已存在。 | 保持 validation 位于 model/tool boundary。 |
| 1 | Coding tools | `opi-coding-agent` | `read`、`write`、`edit`、`bash` 加只读导航 | 部分 | Interactive defaults 匹配 `pi`；`grep`、`find`、`ls` 和额外 `glob` 已存在。 | 保持 `glob` 为额外便利，不让核心流程依赖它。 |
| 1 | TUI shell 和 markdown/code rendering | `opi-tui` | `pi-tui` terminal UI components | 部分 | Ratatui shell、transcript、markdown、code 和 snapshots 已存在；picker snapshots 已覆盖 CJK-width labels。 | 只增加 coding-agent 工作流需要的 primitives。 |
| 1 | Config 和 non-interactive mode | `opi-coding-agent` | `pi` print mode 和 settings | 完整 | TOML config 和 text mode 已存在。 | 保持 TOML；默认不追逐 `pi` JSON config 兼容。 |
| 2 | OpenAI-compatible、OpenAI Responses、OpenRouter、Gemini、Mistral | `opi-ai` | `pi-ai` provider families | 部分 | Phase 2 provider 核心集合和 config-driven OpenAI-compatible profiles 已存在。 | 用 profile configuration 扩展 provider 广度，避免硬编码所有兼容 provider。 |
| 2 | Sessions 和 resume/delete/list/fork | `opi-agent`、`opi-coding-agent` | `pi` session manager | 部分 | Append-only JSONL sessions、resume、list/delete、`--fork`、交互式 `/fork`/`/clone` 新会话路径，以及通过运行时 `parent_id`/`leaf` 条目的同文件活跃分支 continuation 已实现。 | 改善 package-manager workflow 和更丰富 tree metadata display。 |
| 2 | Compaction | `opi-agent`、`opi-coding-agent` | `pi` compaction 和 summarization flow | 完整 | Threshold/manual/overflow compaction primitives 和 session events 已存在。 | 保持 branch-aware compaction tests。 |
| 2 | NDJSON event mode | `opi-coding-agent` | `pi` JSON event mode | 完整 | `--json` 输出 versioned session 和 agent events。 | 保持 schema/event contract tests。 |
| 2 | Thinking、usage、cost、retry | `opi-ai`、`opi-coding-agent` | `pi` model options 和 accounting | 部分 | Thinking、usage accumulation、best-effort cost 和 retry/backoff 已存在。 | 保持 provider capability checks 保守。 |
| 2 | Diff、themes、keybindings | `opi-tui`、`opi-coding-agent` | `pi-tui` 和 coding-agent settings | 部分 | Diff rendering、themes 和 keybindings 已存在。 | 除非命令需要，否则避免扩大成通用 TUI framework。 |
| 3 | Bedrock、Azure OpenAI、Vertex AI | `opi-ai` | `pi-ai` enterprise providers | 部分 | Wire adapters 已存在。 | 单独决策 OAuth/ADC/profile scope，不隐式增加 credential store。 |
| 3 | Image input 和 image tool results | `opi-ai`、`opi-agent`、`opi-coding-agent` | `pi` attachments 和 multimodal messages | 部分 | Image attachments 和 image result serialization 已存在。 | 只有明确产品计划时才扩展更多附件类型。 |
| 3 | Terminal image rendering | `opi-tui` | `pi-tui` image support | 完整 | Terminal image protocol detection/rendering 已存在。 | 维护跨 terminal snapshot/smoke checks。 |
| 3 | Context files | `opi-coding-agent` | `AGENTS.md` / `CLAUDE.md` context loading | 完整 | Workspace-ancestor 和 user-config context loading 已存在。 | 除非迁移计划改变，否则继续排除 `OPI.md`。 |
| 3 | Tool selection 和 safety hooks | `opi-coding-agent`、`opi-agent` | `pi` tool allowlists 和 extension-mediated safety | 完整 | Tool flags 和 mutating-tool opt-in policy 已存在。 | 保持 permission popups 在核心之外。 |
| 3 | `find` / `ls`、completions、model/session picker | `opi-coding-agent`、`opi-tui` | `pi` CLI tools 和 interactive UX | 部分 | Commands/tools/pickers 已存在。 | 优先改善 session tree UX。 |
| 3 | Proxy 和 HTTP pooling | `opi-ai` | `pi-ai` proxy/provider HTTP support | 完整 | Per-provider proxy 和 standard proxy env fallback 已存在。 | 保持 secret redaction 和 no-proxy coverage。 |
| 4 | RPC JSONL 和 SDK event/command model | `opi-agent`、`opi-coding-agent` | `pi` RPC/SDK modes | 完整 | Strict JSONL、correlated responses、async events、shared SDK types 和 `extension_command` dispatch 已存在。 | 保持协议版本化，并诚实拒绝不支持的 runtime mutations。 |
| 4 | Extension hooks、tools、commands、messages、state | `opi-agent`、`opi-coding-agent` | `pi` TypeScript extensions | 部分 | 面向 embedder 的 in-process Rust extension API、RPC/SDK command dispatch，以及 process-JSONL adapter bridge（tool、command、hook、event、state、cancellation）已存在，可供外部 package 使用。 | 保持 adapter protocol 演进；API 稳定后再增加 gRPC 或其他 adapter kind。 |
| 4 | Resource discovery | `opi-coding-agent` | `pi` extension/resource loading | 部分 | User/project/explicit resource metadata loading 已存在。 | 确保 metadata 一致接入 interactive、non-interactive 和 RPC。 |
| 4 | Skills 和 prompt fragments | `opi-coding-agent` | `pi` skills 和 prompt templates | 部分 | Progressive discovery 已存在。 | 增加 invocation 和 metadata paths，但不把 prompt fragments 隐式变成核心命令。 |
| 4 | Themes | `opi-coding-agent`、`opi-tui` | `pi` themes | 部分 | Theme discovery 和 built-in fallback 已存在。 | 增加 precedence 和 missing theme diagnostics 测试。 |
| 4 | Packages | `opi-coding-agent` | `pi` packages 和 package manager | 部分 | `package.toml` discovery、composition、`opi package add/remove/list/doctor` CLI、带 adapter 声明的 manifest V2，以及通过 `opi-extension-jsonl-v1` 运行的 process-JSONL adapter hosting 已存在。 | 保持 adapter kind 可扩展；在后续产品计划明确前不声明 marketplace/registry 支持。 |
| 4 | Custom provider/model registration | `opi-ai`、`opi-coding-agent` | `pi` custom provider extension points | 部分 | Registry registration 已存在；configured profiles 已接入 runtime provider construction 和 `--list-models`。 | 在 extension/package adapter 产品化后，把 extension-provided providers 接入终端用户 runtime paths。 |
| 4 | Branch selection | `opi-agent`、`opi-coding-agent`、`opi-tui` | `pi` session tree、fork、clone、branch selection | 部分 | `/branch` 和 `/tree` 打开分支/会话树选择器；`/fork`、`/clone` 和 `--fork` 会从活跃分支创建新的父子会话；从选中的 branch tip 继续会写入同文件 sibling path。 | 改善更丰富 branch metadata display 和 package-level workflows。 |
| 4 | Streaming proxy | `opi-agent` | `pi` process integration/proxy surfaces | 部分 | Streaming proxy primitives 已存在。 | 澄清 sync/async I/O 语义和生产路径接线。 |
| 4 | Web UI event/state/rendering | `opi-web-ui` | `pi-web-ui` browser package | 有意偏离 | 未发布 Rust consumer crate 已存在。 | 保持声明收窄，或单独创建 browser app 计划。 |
| 4 | MCP、sub-agent、plan mode、todo、permission gate examples | examples/packages | `pi` 将工作流重功能保持在核心之外 | 完整 | Examples/package scaffolds 位于核心之外。 | 除非通过 extension/package registration 路由，否则不要加入内置 CLI。 |
| 5 | Package store、CLI、manifest V2、adapter protocol、adapter host、adapter bridge、example adapters | `opi-coding-agent`、`opi-agent` | `pi` package manager 和 extension adapters | 部分 | `opi package add/remove/list/doctor`、local/git sources、带 `[adapter]` 的 manifest V2、通过 `opi-extension-jsonl-v1` 运行的 `process-jsonl` adapter hosting、adapter-to-runtime bridge（tool、command、hook、event、state、cancellation），以及可运行的 example adapter packages（todo、permission-gate、protected-paths）已存在。 | 在宣称广义 package ecosystem 前稳定 adapter protocol；保持 npm/marketplace 超出范围。 |

## 当前修复优先级

| 优先级 | 领域 | 原因 | 目标结果 |
|---:|---|---|---|
| P0 | 文档事实 | 版本和阶段状态必须匹配 `Cargo.toml` 与 `CHANGELOG.md`。 | 当前文档描述 `0.5.2` workspace，历史 `0.5.1` 行保持历史含义。 |
| P1 | Session tree | 同文件 branch continuation 现在已有运行时 `parent_id` 和 `leaf` 覆盖，但 `pi` 仍有更完整的 tree 产品工作流。 | 改善 branch metadata display 和更高层 package/workflow integration。 |
| P1 | Extension/package execution | 通过 `opi-extension-jsonl-v1` 运行的 process-JSONL adapter 会把 package command、tool、hook、event、state 和 cancellation 桥接进 runtime。Adapter hosting 和 example packages 已存在。 | 稳定 adapter protocol；API 稳定后再增加更广的 adapter kind（gRPC 等）。 |
| P1 | Provider profiles | OpenAI-compatible profiles 和 model metadata 已走 config + registry。 | 保持 profile expansion policy 文档化；OAuth providers 单独跟踪。 |
| P2 | Web UI scope | 当前 `opi-web-ui` 有意窄于 `pi-web-ui`。 | 公开文档不声称 standalone browser app。 |
| P2 | Rust module depth | `opi-agent` crate 边界仍合理；loop internals 已从 `lib.rs` 移出。 | 只在能提升 locality 时继续深化大型 runtime 区域，不改变 crate 边界。 |

## 维护规则

- Phase milestone、public extension surface、package workflow、provider family、session command 或 web-ui 声明变化时更新本矩阵。
- 英文文档有 localized counterpart 时，同一变更中同步中文 counterpart。
- 不要用本矩阵作为复制 TypeScript 模块结构到 Rust 的理由。
- 没有工作用户路径或库路径及测试时，不要标为“完整”。
- 历史发布行保持历史含义；不要把已发布版本段改写为当前 workspace 状态。
