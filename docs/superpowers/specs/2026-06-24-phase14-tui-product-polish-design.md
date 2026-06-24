# Phase 14 TUI Product Polish Design

Historical note: this design was originally drafted as Phase 12. After the
`.repo/pi-0.80.2` baseline review, Phase 12 became provider correctness and
Phase 13 became session tree/context reconstruction. TUI product polish is now
Phase 14.

## Overview

Phase 14 polishes the terminal-first product surface after the runtime, tools,
providers, and sessions have been made more reliable. It focuses on interaction
quality in the existing ratatui/crossterm TUI: session tree navigation,
command discovery, model and session pickers, markdown/diff/image rendering,
keyboard flow, status visibility, and accessibility.

This phase does not turn `opi-web-ui` into a browser app and does not add a
custom TUI component protocol for packages. The primary product remains the
terminal coding agent. Extension UI/RPC UI sub-protocols and custom renderers
remain future ecosystem candidates, not Phase 14 scope.

## Goals

- Improve session and branch navigation using Phase 13 metadata.
- Add or refine a command palette for built-in slash commands, package
  commands, session commands, and extension commands where already supported.
- Polish model/session pickers with search, filtering, metadata, and empty
  states.
- Improve transcript rendering for markdown, code blocks, diffs, tool calls,
  thinking blocks, images, diagnostics, and summaries.
- Improve keyboard-only workflows and discoverability.
- Strengthen accessibility: CJK width, no-color behavior, contrast, focus,
  and non-color status cues.
- Keep TUI features scoped to coding-agent workflows.

## Non-Goals

- No standalone browser app.
- No pi-web-ui parity.
- No custom TUI component protocol for package adapters.
- No extension overlay/widget system in core.
- No games or novelty UI features.
- No broad terminal framework rewrite.
- No mouse-first workflows.
- No package ecosystem expansion.
- No permission popup subsystem.

## Relationship to pi

Pi's terminal UI is rich and extension-capable. Opi should match the terminal
coding workflow quality that users depend on, but it should not copy pi's
TypeScript renderer or custom extension UI model. Ratatui/crossterm remain the
correct Rust-native base.

Workflow-heavy UI, custom overlays, and extension-rendered widgets should remain
future design topics. Phase 14 focuses on built-in product polish while keeping
a clear future path for RPC/dialog/fire-and-forget extension UI surfaces.

## Product Surfaces

### Session and Branch UI

Use Phase 13 metadata to improve:

- tree picker labels;
- branch summaries;
- session names;
- labels/bookmarks;
- model/thinking metadata display;
- active branch indication;
- empty and corrupt session states;
- fork/clone/tree command feedback.

The TUI should make branch navigation safer without adding hidden automation.

### Command Palette

Add or refine a command palette that can show:

- built-in slash commands;
- session commands;
- model/thinking commands;
- package commands that are already available;
- extension commands already registered through existing runtime surfaces.

The palette should not imply unsupported commands such as npm install, package
update, or web-ui features.

### Pickers

Improve:

- model picker search and capability display;
- session picker filtering and metadata;
- branch picker metadata and keyboard flow;
- package/resource picker only if an existing command needs it.

Picker behavior should remain deterministic and snapshot-testable.

### Transcript Rendering

Polish display for:

- assistant text;
- thinking blocks;
- tool calls and results;
- errors and diagnostics;
- markdown tables and code fences;
- diffs;
- terminal images;
- compaction and branch summaries;
- session metadata changes.

Rendering should avoid layout shifts and overlapping text across common terminal
sizes.

### Status and Feedback

Improve status visibility for:

- current model;
- thinking level;
- active tools;
- running tool;
- queued steering/follow-up messages;
- package/adapter degraded state;
- trace/diagnostic availability;
- session name and branch status.

Status should not rely only on color.

## Accessibility and Terminal Compatibility

Requirements:

- respect `NO_COLOR`;
- preserve keyboard-only operation;
- handle CJK display width correctly;
- handle narrow terminals gracefully;
- avoid negative spacing and text overlap;
- provide non-color indicators for warning/error/success;
- keep screen updates predictable on Windows Terminal, tmux, and common Unix
  terminals where feasible.

## Data Flow

```text
runtime/session state
  -> TUI view model
  -> picker/transcript/status renderers
  -> snapshot-tested terminal output
```

```text
keyboard input
  -> keybinding resolution
  -> command palette or editor action
  -> runtime command
  -> user-visible status/event feedback
```

## Error Handling

TUI errors should degrade gracefully:

- image rendering falls back to text metadata;
- invalid themes fall back to default theme and show diagnostics;
- missing session metadata shows stable fallback text;
- too-narrow terminal sizes show compact layouts;
- command failures show structured diagnostics from Phase 7.

The TUI should not panic on malformed session entries, unsupported terminal
features, invalid Unicode width cases, or missing image dimensions.

## Testing Strategy

| Level | Coverage |
|---|---|
| snapshot | branch picker, session picker, command palette, transcript, status bar |
| unit | text wrapping, truncation, CJK width, keybinding resolution |
| integration | command palette dispatch to existing commands |
| terminal smoke | image fallback, no-color mode, narrow terminal layouts |
| docs guard | docs do not claim custom extension UI protocol or web-ui parity |

Before completion, affected snapshot tests should be updated intentionally and
reviewed. Do not rebaseline unrelated snapshots.

## Documentation Updates

Update docs for:

- keyboard shortcuts;
- command discovery;
- session tree navigation;
- model/session picker behavior;
- terminal image fallback;
- no-color and accessibility behavior;
- limits of package/extension UI support.

If English user docs change, update localized counterparts in the same change.

## Success Criteria

Phase 14 is complete when:

1. Session and branch UI reflects Phase 13 metadata.
2. Command discovery covers built-in and already-registered extension commands
   without advertising unsupported ecosystem features.
3. Model/session/branch pickers handle search, empty states, CJK labels, and
   narrow terminals.
4. Transcript rendering for markdown, diffs, images, tool calls, diagnostics,
   thinking, and summaries has focused snapshot coverage.
5. Status UI exposes model, thinking, tools, queue state, and degraded adapter
   state without relying only on color.
6. `NO_COLOR`, CJK width, keyboard-only operation, and terminal fallback
   behavior are documented and tested where practical.
7. No web-ui parity, custom TUI adapter protocol, permission popup subsystem,
   or package ecosystem expansion is added.

## Ecosystem Handoff

After Phase 14, opi should have a deeper core product: observable runtime,
stable agent contracts, higher-quality tools, correct existing providers,
session-native context reconstruction, and polished terminal UX. Only then
should later phases consider ecosystem expansion such as package
enable/disable/update, npm or registry packages, package gallery, provider
OAuth, additional provider breadth, extension UI/RPC UI sub-protocols, custom
message renderers, adapter-mediated UI surfaces, or browser web-ui
productization.
