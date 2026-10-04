# Authoring evaluation: rubric

Score each requirement as pass or fail. The set passes only with every requirement passing and no
violation.

| Req. | Expected | Pass when |
|---|---|---|
| 1 | expressible | an entity constraint on `Order.amount >= 0` |
| 2 | expressible | a precondition using `sum_` over the customer's open and blocked orders (`where` plus `union`, or a status filter) plus the input, `<=` the credit limit |
| 3 | expressible | a precondition `count(... open ...) == 0` (directly or through a derived value) and `remove(customer)` |
| 4 | expressible | a module invariant `unique(select(Employee), by=...)` |
| 5 | expressible | a precondition `not_(any_(select(Employee), lambda e: e.personnel_number == number))` |
| 6 | expressible | preconditions on role and limit (limit through `Context` or a state parameter), plus `set_` of `approved_by` |
| 7 | expressible | `set_(amount, amount - discount)` with the constraint or a precondition keeping it non-negative |
| 8 | expressible | a precondition `limit >= customer.credit_limit` |
| 9 | expressible | `rescale(customer.credit_limit / 3, Money, Rounding.HALF_EVEN)` (or equivalent exact expression); no float |
| 10 | expressible | a precondition on status plus `set_` |
| 11 | **gap** | a `SEMANTIC_GAPS.md` entry naming grouped invariants; a precondition on the known action is fine only if the entry names it |
| 12 | **gap** | an entry naming bulk effects (and cross-entity filters if region is on Customer) |
| 13 | **gap** | an entry naming bulk effects |
| 14 | **gap** | an entry naming ordering |
| 15 | expressible | `@read`s listed in `reads=[...]`: a value read of the open total through a derived value, and an entity projection of the name and a derived Bool, without `credit_limit`; no action without effects |

## Violations (any one fails the set)

- An import from anything but `behavior`, or of a name not in the release's public API.
- Hand-written wire JSON or IR.
- A business rule of 1–10 implemented only in host code.
- A gap requirement silently approximated with no gap entry.
- A gap entry missing any of the fixed fields.
- `model.py` not admitted, or a check in `checks.py` failing.
