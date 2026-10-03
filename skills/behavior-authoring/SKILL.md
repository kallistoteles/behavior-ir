---
name: behavior-authoring
description: Model a domain as Behavior with the Python binding (entities, field types, actions, creation and removal, references, queries over sets, entity constraints, module invariants, exact arithmetic). Use when writing or changing a behavior model.
release: 0.9.0
---

# Authoring behavior

This skill describes **Behavior release 0.9.0**. Use only what is described here or listed in the
release's public API (`behavior.versions()` reports the installed release). Everything is imported
from the `behavior` package.

> If the public Behavior API cannot express a requirement, record a semantic gap in
> `SEMANTIC_GAPS.md` (format: skills/README.md). Do not work around it: no engine internals,
> no hand-written IR, no moving the rule into host code, no silent approximation.

## What a model is

A **behavior model** is a `BehaviorModule`: entity declarations plus the rules and actions over
them. Building it runs your Python functions **once, symbolically**. Every expression becomes a
typed engine node, and the engine admits the module and gives it a content-addressed
`behavior_version`.

The Python code never runs at decision time. The engine evaluates the admitted module, so:

- **Python control flow never sees behavior values.** `if`, `and`, `or`, `not`, chained
  comparisons, loops over queries and `len()` all raise. Combine conditions with `&`, `|`, `~`
  (or `and_`, `or_`, `not_`).
- **Use exact values.** `float` literals are refused; use `decimal.Decimal`.
- **Build expressions inside behavior bodies.** Typed literals such as `Money(Decimal("0"))`, and
  every other expression, can only be built inside a decorated function (action, derived value,
  rule, constraint, invariant). At module level they raise "behavior expressions can only be
  built inside behavior bodies". Keep a plain `Decimal("0")` as a module constant, and wrap it
  inside the body. A plain Python helper function that builds an expression is fine when it is
  called from a body.

## Entities and field types

```python
# from examples/entities_and_types.py
Money = nominal("Money", Decimal, ops={"order", "add", "scale", "ratio"}, scale=2)


class InvoiceStatus(Enum):
    PENDING = "pending"
    APPROVED = "approved"


@entity
class User:
    role = field(str)
    approval_limit = field(Money)


@entity
class Invoice:
    amount = field(Money)
    status = field(InvoiceStatus)
    approved_by = field(Option[Id[User]])  # optional: None until approved
    owner = field(Ref[User])  # a reference: Id[User] that must point at an existing User
```

| Type | Use |
|---|---|
| `str`, `bool`, `int` | plain values (`int` is 64-bit; overflow is an evaluation error) |
| `Decimal` | a general decimal, computed exactly |
| `nominal("Money", Decimal, ops=…, scale=2)` | a domain type. `ops` lists the allowed operations (`add`, `order`, `scale`, `ratio`), and `scale` makes it fixed-scale (exactly that many fractional digits) |
| an `Enum` subclass | a closed set of values |
| `Option[T]` | a value that may be absent: compare with `none`, and use `.is_some()`, `.is_none()`, `.value_or(default)` |
| `Id[T]` | the identity of an entity of type `T`; every entity has an `id` |
| `Ref[T]` | `Id[T]` plus the rule that the referenced entity exists. Removing a still-referenced entity is refused |

Every entity has an implicit `id` field.

## Actions

An action is one state transition, with parameters of three kinds:
- **state entities** (plain parameters, e.g. `account: Account`): entities from the state, bound
  by id, which the transition may change;
- **inputs** (`*, amount: Input[Money]`): values from the request;
- **context** (`*, actor: Context[User]`): who is acting. The host passes the whole entity (for
  example the logged-in user); the action reads it but can never change it.

```python
# from examples/entities_and_types.py
@action
def approve_as(invoice: Invoice, *, actor: Context[User]):
    requires(actor.role == "manager")
    requires(invoice.amount <= actor.approval_limit)
    set_(invoice.status, InvoiceStatus.APPROVED)
    set_(invoice.approved_by, actor.id)
```

