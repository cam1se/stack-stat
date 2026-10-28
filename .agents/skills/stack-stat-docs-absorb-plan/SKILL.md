---
name: stack-stat-docs-absorb-plan
description: Use after a plan ships to fold durable structure and decisions into living docs and close the point-in-time plan.
---

# Absorb a shipped plan

Read the design in `docs/specs/`, the implementation plan and notes in `docs/plans/`, and the current code. These point-in-time directories exist only while work is in flight and are absent when nothing is pending; if neither exists, there is nothing to absorb. Findings from implementation take precedence over intentions when they differ. Leave unshipped plans alone.

Classify each lasting finding:
- Current subsystem structure or an extending recipe belongs in [architecture](../../../docs/architecture/).
- Significant choices and rejected alternatives belong in an [ADR](../../../docs/adr/README.md) via `stack-stat-docs-create-adr`.
- Symbol-level API detail belongs in rustdoc.
- Sequencing and discarded approaches do not belong in living docs.

Update the architecture map in [docs conventions](../../../docs/README.md), cross-link ADRs, and resolve touched local links. After lasting content has been committed, remove the shipped `docs/plans/NNN-*-plan.md` and notes, and the corresponding completed spec if it has no ongoing use. Never delete in-flight artifacts. Report what was absorbed and discarded.
