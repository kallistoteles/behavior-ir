# How verification works (features 002–007)

A behavior module is no longer only a program that can run: its central safety and correctness
properties are turned into mathematical questions automatically, proven or refuted, and kept as
evidence bound to the behavior version.

Details: `specs/002-smt-verification/`, `specs/003-fixed-scale-decimals/`, and
`specs/004-exact-arithmetic-closure/` (spec, research, contracts, quickstart,
`checklists/implementation-review.md`).

**The solver prerequisite (feature 008).** Verification runs the Z3 binary found through
`BEHAVIOR_Z3`, or else `z3` on PATH. A release names the version it was verified with
(`SUPPORTED_Z3` in `crates/behavior-verify/src/solver.rs`, currently 4.16.0).
- **Missing solver.** The error reads "verification needs the Z3 SMT solver (supported:
  4.16.0); install z3 on PATH or set BEHAVIOR_Z3". The CLI then exits 3.
- **Another version.** It runs with a warning, and the attestation records the version that ran.

## Pipeline

```text
                      Behavior IR (admitted, typed, content-addressed)
                                   │
                                   ▼
            Semantic assumptions = exactly what the runtime guarantees
   (type domains, entity constraints on incoming entities, invariants on S,
    distinct state identities, evaluation order; all decimal arithmetic exact, as at runtime:
    fixed-scale values on their grid, exact quantities and ratios as rationals, each rescale as
    its rounding function; exact values bounded at admission to what the runtime represents)
                                   │
          ┌────────────────────────┼────────────────────────┐
          ▼                        ▼                        ▼
   Safety checks (blocking)                          Warning checks
   preservation · postcondition ·                    dead_action · redundant_precondition ·
   evaluation_error                                  always_true · always_false
          │                                                 │
          ▼                                                 ▼
         SMT (pinned Z3, fixed seeds, rlimit budget)       SMT
          │                                                 │
   ┌──────┼──────────┐                              ┌───────┼────────┐
   ▼      ▼          ▼                              ▼       ▼        ▼
 UNSAT   SAT       UNKNOWN                        UNSAT    SAT     UNKNOWN
   │      │          │                              │       │        │
 PROVEN candidate  INCONCLUSIVE                 warning  witness: INCONCLUSIVE
          │                                     finding  no finding
          ▼                                              (not replayed)
   replay through the evaluator
          │
   ┌──────┴────────┐
   ▼               ▼
 reproduced     not reproduced
   │               │
 COUNTEREXAMPLE  INCONCLUSIVE
 (with decision record)
```

Every inconclusive check is a **blocking** finding: undecided is never reported as passed.

Then governance, which never changes the verification result:

```text
Verification Attestation (bound to behavior version, profile, verifier and solver versions)
          ↓
Execution Policy (content-addressed)
  + Waivers (bound to behavior hash + finding hash, Ed25519-signed by trusted keys;
             by default only inconclusive findings are waivable)
          ↓
Commit Authorization (cites policy, attestation, transition, waivers used)
```

## Entity lifecycle (feature 006)

Creations, removals, `exists` and `referenced` are encoded with three uninterpreted functions per
entity type T, over identities of the evaluated state S:
- `ex_T(x)`: x exists;
- `used_T(x)`: x was ever used, with `ex_T(x) → used_T(x)`;
- `refd_T(x)`: an entity *not bound by the action* holds a `Ref` to x, with `refd_T(x) → ex_T(x)`.

