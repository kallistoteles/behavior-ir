# How verification works (features 002–003)

A behavior module is no longer only a program that can run: its central safety and correctness
properties are turned into mathematical questions automatically, proven or refuted, and kept as
evidence bound to the behavior version.

Details: `specs/002-smt-verification/` and `specs/003-fixed-scale-decimals/` (spec, research, contracts, quickstart,
`checklists/implementation-review.md`).

## Pipeline

```text
                      Behavior IR (admitted, typed, content-addressed)
                                   │
                                   ▼
            Semantic assumptions = exactly what the runtime guarantees
   (type domains, entity constraints on incoming entities, invariants on S,
    distinct state identities, evaluation order; fixed-scale values as integers on their grid,
    exact quantities as rationals, each rescale as its rounding function; general decimals
    with rounding intervals)
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
   10^28; `tests/fixtures/verify/sum_rounding.json` guards it.)

## Known limitations — three different kinds of debt

| Limitation | Kind | Consequence | Direction |
|---|---|---|---|
| Unsigned attestations | trust / provenance | Content is tamper-evident (hash), but not *who* produced it; anyone can compute a self-consistent "verified" attestation | Verifier-signed attestations, or `authorize` re-verifying (cheap with the cache) |
| Trusted cache | integrity | Keys are already content-addressed from all inputs, but the stored *result* is not verifiable: a planted entry under the right key is accepted | Authenticated entries (signature/MAC) or checkable proof certificates; keep the cache writable only by the verifier |
| General decimals (no fixed scale) | semantics / expressiveness | **Addressed for money by feature 003**: fixed-scale types (`nominal(Decimal, scale=2)`) are exact on input and in lossless arithmetic, every narrowing is an explicit `rescale` with one of six rounding modes, and the verifier reasons about them exactly (`amount <= remaining(project)` is proven). General decimals keep the interval model and may still be inconclusive | Use fixed-scale types for quantities with a known number of decimals. Remaining: the ratio `T ÷ T` is a general decimal (28-digit rounding) — an exact ratio type is a follow-up; exact quantities have a 512-bit limit the verifier does not model |

Fixed-scale decimals (feature 003) came first, since they directly increase what can be proven.
Signed attestations and a hardened cache matter once results are trusted across trust
boundaries.
