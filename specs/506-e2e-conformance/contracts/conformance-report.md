# Contract: Conformance report (feature 506, FR-021, FR-022)

## 1. Document

`target/conformance/506-report.json` is a canonical JSON document: sorted keys, no
insignificant whitespace, and no timestamp. Its shape is data-model §7. The same outcomes give
byte-identical reports.

## 2. Statuses

| Status | Produced when |
|---|---|
| `passed` | the scenario test returns normally |
| `failed` | an assertion fails, an unexpected exception is raised, or a scenario test is skipped by plain `pytest.skip` |
| `inconclusive` | the test raises `Inconclusive(reason, owner)` |
| `unverifiable` | the test raises `Unverifiable(reason, owner)` |
| `unsupported` | the test raises `Unsupported(reason, owner)` |
| `blocked` | the scenario is listed under an open issue in the readiness registry (data-model §5), or the test raises `Blocked(issue)` |

`Inconclusive`, `Unverifiable`, `Unsupported` and `Blocked` are the suite's own exception
types, defined in `conftest.py`. Every status other than `passed` and `failed` needs a non-empty
`owner`, which is a layer (`core`, `ecosystem`, `environment`) or an issue.

## 3. Coverage

Every scenario id of `reference-workflow.md` §3 must appear at least once in `checks`. A missing
scenario fails the report: the hook adds it as `failed` with the reason "not run".

## 4. Gate rules (`scripts/conformance.sh`)

| Mode | The script fails if |
|---|---|
| `quick` | any check is `failed` |
| `full` (the integration gate CI runs) | any check is not `passed`, except `blocked` |
| `complete` (full conformance) | any check is not `passed`, `blocked` included |

The report's `complete` field is true only when every check passed. A blocked check lets the
integration gate pass, with the blocker recorded, but full conformance stays not passed until
the blocking issue is resolved or the requirement is formally revised.

A readiness entry with `"run": true` does not stop its checks from running. The check reports
`blocked` only when its exact result is the documented limitation (until Core 0.15.0, C19: the uniqueness
postcondition of the two decision actions, inconclusive with `counterexample_not_reproduced`).
Any other result keeps its own status, so a new finding can never hide behind the blocker.

Full mode runs C19 (static verification) and the full property depth. Quick mode reports C19 as
`unsupported` (owner `environment`, reason "static verification runs in full mode").

## 5. Self-tests (`scripts/tests/test_conformance_report.sh`)

- each exception type yields its status;
- a plain skip yields `failed`;
- a status other than `passed` or `failed` without an owner is rejected;
- a scenario missing from the run is `failed` ("not run");
- a scenario under an open readiness entry is `blocked`, never `passed`;
- quick mode passes with `unsupported` C19, and full mode fails with it;
- full mode passes with a `blocked` check, and complete mode fails with it;
- two runs on the same outcomes give byte-identical reports.
