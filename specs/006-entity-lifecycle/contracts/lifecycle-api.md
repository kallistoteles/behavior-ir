# Contract: Lifecycle API

Data shapes: [data-model.md](../data-model.md). Decisions: [research.md](../research.md).

## Python DSL

```python
from behavior import Ref, Input, Id, action, create, remove, exists, referenced, requires, entity, field

@entity
class Customer:
    name = field(str)

@entity
class Account:
    owner = field(Ref[Customer])          # Id[Customer] + constraint exists(owner)
    balance = field(Money)

@action
def open_account(owner: Customer, *, account_id: Input[Id[Account]], initial: Input[Money]):
    requires(initial >= Money(Decimal("0")))
    create(Account, id=account_id, owner=owner.id, balance=initial)

@action
def close_account(account: Account):
    requires(account.balance == Money(Decimal("0")))
    remove(account)

@action
def remove_customer(customer: Customer):
    requires(not_(referenced(customer.id)))
    remove(customer)
```

- **`create` forms:** it takes keyword arguments for every field; `id` is required and must be
  `Id[T]`.
- **`remove` form:** it takes a state parameter.
- **Predicates:** `exists` and `referenced` take an `Id[T]` expression (`exists` also takes an
  `Option[Id[T]]`).

## Engine (Rust, behavior-core)

```rust
/// Current-state facts (exists, incoming) and history facts (used) of an evaluation snapshot.
pub trait EvaluationFacts {
    fn exists(&self, entity: &str, id: &str) -> Result<bool, FactError>;
    fn used(&self, entity: &str, id: &str) -> Result<bool, FactError>;
    fn incoming(&self, entity: &str, id: &str) -> Result<Vec<RefEdge>, FactError>;
}
pub fn evaluate_with(module: &Module, request: &str, facts: &dyn EvaluationFacts)
    -> (DecisionRecord, ObservedReads);   // ObservedReads = fields + existence + identities + references
// `evaluate` / `evaluate_observed` use the request's `facts` section, after checking that it forms a
// valid snapshot (INCONSISTENT_FACTS otherwise). `referenced(id)` is `incoming(id)` non-empty.
// `replay` uses the record's recorded facts.
```

## Store (behavior-store)

- **`Backend` gains** `removed_at(key)`, `incoming_at(target, position)`, `used_at(key,
  position)` (with a default implementation), and a commit that also takes `removals` and
  `ref_changes`.
- **`Store::evaluate`** answers facts from the backend as of the evaluated position.
- **`Store::commit`** additionally:
  - re-derives the facts at the parent and refuses a bundle whose facts differ;
  - checks the registry, removal targets and referential integrity on S';
  - writes creations, removals and index edges atomically.
- **Replay:**
  - `replay_data` rebuilds the universe and the index;
  - `replay_behavior` uses the recorded facts, then checks them against the store.
- **New conformance cases:**
  - `create_and_remove_entity`;
  - `identity_never_reused`;
  - `removed_entity_history`: `load` at earlier states, and both replays;
  - `referential_integrity_on_resulting_state`: remove, retarget and remove in one transition;
  - `reference_index_consistency`: the index equals a reconstruction from entity content, at
    every position;
  - `concurrent_creation_same_identity`: at most one commits;
  - `existence_snapshot`: facts come from the evaluated position even while commits land.

## Python store and backends

- **Evaluation:** `Store.evaluate(...)` is unchanged; facts come from the backend. Plain
  `evaluate(model, action, state=..., input=..., facts={...})`.
- **Python backends** add `removed_at(key)` and `incoming_at(target, position)`. Their `commit`
  receives `removals` and `ref_changes`. `used_at` is optional.

## Errors

| Code | Where |
|---|---|
| `CREATE_INCOMPLETE`, `LIFECYCLE_CONFLICT` (static) | admission |
| `ENTITY_ID_ALREADY_USED`, `LIFECYCLE_CONFLICT` (dynamic) | decision result |
| `DANGLING_REFERENCE` | decision reason (DENY), store refusal |
| `UNKNOWN_FACT` | evaluation error (plain evaluate without the needed fact) |
| `INCONSISTENT_FACTS` | evaluation error (supplied facts do not form a valid snapshot) |
| `ENTITY_NOT_FOUND` | removal of an entity absent at the parent (store) |
