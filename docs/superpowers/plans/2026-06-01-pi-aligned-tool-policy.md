# Pi-Aligned Tool Policy Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Align opi's interactive default tool behavior with pi while preserving conservative non-interactive automation defaults and explicit path policy.

**Architecture:** Centralize active tool resolution in `opi-coding-agent::policy`, then make `CodingHarness` consume resolved tool runtime config instead of exposing tools that hooks deny after prompt construction. Move file path handling from a hard-coded workspace-only helper to a reusable path resolver with `PathPolicy`, and document the new available-vs-active tool model in README and `docs/opi-spec.md`.

**Tech Stack:** Rust 2024, `thiserror`, `clap`, `tokio`, `serde_json`, `tempfile`, existing `opi-ai` mock provider test support.

---

## Source Spec

Design: `docs/superpowers/specs/2026-06-01-pi-aligned-tool-policy-design.md`

Repository rule: do not commit unless the user explicitly asks. This plan intentionally omits commit steps.

## File Structure

- Modify `crates/opi-coding-agent/src/policy.rs`: own `RunMode`, active built-in defaults, mutating-tool validation, and resolved tool runtime config.
- Modify `crates/opi-coding-agent/src/harness.rs`: construct built-in tools from resolved active names; make interactive hooks pass-through; give `read` the correct path policy by run mode.
- Modify `crates/opi-coding-agent/src/runner.rs`: make non-interactive runner construction fallible when tool policy is invalid; use non-interactive tool defaults.
- Modify `crates/opi-coding-agent/src/main.rs`: resolve non-interactive tool policy before provider construction and return config exit code on invalid mutating allowlists.
- Modify `crates/opi-coding-agent/src/cli.rs`: update `--allow-mutating` help text.
- Modify `crates/opi-coding-agent/src/tool/mod.rs`: replace `validate_workspace_path` with explicit path resolution and `PathPolicy`.
- Modify `crates/opi-coding-agent/src/tool/read.rs`: accept `PathPolicy`, support outside-workspace reads when configured, record resolved path metadata.
- Modify `crates/opi-coding-agent/src/tool/write.rs`: use `PathPolicy::WorkspaceOnly` resolver and record path metadata.
- Modify `crates/opi-coding-agent/src/tool/edit.rs`: use `PathPolicy::WorkspaceOnly` resolver and record path metadata.
- Modify `crates/opi-coding-agent/tests/tool_selection.rs`: assert pi-aligned interactive defaults and read-only non-interactive defaults.
- Modify `crates/opi-coding-agent/tests/non_interactive_policy.rs`: assert mutating tools are not advertised by default and opt-in enables coding tools.
- Modify `crates/opi-coding-agent/tests/safety_hooks.rs`: replace interactive mutating-denial tests with pass-through hook and policy validation tests.
- Modify `crates/opi-coding-agent/tests/tools_read_write_edit_bash.rs`: add path-policy tests and update read outside-workspace expectations.
- Modify `crates/opi-coding-agent/README.md`: document available tools, active defaults, and path policy.
- Modify `crates/opi-coding-agent/README.zh.md`: mirror README behavior changes in Chinese.
- Modify `docs/opi-spec.md`: update stale Phase 3 status and tool safety semantics.

## Task 1: Centralize Active Tool Policy

**Files:**
- Modify: `crates/opi-coding-agent/src/policy.rs`
- Test: `crates/opi-coding-agent/tests/tool_selection.rs`

- [ ] **Step 1: Replace policy tests for default active tool behavior**

Edit `crates/opi-coding-agent/tests/tool_selection.rs`.

Add imports:

```rust
use opi_coding_agent::policy::{
    RunMode, ToolFlags, ToolSelection, ToolRuntimeConfig, filter_tool_names,
    resolve_tool_selection,
};
```

Replace `filter_default_includes_all` with:

```rust
#[test]
fn interactive_default_active_tools_match_pi() {
    let config = ToolRuntimeConfig::resolve(RunMode::Interactive, false, ToolSelection::Default)
        .expect("interactive default should be valid");
    assert_eq!(
        config.active_tool_names,
        vec!["read", "write", "edit", "bash"]
    );
}
```

Add these tests below it:

```rust
#[test]
fn non_interactive_default_active_tools_are_read_only() {
    let config =
        ToolRuntimeConfig::resolve(RunMode::NonInteractive, false, ToolSelection::Default)
            .expect("non-interactive read-only default should be valid");
    assert_eq!(
        config.active_tool_names,
        vec!["read", "grep", "find", "ls", "glob"]
    );
}

#[test]
fn non_interactive_mutating_opt_in_uses_coding_tools() {
    let config =
        ToolRuntimeConfig::resolve(RunMode::NonInteractive, true, ToolSelection::Default)
            .expect("non-interactive mutating default should be valid");
    assert_eq!(
        config.active_tool_names,
        vec!["read", "write", "edit", "bash"]
    );
}

#[test]
fn non_interactive_allowlisted_mutating_tool_requires_opt_in() {
    let error = ToolRuntimeConfig::resolve(
        RunMode::NonInteractive,
        false,
        ToolSelection::Allowlist(vec!["read".into(), "bash".into()]),
    )
    .expect_err("bash should require mutating opt-in");
    assert!(
        error
            .to_string()
            .contains("mutating tool 'bash' requires --allow-mutating")
    );
}

#[test]
fn non_interactive_allowlisted_mutating_tool_allowed_with_opt_in() {
    let config = ToolRuntimeConfig::resolve(
        RunMode::NonInteractive,
        true,
        ToolSelection::Allowlist(vec!["read".into(), "bash".into()]),
    )
    .expect("bash should be valid with mutating opt-in");
    assert_eq!(config.active_tool_names, vec!["read", "bash"]);
}
```

