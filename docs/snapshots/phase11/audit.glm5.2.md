# Phase 11 独立代码审计报告（GLM-5.2）

- 审计对象：opi 仓库 Phase 11（Tooling Quality），任务 11.1 – 11.11
- 审计基线：起始提交 `0a3add86`（v0.6.2）→ 终止提交 `3ae3d405`（11.11 HEAD）；HEAD `36996668` 的 `crates/` 源码与 `3ae3d405` 逐字节一致（之后的两个 chore 提交仅改动 `docs/snapshots/**`、`docs/superpowers/plans/**`、`.claude/skills/opi-implement/**`），因此本文行号均取自当前工作树（HEAD）。
- 审计依据：代码、测试、配置、文档与实际 `git diff`。**未读取** 仓库中已有的 `docs/snapshots/phase11/audit.exit-eval.md` 或任何其它 audit/review/评审 文件（详见 §2 污染声明）。
- 审计方法：14 路独立评审（每任务 + 5 个跨切面）+ 对每条发现做对抗式二次验证（共 61 个子代理，2.7M tokens）；4 路评审因子代理限流（429）失败，由本文作者对相应源码（`result.rs`/`tool.rs`/`edit.rs`/`agent_loop.rs`/`mod.rs`/`diagnostic.rs`/`event.rs`/`message.rs`/`bash.rs` 与 4 个 provider 文件）做了**第一手通读**补齐。
- 审计日期：2026-06-30。

---

## 1. 执行摘要（Executive Summary）

Phase 11 的实现整体**质量较高、DoD 基本达成、范围纪律（non-goals）守得住**：read/write/edit 的边界条件、CRLF/原子写、bash 的 cancel≠timeout 判别与 `StreamCapture` 字节完整性、nav 工具的一致化、MaxTurnsExceeded 死代码激活、provider `is_error` 线缆修复，均经第一手验证为**真实修复且有回归测试**。

但审计发现 **1 个跨多个文件的核心安全问题（命令/输出中的密钥泄露面）** 与 **1 个 DoD 范围性缺口（“后台 shell 守卫”名不副实）**，以及若干次要问题。这一密钥泄露主题并非“Phase 11 回归”——多数泄露通道是既有的或在源码注释中被明确“推迟到后续 wire 任务”——但 Phase 11.8 新增的 `ToolExecutionEnd.diagnostics` 线缆字段**显著放大了该泄露面**，且当前没有任何测试覆盖“命令中嵌入密钥”的路径。

发现计数（已对抗式验证）：

| 严重度 | 数量 | 说明 |
|---|---|---|
| Major | 6 | 密钥泄露面 5 个切面 + 后台 shell 守卫缺失 |
| Minor | 16 | 含一条第一手新发现（session JSONL 落盘泄露） |
| Nit | 13 | 测试诚实性、一致性、性能微瑕 |
| Info | 4 | 范围/信息性备注 |
| **小计（confirmed）** | **39 工作流 + 4 第一手补齐** | |
| Refuted | 2 | 经验证不成立（见 §7） |
| Uncertain | 2 | 需运行时方能定论 |

**总评**：Phase 11 可以发布，但建议在进入 Phase 12 之前处理 §3 的密钥泄露主题（至少补齐 `details`/`diagnostics` 在 event/落盘路径的 redaction，以及 `tool_result_no_leak.rs` 的覆盖空洞），并明确“后台 shell 守卫”的真实语义（见 M6）。

---

## 2. 审计方法与污染声明

**污染控制**：用户要求本次审计不得读取仓库中已有的审查报告。审计期间：

1. 全程**未打开** `docs/snapshots/phase11/audit.exit-eval.md`（57 行，由 phase 之后的 chore 提交创建）。
2. 唯一接触到的“既有审计信息”是 `opi-impl-state.json` 中 `phase_exit.11.evaluator_summary` 的一句话总结（“10/10 criteria met, Zero not-met, zero deferred”）——这是关于既有审计**结论的元数据**，不含其发现内容；本文结论不受其影响。
3. 所有 14 路评审子代理均被硬性禁止读取 `docs/snapshots/**` 及任何名称含 `audit`/`review`/`exit-eval`/`评审` 的文件。无子代理报告污染。

**对抗式验证**：每条发现都派一个“怀疑者”子代理重新读真实代码尝试**反驳**；只有代码确实呈现所述问题时才标记 `confirmed`。验证阶段有 2 条发现被反驳（§7）、2 条不确定。

**失败补齐**：因子代理限流，`substrate`/`edit`/`agent`/`portability` 四路评审未跑完；由本文作者对相关源码第一手通读补齐，结论与工作流其余部分一致。

---

## 3. 核心主题：bash 命令/输出中的密钥泄露面（Major）

