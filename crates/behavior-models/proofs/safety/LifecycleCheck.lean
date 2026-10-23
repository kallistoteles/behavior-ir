prelude
import safety.LifecycleDependency

namespace Safety

-- Opaque expression identities are associated with probes built through Core's
-- public admitted Builder. Their semantic interpretation remains a named trust
-- boundary; the kernel checks placement, binding association and finite rules.
structure LifecyclePolicy where
  parent : Nat
  child : Nat
  parentField : Nat
  childField : Nat
  ownerField : Nat
  referenceParent : Nat
  parentValues : List (List Nat × List Nat)
  childValues : List (List Nat × List Nat)
  prohibited : List (List Nat)
  terminal : List (List Nat)
  controls : List Nat
  deriving Repr, DecidableEq

structure GuardProbe where
  dependency : Nat
  parent : Nat
  child : Nat
  noChildren : List Nat
  permittedParent : List Nat
  ownerEquality : List Nat
  parentId : List Nat
  noChildrenMembers : List (List Nat)
  permittedParentMembers : List (List Nat)
  deriving Repr, DecidableEq

structure CoreOperation where
  operation : Operation
  binding : Nat
  value : List Nat
  initializers : List (Nat × List Nat)
  deriving Repr, DecidableEq

structure CoreAction where
  parameters : List (Nat × Bool)
  roots : List (List Nat)
  operations : List CoreOperation
  probes : List GuardProbe
  deriving Repr, DecidableEq

-- Structural relevance includes creation/removal and all three property fields.
def parentRelevant (d : LifecyclePolicy) (o : Operation) : Bool :=
  o.entity == d.parent && (o.kind != .assign || o.field == d.parentField)
def childRelevant (d : LifecyclePolicy) (o : Operation) : Bool :=
  o.entity == d.child && (o.kind != .assign || o.field == d.childField || o.field == d.ownerField)
def relevant (ds : List LifecyclePolicy) (o : Operation) : Bool :=
  ds.any fun d => parentRelevant d o || childRelevant d o

def stateBinding (a : CoreAction) (binding entity : Nat) : Bool :=
  a.parameters[binding]? == some (entity,true)

def memberValue (table : List (List Nat × List Nat)) (selected : List (List Nat)) (hash : List Nat) : Option Bool :=
  (table.find? fun v => v.1 == hash).map fun entry => selected.contains entry.2

def initializer (o : CoreOperation) (field : Nat) : Option (List Nat) :=
  (o.initializers.find? fun v => v.1 == field).map Prod.snd

inductive LifecyclePattern where
  | irrelevant
  | parentState (binding : Nat) (gone : Bool)
  | childState (binding : Nat) (terminal : Bool) (parent : Nat)
  | parentCreate (gone : Bool)
  | childCreate (parent : Nat) (terminal : Bool)
  | parentRemove (binding : Nat)
  | childRemove (binding : Nat)
  deriving Repr, DecidableEq

-- A successful classification retains the exact mandatory guard roots. Its
-- premises are derived from raw operations; Rust supplies no relevance verdict.
structure Classified where
  pattern : LifecyclePattern
  required : List (List Nat)
  deriving Repr, DecidableEq

private def parentWitness (a : CoreAction) (d : LifecyclePolicy) (di : Nat)
    (child : Nat) (creating : Bool) (o : CoreOperation) (terminal : Bool) : Option GuardProbe :=
  a.probes.find? fun p => p.dependency == di && p.child == child &&
    stateBinding a p.parent d.parent &&
    (if creating then initializer o d.ownerField == some p.parentId else a.roots.contains p.ownerEquality) &&
    (terminal || a.roots.contains p.permittedParent)

