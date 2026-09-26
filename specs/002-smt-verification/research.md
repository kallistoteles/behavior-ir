# Research: SMT Verification of Behavior Modules

Decisions taken during `/speckit-plan` on 2026-09-25. Each entry: decision, rationale,
alternatives considered. A probe with the pinned solver (Z3 4.16.0 from the flake's nixpkgs)
confirmed: two runs of the same query give byte-identical output, models use exact rationals
(`(/ 1.0 100.0)`), and consumed resources are reported (`(:rlimit 722)`).

## R1. Solver and how the engine talks to it

- **Decision**: Z3, pinned through the Nix flake, driven as a subprocess with **SMT-LIB 2**
  text: the verifier writes one query per check to the solver's stdin and reads `sat` / `unsat`
  / `unknown` and `get-value` results from stdout. The solver sits behind a small `Solver` trait
  (`check(query) -> SolverAnswer`) with one implementation, `Z3Process`.
- **Rationale**: SMT-LIB text is solver-neutral (cvc5 could replace Z3), easy to audit (each
  query can be stored and hashed next to its result), and avoids native bindings and a C build
  (bindgen, libclang) in the Rust and Python builds. Process start-up costs a few milliseconds per
  check, well inside SC-003.
- **Alternatives**: the `z3` crate (in-process, faster per check, but a native dependency in every
  build including the Python extension, and tied to one solver); a pure-Rust solver (no mature
  option for nonlinear real arithmetic).

## R2. Deterministic budgets

- **Decision**: the "time budget" of the spec is a **resource budget**: Z3's `rlimit` (a
  deterministic count of solver steps), set per check and recorded in the profile and report. A
  generous wall-clock guard stops runaway processes; a check stopped by the wall-clock guard is
  reported as inconclusive with reason `wall_clock_guard`, and such results are **never cached**
  and are flagged as not reproducible. The solver runs with a fixed seed (`smt.random_seed 0`)
  and one thread.
- **Rationale**: a wall-clock timeout makes the result depend on machine speed, which would break
  FR-012 (byte-identical reports across machines). `rlimit` gives the same `unknown` on every
  machine for the same query and solver version.
- **Alternatives**: wall-clock timeouts only (non-deterministic reports).

## R3. Encoding the semantic IR