bash 把模型可控的 `command` 字符串与命令输出**原样**注入多个出站通道。命令中常嵌入密钥（`curl -H "Authorization: Bearer …"`、`gh auth login --with-token`、`mysql -pSecret`、`aws ... --secret-key` 等）。Phase 11 的 no-leak DoD 范围很窄（仅“不 dump 继承的 env 值”，即 `details.env.values_included=false`），因此下列各切面**严格说不是 DoD 违例**，但它们共同构成一个被 11.8 新线缆字段放大的、**测试无覆盖**的真实密钥泄露面。

各通道状态（第一手核实）：

| 通道 | 是否 redact | 证据 | 切面 |
|---|---|---|---|
| Summary 模式 trace（经 `observe`→`tool_owned_diagnostic` 上抬） | **安全** | `diagnostic.rs:429-458` `CONTENT_SENSITIVE_KEYS` 含 `command`/`env`/`cwd`/`path`/`stdout`/`stderr`；`redact_summary` 命中即置 `[REDACTED]`；`trace.rs:241` 经 collector mode redact | — |
| **Verbose 模式 trace** | **泄露** | `diagnostic.rs:478-484` `redact()` 在 Verbose 下仅走 `SecretRedactor`，**不**做 content-sensitive 键 redaction | M3 |
| **NDJSON / RPC event（`ToolExecutionEnd.details` + `.diagnostics`）** | **泄露（原样）** | `agent_loop.rs:213-226` 与 `294-308` 两处把 `result.details.clone()` / `result.diagnostics.clone()` 直接喂给 `AgentEvent::ToolExecutionEnd`，**无 redact**；`event.rs:63-71` 注释明示“serialized verbatim — event-path redaction is deferred to a wire-format task” | M1 |
| **Session JSONL 落盘（at-rest）** | **泄露（原样）** | `message.rs:37-47` `ToolResultMessage.details` 是**未 skip 的 `Serialize` 字段**；该结构体随 `Message::ToolResult` 写入会话 JSONL（`agent_loop.rs:228-238`/`309-319` push 到 messages） | **M2（第一手新发现）** |
| Provider 请求体 | 安全 | 11.9 各转换器只序列化 `t.content`，不含 `details`（见 `anthropic.rs`/`openai_chat.rs`/`gemini.rs`/`openai_responses.rs` 的 11.9 diff） | — |
| **full_output 溢出文件** | **泄露（at-rest）** | `bash.rs:485-510` `write_merged_full_output` 用 `std::fs::File::create`（默认权限）写入 OS temp，**Done 分支永不清理**（注释称其为“keeper”） | M4 |
| 测试覆盖 | **无** | `tool_result_no_leak.rs:50-85` 仅验证 provider 请求体；**不**覆盖命令密钥/输出密钥/落盘路径 | M5 |

### M1 — `ToolExecutionEnd` 事件把 bash `command` 原样泄入 NDJSON/RPC（Major / security）
- **位置**：`crates/opi-agent/src/agent_loop.rs:213-226`、`294-308`；`crates/opi-agent/src/event.rs:63-71`
- **原因**：两个工具执行分支都 `events(AgentEvent::ToolExecutionEnd { details: result.details.clone(), diagnostics: result.diagnostics.clone(), .. })`，agent_loop 内**零处**调用 `redact`/`redacted_payload`。bash 的 `details.command`（`bash.rs:178,205,261` → `bash_operation_metadata`，`result.rs:84`）与 `diagnostics[].context.command`（`bash.rs:352-358`）携带原始命令字符串。
- **影响**：任何 NDJSON（schema v2）/ RPC（schema v3）消费方，以及任何进程内 `AgentEventSink` 消费方，都会看到含密钥的命令字符串。`event.rs:68-69` 注释把该泄露明确“推迟到后续 wire 任务”——这是已知技术债，非回归。
- **建议**：在 runner 的 NDJSON/RPC 输出边界对 `ToolExecutionEnd.details` 与 `.diagnostics` 调用 `redact(.., RedactionMode::Summary)`（与 trace 路径对齐）；或在 `event.rs` 序列化前 redact。同时把“推迟”注释升级为 tracked 残留项。

### M2 — `ToolResultMessage.details` 把 bash `command` 原样落盘到会话 JSONL（Major / security，第一手新发现）
- **位置**：`crates/opi-ai/src/message.rs:37-47`（`details` 非 skip 的 `Serialize` 字段）；写入路径 `agent_loop.rs:228-238`、`309-319`
- **原因**：`ToolResultMessage` 派生 `Serialize`，`details: Option<serde_json::Value>` 无 `skip_serializing`；该消息以 `AgentMessage::Llm(Message::ToolResult(trm))` 持久化到会话 JSONL。bash 结果的 `details` 含完整 `command`。
- **影响**：含密钥的命令以**明文 at-rest**形式留在会话文件中（`%APPDATA%\opi\sessions\…` / `~/.config/opi/sessions/…`），超出 Phase 11 “不 dump env 值”的窄范围。会话文件可能被同步、备份、上传。注意 provider 请求体**不受影响**（11.9 只序列化 `content`）——泄露仅在落盘与 event 通道。
- **建议**：要么对落盘的 `ToolResultMessage.details` 做 redaction，要么从会话持久化形态中剥离 bash `command`（仅保留 `exit_code`/`cancelled`/`timed_out`/`truncated` 等非密钥字段）。补充一条断言“命令密钥不出现在 session JSONL”的测试。

