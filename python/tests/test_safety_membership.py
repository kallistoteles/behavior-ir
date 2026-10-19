"""Membership order is authored Core identity, not a Python normalization rule."""
from enum import Enum
import json
import pytest
import behavior as b

def membership_model(order, *, nested=False, variant="canonical", safety=True):
    class Phase(Enum):
        ACTIVE="Active"
        T1="Terminal1"
        T2="Terminal2"
    @b.entity
    class Parent:
        phase=b.field(Phase)
    @b.entity
    class Child:
        phase=b.field(Phase)
        owner=b.field(b.Ref[Parent])
    values=[getattr(Phase,v) for v in order]
    @b.action
    def stop(p:Parent,q:Parent):
        parent=q if variant=="binding" else p
        guard=b.not_(b.any_(b.select(Child),lambda c:b.and_(c.owner==parent.id,b.not_(c.phase.in_(values)))))
        if variant=="or":guard=b.or_(guard,True)
        if variant!="missing":b.requires(guard)
        b.set_(p.phase,Phase.T1)
    @b.action
    def activate(c:Child,p:Parent,q:Parent):
        b.requires(c.owner==p.id)
        parent=q if variant=="binding" else p
        guard=b.not_(parent.phase.in_(values))
        if variant=="or":guard=b.or_(guard,True)
        if variant!="missing":b.requires(guard)
        b.set_(c.phase,Phase.ACTIVE)
    action=stop if nested else activate
    policies=[b.FieldOwnership(id="parents",entity=Parent,writers={"phase":[action]}),
              b.FieldOwnership(id="children",entity=Child,writers={"owner":[],"phase":[action]}),
              b.LifecycleDependency(id="dependency",parent=Parent,child=Child,owner_field="owner",parent_state="phase",child_state="phase",prohibited=[Phase.T1,Phase.T2],terminal=[Phase.T1,Phase.T2],ownership=["parents","children"])]
    return b.BehaviorModule(entities=[Parent,Child],enums=[Phase],actions=[action],safety=policies if safety else [])

@pytest.mark.parametrize("nested",[False,True])
@pytest.mark.parametrize("order",[("T1","T2"),("T2","T1"),("T2","T1","T2")])
def test_public_order_and_duplicates_certify_exact_authored_module(tmp_path,nested,order):
    model=membership_model(order,nested=nested)
    plain=membership_model(order,nested=nested,safety=False)
    assert model.to_wire_json()==plain.to_wire_json()
    assert model.model_receipts==plain.model_receipts
    assert model.safety_analysis.to_dict()["issues"]==[]
    result=b.certify_safety(model,output=tmp_path)
    assert result.certificate is not None,result.to_dict()
    assert result.to_dict()["behavior_hash"]==model.safety_analysis.to_dict()["inventory"]["behavior_hash"]

@pytest.mark.parametrize("nested",[False,True])
@pytest.mark.parametrize("variant",["binding","or","missing"])
def test_untrusted_order_hints_cannot_certify_wrong_context(tmp_path,nested,variant):
    model=membership_model(["T2","T1"],nested=nested,variant=variant)
    result=b.certify_safety(model,output=tmp_path)
    assert result.certificate is None
    assert result.outcome=="Unsupported"

def test_bounded_discovery_is_inconclusive_not_a_truncated_certificate(tmp_path):
    model=membership_model(["T1","T2"]*2049)
    assert model.safety_analysis.status=="Lowered"
    issues=model.safety_analysis.to_dict()["issues"]
    assert any(i["code"]=="SAFETY_GUARD_SCAN_EXHAUSTED" for i in issues)
    result=b.certify_safety(model,output=tmp_path)
    assert result.outcome=="Inconclusive"
    assert result.certificate is None
    assert result.to_dict()["diagnostics"]==issues
    assert result.to_dict()["action_obligations"][0]["lifecycle"]["result"]=="Inconclusive"

@pytest.mark.parametrize("nested",[False,True])
def test_membership_order_needs_no_safety_runtime_or_replay_dependency(tmp_path,monkeypatch,nested):
    from .test_store_conformance import DictBackend
    model=membership_model(["T2","T1","T2"],nested=nested)
    wire=model.to_wire_json()
    backend=DictBackend()
    seed=[{"entity":"Parent","value":{"id":"p","phase":"Active"}},
          {"entity":"Parent","value":{"id":"q","phase":"Active"}},
          {"entity":"Child","value":{"id":"c","owner":"p","phase":"Terminal1" if nested else "Active"}}]
    monkeypatch.setenv("PATH",str(tmp_path))
    store=b.Store.create(backend,model,b.Store.genesis_for(model,seed))
    bindings={"p":"p","q":"q"}
    if not nested:bindings["c"]="c"
    result=store.evaluate(model,"stop" if nested else "activate",bindings=bindings,commit_time="2026-10-18T00:00:00Z")
    assert result.bundle is not None
    store.commit(model,result.bundle)
    del model
    def forbidden(*args,**kwargs):pytest.fail("Core replay accessed safety or Lean")
    monkeypatch.setattr(b._engine,"analyze_safety",forbidden)
    monkeypatch.setattr(b._engine,"certify_safety",forbidden)
    plain=b.BehaviorModule.from_wire_json(wire)
    restored=b.Store.open(backend)
    assert b.replay_data(restored).ok
    assert b.replay_behavior(restored,[plain]).ok
