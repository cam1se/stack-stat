# Architecture Decision Records

Record a decision when it affects structure and is hard to reverse, or when viable alternatives are worth remembering. Reversible choices belong in [architecture](../architecture/01-l1-context.md).

Number ADRs chronologically with zero-padded permanent IDs (`0001-short-slug.md`). Status moves from Proposed to Accepted, then optionally Deprecated or Superseded. Accepted ADRs are immutable; add a superseding record rather than rewriting history. Cross-link them to architecture pages.

## Index

| ADR | Title | Status |
|---|---|---|
|[ADR-0001](0001-remote-rebase-status.md)|Derive rebase status from remote state|Proposed|
|[ADR-0002](0002-read-stack-from-github.md)|Read stack structure from GitHub; use gh stack only for actions|Accepted; single-mode missing-data and action rules superseded by [ADR-0003](0003-single-pr-and-no-pr-mode.md)|
|[ADR-0003](0003-single-pr-and-no-pr-mode.md)|Show a single PR or current branch without a GitHub stack|Accepted|

## Template

```markdown
# ADR-NNNN: <decision>

- **Status:** Proposed
- **Date:** YYYY-MM-DD
- **Deciders:** <participants>

## Context

Forces and requirements, not the choice.

## Decision

State what changes in active voice.

## Consequences

Benefits, costs, and obligations.

## Alternatives considered

Viable options and reasons they were rejected.
```
