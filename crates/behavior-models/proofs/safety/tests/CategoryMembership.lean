import safety.LifecycleCheck

-- The kernel derives enum category membership from raw policy members, rather
-- than accepting a Boolean category verdict supplied by Rust.
example : Safety.memberValue [([10],[50]),([11],[51])] [[50]] [10] = some true := by decide
example : Safety.memberValue [([10],[50]),([11],[51])] [[50]] [11] = some false := by decide
example : Safety.memberValue [([10],[50]),([11],[51])] [[50]] [99] = none := by decide

example : Safety.validCategoryTable [([10],[50]),([11],[51])] [[50]] = true := by decide
example : Safety.validCategoryTable [([10],[50]),([11],[51])] [[99]] = false := by decide
example : Safety.validCategoryTable [([10],[50]),([10],[51])] [[50]] = false := by decide
example : Safety.validCategoryTable [([10],[50]),([11],[50])] [[50]] = false := by decide

-- Exact sets admit permutations/duplicates; omissions/extras are rejected.
example : Safety.sameMembers [[1],[2],[1]] [[2],[1]] = true := by decide
example : Safety.sameMembers [[1]] [[1],[2]] = false := by decide
example : Safety.sameMembers [[1],[2],[3]] [[1],[2]] = false := by decide
example : (∀ value, value ∈ [[2],[1],[2]] ↔ value ∈ [[1],[2]]) :=
  Safety.membership_order_equivalent _ _ (by decide)

def membershipPolicy : Safety.LifecyclePolicy :=
  ⟨0,1,0,0,1,0,[([10],[1]),([11],[2])],[([12],[1]),([13],[2])],[[1],[2]],[[1],[2]],[0]⟩
def membershipProbe : Safety.GuardProbe :=
  ⟨0,0,1,[20],[21],[22],[30],[[2],[1],[2]],[[1],[2]]⟩
example : Safety.validGuardMembers [membershipPolicy] membershipProbe = true := by decide
example : Safety.validGuardMembers [membershipPolicy] {membershipProbe with noChildrenMembers := [[1]]} = false := by decide
example : Safety.validGuardMembers [membershipPolicy] {membershipProbe with permittedParentMembers := [[1],[2],[3]]} = false := by decide
example : Safety.validGuardMembers [membershipPolicy] {membershipProbe with dependency := 1} = false := by decide
