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