### M3 — Verbose 模式 trace 不做 content-sensitive redaction（Major / security）
- **位置**：`crates/opi-agent/src/diagnostic.rs:478-484, 494-520`
- **原因**：`redact()` 在 `RedactionMode::Verbose` 下仅 `SecretRedactor::default().redact(value)` 后原样返回；`redact_summary` 的 `CONTENT_SENSITIVE_KEYS`/`ABSOLUTE_PATH_RE` 仅在 Summary 模式生效。`command` 虽在敏感键表里，但 Verbose 下不被处理。
- **影响**：任何以 Verbose 模式采集 trace 的部署都会把 bash 命令（含密钥）写入 trace 信封。
- **建议**：让 Verbose 模式同样 redact `command`/`env`/`path` 这类结构性敏感键（密钥形态扫描 + 键名扫描二选一不可遗漏）。或明确文档化 Verbose 模式“仅供本地调试、不得外发”。

### M4 — `full_output` 溢出文件默认权限 + 永不清理（Major / security）
- **位置**：`crates/opi-coding-agent/src/tool/bash.rs:251-257, 479-510`（`write_merged_full_output`、`bash_output_temp_path`）
- **原因**：Done 分支在截断时调用 `write_merged_full_output`，用 `std::fs::File::create`（POSIX 下按 umask，常为 0644 → world-readable）写入 `std::env::temp_dir()`，文件名为 `opi-bash-output-{pid}-{nanos}.log`；该 merged 文件是“keeper”，**Done 路径不删除**（仅 per-stream spill 被 `cleanup_spill` 清理，`bash.rs:256-257`）。
- **影响**：命令的**完整 stdout+stderr**（可能含密钥，例如 `env`、`cat ~/.aws/credentials` 的输出）以世界可读权限长期滞留 OS temp，直到 OS 清理。文件名含 pid，且绝对路径会泄 OS 用户名（`bash.rs:498-510` 注释承认）。
- **建议**：(a) 用 `OpenOptions::new().mode(0o600)`（Unix）/ 限制性 ACL（Windows）创建；(b) 在结果返回前/进程退出前 best-effort 删除 keeper 文件，或改用 workspace 内 `.opi/` 受控目录以便纳入 `git status` 与清理；(c) 文件名加入线程 id 或随机量以消解碰撞（见 B4）。

### M5 — `tool_result_no_leak.rs` 不覆盖命令/输出密钥泄露路径（Major → 归类 Minor/test-quality）
- **位置**：`crates/opi-ai/tests/tool_result_no_leak.rs:50-85`
- **原因**：该守卫仅序列化 provider 请求体并断言不含某截断标记/敏感词；**不**注入“命令含 Bearer”或“stdout 含密钥”的夹具，也不检查 event/落盘路径。
- **影响**：M1–M4 全部泄露面**无测试守护**——任何回归都不会让构建变红。
- **建议**：新增覆盖：(1) bash 命令含哨兵密钥 → 断言 NDJSON/RPC event、session JSONL、Verbose trace 中均不见哨兵；(2) 命令输出含密钥 → 断言 `full_output` 文件在结果返回后被清理或权限受限。

### M6 — DoD 要求的“后台 shell/pty/session-pool 类型守卫”名不副实（Major / scope）
- **位置**：`crates/opi-coding-agent/src/tool/bash.rs:81-97`（spawn 点）；守卫测试 `crates/opi-coding-agent/tests/tools_read_write_edit_bash.rs`（`bash_tool_no_background_shell_symbols_guard`）与 `crates/opi-coding-agent/tests/phase11_tooling_quality_docs.rs:366-374`
- **原因**：11.6 DoD 明文要求“a Rust guard blocks any background-shell/pty/session-pool type”。实际实现只有一个**源码子串 grep 测试**（`bash_src.contains("status = child.wait()")` 等），它只能阻止 opi **自身代码**使用这些符号，**无法**阻止模型通过 `sh -c "sleep 1000 &"`（Unix）或 `cmd /C "start /B ..."`（Windows）后台化。spawn 点（`bash.rs:84-85`）无条件把原始 `command` 喂给 shell。
- **影响**：DoD 条目在“字面”上靠测试满足，但“禁止后台 shell”这一**产品意图**并未被任何结构化手段保证；非目标（Non-Goal）“No persistent background bash”仅靠模型自律与 opi 代码克制，存在被绕过空间。
- **建议**：要么把 DoD 措辞改为“禁止 opi 代码引入后台 shell/pty/session-pool 构件（源码 grep 守卫）”，承认无法在 shell 语法层阻止；要么若要真正阻止，需引入显式命令策略（拒绝含 `&`/`start /B`/`nohup`/`setsid`/`disown` 的命令——但这是产品策略变更，超出 Phase 11 范围，应登记为后续设计项）。当前应在 README/注释中如实陈述守卫的边界。

