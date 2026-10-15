# Implementation log: Core Release 0.13.0 alignment

**Date**: 2026-10-15 | **Branch**: `504-core-release-0-13` (worktree `~/repos/behaviour-ir-504`)
| **Base**: `main` `47b4231` | **Core Release**: 0.12.0 (`b525da4`) → 0.13.0 (`1509497`)

Logs are kept in the session scratchpad (`…/scratchpad/logs/`); each entry gives its SHA-256.
Every command runs in `nix develop` with `CARGO_TARGET_DIR=~/repos/behaviour-ir/target`
(research R6).

## T001 Baseline (unchanged worktree, Core 0.12.0)

`scripts/gates.sh` → `gates: OK`. `cargo test -p behavior-models`: 113 passed; pytest
`python/tests models`: 475 passed, 4 skipped; mypy, models compat, determinism, ownership,
terms, workflows and script tests pass. Log `504-baseline-gates.log`, SHA-256
`8f05981fd70f25d4add25614c80e49df0acb1cac599eb957dd2d47d9d4e30b57`.

(A first attempt failed at the `core pin` gate with `cargo: kommandot finns inte` because the
helper overrode the dev shell's `PATH`; the helper was fixed. Not counted as evidence.)

## T002 Disk and shared target

8.6 GB free on `/` (78 % used); no other cargo process running.

## T003–T005, T012–T013 Red against Core 0.12.0

Log `504-red.log`, SHA-256 `c4d81d1f8c8deeb39b5451b4a72b4f33b2a85ed677f1a747c847c7b6d59b7fca`.

- `core_assumptions` (Rust): 3 passed (existing), **3 failed**: all three
  `command_profile_query_*` tests panic at `Store::invoke` with
  `BundleInvalid("INVALID_CANDIDATE: invalid observed facts")`, the 503 R13 defect.
- pytest: **11 failed**, 22 passed, 4 skipped. Failing: `test_core_alignment.py` (pin is 0.12.0;
  engine, `accepted_wire_ir`, `records`, `verifier` report the 0.12.0 values),
  `test_versions.py` (package 0.12.0), and all three `test_query_command_profile.py` cases with
  `CommitRefused: BUNDLE_INVALID: INVALID_CANDIDATE: invalid observed facts` (the Wire 0.8
  document is admitted and invoked; Core refuses its own candidate).
- `test_release_scripts.py` is skipped in this environment (as in the baseline) and the
  dependency-guard test is independent of the version argument, so T005 has no red evidence;
  its updates are consistency changes.
- Research correction found here: `accepted_wire_ir` changes `0.1…0.8 → 0.4…0.8` (Core 020);
  R3, contracts/versions.md, spec US1 scenario 3 and T003 were updated.

## US1 (T006–T011) Pin moved to 0.13.0

- T006: `core-release.json` built from the published `release-manifest.json` (commit, tag and
  both asset digests agree with `SHA256SUMS`).
- T007: `Cargo.toml` version 0.13.0 and `behavior-engine` rev `1509497…`; `cargo update -p
  behavior-engine` changed only `behavior-{core,engine,store,verify}` (0.12.0 → 0.13.0, new
  source) and the two workspace crates' versions in `Cargo.lock`; no third-party change.
- T008/T009: `check-core-pin: OK (v0.13.0 15094972191aa0ae79705df0397c551a18745399)`,
  `fetch-core` installed and verified both assets, `test_fetch_core: OK` (includes the checksum
  refusal); `cargo build --workspace` and clippy `-D warnings` clean with **no binding source
  change** (research R2 confirmed). Log `504-us1-build.log`.
- T010: `api/public-api.json` `release` 0.13.0 only.
- T011: after `maturin develop` (wheel `behavior-0.13.0`), the version/release tests pass and
  `engine-info` equals contracts/versions.md (engine 0.13.0, verifier 0.9.0, records 0.4–0.8,
  accepted Wire IR 0.4–0.8, historical `wire_ir` 0.1–0.7, store documents unchanged).

## US2 (T014) Green

`core_assumptions`: 6 passed (3 new). pytest (alignment, versions, release, query command
profile): 33 passed, 4 skipped. Log `504-green-us1-us2.log`, SHA-256
`113862de129e7d818ecead57fbbdffe2a57c2caf9453828854e28c61785d0bc6`.

## T015 Full suites at 0.13.0 and classification

Rust `cargo test --workspace`: all pass (116 = baseline 113 + 3 new). pytest: **96 failed**,
382 passed, 4 skipped. Logs `504-t015-full.log` (SHA-256
`e16c9a8d0e55363ee6c261a959168c8e62408895ec5957254d3e6c9dc5a7c334`) and `504-t015-lf.log`.

| Class | Count | Files | Cause |
| --- | --- | --- | --- |
| (e) complete snapshot | ~78 | state_machine 22, request_flow* 24, evaluate 7, queries 5, binding_equivalence 5, lifecycle 2, fixed_scale 2, constraints 2, authorize 2, exact_closure, boundary, cli_entry, intent, skill examples | `INVALID_INPUT / INVALID_STATE_SNAPSHOT`: direct evaluation without a complete universe (Core 017, research R9) |
| (a) fresh record version | 13 | invocation 10, command_documents 2–3 | fresh decision records are 0.8 with `validation_profile` and the parent universe; Core's own fixtures stay historical goldens (Core `cli_invoke.rs` compares result/changes/identity/replay instead of bytes) |
| (f) creation contract | 3 + 1 example | store_conformance 3, skills custom_backend | Core 019: 45 conformance cases (was 31); `create` must recover an equivalent original creation (R10) |
| (g) release metadata | 3 | skills front matter | skills still say 0.12.0 (T025) |
| (d) unexplained | 0 | | |

Research R2 was wrong for the Python API (R9, R10 added). User decision (2026-10-15): option A
without backward compatibility; spec "Scope decision", FR-010–FR-012 and tasks T016–T021,
T029–T031 updated accordingly.

## T016–T017 Complete parent snapshot as required input

- Red (`504-t016-red.log`, SHA-256 `736582b34fa62366a76c26017a499ba3141573a620599e0a8104a510781c20e9`): 5 failed —
  `evaluate` without `facts` did not raise `TypeError`; `evaluate_intent` had no `facts`
  parameter (3 intent tests); `test_invalid_starting_state` (see below).
- Change: `facts` is a required keyword of `behavior.evaluate` and a new required keyword of
  `behavior.evaluate_intent`; the binding forwards it unchanged into the request / the
  `ActionIntent` host document (`crates/behavior-py/src/lib.rs`, `_engine.pyi`). Docstrings
  state the 0.13.0 contract. No fallback, no inference.
- Green: `test_evaluate.py`, `test_intent.py` 15 passed (`504-t017-green.log`).
- Class (c), Core 017: an invariant-violating parent is now `INVALID_INPUT` /
  `INVALID_STATE_SNAPSHOT` with message `facts: Invoice#1042: invariant \`non_negative_amount\`
  is violated` and an empty trace (was `INVALID_STATE` with an `invariant_pre` step): the
  complete parent snapshot is validated before evaluation. The test asserts the new outcome.

## T029 Model compatibility tuple

Both `core_release` entries in `models/fixtures/compat.json` name 0.13.0 / `1509497…`;
`check-models-compat: OK` (3 request_flow + 4 state_machine cases, outputs unchanged) and
`test_models_compat: OK`. Lowered module identities are unchanged under 0.13.0.

## T022–T024 Documentation

README: Core links and release name 0.13.0 (all linked paths exist at `v0.13.0`), quickstart
passes a complete universe, the evaluation bullet and the lifecycle/query paragraphs describe
the complete-snapshot contract. models/README: link. docs/versioning.md: table rows, the
"No backward compatibility before 1.0" policy (user decision 2026-10-15) and the 0.13.0 release
paragraph; the historical 0.12.0 paragraph keeps its `v0.12.0` links.

## T018–T021, T025 Migration to the 0.13.0 contract

Done in three parallel parts with disjoint files; no package helper, shim or fallback was added
(`python/behavior/` changes only in T017). Each test file builds its complete universe locally
from its own data.

- Request flow and state machine (6 files): 108 passed. Every direct evaluation passes the
  complete universe of the model's declared types; creations keep their `identities` facts.
- Remaining binding tests (11 files): 344 passed. `test_invocation.py` / `test_command_documents.py`
  mirror Core's own fixture test (`behavior-cli/tests/cli_invoke.rs` at `v0.13.0`): fresh
  decision records are 0.8 with the golden's result and changes, a correct self-identity, and
  both the fresh record and the frozen golden replay; non-decision outcomes stay byte-equal.
  Fresh authenticated verification uses `governance-v2/*-verifier-0.9.json` (R5; the 0.8
  policies make `governance authorize` fail); historical ones are unchanged.
- Backends (T019): `DictBackend` and the `custom_backend.py` skill example implement Core 019's
  creation contract like Core's `InMemoryBackend::create` (private build; on a held genesis,
  refuse a different genesis, position-0 universe, versions, references or state anchor;
  otherwise no write). The conformance test asserts the 45 case names Core reports instead of
  a count; the deliberately broken backends still fail their cases.