def classify (a : CoreAction) (d : LifecyclePolicy) (di : Nat) (o : CoreOperation) : Option Classified := do
  let pr := parentRelevant d o.operation
  let cr := childRelevant d o.operation
  if pr && cr then none
  else if !pr && !cr then some ⟨.irrelevant,[]⟩
  else if pr then
    match o.operation.kind with
    | .remove => if stateBinding a o.binding d.parent then some ⟨.parentRemove o.binding,[]⟩ else none
    | .create =>
      let value ← initializer o d.parentField
      let gone ← memberValue d.parentValues d.prohibited value
      some ⟨.parentCreate gone,[]⟩
    | .assign =>
      let gone ← memberValue d.parentValues d.prohibited o.value
      if !stateBinding a o.binding d.parent then none
      else if gone then
        let p ← a.probes.find? fun p => p.dependency == di && p.parent == o.binding && a.roots.contains p.noChildren
        some ⟨.parentState o.binding gone,[p.noChildren]⟩
      else some ⟨.parentState o.binding gone,[]⟩
  else
    match o.operation.kind with
    | .remove => if stateBinding a o.binding d.child then some ⟨.childRemove o.binding,[]⟩ else none
    | .create =>
      let value ← initializer o d.childField
      let terminal ← memberValue d.childValues d.terminal value
      let p ← parentWitness a d di 0 true o terminal
      some ⟨.childCreate p.parent terminal,if terminal then [] else [p.permittedParent]⟩
    | .assign =>
      if o.operation.field != d.childField || !stateBinding a o.binding d.child then none
      else
        let terminal ← memberValue d.childValues d.terminal o.value
        if terminal then some ⟨.childState o.binding terminal 0,[]⟩
        else
          let p ← parentWitness a d di o.binding false o terminal
          some ⟨.childState o.binding terminal p.parent,[p.ownerEquality,p.permittedParent]⟩

structure LifecycleData where
  actions : List CoreAction := []
  dependencies : List LifecyclePolicy := []
  deriving Repr

-- Linking this annotation to every occurrence in the FO inventory is performed
-- by checkModule, not delegated to a caller-supplied completeness flag.
def singleRole (ds : List LifecyclePolicy) (o : Operation) : Bool :=
  !(ds.any (fun d => parentRelevant d o) && ds.any (fun d => childRelevant d o))

def validCategoryTable (table : List (List Nat × List Nat)) (members : List (List Nat)) : Bool :=
  decide ((table.map Prod.fst).Pairwise (· ≠ ·)) &&
  decide ((table.map Prod.snd).Pairwise (· ≠ ·)) &&
  members.all (fun member => (table.map Prod.snd).contains member)

def validLifecyclePolicy (d : LifecyclePolicy) : Bool :=
  (d.referenceParent == d.parent && !d.controls.isEmpty) &&
  (validCategoryTable d.parentValues d.prohibited && validCategoryTable d.childValues d.terminal)

-- A finite set-agreement check, independent of hash/probe extraction. Repetitions
-- and order are preserved in the input, but cannot change Boolean membership.
def sameMembers (ordered required : List (List Nat)) : Bool :=
  decide (ordered ⊆ required ∧ required ⊆ ordered)

theorem membership_order_equivalent (ordered required : List (List Nat))
    (checked : sameMembers ordered required = true) :
    ∀ value, value ∈ ordered ↔ value ∈ required := by
  have agreement : ordered ⊆ required ∧ required ⊆ ordered := of_decide_eq_true checked
  exact fun _ => ⟨fun h => agreement.1 h, fun h => agreement.2 h⟩

def MembershipOrderGuarantee : Prop := ∀ ordered required,
  sameMembers ordered required = true → ∀ value, value ∈ ordered ↔ value ∈ required

def validGuardMembers (ds : List LifecyclePolicy) (probe : GuardProbe) : Bool :=
  match ds[probe.dependency]? with
  | none => false
  | some policy => sameMembers probe.noChildrenMembers policy.terminal &&
      sameMembers probe.permittedParentMembers policy.prohibited

def checkLifecycle (m : LifecycleData) : Bool :=
  m.dependencies.all validLifecyclePolicy &&
  m.actions.all fun a =>
    ((a.operations.filter fun o => relevant m.dependencies o.operation).length ≤ 1 &&
    (m.dependencies.zipIdx.all fun (d,di) =>
      a.operations.all fun o => (classify a d di o).isSome)) &&
    (a.operations.all (fun o => singleRole m.dependencies o.operation) &&
      a.probes.all (validGuardMembers m.dependencies))

