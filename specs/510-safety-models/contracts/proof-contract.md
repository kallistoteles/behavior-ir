# Contract: Generic Proofs and Concrete Instantiation

## 1. Versioned statement contract

Review and version the actual definitions of State, operation effects, frame behavior, R, P, Q,
policy authority and accepted transitions. The source/dependency identity and expected formal
type of every exported theorem are proof inputs. A theorem name or source-file hash alone
does not show that its proposition expresses the required guarantee.

The abstraction uses typed finite entity maps with lifecycle and owner projections. Operations
have defined assignment/create/remove semantics; unaffected projections satisfy frame lemmas.
R requires an existing parent for every child. P requires a permitted parent for every
nonterminal child. Q is the conjunction across all configured dependencies.

FO-1 establishes structural permission completeness and the effect/change-witness connection.
LD-1 establishes `Q(S) and T_a(S,S') implies Q(S')` for every supported ordinary transition
and arbitrary finite state cardinality. Initial validity is a separate premise. No theorem may
assume the desired preservation conclusion instead of proving its operation cases.

## 2. Finite concrete checking

For each exact admitted module, encode one closed ModuleData containing all actions and all
assignment/create/remove occurrences, resolved types/bindings, full mandatory precondition
roots, literal/guard probe descriptors and complete policy set. Include actual BehaviorHash,
Core release and canonical inventory identities in the data referenced by the theorem.

The reflective checker must:

1. Validate unique actions, well-formed references and exact identity/data bindings. Rust
   recomputes canonical tagged hashes through public Core APIs; Lean checks their equality/
   association with the concrete inputs, not a second implementation of Core hashing.
2. Visit all actions × policies, deriving relevance rather than accepting a relevance flag.
3. Check every applicable creator/writer/remover restriction, including assignments with
   unchanged values and authorizations from multiple intersecting policies.
4. Derive complete dependency footprints and count distinct relevant operations across their
   union. Creation is one; two assignments are two; FO-only writes are checked independently.
5. Check every affected dependency, immutable ownership, literal destinations, mandatory guard
   roots and correct binding/effect relationships. Reject unsupported mixed roles/effects.
6. Supply a frame argument for actions/effects classified irrelevant to each property.

For data m, require a kernel-checked `checkModule m = true` proof and a concrete theorem applying
generic soundness to m. Kernel computation checks finite premises, not an enumeration of all
states. A false/incomplete/unsupported instance cannot be accepted by a fallback axiom.

Compare encoded action IDs and every operation occurrence independently against the actual
admitted module before issuing evidence. This detects omission mutations; it is still unverified
Rust, not a kernel proof of extraction completeness.

## 3. Supported operation and guard profile

Let `member(x, values)` use the configured enum-member set. For checker 2, recognized ordered
vectors may have any permutation/repetition of exactly that set; the release contract below
requires explicit witnesses and unchanged actual root identity. Empty membership uses an
explicit false literal and its negation an explicit true literal; demonstrate admitted hashes
for these forms in preflight. Do not assume Core accepts or simplifies an empty membership.

| Operation | Required premises beyond ordinary Core validity and FO authority |
|-----------|---------------------------------------------------------------|
| Parent status → prohibited literal | Actual mandatory root `not(any(select Child, c.owner == parent.id AND not(member(c.status, terminal))))` for every dependency. |
| Parent status → permitted literal | Literal classification; one-operation/frame checks. |
| Child status → nonterminal literal | Actual State parent binding; mandatory roots `child.owner == parent.id` and `not(member(parent.status, prohibited))`. |
| Child status → terminal literal | Literal classification; immutable owner and frame checks. |
| Child creation | Owner initializer exactly bound State parent's ID; literal status; mandatory permitted-parent root when nonterminal. |
| Parent creation | Literal status; fresh identity; source validity and one-operation frame prove no existing child owned this previously absent parent. |
| Child removal | One operation; remaining children/projections unchanged; all Core Ref constraints hold. |
| Parent removal | Full resulting-state Ref integrity excludes every surviving dependent, terminal children included. |
| Irrelevant operation | Mechanically checked projection/frame preservation for every dependency. |

Each actual required precondition is a whole root returned by the admitted action. A matching
subexpression under OR, implication, negation or another weakened condition is not sufficient.
Additional preconditions are permitted because they conjoin with the required roots. No arbitrary
equivalence search is required. Computed lifecycle values, scalar-ID creation owners, optional
relationships, owner assignments and unrecognized forms do not enter this proof profile.

One parent operation shared across dependencies can satisfy several separate guards. An
operation that affects both parent and child roles lacks the single-role preservation rule
and is Unsupported even if represented by one assignment or creation.

## 4. Public-Core correspondence preflight

Use public semantic accessors and Core Builder operations. The candidate method reconstructs
the declaration context in a fresh scratch Builder, verifies declaration hashes/Ref markers,
and admits expected expressions as derived bodies with matching entity captures. Their body
hashes are compared to actual mandatory roots, RHSs and owner initializers.

State-role checking is separate from expression hash matching. Compare public returned roles
to a known State assignment target or an admitted scratch State-role anchor. Input/Context
entities must not supply purported Store lifecycle values. Probe actions/definitions are never
emitted into the authored module. Do not use private enums, Debug parsing, discriminant constants,
serialized guard AST classification or copied Core hash algorithms. The release-readiness
contract permits bounded generic string-array discovery only as non-authoritative probe inputs.

