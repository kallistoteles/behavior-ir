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
