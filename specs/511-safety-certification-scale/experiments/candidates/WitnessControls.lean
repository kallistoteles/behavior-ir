import KeyWitness
open Safety.Candidate
example : witnessChecked #[[2],[1],[3]] #[[1],[2],[3]] #[1,0,2] #[1,0,2] = true := by decide
example : witnessChecked #[[1],[1]] #[[1],[2]] #[0,0] #[0,1] = false := by decide
example : witnessChecked #[[2],[1],[3]] #[[1],[2]] #[1,0,1] #[1,0] = false := by decide
example : witnessChecked #[[2],[1]] #[[1],[3]] #[1,0] #[1,0] = false := by decide
example : witnessChecked #[[2],[1]] #[[1],[2]] #[1,0] #[0,1] = false := by decide
example (h q : Array (List Nat)) (r inv : Array Nat)
    (checked : witnessChecked h q r inv = true) : h.toList.Pairwise (· ≠ ·) :=
  witnessChecked_sound h q r inv checked
