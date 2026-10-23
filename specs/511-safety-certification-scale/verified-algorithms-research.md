# Verified library algorithms for the certification proposal

Date: 2026-10-20. Scope: read-only inspection of the installed Lean 4.30.0 sources at `/nix/store/7m5ds60a41w276hsx3rybv76x3609l6k-lean4-4.30.0/src/lean`. No candidate implementation, proof, test, or performance measurement was run for this note. The source supplies verified components; their suitability and speed in this certificate pipeline remain separate questions.

## Sorting and preservation of the exact hashes

Lean already provides `List.mergeSort`, its correctness lemmas, lexicographic comparison of lists, and the relevant order laws. A custom sorting algorithm is unnecessary to establish the proposed mathematical route.

| Declaration | Exact useful guarantee / obligations |
| --- | --- |
| `List.mergeSort` | Stable list sorting by a Boolean comparison function. The logical definition is intentionally simplified and uses well-founded recursion. [Source](</nix/store/7m5ds60a41w276hsx3rybv76x3609l6k-lean4-4.30.0/src/lean/Init/Data/List/Sort/Basic.lean:66>) |
| `List.mergeSort_perm` | For **any** comparator `le`, `mergeSort l le ~ l`. No ordering-law hypothesis is needed for this permutation theorem. Multiplicities are preserved. [Source](</nix/store/7m5ds60a41w276hsx3rybv76x3609l6k-lean4-4.30.0/src/lean/Init/Data/List/Sort/Lemmas.lean:289>) |
| `List.pairwise_mergeSort` | Produces `Pairwise le` only given Boolean comparator transitivity and totality: `∀ a b c, le a b → le b c → le a c` and `∀ a b, le a b || le b a`. This is a non-strict sortedness theorem, not a uniqueness theorem. [Source](</nix/store/7m5ds60a41w276hsx3rybv76x3609l6k-lean4-4.30.0/src/lean/Init/Data/List/Sort/Lemmas.lean:307>) |
| `List.Perm.nodup_iff`, `List.Perm.nodup`, `List.Nodup.perm` | Transfer `Nodup` along a proved permutation. No comparator assumptions. [Source](</nix/store/7m5ds60a41w276hsx3rybv76x3609l6k-lean4-4.30.0/src/lean/Init/Data/List/Perm.lean:527>) |
| `List.nodup_iff_pairwise_ne` | `Nodup l ↔ Pairwise (· ≠ ·) l` is definitionally true (`Iff.rfl`). This reaches the exact existing hash uniqueness proposition. [Source](</nix/store/7m5ds60a41w276hsx3rybv76x3609l6k-lean4-4.30.0/src/lean/Init/Data/List/Pairwise.lean:282>) |
| `List.Pairwise.imp` | Maps a pairwise relation to a weaker relation, useful for strict order implying inequality. [Source](</nix/store/7m5ds60a41w276hsx3rybv76x3609l6k-lean4-4.30.0/src/lean/Init/Data/List/Pairwise.lean:38>) |

Sorting alone cannot prove uniqueness: `[h,h]` is non-strictly sorted. In particular, `pairwise_mergeSort` cannot simply be instantiated with a strict comparator: its totality requirement fails at `a = b`.

A sound route is to sort an auxiliary copy of the exact `List (List Nat)`, prove the resulting list strictly increases at every adjacent pair, derive its pairwise strict order using transitivity, derive `Nodup`, and transfer `Nodup` back through `mergeSort_perm`. This does not require a collision-free SHA assumption, a different hash encoding, or reordering the original action data. Set equality would be insufficient because it forgets duplicates.

**Remaining bridge:** searches of the pinned `Init.Data.List` and `Std` sources did not identify a ready-made adjacent-strict-check-to-`Pairwise`/`Nodup` theorem. A small generic checker and proof bridge would still need to be supplied and kernel checked. `List.pairwise_iff_getElem` is an existing alternative characterization, but quantifies over *all* earlier/later index pairs; it is not itself a linear adjacent checker. [Source](</nix/store/7m5ds60a41w276hsx3rybv76x3609l6k-lean4-4.30.0/src/lean/Init/Data/List/Nat/Pairwise.lean:65>)

## Existing order laws for `List Nat`

`Ord (List α)` uses `List.compareLex compare`; the definition compares actual list elements and lengths. [Source](</nix/store/7m5ds60a41w276hsx3rybv76x3609l6k-lean4-4.30.0/src/lean/Init/Data/Ord/Basic.lean:511>)

