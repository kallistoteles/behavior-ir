import CandidateLemmas
open Safety.Candidate

example : uniqueChecked [[3],[1],[2]] = true := by
  simp +decide [uniqueChecked, List.mergeSort, List.MergeSort.Internal.splitInTwo, ordering, adjacent, less]
example : uniqueChecked [[1],[1]] = false := by
  simp +decide [uniqueChecked, List.mergeSort, List.MergeSort.Internal.splitInTwo, ordering, adjacent, less]
example (xs : List (List Nat)) (h : uniqueChecked xs = true) : xs.Pairwise (· ≠ ·) :=
  uniqueChecked_sound xs h
example : ¬ [[1],[1]].Pairwise (· ≠ ·) := by
  decide
