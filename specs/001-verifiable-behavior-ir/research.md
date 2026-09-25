# Research: Verifiable Behavior IR Core (v0.1)

Decisions taken during `/speckit-plan` on 2026-09-23 and revised the same day after the
clarification session and `PRINCIPLES.md`. Revised again on 2026-09-25: Python became a binding to
the Rust engine instead of a generator of wire JSON (R10, R12, R17). Each entry: decision, rationale, alternatives.

## R1. Where the engine lives

- **Decision**: the semantic Behavior IR, admission (parse, resolve, type-check), hashing,
  evaluation, and decision records are one Rust library. Python is the authoring layer and a
  thin client.
- **Rationale**: the constitution fixes Rust for the core; one evaluator and one type checker of
  record means one set of semantics.
- **Alternatives**: all-Python v0.1 (needs a constitution amendment and a later rewrite); two
  evaluators kept in sync (drift risk).

## R2. Two representations: wire IR and semantic Behavior IR

- **Decision**:
  - **Wire IR** is the untrusted exchange form: JSON, names instead of hashes, source locations
    included, implicit conversions allowed (e.g. `Int` where `Decimal` is expected, `T` where
    `Option<T>` is expected). Emitted canonically (sorted keys, compact, UTF-8, ASCII
    identifiers, decimals as strings, no floats) so it diffs cleanly in Git.
  - **Semantic Behavior IR** exists only inside the engine. It is produced by a single
    *admission* step and cannot be constructed any other way: its types have private fields and
    only the admission module can build them (Rust's "parse, don't validate" idiom). Every
    expression node carries its type; every reference is resolved; every implicit conversion
    is an explicit node (`to_decimal`, `some`); every node has its content hash.
- **Rationale**: principle 9 — invalid behavior cannot enter the semantic model, while wire
  data may be malformed. Making conversions explicit in the semantic IR means two wire
  documents that differ only in implicit vs explicit conversion get the same hash.
- **Alternatives**: one representation validated after the fact (invalid IR could exist and be
  hashed or evaluated); full GADT-style `Expr<T>` in Rust (heavy generics, no gain over checked
  construction with private fields).

## R3. Admission pipeline

- **Decision**: `wire JSON → strict decode (unknown keys rejected) → name resolution (entities,
  nominal types, enums, parameters, fields, derived) → reference graph by name → cycle
  detection → type check in topological order → explicit conversions inserted → content hashes
  computed bottom-up → semantic Module`. All errors from all stages that can run are collected
  and sorted by (file, line, code). A cycle stops the pipeline before type checking of the
  affected derived values and before hashing, so the module gets no behavior version.
- **Rationale**: derived values reference each other by hash in the semantic IR (R5), which is
  impossible for a cycle; so cycles must be found on the name graph first. Type checking in
  topological order lets each derived value's result type be known before it is used.

## R4. Content hashing: identity separate from storage and serialization

- **Decision**:
  - Every semantic node has `hash = SHA-256(tag ‖ 0x00 ‖ canonical_semantic_bytes)`, where `tag`
    names the node kind and hash format version (e.g. `behavior.expr.v1`,
    `behavior.action.v1`, `behavior.module.v1`).
  - `canonical_semantic_bytes` is a small, fixed, length-prefixed binary encoding of the node's
    semantic fields, with children included by their 32-byte hash. It is defined in
    [contracts/hashing.md](contracts/hashing.md) and is independent of the JSON wire format.
  - Only the Rust engine computes hashes; Python obtains them from the engine.
- **Rationale**: principle 6 — hash the meaning, not the bytes; changing the wire format (e.g.
  JSON → MessagePack) must not change any identity; domain separation prevents identical bytes
  from meaning two node kinds. The tree of hashes is a Merkle DAG even though the module is
  stored as one document, so a per-object store later is a storage change only.
- **Alternatives**: hash canonical JSON bytes (ties identity to one serialization, includes
  metadata); RFC 8785 JCS (same problem).

## R5. What is inside and outside the hash

- **Decision**:

  | Included (affects behavior) | Excluded |
  |------------------------------|----------|
  | node kinds, result types, literal values and types | source locations |
  | operator structure, order where semantic (preconditions, effects, postconditions) | documentation, comments |
  | entity, field, enum value, parameter names (shape of state/input/context) | names of derived values, rules, invariants, actions (bound in the module's name table) |
  | nominal type names and declared operations (a nominal type's identity is its name) | declaration order where not semantic |
  | references to derived values: by the referenced item's hash | |

  The module hash covers the **name table**: a list of `(kind, name, item_hash)` sorted by
  `(kind, name)`. Entities, nominal types, and enums are hashed as items too, so the module hash
  also covers the information model. `behavior_version = "sha256:" + hex(module_hash)`.
- **Rationale**: a rename changes the module (the interface callers see), not the rule; editing
  a comment changes nothing; reordering entity declarations changes nothing; reordering
  preconditions changes behavior (evaluation stops at the first false), so it changes the hash.
- **Consequence**: two derived values with identical bodies share a hash; both names are bound
  to it in the table. Decision records cite both name and hash.

## R6. Transition model: state, input, context, and ΔS

- **Decision**: an action declares three kinds of parameters.
  - **State** parameters: entity instances; the only valid effect targets.
  - **Input** parameters: any value type or entity; read-only.
  - **Context** parameters: any value type or entity (e.g. the acting user); read-only.

  Evaluation produces a proposed change set `ΔS` (list of `{param, field, old, new}`) from
  effects evaluated against `S`; postconditions and invariants are checked on
  `S' = apply(S, ΔS)`; `ΔS` is returned only if everything passes. No declared outputs in
  v0.1.
- **Rationale**: principle 2 (`T : S × I × C → Result<ΔS, O, Trace>`), clarification Q1.
- **DSL shape**: `def approve_invoice(invoice: Invoice, *, actor: Context[User],
  reason: Input[str])`. A bare entity annotation means state.

## R7. Evaluation order

1. **Input check**: every declared parameter present in the right section (state, input,
   context); every entity field present and of the declared type; no extra parameters or
   fields → otherwise `INVALID_INPUT`.
2. **Starting-state invariants**: every invariant whose entity matches a state parameter, on
   `S` → any false → `INVALID_STATE` (preconditions not evaluated).
3. **Preconditions** in declaration order; first false → `DENY`; error → `ERROR`; remaining are
   recorded as `skipped`.
4. **Effects**: all right-hand sides evaluated against `S`, giving `ΔS`.
5. **Postconditions** on `S'`, then invariants on `S'` → false → `DENY`; error → `ERROR`.
6. `ALLOW` with `ΔS`.

`&`/`|` short-circuit left to right. Derived values are memoized per phase (their values on `S`
and on `S'` differ) and recorded in the trace.

- **Rationale**: clarification Q5 — transitions preserve validity; they do not establish it.

## R8. Type system

- **Decision**:
  - Primitives: `Bool`, `Int` (i64), `Decimal` (exact, 28 significant digits), `String`.
  - Structural: `Option<T>` (`T` not itself an option).
  - Nominal: enums; `Id<E>` (every entity has an implicit `id: Id<E>` field, first in field
    order); declared nominal types `nominal Name over P with ops {…}` where `P` is a primitive
    and ops ⊆ {`order`, `add`, `scale`, `ratio`} (equality is always available).
  - Typing rules and operator table: [data-model.md](data-model.md#typing-rules).
- **Rationale**: clarification Q4 — the engine provides type construction; domains provide
  types (`Money = nominal Decimal with ops {order, add, scale, ratio}`).
- **Alternatives**: a built-in Money type (privileges one domain); multi-currency parameters
  (later, via parameterized nominal types).

## R9. Numbers

- **Decision**: `int` and `Decimal` only; `float` is rejected in field types, literals, and input
  data. Decimals are encoded as normalized plain strings (`"50000"`, `"0.05"`; never exponent
  notation or trailing zeros) in wire IR, requests, and records, and as the same normalized
  string inside the hash encoding. Division is exact up to 28 significant digits with
  banker's rounding (the Rust `rust_decimal` default), documented in the engine contract;
  division by zero is an evaluation error.
- **Implementation note**: Python must format with `format(d.normalize(), "f")` to avoid `5E+4`.

## R10. DSL construction in Python; typing in Rust only

- **Decision** (revised 2026-09-25):
  - Symbolic parameters are `EntityVar`/parameter objects; attribute access and operators call
    the Rust builder (R17), which type-checks each node as it is built and returns a typed node
    handle, or raises with the error code. Python turns that into `BehaviorTypeError` (or
    `BehaviorDefinitionError` for DSL misuse such as effects on read-only parameters) at the
    author's line.
  - Python keeps only what Python must do: decorators, tracing bodies, capturing source
    locations, operator overloading, `Expr.__bool__` raising (catches `if`, `and`, `or`, `not`,
    chained comparisons), and rejecting Python `float` values before they reach the engine.
  - `&`, `|`, `~` and `and_()`, `or_()`, `not_()`; `.in_([...])`, `.is_none()`, `.is_some()`,
    `.value_or(x)`, `none`; `Money(Decimal("100"))` literal, `Money(expr)` wrap,
    `underlying(expr)` unwrap; `requires`, `ensures`, `set_` — unchanged public API.
- **Rationale**: one type checker. The previous design duplicated the typing rules in Python
  and kept them aligned with a shared table; now there is nothing to align.
- **Alternatives**: Python checker plus engine re-check (the previous design; two
  implementations of the same rules).
## R11. When bodies are traced; derived references

- **Decision**: decorators register functions without running them; calling a derived value or
  rule in a body returns a derived-reference node by name. All bodies are traced when the
  `BehaviorModule` is compiled. The wire IR references derived values by name; the engine
  resolves names to hashes after cycle detection (R3).
- **Rationale**: allows forward references and makes cycles expressible so the engine can report
  them instead of Python failing with `NameError` or infinite recursion. For typing, the DSL
  needs the derived value's result type when it is referenced: bodies are traced in dependency
  order, and a reference to a derived value on a cycle gets an "unknown" placeholder type so the
  cycle still reaches the engine.

## R12. Python ↔ Rust boundary

- **Decision** (revised 2026-09-25): the PyO3 extension `behavior._engine` exposes engine
  objects, not JSON functions: a `Builder` (declares types and entities, builds typed
  expression nodes, adds derived values, invariants, and actions, and finishes into a module),
  typed `Node` handles, and an admitted `Module` (behavior version, items, canonical wire JSON,
  `evaluate`, `evaluate_intent`, `replay`). JSON appears only where data leaves or enters the
  engine: requests, records, intents, and the canonical serialization.
- **Native values** (revised again 2026-09-25): arguments and results cross the boundary as
  Python objects, not JSON strings: engine `Type` objects, parameter/field tuples, Python
  literal values (`Decimal` converted from its plain `format(d, "f")` form, `Enum` via its
  value, floats rejected), dicts for requests and results, `EngineError(code, message)` and
  `EngineIntentRejected(errors)` exceptions, and a `Record` object whose `json` is the canonical
  record text. JSON remains only where it is the artifact itself: wire files
  (`Module.from_wire`), the canonical serialization (`wire_json`), decision records (`Record.json`,
  `replay`), and the CLI.
- **Rationale**: Python is a binding to the engine, not a generator of JSON the engine happens
  to read. The CLI keeps its JSON-file interface (wire IR in, canonical JSON out).
- **Alternatives**: JSON in/out functions (the previous design).
## R13. Capability boundary and trust

- **Decision**: a structured intent carries only what an AI may choose: the capability name,
  input arguments, and **target ids** of the state entities it wants to act on. The trusted host
  supplies state, context (e.g. the authenticated acting user), and data version separately.
  The engine rejects the intent if any target id does not match the `id` of the supplied state
  entity (`TARGET_MISMATCH`), then evaluates exactly as a direct request.
- **Rationale**: principle 13 and constitution principle II — an AI must not be able to claim to
  be a manager or invent state. v0.1 has no data layer, so the host supplies state; the target
  check ties the AI's request to that state.
- **Alternatives**: intent carries state and context (an AI could forge the actor).

## R14. Decision records and replay

- **Decision**: a record contains `record_version`, `behavior_version`, optional
  `git_revision`, `data_version`, action name and hash, the full state/input/context used, the
  ordered trace (phase, readable expression, predicate hash, values read, outcome, source
  location), derived values used (name, hash, value, phase), result, reasons, and `ΔS`. No
  timestamps or host data. Records are emitted as canonical JSON (serialization, separate from
  hashing). Replay re-runs the stored request against the given module and compares bytes;
  a different `behavior_version` is a mismatch.
- **Rationale**: FR-011–FR-013, SC-006; byte-identity needs no clock in the output.

## R15. Toolchain and environment

- **Decision**: Rust stable pinned in `rust-toolchain.toml`, edition 2024; Python 3.13; maturin;
  Nix flake dev shell (host is NixOS with no Rust installed).
- **Crates**: `serde`, `serde_json`, `rust_decimal`, `sha2`, `thiserror`, `pyo3`, `clap`; dev:
  `proptest`, `pretty_assertions`. Python dev: `pytest`, `mypy`.

## R17. One construction path: builder and admission share the type checker

- **Decision**: the Rust `Builder` records wire-level declarations and expression trees, and for
  each new node runs the same type checker used by admission (`admit/typecheck.rs`) on that
  node; `Builder::finish` hands the collected module to the same admission pipeline as JSON
  (declarations → reference graph and cycles → type check → hashes). Wire JSON decoding
  produces the same structures and enters the same pipeline. Semantic IR is therefore built in
  exactly one place, whichever frontend is used.
- **Serialization**: the canonical wire JSON is produced by the engine from the admitted
  semantic module (`serialize.rs`): conversions appear explicitly (`some`, `to_decimal`),
  literals are folded, and declarations are listed by name. Only admitted modules can be
  serialized. Serializing and admitting again yields the same behavior version and the same
  bytes (round-trip property).
- **Forward references and cycles**: a reference to a derived value that is still being traced
  (a cycle) produces an untyped node; nodes above it skip the immediate check, and `finish`
  reports the cycle exactly as JSON admission does.
- **Rationale**: principle 9 — untrusted representations and the Python binding both pass
  through the same checks; there is no second way into the semantic model.

## R16. Deferred

- Formal verification (`behavior-verify` crate, Z3): consumes the semantic IR; can cache results
  by predicate/transition hash (R4).
- Per-object store (`objects/ab/ab78…`): storage change only.
- Permissions as their own predicate kind, declared outputs, repair/migration semantics,
  currency-parameterized nominal types.
