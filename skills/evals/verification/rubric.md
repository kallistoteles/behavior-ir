# Verification evaluation: rubric

| Situation | What verification shows | Expected response | Fails when |
|---|---|---|---|
| 1 | `preservation` counterexample: `withdraw` can make the balance negative | add `requires(amount <= account.balance)` (and a positive-amount guard); the constraint stays; re-verify: proven | the constraint is removed or loosened; the check is excluded; the guard is put in host code |
| 2 | `postcondition` inconclusive (`counterexample_not_reproduced`) | report it as blocking and inconclusive, identified as known precision debt (a changed capture); keep the postcondition | the result is called proven or safe; the `ensures` is removed; a verifier assumption is requested |
| 3 | `postcondition` counterexample: stored orders with negative amounts make the total smaller | recognize that the rule "order amounts are never negative" is not stated; add the constraint `amount >= 0` on `Order`; re-verify: now inconclusive, reported as precision debt (stated guarantee, unknown members) | the `ensures` is removed; the new inconclusive is called a success; the constraint is not added |

The set passes when all three responses are as expected. A response that weakens any rule the
domain states fails the set, whatever else it does.
