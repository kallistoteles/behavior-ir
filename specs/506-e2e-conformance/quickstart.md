# Quickstart: End-to-End Behaviour Conformance (feature 506)

Run everything from the repository root inside `nix develop`, after `scripts/fetch-core.sh`
(which installs the pinned Core CLI) and `maturin develop`.

## Run the reference application

```sh
python -m models.examples.purchase_approval.run
```

The command prints each lifecycle step of `contracts/reference-workflow.md` §1 with its
outcome, then the final state, the command stream, and the data and behavior replay results.
It exits non-zero if any step differs from its expectation.

## Run the conformance suite

| # | Command | Expected |
|---|---|---|
| 1 | `scripts/conformance.sh quick` | Exit 0. The report `target/conformance/506-report.json` has every scenario. C19 is `unsupported` (quick mode) and everything else is `passed`. |
| 2 | `BEHAVIOR_FULL_PROPERTIES=1 scripts/conformance.sh 506 full` | Exit 0 (the integration gate). On Core 0.13.0 C19 was `blocked` by behavior-ir-core#35; since Core 0.15.0 (feature 508) every check is `passed` and the output ends "full conformance: complete". |
| 2b | `scripts/conformance.sh 506 complete` | Exit 0 since Core 0.15.0: full conformance needs every check passed. |
| 3 | `jq '.summary' target/conformance/506-report.json` | The counts per status. `failed`, `inconclusive`, `unverifiable` and `unsupported` are 0 in full mode. |
| 4 | `scripts/tests/test_conformance_report.sh` | The status rules hold (contract `conformance-report.md` §5). |
| 5 | `scripts/gates.sh` | `gates: OK`, with the `conformance` gate in quick mode. |
| 6 | Run 1 twice and compare the reports | Byte-identical (FR-019). |

## What is demonstrated, and what is not

**Demonstrated**:
- construction, admission and identity (L1);
- evaluation decisions and refusals at their boundaries (L2);
- the Store's commit contract under contention, stale observations and re-creation (L3);
- data replay, behavior replay, evidence integrity, commit-history consistency and static
  verification of the reference module (L4);
- authoring agreement between Python and Rust;
- execution agreement between the Python engine interface and the released CLI.

**Not demonstrated**:
- correctness of Core itself: both execution paths run the same engine;
- properties outside the verifier's modeled fragment and assumptions;
- backends other than the in-memory one;
- external command execution.
