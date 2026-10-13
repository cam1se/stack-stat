# 02 — Containers and data

The Cargo workspace has a reusable domain library and a terminal binary. The binary's default command renders the dashboard; its `report` command writes check results and its `update` command updates installed releases.

```mermaid
flowchart LR
  subgraph Binary
    TUI[stack-stat-tui: dashboard and CLI]
  end
  subgraph Library
    Core[stack-stat-core: model, config, providers]
  end
  subgraph Local
    Git[Git worktree and common dir]
    Store[.git/stack-stat/status]
    Config[.stack-stat.toml]
  end
  subgraph Remote
    Gh[gh stack and GitHub API]
  end
  TUI --> Core
  Core --> Git
  Core --> Store
  Core --> Config
  Core --> Gh
```

The library owns the provider contracts so the dashboard can be exercised with fake git, GitHub, and status-store providers. The store lives under Git's common directory to keep results shared among worktrees and out of tracked files. Per-repository configuration stays at the worktree root; user display preferences stay in the platform config directory.

Read [system context](01-l1-context.md) for actors and external systems and [the design](../specs/001-stack-stat-design.md) for detailed planned behavior.
