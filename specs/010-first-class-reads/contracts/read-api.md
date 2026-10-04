# Contract: Read API

## Python

```python
from behavior import read, project, select, sum_, BehaviorModule, replay_read

@read
def open_total(customer: Customer):
    return sum_(select(Order).where(lambda o: o.customer == customer.id), lambda o: o.amount)

@read
def order_view(order: Order):
    return project(order, lambda o: [o.status, o.amount, total(o)])     # one record

@read
def active_cultures():
    return project(select(Culture).where(lambda c: c.active),
                   lambda c: [c.name, c.stage, age(c)])                  # list of records

model = BehaviorModule("lab", entities=[...], derived=[total, age], actions=[...],
                       reads=[open_total, order_view, active_cultures])
```

| Call | Result |
|---|---|
| `behavior.evaluate_read(model, r, *, state={}, input={}, context={}, data_version, facts=None)` | `ReadResult`, plain mode (named like `behavior.evaluate`; `@read` is only the decorator). `r` is a declared read (function or name) or an undeclared `@read` function (ad-hoc) |
| `store.read(model, r, *, bindings={}, input={}, context={}, at=None)` | `ReadResult` at the head (read once) or at the `StateRef` `at`. An identity that does not exist at the position gives `result == "INVALID_BINDING"` (a record). Raises `CommitRefused("SCHEMA_MISMATCH")` on a schema mismatch, and the entity-not-found error for a foreign or future `StateRef` (no record) |
| `behavior.read_intent(model, intent, *, host)` / `store.read_intent(model, intent, *, context={}, at=None)` | `ReadExecution(response, record)`. Raises `IntentRejected` (every problem listed, including `UNKNOWN_TARGET` for a store target that does not exist at the position) before evaluation |
| `behavior.replay_read(model, record)` / `store.replay_read(model, record)` | `ReplayResult(matches, diff)` |

- **`ReadResult`**: `.result` (`"VALUE"`, `"EVALUATION_ERROR"`, `"INVALID_INPUT"` or
  `"INVALID_BINDING"`), `.value`, `.reasons`, `.record_id`, `.record: ReadRecord`, and
  `.response: ReadResponse`.
- **`ReadResponse`**: `.result`, `.value`, `.reasons`, `.record_id`. It has no evidence fields.
- **`ReadRecord`**: `.as_dict()`, `.to_json()`, `.record_id`.
- **Value mapping**: a value read gives a Python value, as in decision records. A query projection
  gives `list[dict]`, and an entity projection gives a `dict`. An absent optional value is `None`
  under its key.
- **DSL errors**, raised as `BehaviorDefinitionError` at trace time:
  - `set_`, `create`, `remove`, `requires` or `ensures` inside `@read`;
  - a projection item that is not a member field or a derived value over the member
    (`UNKNOWN_PROJECTION_ITEM`);
  - calling a `@read` function inside another body (`READ_CALL_NOT_ALLOWED`).

## Rust

- `behavior_core::read::{admit_read, evaluate_read, evaluate_read_with, evaluate_read_intent, replay_read}`
  - `admit_read(&Module, &str) -> Result<ReadItem, AdmissionResult>`, for ad-hoc read documents
  - `evaluate_read(&Module, &ReadSource, request: &str) -> ReadExecution`, plain mode
  - `evaluate_read_with(&Module, &ReadSource, request: &str, &dyn EvaluationFacts) -> ReadExecution`
  - `evaluate_read_intent(&Module, intent: &str, host: &str) -> Result<ReadExecution, IntentRejection>`
  - `replay_read(&Module, record: &str) -> ReplayResult`
- `ReadSource::{Declared(String), AdHoc(ReadItem)}`
- `ReadExecution { pub response: ReadResponse, pub record: ReadRecord }`: distinct types, with no
  conversion from record to response outside the engine.
- `Module::reads() -> &BTreeMap<String, ReadItem>`, `Module::read(name)`
- `Store::read(&self, &Module, &ReadSource, bindings, input, context, at: Option<&StateRef>) -> R<ReadExecution>`
- `Store::read_intent(&self, &Module, intent, context, at) -> R<Result<ReadExecution, IntentRejection>>`
- `Store::replay_read(&self, &Module, record: &str) -> R<ReplayResult>`
- `EvaluationFacts::entity(entity, id) -> Result<Json, FactError>`: defaulted, answered through
  `field`. `StoreFacts` overrides it with one `version_at` per call.
- `behavior_verify`: `evaluation_error` checks with the subject `read:<name>`.
  `VERIFIER_VERSION = "0.6.0"`.

## CLI

| Command | Output, exit code |
|---|---|
| `behavior read <module> <request>` | The read record. Exit 0 for `VALUE`, 3 for `EVALUATION_ERROR`, 2 for `INVALID_INPUT`, `INVALID_BINDING` or a malformed request (the CLI's convention: 3 is ERROR for decisions) |
| `behavior read-intent <module> <intent> <host> [--record <path>]` | The read response, and the record written to `<path>`. Exit 0, 3 or 2 as above; a rejection prints the `IntentRejection` and exits 2 |
| `behavior read-replay <module> <record>` | `{"matches": …, "diff": …}`. Exit 0 on a match, 2 otherwise (as `behavior replay`) |

The plain read request is:
`{"read": "<name>" | {"definition": <read document>}, "data_version", "state", "input", "context", "facts"}`.

## Public API manifest

`api/public-api.json` gains:
- Python: `read` (decorator), `evaluate_read`, `project`, `replay_read`, `read_intent`, `ReadResult`, `ReadResponse`,
  `ReadRecord`, `ReadExecution`, `Store.read`, `Store.read_intent`, `Store.replay_read`;
- the three CLI commands;
- the Rust `read` module entries.
