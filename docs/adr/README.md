# Architecture Decision Records

Record a decision when it affects structure and is hard to reverse, or when viable alternatives are worth remembering. Reversible choices belong in [architecture](../architecture/01-l1-context.md).

Number ADRs chronologically with zero-padded permanent IDs (`0001-short-slug.md`). Status moves from Proposed to Accepted, then optionally Deprecated or Superseded. Accepted ADRs are immutable; add a superseding record rather than rewriting history. Cross-link them to architecture pages.

## Index

| ADR | Title | Status |
|---|---|---|

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
