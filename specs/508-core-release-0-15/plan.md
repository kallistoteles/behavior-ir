# Implementation Plan: Core Release 0.15.0 alignment

**Branch**: `508-core-release-0-15` | **Date**: 2026-10-17 | **Spec**: [spec.md](spec.md)

## Summary

Pin Core 0.15.0 (from 0.13.0) and show two things separately:

1. **Semantics are unchanged.** An archived semantic snapshot, taken on 0.13.0 *before* the pin
   changes, equals the 0.15.0 snapshot. The snapshot covers every compat reference case (all
   receipts without their `core_release`, and BehaviorHash) and the 506 and 507 reference
   histories (decisions, positions, state identities, command streams).
2. **Verification improves as intended.** The 506 full report goes from C19 `blocked` (0.13.0,
   archived) to C19 `passed` (0.15.0). The 507 obligations stay passed. The verifier moves from
   0.9.0 to 0.10.0.

Only then are the new pinned baselines and compat tuples recorded.

## Technical Context

- **Language and tools**: Rust stable, Python 3.13 binding, `behavior-engine` at Core commit
  `6b93898a9375113f7a8946f23ab5535319b6f5d6`. No new dependency.
- **Testing**: the existing gates, the 506 and 507 suites, and `scripts/semantic-snapshot.py`
  (new: snapshot and compare).
- **Constraints**:
  - no Core change and no lowering change;
  - baseline tests keep their strictness;
  - disk: a cold build needs about 4.6 GB.

## Constitution Check

| Principle | Assessment |
|---|---|
| I–II | No AI; typed errors for the new canonicalization failures (FR-002). ✅ |
| III Test-First | The semantic comparison is captured before any change. The 0.15 failures of the existing tests (CORE_31 strict xfails, 0.9 policies, alignment version checks) are observed before they are fixed. ✅ |
| IV Reproducibility | The semantic snapshot is canonical JSON; equal inputs give equal bytes. ✅ |
| V Auditability | Both comparisons are archived under `specs/508-core-release-0-15/evidence/`. ✅ |
| VI Simplicity | One script added; the #29 workaround is removed. ✅ |
| Bindings and packaging | `behavior-engine` only; the exact Core Release in `core-release.json`, `Cargo.toml` and `Cargo.lock`. ✅ |

**Result: PASS.**

## Research (facts, checked 2026-10-17)

| # | Fact | Consequence |
|---|---|---|
| R1 | 0.14.0 (021): `verify` returns `Result<Attestation, CanonicalError>`; `Attestation::to_json_string`, governance `Authorization::to_json_string` and the migration attestation's `to_json_string` return `Result`. New re-export `canonical::CanonicalError`. Nothing removed. | `crates/behavior-py/src/lib.rs`: 4 call sites (authorize, verify, verify_migration, authorize_migration) map the error to an engine error. |
| R2 | 0.14.0 (027): location relativization considers only absolute paths; non-absolute names stay verbatim. Hashes unchanged. | Remove the strict `CORE_31` xfails in `python/tests/test_source_locations.py` and update `python/behavior/location.py`. |
| R3 | 0.14.0 (026): `tests/fixtures/bindings/current/` holds complete requests and expectations. The old fixtures are unchanged. | `python/tests/test_binding_equivalence.py`: drop the snapshot completion and use `current/` (Q10). |
| R4 | 0.15.0 (028): verifier 0.10.0. Fresh attestations name 0.10.0, and fresh authenticated proofs need a 0.10 policy (`governance-v2/*-verifier-0.10.json`). | `python/tests/test_command_documents.py`: use the 0.10 policies (Q9). |
| R5 | `engine-info`: engine 0.15.0, verifier 0.10.0; CLI flags unchanged. | `python/tests/test_core_alignment.py`: update the version expectations. |
| R6 | The 506 Approval module's BehaviorHash is `sha256:a04ba37…531779`, the module Core 028 proves (`specs/028-…/evidence/baseline.json`). | C19 is expected to pass unchanged (Q5). |
| R7 | Wire, record and Store formats, BehaviorHash and SchemaHash rules are unchanged. | The semantic snapshot must be equal (Q4, Q8.1). |
| R8 | Release assets: `behavior-0.15.0-x86_64-linux-musl` sha256 `89668836…f8891b`; `behavior-conformance-0.15.0.tar.gz` sha256 `d1e50c2c…d5f7`. | `core-release.json`. |
| R9 | Governance tests use fixed time (`T0`, `--now T0`). | No clock dependency (edge case). |

## Phases

0. **Baseline on 0.13.0** (before any change): semantic snapshot, `--print` of every receipt,
   and the 506 full report (C19 blocked) and 507 full report, archived under `evidence/0.13.0-*`.
1. **Pin**: `core-release.json`, `Cargo.toml` `rev`, `Cargo.lock`, `fetch-core.sh`; build.
2. **API**: the Result call sites (R1).
3. **Tests that change with the release**: observed failing first (R2, R4, R5), then fixed;
   remove the #29 workaround (R3).
4. **Semantic comparison** on 0.15.0: snapshot, compare with phase 0, archive. Stop on any
   difference.
5. **Verification comparison**:
   - 506 full: C19 passed, with the evidence of FR-007;
   - empty the readiness registry (#29, #31, #35);
   - 507 full: the obligations stay passed;
   - archive.
6. **Baselines and compat**:
   - replace the 0.13.0 tuples with 0.15.0 tuples (identical case hashes; the gate's
     unpinned-release rule);
   - retake `compat-print.txt` and the 506 quick report baseline for the new pin, each citing
     the archived comparisons.
7. **Docs**: README C19 paragraph, 506 readiness texts, `location.py`, `models/README.md`.
8. **Gates**: quick and full, then independent `complete` runs of 506 and 507.