The preflight must demonstrate all supported rows and rejection of changed binding/field/type/
literal, owner initializer, query source, weakened guard, role and mixed-effect controls.
Builder::for_module().finish() does not preserve original declarations; do not assume it does.
If a required public operation is unavailable, stop and report it before dependent implementation.

Passing preflight tests does not mechanically verify the implementation's correspondence.

## 5. Explicit trusted correspondence

Certificates identify these separately from proved results:

- Core admitted semantics and atomic commit/reference-integrity contracts.
- Rust inventory extraction completeness and target/binding/role resolution.
- Core-built canonical probe descriptors faithfully representing the mathematical guards;
  semantic hash correspondence/collision assumptions.
- Correct encoding of extracted data into the proof input and evidence binding.
- Correct Rust snapshot-property evaluation and export metadata binding.
- Pinned compiler/kernel/auditor/build environment integrity.

Lean verifies the encoded transition model. It does not independently interpret opaque Core
expression hashes, prove the Rust extractor, attest a dishonest backend or authorize commits.

## 6. Proof integrity and deterministic encoding

Use the expected locked Nix Lean 4.30.0 derivation and bundled libraries. Keep compilation/import
search paths controlled; build from pinned repository sources and fixed generated wrappers.
Verification regenerates the concrete wrapper from validated data. Never execute supplied
bundle Lean code or load its untrusted compiled modules as a substitute for that regeneration.
Authored names/hashes become validated data bytes, never source identifiers, commands or imports.

For every generic/concrete root:

1. Check its expected fully qualified declaration and formal proposition over the exact data.
2. Collect transitive axiom dependencies structurally; allow only propext, Classical.choice,
   Quot.sound. Reject sorryAx, custom and native-computation axioms.
3. Run pinned `leanchecker --fresh` on the corresponding proof module and import closure.
4. Bind source/dependency/type/data/tool/checker identities and successful results in evidence.

Fresh replay uses Lean's kernel and permits axiom declarations; the axiom audit is mandatory
alongside it. No native-oracle substitution for finite kernel checking. Explicit resource
exhaustion produces Inconclusive. Missing tools, mismatched derivations or incomplete checks
never produce Certified under assumptions.

## 7. Proof and correspondence mutation controls

Remove each essential theorem premise and exhibit a violating operation/state; require its
proof obligation to fail. Mutate concrete permissions, roots, binding/owner equality, literals,
Ref metadata, operation counts and inventory membership; certification must fail closed.

Test proof holes, arbitrary/native axioms, wrong theorem/data targets, stale bundle metadata
and injected authored names. Test useful positive transitions; safety of an always-refusing
action alone cannot satisfy the supported reference cases.

## 8. Implemented proof decomposition

`Data.lean` and `FieldOwnership.lean` retain the ownership operation/map contract.
`LifecycleData.lean` defines the proof-only parent/child projections and R/P; these
are mathematical functions over Core coordinates, never an additional runtime store.
`LifecycleDependency.lean` proves each operation case, configured-policy conjunction,
projection frames and source-guard sequence preservation with at most one relevant operation.
`LifecycleCheck.lean` derives finite classifications from typed operation coordinates,
literal hash/member tables and raw prohibited/terminal member sets, State bindings and whole precondition roots. `LifecycleSequence.lean`
proves the bound for each dependency from the union footprint and applies preservation
to the complete ordered operation projection. `Check.lean` links that annotation to the
full FO inventory and checks ownership policy coverage and immutable owner permissions.

`ProbeSemantics`, `OrdinaryCoreConditions` and projected execution equality are explicit
correspondence premises, not axioms or conclusions disguised as assumptions. They state
canonical guard meanings, Core freshness/Ref contracts and the projection of actual effects.
They do not assume R/P preservation. Physical parent removal uses only Core's resulting-state Ref integrity R; a separate kernel-checked lemma derives the
no-surviving-reference premise under the isolated-operation frame. It does not assume P(S'). The finite checker proves recognition/binding/placement;
Rust/public-Core probe construction and encoding remain trusted for that association.

The module theorem is conditional on initial R/P, every actual source precondition and
these named correspondence premises. Its action proof evaluates all guards in the same
source state. Other effects must satisfy the proved projection frame and cannot introduce
a second relevant operation. A successful proof does not establish host module authorization,
genesis validity or migrations.

## 9. Release-readiness guard correspondence

[Release corrections](release-readiness.md) narrow the original serializer restriction solely
for discovering candidate literal orders. Core-admitted whole-root matching, actual State roles,
exact binding/effect linkage and kernel-checked concrete premises still establish acceptance.
Encode ordered members and their required policy set; machine-check Boolean membership
equivalence under exact set equality, including duplicates. Keep that lemma/check in the audited
dependency closure. Do not replace the actual root with a sorted hash or weaken a theorem.

The public serializer supplies candidates, not a semantic AST classification. Spurious hints
cannot certify; incomplete hints may refuse certification. Discovery completeness affects
availability. Actual expression meaning still depends on the named public Core construction/hash
correspondence. Resource limits and every non-successful required obligation remain blocking.