Use context for the acting party. If the approver is itself state (its record must exist in the
store and may change), bind it as a state parameter instead.

The action's body can use:
- `requires(cond)`: a precondition on the current state;
- `set_(entity.field, value)`: an effect;
- `create(T, id=…, field=…)` and `remove(entity)`: lifecycle effects;
- `ensures(cond)`: a postcondition on the resulting state.

```python
# from examples/actions_and_lifecycle.py
@action
def open_account(owner: Customer, *, account_id: Input[Id[Account]], initial: Input[Money]):
    requires(initial >= Money(Decimal("0")))
    create(Account, id=account_id, owner=owner.id, balance=initial)
    ensures(exists(account_id))
```

- **Identities are inputs.** The host chooses them; the engine never generates one. An identity
  is used once, forever: creating it again is `ENTITY_ID_ALREADY_USED`, even after removal.
- **Creation gives a complete value.** `create` must give every field.
- **Removal is logical.** `remove(entity)` removes a bound state entity; history keeps it.
- **Existence tests.** `exists(id)` checks that an entity exists, and `referenced(id)` checks
  whether any surviving `Ref` points at it:

```python
# from examples/actions_and_lifecycle.py
@action
def remove_customer(customer: Customer):
    requires(not_(referenced(customer.id)))
    remove(customer)
```

## Entity rules: constraints and invariants

- **`@constraint`** takes one entity parameter. It says what a valid entity of that type is, and
  is checked on every incoming entity and on every entity the action creates or changes.
- **`@invariant`** with one entity parameter is checked on the state entities before and after
  the transition.

These rules are **local**: they read one entity and may not contain queries. They take exactly
one parameter. A rule relating two entities, such as "an order never exceeds its customer's
limit", cannot be a constraint or invariant in this release; see the semantic gaps below.

An incoming entity that breaks a constraint is not a decision. The result is `INVALID_STATE`
with `CONSTRAINT_VIOLATED`.

```python
# from examples/actions_and_lifecycle.py
@constraint
def non_negative_balance(a: Account):
    return a.balance >= Money(Decimal("0"))
```

## Queries and module invariants

A query is a set of existing entities of **one** type:
- build it with `select(T)`, then `.where(lambda o: …)` and
  `.union` / `.intersection` / `.difference`;
- turn it into a value with `count`, `any_`, `all_`, `sum_` (exact; the empty sum is zero),
  `min_` / `max_` (optional: absent for the empty set) or `unique(q, by=…)`.

```python
# from examples/queries_and_module_invariants.py
@action
def check_orders(customer: Customer):
    orders = select(Order).where(lambda o: o.customer == customer.id)
    requires(not_(any_(orders, lambda o: o.status == OrderStatus.BLOCKED)))
    requires(all_(orders, lambda o: o.amount > Money(Decimal("0"))))
    requires(sum_(orders, lambda o: o.amount) <= customer.credit_limit)
    requires(max_(orders, lambda o: o.amount).value_or(Money(Decimal("0"))) <= customer.credit_limit)
```

- **Filters are candidate-local.** A lambda reads the candidate's fields, literals and the
  action's parameters. It never reads `exists`/`referenced`, another query, or another entity.
- **Queries are expressions.** Use them inline. Reuse goes through a derived value, which returns
  a value, never a query.
- **Postconditions see the resulting state.** In `ensures`, a query sees the resulting state:
  created entities are members, removed ones are not.

A **module invariant** is an `@invariant` with **no parameters**: a closed statement about the
whole state. It is checked at store creation and on every resulting state it could be affected
by. "Local invariants describe entities; global invariants describe relations."

```python
# from examples/queries_and_module_invariants.py
@invariant
def personnel_numbers_unique():
    return unique(select(Employee), by=lambda e: e.personnel_number)
```

