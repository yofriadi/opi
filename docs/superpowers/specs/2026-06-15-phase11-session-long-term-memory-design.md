# Phase 11 Session Tree and Context Reconstruction Design

## Overview

Phase 11 deepens opi's session system and long-running workflow support. The
scope is session-native context: branch trees, labels, names, summaries, model
and thinking changes, exports, recovery, and deterministic context
reconstruction. It is not a global memory system, vector database, RAG layer,
or user-profile store.

The goal is to make opi better at preserving and navigating development work
over time while staying faithful to pi's terminal-first, append-only session
model.

## Goals

- Introduce an opi session v2 design where needed, preserving safe migration
  from existing opi v1 sessions.
- Add richer session entries for model changes, thinking level changes,
  session info, labels, and branch summaries when they are product-supported.
- Improve tree reconstruction, branch navigation metadata, and context
  building.
- Add export formats for local review and handoff.
- Make compaction and branch summaries more explicit and auditable.
- Keep persisted context bounded to session files and explicit exports.

## Non-Goals

- No vector database.
- No semantic memory service.
- No global user profile memory.
- No automatic cross-project memory injection.
- No pi session v3 read/write compatibility.
- No cloud sync.
- No session sharing service.
- No web UI product.
- No package ecosystem expansion.

## Relationship to pi

Pi session v3 includes tree entries for messages, model changes, thinking level
changes, compaction, branch summaries, extension custom entries, custom
messages, labels, and session info. Opi should learn from that shape but should
not promise pi file compatibility.

Phase 11 should define opi's own session v2 only if new entries cannot be added
cleanly to the existing v1 format. Compatibility means opi can load its own
older sessions and explain migration, not that opi can read arbitrary pi
sessions.

The current opi session format already records messages, compaction entries,
leaf pointers, and extension state. It does not yet make pi-inspired branch
summaries, extension custom messages, labels, or session info first-class in
context reconstruction. Phase 11 closes that semantic gap without claiming pi
session v3 file compatibility.

## Session-Native Context Boundary

Allowed:

- session names;
- labels/bookmarks;
- branch summaries;
- compaction summaries;
- explicit export;
- model/thinking history;
- extension state that is scoped to a session;
- local metadata needed to reconstruct context.

Not allowed:

- automatic retrieval from unrelated sessions;
- global facts about the user;
- background embedding/indexing;
- hidden prompt injection from old sessions;
- remote context sync.

## Session Entry Model

Define or explicitly defer entries for:

| Entry | Purpose |
|---|---|
| `model_change` | Record provider/model change on the active branch |
| `thinking_level_change` | Record reasoning/thinking level change |
| `session_info` | Store user-visible name and optional metadata |
| `label` | Bookmark or label an entry |
| `branch_summary` | Preserve context when leaving or forking a branch |
| `custom_message` | Extension-injected LLM-context message with display semantics |

Existing entries for messages, compaction, leaf pointers, and extension state
remain. Any new entry must have clear context-building semantics and tests.
Entries that participate in LLM context, especially `branch_summary` and
`custom_message`, must have provider-conversion tests or be explicitly deferred
with a product reason.

## Context Building

Context reconstruction should be deterministic:

```text
session file
  -> recover valid entries
  -> find active leaf
  -> walk branch to root
  -> apply model/thinking/session metadata
  -> apply compaction and branch summaries
  -> produce app messages for agent runtime
```

Rules should define:

- how multiple `leaf` entries resolve;
- how corrupt trailing lines are handled;
- how labels affect UI but not LLM context;
- how branch summaries enter LLM context;
- how custom messages enter LLM context and transcript rendering;
- how model/thinking changes affect resumed runs;
- how extension state is restored.

## Branch Summaries

Branch summaries should be explicit and optional. They may be generated:

- when switching branches;
- when forking or cloning;
- manually by command;
- by an extension hook if the runtime supports it.

Phase 11 should not require live provider calls for every branch operation.
If a summary cannot be generated, the branch action should still work and
record a diagnostic.

## Export

Add local export support for:

| Format | Purpose |
|---|---|
| markdown | readable review and handoff |
| json | structured local tooling |
| html | optional static transcript if low-cost and aligned with existing rendering |

Exports are local files. No sharing service is part of Phase 11.

Export should support:

- active branch only;
- full tree;
- include/exclude tool output;
- include/exclude thinking content;
- redaction options using Phase 7 rules.

## Commands and UI Surface

Candidate commands:

```text
/name <name>
/label <label>
/unlabel
/export [path]
/session
```

CLI candidates:

```text
opi --list-sessions --json
opi --export-session <id-or-path> --format markdown --output <file>
```

Only implement commands that have clear tests and do not require a broad TUI
redesign. Phase 12 can polish interactive presentation.

## Data Flow

```text
interactive/session command
  -> SessionCoordinator
  -> append session entry
  -> update active leaf where applicable
  -> emit AgentSessionEvent / diagnostics
  -> TUI or JSON/RPC presentation
```

```text
export command
  -> SessionReader
  -> branch/tree selection
  -> redaction policy
  -> renderer
  -> local file
```

## Error Handling

Session operations should prefer recoverability:

- corrupt final lines are recovered when possible;
- unknown future entries are preserved or skipped according to documented
  compatibility rules;
- failed metadata writes produce errors before claiming success;
- failed branch summary generation records diagnostics but should not destroy
  branch navigation;
- export failures must not modify the session.

## Testing Strategy

| Level | Coverage |
|---|---|
| session storage | new entry round trips, migration, unknown entry behavior |
| context building | model/thinking changes, compaction, branch summaries |
| branch tree | labels, session names, active leaf resolution |
| CLI | list/export/session metadata commands |
| TUI snapshot | branch picker metadata if touched |
| redaction | export redaction for prompts, tool output, secrets |

All tests must use isolated temp session directories or `OPI_SESSIONS_DIR`.

## Documentation Updates

Update docs to state:

- opi session format version and compatibility policy;
- opi does not promise pi session v3 compatibility;
- session files are sensitive;
- session-native context is explicit and bounded to session files/exports;
- export is local and user-controlled.

## Success Criteria

Phase 11 is complete when:

1. Session metadata and context-entry needs are either implemented or
   explicitly deferred.
2. New session entries, if added, round-trip and rebuild context
   deterministically.
3. Existing opi sessions continue to load.
4. Branch, label, name, model, thinking, compaction, and summary semantics are
   documented where implemented.
5. `branch_summary` and `custom_message` are either implemented with
   provider/context semantics or explicitly deferred with reasons.
6. Local export supports at least markdown or JSON with redaction options.
7. Session files are documented as sensitive.
8. No vector memory, global profile, cloud sync, session sharing service, or pi
   session compatibility claim is added.

## Phase 12 Handoff

Phase 12 should improve the terminal presentation of the session model:
pickers, tree views, command palette, keyboard flow, and transcript rendering.
It should not change session semantics without updating this design.
