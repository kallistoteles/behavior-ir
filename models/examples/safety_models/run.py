"""Run neutral reference cases; request build-time proof checking with --certify."""
from __future__ import annotations

import argparse
import importlib
import json
import os
import subprocess
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
    "Safety.checkLifecycle_sound", "Safety.classify_permitted", "Safety.lifecycle_module_preserves",
    "Safety.checkModule_sound", "Safety.module_preserves", "Concrete.checked", "Concrete.certified")
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


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--case", choices=(*CASES, "all"), default="all")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--certify", action="store_true")
    parser.add_argument("--gate", action="store_true", help="Mandatory complete proof suite, including Rust preflight")
    args = parser.parse_args(argv)
    args.output.mkdir(parents=True, exist_ok=True)
    selected = CASES if args.gate or args.case == "all" else (args.case,)
    report = {"format": "behavior.safety_reference_run.v1",
              "cases": {name: {"status": "NotRun", "certified": False} for name in selected},
              "errors": [], "passed": False, "rust_preflight": "NotRequested",
              "claim": "Ordinary-transition guarantees under assumptions; snapshot validity and host authorization are separate"}

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
        passed = required_report(report) and report["rust_preflight"] == "Passed"
    else:
        passed = all(row["certified"] if args.certify else row["status"] == "Lowered" for row in report["cases"].values())
    report["passed"] = passed
    save()
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