Update `filter_allowlist_preserves_order` to reflect the pi-compatible built-in order:

```rust
#[test]
fn filter_allowlist_preserves_builtin_order() {
    let all = vec!["read", "write", "edit", "bash", "grep", "find", "ls", "glob"];
    let result = filter_tool_names(
        &all,
        &ToolSelection::Allowlist(vec!["grep".into(), "read".into(), "bash".into()]),
    );
    assert_eq!(result, vec!["read", "bash", "grep"]);
}
```

- [ ] **Step 2: Run the focused failing tests**

Run:

```sh
cargo test -p opi-coding-agent --test tool_selection
```

Expected: compile failure because `RunMode` and `ToolRuntimeConfig` do not exist yet.

- [ ] **Step 3: Implement policy types and resolver**

Edit `crates/opi-coding-agent/src/policy.rs`.

Add after `is_mutating_tool`:

```rust
/// Application mode used to resolve default active tools.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunMode {
    Interactive,
    NonInteractive,
}

/// Built-in tools in stable prompt order.
pub const BUILTIN_TOOL_NAMES: &[&str] = &[
    "read", "write", "edit", "bash", "grep", "find", "ls", "glob",
];

const CODING_DEFAULT_TOOLS: &[&str] = &["read", "write", "edit", "bash"];
const READ_ONLY_DEFAULT_TOOLS: &[&str] = &["read", "grep", "find", "ls", "glob"];

/// Final tool runtime config consumed by CodingHarness.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolRuntimeConfig {
    pub run_mode: RunMode,
    pub active_tool_names: Vec<String>,
}

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum ToolPolicyError {
    #[error("mutating tool '{tool}' requires --allow-mutating in non-interactive mode")]
    MutatingToolRequiresOptIn { tool: String },
}

impl ToolRuntimeConfig {
    pub fn resolve(
        run_mode: RunMode,
        allow_mutating: bool,
        selection: ToolSelection,
    ) -> Result<Self, ToolPolicyError> {
        let active_tool_names = resolve_active_tool_names(run_mode, allow_mutating, &selection)?;
        Ok(Self {
            run_mode,
            active_tool_names,
        })
    }
}

fn resolve_active_tool_names(
    run_mode: RunMode,
    allow_mutating: bool,
    selection: &ToolSelection,
) -> Result<Vec<String>, ToolPolicyError> {
    match selection {
        ToolSelection::Disabled | ToolSelection::NoBuiltin => Ok(Vec::new()),
        ToolSelection::Allowlist(names) => {
            if run_mode == RunMode::NonInteractive && !allow_mutating {
                if let Some(tool) = names.iter().find(|name| is_mutating_tool(name)) {
                    return Err(ToolPolicyError::MutatingToolRequiresOptIn { tool: tool.clone() });
                }
            }
            Ok(filter_tool_names(BUILTIN_TOOL_NAMES, selection))
        }
        ToolSelection::Default => {
            let names = match (run_mode, allow_mutating) {
                (RunMode::Interactive, _) | (RunMode::NonInteractive, true) => CODING_DEFAULT_TOOLS,
                (RunMode::NonInteractive, false) => READ_ONLY_DEFAULT_TOOLS,
            };
            Ok(names.iter().map(|name| (*name).to_owned()).collect())
        }
    }
}
```

Keep `resolve_tool_selection` unchanged. Keep `filter_tool_names` unchanged except tests now pass `BUILTIN_TOOL_NAMES`.

- [ ] **Step 4: Run policy tests**

Run:

```sh
cargo test -p opi-coding-agent --test tool_selection
```

Expected: policy unit tests pass; harness integration tests may still fail because `CodingHarness` still uses old default tool construction.

## Task 2: Make Harness Tool Visibility Match Policy

**Files:**
- Modify: `crates/opi-coding-agent/src/harness.rs`
- Modify: `crates/opi-coding-agent/src/runner.rs`
- Modify: `crates/opi-coding-agent/src/main.rs`
- Test: `crates/opi-coding-agent/tests/tool_selection.rs`
- Test: `crates/opi-coding-agent/tests/non_interactive_policy.rs`
- Test: `crates/opi-coding-agent/tests/safety_hooks.rs`

- [ ] **Step 1: Update harness integration tests**

In `crates/opi-coding-agent/tests/tool_selection.rs`, replace `harness_default_includes_all_tools` with:

```rust
#[tokio::test]
async fn harness_default_includes_interactive_coding_tools() {
    let workspace = create_temp_workspace();
    let mock = MockProvider::new("mock", vec![text_response("done")]);

    let harness = CodingHarness::new(
        Box::new(mock),
        "mock:mock-model".into(),
        OpiConfig::default(),
        workspace.path().to_path_buf(),
    );

    let system = harness.system_prompt();
    for tool in &["read", "write", "edit", "bash"] {
        assert!(
            system.contains(&format!("- {tool}:")),
            "Default interactive selection should include tool '{tool}'"
        );
    }
    for tool in &["grep", "find", "ls", "glob"] {
        assert!(
            !system.contains(&format!("- {tool}:")),
            "Default interactive selection should not include tool '{tool}'"
        );
    }
}
```

Add this non-interactive harness construction test:

