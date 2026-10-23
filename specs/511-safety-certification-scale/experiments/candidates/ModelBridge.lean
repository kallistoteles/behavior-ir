import safety.Check
import MergeWitness
import CompactPayload

namespace Safety.Candidate

-- Exactly the original non-uniqueness well-formedness premises.
def bounds (m : ModuleData) : Bool :=
  !m.policies.isEmpty && m.actionHashes.length == m.actions.length &&
  (m.actions.zipIdx.all fun (a,i) => a.identity == i &&
    a.operations.all fun op => op.entity < m.entityFields.length &&
      match op.kind with
      | .assign => op.field < m.entityFields[op.entity]!
      | _ => op.field == 0) &&
  (m.policies.all fun p => p.entity < m.entityFields.length &&
    (p.writers.all fun (field,as) => field < m.entityFields[p.entity]! &&
      as.all (· < m.actions.length)) &&
    p.creators.all (· < m.actions.length) && p.removers.all (· < m.actions.length))

theorem wellFormed_from_parts (m : ModuleData)
    (unique : m.actionHashes.Pairwise (· ≠ ·)) (other : bounds m = true) :
    wellFormed m = true := by
  simp only [wellFormed, bounds, Bool.and_eq_true_iff, decide_eq_true_eq] at *
  exact ⟨⟨⟨⟨other.1.1.1, other.1.1.2⟩, unique⟩, other.1.2⟩, other.2⟩

theorem checked_from_parts (m : ModuleData)
    (unique : m.actionHashes.Pairwise (· ≠ ·)) (other : bounds m = true)
    (authority : checkAuthority m = true) (link : checkLifecycleLink m = true)
    (lifecycle : checkLifecycle m.lifecycle = true) : checkModule m = true := by
  simp only [checkModule, Bool.and_eq_true_iff]
  exact ⟨⟨⟨wellFormed_from_parts m unique other, authority⟩, link⟩, lifecycle⟩

end Safety.Candidate
