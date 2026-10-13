# 01 — System context

stack-stat presents the current repository's stacked pull requests to a developer or coding agent in a terminal. It reads local Git and uses the authenticated `gh` CLI and `gh stack` extension to retrieve stack and GitHub state. Agents, hooks, and scripts report local checks into a repository-local store shared across worktrees.

```mermaid
flowchart LR
  subgraph People
    Dev[Developer]
    Agent[Coding agent]
  end
  subgraph Terminal
    Stat[stack-stat]
  end
  subgraph Local
    Git[Git repository and worktrees]
    Store[Local check status store]
    Reporter[Check reporters]
  end
  subgraph Remote
    Gh[gh and gh stack]
    GitHub[GitHub API]
  end
  Dev --> Stat
  Agent --> Stat
  Agent --> Reporter
  Reporter --> Store
  Stat --> Store
  Stat --> Git
  Stat --> Gh --> GitHub
```

The stack-stat binary does not replace `gh stack`; it uses that extension as the authority on stack membership. The dashboard follows the worktree's checked-out branch. When that branch is outside a stack, it displays a not-in-stack view rather than guessing a stack.

See [container boundaries](02-l2-containers.md) for binary and library responsibilities and [the terminal application](06-tui.md) for interaction behavior.
