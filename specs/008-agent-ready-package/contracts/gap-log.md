# Contract: Semantic Gap Log

A consumer project keeps `SEMANTIC_GAPS.md` at its root.

```markdown
# Semantic gaps

## GAP-001: At most three open orders per customer

- **Date**: 2026-10-02
- **Behavior release**: 0.8.0
- **Requirement**: A customer may have at most three open orders at any time.
- **Why inexpressible**: needs a grouped module invariant (a count per customer over all
  customers); module invariants are closed and queries cannot group. Closest construct: a
  precondition `count(orders of customer, open) < 3` on `place_order`, which covers only that
  action.
- **What was done instead**: the precondition on `place_order`; the risk is that other actions
  that open orders are not covered.
- **Severity**: degraded
- **Evidence**: a per-customer invariant draft (`@invariant def f(c: Customer)` counting that
  customer's open orders) was rejected with `QUERY_NOT_ALLOWED`, because queries are not allowed
  in per-entity invariants.
```

## Rules

- IDs increase and are never reused. Entries are never deleted; a resolved gap gets a
  `- **Resolved**: <release> …` line.
- `What was done instead` is `nothing` unless a host-side interim was explicitly reviewed. It is
  never an unmentioned workaround.
- The fields are fixed, so entries from several consumer repositories can be collected
  mechanically.
