# How verification works (features 002–004)

A behavior module is no longer only a program that can run: its central safety and correctness
properties are turned into mathematical questions automatically, proven or refuted, and kept as
evidence bound to the behavior version.

Details: `specs/002-smt-verification/`, `specs/003-fixed-scale-decimals/`, and
`specs/004-exact-arithmetic-closure/` (spec, research, contracts, quickstart,
`checklists/implementation-review.md`).

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
| General decimals and ratios | semantics / expressiveness | **Resolved by features 003–004**: fixed-scale types (`nominal(Decimal, scale=2)`) are exact on input; since 004 *all* decimal arithmetic is exact (ratios `T ÷ T` are exact dimensionless values, general decimals compute exactly), rounding happens only in an explicit `rescale` (six modes), and implicit stores are admitted only when proven representable. The verifier models exactly this; no check is inconclusive because of decimal rounding, and the runtime's 512-bit limit is enforced at admission | Declare a scale for stored quantities; use `rescale` at every lossy boundary. What remains is range: fixed-scale sums can exceed their integer digits, found as `evaluation_error` |

Fixed-scale decimals (feature 003) and exact arithmetic closure (feature 004) came first, since
they directly increase what can be proven.
Signed attestations and a hardened cache matter once results are trusted across trust
boundaries.