```rust
#[tokio::test]
async fn harness_non_interactive_default_includes_read_only_tools() {
    let workspace = create_temp_workspace();
    let mock = MockProvider::new("mock", vec![text_response("done")]);
    let tool_config = ToolRuntimeConfig::resolve(
        RunMode::NonInteractive,
        false,
        ToolSelection::Default,
    )
    .expect("tool config");

    let harness = CodingHarness::new_with_tool_config(
        Box::new(mock),
        "mock:mock-model".into(),
        OpiConfig::default(),
        workspace.path().to_path_buf(),
        tool_config,
    );

    let system = harness.system_prompt();
    for tool in &["read", "grep", "find", "ls", "glob"] {
        assert!(
            system.contains(&format!("- {tool}:")),
            "Non-interactive default should include tool '{tool}'"
        );
    }
    for tool in &["bash", "edit", "write"] {
        assert!(
            !system.contains(&format!("- {tool}:")),
            "Non-interactive default should not include tool '{tool}'"
        );
    }
}
```

- [ ] **Step 2: Run the focused failing tests**

Run:

```sh
cargo test -p opi-coding-agent --test tool_selection
```

Expected: compile failure because `CodingHarness::new_with_tool_config` does not exist.

- [ ] **Step 3: Add tool config constructors to `CodingHarness`**

Edit `crates/opi-coding-agent/src/harness.rs`.

Change policy import:

```rust
use crate::policy::{RunMode, ToolRuntimeConfig, ToolSelection};
```

Add this constructor near `new_with_selection`:

```rust
/// Create a new harness with already resolved tool runtime config.
pub fn new_with_tool_config(
    provider: Box<dyn Provider>,
    model: String,
    config: OpiConfig,
    workspace_root: PathBuf,
    tool_config: ToolRuntimeConfig,
) -> Self {
    Self::new_with_hooks_and_resume_tool_config(
        provider,
        model,
        config,
        workspace_root,
        Box::new(CodingAgentHooks),
        None,
        Vec::new(),
        None,
        tool_config,
    )
}
```

Add a wrapping constructor below `new_with_hooks_and_resume`:

```rust
#[allow(clippy::too_many_arguments)]
pub fn new_with_hooks_and_resume_tool_config(
    provider: Box<dyn Provider>,
    model: String,
    config: OpiConfig,
    workspace_root: PathBuf,
    hooks: Box<dyn AgentHooks>,
    user_system_prompt: Option<String>,
    initial_messages: Vec<AgentMessage>,
    resume: Option<ResumeInfo>,
    tool_config: ToolRuntimeConfig,
) -> Self {
    Self::new_with_global_config_dir_tool_config(
        provider,
        model,
        config,
        workspace_root,
        hooks,
        user_system_prompt,
        initial_messages,
        resume,
        tool_config,
        None,
    )
}
```

Create a new internal implementation that mirrors `new_with_global_config_dir`, but takes `ToolRuntimeConfig` instead of `ToolSelection`:

```rust
#[allow(clippy::too_many_arguments)]
pub fn new_with_global_config_dir_tool_config(
    provider: Box<dyn Provider>,
    model: String,
    config: OpiConfig,
    workspace_root: PathBuf,
    hooks: Box<dyn AgentHooks>,
    user_system_prompt: Option<String>,
    initial_messages: Vec<AgentMessage>,
    resume: Option<ResumeInfo>,
    tool_config: ToolRuntimeConfig,
    global_config_dir: Option<PathBuf>,
) -> Self {
    let tools = Self::build_tools(&workspace_root, &tool_config);
    let tool_defs: Vec<_> = tools.iter().map(|t| t.definition()).collect();
    let mut builder = SystemPromptBuilder::new().tools(tool_defs);
    /* keep the rest of the current new_with_global_config_dir body unchanged */
}
```

Then rewrite the existing `new_with_global_config_dir` to resolve interactive defaults and delegate:

```rust
let tool_config =
    ToolRuntimeConfig::resolve(RunMode::Interactive, true, tool_selection)
        .expect("interactive tool config should be valid");
Self::new_with_global_config_dir_tool_config(
    provider,
    model,
    config,
    workspace_root,
    hooks,
    user_system_prompt,
    initial_messages,
    resume,
    tool_config,
    global_config_dir,
)
```

- [ ] **Step 4: Change tool construction to active names**

In `crates/opi-coding-agent/src/harness.rs`, replace `build_tools` with:

```rust
fn build_tools(workspace_root: &Path, tool_config: &ToolRuntimeConfig) -> Vec<Box<dyn Tool>> {
    let read_policy = match tool_config.run_mode {
        RunMode::Interactive => crate::tool::PathPolicy::AllowOutsideWorkspace,
        RunMode::NonInteractive => crate::tool::PathPolicy::WorkspaceOnly,
    };

    let mut tools: Vec<(&str, Box<dyn Tool>)> = vec![
        (
            "read",
            Box::new(ReadTool::new_with_policy(
                workspace_root.to_path_buf(),
                read_policy,
            )),
        ),
        ("bash", Box::new(BashTool::new(workspace_root.to_path_buf()))),
        ("edit", Box::new(EditTool::new(workspace_root.to_path_buf()))),
        ("write", Box::new(WriteTool::new(workspace_root.to_path_buf()))),
        ("grep", Box::new(GrepTool::new(workspace_root.to_path_buf()))),
        ("find", Box::new(FindTool::new(workspace_root.to_path_buf()))),
        ("ls", Box::new(LsTool::new(workspace_root.to_path_buf()))),
        ("glob", Box::new(GlobTool::new(workspace_root.to_path_buf()))),
    ];

    tools
        .drain(..)
        .filter(|(name, _)| tool_config.active_tool_names.iter().any(|active| active == name))
        .map(|(_, tool)| tool)
        .collect()
}
```

