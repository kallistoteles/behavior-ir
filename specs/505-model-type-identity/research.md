# Research: Declaration identity of named types (feature 505)

## R1. One shared implementation of the identity rule

**Decision**: Move Approval's helpers from `approval/identity.rs` to the kind-neutral
`crates/behavior-models/src/identity.rs`, and delete the name-only `params()`. The helpers are
`named_types` (the named types a `WType` mentions), the typed parameter list `[{name, type,
types}]` and the entity reference `{entity, entity_hash}`. StateMachine, RequestFlow and
Approval all call the moved helpers.

**Rationale**: FR-003 requires one rule with one authoritative implementation. Approval's
helpers already produce the representation the spec adopts, so moving them keeps Approval's
document byte-identical by construction. `params()` has no other caller once StateMachine and
RequestFlow switch.

**Alternatives considered**:
- Copy the helpers into each kind. Rejected: parallel implementations of one rule.
- Keep `params()` and add `types` beside it. Rejected: two representations of a parameter.

## R2. Where declaration identities come from

**Decision**: Read them from the admitted module's public `name_table()`, keyed by
`(kind, name)`, and match the kind through `Kind::as_str()` (`"enum"`, `"nominal"`,
`"entity"`). `behavior_engine` does not re-export `Kind` itself. Entity identities equal
`behavior_engine::schema::declarations(module)`, the hash a store binds its schema to.

**Rationale**: FR-004 requires Core's own identities and no ecosystem hashing. The name table
holds exactly one hash per `(kind, name)`. A reference resolves when the table holds its name
under the expected kind. It fails when the name is missing or exists only under another kind,
which is the spec's "inconsistent" case. Ambiguity cannot arise: Core admits one declaration
per name and kind.

**Alternatives considered**:
- Hash the declarations in `behavior-models`. Rejected by FR-004.
- Copy declarations into the document. Rejected: a copy is not a reference, and Core's identity
  already covers the declaration's whole semantic content.

## R3. What is referenced

**Decision**:
- **StateMachine**: the subject entity becomes `{entity, entity_hash}`. Every transition input
  and context parameter gets `types`. The state field's enum keeps its existing `enum_hash`.
- **RequestFlow**: the actor becomes `{entity, entity_hash}`. Each state binding becomes
  `{name, entity, entity_hash}`. Payload fields and every creation input and context parameter
  get `types`.
- **The embedded machine** follows the StateMachine rule. Its subject is the flow's generated
  request entity, whose identity follows from the payload.

**Rationale**: This is the clarified scope (spec, Clarifications 2026-10-16): every type
reference, in the same shape Approval uses (FR-011).

**Alternatives considered**: References to parameter and payload types only, which was option B
in the clarification and was rejected there.

## R4. Version and format changes

**Decision**:
- `STATE_MACHINE_SEMANTIC_VERSION` and `REQUEST_FLOW_SEMANTIC_VERSION` become 2. Both lowerer
  versions stay 1.
- The document formats become `behavior.model.state_machine.v2` and
  `behavior.model.request_flow.v2`.
- Approval keeps `behavior.model.approval.v1` and its versions.

**Rationale**:
- The semantic version names the identity function, which changes (FR-007).
- The lowerer version names the lowering function, which does not change (FR-008).
- The format tag names the document schema. New fields and a changed `subject` and `actor` shape
  are a schema change, so a reader can tell the shapes apart without consulting versions.
- Approval's schema is unchanged.

**Alternatives considered**:
- Bump the lowerer versions too. Rejected: they would claim a lowering change that did not
  happen.
- Keep the `.v1` format tags. Rejected: two different schemas would share one tag.

## R5. Replacing the v1 tuples: explicit supersession in the gate

**Decision**: The v1 tuples are replaced, not kept. Supersession is an explicit, verifiable
contract (contract `compat-supersession.md`). A higher version alone does not count as evidence
of replacement.

- **Declaration**: a new tuple lists the keys it replaces in `supersedes`.
- **Removal**: an established tuple may be absent only if its Core Release is no longer pinned,
  or if exactly one current tuple supersedes it. That tuple must have:
  - the same model kind and pinned Core Release;
  - a version vector componentwise ≥ the old one and different from it, compared over the
    semantic version, the lowerer version, and the embedded and co-supplied versions by kind;
  - every logical reference case of the old tuple, identified by its reference-case name.
- **Hashes**: the replacement establishes its own golden hashes. Old and new hashes are not
  expected to match.
- **What does not change**: unchanged tuples stay immutable. A removal without a valid
  replacement fails, and so does a `supersedes` entry that names a tuple still present or never
  established.
- **Tooling**: `--write --supersede` records new tuples together with their `supersedes`. Plain
  `--write` never removes a tuple.

**Rationale**:
- FR-009 and the no-backward-compatibility policy (docs/versioning.md) ask for one active tuple
  per case, with no obsolete v1 tuples kept for compatibility.
- The gate must still prove that no reference case disappears unnoticed and that no hash
  changes under an unchanged tuple.
- Keeping identity (one tuple, one set of hashes), supersession (an explicit takeover of named
  cases) and history (the superseded keys recorded in `supersedes`) apart keeps both properties.
- `test_model_staging.py` assumes one tuple per case.

**Alternatives considered**:
- Implicit supersession by a higher version vector. Rejected (review 2026-10-16): a higher
  version does not show that the cases were taken over.
- Keep the v1 tuples as active records. Rejected: they are not reproducible, they duplicate
  cases, and they contradict FR-009.
- Delete the immutability rule. Rejected: it is the gate's purpose.

## R6. Approval tuples

**Decision**: The approval tuples' `co_supplied` versions change (`state_machine` 2,
`request_flow` 2), so the gate computes new keys. Their cases are recorded with byte-identical
ApprovalHashes, generated hashes and BehaviorHashes. The old approval tuples are removed under
the R5 rule.

**Rationale**: FR-003 and FR-009, and the existing `co_supplied` design from feature 503.

## R7. Evidence that lowering is unchanged

**Decision**: For every reference case, the v2 tuple's `behavior_hash` and generated hashes must
equal the v1 values. This is checked in the same change by comparing the manifest before and
after (quickstart 4) and recorded in the implementation log. The existing determinism gate keeps
running.

**Rationale**: FR-008 and SC-003. The gate cannot compare across tuples by itself, so the
comparison is part of the change's evidence.

## R8. Documentation

**Decision**:
- **`models/README.md`, "Model identity":** state that a model's identity depends on the exact
  declarations it references (subject, actor, state bindings, parameter and payload types), and
  on nothing else in the module.
- **Earlier specs:** 501 and 502 stay as historical records and are not edited.
- **This feature's contracts:** they hold the current rule.

**Rationale**: The README is the maintained description of the model rules. Feature
directories record decisions at their time.
