"""Run neutral reference cases; request build-time proof checking with --certify."""
from __future__ import annotations

import argparse
import importlib
import json
import os
import subprocess
import behavior as core_api
from pathlib import Path
from typing import Sequence

from behavior import ModelBuildError, certify_safety, validate_safety_snapshot, Store, InMemoryBackend

def state_evidence(name, reference):
    model, store = reference.history()
    anchor = store.current_history()
    valid = validate_safety_snapshot(store, model, at=anchor)
    assert valid.status == "Valid"
    assert valid.matches_anchor(model, anchor)
    unknown = anchor.as_dict(); unknown["position"] += 1000
    missing = validate_safety_snapshot(store, model, at=unknown)
    assert missing.status == "Unestablished"
    parent, child = ("VM", "Resource") if name == "ld_vm_resource" else ("Parent", "Child")
    rows = [{"entity": parent, "value": {"id": "p", "phase": "Abandoned"}},
            {"entity": child, "value": {"id": "c", "phase": "Active", "owner": "p"}}]
    invalid_store = Store.create(InMemoryBackend(), model, Store.genesis_for(model, rows))
    invalid = validate_safety_snapshot(invalid_store, model)
    assert invalid.status == "Invalid"
    return {"validated": valid.to_dict(), "missing_base": missing.to_dict(), "invalid_base": invalid.to_dict(),
            "scope": "Only each recorded anchor; preservation and host authorization are separate"}


CASES = ("fo_field", "fo_state_machine", "ld_parent_child", "ld_shared_parent", "ld_vm_resource")


REQUIRED_ROOTS = (
    "Safety.FieldOwnership.sound", "Safety.LifecycleDependency.sound",
    "Safety.LifecycleDependency.conjunction", "Safety.LifecycleDependency.projection_frame",
    "Safety.membership_order_equivalent", "Safety.checkLifecycle_sound", "Safety.classify_permitted", "Safety.lifecycle_module_preserves",
    "Safety.checkModule_sound", "Safety.module_preserves", "Concrete.checked", "Concrete.certified",
    "Safety.Candidate.traceChecked_sound", "Safety.Candidate.compactPayload_flatten",
    "Safety.Candidate.literalBytes_injective", "Safety.Candidate.checked_from_parts")
REQUIRED_OBLIGATIONS = ("inventory_comparison", "generic_proof", "concrete_proof",
                        "theorem_types", "axiom_audit", "fresh_replay")


def required_assessment(doc):
    """Aggregate our trusted runner's outputs; importing a report grants no authority."""
    if doc.get("outcome") != "Certified under assumptions":
        return False
    if any(doc.get("obligations", {}).get(k) != "Passed" for k in REQUIRED_OBLIGATIONS):
        return False
    audit = doc.get("proof", {}).get("audit", [])
    if not isinstance(audit, list) or len(audit) != len(REQUIRED_ROOTS):
        return False
    if {row.get("declaration") for row in audit} != set(REQUIRED_ROOTS):
        return False
    allowed = {"propext", "Classical.choice", "Quot.sound"}
    return all(row.get("expected_type_checked") is True and isinstance(row.get("axioms"), list)
               and all(isinstance(a, str) and a in allowed for a in row["axioms"]) for row in audit)


def required_report(report):
    cases = report.get("cases", {})
    return set(cases) == set(CASES) and all(
        row.get("certified") is True and required_assessment(row.get("assessment", {}))
        and (not name.startswith("ld_") or row["assessment"].get("obligations", {}).get("lifecycle_preservation") == "Passed")
        for name, row in cases.items())


def rust_preflight(output):
    with (output / "rust-preflight.log").open("w") as log:
        subprocess.run(["cargo", "test", "-q", "-p", "behavior-models",
                        "--test", "safety_inventory", "--test", "safety_correspondence",
                        "--test", "safety_snapshot", "--test", "safety_deployment"],
                       cwd=Path(__file__).resolve().parents[3], stdout=log,
                       stderr=subprocess.STDOUT, check=True)


def new_guard_contract_report():
    from .guard_contract import VARIANTS
    return {"format":"behavior.safety_guard_contract.v1","cases":{name:{"status":"NotRun"} for name in VARIANTS},
            "complete":False,"passed":False,"errors":[],"versions":core_api.versions(),
            "claim":"Local Core checks and conditional inductive Safety preservation are separate evidence."}


