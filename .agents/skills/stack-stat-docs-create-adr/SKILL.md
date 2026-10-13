---
name: stack-stat-docs-create-adr
description: Use when recording an architectural decision with significant alternatives or high reversal cost, or when asked for an ADR.
---

# Record an architecture decision

Read [ADR conventions and template](../../../docs/adr/README.md) and [docs conventions](../../../docs/README.md). Record a single decision, not an implementation diary.

1. Find the highest existing numbered ADR in `docs/adr/`; the new number is the next zero-padded four-digit ID. Never renumber.
2. Gather context, the exact choice, alternatives genuinely considered, consequences, deciders, and affected architecture pages. Resolve unclear decisions before writing.
3. Write `docs/adr/NNNN-short-slug.md` with Status (Proposed initially), Date, Deciders, Context, Decision, Consequences, Alternatives considered, and Related documentation.
4. Add the ADR to the index in `docs/adr/README.md` and link both ways with the affected architecture page. Use real Markdown links.
5. Accept only when the decision is confirmed or already shipped. Accepted records are immutable; a new ADR supersedes them. Verify links and commit the decision with its architecture update.
