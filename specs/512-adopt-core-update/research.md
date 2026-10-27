# Research: Core update adoption

## R1 — Released-Core dependency

**Decision:** Require a published immutable release containing 029/030 before pin adoption. Initial discovery on 2026-10-27 returned only v0.15.0. Subsequently, a separately authorized Core 0.16.0 publication satisfied the dependency; exact provenance and successful artifact/CLI verification are in [core-release.json](evidence/core-release.json).

**Rationale:** Constitution packaging rules and FR-001 forbid branch/path substitutes in required checks. A merged PR has no released asset/manifest contract.

**Alternatives considered:** Pin merged main or build replacement assets locally: rejected. Publish Core implicitly: outside this feature and the user's separate publication boundary.

Sources: [published releases](https://github.com/kallistoteles/behavior-ir-core/releases), `core-release.json`, `scripts/check-core-pin.sh`, `scripts/fetch-core.sh`.

## R2 — Public consumer path

**Decision:** Reuse the current Python Store and complete report passthrough. Author profile 0.8 using an inert command declaration; an explicit declared read selects profile 0.7 for the ordinary fixture.

**Rationale:** `PyStore.inner` owns a persistent native Store; Python `Store._inner` retains it across evaluate/commit calls. Attestation data preserves all checks without filtering reference-world subjects. `BehaviorModule` chooses the command profile when command declarations exist. Actual execution showed a declaration-free ordinary module retains an older format, so an inert declared read is included to establish 0.7 explicitly.

**Alternatives considered:** Redesign binding caching or add a special verification facade: not justified.

Sources: `crates/behavior-py/src/lib.rs`, `python/behavior/store.py`, `python/behavior/verify.py`, `python/tests/test_query_command_profile.py`.

## R3 — Supported removal fixtures

**Decision:** Use required Ref fields. Joint/retarget patterns need exact holder association plus filtered incoming Holder count equal to one; free removal needs the actual unreferenced guard. Include unrelated holders pointing elsewhere.

**Rationale:** Existing investigation mixed examples demonstrate allowed concrete execution but omit the incoming-count premise needed for universal preservation. Do not mislabel them as positive proof fixtures. Required positives need real reference checks and allowed runtime results; unsafe variants must not prove.

**Alternatives considered:** Copy Core fixtures: forbidden. Change Ref to Id or impose a whole-population cap: weakens the intended test.

## R4 — Compatibility and evidence

**Decision:** Compare stable semantic artifacts separately from verifier artifacts and dependency-bound Safety inventory/evidence. Add new-Core compatibility tuples using existing writers; preserve established entries and published bytes.

**Rationale:** Safety inventory hashes include Core identity and evidence embeds the lock/toolchain. Those changes are expected even when policy meanings and Model/Behavior hashes stay fixed. Fresh evidence must be rebuilt and independently checked. No theorem or policy/checker-version bump follows automatically from a pin update.

**Alternatives considered:** Rewrite historical fixtures or accept old bundles under a new environment: rejected.

Sources: `scripts/semantic-snapshot.py`, `scripts/check-models-compat.py`, `crates/behavior-models/src/safety/{inventory,evidence,certify}.rs`.

## R5 — Historical receipt gate

**Decision:** Normalize only top-level receipt `core_release` in `scripts/tests/test_pinned_identity_baseline.sh`, with adversarial tests preserving all semantic fields and multiplicities.

**Rationale:** The existing script compares complete receipts to the immutable 507 baseline; a pin change alone would fail it. The existing semantic snapshot already makes this exact metadata distinction. Baseline bytes remain untouched.

**Alternatives considered:** Overwrite the baseline, omit hashes/source maps, recursively strip fields, or skip the gate: rejected.

## R6 — Growth evidence

**Decision:** Measure a fixed eight-creation window using a retained native Store at three initial populations, three repetitions, then replay. Record setup separately and full evaluate/commit timings and serialized output sizes. Freeze an improvement profile after the baseline pilot and before any new-Core candidate runs.

**Rationale:** Consumer conversion/complete record output can remain population-dependent even when Core avoids redundant validation. No whole-path constant-time claim or unmeasured performance threshold is sound. Functional limit-1000 and unrelated-update controls are separate.

**Alternatives considered:** Time an isolated validator or constant-true invariant: insufficient. Introduce a general benchmark framework or production application: unnecessary.
