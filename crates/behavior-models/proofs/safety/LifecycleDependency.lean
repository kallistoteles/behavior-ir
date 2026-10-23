prelude
import safety.LifecycleData

namespace Safety.LifecycleDependency
open Safety.Lifecycle

private theorem original (s : Lifecycle.State np nc) (h : safe s) (i : Fin nc)
    (row : Child np) (hrow : s.children i = some row) :
    ((s.parents row.owner).isSome = true) ∧
    (row.terminal = false → s.parents row.owner = some false) := by
  exact ⟨by simpa [R,hrow] using h.1 i, by simpa [P,hrow] using h.2 i⟩

private theorem from_rows (s : Lifecycle.State np nc)
    (h : ∀ i row, s.children i = some row →
      ((s.parents row.owner).isSome = true) ∧
      (row.terminal = false → s.parents row.owner = some false)) : safe s := by
  constructor <;> intro i <;> cases hrow : s.children i with
  | none => simp_all
  | some row =>
    first | exact (h i row hrow).1 | exact (h i row hrow).2

/- LD-1: actual operation cases, without assuming preservation as a premise. -/
theorem sound : Safety.LifecycleGuarantee := by
  intro np nc op s hs guard
  apply from_rows
  intro i row hrow
  cases op with
  | irrelevant => exact original s hs i row hrow
  | childRemove target =>
    by_cases hit : i = target
    · simp [apply,update,hit] at hrow
    · have old : s.children i = some row := by simpa [apply,update,hit] using hrow
      exact original s hs i row old
  | parentRemove target =>
    have old : s.children i = some row := hrow
    have different : row.owner ≠ target := by simpa [permitted,old] using guard i
    simpa [apply,update,different] using original s hs i row old
  | parentCreate target gone =>
    have old : s.children i = some row := hrow
    have source := original s hs i row old
    have different : row.owner ≠ target := by
      intro eq
      have present := source.1
      have absent : s.parents target = none := guard
      simp_all
    simpa [apply,update,different] using source
  | parentState target gone =>
    have old : s.children i = some row := hrow
    have source := original s hs i row old
    by_cases hit : row.owner = target
    · constructor
      · simp [apply,update,hit]
      · intro active
        have allowed : gone = false := by
          cases hg : gone with
          | false => rfl
          | true =>
            have term := guard hg i
            simp [old,hit,active] at term
        simp [apply,update,hit,allowed]
    · simpa [apply,update,hit] using source
  | childCreate target parent terminal =>
    by_cases hit : i = target
    · have eq : row = ⟨parent,terminal⟩ := by simpa [apply,update,hit] using hrow.symm
      subst row
      exact ⟨guard.2.1,guard.2.2⟩
    · have old : s.children i = some row := by simpa [apply,update,hit] using hrow
      exact original s hs i row old
  | childState target terminal =>
    by_cases hit : i = target
    · cases old : s.children target with
      | none => simp [apply,update,hit,old] at hrow
      | some prior =>
        have eq : row = {prior with terminal} := by simpa [apply,update,hit,old] using hrow.symm
        subst row
        constructor
        · exact (original s hs target prior old).1
        · intro active
          have h := guard active
          simpa [permitted,old] using h
    · have old : s.children i = some row := by simpa [apply,update,hit] using hrow
      exact original s hs i row old

theorem conjunction : Safety.LifecycleConjunctionGuarantee := by
  intro n np nc ops states initial guards i
  exact sound (ops i) (states i) (initial i) (guards i)

end Safety.LifecycleDependency

namespace Safety.LifecycleDependency

-- This projection reads only ordinary Core coordinates. Owner decoding and the
-- enum category interpretation are part of the explicit typed correspondence.
def project (s : Safety.State ne ni nf)
    (parent child : Fin ne) (parentField childField ownerField : Fin nf)
    (gone terminal : Nat → Bool) (owner : Nat → Fin ni) : Lifecycle.State ni ni :=
  ⟨fun i => (observe s parent i parentField).map gone,
   fun i => (observe s child i ownerField).bind fun p =>
     (observe s child i childField).map fun phase => ⟨owner p,terminal phase⟩⟩

def ProjectionFrameGuarantee : Prop :=
  ∀ {ne ni nf : Nat} (effect : Effect ne ni nf) (s : Safety.State ne ni nf)
    (parent child : Fin ne) (parentField childField ownerField : Fin nf)
    (gone terminal : Nat → Bool) (owner : Nat → Fin ni)
 ,
    affects effect.operation parent.val parentField.val = false →
    affects effect.operation child.val childField.val = false →
    affects effect.operation child.val ownerField.val = false →
    project (effect.apply s) parent child parentField childField ownerField gone terminal owner =
    project s parent child parentField childField ownerField gone terminal owner

