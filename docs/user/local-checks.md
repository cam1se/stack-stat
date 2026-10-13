# Local checks

`stack-stat report` records check outcomes for the branch and commit an agent actually tested. Results are stored under Git's common directory and are visible from the repository's other worktrees. If the branch tip changes, the earlier outcome is marked stale rather than presented as a fresh pass.

```sh
stack-stat report lint pass --message 'clippy clean'
stack-stat report test fail --branch feat/api --sha HEAD~1 --message 'three assertions failed'
stack-stat report --list
stack-stat report --list --json
stack-stat report --clear --branch feat/api
```

Use `--branch` and `--sha` when an agent moves between branches before recording a result. `--sha` accepts any revision Git resolves to a commit. When only `--branch` is supplied, the report applies to that branch's current tip; without either flag, the current worktree branch and its tip are used. A detached HEAD needs an explicit branch. `--list --json` includes a `stale` flag so tools can distinguish old evidence.

A repository can declare expected checks under `[checks.local]` in `.stack-stat.toml`. Reporting an undeclared name is accepted but prints a note. The dashboard will show declared checks without a result as missing. [Configuration](configuration.md) describes the file; see [the installer](index.md) for setup.
