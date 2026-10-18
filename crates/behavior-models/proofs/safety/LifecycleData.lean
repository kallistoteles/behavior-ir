import safety.FieldOwnership

/-! Statement contract v1. These maps are a proof-only projection of ordinary
Core state. Cardinalities are arbitrary; they are not bounded-model test limits.
No corresponding runtime store, registry or event stream is introduced. -/
namespace Safety.Lifecycle

structure Child (np : Nat) where
  owner : Fin np
  terminal : Bool
  deriving DecidableEq, Repr

structure State (np nc : Nat) where
  parents : Fin np → Option Bool
  children : Fin nc → Option (Child np)

def R (s : State np nc) : Prop :=
  ∀ i, match s.children i with
    | none => True
    | some c => (s.parents c.owner).isSome = true

def P (s : State np nc) : Prop :=
  ∀ i, match s.children i with
    | none => True
    | some c => c.terminal = false → s.parents c.owner = some false

def safe (s : State np nc) : Prop := R s ∧ P s
instance (s : State np nc) : Decidable (safe s) := by
  letI dr : DecidablePred (fun i => match s.children i with | none => True | some c => (s.parents c.owner).isSome = true) :=
    fun i => by cases h : s.children i <;> simp only [h] <;> infer_instance
  letI dp : DecidablePred (fun i => match s.children i with | none => True | some c => c.terminal = false → s.parents c.owner = some false) :=
    fun i => by cases h : s.children i <;> simp only [h] <;> infer_instance
  unfold safe R P
  exact instDecidableAnd

inductive Operation (np nc : Nat) where
  | parentState (p : Fin np) (gone : Bool)
  | childState (c : Fin nc) (terminal : Bool)
  | parentCreate (p : Fin np) (gone : Bool)
  | childCreate (c : Fin nc) (p : Fin np) (terminal : Bool)
  | parentRemove (p : Fin np)
  | childRemove (c : Fin nc)
  | irrelevant

def update (f : Fin n → α) (i : Fin n) (v : α) : Fin n → α :=
  fun j => if j = i then v else f j

def apply (op : Operation np nc) (s : State np nc) : State np nc :=
  match op with
  | .parentState p gone | .parentCreate p gone => ⟨update s.parents p (some gone),s.children⟩
  | .childState c terminal => ⟨s.parents,update s.children c ((s.children c).map fun row => {row with terminal})⟩
  | .childCreate c p terminal => ⟨s.parents,update s.children c (some ⟨p,terminal⟩)⟩
  | .parentRemove p => ⟨update s.parents p none,s.children⟩
  | .childRemove c => ⟨s.parents,update s.children c none⟩
  | .irrelevant => s

-- Canonical source guards and Core freshness/result Ref integrity, not Q(S').
def permitted (op : Operation np nc) (s : State np nc) : Prop :=
  match op with
  | .parentState p gone => gone = true → ∀ i, match s.children i with
      | none => True
      | some c => c.owner = p → c.terminal = true
  | .childState i terminal => terminal = false → match s.children i with
      | none => True
      | some c => s.parents c.owner = some false
  | .parentCreate p _ => s.parents p = none
  | .childCreate i p terminal => s.children i = none ∧
      ((s.parents p).isSome = true) ∧ (terminal = false → s.parents p = some false)
  | .parentRemove p => ∀ i, match s.children i with
      | none => True
      | some c => c.owner ≠ p
  | .childRemove _ | .irrelevant => True
instance (op : Operation np nc) (s : State np nc) : Decidable (permitted op s) := by
  cases op with
  | parentState p gone =>
    letI d : DecidablePred (fun i => match s.children i with | none => True | some c => c.owner = p → c.terminal = true) :=
      fun i => by cases h : s.children i <;> simp only [h] <;> infer_instance
    exact inferInstanceAs (Decidable (gone = true → ∀ i, _))
  | parentRemove p =>
    letI d : DecidablePred (fun i => match s.children i with | none => True | some c => c.owner ≠ p) :=
      fun i => by cases h : s.children i <;> simp only [h] <;> infer_instance
    exact inferInstanceAs (Decidable (∀ i, _))
  | childState i terminal =>
    unfold permitted
    cases h : s.children i <;> simp only [h] <;> infer_instance
  | parentCreate _ _ | childCreate _ _ _ | childRemove _ | irrelevant => unfold permitted; infer_instance

end Safety.Lifecycle

namespace Safety

def LifecycleGuarantee : Prop :=
  ∀ {np nc : Nat} (op : Lifecycle.Operation np nc) (s : Lifecycle.State np nc),
    Lifecycle.safe s → Lifecycle.permitted op s → Lifecycle.safe (Lifecycle.apply op s)

-- Each configured property is checked independently, including shared-parent updates.
def LifecycleConjunctionGuarantee : Prop :=
  ∀ {n : Nat} {np nc : Fin n → Nat}
    (ops : (i : Fin n) → Lifecycle.Operation (np i) (nc i))
    (states : (i : Fin n) → Lifecycle.State (np i) (nc i)),
    (∀ i, Lifecycle.safe (states i)) →
    (∀ i, Lifecycle.permitted (ops i) (states i)) →
    ∀ i, Lifecycle.safe (Lifecycle.apply (ops i) (states i))

end Safety
