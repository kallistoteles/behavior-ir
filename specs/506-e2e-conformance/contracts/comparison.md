# Contract: Authoring and execution comparison (feature 506, FR-016)

Two separate obligations, each with its own normalization, reported as C16a and C16b.

## 1. Authoring conformance (C16a)

**Inputs**: the reference workflow, constructed independently through:
- the Python binding (`models/examples/purchase_approval/workflow.py`);
- the Rust model authority (`crates/behavior-models/tests/e2e_reference.rs`).

Both are written from data-model §1. Neither reads the other's output.

**Normalization `N_A`** maps an admitted module and its receipts to the authoring identity:

```text
{ behavior_hash,
  items:     { item name → Core item hash }        // every admitted item
  models:    { model id → {kind, model_semantic_version, lowerer_version, model_hash} },
  generated: { approval id → generated enum, entity and derived hashes } }
```

All of these are Core's or the model authority's canonical hashes. They are compared exactly,
as canonical JSON. Excluded are:
- source locations, which are outside semantics and checkout-dependent until core#31 (fixed in Core 0.14.0, adopted by feature 508);
- display names and descriptions, which are outside semantics.

Nothing semantically meaningful is excluded: every declaration, action, derived item, read and
command is covered by its item hash.

**Pass**: `N_A(Python) = N_A(Rust)`. A missing half is `unsupported`.

## 2. Execution conformance (C16b)

**Inputs**: one Python-authored admitted module, given to both paths as the same wire bytes
(`to_wire_json()`). For each compared operation, the request is identical. It is either:
- an evaluation request `{data_version, state, input, context, facts}` with a complete parent
  snapshot; or
- an invocation document with its snapshot.

**Paths**:
- the Python engine interface: `evaluate(...)` → `Decision.record_json`, and `invoke(...)` →
  `InvocationRecord`;
- the released CLI in a subprocess: `behavior eval WIRE REQUEST` and
  `behavior invoke WIRE INVOCATION SNAPSHOT`.

**Normalization `N_E`**: the record parsed as JSON and re-serialized canonically (sorted keys,
no insignificant whitespace). Records of the command profile carry no source diagnostics, so
nothing else is removed. For a refusal before evaluation (invalid input or an invalid
snapshot), `N_E` is `{error: code}`: the Python `BehaviorError` code, or the CLI's structured
error code.

**Pass**: `N_E(Python) = N_E(CLI)` for every compared operation, allowed and refused alike. The
`record_id`s are equal as a consequence.

## 3. What these comparisons do not show

Both execution paths run the same Core engine, so agreement does not prove Core correct. The
expected outcomes of `reference-workflow.md` remain the oracle. C16a and C16b are reported
separately so that a failure is localized: authoring for C16a, consumption for C16b.