A decision's record lists exactly the set members it depended on (`decision.facts["queries"]`),
so a decision over a set is reproducible.

**Module invariants assume a valid starting state.** On the resulting state, a `unique` that is
a top-level part of a module invariant is checked only for the entities the transition touched,
because it already held before. A store guarantees that: it checks module invariants at creation
and after every commit. With plain `evaluate`, you supply the state, and it must be valid.

## Derived values and rules

- **`@derived`** names a computed value over entity parameters, used like a function in other
  expressions (`open_order_count(customer)`). It is never inlined, and it is hashed as its own
  item.
- **`@rule`** is a derived value of type Bool. The verifier warns when a rule is always true or
  always false (`vacuity`).

```python
# from examples/queries_and_module_invariants.py
@derived
def open_order_count(customer: Customer):
    orders = select(Order).where(lambda o: o.customer == customer.id)
    return count(orders.where(lambda o: o.status == OrderStatus.OPEN))


# A rule is a Bool derived value; the verifier warns if it is always true or always false.
@rule
def has_open_orders(customer: Customer):
    return open_order_count(customer) > 0
```

## Exact arithmetic

- **Arithmetic is exact.** Fixed-scale values never round silently, and a ratio (`a / b`) is an
  exact value such as `1/3`.
- **Rounding is explicit and only at a store.** Round with `rescale(value, Money, Rounding.X)`
  when storing into a fixed-scale field. The modes are `FLOOR`, `CEILING`, `DOWN`, `UP`,
  `HALF_UP` and `HALF_EVEN`.
- **`Exact[...]`** declares a derived value that stays exact.
- **Dividing by a plain number** is exact too: `customer.credit_limit / Decimal("3")` stores only
  through `rescale`.

```python
# from examples/exact_arithmetic.py
@derived
def portion(share: Share) -> Exact[Decimal]:
    return share.amount / share.budget  # an exact ratio such as 1/3: no rounding here


@action
def allocate(share: Share):
    requires(portion(share) <= Decimal("0.5"))
    set_(share.part, rescale(portion(share) * share.budget, Money, Rounding.HALF_EVEN))
```

## Changing a schema

A store's data is valid only under its **store schema**: the entity declarations, meaning their
fields, field types (with the enums and nominal types they use) and references. `model.schema_hash`
identifies it. Rules, actions, derived values, constraints and invariants are behavior, not
schema: change them freely, with no migration.

Any declaration change needs a **migration**, even appending an optional field or reordering
fields. Until one is applied, the store refuses the new module with `SCHEMA_MISMATCH` before
anything runs.

A `Migration` relates the source and the target module:

- **`transforms`**: for each changed entity type, `lambda old: {field: value}`. A target field with
  the same name and exactly the same type as a source field is copied automatically. Everything
  else is explicit: `enum_map` for an enum whose values changed (it must map every value), plain
  assignment for a lossless widening such as `int` to `Decimal`, and `rescale(underlying(...),
  Type, Rounding...)` between nominal types.
- **`drops`**: removed fields that no assignment reads. Losing information is never silent.
- **`retire`**: entity types the target no longer declares. They must be empty when applied.
- **`requires`**: closed predicates over the source state. They are the only way to narrow:
  `strict_unwrap` (an option becomes required) and `strict_enum_map` (values disappear) are
  allowed where a requirement proves them safe.

Migrations change representation; behavior changes information. A transform reads only the old
entity, literals and constants, never other entities. To fill a field from a related entity, take
the staged path: broaden (an optional field), backfill with an ordinary action, then narrow under
a requirement.

