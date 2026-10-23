import CandidateLemmas

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
