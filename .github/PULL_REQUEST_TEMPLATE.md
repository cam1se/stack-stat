## Summary

<!-- Explain the change and why it matters. -->

## Testing

<!-- Check each item with observed evidence or N/A because ... -->
- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --workspace --all-targets --locked -- -D warnings`
- [ ] `cargo test --workspace --locked --no-fail-fast`
- [ ] TUI smoke via `make demo` / N/A because runtime behavior is unchanged

## Docs and architecture

- [ ] Living architecture docs updated / N/A because structure is unchanged
- [ ] ADR written for a significant decision / N/A because no decision warrants one
- [ ] `cargo doc --workspace --no-deps --locked`; public items documented

## Notes for reviewers

<!-- Known limitations and deferred verification. -->
