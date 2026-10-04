# Quickstart: validating the split

Run every command inside `nix develop` in the repository named. Expected outcomes refer to the
spec's success criteria. The contracts hold the details:

- [behavior-engine](contracts/behavior-engine.md)
- [core release](contracts/core-release.md)
- [ownership](contracts/ownership.md)
- [workflows](contracts/workflows.md)
- [Spec Kit](contracts/spec-kit.md)

## 0. Prerequisites

- Feature 010 is committed and merged; `main` is at 0.10.0.
- `../behavior-ir-core` is cloned.
- `git-filter-repo` is available (`nix shell nixpkgs#git-filter-repo`).
- A GitHub token can create releases in both repositories. CI needs `CORE_READ_TOKEN` if the
  core repository is private.

## 1. Preflight (behavior-ir, before the move)

```sh
scripts/conformance-digest.sh > /tmp/digest-before.json
scripts/preflight.sh
```

Expected: five lines, `preflight: <criterion> OK`, and exit 0. Each criterion is seen failing
first on today's tree. For example, `ecosystem consumes the public surface` fails because
`behavior-py` depends on `behavior-cli`.

## 2. The core stands alone (US1, SC-001)

```sh
git clone git@github.com:kallistoteles/behavior-ir-core.git /tmp/core && cd /tmp/core
nix develop -c scripts/gates.sh                     # fmt, clippy, tests, determinism, boundary, surface
nix develop -c scripts/check-consumer.sh            # external consumer of behavior-engine only
scripts/conformance-digest.sh | diff - /tmp/digest-before.json   # SC-009: no output
rg -l 'behavior-py|behavior\._engine|python/behavior' --glob '!specs/011-*' --glob '!docs/proposals/**'   # no output
git log --oneline -- crates/behavior-core/src/read.rs | wc -l     # > 1: history preserved (FR-020)
```

## 3. First Core Release (FR-006a, FR-028, FR-029)

```sh
git tag -a v0.10.2 -m test && git push origin v0.10.2   # wrong version → core-release refuses, naming 0.10.2 and 0.10.1
git push origin :v0.10.2                                # (test tag only; real tags are never deleted)
git tag -a v0.10.1 -m "Behavior Core 0.10.1" && git push origin v0.10.1
```

Expected: the release has `release-manifest.json`, `SHA256SUMS`, the conformance tarball and
the CLI binary. Then:

```sh
scripts/release-verify.sh v0.10.1
```

It prints `identical` (SC-011).

## 4. The ecosystem uses only the public contract (US2, SC-002, SC-008)

```sh
cd ~/repos/deterministic_ai_system        # behavior-ir
scripts/check-core-pin.sh                 # rev == core-release.json commit, no override
scripts/check-public-surface.sh --consumer
scripts/fetch-core.sh                     # downloads + verifies the bundle and CLI into .core/0.10.1/
scripts/gates.sh                          # maturin develop, pytest, mypy, determinism (ecosystem)
scripts/conformance-digest.sh --core-dir .core/0.10.1 | diff - /tmp/digest-before.json   # SC-003
```

Negative checks, each expected to fail and name the offending item:

- Add `use behavior_core::admit;` to `crates/behavior-py/src/lib.rs`.
  `check-public-surface.sh` names `behavior_core`.
- Set `rev` to another commit. `check-core-pin.sh` names both commits.
- Build against a core whose engine version is 0.10.9, then `python -c "import behavior"`.
  The `ImportError` names 0.10.9 and 0.10.1.

## 5. One semantic truth (US3, SC-004)

```sh
python -m pytest -q python/tests/test_binding_equivalence.py
```

Expected: one pair per wire IR form passes, covering entities and types, actions and lifecycle,
queries and invariants, exact arithmetic, migrations and reads. Change one DSL emission, for
example a field order. The test fails and names the fixture and the differing item hash.

## 6. One install (US4, SC-005)

```sh
scripts/release-check.sh 0.10.1
python3.13 -m venv /tmp/u && /tmp/u/bin/pip install dist/v0.10.1/*.whl
/tmp/u/bin/python -c "import behavior, json; print(json.dumps(behavior.versions()['core']))"
/tmp/u/bin/behavior engine-info
```

Expected:

- The release check prints `release-check: OK`.
- The `core` entry is `{"commit": "<core sha>", "version": "0.10.1"}`.
- `engine-info` reports `engine: 0.10.1`.

## 7. Where a concept belongs (US5, SC-007)

```sh
python -m pytest -q models/examples/state_machine
```

Expected: the lowered `submit` transition admits through the core, and its IR contains only
`requires`, `set` and `ensures`. Then open the core's `ARCHITECTURE.md` and classify the 12
concepts from the table. Each concept has exactly one layer.

## 8. CI and releases (US6, SC-010, SC-012)

- Open one pull request per gate in [contracts/workflows.md](contracts/workflows.md) §Proving
  each gate. Each turns its job red.
- Merge a green pull request to `main`. No release is created.
- Run `/speckit-specify --number 500 …` in the ecosystem; it creates `specs/500-…`. The next
  feature, created without `--number`, gets 501.
- Run `/speckit-specify` in the core; it creates `specs/012-…`.