> 注：该主题与 `secrets`/`wire`/`scope` 三个工作流的发现重合；M1–M5 是同一根因（命令/输出原样出站、仅 trace-summary 通道 redact）的不同切面，建议作为**一个修复批次**处理。

---

## 4. 其余发现（Minor / Nit / Info）

> 下列每条均含：位置、原因、影响、建议。标注 **[第一手]** 表示本文作者亲自读码核实；其余为对抗式验证子代理 confirmed（证据文件/行号已核）。

### 4.1 Minor

**R1 — read 仅按行数截断，无字节预算，单行多 MiB 文件全量返回 [第一手]**
- 位置：`crates/opi-coding-agent/src/tool/read.rs:20-23, 165-210`
- 原因：DoD 写的是“default line/byte cap”，实现仅 `DEFAULT_READ_LINES=2000` 行截断；`read.rs:20-22` 注释把字节级截断“明确排除出 11.3 范围”。
- 影响：压缩/混淆的单行文件（minified JS、base64 blob、单行 JSON，数 MiB）行数为 1，永不触发截断，全量灌入模型上下文。
- 建议：要么修订 DoD 删去 “byte cap” 措辞、把注释升级为 tracked 残留；要么加二级字节预算（累加所选行字节数，超限即停并置 `truncated`）。

**R2 — read 的 NUL 二进制扫描整文件读入内存、无边界采样 [第一手]**
- 位置：`crates/opi-coding-agent/src/tool/read.rs:107-137`
- 原因：`tokio::fs::read` 整文件入 `Vec<u8>`，再 `bytes.contains(&0u8)` 全扫；无头部采样提前退出。
- 影响：巨型文本/二进制文件在截断逻辑生效前即被全量分配与扫描（性能悬崖，非正确性问题）。
- 建议：先读有界头部采样判 NUL，干净且小于文件时再降级全读；或登记为已知限制。

**R3 — read 把绝对 `resolved_path` 放进 `details`（扩大泄露半径）[第一手]**
- 位置：`crates/opi-coding-agent/src/tool/read.rs:195-205`（经 `result::path_metadata`）
- 影响：`resolved_path` 是绝对路径，流入 M1/M2 的 event/落盘通道。好在 `path`/`resolved_path`/`user_path` 已在 `CONTENT_SENSITIVE_KEYS`（Summary trace 安全），但 event/落盘路径仍原样（见 §3）。
- 建议：随 M1/M2 一并 redact；本条独立严重度 info，因它是既有形态被 11.8 放大。

**W1 — write 在“写 temp → rename”之间被取消/future drop 会残留 temp 文件**
- 位置：`crates/opi-coding-agent/src/tool/write.rs:165-176`
- 原因：temp 写入与 rename 之间存在窗口；若 `CancellationToken` 触发或 future 被 drop，`.opi-write-tmp-*` 可能留在目标目录。
- 影响：工作树出现孤立隐藏 temp 文件（非数据损坏，rename 原子性仍保证目标文件完整）。
- 建议：用 RAII guard（Drop 时 `remove_file`）包裹 temp 路径，覆盖 cancel/drop 路径。edit 同形（`edit.rs:310-323`）。

**B1 — bash `StreamCapture`/keeper 文件未用限制性权限创建（POSIX）**
- 位置：`crates/opi-coding-agent/src/tool/bash.rs:449-458`（`ensure_spill`：`std::fs::File::create`）
- 与 M4 同根因；per-stream spill 同样默认权限。建议同 M4(a)。

**B2 — bash temp 文件名仅 `pid+nanos`，快速连续调用存在 TOCTOU 碰撞**
- 位置：`crates/opi-coding-agent/src/tool/bash.rs:503-510`；edit 同形 `edit.rs:305-310`
- 原因：`File::create` 带 `O_TRUNC` 静默覆盖同名；同一 pid 在同一纳秒（或时钟分辨率不足，Windows `SystemTime` 纳秒分辨率有限）下并发 bash 调用会互相覆盖 spill。
- 影响：极端并发下 full_output 内容错乱（低概率）。
- 建议：文件名加入线程 id 或一次性随机量；或用 `tempfile` crate。

