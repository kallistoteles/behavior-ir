import MergeWitness
open Safety.Candidate
example : traceChecked [[2],[1]] (.branch .leaf .leaf [false,true]) = true := by decide
example : traceChecked [[2],[1]] (.branch .leaf .leaf [true,false]) = false := by decide
example : traceChecked [[1],[1]] (.branch .leaf .leaf [true,false]) = false := by decide
example : traceChecked [[1],[2]] .leaf = false := by decide
example : traceChecked [[1]] (.branch .leaf .leaf [true,false]) = false := by decide
example (xs : List (List Nat)) (t : MergeTrace) (h : traceChecked xs t = true) : xs.Pairwise (· ≠ ·) :=
  traceChecked_sound xs t h
-- Different valid tree shapes certify the same authoritative input.
example : traceChecked [[4],[1],[3],[2]]
  (.branch (.branch .leaf .leaf [false,true]) (.branch .leaf .leaf [false,true])
    [true,false,false,true]) = true := by decide
example : traceChecked [[4],[1],[3],[2]]
  (.branch .leaf (.branch .leaf (.branch .leaf .leaf [false,true])
    [true,false,false]) [false,false,false,true]) = true := by decide
-- Missing/extra consumption cannot hide an input or invent an element.
example : traceChecked [[1],[2]] (.branch .leaf .leaf [true]) = false := by decide
example : traceChecked [[1],[2]] (.branch .leaf .leaf [true,false,true]) = false := by decide
example : traceChecked [[1],[2],[3]] (.branch .leaf .leaf [true,false]) = false := by decide
example : traceChecked [[1],[2]] (.branch .empty .leaf [false]) = false := by decide