- Examples (T025): every skill example and `examples/` script that evaluates directly passes a
  complete universe; `evaluate_intent` gets `facts`; reason messages and trace source text are
  read from `diagnostics` (record 0.8 keeps them out of canonical `reasons`/`trace`). All ten
  `examples.*` scripts run. `release/smoke.py` (determinism gate) passes a complete universe.

Class (c) outcome changes, all Core 017 / record 0.8, each asserted with a comment:

| Test | Before | After |
| --- | --- | --- |
| `test_evaluate::test_invalid_starting_state`, invoice/tryout examples | `INVALID_STATE` (`invariant_pre`) | `INVALID_INPUT` / `INVALID_STATE_SNAPSHOT` naming the invariant |
| `test_queries` incomplete snapshot | `ERROR` / `UNKNOWN_FACT` | `INVALID_INPUT` / `INVALID_STATE_SNAPSHOT` |
| `test_queries` allowed `close_customer` | records the Order query | also the Employee query: resulting-state validity evaluates module invariant `personnel_numbers_unique` |
| `test_lifecycle` `free.facts` | `references` only | the complete parent universe (decision 0.8 archives it) |
| `test_request_flow_authority` evaluation error | message in `reasons[0]` | message in `diagnostics["reasons"][0]` (canonical reasons keep codes only) |
| `test_exact_closure` | fresh `record_version` 0.4 | 0.8 |

