import safety.LifecycleDependency
import safety.Audit

#safety_audit Safety.LifecycleDependency.sound : Safety.LifecycleGuarantee

-- These are mathematical projections of Core state, not runtime storage.
open Safety.Lifecycle
def active : State 1 1 := ⟨fun _ => some false, fun _ => some ⟨0,false⟩⟩
def terminal : State 1 1 := ⟨fun _ => some true, fun _ => some ⟨0,true⟩⟩
def empty : State 1 1 := ⟨fun _ => some false, fun _ => none⟩
def noParent : State 1 1 := ⟨fun _ => none, fun _ => none⟩

-- Positive states and rejected preservation premises are checked together.
example : safe active := by decide
example : safe terminal := by decide
example : safe empty := by decide
example : safe noParent := by decide
-- An irrelevant projection has a defined identity/frame behavior. An arbitrary
-- changed result cannot be substituted for it by dropping correspondence.
example : apply .irrelevant active = active := rfl

-- Missing no-nonterminal-children guard.
example : ¬ safe (apply (.parentState 0 true) active) := by decide
example : ¬ permitted (.parentState 0 true) active := by decide
-- Missing reactivation guard.
example : ¬ safe (apply (.childState 0 false) terminal) := by decide
example : ¬ permitted (.childState 0 false) terminal := by decide
-- Missing creation freshness lets creation overwrite an existing safe parent.
example : ¬ safe (apply (.parentCreate 0 true) active) := by decide
example : ¬ permitted (.parentCreate 0 true) active := by decide
-- Even a terminal child requires an existing parent.
example : ¬ safe (apply (.childCreate 0 0 true) noParent) := by decide
example : ¬ permitted (.childCreate 0 0 true) noParent := by decide
-- Result-state Ref integrity must block physical deletion with surviving children.
example : ¬ safe (apply (.parentRemove 0) terminal) := by decide
example : ¬ permitted (.parentRemove 0) terminal := by decide
-- Two operations can each pass source-state guards and jointly break safety.
example : permitted (.parentState 0 true) empty := by decide
example : permitted (.childCreate 0 0 false) empty := by decide
example : ¬ safe (apply (.childCreate 0 0 false) (apply (.parentState 0 true) empty)) := by decide
-- The wrong parent binding cannot justify reactivation of the actual owner.
def wrongOwner : State 2 1 := ⟨fun p => if p=0 then some true else some false,fun _ => some ⟨0,true⟩⟩
example : safe wrongOwner := by decide
example : wrongOwner.parents 1 = some false := by decide
example : ¬ safe (apply (.childState 0 false) wrongOwner) := by decide
-- One checked dependency does not establish preservation for another.
example : safe (apply (.parentState 0 true) terminal) := by decide
example : ¬ safe (apply (.parentState 0 true) active) := by decide

-- Independent finite-input controls. Actual role/guard/value data is changed;
-- no Rust-supplied acceptance or relevance flags are used by this checker.
open Safety

def policy : LifecyclePolicy := ⟨0,1,0,0,1,0,[([10],[50]),([11],[51])],[([12],[52]),([13],[53])],[[50]],[[53]],[0]⟩
def probe : GuardProbe := ⟨0,0,1,[20],[21],[22],[30]⟩
def parentWrite : CoreOperation := ⟨⟨0,.assign,0⟩,0,[10],[]⟩
def childWrite : CoreOperation := ⟨⟨1,.assign,0⟩,1,[12],[]⟩
def goodAction : CoreAction := ⟨[(0,true),(1,true)],[[20],[21],[22]],[parentWrite],[probe]⟩
def one (action : CoreAction) (p : LifecyclePolicy := policy) : LifecycleData := ⟨[action],[p]⟩
example : checkLifecycle (one goodAction) = true := by decide
example : checkLifecycle (one {goodAction with operations := [childWrite]}) = true := by decide
example : checkLifecycle (one {goodAction with roots := []}) = false := by decide
example : checkLifecycle (one {goodAction with parameters := [(0,false),(1,true)]}) = false := by decide
example : checkLifecycle (one {goodAction with operations := [{parentWrite with value := [99]}]}) = false := by decide
example : checkLifecycle (one goodAction {policy with referenceParent := 2}) = false := by decide
example : checkLifecycle (one {goodAction with probes := [{probe with parent := 1}]}) = false := by decide
example : checkLifecycle (one {goodAction with operations := [parentWrite,childWrite]}) = false := by decide
example : checkLifecycle (one {goodAction with operations := [⟨⟨1,.assign,1⟩,1,[30],[]⟩]}) = false := by decide
example : checkLifecycle (one {goodAction with operations := [⟨⟨1,.create,0⟩,0,[],[(0,[12]),(1,[30])]⟩],probes := [{probe with child := 0}]}) = true := by decide
example : checkLifecycle (one {goodAction with operations := [⟨⟨1,.create,0⟩,0,[],[(0,[12]),(1,[99])]⟩],probes := [{probe with child := 0}]}) = false := by decide
example : checkLifecycle (one goodAction {policy with child := 0}) = false := by decide
-- Shared-parent operation counts once, but the second dependent's own guard
-- and binding descriptor cannot be omitted.
def second : LifecyclePolicy := {policy with child := 2}
def secondProbe : GuardProbe := {probe with dependency := 1,noChildren := [23]}
def shared : LifecycleData := ⟨[{goodAction with probes := [probe,secondProbe],roots := [[20],[21],[22],[23]]}],[policy,second]⟩
example : checkLifecycle shared = true := by decide
example : checkLifecycle {shared with actions := [{goodAction with probes := [probe,secondProbe]}]} = false := by decide
example : checkLifecycle {shared with actions := [{goodAction with roots := [[20],[21],[22],[23]]}]} = false := by decide

-- A broken concrete safety claim is rejected by the kernel while its explicit
-- violating witness above is accepted. Missing premises cannot become axioms.
example : True := by
  fail_if_success have badClaim : safe (apply (.parentState 0 true) active) := by decide
  fail_if_success have badClaim : safe (apply (.childState 0 false) terminal) := by decide
  fail_if_success have badClaim : safe (apply (.parentCreate 0 true) active) := by decide
  fail_if_success have badClaim : safe (apply (.childCreate 0 0 true) noParent) := by decide
  fail_if_success have badClaim : safe (apply (.parentRemove 0) terminal) := by decide
  fail_if_success have badClaim : checkLifecycle (one {goodAction with roots := []}) = true := by decide
  trivial

-- One Middle assignment is a child update under one policy and a parent update
-- under another. Even independently safe instances remain outside the v1 profile.
def chainedParent : LifecyclePolicy := {policy with parent := 2,child := 0,referenceParent := 2,childValues := [([10],[50]),([11],[51])],terminal := [[50]]}
example : checkLifecycle (one goodAction chainedParent) = true := by decide
example : checkLifecycle ⟨[goodAction],[policy,chainedParent]⟩ = false := by decide