The library supplies `TransOrd Nat` and `LawfulEqOrd Nat`, plus instances lifting `TransOrd`, `LawfulEqOrd`, and `LawfulBEqOrd` to lists. Thus the standard `compare` on `List Nat` already has the laws needed here; these need not be postulated for an ad hoc comparator. [Nat instances](</nix/store/7m5ds60a41w276hsx3rybv76x3609l6k-lean4-4.30.0/src/lean/Init/Data/Order/Ord.lean:527>), [list instances](</nix/store/7m5ds60a41w276hsx3rybv76x3609l6k-lean4-4.30.0/src/lean/Init/Data/Order/Ord.lean:714>)

For a strict relation `compare a b = .lt`, `Std.TransCmp.lt_trans` supplies transitivity and `Std.ReflCmp.compare_self` supplies irreflexivity by contradiction with `.lt`. `LawfulEqCmp` additionally states that comparator equality implies actual equality, relevant when an index or deduplication structure identifies keys by comparison. [Transitivity](</nix/store/7m5ds60a41w276hsx3rybv76x3609l6k-lean4-4.30.0/src/lean/Init/Data/Order/Ord.lean:243>), [reflexivity](</nix/store/7m5ds60a41w276hsx3rybv76x3609l6k-lean4-4.30.0/src/lean/Init/Data/Order/Ord.lean:40>), [equality law](</nix/store/7m5ds60a41w276hsx3rybv76x3609l6k-lean4-4.30.0/src/lean/Init/Data/Order/Ord.lean:326>)

## Verified membership structures for ownership

`Std.HashSet` offers the most direct existing Boolean identity:

`Std.HashSet.contains_ofList [EquivBEq α] [LawfulHashable α] : (ofList l).contains k = l.contains k`.

It can justify replacing repeated lookup in one writer list with lookup in a set built from that *same* list. It does not justify combining permissions across policies. It is available in `Std.Data.HashSet.Lemmas`; the set is a separate-chaining hash table with bundled well-formedness. For Nat identities, the standard lawful equality supports the required hash law. `LawfulHashable` only requires equal keys to have equal hashes, not collision freedom. [Membership theorem](</nix/store/7m5ds60a41w276hsx3rybv76x3609l6k-lean4-4.30.0/src/lean/Std/Data/HashSet/Lemmas.lean:1280>), [structure](</nix/store/7m5ds60a41w276hsx3rybv76x3609l6k-lean4-4.30.0/src/lean/Std/Data/HashSet/Basic.lean:33>), [hash-law definition and lawful-equality instance](</nix/store/7m5ds60a41w276hsx3rybv76x3609l6k-lean4-4.30.0/src/lean/Init/Data/LawfulHashable.lean:14>)

`Std.TreeSet.contains_ofList [TransCmp cmp] [BEq α] [LawfulBEqCmp cmp]` likewise proves `(ofList l cmp).contains k = l.contains k`. The exact compatibility law between Boolean equality and comparator equality is required; merely supplying an executable comparison function is insufficient. With the original lawful Nat equality, this compatibility also gives actual key equality (`LawfulBEqCmp.lawfulEqCmp`); raw `mem_insert` alone only speaks about comparator equality. `TreeSet` is a size-bounded search tree with bundled well-formedness. [Membership theorem](</nix/store/7m5ds60a41w276hsx3rybv76x3609l6k-lean4-4.30.0/src/lean/Std/Data/TreeSet/Lemmas.lean:1194>), [structure and comparator requirements](</nix/store/7m5ds60a41w276hsx3rybv76x3609l6k-lean4-4.30.0/src/lean/Std/Data/TreeSet/Basic.lean:33>)

Both structures have `containsThenInsert` and insertion/membership lemmas, usable to build a duplicate-detection fold with a separately proved invariant. Neither `HashSet.size_ofList` nor `TreeSet.size_ofList` is already the desired converse: their inspected signatures **assume** distinctness to derive size equality. Using those lemmas alone to infer distinctness would be circular. [HashSet size theorem](</nix/store/7m5ds60a41w276hsx3rybv76x3609l6k-lean4-4.30.0/src/lean/Std/Data/HashSet/Lemmas.lean:1336>), [TreeSet size theorem](</nix/store/7m5ds60a41w276hsx3rybv76x3609l6k-lean4-4.30.0/src/lean/Std/Data/TreeSet/Lemmas.lean:1249>)

