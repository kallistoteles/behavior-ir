"""An imported status is unverified data; verification regenerates fixed proofs."""
import copy
import json
from pathlib import Path
import pytest
import behavior as b

@pytest.fixture(scope="module")
def certified_bundle(tmp_path_factory):
    from models.examples.safety_models.fo_field import build
    model=build();output=tmp_path_factory.mktemp("safety-evidence")
    assessment=b.certify_safety(model,output=output)
    assert assessment.certificate is not None
    bundle=b.load_safety_bundle(output/"bundle.json")
    assert not bundle.is_verified
    return model,assessment,bundle,output

def test_public_bundle_verification_issues_an_opaque_verified_certificate(certified_bundle,tmp_path):
    model,assessment,bundle,_=certified_bundle
    assert bundle.claimed_outcome=="Certified under assumptions"
    assert not hasattr(bundle,"certificate")
    verified=b.verify_safety_evidence(model,bundle,output=tmp_path/"verification")
    assert verified.certificate is not None
    assert verified.to_dict()["evidence_hash"]==assessment.to_dict()["evidence_hash"]

@pytest.mark.parametrize("field",["behavior_hash","policy_set_hash","inventory","proof"])
def test_tampered_reports_do_not_become_certificates(certified_bundle,tmp_path,field):
    model,_,bundle,_=certified_bundle
    data=copy.deepcopy(bundle.to_dict())
    if field=="inventory":data["evidence"]["inventory"]["actions"].pop()
    elif field=="proof":data["evidence"]["proof"]["statement_version"]=999
    else:data["evidence"][field]="sha256:"+"0"*64
    result=b.verify_safety_evidence(model,data,output=tmp_path/field)
    assert result.certificate is None
    assert result.outcome=="Failed"

@pytest.mark.parametrize("key",["lean_source","compiled_proof","proof_files"])
def test_bundle_code_is_rejected_before_execution(certified_bundle,tmp_path,key):
    model,_,bundle,_=certified_bundle
    marker=tmp_path/"must-not-exist"
    data=bundle.to_dict();data[key]=f'run_io IO.FS.writeFile "{marker}" "executed"'
    with pytest.raises(b._engine.EngineError):
        b.verify_safety_evidence(model,data,output=tmp_path/"rejected")
    assert not marker.exists()

def test_wrong_tools_and_forged_flags_fail_closed(certified_bundle,tmp_path):
    model,_,bundle,_=certified_bundle
    with pytest.raises(b._engine.EngineError):
        b.verify_safety_evidence(model,bundle,output=tmp_path/"wrong-tool",toolchain=str(tmp_path/"fake"))
    data=bundle.to_dict();data["evidence"]["proof"]["audit"][0]["expected_type_checked"]=False
    result=b.verify_safety_evidence(model,data,output=tmp_path/"forged")
    assert result.certificate is None

def test_bundle_json_duplicate_keys_are_not_silently_normalized(tmp_path):
    path=tmp_path/"duplicate.json";path.write_text('{"format":"behavior.safety_bundle.v1","format":true}')
    with pytest.raises(b._engine.EngineError):b.load_safety_bundle(path)

def test_shared_lifecycle_bundle_reconstructs_the_complete_policy_set(tmp_path):
    from models.examples.safety_models.ld_shared_parent import build
    model=build();output=tmp_path/"shared"
    original=b.certify_safety(model,output=output)
    assert original.certificate is not None
    imported=b.load_safety_bundle(output/"bundle.json")
    verified=b.verify_safety_evidence(model,imported,output=tmp_path/"shared-verified")
    assert verified.certificate is not None,verified.to_dict()
    assert verified.to_dict()["evidence_hash"]==original.to_dict()["evidence_hash"]
    assert len(verified.to_dict()["policies"])==5
    # The reference host requires the opaque checked certificate and validates
    # the exact incoming state while excluding all coordinated writes.
    from models.examples.safety_models import ld_shared_parent as shared
    from models.examples.safety_models.deployment import CoordinatedHost
    _,store=shared.history()
    host=CoordinatedHost(store)
    with pytest.raises(TypeError):host.activate(model,verified.certificate.to_dict())
    record=host.activate(model,verified.certificate)
    assert record.status=="Valid"
    assert record.matches_anchor(model,store.current_history())
    assert host.invoke("reopen",bindings={"p":"p1"}).decision.result=="ALLOW"
    assert not record.matches_anchor(model,store.current_history())
    altered=imported.to_dict();altered["policies"].pop()
    rejected=b.verify_safety_evidence(model,altered,output=tmp_path/"missing-policy")
    assert rejected.certificate is None

