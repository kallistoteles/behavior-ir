import safety.LifecycleCheck

namespace Safety

-- Preserve source order and every occurrence. None is blocked by finite checking;
-- it can never be treated as an irrelevant successful certification.
def selected (a : CoreAction) (d : LifecyclePolicy) (di : Nat) (ops : List CoreOperation) : List (CoreOperation × Classified) :=
  ops.filterMap fun o => (classify a d di o).map fun c => (o,c)

private theorem classify_irrelevant (a : CoreAction) (d : LifecyclePolicy) (di : Nat)
    (ds : List LifecyclePolicy) (hd : d ∈ ds) (o : CoreOperation)
    (irrelevant : relevant ds o.operation = false) : classify a d di o = some ⟨.irrelevant,[]⟩ := by
  have h := List.any_eq_false.mp irrelevant d hd
  have hf : (parentRelevant d o.operation || childRelevant d o.operation) = false := by
    cases he : (parentRelevant d o.operation || childRelevant d o.operation) <;> simp_all
  have h := Bool.or_eq_false_iff.mp hf
  simp [classify,h.1,h.2]

def projectedOperations (a : CoreAction) (d : LifecyclePolicy) (di : Nat)
    (ops : List CoreOperation) (b : RuntimeBindings np nc) : List (Lifecycle.Operation np nc) :=
  (selected a d di ops).map fun pair => pair.2.pattern.operation b

private theorem active_bound (a : CoreAction) (d : LifecyclePolicy) (di : Nat)
    (ds : List LifecyclePolicy) (hd : d ∈ ds) (ops : List CoreOperation) (b : RuntimeBindings np nc) :
    ((projectedOperations a d di ops b).filter LifecycleDependency.nontrivial).length ≤
    (ops.filter fun o => relevant ds o.operation).length := by
  induction ops with
  | nil => simp [projectedOperations,selected]
  | cons o rest ih =>
    cases hc : classify a d di o with
    | none =>
      cases hr : relevant ds o.operation with
      | false => simpa [projectedOperations,selected,hc,hr] using ih
      | true => simpa [projectedOperations,selected,hc,hr] using Nat.le_succ_of_le ih
    | some c =>
      cases hr : relevant ds o.operation with
      | false =>
        have inactive := classify_irrelevant a d di ds hd o hr
        have eq : c = ⟨.irrelevant,[]⟩ := Option.some.inj (hc.symm.trans inactive)
        subst c
        simpa [projectedOperations,selected,hc,hr,LifecyclePattern.operation,LifecycleDependency.nontrivial] using ih
      | true =>
        cases hp : LifecycleDependency.nontrivial (c.pattern.operation b) with
        | false => simpa [projectedOperations,selected,hc,hr,hp] using Nat.le_succ_of_le ih
        | true => simpa [projectedOperations,selected,hc,hr,hp] using Nat.succ_le_succ ih

private theorem projected_guards (a : CoreAction) (d : LifecyclePolicy) (di : Nat)
    (b : RuntimeBindings np nc) (s : Lifecycle.State np nc) (holds : List Nat → Prop)
    (roots : ∀ hash ∈ a.roots, holds hash) (semantics : ProbeSemantics a di b s holds)
    (core : ∀ o ∈ a.operations, OrdinaryCoreConditions a d o b s) :
    ∀ op ∈ projectedOperations a d di a.operations b, Lifecycle.permitted op s := by
  intro op member
  obtain ⟨pair,hpair,rfl⟩ := List.mem_map.mp member
  obtain ⟨o,ho,eq⟩ := List.mem_filterMap.mp hpair
  cases hc : classify a d di o with
  | none => simp [hc] at eq
  | some c =>
    have same : (o,c) = pair := by simpa [hc] using eq
    subst pair
    exact classify_permitted a d di o c hc b s holds roots semantics (core o ho)

-- All source guards hold in one source state. Union footprint checking plus
-- projection frames makes every ignored effect mathematically irrelevant.
theorem action_preserves (m : LifecycleData) (checked : checkLifecycle m = true)
    (a : CoreAction) (ha : a ∈ m.actions) (d : LifecyclePolicy) (di : Nat)
    (hd : (d,di) ∈ m.dependencies.zipIdx)
    (b : RuntimeBindings np nc) (s result : Lifecycle.State np nc) (holds : List Nat → Prop)
    (roots : ∀ hash ∈ a.roots, holds hash) (semantics : ProbeSemantics a di b s holds)
    (core : ∀ o ∈ a.operations, OrdinaryCoreConditions a d o b s)
    (execution : result = LifecycleDependency.execute (projectedOperations a d di a.operations b) s)
    (initial : Lifecycle.safe s) : Lifecycle.safe result := by
  have premises := checkLifecycle_sound m checked a ha
  have bound := active_bound a d di m.dependencies (List.fst_mem_of_mem_zipIdx hd) a.operations b
  rw [execution]
  exact LifecycleDependency.sequence_preserves _ s (Nat.le_trans bound premises.1)
    (projected_guards a d di b s holds roots semantics core) initial

end Safety
