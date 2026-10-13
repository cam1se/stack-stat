# Configuration

The dashboard will read repository-specific `.stack-stat.toml` from the worktree root and personal display preferences from the user config directory. It will use the repository file to declare expected local checks and filter CI check names. The [design](https://github.com/tcamise-gpsw/stack-stat/blob/main/docs/specs/001-stack-stat-design.md) defines the intended schema until the feature ships.