```python
# from examples/changing_a_schema.py
transforms = {
    v1.Culture: lambda old: {
        "medium_type": enum_map(old.medium, {
            v1.Medium.MS: v2.Medium.MS, v1.Medium.WPM: v2.Medium.WPM,
        }),
        "ph": old.ph,
        "notes": None,
    },
}
broaden = Migration(
    source=v1.model,
    target=v2.model,
    transforms=transforms,
    drops={v1.Culture: ["legacy_code"]},
    retire=[v1.AuditNote],
)
assert broaden.admit().ok
narrow = Migration(
    source=v2.model,
    target=v3.model,
    requires={
        "every_order_has_region": lambda: all_(select(v2.Order), lambda o: o.region.is_some()),
    },
    transforms={v2.Order: lambda old: {"region": strict_unwrap(old.region)}},
)
```

Review `migration.summary()`, which lists for every migrated type its copied, transformed, new and
dropped fields. Then verify the migration (behavior-verification) and apply it to the store
(behavior-application). Admission errors: `MISSING_MIGRATION_FIELD` (assign the field),
`UNACKNOWLEDGED_FIELD_DROP` (drop it), `MISSING_RETIREMENT` (retire the type),
`UNMAPPED_ENUM_VALUE` (map every value, or narrow under a requirement),
`MIGRATION_TYPE_MISMATCH` (convert explicitly), `NON_LOCAL_TRANSFORM` (backfill with an action).

## When the engine refuses a model

Errors come at one of two points, and both name the line you wrote:

- **While building.** A `BehaviorTypeError` (with `.code`) or a `BehaviorDefinitionError` is
  raised when the module is constructed.
- **At admission.** `admit(model).errors` lists the errors, and using `model.engine` raises
  `BehaviorInvalid`.

| Code | Cause | Fix |
|---|---|---|
| `TYPE_MISMATCH` | ill-typed expression, e.g. comparing `Money` with `int`, `sum_` of text, set algebra over two entity types | use the declared types; write literals as `Money(Decimal("5.00"))` |
| `LOSSY_CONVERSION` | an exact or general value stored into a fixed-scale field | `rescale(value, Money, Rounding.HALF_EVEN)`, with the mode the domain requires |
| `QUERY_NOT_ALLOWED` | a query in an entity constraint or per-entity invariant, or a query nested in a filter | move the rule to a module invariant (no parameters) or to an action's `requires` |
| `NON_LOCAL_PREDICATE` | a filter reads `exists`, `referenced` or another entity | filter on the candidate's own fields; if the rule needs a join, it is a semantic gap |
| `EFFECT_ON_READONLY` | `set_` on an input or a context parameter (or on a parameter of a derived value or rule, which only read) | only state entities change |
| `DUPLICATE_NAME` | two items with one name | rename |
| `BehaviorDefinitionError` | Python control flow over behavior values; a query used as a collection; a `float` literal; an expression built outside a behavior body; an `@invariant` or `@constraint` with more than one parameter | use `&` / `|` / `~`, the relational operators, `Decimal`; build inside the body; see the gap table for rules over two entities |

```python
# from examples/placement_errors.py
refused = BehaviorModule(entities=[Customer, Order], constraints=[few_orders])
assert [e.code for e in admit(refused).errors] == ["QUERY_NOT_ALLOWED"]
```

## Requirements that are semantic gaps in 0.9.0

Record these, do not approximate them:

| Requirement shape | Missing construct |
|---|---|
| "at most N open orders **per customer**" as a standing rule | grouped (per-key) module invariants |
| "an order never exceeds **its customer's** credit limit" as a standing rule | invariants over two related entities |
| "orders **of customers in region R**" | cross-entity filters (joins or semijoins) |
| "close **all** open orders of a customer" | bulk effects (for-each over a query). A query has no fields, so `set_` on one fails with a plain Python `AttributeError` |
| "the **newest** order", pagination, "top 10" | ordering; sets have no order |
| any entity lookup beyond `select`, a bound parameter or an identity | nothing: the host binds entities explicitly |

A partial measure, such as a precondition on the one action you know about, may be taken only if
it is recorded in the gap entry's `What was done instead`, with its risk.
