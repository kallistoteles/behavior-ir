# Implementation Review: SMT Verification of Behavior Modules

**Purpose**: Review of the implementation against `PRINCIPLES.md` and the constitution (task T051)
**Date**: 2026-09-26
**Scope**: `crates/behavior-verify/`, changes in `crates/behavior-core/`, `crates/behavior-cli/`,
`crates/behavior-py/`, `python/behavior/`, `tests/fixtures/{verify,governance}/`, `scripts/`

Each item was checked in the code or by a test. Evidence is noted.

## PRINCIPLES.md

- [x] §2 Identity is semantic; parameter names are only bindings: the evaluator rejects two
  state parameters bound to the same `(entity, id)` with `INVALID_BINDING` /
  `STATE_ALIAS_NOT_ALLOWED` (`evaluate_002.rs`, fixture `requests/002/alias`); ΔS
  entries address cells `{entity, id, field}`; the encoder asserts distinct ids for state
  parameters of the same entity type, exactly the runtime check (`encode.rs`,
  `ActionEncoding::build`); counterexample ids are renamed per entity type so equality between
  comparable identities is kept (`preservation.rs` checks no aliasing in counterexamples).
- [x] §6 Content addressing: finding hashes cover kind and cited content hashes only (plus the
  parameter *position*, never a name); check keys cover every content hash the outcome depends
  on plus profile, verifier, and solver versions; governance documents are hashed over canonical
  JSON under their own tags (`hashing.rs`, `hashing.rs` tests).
- [x] §10 The verifier assumes exactly what the runtime guarantees: type domains (i64, enum
  indices, decimals of at most 28 digits), entity constraints on every incoming entity, state
  invariants on S, distinct state identities, the runtime's evaluation order with
  short-circuiting, and a rounding interval wherever the engine rounds. Nothing is assumed that
  the runtime does not check.
  - **Soundness fix found during implementation**: research R5 modelled only decimal `*` and `/`
    as rounding. The engine also rounds `+` and `-` when the result needs more than 96 bits of
    mantissa (`1e27 + 1e-28 = 1e27`). With exact sums the verifier would have *proven*
    `w := v + amount; ensures w > v`, which the engine violates. Sums now get the interval too
    (exact below magnitude 7, where it cannot round); `sum_rounding.json` has the confirmed
    counterexample as a regression test.
  - The same expression over the same bindings is encoded once (one rounded value), which is
    sound because evaluation is deterministic; it keeps invariant-shaped preconditions provable.
- [x] §10 Authority requires evidence: a waiver has no reviewer field; it counts only with a
  detached Ed25519 signature over `"behavior.waiver.v1" ‖ 0x00 ‖ hash bytes` by a key the policy
  trusts with a role required for the finding kind (`governance.rs`; `governance.rs` tests:
  untrusted key, corrupted signature, expired, other behavior version, other finding, forbidden
  kind). Test keys are marked test-only in `tests/fixtures/governance/keys.json` and README.
- [x] §10 Never a wrong proof: every satisfiable answer is replayed through the evaluator before
  it is reported as a counterexample; unreproduced models are `inconclusive`
  (`counterexample_not_reproduced`); inconclusive checks are blocking findings; wall-clock
  results are marked `reproducible: false` and never cached (`cache.rs` tests).
- [x] §11 Constraints vs invariants: entity constraints are assumed for incoming entities of
  every role and checked on S' for state parameters; state invariants are assumed on S and
  checked on S' (`constraint_break` fixture: `transfer` breaks `non_negative_balance` for
  `from_`, preserves it for `to`).

## Constitution

- [x] I Deterministic core: the solver runs with fixed seeds and a resource budget (`rlimit`);
  attestations and authorizations are byte-identical across runs
  (`scripts/determinism-check.sh` runs `verify` twice on every fixture and `authorize` twice on a
  waived case).
- [x] IV Reproducibility: counterexamples carry their decision records; authorizations cite the
  policy hash, attestation hash, and transition hash; `authorize` replays the record.
- [x] V Auditability: attestations, waivers, signatures, policies, and authorizations are
  canonical, content-addressed JSON.