- **Decision**: one SMT variable per value the check can observe:
  - Every field of every state entity on S; separate variables for S' are not needed, because
    effects are functions of S (S' fields are defined as terms).
  - Every field of input and context entities, and every scalar input or context parameter.
  - Types: `Bool` → Bool; `Int` → Int bounded to the signed 64-bit range; `Decimal` → Real bounded
    by the engine's decimal range; `String` and `Id<E>` → SMT strings; enums → Int in `[0, n)`;
    nominal types → their underlying sort; `Option<T>` → a Bool `is_some` plus a value of `T`.
  - Derived values → defined terms, instantiated per argument binding and per state (S or S').
- **Rationale**: a direct, auditable translation of data-model.md's semantics; the query shows
  exactly what the engine assumes.

## R4. What the verifier assumes (FR-026)

- **Decision**: every query asserts exactly the runtime's pre-evaluation guarantees:
  1. type domains of all values (ranges, enum values, option structure);
  2. entity constraints on every state, input, and context entity;
  3. state invariants on every state entity on S;
  4. pairwise-distinct identities of state parameters of the same entity type (FR-027).
  Nothing else: no knowledge of real data, no invariants on input or context.
- **Rationale**: the principle "the verifier may assume exactly what the runtime guarantees".
  Each assumption has a matching runtime check in feature 001's evaluator (extended here, R11).

## R5. Decimal values: representability and rounding (FR-013, FR-013a)

- **Decision**:
  - **Proofs** use the decimal range as a bounded interval of reals. Proving over this superset of
    representable decimals is sound.
  - **Counterexample search** first restricts inputs to "nice" decimals (at most 4 fractional
    digits, magnitude ≤ 10^15) so counterexamples are readable; if that is unsatisfiable, it
    retries without the restriction. A counterexample that the engine cannot represent or does
    not reproduce is reported as inconclusive (`counterexample_not_reproduced`), never as a
    finding and never as proven.
  - **Rounding**: addition and subtraction of decimals are exact in the engine and are modelled
    exactly. Multiplication and division may round: each is modelled as a fresh value `r` with
    `|r − exact| ≤ max(10^-28, |exact| · 10^-27)` (at most one unit in the last place for
    28-significant-digit decimals). Bounds compose automatically because every rounding
    operation introduces its own bounded variable. Division by a variable is encoded as
    `exact · divisor = dividend` under the guard `divisor ≠ 0`.
  - A check whose `unsat` depends on nothing but these bounds is proven; `sat` produces a
    candidate counterexample confirmed by evaluation; if the solver answers `unknown`, or the
    candidate is not reproduced, the check is inconclusive.
- **Rationale**: the user's decision (clarification Q2): exact value plus a runtime-derived
  bound, proven only for the whole interval. One bounded variable per rounding operation is the
  simplest sound composition.
- **Alternatives**: bit-precise modelling of `rust_decimal` (heavy); ignoring rounding (unsound).

## R6. Checks and their queries

For action `a` with assumptions `A` (R4), preconditions `p1…pn`, effects `E`, postconditions
`q1…qm`, and state invariants and entity constraints `R` on its state entities:

| Check | Query (satisfiable means a finding) | Severity |
|-------|-------------------------------------|----------|
| invariant / constraint preservation | `A ∧ p1…pn ∧ ¬r(S')` for each `r ∈ R` | blocking |
| postcondition | `A ∧ p1…pn ∧ ¬qj(S')` for each `j` | blocking |
| evaluation error | `A ∧ guard(e) ∧ error(e)` for each division and each arithmetic operation, where `guard(e)` is the path condition under which the engine evaluates `e` (earlier preconditions true, short-circuit context of `and`/`or`) | blocking |
| dead action | `A ∧ p1…pn` **unsatisfiable** | warning |
| redundant precondition | `A ∧ p1…pi-1 ∧ ¬pi` **unsatisfiable** | warning |
| always-true / always-false rule | `A_E ∧ ¬rule` / `A_E ∧ rule` unsatisfiable, where `A_E` is the rule parameters' type domains and entity constraints | warning |

Postconditions and S' invariants are only meaningful when evaluation reaches them, so their
queries include `p1…pn`; errors in effects are covered by the evaluation-error check.

## R7. Counterexamples and confirmation (FR-008)

- **Decision**: a `sat` model is converted to an EvaluationRequest (state entities with ids from
  the model, input, context, `data_version` `"verification"`), evaluated by the engine, and the
  finding is reported only if the decision record shows the predicted failure (the named
  invariant or constraint on S', the named postcondition, or `ERROR` at the named expression).
  The record is embedded in the finding.
- **Rationale**: a modelling mistake can never produce a false blocking finding, and every
  finding is reproducible by anyone with the behavior version.

## R8. Identities, caching, and verifier version

- **Decision**:
  - **Finding hash** = `SHA-256("behavior.finding.v1" ‖ 0x00 ‖ kind ‖ cited hashes)`; independent of
    counterexample values and messages (FR-019).
  - **Check key** = `SHA-256("behavior.check.v1" ‖ 0x00 ‖ check kind ‖ action hash ‖ hashes of the
    invariants, constraints, and derived values it involves ‖ entity hashes ‖ profile hash ‖
    verifier version ‖ solver version)`.
  - The **cache** is a directory (default `.behavior/verify-cache/`, configurable) with one
    canonical JSON file per check key. Results stopped by the wall-clock guard are not stored.
- **Rationale**: everything a check depends on is in its key, so a change anywhere relevant
  produces a new key and nothing else is recomputed (SC-004).

## R9. Governance objects: canonical form and hashes

- **Decision**: verification attestations, waivers, policies, and commit authorizations are
  documents; their canonical form is the engine's canonical JSON (sorted keys, no floats), and
  their hash is `SHA-256(tag ‖ 0x00 ‖ canonical bytes)` with the tags `behavior.verification.v1`,
  `behavior.waiver.v1`, `behavior.policy.v1`, and `behavior.authorization.v1`. Signatures and
  other provenance are kept outside the hashed content.
- **Rationale**: these objects are data about behavior, not behavior; their JSON form is their
  meaning. Tags version the encoding so it can change without ambiguity.
- **Alternatives**: a binary encoding like the behavior hashing contract (more work, no benefit
  for flat documents).

## R10. Signatures (FR-020a, FR-020b)

- **Decision**: Ed25519 via the `ed25519-dalek` crate. A signed attestation is
  `{waiver_hash, key_id, signature}`, where `key_id = "ed25519:" + hex(public key)` and the
  signature covers `"behavior.waiver.v1" ‖ 0x00 ‖ waiver hash bytes`. The policy lists trusted
  keys with principal and roles. The engine only verifies; creating and storing private keys is
  outside the engine (tests use fixed test keys).
- **Rationale**: small, audited, deterministic signatures; detached so one waiver can carry
  several signatures (clarification Q5).

## R11. Changes to feature 001's engine

- **Decision**:
  - **Entity constraints**: new construct `constraints` in wire IR (`ir_version "0.2"`; `"0.1"`
    stays accepted and means no constraints), semantic item `ConstraintItem`, hash tag
    `behavior.constraint.v1`, module entry kind 7. Modules without constraints keep their 0.1
    hashes, so feature 001's versions are unchanged. DSL: `@constraint` and
    `BehaviorModule(constraints=[...])`.
  - **Evaluation order** becomes: shape and type check → binding check (`INVALID_BINDING`,
    `STATE_ALIAS_NOT_ALLOWED`) → entity constraints on every incoming entity (`INVALID_STATE`,
    `INVALID_INPUT`, `INVALID_CONTEXT`) → state invariants on S → preconditions → effects →
    postconditions, state invariants, and entity constraints on S' state entities (`DENY`).
  - **Change sets** gain `entity` and `id` per entry (FR-028); records get `record_version "0.2"`.
    Feature 001's golden records are regenerated and reviewed (only the added fields change).
- **Rationale**: the shared runtime/verifier contract (clarifications Q1, Q4) needs these checks
  in the runtime before the verifier may assume them.

## R12. Policy evaluation

- **Decision**: `authorize(policy, behavior version, decision record, attestation?, waivers,
  signatures, now) -> CommitAuthorization`. Steps: the attestation must match the behavior hash
  and a required profile; if it is verified and the policy requires verified or
  verified-or-waived → allow; otherwise every blocking finding must be covered by an accepted
  waiver (bindings match, not expired at `now`, kind waivable, signed by a trusted key with a
  required role) and the policy must allow waivers. `now` is supplied by the caller and recorded.
- **Rationale**: the smallest deterministic evaluator that implements FR-022–FR-024; the
  declarative policy document keeps the door open for a policy IR later.

## R13. Surfaces

- **Decision**: new crate `behavior-verify` (depends on `behavior-core`; owns encoding, solver
  process, checks, cache, attestations, waivers, policy). CLI: `behavior verify`,
  `behavior authorize`, `behavior waiver-hash`. Python: `verify(model, ...)`,
  `authorize(...)`, native values across PyO3 as in feature 001.
- **Rationale**: keeps the solver dependency out of `behavior-core`, which stays usable without it.
