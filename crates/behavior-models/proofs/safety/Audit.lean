import Lean
import Lean.Util.CollectAxioms
import safety.Check

namespace Safety
open Lean Elab Command Term Meta

def permittedAxiom (name : Name) : Bool :=
  #[``propext, ``Classical.choice, ``Quot.sound].contains name

def auditAllowed (name : Name) : CoreM Bool := do
  return (← collectAxioms name).all permittedAxiom

def auditExpected (name : Name) (expected : Expr) : MetaM Json := do
  let info ← getConstInfo name
  unless ← isDefEq info.type expected do
    throwError "Safety theorem {name} has the wrong proposition"
  let axioms ← collectAxioms name
  unless axioms.all permittedAxiom do
    throwError "Safety theorem {name} uses forbidden axioms: {axioms}"
  let sorted := axioms.map toString |>.qsort (· < ·)
  return Json.mkObj [("declaration",toJson name.toString),
    ("axioms",toJson sorted), ("expected_type_checked",toJson true)]

-- Fixed wrappers use this command. Output is structured JSON, not #print text.
elab "#safety_audit " root:ident " : " expected:term : command => do
  let report ← liftTermElabM do
    let type ← elabTerm expected none
    let type ← instantiateMVars type
    auditExpected root.getId type
  liftIO <| IO.FS.withFile "audit.jsonl" .append fun h => h.putStrLn report.compress

end Safety