This step will not compile until Task 3 adds `PathPolicy` and `ReadTool::new_with_policy`.

- [ ] **Step 5: Make interactive hooks pass-through**

In `crates/opi-coding-agent/src/harness.rs`, replace `InteractiveCodingHooks` with:

```rust
/// Interactive hooks for the coding agent.
///
/// Tool safety is controlled by active tool selection and future extension
/// hooks, not by a core interactive permission popup.
pub struct InteractiveCodingHooks;

impl InteractiveCodingHooks {
    pub fn new(_allow_mutating: bool) -> Self {
        Self
    }
}

impl AgentHooks for InteractiveCodingHooks {
    fn convert_to_llm(&self, messages: &[AgentMessage]) -> Result<Vec<Message>, AgentError> {
        Ok(agent_messages_to_llm(messages))
    }
}
```

- [ ] **Step 6: Make non-interactive runner construction fallible**

Edit `crates/opi-coding-agent/src/runner.rs`.

Change imports:

```rust
use crate::policy::{RunMode, ToolPolicyError, ToolRuntimeConfig, ToolSelection, is_mutating_tool};
```

Change `NonInteractiveRunner::new` and `new_with_resume` to return `Result<Self, ToolPolicyError>`:

```rust
pub fn new(...) -> Result<Self, ToolPolicyError> {
    Self::new_with_resume(
        provider,
        model,
        config,
        workspace_root,
        allow_mutating,
        user_system_prompt,
        initial_messages,
        None,
        ToolSelection::Default,
    )
}
```

In `new_with_resume`, resolve tool config before creating the harness:

```rust
let tool_config =
    ToolRuntimeConfig::resolve(RunMode::NonInteractive, allow_mutating, tool_selection)?;
let hooks = Box::new(NonInteractiveHooks { allow_mutating });
let harness = CodingHarness::new_with_hooks_and_resume_tool_config(
    provider,
    model,
    config,
    workspace_root,
    hooks,
    user_system_prompt,
    initial_messages,
    resume_info,
    tool_config,
);
Ok(Self { harness })
```

Keep `NonInteractiveHooks` as defense-in-depth. It should rarely deny now because mutating tools are not active without opt-in.

- [ ] **Step 7: Update main to surface policy errors before running**

Edit `crates/opi-coding-agent/src/main.rs`.

In `run_non_interactive`, handle the fallible runner:

```rust
let mut runner = match NonInteractiveRunner::new_with_resume(
    provider,
    config.defaults.model.clone(),
    config.clone(),
    workspace_root,
    allow_mutating,
    user_system_prompt,
    resumed_messages.unwrap_or_default(),
    resume_info,
    tool_selection,
) {
    Ok(runner) => runner,
    Err(e) => {
        eprintln!("opi: {e}");
        return ExitCode::ConfigError as i32;
    }
};
```

In `run_interactive`, stop using config to control mutating permission and resolve interactive tool config:

```rust
let hooks = Box::new(InteractiveCodingHooks::new(true));
let tool_config = opi_coding_agent::policy::ToolRuntimeConfig::resolve(
    opi_coding_agent::policy::RunMode::Interactive,
    true,
    tool_selection,
)
.expect("interactive tool config should be valid");
let harness = CodingHarness::new_with_hooks_and_resume_tool_config(
    provider,
    config.defaults.model.clone(),
    config.clone(),
    workspace_root,
    hooks,
    user_system_prompt,
    initial_messages,
    resume_info,
    tool_config,
);
```

Remove the local `allow_mutating` variable from `run_interactive` if it becomes unused.

- [ ] **Step 8: Update runner tests to unwrap fallible constructors**

In `crates/opi-coding-agent/tests/non_interactive_policy.rs` and `crates/opi-coding-agent/tests/safety_hooks.rs`, replace direct runner construction:

```rust
let mut runner = NonInteractiveRunner::new(...);
```

with:

```rust
let mut runner = NonInteractiveRunner::new(...)
    .expect("runner should be constructed");
```

For `new_with_resume` calls, use the same `.expect("runner should be constructed")`.

- [ ] **Step 9: Update safety hook tests**

In `crates/opi-coding-agent/tests/safety_hooks.rs`, replace `interactive_denies_mutating_when_not_allowed` with:

```rust
#[tokio::test]
async fn interactive_hooks_do_not_deny_mutating_tools() {
    let hooks = InteractiveCodingHooks::new(false);
    for tool in &["read", "write", "edit", "bash", "glob", "grep"] {
        let result = hooks.before_tool_call(make_before_ctx(tool)).await;
        assert!(
            matches!(result, BeforeToolCallResult::Allow),
            "interactive hook should not deny tool '{tool}'"
        );
    }
}
```

Replace `non_interactive_denies_mutating_by_default` assertion text so it expects unknown tool feedback or follow-up text, not a hook denial:

```rust
assert_eq!(result.exit_code, 0, "Should succeed after unknown tool + follow-up");
assert!(result.stdout.contains("done"));
```

Replace `e2e_json_mode_tool_denial` with:

