# Feature 511 experiments

These drivers call the public installed Behavior API. They are not certificate authorities.

## Provenance

- `../evidence/remediation-baseline.json`: exact released wheel, source/dependency/tool pins,
  initial unrelated-work hashes and preexisting gate failures.
- `../evidence/investigation-inputs.json`: retained research input checksums at remediation start.
- `../evidence/v1-regression-corpus.json`: five independently counted reference constructions and
  existing v1 regression groups. These entries do not claim that proofs were rerun.
- Prior raw attempts: `/tmp/behavior-511-investigation.zqro9o/`; compact archive and discarded
  controls remain in `../evidence/` and `../root-cause-analysis.md`.

Use a new task-owned output directory under `target/safety-models/scale-511/` for each attempt.
Never overwrite an earlier attempt or a published artifact. Store compact input hashes, commands,
status and measurements in this feature's evidence directory; avoid duplicating large generated
proof files. Failed/unfinished attempts remain observations and cannot become passing results.

Heavy proofs and measurements run sequentially. Baseline/producer/consumer use installed packages
and fresh workspaces, with independent expected module/policy reconstruction. Result reuse cannot
replace required checks. Freeze acceptance conditions separately from per-build identities.

Existing `certification_probe.py` is FO-only; `--verify` uses the same process and is not the final
cold runner. `lean_stage_probe.py` is diagnostic, not production certification. Historical invalid
marker/import controls were discarded explicitly in the analysis; preserve that classification.

## Environment

The released consumer installation is isolated from the source binding. Select the pinned Rust/
Python environment for later builds. The existing `.gitignore` already ignores Rust target output,
Python caches/environments, wheels under dist, logs and temporary files; no ignore edits are needed.
The initial public-surface gate is blocked by two unrelated untracked user documents without
ownership rules. Preserve them. Required final checks must isolate feature changes from that
preexisting state rather than delete user work or change unrelated ownership policy.

New runner/generator test and CLI instructions will be added with their implementations.
