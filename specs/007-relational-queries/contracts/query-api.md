# Contract: Query API

Data shapes: [data-model.md](../data-model.md). Decisions: [research.md](../research.md).

## Python DSL

```python
from behavior import (select, count, any_, all_, sum_, min_, max_, unique,
                      action, invariant, derived, requires, ensures, create, remove)

@derived
def open_order_count(customer: Customer):
    return count(select(Order).where(
        lambda o: (o.customer == customer.id) & (o.status == OrderStatus.OPEN)))

@action
def close_customer(customer: Customer):
    requires(open_order_count(customer) == 0)
    remove(customer)

@action
def place_order(customer: Customer, *, order_id: Input[Id[Order]], amount: Input[Money]):
    create(Order, id=order_id, customer=customer.id, amount=amount, status=OrderStatus.OPEN)
    ensures(sum_(select(Order).where(lambda o: o.customer == customer.id),
                 lambda o: o.amount) <= customer.credit_limit)

@invariant
def personnel_numbers_unique():            # no parameters: a module-level invariant
    return unique(select(Employee), by=lambda e: e.personnel_number)
```

- `select(T)` returns a `Query`: `.where(fn)`, `.union(q)`, `.intersection(q)`, `.difference(q)`.
- `count(q)`, `any_(q, fn)`, `all_(q, fn)`, `sum_(q, fn)`, `min_(q, fn)`, `max_(q, fn)`,
  `unique(q, by=fn)` build relational expressions.
- Iterating, `len()` or truth-testing a `Query` raises `BehaviorDefinitionError`.
- `evaluate(model, action, ..., facts={"queries": [...], "fields": [...], ...})` supplies query
  facts for plain evaluation.

## Engine (Rust, behavior-core)

```rust
pub trait EvaluationFacts {
    // feature 006: exists, used, incoming
    fn query(&self, q: &QueryRequest) -> Result<Vec<String>, FactError>;   // member ids, sorted
    fn field(&self, entity: &str, id: &str, field: &str) -> Result<Json, FactError>;
}
pub struct QueryRequest<'a> { pub entity: &'a str, pub definition: Hash, pub instance: Hash,
                              pub predicate: &'a QueryPredicate, pub captures: &'a [(String, Json)] }
```

`QueryPredicate` lets a provider evaluate membership of a candidate value (the store uses it to
answer from entity versions; plain evaluation answers from supplied facts).

## Store (behavior-store)

- **`Backend` gains** `keys_at(entity_type, position)` and `keys_by_field_at(entity_type, field,
  value, position)` (default `None`).
- **`Store::evaluate`** answers queries and member fields as of the evaluated position, using a
  field index when the predicate has an indexable equality conjunct.
- **`Store::create`** checks module invariants over the seed.
- **`Store::commit`** re-derives query and field facts at the parent (`BUNDLE_INVALID` if they
  differ).
- **New conformance cases**:
  - `query_snapshot`: query results come from the evaluated position while commits land;
  - `query_index_consistency`: type and field indexes equal a scan at every position;
  - `query_order_independence`: results and records do not depend on backend iteration order;
  - `module_invariant_preserved`: a transition breaking a module invariant is refused.

## Errors

| Code | Where |
|---|---|
| `QUERY_NOT_ALLOWED`, `NON_LOCAL_PREDICATE` | admission |
| `UNKNOWN_FACT` | evaluation error (a needed query or field fact was not supplied) |
| `INCONSISTENT_FACTS` | evaluation error (supplied query facts contradict other facts or the predicate) |
| `INVARIANT_VIOLATED` | decision reason for a failing module invariant (result `DENY`); `GENESIS_INVALID` at store creation |
