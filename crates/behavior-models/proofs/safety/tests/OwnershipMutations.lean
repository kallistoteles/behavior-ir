import safety.Check
import safety.Audit

-- Require the versioned public propositions, positive kernel controls and
-- premise-removal witnesses together; blanket failure must not pass this test.
#safety_audit Safety.FieldOwnership.sound : Safety.OwnershipGuarantee
#safety_audit Safety.checkModule_sound : Safety.CheckedAuthority
#safety_audit Safety.module_preserves : Safety.ModuleSoundness

open Safety
def deniedPolicy : Ownership := ⟨0,[(0,[])],[],[]⟩
def assigned : Operation := ⟨0,.assign,0⟩
def created : Operation := ⟨0,.create,0⟩
def removed : Operation := ⟨0,.remove,0⟩
def write : Effect 1 1 1 := ⟨0,0,.assign 0 1⟩
def before : State 1 1 1 := fun _ _ => some (fun _ => 0)

-- These witnesses are accepted by the kernel, while each attempted false
-- permission/premise is required to fail. This is not a blanket-failing harness.
example : observe (write.apply before) 0 0 0 ≠ observe before 0 0 0 := by decide
example : allowed deniedPolicy 0 assigned = false := by decide
example : allowed deniedPolicy 0 created = false := by decide
example : allowed deniedPolicy 0 removed = false := by decide
example : allowed ⟨0,[(0,[])],[0],[]⟩ 0 created = true := by decide
example : allowed ⟨0,[(0,[0])],[],[]⟩ 0 assigned = true := by decide
example : ¬ ∃ effect ∈ [write], affects effect.operation 0 0 = true ∧
    allowed deniedPolicy 0 effect.operation = true := by decide

-- Dropping structural authority would admit a genuine changed field.
example : True := by
  fail_if_success have : allowed deniedPolicy 0 assigned = true := by decide
  trivial

-- An empty declared action is vacuously authorized but does not describe write.
example : ([] : List Operation).all (allowed deniedPolicy 0) = true := by decide
example : [write].map Effect.operation ≠ ([] : List Operation) := by decide
example : True := by
  fail_if_success have : [write].map Effect.operation = [] := by decide
  trivial

-- Without an effect/frame relation an unrelated state may change arbitrarily.
example : affects (⟨1,.assign,0⟩ : Operation) 0 0 = false := by decide
example : True := by
  fail_if_success have : observe (write.apply before) 0 0 0 = observe before 0 0 0 := by decide
  trivial

-- Proof holes and arbitrary axioms are accepted by Lean's elaborator, but must
-- be rejected by our separate transitive dependency audit.
axiom CustomUnsafe : False
theorem customUnsafe : False := CustomUnsafe
theorem holeUnsafe : False := by sorry
theorem nativeUnsafe : True := Lean.trustCompiler

-- An audit that accepts either root makes this module fail.
run_cmd do
  unless !(← Lean.Elab.Command.liftCoreM (Safety.auditAllowed ``customUnsafe)) do throwError "accepted custom axiom"
  unless !(← Lean.Elab.Command.liftCoreM (Safety.auditAllowed ``holeUnsafe)) do throwError "accepted sorryAx"
  unless !(← Lean.Elab.Command.liftCoreM (Safety.auditAllowed ``nativeUnsafe)) do throwError "accepted native oracle"

-- A theorem of True is not a proof of the intended ownership statement.
theorem weakened : True := True.intro
run_cmd do
  let rejected ← Lean.Elab.Command.liftTermElabM do
    try
      discard <| Safety.auditExpected ``weakened (Lean.mkConst ``Safety.OwnershipGuarantee)
      return false
    catch _ => return true
  unless rejected do throwError "accepted weakened statement"

-- Checker 3: producer data is not a completeness or uniqueness assumption.
open Safety.Candidate
example : traceChecked [[2],[1]] (.branch .leaf .leaf [false,true]) = true := by decide
example : traceChecked [[1],[1]] (.branch .leaf .leaf [true,false]) = false := by decide
example : traceChecked [[1],[2]] .leaf = false := by decide
example : traceChecked [[1],[2]] (.branch .leaf .leaf [true,false,true]) = false := by decide
example : traceChecked [[1]] (.branch .leaf .leaf [true,false]) = false := by decide
example (keys : List (List Nat)) (trace : MergeTrace) (h : traceChecked keys trace = true) :
    keys.Pairwise (· ≠ ·) := traceChecked_sound keys trace h
example : literalBytes "å" = [195,165] := by decide
example : (compactPayload "exact").flatten = literalBytes "exact" := compactPayload_flatten _
example (a b : String) (h : literalBytes a = literalBytes b) : a = b := literalBytes_injective h
-- Kernel-valid weakening is not a proof of checker soundness.
run_cmd do
  let rejected ← Lean.Elab.Command.liftTermElabM do
    try
      discard <| Safety.auditExpected ``weakened (← Lean.Elab.Term.elabTerm (← `(∀ (xs : List (List Nat)) (t : Safety.Candidate.MergeTrace), Safety.Candidate.traceChecked xs t = true → xs.Pairwise (· ≠ ·))) none)
      return false
    catch _ => return true
  unless rejected do throwError "accepted weakened trace soundness"
#safety_audit Safety.Candidate.traceChecked_sound : ∀ (xs : List (List Nat)) (t : Safety.Candidate.MergeTrace), Safety.Candidate.traceChecked xs t = true → xs.Pairwise (· ≠ ·)
#safety_audit Safety.Candidate.literalBytes_injective : ∀ {a b : String}, Safety.Candidate.literalBytes a = Safety.Candidate.literalBytes b → a = b
#safety_audit Safety.Candidate.compactPayload_flatten : ∀ s, (Safety.Candidate.compactPayload s).flatten = Safety.Candidate.literalBytes s
#safety_audit Safety.Candidate.checked_from_parts : ∀ (m : Safety.ModuleData), m.actionHashes.Pairwise (· ≠ ·) → Safety.Candidate.bounds m = true → Safety.checkAuthority m = true → Safety.checkLifecycleLink m = true → Safety.checkLifecycle m.lifecycle = true → Safety.checkModule m = true
