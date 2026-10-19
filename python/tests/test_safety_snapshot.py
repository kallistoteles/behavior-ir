"""State validity is exact, provenance-bound and independent of proof certification."""
import copy
import behavior as b
import pytest
from models.examples.safety_models import ld_parent_child as reference


def seeded(phase="Open",child_phase="Active"):
    model=reference.build()
    store=b.Store.create(b.InMemoryBackend(),model,b.Store.genesis_for(model,[
        {"entity":"Parent","value":{"id":"p","phase":phase}},
        {"entity":"Child","value":{"id":"c","owner":"p","phase":child_phase}}]))
    return model,store

@pytest.mark.parametrize("phase,child_phase,status",[("Open","Active","Valid"),("Gone","Active","Invalid"),("Gone","Terminal","Valid")])
def test_actual_export_and_separate_snapshot_result(phase,child_phase,status,monkeypatch):
    model,store=seeded(phase,child_phase)
    def forbidden(*args,**kwargs): pytest.fail("Snapshot validation invoked Lean")
    monkeypatch.setattr(b._engine,"certify_safety",forbidden)
    at=store.current_history()
    result=b.validate_safety_snapshot(store,model,at=at)
    assert result.status==status
    doc=result.to_dict()
    assert doc["history"]==at.as_dict()
    assert doc["policy_set_hash"]==model.safety_analysis.policy_set_hash
    assert "certificate" not in doc
    assert model.safety_analysis.is_certified is False
    assert result.matches_anchor(model,at)==(status=="Valid")
    with pytest.raises((TypeError,AttributeError)): result.status="Valid"


def test_forged_detached_export_is_not_accepted_as_a_store():
    model,store=seeded()
    detached=store.export_seed_at(model,store.current_history())
    detached["trusted"]=True
    with pytest.raises((TypeError,AttributeError)): b.validate_safety_snapshot(detached,model)
    bad=copy.deepcopy(store.current_history().as_dict()); bad["record"]="sha256:"+"a"*64
    result=b.validate_safety_snapshot(store,model,at=bad)
    assert result.status=="Unestablished"
    assert result.to_dict()["findings"]


def test_expected_complete_policy_set_and_exact_incoming_module():
    model,store=seeded()
    r=b.validate_safety_snapshot(store,model)
    plain=b.BehaviorModule.from_wire_json(model.to_wire_json())
    assert not r.matches_anchor(plain,store.current_history())
    from models.examples.safety_models import ld_shared_parent as shared
    sm,ss=shared.history()
    record=b.validate_safety_snapshot(ss,sm)
    assert len(record.to_dict()["properties"])==2
    assert record.status=="Valid"


def test_host_example_does_not_confuse_state_identity_with_history():
    # Unit control for the example. The matching actual Core command-only event
    # is exercised by safety_deployment.rs, without adding DSL capabilities.
    from models.examples.safety_models.deployment import anchor_pitfalls
    store="sha256:"+"a"*64
    state="sha256:"+"b"*64
    before=b.HistoryRef(store,state,0,store)
    after=b.HistoryRef(store,state,1,"sha256:"+"c"*64)
    assert anchor_pitfalls(before,after)=={"same_state":True,"same_full_history":False,
        "unlocked_comparison_enforces_continuity":False}
