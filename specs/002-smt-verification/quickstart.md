# Quickstart: SMT Verification of Behavior Modules

Validation guide for feature 002. Contracts: [verification-report.md](contracts/verification-report.md),
[governance.md](contracts/governance.md), [engine-api.md](contracts/engine-api.md).

## Prerequisites

`nix develop` (the dev shell now also provides the pinned `z3` binary), then `maturin develop`.

## 1. Gates

```bash
cargo fmt --check && cargo clippy --all-targets -- -D warnings
cargo test --workspace            # includes verifier, governance, and 001 regression tests
pytest python/tests
mypy
scripts/determinism-check.sh      # now also runs verify twice per fixture and compares bytes
cargo test --release -p behavior-verify --test perf -- --ignored --nocapture   # SC-003, SC-004
```

## 2. Invariant preservation (US1)

```bash
python -m examples.tryout.dump > /tmp/purchase.json
behavior verify /tmp/purchase.json; echo "exit=$?"
```

Expected: exit 1; a blocking `preservation` finding for `approve` × `within_budget`, whose
counterexample evaluates to `DENY` because of `within_budget`. After adding the precondition
`project.spent + purchase.amount <= project.budget` to `approve`
(`tests/fixtures/verify/purchase_fixed.json`): exit 0, the pair is `proven`. The same check
written as `purchase.amount <= remaining(project)` (`purchase_remaining.json`) is
`inconclusive` (`counterexample_not_reproduced`): decimal subtraction and addition may round
differently for values near 10^28, so the rounding interval crosses the budget boundary.

## 3. Postconditions and evaluation errors (US3)

`behavior verify tests/fixtures/wire/valid/project_margin.json`: a blocking `evaluation_error`
finding for `flag_project_unguarded` at the division in `margin`, counterexample
`revenue = 0`, confirmed as `ERROR`; `flag_project` has no division-by-zero finding. Both
actions have a confirmed `numeric overflow` finding at the same division (a tiny `revenue` with
a huge `cost`), because the module has no entity constraints bounding the fields; the
subtraction cannot overflow (request decimals have at most 28 digits).

## 4. Dead actions and vacuous rules (US4)

`tests/fixtures/verify/dead_and_vacuous.json`: a `dead_action` warning (preconditions
`amount > 100` and `amount < 50`) and an `always_true` warning; the result is still `verified`
if there are no blocking findings.

## 5. Decimal intervals (FR-013a)

`tests/fixtures/verify/rounding.json`: a rule comparing `a / b` with a bound it can only miss by
less than the rounding bound is `inconclusive`; the same rule with a clear margin is `proven`.
`tests/fixtures/verify/sum_rounding.json`: `w := v + amount; ensures w > v` (with `amount > 0`)
has a confirmed counterexample, because the engine rounds sums that need more than 28 digits.

## 6. Entity constraints and binding (runtime changes)

- A context employee with a negative approval limit, under the constraint
  `approval_limit >= 0`, evaluates to `INVALID_CONTEXT`; verification never produces such a
  counterexample.
- `transfer(from, to)` with the same account twice evaluates to `INVALID_BINDING`
  (`STATE_ALIAS_NOT_ALLOWED`).
- Feature 001 golden records differ only by `record_version "0.2"` and `entity`/`id` in changes;
  all 001 behavior versions and hash vectors are unchanged.

## 7. Caching and determinism (US2)

Run `behavior verify --cache .behavior/verify-cache` twice: the second run marks every check
`cached` and prints the same attestation hash. Change one action and re-run: only its checks are
recomputed.

## 8. Governance (US2)

With the fixtures in `tests/fixtures/governance/`:

- `behavior authorize … --policy require_verified.json` for an unverified module → exit 1,
  reason `not_verified`.
- A waiver for an inconclusive finding (`tests/fixtures/verify/purchase_remaining.json` has one;
  the record comes from `behavior eval … governance/approve_request.json`), signed with the test
  key trusted by
  `verified_or_waived.json` → exit 0, `waivers_used` records the waiver hash, key id, and
  signature; the attestation still says `not_verified`.
- The same waiver after a semantic change to the behavior, with an untrusted key, expired at
  `--now`, or for a forbidden finding kind → exit 1 with the matching reason.
- `behavior waiver-hash` of a waiver is unchanged when a second signature is added.