@pytest.mark.parametrize("mode,code",[
    ("binding","SAFETY_PARENT_BINDING_REQUIRED"),
    ("association","SAFETY_OWNER_ASSOCIATION_REQUIRED"),
    ("combined","SAFETY_OWNER_ASSOCIATION_REQUIRED"),
    ("guard","SAFETY_PARENT_GUARD_REQUIRED"),
    ("cross","SAFETY_PARENT_GUARD_REQUIRED"),
])
def test_assessment_preserves_original_typed_parent_issues(tmp_path,mode,code):
    from .test_safety_lifecycle import diagnostic_model
    model,_=diagnostic_model(mode)
    original=model.safety_analysis.to_dict()["issues"]
    result=b.certify_safety(model,output=tmp_path)
    assert result.certificate is None and result.outcome=="Unsupported"
    assert result.to_dict()["diagnostics"]==original
    assert original[0]["code"]==code
    assert result.to_dict()["diagnostic_codes"]==[code]

@pytest.mark.parametrize("field,value", [
    ("nodes", "run_io IO.println \"untrusted\""),
    ("root", False),
    ("root", -1),
    ("format", "behavior.safety_merge_trace.v999"),
])
def test_witness_transport_is_strict_typed_data(certified_bundle, tmp_path, field, value):
    _, _, bundle, _ = certified_bundle
    data = bundle.to_dict()
    assert data["format"] == "behavior.safety_bundle.v2"
    data["witness"][field] = value
    with pytest.raises(b._engine.EngineError):
        b.load_safety_bundle(data)


def test_witness_never_accepts_source_or_compiled_code(certified_bundle, tmp_path):
    _, _, bundle, _ = certified_bundle
    marker = tmp_path / "execution-sentinel"
    for key in ("lean_source", "script", "olean", "success"):
        data = bundle.to_dict()
        assert data["format"] == "behavior.safety_bundle.v2"
        data["witness"][key] = f'run_io IO.FS.writeFile "{marker}" "executed"'
        with pytest.raises(b._engine.EngineError):
            b.load_safety_bundle(data)
        assert not marker.exists()


def test_version_two_transport_remains_unverified_even_with_a_success_claim():
    data = {"format": "behavior.safety_bundle.v2", "module": "{}", "inventory": {}, "policies": [],
            "witness": {"format": "behavior.safety_merge_trace.v1", "nodes": [{"kind": "leaf"}], "root": 0},
            "evidence": {"format": "behavior.safety_evidence.v2", "obligations": {}, "proof": {},
                         "behavior_hash": "x", "policy_set_hash": "p", "evidence_hash": "e",
                         "outcome": "Certified under assumptions"}}
    bundle = b.load_safety_bundle(data)
    assert not bundle.is_verified
    assert not hasattr(bundle, "certificate")


def test_previous_core_bundle_has_no_fresh_certification_authority(tmp_path):
    from models.examples.safety_models.fo_field import build
    archive = Path(__file__).resolve().parents[2] / "specs/512-adopt-core-update/evidence/baseline-safety-bundle.json"
    bundle = b.load_safety_bundle(archive)
    assert not bundle.is_verified
    result = b.verify_safety_evidence(build(), bundle, output=tmp_path / "old-core")
    assert result.certificate is None and result.outcome == "Failed"
    record = result.to_dict()
    assert record["diagnostic_codes"] == ["SAFETY_INVALID_DATA"]
    assert "wrong Core release" in json.dumps(record["diagnostics"])
    assert record["obligations"]["generic_proof"] == "NotRun"
