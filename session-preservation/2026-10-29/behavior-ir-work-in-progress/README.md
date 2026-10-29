# Session preservation — 2026-10-29

This is a work-in-progress preservation snapshot, not an approved release or deployment.

Original branch: `510-safety-models`; original HEAD: `21ba441299998ca0c6baf53a5178d1b591216b1f`.

The commit preserves tracked modifications and nonignored source/research files. Existing published tags and records are unchanged. Rebuildable caches and machine private keys are not included. Nested Git worktrees are preserved independently.

Local data/configuration and retained distribution files are encrypted to the existing user age identity. Restore with your separately retained age identity:

```sh
age --decrypt --identity /path/to/age-identity local-state.tar.gz.age | tar -xz -C /path/to/restored-workspace
```

For the MCP test clone, `parent-workspace/` contains the surrounding non-Git test workspace. SQLite files use individually consistent SQLite backup snapshots; no atomic snapshot across databases is claimed.