theorem projection_frame (effect : Effect ne ni nf) (s : Safety.State ne ni nf)
    (parent child : Fin ne) (parentField childField ownerField : Fin nf)
    (gone terminal : Nat → Bool) (owner : Nat → Fin ni)
    (hp : affects effect.operation parent.val parentField.val = false)
    (hc : affects effect.operation child.val childField.val = false)
    (ho : affects effect.operation child.val ownerField.val = false) :
    project (effect.apply s) parent child parentField childField ownerField gone terminal owner =
    project s parent child parentField childField ownerField gone terminal owner := by
  unfold project
  congr 1
  · funext i
    rw [FieldOwnership.frame effect s parent i parentField hp]
  · funext i
    rw [FieldOwnership.frame effect s child i childField hc,
        FieldOwnership.frame effect s child i ownerField ho]

end Safety.LifecycleDependency

namespace Safety.LifecycleDependency
open Safety.Lifecycle

def nontrivial : Lifecycle.Operation np nc → Bool
  | .irrelevant => false
  | _ => true

def execute (ops : List (Lifecycle.Operation np nc)) (s : Lifecycle.State np nc) : Lifecycle.State np nc :=
  ops.foldl (fun state op => Lifecycle.apply op state) s

private theorem inactive_identity (op : Lifecycle.Operation np nc) (s : Lifecycle.State np nc)
    (inactive : nontrivial op = false) : Lifecycle.apply op s = s := by
  cases op <;> simp_all [nontrivial, Lifecycle.apply]

private theorem inactive_sequence (ops : List (Lifecycle.Operation np nc)) (s : Lifecycle.State np nc)
    (inactive : (ops.filter nontrivial).length = 0) : execute ops s = s := by
  induction ops generalizing s with
  | nil => rfl
  | cons op rest ih =>
    cases active : nontrivial op with
    | true => simp [active] at inactive
    | false =>
      have tail : (rest.filter nontrivial).length = 0 := by simpa [List.filter_cons,active] using inactive
      exact (ih (Lifecycle.apply op s) tail).trans (inactive_identity op s active)

-- All source guards are evaluated in the original state. The one-operation
-- restriction, rather than a false assumption about sequential guard checking,
-- makes simultaneous ordinary effects preserve each projected property.
theorem sequence_preserves (ops : List (Lifecycle.Operation np nc)) (s : Lifecycle.State np nc)
    (single : (ops.filter nontrivial).length ≤ 1)
    (guards : ∀ op ∈ ops, Lifecycle.permitted op s)
    (initial : Lifecycle.safe s) : Lifecycle.safe (execute ops s) := by
  induction ops generalizing s with
  | nil => exact initial
  | cons op rest ih =>
    cases active : nontrivial op with
    | false =>
      have identity := inactive_identity op s active
      have tail : (rest.filter nontrivial).length ≤ 1 := by simpa [List.filter_cons,active] using single
      change Lifecycle.safe (execute rest (Lifecycle.apply op s))
      rw [identity]
      exact ih s tail (fun item member => guards item (List.mem_cons_of_mem op member)) initial
    | true =>
      have zero : (rest.filter nontrivial).length = 0 := by
        have bound : (rest.filter nontrivial).length + 1 ≤ 1 := by simpa [List.filter_cons,active] using single
        exact Nat.eq_zero_of_le_zero (Nat.le_of_succ_le_succ bound)
      change Lifecycle.safe (execute rest (Lifecycle.apply op s))
      rw [inactive_sequence rest (Lifecycle.apply op s) zero]
      exact sound op s initial (guards op (by simp))

end Safety.LifecycleDependency

namespace Safety.LifecycleDependency

-- Derive the removal premise from Core's complete resulting-state reference
-- integrity, not from an authored source guard. Under the isolated operation,
-- child rows are unchanged, so even a terminal child blocks physical deletion.
theorem parent_removal_integrity (s : Lifecycle.State np nc) (parent : Fin np)
    (resultRefs : Lifecycle.R (Lifecycle.apply (.parentRemove parent) s)) :
    Lifecycle.permitted (.parentRemove parent) s := by
  intro i
  cases hc : s.children i with
  | none => trivial
  | some row =>
    intro same
    have valid := resultRefs i
    simp [Lifecycle.apply,Lifecycle.update,hc,same] at valid

end Safety.LifecycleDependency
