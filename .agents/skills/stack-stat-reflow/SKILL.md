---
name: stack-stat-reflow
description: Use to tidy, squash, or reword commits on a stack-stat feature branch before a PR is opened or shipped.
---

# Reflow feature commits

1. Fetch `origin/main`; inspect `git log --oneline origin/main..HEAD`. Stop if the range is empty.
2. Create a backup branch at HEAD before changing history; print its name.
3. Group WIP/fixup commits with the concern they repair, retaining independent concerns. Aim for a small set of durable Conventional Commits (typically one to five), with subjects readable as changelog entries.
4. Read `.gitmessage` for valid types and scopes. Rebase interactively onto `origin/main`, squashing and rewording as needed. Never rewrite main.
5. Run `bash scripts/commit-lint.sh`. Verify `git diff <backup> HEAD` is empty so the reflow preserves the tree.
6. Push with `git push --force-with-lease origin <branch>`, never plain `--force`. Only then delete the backup branch.

If rebase conflicts or verification fails, keep the backup and stop with the actionable state; never discard user work. PR creation belongs to `stack-stat-ship`.