```rust
#[allow(clippy::await_holding_lock)]
#[tokio::test]
async fn e2e_json_mode_mutating_tool_is_not_available_by_default() {
    let _lock = session_lock();
    with_session_dir(|| async {
        let workspace = create_temp_workspace();
        let mock = MockProvider::new(
            "mock",
            vec![
                tool_call_response("tc-1", "bash", r#"{"command":"echo hi"}"#),
                text_response("done"),
            ],
        );

        let mut runner = NonInteractiveRunner::new(
            Box::new(mock),
            "mock:mock-model".into(),
            OpiConfig::default(),
            workspace.path().to_path_buf(),
            false,
            None,
            vec![],
        )
        .expect("runner");

        let result = runner.run_json("test prompt").await;
        assert!(
            result.stdout.contains("unknown tool: bash"),
            "JSON output should contain unavailable tool information: {}",
            result.stdout
        );
    })
    .await
}
```

Replace `session_audit_tool_denial` denial check with:

```rust
let has_unavailable_tool_result = entries.iter().any(|e| {
    let json = serde_json::to_string(e).unwrap_or_default();
    json.contains("unknown tool: write")
});
assert!(
    has_unavailable_tool_result,
    "Session entries should contain unavailable tool audit record"
);
```

- [ ] **Step 10: Run focused tests**

Run:

```sh
cargo test -p opi-coding-agent --test tool_selection
cargo test -p opi-coding-agent --test non_interactive_policy
cargo test -p opi-coding-agent --test safety_hooks
```

Expected: compile failures remain only for `PathPolicy` and `ReadTool::new_with_policy` until Task 3 is complete.

## Task 3: Implement Explicit Path Policy

**Files:**
- Modify: `crates/opi-coding-agent/src/tool/mod.rs`
- Modify: `crates/opi-coding-agent/src/tool/read.rs`
- Modify: `crates/opi-coding-agent/src/tool/write.rs`
- Modify: `crates/opi-coding-agent/src/tool/edit.rs`
- Test: `crates/opi-coding-agent/tests/tools_read_write_edit_bash.rs`

- [ ] **Step 1: Add path-policy tests**

Edit `crates/opi-coding-agent/tests/tools_read_write_edit_bash.rs`.

Update import:

```rust
use opi_coding_agent::tool::{BashTool, EditTool, PathPolicy, ReadTool, WriteTool};
```

Replace `read_tool_rejects_path_outside_workspace` with:

```rust
#[tokio::test]
async fn read_tool_workspace_policy_rejects_path_outside_workspace() {
    let dir = tempfile::tempdir().unwrap();
    let tool = ReadTool::new(dir.path().to_path_buf());
    let result = tool
        .execute(
            "escape-3",
            json!({ "path": "../etc/passwd" }),
            CancellationToken::new(),
            None,
        )
        .await
        .unwrap();

    assert!(result.is_error, "should reject path outside workspace");
    let text = tool_result_text(&result);
    assert!(
        text.contains("outside the workspace"),
        "error should mention workspace boundary, got: {text}"
    );
}
```

Add:

```rust
#[tokio::test]
async fn read_tool_allow_outside_policy_reads_absolute_path_outside_workspace() {
    let workspace = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let outside_file = outside.path().join("outside.txt");
    std::fs::write(&outside_file, "outside data").unwrap();

    let tool = ReadTool::new_with_policy(
        workspace.path().to_path_buf(),
        PathPolicy::AllowOutsideWorkspace,
    );
    let result = tool
        .execute(
            "outside-read",
            json!({ "path": outside_file.to_string_lossy() }),
            CancellationToken::new(),
            None,
        )
        .await
        .unwrap();

    assert!(
        !result.is_error,
        "unexpected error: {}",
        tool_result_text(&result)
    );
    assert!(tool_result_text(&result).contains("outside data"));
    let details = result.details.expect("details");
    assert_eq!(details.get("inside_workspace").and_then(|v| v.as_bool()), Some(false));
    assert!(details.get("resolved_path").is_some());
}

#[tokio::test]
async fn read_tool_workspace_policy_rejects_absolute_path_outside_workspace() {
    let workspace = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let outside_file = outside.path().join("outside.txt");
    std::fs::write(&outside_file, "outside data").unwrap();

    let tool = ReadTool::new(workspace.path().to_path_buf());
    let result = tool
        .execute(
            "outside-read-denied",
            json!({ "path": outside_file.to_string_lossy() }),
            CancellationToken::new(),
            None,
        )
        .await
        .unwrap();

    assert!(result.is_error);
    assert!(tool_result_text(&result).contains("outside the workspace"));
}
```

Update `read_tool_rejects_symlink_escape_via_new_subpath` name to `read_tool_workspace_policy_rejects_symlink_escape_via_new_subpath`. Keep the body using `ReadTool::new(...)`.

Update `read_tool_reports_workspace_boundary`, `write_tool_safety_context_in_details`, and `edit_tool_safety_context_in_details` to assert `inside_workspace` and `resolved_path`:

```rust
assert_eq!(details.get("inside_workspace").and_then(|v| v.as_bool()), Some(true));
assert!(details.get("resolved_path").is_some());
```

- [ ] **Step 2: Run the focused failing tool tests**

Run:

```sh
cargo test -p opi-coding-agent --test tools_read_write_edit_bash
```

Expected: compile failure because `PathPolicy` and `ReadTool::new_with_policy` do not exist.

- [ ] **Step 3: Add path resolver and policy**

Edit `crates/opi-coding-agent/src/tool/mod.rs`.

