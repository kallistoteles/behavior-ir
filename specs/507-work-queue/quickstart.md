# Quickstart: WorkQueue (feature 507)

Run everything from the repository root inside `nix develop`, after `scripts/fetch-core.sh` and
`maturin develop`.

## Run the reference application

```sh
python -m models.examples.work_queue.run
```

It does the following:
- prints each step of the reference history (spec, Reference history) with its expected and
  actual outcome;
- prints the final state and the committed commands;
- runs data and behavior replay;
- exits non-zero on any difference.

## Run the checks

The 507 quick command enforces the reviewed **35 s** runtime budget after writing its report.
The original 30 s goal was exceeded; plan.md records the explicit amendment and measurements.
Build the Rust targets and extension first, as above. Full and complete modes retain their
larger property suites and have no quick-mode time limit.

| # | Command | Expected |
|---|---|---|
| 1 | `cargo test -p behavior-models --test work_queue_resolve --test work_queue_lower --test work_queue_ownership --test work_queue_identity --test work_queue_laws --test work_queue_reference --test state_machine_bindings` | resolution, lowering, ownership, identity, the lowering laws, the Rust reference authoring and the transition bindings pass |
| 2 | `scripts/conformance.sh 507 quick` | exit 0; `target/conformance/507-report.json` lists W01–W22, W23a, W23b, W24 and W25 (56 passed, 5 unsupported). Every VO has its dependencies; the SMT and backend dependencies are `unsupported` in quick mode (5 VOs passed, 7 unsupported) |
| 3 | `BEHAVIOR_FULL_PROPERTIES=1 scripts/conformance.sh 507 full` | exit 0; 61 passed; the verifier's preservation, postcondition and evaluation-error checks are `passed`; all 12 obligations passed; "full conformance: complete" |
| 4 | `scripts/conformance.sh 507 complete` | exit 0 only when every check and every VO dependency passed |
| 5 | `scripts/conformance.sh 506 quick && scripts/tests/test_conformance_506_bytes.sh` | byte-identical to the baseline from `main` (506 unaffected) |
| 6 | `jq '.obligations["VO-005"]' target/conformance/507-report.json` | the structural, SMT, test and negative-control dependencies, with assumptions listed separately |
| 7 | `scripts/gates.sh` | `gates: OK`; the compat gate includes the `work_queue` tuple, and existing tuples are unchanged |

## What is demonstrated, and what is not

**Demonstrated**:
- valid construction, lowering and identity of a WorkQueue;
- every capability's guards and effects, evaluated by Core;
- fencing of stale generations through ordinary transitions;
- exclusive claims under the Store contract;
- replay;
- full-module ownership, with a reviewable argument and negative controls for each safety
  property.

**Not demonstrated**:
- physical deadline enforcement at commit (results are relative to the recorded `now`);
- single physical execution or exactly-once effects;
- safety across migrations that break the host migration contract;
- provenance of leases imported at genesis;
- fairness or ordering.
