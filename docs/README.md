# stack-stat documentation

## Conventions

1. Documentation is first-class: update the documents describing a change in the same PR.
2. Earn your words. Prefer missing documentation to inaccurate or weightless content.
3. Avoid rot-prone counts, exhaustive lists, and copied API signatures. Describe patterns and link to code or rustdoc.
4. Use Mermaid `flowchart` or `sequenceDiagram` for relationships, with `subgraph` boundaries where useful.
5. One source of truth per fact; link rather than repeat.
6. Cross-references to docs must be navigable Markdown links, not paths in backticks.
7. Living architecture docs move with structural changes; significant hard-to-reverse decisions get ADRs.
8. The top-level `docs/` file set is complete: only this file and [goals](goals.md) belong here. Put new reference material in a subdirectory.

## Map

| Location | Content | Lifetime |
|---|---|---|
| Root [README](../README.md) | Summary and install pointer | Living |
| [User docs](user/) | Installation, usage, configuration; published on [GitHub Pages](https://tcamise-gpsw.github.io/stack-stat/) | Living |
| Root [AGENTS.md](../AGENTS.md) | Contributor and agent workflow | Living |
| [Architecture](architecture/) | Context, crate boundaries, rationale, extending recipes | Living |
| [ADRs](adr/README.md) | Significant decisions and alternatives | Append-only |
| [Specs](specs/) | Reviewed feature intent | Point-in-time |
| [Plans](plans/) | Change sequencing and execution notes | Point-in-time |
| Rustdoc (`cargo doc`) | Per-symbol API documentation | Generated |

Living docs describe what currently exists. Point-in-time docs describe intended changes and must not be mistaken for implemented behavior. Do not add a parallel guide hierarchy that duplicates these sources.

## Architecture

Read in this order:

| Page | Scope |
|---|---|
| [01 Context](architecture/01-l1-context.md) | Users and external systems |
| [02 Containers](architecture/02-l2-containers.md) | Binary, library, and local data |
| [03 Distribution](architecture/03-distribution.md) | Versioning, packages, and documentation site |
| [04 Domain and config](architecture/04-domain-and-config.md) | Model vocabulary and configuration precedence |

The first two pages follow the [C4 model](https://c4model.com); later numbered pages cover subsystems. Put extending recipes beside their subsystem. Do not hand-model generated schemas or public Rust signatures. Link to rustdoc instead.

## ADRs

[ADR conventions and template](adr/README.md) cover numbering, lifecycle, and cross-linking. Rationale for everyday reversible choices belongs inline in architecture docs.

## Work in progress

[Specifications](specs/) capture reviewed design; [plans](plans/) capture execution order and findings. Fold durable structure into architecture docs and durable choices into ADRs when work ships.