-- This is the versioned finite premise proposition; its truth is checked by
-- kernel reduction over the exact module, rather than a Rust acceptance flag.
def LifecyclePremises (m : LifecycleData) : Prop :=
  ∀ a ∈ m.actions,
    (a.operations.filter fun o => relevant m.dependencies o.operation).length ≤ 1 ∧
    ∀ d di, (d,di) ∈ m.dependencies.zipIdx →
      d.referenceParent = d.parent ∧ d.controls ≠ [] ∧
      ∀ o ∈ a.operations, ∃ c, classify a d di o = some c

theorem checkLifecycle_sound (m : LifecycleData) (h : checkLifecycle m = true) : LifecyclePremises m := by
  have global := (Bool.and_eq_true_iff.mp h).1
  have actions := (Bool.and_eq_true_iff.mp h).2
  intro a ha
  have h := List.all_eq_true.mp actions a ha
  have h := Bool.and_eq_true_iff.mp (Bool.and_eq_true_iff.mp h).1
  refine ⟨of_decide_eq_true h.1,?_⟩
  intro d di hd
  have checkedPolicy := List.all_eq_true.mp global d (List.fst_mem_of_mem_zipIdx hd)
  have head := Bool.and_eq_true_iff.mp (Bool.and_eq_true_iff.mp checkedPolicy).1
  have operations := List.all_eq_true.mp h.2 (d,di) hd
  refine ⟨by simpa using head.1,?_,?_⟩
  · intro empty
    simp [empty] at head
  · intro o ho
    have present := List.all_eq_true.mp operations o ho
    cases hc : classify a d di o with
    | none => simp [hc] at present
    | some c => exact ⟨c,rfl⟩

def LifecycleCheckerGuarantee : Prop := ∀ m, checkLifecycle m = true → LifecyclePremises m

end Safety

namespace Safety

structure RuntimeBindings (np nc : Nat) where
  parent : Nat → Fin np
  child : Nat → Fin nc
  newParent : Fin np
  newChild : Fin nc

def LifecyclePattern.operation (p : LifecyclePattern) (b : RuntimeBindings np nc) : Lifecycle.Operation np nc :=
  match p with
  | .irrelevant => .irrelevant
  | .parentState i gone => .parentState (b.parent i) gone
  | .childState i terminal _ => .childState (b.child i) terminal
  | .parentCreate gone => .parentCreate b.newParent gone
  | .childCreate i terminal => .childCreate b.newChild (b.parent i) terminal
  | .parentRemove i => .parentRemove (b.parent i)
  | .childRemove i => .childRemove (b.child i)

-- This names the precise unverified public-Core probe correspondence. It does
-- not assume preservation or permitted transitions. Each implication states
-- the original meaning of one canonical required source guard at its bindings.
def ProbeSemantics (a : CoreAction) (di : Nat) (b : RuntimeBindings np nc)
    (s : Lifecycle.State np nc) (holds : List Nat → Prop) : Prop :=
  ∀ probe ∈ a.probes, probe.dependency = di →
    (holds probe.noChildren → ∀ i, match s.children i with
      | none => True
      | some c => c.owner = b.parent probe.parent → c.terminal = true) ∧
    (holds probe.permittedParent → s.parents (b.parent probe.parent) = some false) ∧
    (holds probe.ownerEquality → ∀ i row, i = b.child probe.child →
      s.children i = some row → row.owner = b.parent probe.parent)

-- Separate Core contracts. Fresh creation and result-state Ref validity are
-- not SMT conclusions or additional Model runtime checks.
def OrdinaryCoreConditions (a : CoreAction) (d : LifecyclePolicy) (o : CoreOperation)
    (b : RuntimeBindings np nc) (s : Lifecycle.State np nc) : Prop :=
  (∀ i, stateBinding a i d.parent = true → (s.parents (b.parent i)).isSome = true) ∧
  (o.operation.kind = .create → o.operation.entity = d.parent → s.parents b.newParent = none) ∧
  (o.operation.kind = .create → o.operation.entity = d.child → s.children b.newChild = none) ∧
  (o.operation.kind = .remove → o.operation.entity = d.parent →
    Lifecycle.R (Lifecycle.apply (.parentRemove (b.parent o.binding)) s))