**B3 — bash `out_cap.total + err_cap.total` 用 wrapping 而非 saturating 加法**
- 位置：`crates/opi-coding-agent/src/tool/bash.rs:236-237`
- 影响：理论溢出（u64，实际不可达）；一致性 nit。
- 建议：改 `saturating_add` 以表达“不会溢出”的意图。

**B4 — bash WaitFailed 分支无 details/env-token，破坏“稳定操作键集合”契约的该分支**
- 位置：`crates/opi-coding-agent/src/tool/bash.rs:227-233`（`Control::WaitFailed` → 裸 `result::err`）[第一手]
- 原因：`result::err`（`result.rs:35-44`）硬编码 `details: None`；WaitFailed（以及 malformed-args `bash.rs:71`、spawn-fail `bash.rs:93`）不推 `ToolDiagnostic`，故 11.8 的 uniform lift 取不到这些分支的上下文。
- 影响：极少数错误路径（`child.wait()` 失败、参数非法、spawn 失败）在 trace/diagnostics 中缺少操作元数据；与 DoD“所有分支统一键集”存在边缘偏离。
- 建议：给这三个 `result::err` 分支也补一个 `ToolDiagnostic`（至少含 `command`），或修订 DoD 显式排除“进程未运行”的分支。

**B5 — bash cancel/timeout 吞掉 `child.kill()` 错误且未设 `kill_on_drop`，存在孤儿进程风险**
- 位置：`crates/opi-coding-agent/src/tool/bash.rs:84-89, 154-161`
- 原因：`let _ = child.kill().await;` 丢弃结果；`Command::new` 未 `.kill_on_drop(true)`。
- 影响：若 kill 失败、或 control 分支 `child.kill()` 进行中 future 被 drop，子进程可能残留。
- 建议：`.kill_on_drop(true)` 兜底；并记录 kill 失败（至少 trace 一条）。

**N1 — grep 对非 UTF-8 文件名做 lossy 转换，无 `UnsupportedEncoding` 诊断（与 find/glob/ls 不对称）**
- 位置：`crates/opi-coding-agent/src/tool/grep.rs:108-117`
- 影响：11.2 为路径工具建立了 `UnsupportedEncoding` 分类法，grep 对文件名静默 `to_string_lossy`（U+FFFD），破坏一致性。
- 建议：grep 命中非 UTF-8 名时计数并/或发 `CODE_TOOL_UNSUPPORTED_ENCODING`。

**N2 — grep 静默跳过非 UTF-8/二进制文件内容，无计数/诊断**
- 位置：`crates/opi-coding-agent/src/tool/grep.rs:104-107`
- 影响：用户无从得知哪些文件被跳过；与“clear diagnostics”目标相悖。
- 建议：在 `details` 暴露 `files_skipped_nonutf8` / `files_skipped_binary` 计数。

**N3 — ls 不在 `details` 暴露 `omitted_count`（nav 工具截断元数据不一致）**
- 位置：`crates/opi-coding-agent/src/tool/ls.rs:164-204`
- 影响：grep/find/glob 经 `cap_nav_results`（`mod.rs:190-210`）返回 `omitted_count`，ls 自走 `DEFAULT_MAX_ENTRIES` 不暴露该键，破坏“一致契约”。
- 建议：ls 截断时也置 `omitted_count`（即使常量分立）。

**P1 — OpenAI `[tool_error]` 标记在成功路径未转义，成功工具输出以该字面量开头会被模型误读**
- 位置：`crates/opi-ai/src/openai_chat.rs:1051-1082`；`openai_responses.rs` 同形
- 原因：失败时前缀 `[tool_error] `，但成功输出若恰好以该串开头（如返回包含该标记的日志），模型侧无 native 字段可区分。
- 影响：误分类风险（低概率但真实）；是“无 native 字段”的固有代价。
- 建议：文档化该限制；或换用更不可能碰撞的标记/同时附加结构化 metadata（若 Responses/Chat 未来支持）。

**D1 — docs-guard 的 EN 权限理由断言锁定了一个近乎无意义的松散子串 `'subsystem.'`**
- 位置：`crates/opi-coding-agent/tests/phase11_tooling_quality_docs.rs:245-249`
- 影响：测试仅检查 README 含 “subsystem.”，几乎任何文本都能通过；无法真正守住“permission rationale”内容。
- 建议：断言更具区分度的短语（如 “permission prompts are not a core feature”）。

**D2 — SC8 bash 前台等待断言是对本工作流之外文件的脆弱源码字面 grep**
- 位置：`crates/opi-coding-agent/tests/phase11_tooling_quality_docs.rs:366-373`（`bash_src.contains("status = child.wait()")`）
- 影响：源码重构（如改写 wait 调用）会让守卫假阴；且与 M6 同类——是源码 grep 而非行为证明。
- 建议：改为行为断言（每次 execute 只 spawn 一个子进程并对齐 wait），或显式声明此为源码形状守卫。

