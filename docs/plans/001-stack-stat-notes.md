# Notes

<!-- Append-only. Group entries under ### Task N: [name] headings as you go. -->

### Task 1: Workspace skeleton, contributor conventions, and living-docs scaffold

- The workflow engine seeded and pushed `main` with plan/spec/notes before execution; this is not the first commit. Bootstrap remains on `main` as planned; later work belongs on the feature branch.
- `skill://commit` is not registered in this session. Follow the repository's `.gitmessage` directly until the local commit skill exists.

### Task 2: AI agent skills

N/A

### Task 3: CI, release automation, install scripts, and Pages site scaffold

- `Cross.toml` was not carried over: its only settings installed libudev, which this project does not use.
- The MkDocs nav lists future user pages, so short accurate introductory pages were created now to keep strict site builds green; Task 14 will expand them.
- Preview port 9000 was already occupied. `serve-docs.sh` now selects the first free port in 9000–9099 (or honors `PORT`); the site built and served at 9017.

### Task 4: Domain model and configuration

- Used a small allocation-free `*`/`?` matcher instead of a glob dependency; configured patterns are only evaluated while composing a snapshot.
- `LocalCheckResult.recorded_at` is optional: a configured check in `Missing` state was never reported and has no valid timestamp. Check summaries moved into `model/summary.rs` after the model file passed the size signal.

### Task 5: Provider traits, wire types, and fakes

- GraphQL and stack fixtures are hand-authored from the documented response shapes; the actual `gh stack view --json` payload must be checked against a live stack in Task 7. No active stack exists in this repo yet.
- `types.rs` was split so the aliased GraphQL response and normalization live in `providers/types/graphql.rs` rather than a monolithic wire file.

### Task 6: Git CLI provider and file status store

- macOS resolves temporary worktrees under `/private/var` even when the test created them under `/var`. The test compares canonical paths; provider discovery keeps Git's returned worktree path and canonicalizes only the common dir.
- The file store exposes `read_all_with_warnings()` in addition to the trait's `read_all()` so the polling layer can surface corrupt/unreadable files without discarding valid entries.

### Task 7: GitHub CLI provider

- A throwaway local stack initialized with `gh stack init --base main test-layer` produced `gh stack view --json` without `head` or `pr` on the unsubmitted branch. The wire type now accepts absent `head`; a regression fixture records this observed shape. The live provider returned `stack=main, layers=1` and fetched PR #1 with five checks using the aliased GraphQL query. No remote stack was created or pushed.
- `resolve_repo` is asynchronous rather than the plan's synchronous signature so its `gh repo view` read obeys the same 20-second timeout and process cleanup as all other `gh` reads.
