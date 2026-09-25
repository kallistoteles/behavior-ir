# Contract: Engine API v0.1 (Rust library, Python binding, CLI)

The engine exposes the same operations through three surfaces. Inputs are JSON (wire IR per
[ir-encoding.md](ir-encoding.md)); outputs are canonical JSON (sorted keys, compact, no floats).
The engine never panics on input.

## Operations

| Operation | Input | Output |
|-----------|-------|--------|
| `admit` | wire IR | AdmissionResult |
| `evaluate` | wire IR, EvaluationRequest | DecisionRecord, or AdmissionResult (`ok: false`) |
| `evaluate_intent` | wire IR, StructuredIntent, HostContext | DecisionRecord, IntentRejection, or AdmissionResult |
| `replay` | wire IR, DecisionRecord | ReplayResult |

Every operation admits the wire IR first; only the semantic module is evaluated.

### Rust (`behavior-core`)

```rust
pub fn admit(wire: &str) -> Result<Module, AdmissionResult>;   // Module = semantic IR (private fields)
pub fn admission_report(wire: &str) -> AdmissionResult;
pub fn evaluate(module: &Module, request: &str) -> DecisionRecord;
pub fn evaluate_intent(module: &Module, intent: &str, host: &str) -> Result<DecisionRecord, IntentRejection>;
pub fn replay(module: &Module, record: &str) -> ReplayResult;
```

`Module` and all semantic node types can only be constructed by `admit`.

### Python binding (`behavior._engine`, private)

Revised 2026-09-25 (research R12, R17): Python holds engine objects and passes native values.