Bound state entities exist and are used, and their references point at existing entities (the store
keeps referential integrity). A creation assumes an unused identity, distinct within the
transition, and checks the new entity's constraints (including `Ref` existence via `exists'`) and
invariants on S'. `exists` on S' is `(ex_T ∨ created) ∧ ¬removed`; `referenced` on S' also sees
bound and created references. Each removal of a referenced type gets a blocking
`referential_integrity` check (part of the preservation profile): can a `Ref` to the removed
identity survive? Counterexamples carry a `facts` section (with an invented unbound referrer where
`refd_T` holds), so plain evaluation confirms them without a store. Example: in
`wire/valid/accounts.json`, `switch_and_remove` has a real counterexample — another account may
still reference the removed customer.

## Relational queries (feature 007)

The verifier need not know the whole state; it must know exactly how the transition changes what
it knows.

- **Symbolic summaries over S:** the count, sums, extrema and uniqueness of a query instance are
  uninterpreted functions of the instance's captured values. Equal captures are the same instance
  (congruence); a changed capture is an unrelated summary. The action's bound entities are known
  candidates, and every summary accounts for them.
- **Exact change:** on S', a summary is the S summary of the S' instance plus the exact effect of
  every created, removed or changed bound candidate. `count` and `sum` compose exactly; `any`/`all`
  are counts of narrowed instances (`any(q, p) ≡ count(where(q, p)) > 0`), so a precondition
  `not any(select(Employee), e → e.personnel_number == n)` and the `unique` delta rule share one
  summary; `min`/`max` compose for creations and are fresh after a removal or change.
- **Module invariants** are assumed on S and checked on S' (preservation, subject kind
  `invariant_global`), for the actions the dependency analysis says can affect them.
- **Counterexamples:** a witness query adds a few explicit unknown-member slots per queried type
  and defines every summary as a fold over the known candidates and the present slots. The model
  becomes a request with `universe` facts; the engine confirms it, and the finding reports the
  query and field facts the evaluation observed. **The slots are witnesses only:** proofs never
  see them, so nothing is proven by a finite-universe bound. No witness within the slots, or no
  confirmation, is `inconclusive`.

Example (`tests/fixtures/verify/queries.expected.json`): `add_counted_order` (count exactly `n+1`),
`hire` (uniqueness preserved) and `place_order` (credit limit) are proven; `place_order_unchecked`
and `hire_unchecked` have confirmed counterexamples; `raise_limit` (a changed capture) and
`remove_cheapest` (`min` after a removal) are inconclusive.

## What is proven — and what is not

The verifier does **not** prove "the system is correct". It proves:

> The automatically generated properties hold for the semantics that the Behavior IR expresses,
> as modelled in SMT.

Two consequences:

1. **Unstated requirements are invisible.** If "no customer may be charged twice" is not an
   invariant, constraint, state model, or postcondition, nothing can detect that it is broken.
   The central human task becomes: *What must always be true? What may change, and under which
   conditions? What does an action promise afterwards?* — rather than how to implement it.
2. **Proofs and counterexamples rest on different trust.** Counterexamples are confirmed by the
   engine itself, so they do not depend on the encoding. Proofs depend on the SMT model matching
   the engine. (Feature 002 found and fixed one such mismatch: decimal `+`/`-` also round near
   10^28. Feature 004 removed the class: arithmetic is exact in both, and admission rejects any
   exact intermediate the runtime could not represent, `EXACT_BOUND_EXCEEDED`.)

## Known limitations — three different kinds of debt

| Limitation | Kind | Consequence | Direction |
|---|---|---|---|
| Unsigned attestations | trust / provenance | Content is tamper-evident (hash), but not *who* produced it; anyone can compute a self-consistent "verified" attestation | Verifier-signed attestations, or `authorize` re-verifying (cheap with the cache) |
| Trusted cache | integrity | Keys are already content-addressed from all inputs, but the stored *result* is not verifiable: a planted entry under the right key is accepted | Authenticated entries (signature/MAC) or checkable proof certificates; keep the cache writable only by the verifier |
| Relational precision (feature 007) | precision | A changed capture gets a fresh summary, `min`/`max` after a removal or change are fresh, and a `sum`'s partial-sum range checks (made in canonical order over unknown members) are a free flag: such checks can be `inconclusive` although they hold (e.g. `raise_limit`, `remove_cheapest`, a postcondition `sum` with non-negative members) | Monotonicity lemmas for changed captures; constraint-derived bounds on members (all amounts ≥ 0 bounds every partial sum by the total). This applies only to guarantees the model states (entity constraints, preconditions); an unstated expectation belongs in the model, never in a verifier assumption |
| General decimals and ratios | semantics / expressiveness | **Resolved by features 003–004**: fixed-scale types (`nominal(Decimal, scale=2)`) are exact on input; since 004 *all* decimal arithmetic is exact (ratios `T ÷ T` are exact dimensionless values, general decimals compute exactly), rounding happens only in an explicit `rescale` (six modes), and implicit stores are admitted only when proven representable. The verifier models exactly this; no check is inconclusive because of decimal rounding, and the runtime's 512-bit limit is enforced at admission | Declare a scale for stored quantities; use `rescale` at every lossy boundary. What remains is range: fixed-scale sums can exceed their integer digits, found as `evaluation_error` |

Fixed-scale decimals (feature 003) and exact arithmetic closure (feature 004) came first, since
they directly increase what can be proven.
Signed attestations and a hardened cache matter once results are trusted across trust
boundaries.