private theorem parentWitness_facts (a : CoreAction) (d : LifecyclePolicy) (di child : Nat)
    (creating terminal : Bool) (o : CoreOperation) (probe : GuardProbe)
    (h : parentWitness a d di child creating o terminal = some probe) :
    probe ∈ a.probes ∧ probe.dependency = di ∧ probe.child = child ∧
    stateBinding a probe.parent d.parent = true ∧
    (if creating then initializer o d.ownerField == some probe.parentId else a.roots.contains probe.ownerEquality) = true ∧
    (terminal || a.roots.contains probe.permittedParent) = true := by
  have mem := List.mem_of_find?_eq_some h
  have found := List.find?_some h
  simp only [Bool.and_eq_true,beq_iff_eq] at found
  exact ⟨mem,found.1.1.1.1,found.1.1.1.2,found.1.1.2,found.1.2,found.2⟩

private theorem witness_child_permitted (a : CoreAction) (d : LifecyclePolicy) (di child : Nat)
    (creating terminal : Bool) (o : CoreOperation) (probe : GuardProbe)
    (h : parentWitness a d di child creating o terminal = some probe)
    (b : RuntimeBindings np nc) (s : Lifecycle.State np nc) (holds : List Nat → Prop)
    (roots : ∀ hash ∈ a.roots, holds hash)
    (semantics : ProbeSemantics a di b s holds) (active : terminal = false) :
    s.parents (b.parent probe.parent) = some false := by
  have facts := parentWitness_facts a d di child creating terminal o probe h
  have has : a.roots.contains probe.permittedParent = true := by simpa [active] using facts.2.2.2.2.2
  exact (semantics probe facts.1 facts.2.1).2.1 (roots _ (List.contains_iff_mem.mp has))

/- This theorem connects finite raw-IR classification to the separately proved
inductive theorem. Literal and binding projection correctness remain in the
explicit Core-to-abstract execution correspondence, never an unchecked axiom. -/
def ClassificationGuarantee : Prop :=
  ∀ {np nc : Nat} (a : CoreAction) (d : LifecyclePolicy) (di : Nat) (o : CoreOperation)
    (c : Classified), classify a d di o = some c →
    ∀ (b : RuntimeBindings np nc) (s : Lifecycle.State np nc) (holds : List Nat → Prop),
    (∀ hash ∈ a.roots, holds hash) → ProbeSemantics a di b s holds →
    OrdinaryCoreConditions a d o b s →
    Lifecycle.permitted (c.pattern.operation b) s