| Name | Contract |
|------|----------|
| `Type` | `Type.bool()`, `int()`, `decimal()`, `string()`, `option(t)`, `enum(name)`, `nominal(name)`, `id(entity)`, `entity(name)`; `is_option()`, `inner()`; equality |
| `Builder` | `declare_enum(name, values, file, line)`, `declare_nominal(name, underlying: Type, ops, file, line)`, `declare_entity(name, fields: [(name, Type, file, line)], file, line)`, `push_scope(site, params: [(name, role or None, Type)], file, line)`, `pop_scope()`, node constructors `lit(Type, value, …)`, `field`, `param`, `derived_ref`, `op`, `in_(node, values, …)`, `wrap` (each takes the author's `file`, `line`), `check_condition`, `check_effect`, `add_derived`, `add_invariant`, `add_action`, `finish(root) -> (Module or None, admission dict)` |
| `Node` | `type` (`Type`, or `None` below an unresolved cycle reference), `role` |
| `Module` | `behavior_version`, `admission() -> dict`, `wire_json()` (canonical serialization), `evaluate(action, state, input, context, data_version, git_revision=None) -> Record`, `evaluate_intent(intent, state, context, data_version, git_revision=None) -> Record`, `replay(record_json) -> (matches, diff)`; `Module.from_wire(wire_json) -> (Module or None, admission dict)` |
| `Record` | `result`, `data` (dict), `json` (canonical record text) |
| `EngineError` | raised by builder calls with `args == (code, message)` |
| `EngineIntentRejected` | raised by `evaluate_intent` with `args == (errors,)`, a list of `{code, message, path}` dicts |

Values: `None`, `bool`, `int`, `str`, `Decimal` (converted from its plain `format(d, "f")` form),
`Enum` members (their value), and dicts and lists of these. `float` raises `TypeError`.

### CLI (`behavior`)

```text
behavior admit   <wire.json>
behavior version <wire.json>                         # prints behavior_version
behavior hashes  <wire.json>                         # name → item hash table
behavior eval    <wire.json> <request.json>
behavior intent  <wire.json> <intent.json> <host.json>
behavior replay  <wire.json> <record.json>
```

Exit codes: `0` admitted / ALLOW / replay match; `1` DENY; `2` admission failure /
INVALID_INPUT / INVALID_STATE / intent rejected / replay mismatch; `3` ERROR; `64` usage error.

## Payloads

### AdmissionResult

```json
{"ok": true, "behavior_version": "sha256:…", "errors": [],
 "evaluation_order": ["margin", "high_risk"],
 "items": {"action:approve_invoice": "sha256:…", "derived:margin": "sha256:…", "entity:Invoice": "sha256:…"}}
```

On failure: `ok: false`, no `behavior_version`, `errors` sorted by file, line, code, each
`{code, message, loc, related_locs}` (codes in [data-model.md](../data-model.md)). Example
cycle message: `derived values form a cycle: a → b → c → a`.

### EvaluationRequest

```json
{"action": "approve_invoice", "data_version": "18342", "git_revision": "7f83ca2",
 "state":   {"invoice": {"id": "1042", "amount": "43200", "status": "pending", "approved_by": null}},
 "input":   {},
 "context": {"actor": {"id": "anna", "role": "manager", "approval_limit": "50000"}}}
```

### StructuredIntent (what an AI may choose) and HostContext (what the trusted host supplies)

```json
{"capability": "approve_invoice", "targets": {"invoice": "1042"}, "input": {}}
```

```json
{"data_version": "18342",
 "state":   {"invoice": {...}},
 "context": {"actor": {...}}}
```

IntentRejection: `{"rejected": true, "errors": [{"code", "message", "path"}]}`, all problems
listed, sorted by `path`. Codes: `UNKNOWN_CAPABILITY`, `MISSING_ARGUMENT`, `EXTRA_ARGUMENT`,
`WRONG_TYPE`, `MISSING_TARGET`, `EXTRA_TARGET`, `TARGET_MISMATCH` (target id ≠ supplied state
entity's `id`). An accepted intent is merged with the host context into an EvaluationRequest
and evaluated exactly like one (same record bytes).

### DecisionRecord

```json
{"record_version": "0.1",
 "behavior_version": "sha256:…", "git_revision": "7f83ca2", "data_version": "18342",
 "action": {"name": "approve_invoice", "hash": "sha256:…"},
 "state":   {"invoice": {"amount": "43200", "approved_by": null, "id": "1042", "status": "pending"}},
 "input":   {},
 "context": {"actor": {"approval_limit": "50000", "id": "anna", "role": "manager"}},
 "result": "ALLOW",
 "reasons": [],
 "derived": [],
 "trace": [
   {"phase": "invariant_pre", "name": "non_negative_amount", "hash": "sha256:…",
    "expr_text": "invoice.amount >= Money(0)", "reads": {"invoice.amount": "43200"},
    "outcome": true, "loc": {"file": "invoice.py", "line": 22}},
   {"phase": "precondition", "hash": "sha256:…", "expr_text": "invoice.status == InvoiceStatus.pending",
    "reads": {"invoice.status": "pending"}, "outcome": true, "loc": {"file": "invoice.py", "line": 27}},
   {"phase": "precondition", "hash": "sha256:…", "expr_text": "actor.role == \"manager\"",
    "reads": {"actor.role": "manager"}, "outcome": true, "loc": {"file": "invoice.py", "line": 28}},
   {"phase": "precondition", "hash": "sha256:…", "expr_text": "invoice.amount <= actor.approval_limit",
    "reads": {"actor.approval_limit": "50000", "invoice.amount": "43200"}, "outcome": true,
    "loc": {"file": "invoice.py", "line": 29}},
   {"phase": "effect", "hash": "sha256:…", "expr_text": "invoice.status := InvoiceStatus.approved",
    "reads": {}, "outcome": {"assigned": "approved"}, "loc": {"file": "invoice.py", "line": 30}},
   {"phase": "effect", "hash": "sha256:…", "expr_text": "invoice.approved_by := some(actor.id)",
    "reads": {"actor.id": "anna"}, "outcome": {"assigned": "anna"}, "loc": {"file": "invoice.py", "line": 31}},
   {"phase": "postcondition", "hash": "sha256:…", "expr_text": "invoice.approved_by == some(actor.id)",
    "reads": {"actor.id": "anna", "invoice.approved_by": "anna"}, "outcome": true,
    "loc": {"file": "invoice.py", "line": 32}},
   {"phase": "invariant_post", "name": "non_negative_amount", "hash": "sha256:…",
    "expr_text": "invoice.amount >= Money(0)", "reads": {"invoice.amount": "43200"},
    "outcome": true, "loc": {"file": "invoice.py", "line": 22}}
 ],
 "changes": [
   {"param": "invoice", "field": "status", "old": "pending", "new": "approved"},
   {"param": "invoice", "field": "approved_by", "old": null, "new": "anna"}
 ]}
```

- `expr_text` comes from the engine's pretty printer over the semantic IR (explicit conversions
  shown), not from Python source.
- `reads` keys are `param.field`, `param`, or a derived value's name; values use the request
  encoding.
- `outcome`: `true`, `false`, `"skipped"`, `{"error": "<message>"}`, or `{"assigned": value}`.
- `derived`: `{"name", "hash", "phase_state": "S" | "S'", "value"}` in evaluation order.

### ReplayResult

`{"matches": true}` or `{"matches": false, "diff": "<first differing JSON path and values>"}`.
A record whose `behavior_version` differs from the admitted module is a mismatch.

## Numeric semantics

Exact decimal arithmetic with at most 28 significant digits; inexact division rounds half to
even at 28 digits. Overflow and division by zero are evaluation errors (`ERROR`), never silent.
