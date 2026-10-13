---
name: stack-stat-ship
description: Use when asked to ship stack-stat, open a pull request, or update an existing pull request. Verify, refresh docs, and complete PR evidence.
---

# Ship a change

1. Run `make preflight` and `cargo doc --workspace --no-deps --locked`. Stop to fix failures. For runtime changes, launch the actual dashboard in a repo with an active stack, exercise the changed path, and report observed behavior; otherwise explain N/A.
2. Invoke `stack-stat-docs-refresh` to inspect living docs against the final code. Commit needed changes before preparing PR text.
3. Invoke `stack-stat-reflow` to consolidate fixup commits. Run `bash scripts/commit-lint.sh` on the final history. Verify the reflow did not change the tree.
4. Detect an existing PR using `gh pr view --json number,url,title` on the current branch. Update it instead of opening a duplicate. For a new change, open a draft PR.
5. Fill every item in [the PR template](../../../.github/PULL_REQUEST_TEMPLATE.md) with `[x]` and an observed command result or explicit `N/A because …`. Never leave placeholders or unchecked items. Link architectural changes to [architecture](../../../docs/architecture/) and decisions to [ADRs](../../../docs/adr/README.md).
6. Push, inspect the resulting PR, and report its URL and outstanding checks. Do not claim CI passed until its checks finish successfully.

PR titles are short human-readable summaries. Never infer evidence from a pre-reflow commit; run or verify against the final tree. This skill is safe to repeat on the same branch.
