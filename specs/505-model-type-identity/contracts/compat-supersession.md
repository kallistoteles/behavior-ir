# Contract: Explicit supersession in the compatibility gate (feature 505)

Extends `scripts/check-models-compat.py` (features 501–503). Three concepts are kept apart:

- **Identity**: a version tuple always yields the same recorded hashes.
- **Supersession**: a new tuple explicitly takes over a removed tuple's reference cases.
- **History**: a superseded tuple's key stays documented in its replacement; it is no longer an
  active compatibility requirement.

## 1. Unchanged rules

1. **Reproduction.** Every reference case, rebuilt now, reproduces its current tuple's recorded
   entry.
2. **Immutability.** An established tuple (present at the merge base with `main`) that is still
   in the manifest stays byte-identical.
3. **Coverage.** The current tuples cover every reference case, each under exactly one tuple.

## 2. Declaring supersession

A tuple may carry `supersedes`: the list of tuple keys it replaces, each written in full as the
key fields of the removed tuple. The keys are `model_kind`, `model_semantic_version`,
`lowerer_version`, `core_release`, and `embedded` and `co_supplied` where present. The list is
sorted by the gate's canonical key order. A replacement records its own cases with their own
golden hashes. Old and new hashes are not expected to match.

## 3. Removing an established tuple

An established tuple `T₁` may be absent from the manifest only if:

- **(a)** its Core Release is no longer pinned (unchanged from before), or
- **(b)** exactly one current tuple `T₂` lists `T₁`'s key in `supersedes`, and all of these hold:
  1. **Same kind and Core.** `T₂.model_kind = T₁.model_kind` and
     `T₂.core_release = T₁.core_release`, the pinned release.
  2. **Higher version.** `v(T₂) ⪰ v(T₁)` and `v(T₂) ≠ v(T₁)`. The version vector `v` is
     `model_semantic_version`, `lowerer_version`, and the `model_semantic_version` and
     `lowerer_version` of each `embedded` and `co_supplied` model, matched by kind. If the two
     tuples do not have the same set of embedded and co-supplied kinds, the vectors are
     incomparable and the supersession is invalid.
  3. **No lost case.** Every case of `T₁` has a corresponding case in `T₂`: `∀c ∈ Cases(T₁):
     ∃c′ ∈ Cases(T₂): LogicalCase(c′) = LogicalCase(c)`. A logical case is the reference-case
     name, the key in `models/fixtures/reference_cases.py`, which is the same in both tuples.
     Only the recorded hashes may differ.

Any other absence fails: `established tuple … was removed` when no replacement names it, or
`invalid supersession of … by …: <condition>` naming the failed condition.

A `supersedes` entry naming a tuple that is still present fails. A **newly introduced** entry
(one that the same tuple did not already list at the merge base) must also name a tuple
established at the merge base, and the tuple listing it must be a tuple of the current build,
so the cases it takes over are rebuilt and checked under rule 1. Entries already accepted at the
merge base are history: their predecessors are gone there too, so they are not validated again.
The `supersedes` list of an established tuple is part of what rule 2 keeps byte-identical.

## 4. Tooling

`scripts/check-models-compat.py --write --supersede` records the current tuples that are new.
For each new tuple, it removes the established tuples that the new tuple validly supersedes
under §3(b) and lists their keys in its `supersedes`. Predecessors resolve against the merge
base. A branch-local tuple that the new tuple replaces (a second version bump before the merge)
is dropped, and the established predecessors it superseded carry over to the new tuple. If one
of them cannot be validly superseded by the new tuple, the writer fails instead of writing a
manifest that the gate would reject. A tuple that is new but supersedes nothing
is recorded without the field. Plain `--write` never removes a tuple.

## 5. Consequences

- A hash never changes under an unchanged tuple. Rule 2 still compares every remaining
  established tuple byte for byte.
- A lowering or identity change without a version change still fails: the case reproduces
  differently under its existing key (rule 1).
- No reference case can disappear through supersession (§3(b)3 together with rule 3).
- Lowering a version, changing kind, or changing Core Release is never a supersession.

## 6. Tests (`scripts/tests/test_models_compat.sh`)

| Case | Expected |
|---|---|
| v1 tuple removed; a v2 tuple of the same kind and Core lists it in `supersedes` and has all its cases | OK |
| v1 tuple removed; no tuple lists it | FAIL `was removed` |
| v1 tuple removed; the v2 tuple lists it but lacks one of its cases | FAIL `invalid supersession`: a lost case |
| v1 tuple removed; the listing tuple has another kind | FAIL `invalid supersession`: kind |
| The listing tuple's vector is higher in one component and lower in another | FAIL `invalid supersession`: version |
| The listing tuple's vector equals the removed tuple's | FAIL `invalid supersession`: version |
| An approval tuple removed; the replacement has higher `co_supplied` versions and the same cases | OK |
| `supersedes` names a tuple that is still present, or one never established | FAIL |
| An established tuple edited in place | FAIL `was changed` (unchanged) |
| `--write --supersede` on a version bump | Records the v2 tuple with `supersedes`, removes v1, and the gate then passes |
| After the merge: `main` holds v2 with its accepted `supersedes` history, unchanged | OK |
| An established tuple's `supersedes` history edited | FAIL `was changed` |
| A tuple that is not built supersedes v1 (hiding that the built tuple lost a case) | FAIL `not a tuple of the current build` |
| Two bumps before the merge (v1 → v2 → v3 with `--write --supersede` each time) | v3 supersedes v1, v2 is gone, and the gate passes |
