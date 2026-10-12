# Contract: Model Identity, Receipts and Source Maps

**Feature**: 501

## 1. Definitions

```text
ModelHash(M)    = tagged_hash("behavior.model.v1", CanonicalModelDocument(M))
BehaviorHash(B) = B.behavior_version()            // Core's module hash, "sha256:…"
```

`tagged_hash` and `to_canonical_string` are Core's public canonical functions (research R1, R5):
sorted object keys, no floats, UTF-8, `SHA-256(tag ‖ 0x00 ‖ canonical JSON)`. The output is
displayed as `sha256:<hex>`.

## 2. CanonicalModelDocument (StateMachine, semantic version 1)

Every field below is semantic and enters the hash. Nothing else does.

```json
{
  "format": "behavior.model.state_machine.v1",
  "model_kind": "state_machine",
  "model_semantic_version": 1,
  "model_id": "purchase_request",
  "subject": { "entity": "PurchaseRequest" },
  "field":   { "name": "status", "enum": "RequestStatus", "enum_hash": "sha256:…" },
  "subject_param": "purchase_request",
  "entry_states": ["Draft"],
  "transitions": [
    {
      "id": "approve",
      "from": ["InReview"],
      "to": "Approved",
      "inputs":  [ { "name": "reason", "type": <WType as canonical JSON> } ],
      "context": [ { "name": "context", "type": <WType as canonical JSON> } ],
      "guards":         ["sha256:…"],
      "effects":        ["sha256:…"],
      "postconditions": ["sha256:…"],
      "commands":       ["sha256:…"]
    }
  ]
}
```

Rules:

| Fact | Encoding | Order |
|---|---|---|
| subject entity | name only. Unrelated changes to the entity's other fields do not change the model's identity (they do change BehaviorHash). | — |
| owned field's enum | name plus Core enum item hash from `module.name_table()` | — |
| entry states, `from` | arrays of case names | sorted (code point) |
| transitions | array of objects | sorted by `id` |
| inputs, context | arrays of `{name, type}` | sorted by name: they are mappings, and declaration order is never semantic |
| guards, effects, postconditions, commands | arrays of Core semantic hashes of each fragment | author order (sequences, research R6) |
| types | `WType` serialized as canonical JSON | — |

**Fragment identity**: each entry is the `hash()` that Core computed for that fragment in the
admitted generated action, read at the layout positions of the lowering contract §3:

- guards: `preconditions()[1..]`
- postconditions: `postconditions()[1..]`
- effects: `effects()[1..]`
- commands: `command_emissions()`

Core's hash covers semantic content only, so authoring syntax, binding variable names (which
never reach Rust, research R3) and locations cannot affect it.

**Explicitly excluded**: display names, descriptions, comments, documentation, source files and
lines, editor metadata, input order of unordered sets, the binding's identity, the lowerer
version, generated action names (derived from the ids) and the source map.

The enum hash binds the model to the owned field's type. Because `Q` is derived from `T_f`, the
hash commits to the enum's identity, not a copy of its cases. That the field exists with that
type is checked at resolution (`STATE_FIELD_NOT_ENUM`).

Law (lowering contract L7): `ModelHash(M₁) = ModelHash(M₂) ⇒ BehaviorHash(L(M₁)) =
BehaviorHash(L(M₂))` for fixed `(K, s, l, c)` and identical ordinary behavior. This holds
because every input to the generated actions is a function of the document plus `(l, c)`. The
converse is not required.

## 3. Lowering receipt

One receipt per model per build (data-model §9). It includes the canonical document itself (`model`), so an auditor can re-derive `model_hash` from the receipt alone. The receipt is a plain JSON document
(`behavior.model_receipt.v1`), returned by `compose` and exposed by the binding. It is never
added to the Behavior module, never stored by Core, and never part of any Core hash.

`behavior_hash` is the hash of the whole composed module, which the model is part of. It can be
reproduced from `model_hash`, the versions and the core release together with the module's
ordinary behavior.

## 4. Source map

```text
SourceMap.entries: sorted by (core.kind, core.name or entity.field)
  { core:  { kind: "action", name: "purchase_request__approve", item_hash: "sha256:…" },
    model: "purchase_request/transition/approve",
    display: { display_name: "Approve request" },
    source:  { file: "procurement.py", line: 42 } }
  { core:  { kind: "field", entity: "PurchaseRequest", field: "status" },
    model: "purchase_request/state_field" }
```

Source locations are relative and never depend on the checkout location (Constitution IV): they
are relative to the build `root` when one is given (as Core relativizes module locations), and
otherwise to the common directory of the model's own source files.

Obligations:
- **Total**: every generated action has exactly one entry, and every model transition maps to
  exactly one generated action (SC-005).
- **Provenance only**: the source map is produced after the module is finished. It never feeds
  back into the module, ModelHash or BehaviorHash. A test mutates only display and source data
  and asserts both hashes are unchanged (law L9).
- **Explanation**: tooling may map a Core denial on `purchase_request__approve` (first
  precondition false) to "approve is not allowed from Draft". Core records keep referring to
  Core objects only.

## 5. Determinism of outputs

Receipts, source maps and error lists are emitted as canonical JSON (sorted keys, sorted
arrays as specified). Two builds of equal inputs produce byte-identical receipts. This is
checked by `scripts/determinism-check.sh`.
