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

### Task 8: Compositor

- `BranchState.pr_number` preserves a PR number from stack metadata even if a GraphQL alias is inaccessible or missing. Detailed PR fields stay absent rather than falsely labeling that row Ready.
- `LocalCheckResult.recorded_at` remains absent for never-reported configured checks. The compositor does not infer a timestamp from the poll time.

### Task 9: CLI entry point, startup preflight, and `report` subcommand

- `CommandRunner::run` returns `io::Result<CommandOutput>` instead of a bare `Output` so a missing `gh` executable can be represented without panicking. The startup CLI still prints the spec's actionable error and exits 1.
- The report command's underlying Git adapter is concrete, as planned, because `resolve_rev` is not part of the read-only `GitProvider` trait.

### Task 10: App state machine, effects, pollers, and test harness

- The fake store and file store now share a `read_with_warnings()` trait method with a default implementation so polling can show malformed-file diagnostics without dropping valid data.
- A throwaway local stack (not pushed) launched the actual alternate-screen TUI; the minimal frame showed `test-layer`, and `q` exited with code 0 and restored the terminal. Full rendering follows in Task 11.
- The file writer unexpectedly marked `tests/support/mod.rs` executable; its mode was reset to 0644 and the tool behavior was reported.
- Tokio watch sends a change notification even for an identical value; the App handles frequent Git updates. The GitHub activity watch now publishes only transitions, avoiding accidental remote polling on every local Git tick.

### Task 11: Rendering: header, table, detail pane, status bar, help, theme

- The user supplied a real, read-only stacked worktree. Its status directory did not exist before or after the smoke; the dashboard showed stack layers, PR states, checks, reviews, and the moving selection without mutating that repo. `q` exited 0. One selected PR had dozens of checks; the approved no-scroll detail behavior truncates before later review/local sections. This limits inspection of lower sections for large PRs, despite the summary review column remaining visible.
- A narrow 80-column rendering test exposed the footer dropping its `q quit` hint because verbose hints filled the line. Compact hints now preserve both quit and active status text at narrow widths.
- The `proc://` PTY input helper appends an Enter to nonempty writes. Sending `?` opened help then immediately closed it; a key-event trace confirmed both inputs. The real keymap and `TestBackend` help overlay behaved correctly, and the tool issue was reported. Temporary tracing/probe files were removed.
- After the real-stack smoke exposed unreachable review/local detail on PRs with many checks, the user explicitly requested scrolling and rejected a focus-navigation mode. This supersedes the plan's truncation-only rule: `J/K` and Page Up/Down scroll details; wheel events route by hovered pane and table clicks select rows. Compact help uses two columns rather than clipping bindings.
- `tachytronix` was offered as an optional animation dependency; evaluate it during Task 12 against the fixed six-effect contract, with a simpler internal tick renderer as the default unless it removes real complexity.

### Task 12: Animations

