# Failure Decision Gate Reference

When `iteration_count` reaches `max_iterations` (default 5), STOP and hand
the decision to the user via `AskUserQuestion`. No self-deliberation past this.

## Gate Payload

Print this information:

```text
Task: <id> <title>
DoD: <definition_of_done>
Tier: <tier>
Iterations: <iteration_count> / <max_iterations>
Last gate output (truncated to 50 lines): <…>
Tests added but failing: <list>
Files modified: <list>
Smallest failing assertion: <quote from test output>
Start commit: <tasks[].start_commit>
Dirty status: <git status --short>
Reproduction commands: <exact commands>
```

## Options

| Option | Effect |
|---|---|
| (a) Retry with extended cap | +5 attempts (total 10). Status stays `in_progress`. |
| (b) Escalate to design | Invoke `superpowers:brainstorming` on DoD interpretation. User may amend spec + `--reinit`. |
| (c) Mark blocked | Record blocker text. Leave failing tests. Stage nothing. Status → `blocked`. Skipped on auto until `--clear-blocker`. |
| (d) Drop to manual | Print reproduction commands, touched files, suggested cleanup. Do NOT run cleanup. User finishes manually, then `--resume-from-manual`. |

**No "auto-revert" option.** MUST NOT run `git restore`, `git clean`,
`git reset`, or equivalent. If cleanup needed, print candidate commands
scoped to files changed since `start_commit` and exit.

## Meta-Warning

If **three consecutive** task invocations hit the failure gate, print:

> "Harness components may be misaligned with the current spec or model.
> Consider re-reading opi-spec.md §15 exit criteria, or grilling the design
> via `superpowers:brainstorming` before continuing."