theorem classify_permitted (a : CoreAction) (d : LifecyclePolicy) (di : Nat) (o : CoreOperation)
    (c : Classified) (classified : classify a d di o = some c)
    (b : RuntimeBindings np nc) (s : Lifecycle.State np nc) (holds : List Nat → Prop)
    (roots : ∀ hash ∈ a.roots, holds hash)
    (semantics : ProbeSemantics a di b s holds)
    (core : OrdinaryCoreConditions a d o b s) :
    Lifecycle.permitted (c.pattern.operation b) s := by
  unfold classify at classified
  dsimp only at classified
  split at classified
  · contradiction
  · split at classified
    · cases classified; trivial
    · split at classified
      · rename_i parent
        cases hk : o.operation.kind with
        | remove =>
          simp only [hk] at classified
          split at classified
          · cases classified
            have pe : o.operation.entity = d.parent := by
              simpa [parentRelevant] using (Bool.and_eq_true_iff.mp parent).1
            exact LifecycleDependency.parent_removal_integrity s (b.parent o.binding) (core.2.2.2 hk pe)
          · contradiction
        | create =>
          simp only [hk] at classified
          cases hi : initializer o d.parentField with
          | none => simp [hi] at classified
          | some value =>
            cases hv : memberValue d.parentValues d.prohibited value with
            | none => simp [hi,hv] at classified
            | some gone =>
              simp only [hi,hv,Option.bind_eq_bind,Option.bind_some,Option.some.injEq] at classified
              cases classified
              have pe : o.operation.entity = d.parent := by simpa [parentRelevant] using (Bool.and_eq_true_iff.mp parent).1
              exact core.2.1 hk pe
        | assign =>
          simp only [hk] at classified
          cases hv : memberValue d.parentValues d.prohibited o.value with
          | none => simp [hv] at classified
          | some gone =>
            simp only [hv,Option.bind_eq_bind,Option.bind_some] at classified
            split at classified
            · contradiction
            · split at classified
              · rename_i prohibited
                cases hp : a.probes.find? (fun p => p.dependency == di && p.parent == o.binding && a.roots.contains p.noChildren) with
                | none => simp only [hp,Option.bind_none] at classified; cases classified
                | some probe =>
                  simp only [hp,Option.bind_some,Option.some.injEq] at classified
                  cases classified
                  intro _
                  have found := List.find?_some hp
                  simp only [Bool.and_eq_true,beq_iff_eq] at found
                  have meaning := semantics probe (List.mem_of_find?_eq_some hp) found.1.1
                  simpa [found.1.2] using meaning.1 (roots _ (List.contains_iff_mem.mp found.2))
              · cases classified
                rename_i notGone
                intro gone
                simp_all
      · rename_i notParent
        cases hk : o.operation.kind with
        | remove =>
          simp only [hk] at classified
          split at classified
          · cases classified; trivial
          · contradiction
        | create =>
          simp only [hk] at classified
          cases hi : initializer o d.childField with
          | none => simp [hi] at classified
          | some value =>
            cases hv : memberValue d.childValues d.terminal value with
            | none => simp [hi,hv] at classified
            | some terminal =>
              cases hp : parentWitness a d di 0 true o terminal with
              | none => simp [hi,hv,hp] at classified
              | some probe =>
                simp only [hi,hv,hp,Option.bind_eq_bind,Option.bind_some,Option.some.injEq] at classified
                cases classified
                have cr : childRelevant d o.operation = true := by
                  simp_all
                have ce : o.operation.entity = d.child := by simpa [childRelevant] using (Bool.and_eq_true_iff.mp cr).1
                have facts := parentWitness_facts a d di 0 true terminal o probe hp
                refine ⟨core.2.2.1 hk ce,core.1 _ facts.2.2.2.1,?_⟩
                exact witness_child_permitted a d di 0 true terminal o probe hp b s holds roots semantics
        | assign =>
          simp only [hk] at classified
          split at classified
          · contradiction
          · cases hv : memberValue d.childValues d.terminal o.value with
            | none => simp [hv] at classified
            | some terminal =>
              simp only [hv,Option.bind_eq_bind,Option.bind_some] at classified
              split at classified
              · cases classified
                intro active
                simp_all
              · cases hp : parentWitness a d di o.binding false o terminal with
                | none => simp only [hp,Option.bind_none] at classified; cases classified
                | some probe =>
                  simp only [hp,Option.bind_some,Option.some.injEq] at classified
                  cases classified
                  intro active
                  have parent := witness_child_permitted a d di o.binding false terminal o probe hp b s holds roots semantics active
                  have facts := parentWitness_facts a d di o.binding false terminal o probe hp
                  have equality := (semantics probe facts.1 facts.2.1).2.2
                  have root : holds probe.ownerEquality := roots _ (List.contains_iff_mem.mp (by simpa using facts.2.2.2.2.1))
                  cases hc : s.children (b.child o.binding) with
                  | none => trivial
                  | some row =>
                    have same := equality root _ row (by rw [facts.2.2.1]) hc
                    simpa [same] using parent

-- Execution correspondence supplies the equality to the actual projected
-- result, including all irrelevant effects framed by projection_frame.
theorem classified_preserves (a : CoreAction) (d : LifecyclePolicy) (di : Nat) (o : CoreOperation)
    (c : Classified) (classified : classify a d di o = some c)
    (b : RuntimeBindings np nc) (s result : Lifecycle.State np nc) (holds : List Nat → Prop)
    (roots : ∀ hash ∈ a.roots, holds hash)
    (semantics : ProbeSemantics a di b s holds)
    (core : OrdinaryCoreConditions a d o b s)
    (execution : result = Lifecycle.apply (c.pattern.operation b) s)
    (initial : Lifecycle.safe s) : Lifecycle.safe result := by
  rw [execution]
  exact LifecycleDependency.sound _ s initial
    (classify_permitted a d di o c classified b s holds roots semantics core)

end Safety