def required_guard_row(name,row):
    from .guard_contract import VARIANTS
    if name not in VARIANTS or row.get("status")!="Complete":return False
    try:
        bh=row["behavior_hash"];core=row["core"];safety=row["safety"]
        att=core["attestation"];checks=core["checks"]
        if not isinstance(bh,str) or not bh.startswith("sha256:"):return False
        if json.loads(core["canonical_attestation"])!=att or att["behavior_version"]!=bh:return False
        if core["attestation_hash"]!=att["hash"] or core["result"]!=att["result"]:return False
        if att["verifier_version"]!=core_api.versions()["verifier"] or not isinstance(checks,list) or not checks or checks!=att["checks"]:return False
        if core["profile"]["checks"]!=["postcondition"]:return False
        if any(att["profile"].get(k)!=v for k,v in core["profile"].items()):return False
        if not row["postcondition_hashes"]:return False
        if len(checks)!=len(row["postcondition_hashes"]):return False
        if any(c.get("kind")!="postcondition" or not c.get("subject") or c["subject"].get("hash") not in row["postcondition_hashes"]
               or c["action"]["hash"]!=row["action_hash"] or not isinstance(c.get("outcome"),str) for c in checks):return False
        expected=name in {"canonical","permuted"}
        if safety["certificate_present"] is not expected:return False
        doc=safety["assessment"]
        if doc["behavior_hash"]!=bh or safety["outcome"]!=doc["outcome"] or safety["evidence_hash"]!=doc["evidence_hash"]:return False
        if safety["diagnostics"]!=doc.get("diagnostics",[]):return False
        return (required_assessment(doc) and doc["obligations"].get("lifecycle_preservation")=="Passed") if expected else doc["outcome"]=="Unsupported"
    except (KeyError,TypeError,ValueError):return False


def required_guard_contract(report):
    from .guard_contract import VARIANTS
    cases=report.get("cases",{})
    return (report.get("complete") is True and not report.get("errors")
            and report.get("versions",{}).get("core")==core_api.versions()["core"]
            and report.get("versions",{}).get("verifier")==core_api.versions()["verifier"]
            and report.get("versions",{}).get("engine")==core_api.versions()["engine"] and set(cases)==set(VARIANTS)
            and all(required_guard_row(name,row) for name,row in cases.items()))