- [ ] III Test-first: mostly followed, with three deviations (below).
- [x] Gates (T050): fmt, clippy `-D warnings`, 96 Rust tests, 60 Python tests, mypy,
  determinism check, and both ignored performance suites pass. SC-003: the three example
  modules verify in about 1 s (limit 10 s). SC-004: after changing one of 50 actions, a warm
  re-verification takes about 0.34 s versus 12.4 s cold (10 of 500 checks recomputed).

## Deviations from tasks.md

1. **Test-first** was broken three times: the encoder (T022) before its test (T012); the cache
   (T036) before its test (T032, written right after); `governance.rs` (T037) before
   `tests/governance.rs` (T034). The tests were written immediately afterwards and cover the
   listed scenarios.
2. **FR-009 read literally**: *every* inconclusive check, including warning checks (dead action,
   redundancy, vacuity), is a blocking `inconclusive` finding. The earlier idea of letting
   inconclusive warning checks stay non-blocking was dropped. Worth revisiting: blocking a
   version because a *warning* is undecided may be stricter than intended.
3. **Fixed demo precondition** (T024, quickstart §2): `purchase_fixed` now uses
   `project.spent + purchase.amount <= project.budget`. The planned
   `purchase.amount <= remaining(project)` is `inconclusive` under the sound rounding model and
   is kept as `purchase_remaining.json`. It may even be genuinely violable for a huge negative
   `spent`. Quickstart §2 is updated.
4. **project_margin** (T040): `flag_project` has no division-by-zero finding, as planned, but both
   actions have a confirmed *overflow* at the division (tiny `revenue`, huge `cost`; the module
   has no entity constraints). The subtraction cannot overflow within the request domain.
   Quickstart §3 is updated.
5. **Cache test** (T032): "only changed checks recomputed" uses the pair `purchase_fixed` →
   `unchanged_entity` (the same `approve` plus a new `reject`) instead of editing
   `purchase.json`; the perf test changes one of 50 actions.
6. **Governance tests** use `purchase_remaining.json` (a natural inconclusive) instead of a tiny
   `rlimit`: with shared rounded terms, `purchase_fixed` is decided during preprocessing even at
   `rlimit = 1`.
7. **CLI `--cache`** has no default directory; `cache::DEFAULT_DIR` (`.behavior/verify-cache`) is
   provided and git-ignored. An implicit default would write into the working directory
   unasked.
8. **Additions**: `behavior sign-waiver` / `governance::sign_waiver` / Python `sign_waiver` (needed
   to produce signatures in tests and by reviewers); `authorize` also refuses records from
   another behavior version (`behavior_mismatch`), records that do not replay
   (`record_not_reproducible`), and records that are not `ALLOW` (`nothing_to_commit`); tampered
   attestations are invalid input. The `Solver` trait takes a `Query` (script, symbols,
   `rlimit`) rather than a string. `solver unavailable` is exit code 3. Documented in the
   contracts.
9. **Evaluation errors** are checked in preconditions, effects, postconditions, and rules on S';
   errors inside incoming entity constraints and invariants on S are not (they bound validity
   rather than being reached from a valid state).
10. **Redundancy** is skipped for actions proven dead only when `dead_action` is in the profile;
    with `redundancy` alone, preconditions after a contradiction are reported as vacuously
    redundant.
11. `from_` instead of `from` as a parameter name in the constraints fixture (`from` is a Python
    keyword).

## Open risks (not blocking this feature, need a decision)

- **Attestations are not signed.** `authorize` checks an attestation's hash against its content,
  which makes it tamper-evident but not authentic: anyone can compute a self-consistent
  "verified" attestation. Hosts must produce attestations themselves (or re-run `verify`, cheap
  with the cache). A verifier signature, or `authorize` re-verifying, would close this.
- **The cache is trusted.** Anyone who can write to the cache directory can plant "proven"
  results under the right key. It should live in a directory only the verifier writes.
- **Decimal proofs through derived differences are often inconclusive**, because the interval
  model cannot track decimal scale. Scale-aware decimals (for example a fixed-scale `Money`
  nominal enforced at the boundary) would make money arithmetic exact and provable.
