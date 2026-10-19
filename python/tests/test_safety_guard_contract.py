"""Public local-Core versus inductive-Safety comparison, never a consumer app."""
import copy
import json
from pathlib import Path
import pytest
import behavior as b
from models.examples.safety_models import run

VARIANTS=("canonical","permuted","equivalent_unsupported","missing","wrong_parent","insufficient_or")

@pytest.fixture(scope="module")
def comparison(tmp_path_factory):
    output=tmp_path_factory.mktemp("guard-contract")
    assert run.main(["--guard-contract","--certify","--output",str(output)])==0
    return json.loads((output/"guard-contract.json").read_text())

def test_public_mode_records_six_exact_independent_outcomes(comparison):
    assert comparison["complete"] is True and comparison["passed"] is True
    assert set(comparison["cases"])==set(VARIANTS)
    hashes=set()
    for name,row in comparison["cases"].items():
        hashes.add(row["behavior_hash"])
        core=row["core"]
        assert core["checks"] and all(c["kind"]=="postcondition" and c["subject"] for c in core["checks"])
        assert json.loads(core["canonical_attestation"])==core["attestation"]
        assert core["attestation"]["behavior_version"]==row["behavior_hash"]
        assert core["checks"]==core["attestation"]["checks"]
        assert core["profile"]["checks"]==["postcondition"]
        expected=name in {"canonical","permuted"}
        assert row["safety"]["certificate_present"] is expected
        assert row["safety"]["outcome"]==("Certified under assumptions" if expected else "Unsupported")
        assert row["safety"]["assessment"]["behavior_hash"]==row["behavior_hash"]
        assert row["safety"]["evidence_hash"]
    assert len(hashes)==6

def test_incomplete_or_contradictory_comparisons_fail_closed(comparison):
    assert run.required_guard_contract(comparison)
    for mutation in ["row","checks","certificate","proof"]:
        doc=copy.deepcopy(comparison)
        if mutation=="row":doc["cases"].pop("missing")
        elif mutation=="checks":doc["cases"]["canonical"]["core"]["checks"]=[]
        elif mutation=="certificate":doc["cases"]["missing"]["safety"]["certificate_present"]=True
        else:doc["cases"]["canonical"]["safety"]["assessment"]["obligations"]["fresh_replay"]="NotRun"
        assert not run.required_guard_contract(doc),mutation

def test_new_mode_requires_explicit_certification(tmp_path):
    assert run.main(["--guard-contract","--output",str(tmp_path)])!=0
    doc=json.loads((tmp_path/"guard-contract.json").read_text())
    assert doc["passed"] is False and doc["errors"]
    assert all(r["status"]=="NotRun" for r in doc["cases"].values())

def test_child_examples_require_parent_checks_even_with_unchanged_owner():
    from models.examples.safety_models.guard_contract import child_transition_examples
    examples=child_transition_examples()
    assert set(examples)=={"Planned → Created","Terminal → Active"}
    for row in examples.values():
        assert row["owner_unchanged"] is True
        assert row["guarded"]["issues"]==[]
        assert [i["code"] for i in row["unguarded"]["issues"]]==["SAFETY_PARENT_GUARD_REQUIRED"]

def test_genuine_core_resource_inconclusive_is_separate_from_safety(comparison):
    from models.examples.safety_models.guard_contract import build
    model=build("permuted")
    assert model.behavior_version==comparison["cases"]["permuted"]["behavior_hash"]
    actual=b.verify(model,b.Profile(checks=["postcondition"],rlimit=1))
    assert actual.checks
    assert any(c["outcome"]=="inconclusive" for c in actual.checks),actual.data
    assert comparison["cases"]["permuted"]["safety"]["certificate_present"] is True
