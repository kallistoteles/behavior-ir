prelude
import safety.Data
import Init.Data.String.Basic
import Init.Data.UInt.Lemmas

namespace Safety.Candidate

-- This checks only adjacent edges; transitivity supplies all distant pairs.
def adjacent (r : α → α → Bool) : List α → Bool
  | [] => true
  | [_] => true
  | a :: b :: rest => r a b && adjacent r (b :: rest)

theorem adjacent_pairwise (r : α → α → Bool)
    (trans : ∀ a b c, r a b = true → r b c = true → r a c = true)
    (xs : List α) (checked : adjacent r xs = true) :
    xs.Pairwise (fun a b => r a b = true) := by
  induction xs with
  | nil => exact .nil
  | cons a rest ih =>
    cases rest with
    | nil => simp
    | cons b tail =>
      have edges := Bool.and_eq_true_iff.mp checked
      have pairwise := ih edges.2
      apply List.Pairwise.cons
      · intro c hc
        rcases List.mem_cons.mp hc with eq | mem
        · subst c; exact edges.1
        · exact trans a b c edges.1 ((List.pairwise_cons.mp pairwise).1 c mem)
      · exact pairwise

def less (a b : List Nat) : Bool := compare a b == .lt

theorem less_trans (a b c : List Nat) (hab : less a b = true) (hbc : less b c = true) :
    less a c = true := by
  simp only [less, beq_iff_eq] at *
  exact Std.TransCmp.lt_trans hab hbc

theorem less_ne {a b : List Nat} (h : less a b = true) : a ≠ b := by
  intro eq
  subst b
  simp [less, Std.ReflCmp.compare_self] at h


end Safety.Candidate

namespace Safety.Candidate

def literalBytes (s : String) : List Nat := s.toByteArray.data.toList.map UInt8.toNat

def compactPayload (s : String) : List (List Nat) := [literalBytes s]

theorem compactPayload_flatten (s : String) : (compactPayload s).flatten = literalBytes s := by
  simp [compactPayload]

theorem literalBytes_injective {a b : String} (h : literalBytes a = literalBytes b) : a = b := by
  apply String.toByteArray_inj.mp
  apply ByteArray.ext
  apply Array.toList_inj.mp
  exact (List.map_inj_right (fun x y eq => UInt8.toNat_inj.mp eq)).mp h

end Safety.Candidate

namespace Safety.Candidate

inductive MergeTrace where
  | empty
  | leaf
  | branch (left right : MergeTrace) (choices : List Bool)
  deriving Repr

-- Only consume the selected head. Every input element must be consumed exactly once.
def replayMerge : List Bool → List α → List α → Option (List α)
  | [], [], [] => some []
  | [], _, _ => none
  | true :: choices, a :: xs, ys => (replayMerge choices xs ys).map (a :: ·)
  | false :: choices, xs, b :: ys => (replayMerge choices xs ys).map (b :: ·)
  | _, _, _ => none

theorem replayMerge_perm (choices : List Bool) (xs ys output : List α)
    (checked : replayMerge choices xs ys = some output) : output.Perm (xs ++ ys) := by
  induction choices generalizing xs ys output with
  | nil => cases xs <;> cases ys <;> simp_all [replayMerge]
  | cons choice choices ih =>
    cases choice with
    | false =>
      cases ys with
      | nil => simp [replayMerge] at checked
      | cons b ys =>
        simp only [replayMerge, Option.map_eq_some_iff] at checked
        obtain ⟨rest, hrest, rfl⟩ := checked
        exact ((ih xs ys rest hrest).cons b).trans List.perm_middle.symm
    | true =>
      cases xs with
      | nil => simp [replayMerge] at checked
      | cons a xs =>
        simp only [replayMerge, Option.map_eq_some_iff] at checked
        obtain ⟨rest, hrest, rfl⟩ := checked
        exact (ih xs ys rest hrest).cons a

-- Leaves consume the authoritative original input in order. No key values or
-- completeness assertion supplied by the producer can redefine that input.
def replayTrace (trace : MergeTrace) (xs : List α) : Option (List α × List α) :=
  match trace with
  | .empty => some ([], xs)
  | .leaf => match xs with | [] => none | x :: rest => some ([x], rest)
  | .branch left right choices =>
    match replayTrace left xs with
    | none => none
    | some (ls, remaining) =>
      match replayTrace right remaining with
      | none => none
      | some (rs, remaining) =>
        match replayMerge choices ls rs with
        | none => none
        | some output => some (output, remaining)

theorem replayTrace_perm (trace : MergeTrace) (xs sorted remaining : List α)
    (checked : replayTrace trace xs = some (sorted, remaining)) :
    (sorted ++ remaining).Perm xs := by
  induction trace generalizing xs sorted remaining with
  | empty =>
    simp only [replayTrace, Option.some.injEq, Prod.mk.injEq] at checked
    rcases checked with ⟨rfl, rfl⟩
    exact .refl _
  | leaf =>
    cases xs with
    | nil => simp [replayTrace] at checked
    | cons x rest =>
      simp only [replayTrace, Option.some.injEq, Prod.mk.injEq] at checked
      rcases checked with ⟨rfl, rfl⟩
      exact .refl _
  | branch left right choices ihl ihr =>
    cases hl : replayTrace left xs with
    | none => simp [replayTrace, hl] at checked
    | some l =>
      rcases l with ⟨ls, lr⟩
      cases hr : replayTrace right lr with
      | none => simp [replayTrace, hl, hr] at checked
      | some r =>
        rcases r with ⟨rs, rr⟩
        cases hm : replayMerge choices ls rs with
        | none => simp [replayTrace, hl, hr, hm] at checked
        | some merged =>
          simp only [replayTrace, hl, hr, hm, Option.some.injEq, Prod.mk.injEq] at checked
          rcases checked with ⟨rfl, rfl⟩
          have leftperm := ihl xs ls lr hl
          have rightperm := ihr lr rs rr hr
          have mergedperm := replayMerge_perm choices ls rs merged hm
          have assembly := (rightperm.append_left ls).trans leftperm
          have combined := (mergedperm.append_right rr)
          exact combined.trans (by simpa only [List.append_assoc] using assembly)

def traceChecked (xs : List (List Nat)) (trace : MergeTrace) : Bool :=
  match replayTrace trace xs with
  | none => false
  | some (sorted, remaining) => remaining.isEmpty && adjacent less sorted

theorem traceChecked_sound (xs : List (List Nat)) (trace : MergeTrace)
    (checked : traceChecked xs trace = true) : xs.Pairwise (· ≠ ·) := by
  cases replay : replayTrace trace xs with
  | none => simp [traceChecked, replay] at checked
  | some result =>
    rcases result with ⟨sorted, remaining⟩
    have both : remaining.isEmpty = true ∧ adjacent less sorted = true := by
      simpa only [traceChecked, replay, Bool.and_eq_true_iff] using checked
    have empty : remaining = [] := List.isEmpty_iff.mp both.1
    subst remaining
    have permutation := replayTrace_perm trace xs sorted [] replay
    simp only [List.append_nil] at permutation
    exact permutation.nodup ((adjacent_pairwise less less_trans sorted both.2).imp (fun h => less_ne h))

end Safety.Candidate
