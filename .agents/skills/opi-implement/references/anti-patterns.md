# Anti-Pattern Guards Reference

These are explicit rules. Each maps to a documented failure mode. The **Why**
column explains reasoning so you can apply judgment in edge cases.

| Rule | Why |
|---|---|
| Never delete or weaken tests to make them pass | A passing suite that doesn't catch regressions creates false confidence. Fix the implementation, not the test. |
| Never `git push --force` | Rewrites shared history. Others may have fetched old refs; causes silent data loss and broken bisects. |
| Never bypass clippy with crate-wide `#[allow]` | Suppresses future warnings too. Targeted `#[allow]` on specific item with comment is OK; blanket suppression hides real issues. |
| Never commit with broken smoke | Smoke is cheapest proof prior work holds. Broken baseline means next invocation can't distinguish old from new breakage. |
| Never commit unstaged secrets | Secrets in git history are effectively public. Rotation cost far exceeds checking cost. |
| Never bypass git hooks (`--no-verify`) | Hooks encode project invariants. Bypassing means commit may fail CI later. |
| Never `git reset --hard` + force push for rollback | Destroys history for all collaborators. Use `git revert` instead. |
| Never `--amend` on already-pushed commits | Rewrites public SHA. Anyone who fetched original now has diverged history. |
| Never self-grade verification | LLMs rationalize success. Mechanical gates (exit codes, grep) are deterministic and auditable. |
| Never auto-accept TUI snapshot changes | Snapshot diffs are visual regressions until proven otherwise. Only human can judge intent. |
| Never silently rewrite inferred task graph metadata | Graph is a reviewed contract. Silent changes reorder execution, skip gates, break confirmed assumptions. |
| Never run live provider tests from this skill | Non-deterministic, costs money, hits rate limits. Belong in `#[ignore]`-gated tests run manually. |
| Never commit ledger/tmp/draft files | High-churn runtime artifacts. Pollutes history, creates merge conflicts. |
| Never skip `[workspace.dependencies]` for internal deps | Lockstep versioning requires workspace table. Bare path deps break `cargo publish`. |
| Never satisfy DoD with placeholder stubs/TODOs | Stubs pass gates but don't deliver value. Poisons downstream tasks depending on real behavior. |
| Never broaden into cross-task refactors without graph update | Scope creep invalidates adjacent task assumptions. Graph must reflect reality. |
| Never clean/restore/discard user changes from failure gate | Working tree may contain in-progress manual fixes. Automated cleanup destroys expensive context. |
| Never let sub-agent completion order decide result order | Non-deterministic ordering = unreproducible results. `parallelize` array defines canonical order. |

The skill refuses to act if any rule would be violated, even if the user
requests it during an interactive failure-decision gate.
