# Quickstart: Agent-Ready Package and Skills Library

A validation guide for feature 008. References: [contracts/release.md](contracts/release.md),
[contracts/skills.md](contracts/skills.md), [contracts/public-api.md](contracts/public-api.md).

## Prerequisites

Run `nix develop` (the shell now includes zig and auditwheel), then `maturin develop` for the
in-repo checks.

## 1. Gates and in-repo checks

```bash
cargo fmt --check && cargo clippy --all-targets -- -D warnings
cargo test --workspace
pytest python/tests && mypy        # includes public API, skill drift and binding equivalence tests
scripts/determinism-check.sh
```

Expected: everything green. `python/tests/test_public_api.py`, `test_skills.py` and
`test_binding_equivalence.py` are among the passing tests.

## 2. Build and check a release (US1, US5)

```bash
scripts/release-check.sh            # builds dist/v0.8.0/ into a temp dir and runs all release checks
```

Expected:

1. `auditwheel show` reports `manylinux_2_28_x86_64` compliance.
2. In a fresh venv outside the repository, with no `cargo` or `rustc` on `PATH`, the wheel
   installs with `--no-index --require-hashes`.
3. `release/smoke.py` succeeds there, covering author, admit, evaluate, verify, store, commit,
   replay and the CLI `admit`, `eval`, `replay` and `verify`. Its canonical output is identical to the
   in-repo run.
4. Every `skills/*/examples/*.py` passes in that venv.
5. With `BEHAVIOR_Z3=/nonexistent`, verification fails with the named prerequisite; evaluation and
   replay still work.
6. `behavior engine-info` and `behavior.versions()` equal `release-manifest.json`.

A deliberately broken example, for instance a changed assertion in
`skills/behavior-authoring/examples/`, makes the check fail and name the file.

## 3. Consumer install (by hand, US1)

```bash
mkdir /tmp/consumer && cd /tmp/consumer && python3.13 -m venv .venv
.venv/bin/pip install --require-hashes -r requirements.txt   # contract: release.md
.venv/bin/python -c "import behavior; print(behavior.versions())"
.venv/bin/behavior engine-info
```

## 4. Skills (US2–US4, US6)

- `ls skills/` lists exactly `behavior-authoring`, `behavior-verification`,
  `behavior-application`, plus `README.md` and `evals/`.
- `.claude/skills/behavior-engine-development/SKILL.md` exists, and `skills/` does not contain it.
- Each consumer `SKILL.md` has `release: 0.8.0` and contains the verbatim gap rule.

## 5. Agent acceptance (manual, SC-004–SC-006)

Copy `skills/*` from the release tag into a scratch consumer repository's agent skills directory,
and install the pinned wheel. Run an agent on each set in `skills/evals/` and score it with the
set's rubric. Record the results in `specs/008-agent-ready-package/evals/results.md`.
