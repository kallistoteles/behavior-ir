import CandidateLemmas

namespace Safety.Candidate

-- Both inputs are original full keys. The producer's proposed ranks are never authority.
def forwardKeys (a b : Array (List Nat)) (r inverse : Array Nat) : Bool :=
  (List.range a.size).all fun i =>
    match a[i]?, r[i]? with
    | some key, some rank => b[rank]? == some key && inverse[rank]? == some i
    | _, _ => false

def witnessChecked (a b : Array (List Nat)) (r inverse : Array Nat) : Bool :=
  (a.size == b.size && a.size == r.size && b.size == inverse.size) &&
    (adjacent less b.toList && (forwardKeys a b r inverse && forwardKeys b a inverse r))

theorem forwardKeys_at (a b : Array (List Nat)) (r inverse : Array Nat)
    (checked : forwardKeys a b r inverse = true) (i : Nat) (hi : i < a.size) :
    ∃ rank : Nat, b[rank]? = some a[i] ∧ inverse[rank]? = some i := by
  have edge := List.all_eq_true.mp checked i (List.mem_range.mpr hi)
  cases hr : r[i]? with
  | none => simp [hr, Array.getElem?_eq_getElem hi] at edge
  | some rank =>
    simp only [Array.getElem?_eq_getElem hi, hr] at edge
    have both := Bool.and_eq_true_iff.mp edge
    exact ⟨rank, by simpa only [beq_iff_eq] using both.1,
      by simpa only [beq_iff_eq] using both.2⟩

theorem witnessChecked_sound (a b : Array (List Nat)) (r inverse : Array Nat)
    (checked : witnessChecked a b r inverse = true) : a.toList.Pairwise (· ≠ ·) := by
  have body := (Bool.and_eq_true_iff.mp checked).2
  have ordered := (Bool.and_eq_true_iff.mp body).1
  have forward := (Bool.and_eq_true_iff.mp (Bool.and_eq_true_iff.mp body).2).1
  have unique : b.toList.Nodup :=
    (adjacent_pairwise less less_trans b.toList ordered).imp (fun edge => less_ne edge)
  apply List.pairwise_iff_getElem.mpr
  intro i j hi hj hij equal
  have hai : i < a.size := by simpa using hi
  have haj : j < a.size := by simpa using hj
  obtain ⟨ri, keyi, invi⟩ := forwardKeys_at a b r inverse forward i hai
  obtain ⟨rj, keyj, invj⟩ := forwardKeys_at a b r inverse forward j haj
  have hri : ri < b.toList.length := by
    have bound := (Array.getElem?_eq_some_iff.mp keyi).1
    simpa using bound
  have keyeq : b.toList[ri]? = b.toList[rj]? := by
    rw [Array.getElem?_toList, Array.getElem?_toList, keyi, keyj]
    simpa using congrArg some equal
  have ranks : ri = rj := (List.getElem?_inj hri unique).mp keyeq
  rw [ranks] at invi
  have indices : i = j := Option.some.inj (invi.symm.trans invj)
  exact (Nat.ne_of_lt hij) indices

-- This is checked too: every key position in each direction has an exact inverse/key link.
theorem witnessChecked_coverage (a b : Array (List Nat)) (r inverse : Array Nat)
    (checked : witnessChecked a b r inverse = true) :
    (∀ i : Nat, ∀ hi : i < a.size, ∃ rank : Nat, b[rank]? = some a[i] ∧ inverse[rank]? = some i) ∧
    (∀ j : Nat, ∀ hj : j < b.size, ∃ index : Nat, a[index]? = some b[j] ∧ r[index]? = some j) := by
  have body := (Bool.and_eq_true_iff.mp checked).2
  have both := Bool.and_eq_true_iff.mp (Bool.and_eq_true_iff.mp body).2
  exact ⟨forwardKeys_at a b r inverse both.1, forwardKeys_at b a inverse r both.2⟩

end Safety.Candidate
