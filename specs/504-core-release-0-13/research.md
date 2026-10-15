# Research: Core Release 0.13.0 alignment

All facts are read from tag `v0.13.0` (`1509497…`) in the Core repository and from its GitHub
release (published 2026-10-14T22:22:38Z).

## R1. Release identity and assets

**Decision**: pin `version 0.13.0`, `tag v0.13.0`, `commit 15094972191aa0ae79705df0397c551a18745399`,
CLI `behavior-0.13.0-x86_64-linux-musl` sha256
`b6e0d94a45110c0030a524683d6eb78d1fb266fd14329f596e430f0985a59ad0`, conformance
`behavior-conformance-0.13.0.tar.gz` sha256
`f7795af93736cb5402ca1be33bd1d0f19e25d0203fac68c5ad09391057f06bbb`, taken from the published
`release-manifest.json` (sha256 `d66d64ca…`) and `SHA256SUMS`, which agree.
**Rationale**: the pin derives from the published manifest only (feature 011, 500).
**Alternatives**: pinning untagged `main` — rejected, breaks `core_pin.v1` and release checks.

## R2. Public Rust surface

**Correction (T015)**: the Rust signatures are indeed unchanged and the binding compiles
unchanged, but two *behavioral* contracts behind those signatures changed (R9, R10); the
conclusion "no binding change" was wrong for the Python API.

**Original decision**: expect no binding signature changes.
**Rationale**: `release-manifest.json` `public_surface.items_sha256` is
`6cc78f41…` for both 0.12.0 and 0.13.0, and `api/engine-surface.txt` has no diff between the tags.
**Alternatives**: none needed; a compile error would contradict this and is a stop condition.

## R3. Reported versions

**Decision**: expected `versions()` changes: engine `0.13.0`, verifier `0.8.0 → 0.9.0`, records
`0.4…0.7 → 0.4…0.8`, accepted Wire IR `0.1…0.8 → 0.4…0.8` (Core 020: accurate fresh-admission
metadata; 0.1–0.3 were never admitted, the legacy Python DSL emits 0.4; found while writing T003,
the first version of this note wrongly said unchanged). Unchanged: historical `wire_ir` `0.1…0.7`,
read records v1/v2, store documents, command stream/occurrence domains, solver z3 4.16.0.
**Rationale**: diff of the 0.12.0 and 0.13.0 release manifests.

## R4. Fresh records

**Decision**: fresh evaluations under the current path write record `0.8` with a
`validation_profile` (`DecisionRecord::with_current_profile`, `eval.rs:294`); legacy-profile
modules are normalized into it. Module bytes and semantic identities do not change. Expected
test impact: assertions on fresh `record_version` and on fresh record hashes. Each change must
cite Core feature 017 (resulting-state validity). Historical records replay with their own
version (Core: "existing historical records retain their recorded profiles, bytes").
**Alternatives**: regenerate goldens wholesale — rejected (FR-006).

## R5. Governance fixtures for verifier 0.9.0

**Decision**: tests that perform fresh authenticated verification switch from
`governance-v2/policy-verifier-0.8.json` / `evidence-policy-verifier-0.8.json` to the
`-verifier-0.9.json` fixtures shipped in the 0.13.0 conformance archive. Tests of historical
authorizations keep the 0.8 fixtures and must still verify/replay as historical.
**Rationale**: Core requires deliberate policy selection; nothing is upgraded implicitly (FR-007).

## R6. Disk budget

**Decision**: build the worktree with `CARGO_TARGET_DIR` pointed at the main checkout's
`target/` so unchanged dependencies are reused; run focused tests before the full gate; no
second full target directory.
**Rationale**: ~9 GB free until the new disk arrives; a fresh target is ~4 GB.

## R7. The 503 blocker (US2)

**Decision**: regression = minimal command-profile module, v2 genesis with policy `none`, one
action with precondition `¬any(select(T), _ → true)`: allowed+commit, refused, and altered query
evidence refused at commit. Same three cases through the Python binding.
**Rationale**: reproduces 503 R13 without Approval; fixed in Core by 023 (`4f44fb8`), included in
0.13.0. Ordinary `Facts` still rejects `result_hash`; the test asserts only through bundles.

## R8. Ecosystem version

**Decision**: `0.12.0 → 0.13.0` (minor: new record version and verifier outcomes, per
`docs/versioning.md`), keeping the release number aligned with the bundled Core as since 500.
Core 0.12.1 was never pinned; its patch changes arrive with 0.13.0.

## R9. Complete parent snapshots for fresh evaluation (found in T015)

**Finding**: Core 017 validates every fresh action evaluation (`ActionExecution::Current`,
including plain `evaluate` of legacy modules and `evaluate_intent`) with
`validate_complete_snapshot`: `facts.universe` must list every declared entity type (empty
types as `members: []`), consistent with the bound `state`. Otherwise the result is
`INVALID_INPUT / INVALID_STATE_SNAPSHOT` ("a complete universe of every declared entity type
is required"). Core docs (invocation.md): "Bound values are not an implicit complete universe";
"The engine does not infer a universe from `targets` or repair omitted facts". Store paths are
unaffected (the Store supplies the snapshot).
**Decision** (user, no backward compatibility): `facts` becomes a required keyword of
`behavior.evaluate` and `behavior.evaluate_intent` (the latter gains it; Core's `ActionIntent`
accepts host facts). Tests and examples build the universe explicitly from their own data; no
binding helper infers it.
**Alternatives**: infer the universe from `state` — rejected as unsound by Core 017 itself;
keep `facts` optional — rejected, the signature would advertise a call that always refuses.

## R10. Store creation contract (found in T015)

**Finding**: Core 019 adds 14 backend conformance cases (31 → 45). `Backend.create` must be an
idempotent no-op when the backend already holds the identical original creation (same genesis,
same position-0 versions, references and state anchor), and must refuse a different one; Core's
`InMemoryBackend::create` is the reference (`memory.rs`). The Python test backends and the
`custom_backend.py` skill example assert "already created" instead.
**Decision**: implement the contract directly in each Python backend (compare genesis, seed
versions at position 0, seed references and the original head's state anchor; no assignment on
an equivalent retry); no protocol method is added.
