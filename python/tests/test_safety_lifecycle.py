"""Lifecycle authoring is a recording layer over the Rust checker."""
import importlib
import pytest
import behavior as b
from enum import Enum

@pytest.mark.parametrize("case", ["ld_parent_child","ld_shared_parent","ld_vm_resource"])
def test_lifecycle_public_cases_are_state_transparent_and_executable(case):
    reference=importlib.import_module("models.examples.safety_models."+case)
    plain,protected=reference.build(safety=False),reference.build()
    assert plain.to_wire_json()==protected.to_wire_json()
    assert plain.model_receipts==protected.model_receipts
    assert protected.safety_analysis.status=="Lowered"
    assert not protected.safety_analysis.is_certified
    assert reference.exercise()["passed"]

def test_dependency_object_records_explicit_control_references():
    @b.entity
    class Parent:
        phase=b.field(str)
    @b.entity
    class Child:
        owner=b.field(b.Ref[Parent])
        phase=b.field(str)
    policy=b.LifecycleDependency(id="children",parent=Parent,child=Child,owner_field="owner",
        parent_state="phase",child_state="phase",prohibited=["Gone"],terminal=["Terminal"],
        ownership=["parent_control","child_control"])
    with pytest.raises(b.ModelBuildError):
        # The Rust authority rejects missing controls/non-enum lifecycle fields.
        b.BehaviorModule(entities=[Parent,Child],safety=[policy])

def test_all_planned_case_identifiers_are_present():
    from models.examples.safety_models.run import CASES
    assert set(CASES)=={"fo_field","fo_state_machine","ld_parent_child","ld_shared_parent","ld_vm_resource"}


def test_optional_pool_owned_domain_remains_valid_without_this_restricted_certificate():
    class Phase(Enum):
        OPEN="Open"
        GONE="Gone"
    @b.entity
    class VM:
        phase=b.field(Phase)
    @b.entity
    class Pool:
        name=b.field(str)
    @b.entity
    class Resource:
        vm=b.field(b.Option[b.Ref[VM]])
        pool=b.field(b.Option[b.Ref[Pool]])
        phase=b.field(Phase)
    # Safety's restricted reference fixture must not reshape an application's schema.
    plain=b.BehaviorModule(entities=[VM,Pool,Resource],enums=[Phase])
    seed=[{"entity":"Pool","value":{"id":"pool1","name":"pool"}},
          {"entity":"Resource","value":{"id":"r1","vm":None,"pool":"pool1","phase":"Open"}}]
    store=b.Store.create(b.InMemoryBackend(),plain,b.Store.genesis_for(plain,seed))
    assert store.load("Resource","r1")["value"]["pool"]=="pool1"
    controls=[b.FieldOwnership(id="vms",entity=VM,writers={"phase":[]}),
              b.FieldOwnership(id="resources",entity=Resource,writers={"phase":[],"vm":[]})]
    dep=b.LifecycleDependency(id="vms_resources",parent=VM,child=Resource,owner_field="vm",
        parent_state="phase",child_state="phase",prohibited=[Phase.GONE],terminal=[Phase.GONE],
        ownership=["vms","resources"])
    with pytest.raises(b.ModelBuildError):
        b.BehaviorModule(entities=[VM,Pool,Resource],enums=[Phase],safety=[*controls,dep])


def test_nonliteral_lifecycle_assignment_stays_explicitly_uncertified():
    class Phase(Enum):
        OPEN="Open"
        GONE="Gone"
    @b.entity
    class Parent:
        phase=b.field(Phase)
    @b.entity
    class Child:
        owner=b.field(b.Ref[Parent])
        phase=b.field(Phase)
    @b.action
    def change(c:Child):
        b.set_(c.phase,c.phase)
    policies=[b.FieldOwnership(id="parents",entity=Parent,writers={"phase":[]}),
              b.FieldOwnership(id="children",entity=Child,writers={"owner":[],"phase":[change]}),
              b.LifecycleDependency(id="relation",parent=Parent,child=Child,owner_field="owner",
                  parent_state="phase",child_state="phase",prohibited=[Phase.GONE],terminal=[Phase.GONE],
                  ownership=["parents","children"])]
    model=b.BehaviorModule(entities=[Parent,Child],enums=[Phase],actions=[change],safety=policies)
    assert model.safety_analysis.status=="Lowered"
    assert not model.safety_analysis.is_certified
    assert model.safety_analysis.to_dict()["issues"]