def guard_contract(output,*,certify):
    from .guard_contract import VARIANTS,LOCAL_PROPERTY,build
    report=new_guard_contract_report()
    def save():
        (output/"guard-contract.json").write_text(json.dumps(report,sort_keys=True,indent=2)+"\n")
    save()
    try:
        if not certify:raise ValueError("--guard-contract requires --certify")
        if any(os.environ.get(k) for k in ("BEHAVIOR_LEAN_BINARY","BEHAVIOR_LEANCHECKER_BINARY","BEHAVIOR_CLI_BINARY")):
            raise ValueError("Pinned comparison rejects executable overrides")
        for name in VARIANTS:
            row=report["cases"][name]
            try:
                model=build(name);plain=build(name,safety=False)
                assert model.to_wire_json()==plain.to_wire_json() and model.model_receipts==plain.model_receipts
                inventory=model.safety_analysis.to_dict()["inventory"]["actions"]
                assert len(inventory)==1 and inventory[0]["postconditions"]
                row.update({"behavior_hash":model.behavior_version,"local_property":LOCAL_PROPERTY,
                            "action_hash":inventory[0]["hash"],"postcondition_hashes":inventory[0]["postconditions"]})
                profile=core_api.Profile(checks=["postcondition"])
                att=core_api.verify(model,profile)
                row["core"]={"profile":{"checks":profile.checks,"rlimit":profile.rlimit,"wall_clock_guard_ms":profile.wall_clock_guard_ms},
                    "attestation":att.data,"canonical_attestation":att.json,"attestation_hash":att.hash,"result":att.result,"checks":att.checks}
                assessment=certify_safety(model,output=output/name)
                doc=assessment.to_dict()
                row["safety"]={"outcome":assessment.outcome,"certificate_present":assessment.certificate is not None,
                               "diagnostics":doc.get("diagnostics",[]),"evidence_hash":doc.get("evidence_hash"),"assessment":doc}
                row["status"]="Complete"
                row["passed"]=required_guard_row(name,row)
            except Exception as error:
                row["status"]="Failed";row["error"]=str(error);report["errors"].append(f"{name}: {error}")
                save();break
            save()
        report["complete"]=all(row["status"]=="Complete" for row in report["cases"].values())
        report["passed"]=required_guard_contract(report)
    except Exception as error:
        report["errors"].append(str(error))
    save()
    return report


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--case", choices=(*CASES, "all"), default="all")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--certify", action="store_true")
    parser.add_argument("--guard-contract",action="store_true",help="Compare six Core/Safety guard outcomes (requires --certify)")
    parser.add_argument("--gate", action="store_true", help="Mandatory complete proof suite, including Rust preflight")
    args = parser.parse_args(argv)
    args.output.mkdir(parents=True, exist_ok=True)
    if args.guard_contract and not args.gate:
        comparison=guard_contract(args.output,certify=args.certify)
        return 0 if comparison["passed"] else 1
    selected = CASES if args.gate or args.case == "all" else (args.case,)
    report = {"format": "behavior.safety_reference_run.v1",
              "cases": {name: {"status": "NotRun", "certified": False} for name in selected},
              "errors": [], "passed": False, "rust_preflight": "NotRequested",
              "claim": "Ordinary-transition guarantees under assumptions; snapshot validity and host authorization are separate"}

    if args.gate:
        report["guard_contract"]=new_guard_contract_report()
        (args.output/"guard-contract.json").write_text(json.dumps(report["guard_contract"],sort_keys=True,indent=2)+"\n")

    def save():
        (args.output / "summary.json").write_text(json.dumps(report, sort_keys=True, indent=2) + "\n")
    save()
    try:
        if any(os.environ.get(key) for key in ("BEHAVIOR_LEAN_BINARY", "BEHAVIOR_LEANCHECKER_BINARY", "BEHAVIOR_CLI_BINARY")):
            raise ValueError("Pinned Safety proof path rejects executable overrides")
        if args.gate:
            if args.case != "all" or not args.certify:
                raise ValueError("Mandatory proof gate requires all five cases and certification")
            report["rust_preflight"] = "Running"
            save()
            rust_preflight(args.output)
            report["rust_preflight"] = "Passed"
    except Exception as error:
        report["errors"].append(str(error))
        if report["rust_preflight"] == "Running":
            report["rust_preflight"] = "Failed"
        save()
        return 1

    for name in selected:
        row = report["cases"][name]
        try:
            reference = importlib.import_module(f"models.examples.safety_models.{name}")
            model = reference.build()
            plain = reference.build(safety=False)
            assert model.to_wire_json() == plain.to_wire_json()
            assert model.model_receipts == plain.model_receipts
            try:
                reference.build(extra_writer=True)
            except ModelBuildError:
                pass
            else:
                raise AssertionError(f"{name}: unauthorized writer was accepted")
            row.update({"status": "Lowered", "behavior_hash": model.behavior_version,
                        "analysis": model.safety_analysis.to_dict(), "execution": reference.exercise()})
            if name.startswith("ld_"):
                row["state_validation"] = state_evidence(name, reference)
            if args.certify:
                try:
                    assessment = certify_safety(model, output=args.output / name)
                    row["assessment"] = assessment.to_dict()
                    row["status"] = assessment.outcome
                    row["certified"] = assessment.certificate is not None and required_assessment(row["assessment"])
                except Exception as error:
                    row["status"] = "Inconclusive"
                    row["error"] = str(error)
        except Exception as error:
            row["status"] = "Failed"
            row["error"] = str(error)
        save()
        print(f"{name}: {row['status']}", flush=True)
    if args.gate:
        report["guard_contract"]=guard_contract(args.output,certify=args.certify)
        passed = required_report(report) and report["rust_preflight"] == "Passed" and required_guard_contract(report["guard_contract"])
    else:
        passed = all(row["certified"] if args.certify else row["status"] == "Lowered" for row in report["cases"].values())
    report["passed"] = passed
    save()
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
