prelude
import Init.Data.List.Perm
import Init.Data.List.Nat.Range
import Init.Data.List.Nat.Pairwise
import Init.Data.List.Zip
import Init.Data.Fin.Lemmas
import Init.Data.Repr
import Init.Data.Order.Ord
import Init.RCases
import Init.Classical

/-!
Safety statement contract v1. Entity, identity and field bounds are arbitrary;
every finite typed state embeds in such a bounded map. Unused field slots are
padding. Core-to-this-map correspondence is an explicit external assumption.
No bound used here is a test exploration bound.
-/
namespace Safety

inductive OperationKind where
  | assign | create | remove
  deriving DecidableEq, Repr

structure Operation where
  entity : Nat
  kind : OperationKind
  field : Nat := 0
  deriving DecidableEq, Repr

structure Action where
  identity : Nat
  operations : List Operation
  deriving DecidableEq, Repr

structure Ownership where
  entity : Nat
  writers : List (Nat × List Nat)
  creators : List Nat
  removers : List Nat
  deriving DecidableEq, Repr

def allowed (p : Ownership) (action : Nat) (op : Operation) : Bool :=
  if op.entity != p.entity then true else
    match op.kind with
    | .assign => p.writers.all fun (field, writers) => field != op.field || writers.contains action
    | .create => p.creators.contains action
    | .remove => p.removers.contains action

def affects (op : Operation) (entity field : Nat) : Bool :=
  op.entity == entity && match op.kind with
    | .assign => op.field == field
    | .create | .remove => true

abbrev State (entities identities fields : Nat) :=
  Fin entities → Fin identities → Option (Fin fields → Nat)

inductive EffectKind (fields : Nat) where
  | assign (field : Fin fields) (value : Nat)
  | create (row : Fin fields → Nat)
  | remove

structure Effect (entities identities fields : Nat) where
  entity : Fin entities
  identity : Fin identities
  kind : EffectKind fields

def Effect.operation (e : Effect ne ni nf) : Operation :=
  match e.kind with
  | .assign f _ => ⟨e.entity.val, .assign, f.val⟩
  | .create _ => ⟨e.entity.val, .create, 0⟩
  | .remove => ⟨e.entity.val, .remove, 0⟩

def Effect.apply (e : Effect ne ni nf) (s : State ne ni nf) : State ne ni nf :=
  fun entity identity =>
    if entity = e.entity ∧ identity = e.identity then
      match e.kind with
      | .assign field value => (s entity identity).map (fun row f => if f = field then value else row f)
      | .create row => some row
      | .remove => none
    else s entity identity

def observe (s : State ne ni nf) (e : Fin ne) (i : Fin ni) (f : Fin nf) : Option Nat :=
  (s e i).map (fun row => row f)

def execute (effects : List (Effect ne ni nf)) (s : State ne ni nf) : State ne ni nf :=
  effects.foldl (fun state effect => effect.apply state) s

/- The versioned mathematical contract is separate from its proof implementation.
The auditor compares theorem propositions with this definition by kernel
definitional equality, rather than trusting a declaration name. -/
def OwnershipGuarantee : Prop :=
  ∀ {ne ni nf : Nat} (p : Ownership) (action : Action)
    (effects : List (Effect ne ni nf)) (s : State ne ni nf)
    (entity : Fin ne) (identity : Fin ni) (field : Fin nf),
    effects.map Effect.operation = action.operations →
    (∀ op ∈ action.operations, allowed p action.identity op = true) →
    observe (execute effects s) entity identity field ≠ observe s entity identity field →
    ∃ effect ∈ effects, affects effect.operation entity.val field.val = true ∧
      allowed p action.identity effect.operation = true

end Safety
