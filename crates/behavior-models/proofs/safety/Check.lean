import safety.FieldOwnership
import safety.LifecycleSequence

namespace Safety

structure ModuleData where
  behaviorHash : List Nat
  inventoryHash : List Nat
  policySetHash : List Nat
  inventoryPayload : List (List Nat)
  policyPayload : List (List Nat)
  actionHashes : List (List Nat)
  entityFields : List Nat
  actions : List Action
  policies : List Ownership
  lifecycle : LifecycleData := {}
  deriving Repr

def checkAuthority (m : ModuleData) : Bool :=
  m.actions.all fun a => m.policies.all fun p => a.operations.all (allowed p a.identity)

def wellFormed (m : ModuleData) : Bool :=
  !m.policies.isEmpty && m.actionHashes.length == m.actions.length &&
  decide (m.actionHashes.Pairwise (· ≠ ·)) &&
  (m.actions.zipIdx.all fun (a,i) => a.identity == i &&
    a.operations.all fun op => op.entity < m.entityFields.length &&
      match op.kind with
      | .assign => op.field < m.entityFields[op.entity]!
      | _ => op.field == 0) &&
  (m.policies.all fun p => p.entity < m.entityFields.length &&
    (p.writers.all fun (field,as) => field < m.entityFields[p.entity]! &&
      as.all (· < m.actions.length)) &&
    p.creators.all (· < m.actions.length) && p.removers.all (· < m.actions.length))

def checkLifecycleLink (m : ModuleData) : Bool :=
  if m.lifecycle.dependencies.isEmpty then m.lifecycle.actions.isEmpty
  else
    m.lifecycle.actions.length == m.actions.length &&
    (m.lifecycle.actions.zip m.actions).all (fun (core,action) => core.operations.map CoreOperation.operation == action.operations) &&
    m.lifecycle.dependencies.all fun d =>
      d.parent < m.entityFields.length && d.child < m.entityFields.length &&
      d.parentField < m.entityFields[d.parent]! && d.childField < m.entityFields[d.child]! && d.ownerField < m.entityFields[d.child]! &&
      d.controls.all (· < m.policies.length) &&
      ([(d.parent,d.parentField),(d.child,d.childField),(d.child,d.ownerField)].all fun (entity,field) =>
        d.controls.any fun index => match m.policies[index]? with
          | none => false
          | some p => p.entity == entity && p.writers.any (fun entry => entry.1 == field)) &&
      m.actions.all fun action => !(m.policies.all fun p => allowed p action.identity ⟨d.child,.assign,d.ownerField⟩)

def checkModule (m : ModuleData) : Bool :=
  (wellFormed m && checkAuthority m) && checkLifecycleLink m && checkLifecycle m.lifecycle

def CheckedAuthority : Prop :=
  ∀ (m : ModuleData), checkModule m = true →
    ∀ (action : Action), action ∈ m.actions → ∀ (p : Ownership), p ∈ m.policies →
    ∀ op ∈ action.operations, allowed p action.identity op = true

def ModuleGuarantee (m : ModuleData) : Prop :=
  ∀ {ne ni nf : Nat} (action : Action), action ∈ m.actions →
    ∀ (p : Ownership), p ∈ m.policies →
    ∀ (effects : List (Effect ne ni nf)) (s : State ne ni nf)
      (entity : Fin ne) (identity : Fin ni) (field : Fin nf),
    effects.map Effect.operation = action.operations →
    observe (execute effects s) entity identity field ≠ observe s entity identity field →
    ∃ effect ∈ effects, affects effect.operation entity.val field.val = true ∧
      allowed p action.identity effect.operation = true

def ModuleSoundness : Prop := ∀ m, checkModule m = true → ModuleGuarantee m

theorem checkModule_sound (m : ModuleData) (checked : checkModule m = true)
    (action : Action) (ha : action ∈ m.actions) (p : Ownership) (hp : p ∈ m.policies) :
    ∀ op ∈ action.operations, allowed p action.identity op = true := by
  have h := (Bool.and_eq_true_iff.mp (Bool.and_eq_true_iff.mp (Bool.and_eq_true_iff.mp checked).1).1).2
  exact List.all_eq_true.mp (List.all_eq_true.mp (List.all_eq_true.mp h action ha) p hp)

theorem module_preserves (m : ModuleData) (checked : checkModule m = true) : ModuleGuarantee m := by
  intro ne ni nf action ha p hp effects s entity identity field coverage changed
  exact FieldOwnership.sound p action effects s entity identity field coverage
    (checkModule_sound m checked action ha p hp) changed

def LifecycleModuleGuarantee (m : ModuleData) : Prop :=
  checkLifecycleLink m = true ∧ LifecyclePremises m.lifecycle ∧
  ∀ a ∈ m.lifecycle.actions, ∀ d di, (d,di) ∈ m.lifecycle.dependencies.zipIdx →
    ∀ {np nc} (b : RuntimeBindings np nc) (s result : Lifecycle.State np nc) (holds : List Nat → Prop),
      (∀ hash ∈ a.roots, holds hash) → ProbeSemantics a di b s holds →
      (∀ o ∈ a.operations, OrdinaryCoreConditions a d o b s) →
      result = LifecycleDependency.execute (projectedOperations a d di a.operations b) s →
      Lifecycle.safe s → Lifecycle.safe result

theorem lifecycle_module_preserves (m : ModuleData) (checked : checkModule m = true) : LifecycleModuleGuarantee m := by
  have h := (Bool.and_eq_true_iff.mp checked).2
  have link := (Bool.and_eq_true_iff.mp (Bool.and_eq_true_iff.mp checked).1).2
  refine ⟨link,checkLifecycle_sound m.lifecycle h,?_⟩
  intro a ha d di hd np nc b s result holds roots semantics core execution initial
  exact action_preserves m.lifecycle h a ha d di hd b s result holds roots semantics core execution initial

-- Identity equalities are part of the concrete proposition. The expected bytes
-- are supplied independently in the regenerated audit wrapper, so an otherwise
-- equivalent proof about another module/data record cannot pass that audit.
def InstanceGuarantee (m : ModuleData) (behavior inventory policies : List Nat) (payload policyPayload : List (List Nat)) : Prop :=
  m.behaviorHash = behavior ∧ m.inventoryHash = inventory ∧ m.policySetHash = policies ∧
  m.inventoryPayload = payload ∧ m.policyPayload = policyPayload ∧ ModuleGuarantee m ∧ LifecycleModuleGuarantee m


def LifecycleModuleSoundness : Prop := ∀ m, checkModule m = true → LifecycleModuleGuarantee m

end Safety
