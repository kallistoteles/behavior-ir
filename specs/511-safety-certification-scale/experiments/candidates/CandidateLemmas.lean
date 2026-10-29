import Init

namespace Safety.Candidate

-- This checks only adjacent edges; transitivity supplies all distant pairs.
def adjacent (r : α → α → Bool) : List α → Bool
  | [] => true
  | [_] => true
  | a :: b :: rest => r a b && adjacent r (b :: rest)

theorem adjacent_pairwise (r : α → α → Bool)
    (trans : ∀ a b c, r a b = true → r b c = true → r a c = true)
    (xs : List α) (checked : adjacent r xs = true) :
    xs.Pairwise (fun a b => r a b = true) := by
  induction xs with
  | nil => exact .nil
  | cons a rest ih =>
    cases rest with
    | nil => simp
    | cons b tail =>
      have edges := Bool.and_eq_true_iff.mp checked
      have pairwise := ih edges.2
      apply List.Pairwise.cons
      · intro c hc
        rcases List.mem_cons.mp hc with eq | mem
        · subst c; exact edges.1
        · exact trans a b c edges.1 ((List.pairwise_cons.mp pairwise).1 c mem)
      · exact pairwise

def less (a b : List Nat) : Bool := compare a b == .lt

theorem less_trans (a b c : List Nat) (hab : less a b = true) (hbc : less b c = true) :
    less a c = true := by
  simp only [less, beq_iff_eq] at *
  exact Std.TransCmp.lt_trans hab hbc

theorem less_ne {a b : List Nat} (h : less a b = true) : a ≠ b := by
  intro eq
  subst b
  simp [less, Std.ReflCmp.compare_self] at h

def ordering (a b : List Nat) : Bool := compare a b != .gt

def uniqueChecked (xs : List (List Nat)) : Bool := adjacent less (xs.mergeSort ordering)

theorem uniqueChecked_sound (xs : List (List Nat)) (h : uniqueChecked xs = true) :
    xs.Pairwise (· ≠ ·) := by
  have sorted := adjacent_pairwise less less_trans (xs.mergeSort ordering) h
  have distinct := sorted.imp (fun edge => less_ne edge)
  exact (List.mergeSort_perm xs ordering).nodup distinct

-- An index is untrusted. Exact value checking implies original list membership.
theorem indexed_permission_sound (writers : Array Nat) (position action : Nat)
    (checked : writers[position]? = some action) : writers.toList.contains action = true := by
  exact List.contains_iff_mem.mpr (Array.mem_toList_iff.mpr (Array.mem_of_getElem? checked))

end Safety.Candidate
