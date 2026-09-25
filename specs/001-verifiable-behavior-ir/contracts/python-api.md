# Contract: Python Authoring API v0.1 (`behavior` package)

The public surface a behavior author uses. Everything not listed here is private. The DSL only
**constructs** behavior; it never evaluates it (PRINCIPLES.md §5).

## Declarations

```python
from decimal import Decimal
from enum import Enum
from behavior import (
    entity, field, nominal, Option, Id, Context, Input, none, underlying,
    derived, rule, invariant, action,
    requires, ensures, set_, and_, or_, not_,
    BehaviorModule, admit, evaluate, evaluate_intent, replay,
)

Money = nominal("Money", Decimal, ops={"order", "add", "scale", "ratio"})

class InvoiceStatus(Enum):              # plain Python Enum with str values
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
    approved_by = field(Option[Id[User]])

@invariant
def non_negative_amount(invoice: Invoice):
    return invoice.amount >= Money(Decimal("0"))

@action
def approve_invoice(invoice: Invoice, *, actor: Context[User]):
    requires(invoice.status == InvoiceStatus.PENDING)
    requires(actor.role == "manager")
    requires(invoice.amount <= actor.approval_limit)
    set_(invoice.status, InvoiceStatus.APPROVED)
    set_(invoice.approved_by, actor.id)          # Id[User] → Option[Id[User]]
    ensures(invoice.approved_by == actor.id)

@entity
class Project:
    revenue = field(Money)
    cost = field(Money)

@derived
def margin(project: Project):
    return (project.revenue - project.cost) / project.revenue    # Money / Money → Decimal (ratio)

@rule
def high_risk(project: Project):
    return margin(project) < Decimal("0.05")
```

| Name | Contract |
|------|----------|
| `field(T)` | `T` ∈ `bool`, `int`, `Decimal`, `str`, an `Enum` subclass with `str` values, a nominal type, `Id[E]`, or `Option[T]`. `float` raises `BehaviorDefinitionError` ("use decimal.Decimal"). Every entity also gets `id: Id[Self]`; declaring `id` is an error. |
| `nominal(name, P, ops=…)` | Declares a nominal type over primitive `P` (`bool`, `int`, `Decimal`, `str`); `ops` ⊆ {`order`, `add`, `scale`, `ratio`}. `N(literal)` makes a literal; `N(expr)` wraps an expression of type `P`; `underlying(expr)` unwraps. |
| `Option[T]`, `none` | Optional type and its empty literal. Expressions: `.is_none()`, `.is_some()`, `.value_or(d)`, `== none`. |
| `Id[E]` | Identity of entity `E`. |
| `Context[T]`, `Input[T]` | Keyword-only action parameter roles (read-only). A bare entity annotation is a **state** parameter (the only kind `set_` may target). |
| `@derived`, `@rule`, `@invariant`, `@action` | Register without running; bodies are traced when the module is compiled. `@invariant` takes exactly one entity parameter. |
| Calling a `@derived`/`@rule` | Returns a derived reference (no inlining). |
| `requires(e)`, `ensures(e)` | Pre/postcondition; only inside `@action`; `e` must be Bool. |
| `set_(target, value)` | Effect; `target` must be a field of a state parameter and not `id`. |
| Operators | `== != < <= > >= + - * /`, `&` `\|` `~`, `and_()` `or_()` `not_()`, `.in_([...])`. Operand types are checked on construction by the rules in [data-model.md](../data-model.md#typing-rules). |
| `bool(expr)` | Raises: behavior cannot drive Python control flow (`if`, `and`, `or`, `not`, `a < b < c`). |

Construction-time errors are `BehaviorTypeError` (ill-typed expression) or
`BehaviorDefinitionError` (misuse of the DSL), both with file and line of the author's code.

## Module, admission, evaluation

```python
model = BehaviorModule(
    entities=[User, Invoice, Project],
    derived=[margin, high_risk],
    invariants=[non_negative_amount],
    actions=[approve_invoice],
    root=".",            # optional base for relative source paths
)                        # enums and nominal types used by fields are collected automatically

model.to_wire_json() -> str           # canonical wire IR, serialized by the engine from the admitted module

result = admit(model)                 # AdmissionResult (engine-side parse, resolve, type-check, hash)
result.ok, result.errors, result.behavior_version, result.evaluation_order, result.items

decision = evaluate(
    model, "approve_invoice",
    state={"invoice": {"id": "1042", "amount": Decimal("43200"), "status": "pending", "approved_by": None}},
    context={"actor": {"id": "anna", "role": "manager", "approval_limit": 50000}},
    input={},
    data_version="18342",
    git_revision=None,
)
decision.result        # "ALLOW" | "DENY" | "ERROR" | "INVALID_INPUT" | "INVALID_STATE"
decision.changes       # ΔS
decision.reasons, decision.trace, decision.record_json

evaluate_intent(
    model,
    intent={"capability": "approve_invoice", "targets": {"invoice": "1042"}, "input": {}},
    state=..., context=..., data_version="18342",
)                      # Decision, or raises IntentRejected with all errors

replay(model, decision.record_json)   # ReplayResult: .matches, .diff
```

- `evaluate`/`evaluate_intent` admit the module first; if admission fails they raise
  `BehaviorInvalid` carrying the `AdmissionResult` (FR-009).
- Input values: `bool`, `int`, `Decimal`, `str`, `None` (option none), `Enum` members or their
  string values. `float` raises `TypeError` before reaching the engine.
- Hashes and behavior versions always come from the engine; the Python package does not hash.
- Type checking happens only in the engine: each operator calls the engine's builder, which
  checks the node and raises at the author's line (revised 2026-09-25). `to_wire_json()` is
  only available for admitted modules; it shows conversions explicitly and lists declarations
  by name, so it round-trips to the same behavior version.
- Result objects are frozen dataclasses built from the engine's JSON
  ([engine-api.md](engine-api.md)).
