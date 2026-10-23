# Contract: Mathematical Soundness and Correspondence

## Required implication

For exact admitted module M, complete expected policy set P and arbitrary proposed data w:

`Check(M, P, w) = true -> SafetyGuarantee(M, P)`.

This is a planned machine-checked checking contract, not an already implemented Lean declaration.
`SafetyGuarantee` includes structural FO permissions and conditional LD preservation across every
covered ordinary action. It contains no assumption of producer correctness or earlier acceptance.
If a direct checker is selected, the supplied auxiliary data may be empty; independently required
coverage and checking soundness are unchanged.

## Interpretation and coverage

A changed representation needs a machine-checked interpretation/refinement preserving the intended
finite semantics. Define coordinates for all actions, operation occurrences and policies, and bind
concrete input to the exact request. Equivalent chunk layouts need not share proof syntax; all
semantic obligations and identity relationships must survive the versioned mapping.

Independently check action/operation coverage against admitted Core and the complete expected policy
set. Reordered auxiliary indices cannot change Core order or permission bindings. Every applicable
restriction is conjunctive; creator, writer and remover rules are independent. Preserve duplicate
input rejection and same-value/unreachable assignment permissions.

Any uniqueness replacement must imply full original-key uniqueness, preserving multiplicities.
Non-strict sortedness, set equality, equal length, truncated keys and an assumed collision-free hash
are insufficient. Ordering/membership components need their actual laws and checked application
bridges. Do not use a theorem that assumes distinctness to establish distinctness circularly.

## Trusted boundary

| Layer | Assurance / remaining assumption |
| --- | --- |
| Public Core admission/transition semantics | Existing Core authority and declared integrity assumptions. |
| Rust inventory extraction and independent comparison | Trusted implementation correspondence, checked by independent tests; not Lean-verified by implication. |
| Canonical guard/binding probes | Explicit correspondence assumption to actual admitted precondition placement and effects. |
| Rust encoding into the finite model | Explicit trusted bridge unless separately verified; checked new representation refinement must not hide an unverified decoder. |
| Generic checking/refinement and safety theorems | Machine-checked statements, reviewed/versioned for intended meaning. |
| Concrete premises and identity/type wrapper | Checked for exact admitted request under the explicit correspondence assumptions. |
| Producer/search | Untrusted; correctness not part of checking soundness. |
| Proof tools/audit infrastructure | Pinned trusted components; fresh replay is another pass with the pinned kernel, not a separate kernel implementation. |

Lean identity equalities and content hashes do not establish that Rust extracted Core semantics
correctly. State this distinction in both evidence and documentation. Genesis, migrations, complete
snapshot validity, approved-module enforcement and exact activation continuity remain separate
obligations. Safety evidence grants no governance authorization.

## Integrity and compatibility

Inventory the twelve baseline audited roots by semantic obligation. Preserve all their conclusions
and add selected checker-soundness/representation obligations. New root names and proof targets are
allowed only with an explicit reviewed versioned mapping; exact old syntax is not the invariant.

Every required generic/concrete/bridge theorem must pass intended-type checks, transitive axiom
audit and `leanchecker --fresh` over the actual complete dependency closure. Only `propext`,
`Classical.choice`, `Quot.sound` are permitted. Reject `sorryAx`, custom/native axioms, weakened
unrelated statements, absent audit rows and missing tools/artifacts. Never use native Boolean
execution, test success, checksum identity or cached acceptance as mathematical proof.

Preserve all supported v1 constructions and premises. Assess language compatibility, proved
completeness relative to the contract and practical bounded capacity separately. Do not claim a
universal completeness result from passing regression tests.

## Required controls

Actual mutations must target the imported proof being checked, with positive parent controls:
remove/weaken each essential premise, omit action/operation/policy, change identity, break key laws
or indices, introduce duplicate keys, authorize one operation through only one overlapping policy,
remove/wrongly bind a lifecycle guard, insert a proof hole or weaken the final type. Audit failures
and kernel failures both block certification. A mutated harness accidentally importing the original
is an invalid observation, not a passed negative control.

## Integrated checker-3 refinement

`Finite.lean` interprets arbitrary untrusted merge trees against original full keys. The trace
permutation theorem preserves multiplicity; strict adjacent comparison with pinned transitivity
establishes global uniqueness. No correctness axiom about the Rust producer occurs.
`Check.lean` reconstructs the original `checkModule` predicate from uniqueness and unchanged
bounds/authority/lifecycle premises. All baseline safety claims remain their original types.
The UTF-8 flatten/injectivity lemmas establish representation properties without certifying
Core extraction or a JSON decoder. Expected anchors are regenerated independently from the
consumer's original admitted module and complete policies.

All sixteen root types/allowed transitive axioms are checked. Fresh replay imports the complete
actual proof closure under pinned Lean; this is fresh kernel verification, not an independently
implemented alternative kernel. Narrow imports and noncomputable proof data remove unrelated
code-generation work, never a required mathematical obligation. Core semantics, Rust extraction,
canonical probes, fixed source generation and build integrity remain trusted correspondence.