**T1 — 操作上下文（timed_out/cancelled）的 trace 镜像只在工具层 pinned，未在 agent_loop DiagnosticLinked 路径 pinned**
- 位置：`crates/opi-coding-agent/tests/tools_read_write_edit_bash.rs:2092-2170`
- 影响：11.8 的“diagnostics→trace”端到端在 agent_loop 边界缺少断言；若 lift 回归只退化到 `tool_name`，测试不红。
- 建议：加一条 agent_loop 级断言：bash 超时/取消的 `DiagnosticLinked` trace 记录携带 `timed_out`/`cancelled` 上下文（与 M1 修复一起做）。

**T2 — runner 级“denied before execution”测试断言 stdout/stderr 子串，而非“工具体从未运行”的哨兵**
- 位置：`crates/opi-coding-agent/tests/non_interactive_policy.rs:26-130`
- 影响：无法证明 write/edit/bash 的函数体真的没跑（仅证明返回了拒绝消息）。
- 建议：用一个“若执行则写文件/递增计数器”的哨兵工具，断言哨兵未被触发。

**T3 — 无测试验证 tool-owned diagnostics 传播到 `AgentEvent::ToolExecutionEnd.diagnostics` 线缆字段**
- 位置：`crates/opi-agent/tests/truncated_propagation.rs:100-138`
- 影响：11.8 新增的 `diagnostics` 字段（event.rs:70-71）在 `truncated_propagation` 里只断言 `truncated`，diagnostics 字段的端到端传播无守护——而这正是 M1 泄露的同一字段。
- 建议：补一条断言：失败工具结果的 `diagnostics` 出现在 `ToolExecutionEnd`（并随 M1 redact 后断言不含命令密钥）。

**S1 — `full_output` 键不在 `CONTENT_SENSITIVE_KEYS`** ⚠ **经对抗验证为 REFUTED（见 §7）**：其值是绝对路径，会被 `ABSOLUTE_PATH_RE`（`diagnostic.rs:465-470`）在 Summary 模式 redact。故该项**不计入**有效发现，列此仅为说明审计曾检查并排除。

### 4.2 Nit

| ID | 位置 | 摘要 | 建议 |
|---|---|---|---|
| n1 | `bash.rs:185` 隐含 | read 截断标记 `... N lines omitted` 对 N==1 不区分单复数 | `1 line omitted` vs `N lines omitted` |
| n2 | `write.rs:113-115` | async `execute` 内用阻塞 `std::fs`（`exists`、`first_file_ancestor` 的 metadata 循环） | 改 `tokio::fs` 或 `spawn_blocking` |
| n3 | `bash.rs:241-246` | merged preview 按 `MAX_BASH_OUTPUT_BYTES` 字节切片用 `from_utf8_lossy`，边界可能切坏多字节 → U+FFFD | 按字符边界回退（仿 `edit.rs:406-415`） |
| n4 | `bash.rs:362-375` | `with_env_policy` 原地改 `Value`，绕过“禁手写 details”结构守卫 | 守卫扩展到覆盖 builder 返回后的 in-place 注入 |
| n5 | `bash.rs:569-588` | exact-fit-then-overflow 回归仅作 StreamCapture 单元测试，未在 bash 集成层复现 | 加一条集成级截断测试 |
| n6 | `grep.rs:82-118` | grep 取消轮询粒度为 per-entry，单次超大读盘仍要等读完 | 在读盘循环内插取消点 |
| n7 | `mod.rs:160-169` | `nav_walk_builder` 同时 `git_ignore(true)` 与 `add_custom_ignore_filename(".gitignore")`，重复注册（注释解释为“非 git 仓库也尊重 .gitignore”，合理） | 文档化该意图，避免误读为 bug |
| n8 | `openai_chat.rs:1076-1082` | `TOOL_ERROR_MARKER` 在 chat/responses 两模块私有重复，仅靠注释声称有测试 pin | 确认 `tool_result_wire.rs` 确有字节同一性断言（ledger 称有） |
| n9 | `non_interactive.rs:428-431` | CLI help “mutating” 断言仅证 doc-comment 词存在，非用户能学到 opt-in | 断言 `--allow-mutating` 出现在渲染后的 help |
| n10 | `phase11_tooling_quality_docs.rs:272-286` | help-render 守卫在两个测试文件重复 | 抽公共 helper |
| n11 | `phase11_tooling_quality_docs.rs:183-190` | bash shell 守卫用大小写敏感 `contains("cmd /C")`，大小写回归会假阴 | 大小写不敏感或正则 |
| n12 | `agent_loop.rs:937-945` | `tool_owned_diagnostic` 对 null/非对象 context 包成 `{"context": null,..}`（bash 不触发） | null 时直接 `{"tool_name":..}` |
| n13 | `agent_loop_semantics.rs:612-661` | `phase8_tool_scheduling_contract` 并行证明依赖真实 60ms/5ms sleep，时序敏感 | 用 mock 计时/通道序断言 |
| n14 | `tool_result_wire.rs:344-415` | no-Phase12-breadth 守卫是子串黑名单 + 冻结模块表，可被重命名绕过 | 文档化为“意图守卫” |
| n15 | `phase11_tooling_quality_docs.rs:366-374` | bash 前台正向控制是源码子串 grep，非行为 | 同 D2 |
| n16 | `README.md:111-124` | README 未显式把 glob 框定为“opi 额外便利、非 pi-parity” | 补一句对齐 `## Relationship to pi` |