## Imports, trust and performance limits

- For a source importing `Init`, list sorting, permutation, pairwise lemmas, lexicographic comparison and order laws are already in its transitive import graph: `Init → Init.Data → Init.Data.List / Init.Data.Ord / Init.Data.Order`; `Init.Data.List` imports `Sort`, `Perm`, `Pairwise` and `Lex`. `Std.Data.HashSet.Lemmas` and `Std.Data.TreeSet.Lemmas` would add Std imports, but no external package. [Init](</nix/store/7m5ds60a41w276hsx3rybv76x3609l6k-lean4-4.30.0/src/lean/Init.lean:20>), [Data](</nix/store/7m5ds60a41w276hsx3rybv76x3609l6k-lean4-4.30.0/src/lean/Init/Data.lean:15>), [List](</nix/store/7m5ds60a41w276hsx3rybv76x3609l6k-lean4-4.30.0/src/lean/Init/Data/List.lean:25>)
- `List.mergeSort` has a proved compiler replacement, `@[csimp] List.MergeSort.Internal.mergeSort_eq_mergeSortTR₂`; `List.merge` has `List.MergeSort.Internal.merge_eq_mergeTR`. The source explicitly distinguishes the convenient logical definition from faster runtime implementations. Compiled sorting speed therefore does not establish the cost of kernel reduction of a concrete certificate. An explicit equality proof could rewrite a logical expression to a proved equivalent implementation, but that is additional proof construction with unmeasured cost. [Logical definition](</nix/store/7m5ds60a41w276hsx3rybv76x3609l6k-lean4-4.30.0/src/lean/Init/Data/List/Sort/Basic.lean:66>), [compiler replacement](</nix/store/7m5ds60a41w276hsx3rybv76x3609l6k-lean4-4.30.0/src/lean/Init/Data/List/Sort/Impl.lean:240>)
- Array merge sort is also verified: `Array.toList_mergeSort` relates it to list sorting, and `Array.mergeSort_perm` proves permutation. It adds another representation/conversion path, so its existence alone does not justify choosing it for kernel computation. [Source](</nix/store/7m5ds60a41w276hsx3rybv76x3609l6k-lean4-4.30.0/src/lean/Init/Data/Array/Sort/Lemmas.lean:133>)
- These source facts support mathematically sound building blocks, not a completed replacement certificate. Any concrete solution still needs its generic bridge/equivalence theorem, the original theorem-type check and axiom audit, and fresh kernel replay. No speedup is established by this inspection, and these algorithms do not address measured time spent elaborating anchor definitions before the checker theorem.

## G1 implementation assessment (2026-10-22)

Selected: the pinned Lean library's `List.Perm.cons`, `List.perm_middle`, append congruences
and `List.Perm.nodup`; `Std.TransCmp.lt_trans` and `Std.ReflCmp.compare_self` for full-key order.
The small custom adapter proves that replay consumes exactly the original keys (with multiplicity),
then strict adjacency plus transitivity establishes global uniqueness. All statements were checked
for arbitrary input and arbitrary trace, with only the three permitted logical axioms. No producer
algorithm theorem is required: the producer is not proof authority. The original permission and
lifecycle predicates and preservation theorems remain unchanged.

`List.mergeSort_perm` provided a correct direct alternative but concrete A256 simplification hit
its resource bound. Array indexing/member lemmas provided another correct bridge, but the measured
inverse-array candidate exhausted heartbeats. This does not establish that all direct or indexed
algorithms are inherently slower; it justifies selecting the tested trace checker for this remedy.

The UTF-8 interpretation uses pinned `String.toByteArray_inj`, `ByteArray.ext`,
`Array.toList_inj`, `List.map_inj_right`, `UInt8.toNat_inj`. These establish exact representation
properties, not Core/JSON extraction correctness. No additional libraries, axioms, toolchain
manager or native evaluation oracle are introduced.

Proof-only concrete definitions and narrow explicit imports are independent measured reductions
in code-generation/dependency overhead. `leanchecker --fresh` still checks the complete actual
claim closure. The audit module imports Lean's metaprogramming infrastructure outside the concrete
proof's dependency closure; every audited root's type and transitive axioms remain checked.
See [selection](design-decision.md) and [measurements](evidence/candidate-comparison.json).