- The user clarified the optional crate was [TachyonFX](https://github.com/ratatui/tachyonfx), a stateful Ratatui buffer-effects engine (not tachyonix, the channel crate). The six required effects are fixed, cell-specific, and tick-deterministic; the in-repo animation state avoids a second scheduler and extra dependency. TachyonFX remains suitable if future designs need composable wipes/fades.
- A read-only run against the user-supplied stack showed actual in-progress CI spinners and the header fetch indicator; no local status directory existed to be modified. The active branch pulse is also covered by a deterministic `TestBackend` 16-color modifier test.
- No in-progress check finished during the short read-only real-repo smoke. A throwaway rendered-buffer probe instead transitioned a controlled check from `IN_PROGRESS` to `SUCCESS` and observed `completed-check cell flashed: true`, then `flash ended after five ticks: true`; the probe was removed. This verifies the completion surface without altering CI or the user-supplied repository.

### Task 13: Self-update and startup version check

- GitHub Releases for `tcamise-gpsw/stack-stat` is empty. `cargo run -- update` exited 1 with `no stable release found in tcamise-gpsw/stack-stat`; end-to-end replacement cannot be exercised until the first release exists. Archive checksum/extraction and version selection are covered with generated test archives on disk.
- The TUI draws its first frame before starting the one-shot release query. A read-only local-stack smoke showed the dashboard and quit successfully while the background check found no release.

### Task 14: User documentation, README, and dogfood config

- The MkDocs nav and documentation map already included all planned pages from earlier tasks, so no change to `mkdocs.yml` or `docs/README.md` was needed. The goal framing in `docs/goals.md` also remained accurate.
- No stable release exists yet; installation and real binary replacement cannot be exercised. The user docs explicitly condition installer use on a published release and document the observed `no stable release found` update error.

### Task 15: Final Validation

- Initial Windows CI failed `concurrent_reader_never_observes_partial_write`: `NamedTempFile::persist` returned Access is denied (os error 5) while another thread read the existing JSON, and a reader briefly saw zero entries. `tempfile` uses `MoveFileExW(..., MOVEFILE_REPLACE_EXISTING)` on Windows; replacement can be blocked while the destination is open. The store now coordinates shared readers and exclusive writes/GC across processes using a separate std `File` lock, with branch directory creation inside the write lock. The existing concurrent-reader regression test remains unchanged; CI rerun will verify Windows.
- Scenario 10's validation uncovered another defect: a successful local poll cleared a still-active GitHub network warning. The App now tracks errors by source; the regression test reports a local check during a GitHub failure and asserts the remote warning stays visible.
- The supplied real stacked repository was used strictly read-only for PR/check/review columns, detail, filter, sort, bot toggle, and clean exit. Branch checkout and `report` were exercised only in an owned throwaway two-layer stack with a separate reporter worktree; the `←wt` marker moved after an external checkout, and a report written from the second worktree appeared in the first TUI. No fixture branches were retained in this repository: the permanent fake-backed `TestScenario` builder and throwaway real Git stack cover repeatable paths without cluttering tracked history.

| E2E scenario | Test evidence |
|---|---|
| 1 — all-green stack | `render::four_layer_healthy_stack_has_passing_status_without_staleness` |
| 2 — mixed CI | `render::branch_table_shows_pr_lifecycle_ci_reviews_and_local_results` |
| 3 — agent branch switch | `app_state::agent_branch_switch_moves_current_row_after_git_poll`; `render::branch_switch_and_stale_check_have_distinct_visual_markers` |
| 4 — draft/ready toggle | `app_state::draft_toggle_requires_confirmation_and_records_effect` |
| 5 — stale local checks | `render::branch_switch_and_stale_check_have_distinct_visual_markers`; `compose::previous_commit_result_is_stale_and_declared_unreported_check_is_missing` |
| 6 — detail pane | `render::details_toggle_exposes_checks_reviews_local_results_and_rebase`; `render::keyboard_scroll_reaches_reviews_after_many_checks_without_changing_selected_branch` |
| 7 — CI filtering | `render::skipped_ci_check_is_visible_in_details_not_row_total` |
| 8 — bot visibility | `render::bot_review_visibility_toggles_in_detail_pane` |
| 9 — no stack | `app_state::off_stack_replaces_rows_but_network_failure_retains_last_good_rows`; `render::loading_not_in_stack_help_and_confirmation_render_without_panics` |
| 10 — fetch failure | `app_state::off_stack_replaces_rows_but_network_failure_retains_last_good_rows` (includes local store update while remote fails) |
| 11 — responsive layout | `render::narrow_widths_collapse_review_and_local_columns` |
| 12 — keybindings | `keymap::every_normal_mode_binding_has_its_documented_effect` |
| 13 — change flash | `animation::completed_check_flashes_only_its_cell_for_five_ticks` |
| 14 — report CLI | `report::report_writes_lists_and_clears_real_git_common_store`; `report::branch_override_and_revision_resolve_to_full_sha_and_stale_entry` |