### 4.3 Info

- **i1**：`result.rs:100-108` `WorkspaceRelation::Unresolved` 是自文档化的死变体（`resolve_tool_path` 出错即返回 `Err`，从不填 Unresolved）。建议保留为保留值并在 doc 标注，或删除。[第一手]
- **i2**：`tool.rs:69-79` `ToolResult::from_validation_error` 不推 `ToolDiagnostic`（与 B4 同类的“验证错误无 per-cause 诊断”一致性 nit）。[第一手]
- **i3**：`write.rs:113-118` 存在性 probe 的 TOCTOU 仅影响审计标注（`action`/`bytes_before`），不影响安全（原子 temp+rename 仍保证完整性）。信息性。[第一手]
- **i4**：`event.rs:55-72` 新事件字段在 NDJSON v2 / RPC v3 下对 serde 是加性的（`truncated`/`diagnostics` 均 `#[serde(default)]`/`skip_serializing_if`），但对手写严格解析器是破坏性；未升 schema 版本。建议在 CHANGELOG 显式提示 embedder。`message.rs:42` 的 `details` 字段反向兼容良好（Option，旧会话反序列化为 None）。

---

## 5. 逐任务 DoD 覆盖评估

| 任务 | DoD 达成 | 主要残留（见 §3/§4） |
|---|---|---|
| 11.1 工具结果契约 | ✅ 基本达成 | `from_validation_error` 无诊断（i2）；Unresolved 死变体（i1）；n4 守卫可绕过 |
| 11.2 文件系统分类法 | ✅ 达成 | `UnsupportedEncoding` 单文件用例直构诊断、变体为目录形（`edit.rs:222-241`），轻微不一致 |
| 11.3 read 加固 | ⚠ 部分 | R1 字节级 cap 未做（DoD 措辞 vs 注释自排除）；R2 NUL 扫描性能 |
| 11.4 write 加固 | ✅ 达成 | W1 cancel/drop 残留 temp；n2 阻塞 std::fs in async |
| 11.5 edit 加固 | ✅ 达成（第一手：CRLF/原子写/多匹配/no-fuzzy/1MiB 守卫均实装且正确） | temp 文件名 TOCTOU（B2 同形）；no-fuzzy 守卫为源码 grep（依赖测试诚实性） |
| 11.6 bash 加固 | ⚠ 部分 | **M6 后台 shell 守卫名不副实**；M1/M4/M5 命令/输出密钥泄露；B4/B5 分支与 kill_on_drop |
| 11.7 nav 一致化 | ✅ 基本达成 | N1/N2 grep 非 UTF-8 不对称；N3 ls 无 omitted_count；n6 取消粒度 |
| 11.8 诊断/trace/MaxTurns/调度 | ⚠ 部分 | **M1/M2/M3 命令泄露面（本阶段新放大）**；T1/T3 trace/wire 传播测试缺；MaxTurnsExceeded 已正确激活（第一手）✅ |
| 11.9 provider is_error | ✅ 达成（第一手：Anthropic native / Gemini response.error / OpenAI 标记 / Bedrock status 均正确，成功体字节同一） | P1 标记碰撞；n8 常量重复 |
| 11.10 文档 | ✅ 达成 | EN/ZH 基本同步；D1 松散断言；n16 glob 框定 |
| 11.11 docs-guard/CLI/SC8 | ✅ 达成 | D2/n11/n15 源码 grep 式守卫；T2 哨兵缺失 |

**MaxTurnsExceeded 专项（第一手）**：`agent_loop.rs:530-534` 在 for 循环穿底且 `has_tools_pending` 时返回 `Err(AgentError::MaxTurnsExceeded)`，并在 `diagnostic.rs:661-668` 分类为 `CODE_AGENT_MAX_TURNS_EXCEEDED`（Warning + 行动建议）。原 `diagnostic.rs:467-474` 的死分类已激活。该 DoD 条目**真实满足**。

**cancel≠timeout 专项（第一手）**：`bash.rs:151-167` 用 `tokio::select! { biased; cancel, timeout, wait }`，cancel 优先于 timeout；Cancelled 分支 `cancelled=true/timed_out=false`、exit_code=None（`bash.rs:181-184`），TimedOut 分支 `timed_out=true/cancelled=false`、exit_code=None（`bash.rs:208-211`）。`StreamCapture` 经 `ensure_spill`（`bash.rs:449-458`）在首次溢出时用冻结的 cap 字节预览 seed spill 文件，exact-fit-then-overflow 字节完整性已修复且有回归测试（`bash.rs:569-588`）。**该 DoD 条目真实满足**。

