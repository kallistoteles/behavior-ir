# Data Model: First-Class Reads

**Feature**: 010-first-class-reads | **Date**: 2026-10-03

The decisions behind each element are in [research.md](research.md), cited as R*n*.

## Declared read / ad-hoc read (`ReadItem`, core semantic IR)

| Field | Type | Notes |
|---|---|---|
| `name` | identifier | Unique among reads, actions and derived values (R3) |
| `params` | `[Param]` | Roles: `State` (entity bound by identity), `Input`, `Context` |
| `body` | `ReadBody` | `Value(Expr)` or `Project(Projection)` |
| `hash` | `Hash` | `H(behavior.read.v1, …)` (R4) |
| `declared` | bool | True for an entry in the module's `reads` section; false for an ad-hoc read admitted per call |
| `loc` | `Loc` | Source metadata; not hashed |

**Validation (admission, R3):**
- the parameter types are known, and the roles are `state`, `input` or `context`;
- a value body is typed in the derived-value context (queries allowed) and is not entity-typed;
- no expression in the module references a declared read (`READ_CALL_NOT_ALLOWED`);
- no read shares its name with an action or a derived value (`DUPLICATE_CAPABILITY`).

A declared read is in the name table as `Kind::Read`. An ad-hoc read never is.

## Projection (`Projection`)

| Field | Type | Notes |
|---|---|---|
| `over` | `Over::Query(QueryNode)` or `Over::Param(name)` | A query of `T` (query projection), or a `state` parameter of type `Entity<T>` (entity projection) |
| `entity` | entity type name `T` | Taken from `over` |
| `member` | identifier | The member variable of `items` |
| `items` | `[Item]` | In author order; `id` is always included and never listed |

`Item` is one of:
- `Field(name)`: a declared field of `T`;
- `Derived { name, target: Hash }`: a derived value whose only parameter has type `Entity<T>`.

**Validation:**
- every item resolves, otherwise `UNKNOWN_PROJECTION_ITEM` naming the item (a reference path such
  as `customer.name` never resolves);
- no item appears twice, and no field and derived item share a name (`DUPLICATE_PROJECTION_ITEM`).

**Cardinality.** A query projection gives zero or more records, in canonical identity order. An
entity projection gives exactly one record.

## Read request (plain mode and store)

| Field | Plain mode | Store |
|---|---|---|
| read | the declared read's name, or a read document (ad-hoc) | the same, as `ReadSource` |
| `state` | the bound entities' values | `bindings`: param → id, loaded at the position |
| `input`, `context` | JSON objects | the same |
| `data_version` | host-supplied | derived: store identity and state reference at the position |
| `facts` | supplied facts (same forms and consistency rules as evaluation) | never; `StoreFacts` at the position |
| position | n/a | `at: Option<StateRef>`; `None` means the head, read once |

## Read record (`behavior.read_record.v1`)

| Field | Type | Notes |
|---|---|---|
| `format` | string | `"behavior.read_record.v1"` |
| `behavior_version` | string | The module hash |
| `read` | object | `name`, `hash`, `declared`; plus `definition` (canonical wire) for an ad-hoc read |
| `data_version` | string | The exact state read |
| `state` | object | The bound entities' canonical values |
| `input`, `context` | object | Decoded and normalized |
| `result` | enum | `VALUE`; `EVALUATION_ERROR` (reasons include `UNKNOWN_FACT` for a missing fact); `INVALID_INPUT` (invalid parameters, or `INCONSISTENT_FACTS`); `INVALID_BINDING` (a `state` parameter's identity does not exist at the read's state; its existence fact `false` is recorded, so replay reproduces it). A schema mismatch or an invalid position is an error with no record |
| `value` | JSON | For `VALUE` only. A value read gives its canonical value. A query projection gives `[{"id", item…}]` sorted by `id`. An entity projection gives `{"id", item…}`. Absent optional values are `null`, never omitted |
| `reasons` | `[Reason]` | For results other than `VALUE`. An evaluation error names its location and, for a projection, the member `T#id` and the item |
| `derived` | `[DerivedEntry]` | As in decision records; the phase is always `S` |
| `observed` | `[[param, field]]` | Field reads of bound entities, sorted; for an entity projection, every projected stored field |
| `facts` | Facts JSON | Observed existence, identity, reference, query and field facts; for a query projection, every projected stored field of every member |
| `record_id` | string | `read:sha256:…`, the hash of the canonical record without `record_id` (R6) |

**Invariants:**
- The record is canonical and byte-deterministic (FR-010).
- It is never written to a store (FR-012).
- Replayed from its own facts, or against the store at `data_version`, it reproduces every byte,
  including `record_id` (FR-011).

## Read response (`ReadResponse`, capability output)

| Field | Type | Notes |
|---|---|---|
| `result` | enum | As in the record |
| `value` | JSON | For `VALUE` |
| `reasons` | `[Reason]` | For refusals and errors |
| `record_id` | string | The identity of the record that produced it |

There is no `state`, `facts`, `observed`, `derived` or `definition` field. This is a distinct type
from `ReadRecord` (FR-016a, R7).

## Read execution

`ReadExecution { response: ReadResponse, record: ReadRecord }` is what every read returns to the
trusted host. The host forwards `response` to an untrusted caller and keeps or discards `record`.

## Read intent (capability call)

| Field | Notes |
|---|---|
| `capability` | The name of a declared read; an action name is `UNKNOWN_CAPABILITY` |
| (store only) | Every target must exist at the read position; a missing one is `UNKNOWN_TARGET`, listed with every other problem |
| `targets` | `state` parameter → identity |
| `input` | `input` parameter → value |

Any other key is `EXTRA_ARGUMENT`. Context and data version come from the host. Every problem is
reported before evaluation (R9).

## State transitions

None. A read has no lifecycle and changes no store state. The only state involved is the
snapshot it names.
