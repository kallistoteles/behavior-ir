prelude
import safety.Data

namespace Safety.FieldOwnership
open Safety

theorem frame (effect : Effect ne ni nf) (s : State ne ni nf)
    (entity : Fin ne) (identity : Fin ni) (field : Fin nf)
    (irrelevant : affects effect.operation entity.val field.val = false) :
    observe (effect.apply s) entity identity field = observe s entity identity field := by
  rcases effect with ⟨target, id, kind⟩
  cases kind with
  | assign f value =>
    by_cases he : entity = target
    · subst entity
      have hf : field ≠ f := by
        intro eq; subst field
        simp [affects, Effect.operation] at irrelevant
      by_cases hi : identity = id
      · simp [Effect.apply, observe, hi, Option.map_map, hf, Function.comp_def]
      · simp [Effect.apply, observe, hi]
    · simp [Effect.apply, observe, he]
  | create row =>
    have he : entity ≠ target := by
      intro eq; subst entity
      simp [affects, Effect.operation] at irrelevant
    simp [Effect.apply, observe, he]
  | remove =>
    have he : entity ≠ target := by
      intro eq; subst entity
      simp [affects, Effect.operation] at irrelevant
    simp [Effect.apply, observe, he]

theorem frame_many (effects : List (Effect ne ni nf)) (s : State ne ni nf)
    (entity : Fin ne) (identity : Fin ni) (field : Fin nf)
    (irrelevant : ∀ e ∈ effects, affects e.operation entity.val field.val = false) :
    observe (execute effects s) entity identity field = observe s entity identity field := by
  induction effects generalizing s with
  | nil => rfl
  | cons e rest ih =>
    have tail := ih (e.apply s) (fun x hx => irrelevant x (List.mem_cons_of_mem e hx))
    exact tail.trans (frame e s entity identity field (irrelevant e (by simp)))

/- FO-1: complete operation coverage + structural authorization imply every
protected change has an authorized mutation witness. No reachability/value
analysis relaxes structural permissions. State cardinalities are arbitrary. -/
theorem sound (p : Ownership) (action : Action)
    (effects : List (Effect ne ni nf)) (s : State ne ni nf)
    (entity : Fin ne) (identity : Fin ni) (field : Fin nf)
    (coverage : effects.map Effect.operation = action.operations)
    (authority : ∀ op ∈ action.operations, allowed p action.identity op = true)
    (changed : observe (execute effects s) entity identity field ≠ observe s entity identity field) :
    ∃ effect ∈ effects, affects effect.operation entity.val field.val = true ∧
      allowed p action.identity effect.operation = true := by
  classical
  have witness : ∃ e ∈ effects, affects e.operation entity.val field.val = true := by
    apply Classical.byContradiction
    intro h
    apply changed
    apply frame_many
    intro e he
    cases hval : affects e.operation entity.val field.val with
    | false => rfl
    | true => exact False.elim (h ⟨e,he,hval⟩)
  obtain ⟨e,he,ha⟩ := witness
  refine ⟨e,he,ha,authority e.operation ?_⟩
  rw [← coverage]
  exact List.mem_map.mpr ⟨e,he,rfl⟩

-- The semantic execution functions above also cover irrelevant assignments,
-- initialization (creator permission), and physical removal (remover permission).
end Safety.FieldOwnership