Class (d): none. Two authority tests that only asserted `result != "ALLOW"` had passed for the
wrong reason (snapshot refusal); they now evaluate against a complete snapshot and pass for
their own reason.

Finding for Core (reported as [behavior-ir-core#29](https://github.com/kallistoteles/behavior-ir-core/issues/29)): Core 0.13.0's own `tests/fixtures/bindings/*.request.json`
do not carry complete universes, so they do not satisfy Core's complete-snapshot requirement
as plain requests; `test_binding_equivalence.py` completes them with an explicit test world.

## T030 Obsolete code and docs

Removed or rewritten: the "missing fact → `UNKNOWN_FACT`" demo in `plain_evaluation.py` (now
the snapshot refusal); the `INVALID_STATE` row and "plain evaluation assumes a valid state" in
the application skill; the `INVALID_STATE` / `CONSTRAINT_VIOLATED` and "you supply the state,
and it must be valid" statements in the authoring skill; the `facts` docstring describing
optional per-fact sections; the bare conformance case count. `evaluate_read`'s optional
`facts` stays: plain reads require complete snapshots only for Wire 0.8 modules (read record
v2), which Core enforces. No package helper became obsolete.

## T031 / T026 Full suites and gates

`cargo test --workspace` and `scripts/gates.sh` → `gates: OK`: Rust 116 passed (each run;
baseline 113 + 3 US2), pytest **480 passed, 4 skipped** (baseline 475 + 3 US2 + 2 contract tests),
mypy, models compat, determinism, ownership, terms, workflows, script tests. Log
`504-gates-final.log`, SHA-256 `165f745e5c753199f325052433bc2e32e1a5ac008a73000656e5f2cec5a81a08`.

## T027 Release check

`scripts/release-check.sh --skip-gates` (gates ran in T026) → `release-check: OK`: clean install
outside the checkout, smoke, versions (core 0.13.0, commit `1509497…`), byte identity with the
repository, skill examples, drift and equivalence (48 passed), reproducible rebuild. Wheel
`behavior-0.13.0-cp313-abi3-manylinux_2_28_x86_64.whl`, SHA-256
`1dc199f930c0e70eba852f5b9ab28d7c159141d6996567222d0d65286a62e222`. Log `504-release-check.log`,
SHA-256 `6a792c1283f7e26665a62c48812c67f714bb6ae6eb42e79daad341dedef43d86`.

## T028 Summary

- Tested revisions: ecosystem worktree on `504-core-release-0-13` from `main` `47b4231`
  (uncommitted at the time of testing); Core Release 0.13.0 `15094972191aa0ae79705df0397c551a18745399`.
- The Core repository was not modified.
- Contract changes adopted without backward compatibility (user decision 2026-10-15): required
  complete parent snapshot in `evaluate`/`evaluate_intent` (Core 017), idempotent original
  creation in backends (Core 019); record 0.8 and verifier 0.9.0 outcomes (Core 017).
- Unchanged: module bytes and semantic identities (models compat OK), replay of historical
  records and stores, determinism gate.
- Feature 503 can now rebase onto this branch and run its command-profile runtime tests without
  any workaround (SC-004); 503's `core_v2_candidates_refuse_query_facts` pinning test must then
  flip.
- Remaining limits: no ecosystem package release is published by this feature; the Core
  fixture finding above is reported as behavior-ir-core#29.