Replace `validate_workspace_path` with:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathPolicy {
    WorkspaceOnly,
    AllowOutsideWorkspace,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedToolPath {
    pub path: PathBuf,
    pub inside_workspace: bool,
}

pub fn resolve_tool_path(
    workspace_root: &Path,
    user_path: &str,
    policy: PathPolicy,
) -> Result<ResolvedToolPath, String> {
    let expanded = expand_user_path(user_path);
    let input = PathBuf::from(expanded);
    let resolved = if input.is_absolute() {
        input
    } else {
        workspace_root.join(input)
    };

    let canonical_root = std::fs::canonicalize(workspace_root)
        .map_err(|e| format!("cannot canonicalize workspace root: {e}"))?;
    let canonical = canonicalize_existing_or_nearest(&resolved)?;
    let inside_workspace = canonical.starts_with(&canonical_root);

    if policy == PathPolicy::WorkspaceOnly && !inside_workspace {
        return Err(format!(
            "path '{}' resolves outside the workspace",
            user_path
        ));
    }

    Ok(ResolvedToolPath {
        path: canonical,
        inside_workspace,
    })
}
```

Add helpers in the same file:

```rust
fn expand_user_path(user_path: &str) -> PathBuf {
    let path = user_path.strip_prefix('@').unwrap_or(user_path);
    if path == "~" {
        return home_dir().unwrap_or_else(|| PathBuf::from(path));
    }
    if let Some(rest) = path.strip_prefix("~/").or_else(|| path.strip_prefix("~\\")) {
        if let Some(home) = home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(path)
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

fn canonicalize_existing_or_nearest(path: &Path) -> Result<PathBuf, String> {
    if let Ok(canonical) = std::fs::canonicalize(path) {
        return Ok(canonical);
    }

    let mut ancestor = path;
    let mut suffix_components: Vec<std::ffi::OsString> = Vec::new();
    while let Some(parent) = ancestor.parent() {
        if let Some(name) = ancestor.file_name() {
            suffix_components.push(name.to_os_string());
        }
        if let Ok(canonical_ancestor) = std::fs::canonicalize(parent) {
            suffix_components.reverse();
            let suffix: PathBuf = suffix_components.iter().collect();
            return Ok(canonical_ancestor.join(suffix));
        }
        ancestor = parent;
    }

    Ok(normalize_path_components(path))
}
```

Keep `normalize_path_components` and add unit tests at the bottom of `tool/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_user_path_strips_at_prefix() {
        let path = expand_user_path("@Cargo.toml");
        assert_eq!(path, PathBuf::from("Cargo.toml"));
    }

    #[test]
    fn normalize_path_components_removes_parent_segments() {
        let path = normalize_path_components(Path::new("/tmp/a/../b"));
        assert!(path.ends_with(Path::new("tmp").join("b")));
    }
}
```

- [ ] **Step 4: Update `ReadTool`**

Edit `crates/opi-coding-agent/src/tool/read.rs`.

Change import:

```rust
use super::PathPolicy;
```

Change struct:

```rust
pub struct ReadTool {
    workspace_root: PathBuf,
    path_policy: PathPolicy,
    schema: serde_json::Value,
}
```

Change constructors:

```rust
impl ReadTool {
    pub fn new(workspace_root: PathBuf) -> Self {
        Self::new_with_policy(workspace_root, PathPolicy::WorkspaceOnly)
    }

    pub fn new_with_policy(workspace_root: PathBuf, path_policy: PathPolicy) -> Self {
        let schema = schemars::schema_for!(ReadArgs);
        Self {
            workspace_root,
            path_policy,
            schema: serde_json::to_value(&schema).unwrap_or_default(),
        }
    }
}
```

Change path resolution:

```rust
let resolved_path =
    match super::resolve_tool_path(&self.workspace_root, &args.path, self.path_policy) {
        Ok(p) => p,
        Err(msg) => {
            return Box::pin(async move {
                Ok(ToolResult {
                    content: vec![OutputContent::Text { text: msg }],
                    details: None,
                    is_error: true,
                    terminate: false,
                })
            });
        }
    };
let file_path = resolved_path.path;
let inside_workspace = resolved_path.inside_workspace;
```

Change details:

```rust
let details = serde_json::json!({
    "workspace_root": workspace_root.to_string_lossy(),
    "path": path_for_display,
    "resolved_path": file_path.to_string_lossy(),
    "inside_workspace": inside_workspace,
});
```

- [ ] **Step 5: Update `WriteTool` and `EditTool`**

In `crates/opi-coding-agent/src/tool/write.rs`, replace `validate_workspace_path` call with:

```rust
let resolved_path = match super::resolve_tool_path(
    &self.workspace_root,
    &args.path,
    super::PathPolicy::WorkspaceOnly,
) {
    Ok(p) => p,
    Err(msg) => {
        return Box::pin(async move {
            Ok(ToolResult {
                content: vec![OutputContent::Text { text: msg }],
                details: None,
                is_error: true,
                terminate: false,
            })
        });
    }
};
let file_path = resolved_path.path;
let inside_workspace = resolved_path.inside_workspace;
```

Update details:

```rust
let details = serde_json::json!({
    "workspace_root": workspace_root.to_string_lossy(),
    "path": path_for_display,
    "resolved_path": file_path.to_string_lossy(),
    "inside_workspace": inside_workspace,
});
```

Apply the same resolver pattern in `crates/opi-coding-agent/src/tool/edit.rs`. Preserve existing `before` and `after` fields:

```rust
let details = serde_json::json!({
    "workspace_root": workspace_root.to_string_lossy(),
    "path": path_for_display,
    "resolved_path": file_path.to_string_lossy(),
    "inside_workspace": inside_workspace,
    "before": before,
    "after": new_content,
});
```

- [ ] **Step 6: Run path tests**

Run:

```sh
cargo test -p opi-coding-agent --test tools_read_write_edit_bash
```

Expected: pass after fixing any compile errors from moved function names.

## Task 4: Finish Runner and Policy Regression Tests

**Files:**
- Modify: `crates/opi-coding-agent/tests/non_interactive_policy.rs`
- Modify: `crates/opi-coding-agent/tests/safety_hooks.rs`
- Modify: `crates/opi-coding-agent/tests/tool_selection.rs`

- [ ] **Step 1: Update non-interactive policy expectations**

In `crates/opi-coding-agent/tests/non_interactive_policy.rs`, update mutating default tests to assert that mutating tools are unavailable, not hook-denied:

```rust
assert!(
    result.stdout.contains("unknown tool: write")
        || result.stderr.contains("unknown tool: write")
        || result.stdout.contains("Write was denied"),
    "should indicate write was not available, stdout: {:?}, stderr: {:?}",
    result.stdout,
    result.stderr
);
```

Use `unknown tool: edit` and `unknown tool: bash` in the edit and bash tests.

Update `policy_readonly_tools_always_allowed` to cover all non-interactive default read-only tools through system prompt visibility if it is easier than making the mock provider call each tool:

```rust
let config =
    opi_coding_agent::policy::ToolRuntimeConfig::resolve(
        opi_coding_agent::policy::RunMode::NonInteractive,
        false,
        opi_coding_agent::policy::ToolSelection::Default,
    )
    .expect("tool config");
assert_eq!(
    config.active_tool_names,
    vec!["read", "grep", "find", "ls", "glob"]
);
```

- [ ] **Step 2: Add explicit invalid allowlist test**

In `crates/opi-coding-agent/tests/non_interactive_policy.rs`, add:

```rust
#[test]
fn non_interactive_tools_bash_without_allow_mutating_is_policy_error() {
    let error = opi_coding_agent::policy::ToolRuntimeConfig::resolve(
        opi_coding_agent::policy::RunMode::NonInteractive,
        false,
        opi_coding_agent::policy::ToolSelection::Allowlist(vec!["bash".into()]),
    )
    .expect_err("bash should require opt-in");

    assert!(
        error
            .to_string()
            .contains("mutating tool 'bash' requires --allow-mutating")
    );
}
```

- [ ] **Step 3: Run all affected tests**

Run:

```sh
cargo test -p opi-coding-agent --test tool_selection
cargo test -p opi-coding-agent --test non_interactive_policy
cargo test -p opi-coding-agent --test safety_hooks
cargo test -p opi-coding-agent --test tools_read_write_edit_bash
```

Expected: all four test binaries pass.

## Task 5: Update CLI Help and Documentation

**Files:**
- Modify: `crates/opi-coding-agent/src/cli.rs`
- Modify: `crates/opi-coding-agent/README.md`
- Modify: `crates/opi-coding-agent/README.zh.md`
- Modify: `docs/opi-spec.md`

- [ ] **Step 1: Update CLI help**

In `crates/opi-coding-agent/src/cli.rs`, change:

```rust
/// Allow mutating tools (write, edit, bash) in non-interactive mode.
#[arg(long)]
pub allow_mutating: bool,
```

to:

```rust
/// Allow mutating tools (write, edit, bash) in non-interactive mode.
#[arg(long)]
pub allow_mutating: bool,
```

If the existing text already matches this exact wording, leave the code unchanged. The important behavior change is that interactive mode does not require this flag.

- [ ] **Step 2: Update English README status and quick start**

In `crates/opi-coding-agent/README.md`, replace the status paragraph with:

```markdown
This crate produces the `opi` CLI and exposes the coding harness as a Rust library. It supports interactive TUI mode, positional-prompt non-interactive mode, NDJSON output, nine provider prefixes, eight available built-in tools, pi-aligned interactive default tools, conservative non-interactive default tools, image attachments, model/session pickers, shell completion generation, context file loading, session persistence, resume/list/delete session commands, context compaction, configurable keybindings/themes, per-provider proxy config, retry, token usage totals, and best-effort cost summaries.
```

Replace quick start mutating example:

```markdown
# Allow mutating tools in non-interactive automation
opi --allow-mutating "Update the README."
```

- [ ] **Step 3: Update English README CLI flag table**

In `crates/opi-coding-agent/README.md`, ensure the `--allow-mutating` row is:

```markdown
| `--allow-mutating` | Allow `write`, `edit`, and `bash` in non-interactive mode |
```

Keep `--tools`, `--no-tools`, and `--no-builtin-tools` rows, but update `--tools` to:

```markdown
| `--tools <TOOLS>` | Comma-separated active tool allowlist, for example `read,grep` |
```

- [ ] **Step 4: Update English README built-in tools section**

In `crates/opi-coding-agent/README.md`, replace the paragraph after the tools table with:

```markdown
Available built-in tools are `read`, `write`, `edit`, `bash`, `grep`, `find`, `ls`, and opi-native `glob`.

Default active tools depend on run mode:

- Interactive mode: `read`, `write`, `edit`, `bash`.
- Non-interactive mode: `read`, `grep`, `find`, `ls`, `glob`.
- Non-interactive mode with `--allow-mutating` or `defaults.allow_mutating_tools = true`: `read`, `write`, `edit`, `bash`.

Use `--tools <TOOLS>` to provide an explicit active tool allowlist. In non-interactive mode, allowlists containing `write`, `edit`, or `bash` require `--allow-mutating` or `defaults.allow_mutating_tools = true`.

Path policy is mode-aware. File writes and edits are restricted to the harness workspace root. Interactive `read` can resolve absolute paths and paths outside the workspace; non-interactive file tools remain workspace-only by default. File tool details include `workspace_root`, `resolved_path`, and `inside_workspace`.
```

- [ ] **Step 5: Update Chinese README**

`crates/opi-coding-agent/README.zh.md` currently renders as mojibake in PowerShell output. Preserve its existing encoding and make the same semantic changes:

```markdown
可用内置工具包括 `read`、`write`、`edit`、`bash`、`grep`、`find`、`ls`，以及 opi 原生的 `glob`。

默认启用工具取决于运行模式：

- 交互模式：`read`、`write`、`edit`、`bash`。
- 非交互模式：`read`、`grep`、`find`、`ls`、`glob`。
- 非交互模式带 `--allow-mutating` 或 `defaults.allow_mutating_tools = true`：`read`、`write`、`edit`、`bash`。

`--tools <TOOLS>` 用于显式指定启用工具 allowlist。非交互模式下，如果 allowlist 包含 `write`、`edit` 或 `bash`，必须同时设置 `--allow-mutating` 或 `defaults.allow_mutating_tools = true`。

路径策略按模式区分。写入和编辑默认限制在 harness workspace 根目录内。交互模式的 `read` 可以解析绝对路径和 workspace 外路径；非交互模式的文件工具默认保持 workspace-only。文件工具 details 会记录 `workspace_root`、`resolved_path` 和 `inside_workspace`。
```

After editing, run:

```sh
git diff -- crates/opi-coding-agent/README.zh.md
```

Expected: only the intended sections change; no whole-file re-encoding.

- [ ] **Step 6: Update `docs/opi-spec.md` document control**

In `docs/opi-spec.md`, update Document Control:

```markdown
| Last updated | 2026-06-01 |
| Current implementation | `opi` 0.3.0, Phase 3 complete |
| Next milestone | 0.4.0 Phase 4 extensibility |
```

Update Executive Summary sentence:

```markdown
The repository has completed Phase 3: the `opi` binary is usable, multi-provider streaming works, sessions persist as JSONL, compaction and JSON mode exist, image/context/provider hardening is present, and the TUI has daily-use basics. Phase 4 should add RPC, SDK, extension, skill, package, and web surfaces without turning MCP, sub-agents, plan mode, todos, or permission gates into core features.
```

- [ ] **Step 7: Update `docs/opi-spec.md` tool safety section**

In section 9 around the built-in tools and safety boundary, replace the current safety paragraphs with:

```markdown
Interactive mode SHOULD default to the pi coding tool set: `read`, `write`, `edit`, and `bash`. Non-interactive mode SHOULD default to a conservative read-only tool set: `read`, `grep`, `find`, `ls`, and opi-native `glob`. Non-interactive mutating tools require explicit opt-in through `--allow-mutating` or `defaults.allow_mutating_tools = true`.

Tool visibility and tool execution policy MUST agree. Opi should not advertise `write`, `edit`, or `bash` to the model in non-interactive mode unless those tools can execute under the resolved policy.

File tools MUST use explicit path policy. `write` and `edit` remain workspace-only by default. Interactive `read` MAY resolve absolute paths and workspace-external paths for pi-style usability. Non-interactive file tools remain workspace-only by default.

Interactive confirmation MAY exist in Phase 4+ as an extension-mediated safeguard, but reusable permission profiles and permission popups are not core parity with pi; richer gates should be built via tool allowlists, hooks, extensions, packages, containers, or external wrappers.
```

- [ ] **Step 8: Run docs-oriented checks**

Run:

```sh
rg -n "Phase 2 complete|Next milestone.*Phase 3|All file paths are validated|Mutating tools are denied unless" docs/opi-spec.md crates/opi-coding-agent/README.md crates/opi-coding-agent/README.zh.md
```

Expected: no stale matches that contradict the new behavior.

## Task 6: Full Verification

**Files:**
- No additional edits expected.

- [ ] **Step 1: Format**

Run:

```sh
cargo fmt --all
```

Expected: command succeeds.

- [ ] **Step 2: Run focused crate tests**

Run:

```sh
cargo test -p opi-coding-agent --all-targets
```

Expected: all `opi-coding-agent` tests pass.

- [ ] **Step 3: Run workspace clippy**

Run:

```sh
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: no warnings.

- [ ] **Step 4: Check working tree**

Run:

```sh
git status --short
```

Expected: modified files are limited to the implementation files, affected tests, README files, `docs/opi-spec.md`, the design spec, and this plan.

## Self-Review Checklist

- Spec coverage: tool defaults, non-interactive mutating opt-in, path policy, CLI docs, README docs, and opi-spec drift are each covered by tasks.
- Test coverage: policy unit tests, harness prompt visibility tests, runner policy tests, hook behavior tests, and path boundary tests are included.
- Red-flag scan: this plan avoids unresolved markers and avoids instructing agents to commit.
- Type consistency: `RunMode`, `ToolRuntimeConfig`, `ToolPolicyError`, `PathPolicy`, and `ResolvedToolPath` are introduced before downstream tasks consume them.