---

## 6. 跨切面小结

- **测试诚实性（test-quality）**：Phase 11 大量“守卫”实为源码子串 grep（no-fuzzy、no-background-shell、no-Phase12-breadth、SC8 前台、docs 同步），它们能守住“opi 代码不引入 X”，但**不能**守住产品意图（模型无法后台化、provider 无 breadth）。多个测试断言子串而非哨兵/行为（T2、D1、D2、n11、n15）。建议区分“源码形状守卫”与“行为守卫”并在测试名/注释标明。
- **范围/非目标（scope/non-goals）**：9 个非目标均未以“构件”形式进入 core（无 sandbox、无 pty/session-pool crate、无 LSP、无 IDE 索引、无 auto-format、无新工作流工具、无 remote exec、无 package 扩展、无新 provider family）。唯一名实不符是 M6（后台 shell）。glob 仍为 opi 便利（n16 建议更显式）。
- **线缆兼容（wire-compat）**：新字段（`truncated`/`diagnostics`）对 serde 加性、旧会话可恢复（`details` Option、`truncated` default）；但未升 NDJSON v2 / RPC v3 版本号，对手写严格解析器是破坏性（i4），且 `ToolExecutionEnd.diagnostics` 是新泄露通道（M1）。schema 版本策略需在 CHANGELOG 显式提示。

---

## 7. 经检查并排除/未定论项（审计可信度）

对抗式验证反驳/未定的发现，列此以示审计边界：

- **Refuted-1**：声称“`full_output` 不在 `CONTENT_SENSITIVE_KEYS` 故其路径在 Summary 模式泄露”——**反驳成立**：`full_output` 的**值**是绝对路径，命中 `ABSOLUTE_PATH_RE`（`diagnostic.rs:465-470`）被 redact。该发现不计入。
- **Refuted-2 / Uncertain-×2**：另有一条在验证中被反驳、两条因需运行时方能定论而标记 uncertain（涉及 provider marker 在真实 API 接受度、Windows junction 在 CI 上的可重现性）。这些未计入有效发现，但提示 P1 与 Windows 路径分支值得在 Phase 12 的真实环境/CI 矩阵中再核。

---

## 8. 建议的修复批次与优先级

1. **P0（安全批次，合并处理 §3 M1–M5 + T3）**：在 runner NDJSON/RPC 输出边界与 session JSONL 落盘前，对 `ToolExecutionEnd.details`/`.diagnostics` 与 `ToolResultMessage.details` 调用 `redact(.., Summary)`；让 Verbose trace 也 redact 结构性敏感键；`full_output`/spill 文件用 `0o600` 并在 Done 路径清理；补 `tool_result_no_leak.rs` 的“命令含密钥/输出含密钥/落盘不含密钥/文件已清理”覆盖。
2. **P1（DoD 措辞/守卫诚实性）**：M6、D1、D2、T2、n4、n11、n15——要么把守卫升级为行为断言，要么修订 DoD/注释如实陈述守卫边界。
3. **P2（一致性/正确性微瑕）**：B4（WaitFailed 等分支补诊断）、B5（kill_on_drop）、N1/N2/N3（nav 一致性）、R1（read 字节预算，或修订 DoD）、W1（temp RAII）、B2（temp 文件名加随机）。
4. **P3（信息性/文档）**：i1/i2/i3/i4、n7/n8/n10/n16、CHANGELOG 提示 schema 加性变更。

---

## 附：审计覆盖说明

- **第一手通读**（本文作者）：`bash.rs`、`tool/mod.rs`、`tool/result.rs`、`tool.rs`、`diagnostic.rs`（codes + redaction）、`event.rs`、`agent_loop.rs`（MaxTurns + 两个 ToolExecutionEnd 站点 + lift）、`tool/edit.rs`、`message.rs`、`anthropic.rs`/`openai_chat.rs`/`openai_responses.rs`/`gemini.rs` 的 11.9 diff。
- **工作流对抗式验证覆盖**：read/write/bash/nav/provider/docs/secrets/testquality/wire/scope 共 10 路（43 confirmed / 2 refuted / 2 uncertain）。
- **未覆盖/弱覆盖**：`read.rs`/`write.rs`/`grep.rs`/`find.rs`/`ls.rs`/`glob.rs`/`harness.rs`/`runner.rs`/`policy.rs`/`cli.rs` 的**逐行**第一手复核依赖工作流（已验证）；如需更高置信，建议对 §4 中标 [第一手] 以外的 minor/nit 按需二次复核。
