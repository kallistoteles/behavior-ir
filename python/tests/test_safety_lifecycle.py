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


def diagnostic_model(mode):
    class Phase(Enum):
        OPEN="Open"
        GONE="Gone"
        TERMINAL="Terminal"
    @b.entity
    class Parent:
        phase=b.field(Phase)
    @b.entity
    class Child:
        phase=b.field(Phase)
        owner=b.field(b.Ref[Parent])
    @b.action
    def child_only(c:Child):
        b.set_(c.phase,Phase.TERMINAL if mode=="terminal_assign" else Phase.OPEN)
    @b.action
    def change(c:Child,p:Parent,q:Parent):
        if mode in {"guard","cross","good","later"}:
            b.requires(c.owner==(q.id if mode=="later" else p.id))
        if mode in {"association","cross","good","later"}:
            parent=q if mode in {"cross","later"} else p
            b.requires(b.not_(parent.phase.in_([Phase.GONE])))
        b.set_(c.phase,Phase.OPEN)
    @b.action
    def create_terminal(p:Parent,*,new_id:b.Input[b.Id[Child]],owner_id:b.Input[b.Id[Parent]]):
        b.create(Child,id=new_id,owner=owner_id if mode=="terminal_create_no_assoc" else p.id,phase=Phase.TERMINAL)
    selected=(create_terminal if mode.startswith("terminal_create") else child_only if mode in {"binding","terminal_assign"} else change)
    creating=mode.startswith("terminal_create")
    policies=[b.FieldOwnership(id="parents",entity=Parent,writers={"phase":[]}),
        b.FieldOwnership(id="children",entity=Child,writers={"owner":[],"phase":[] if creating else [selected]},creators=[selected] if creating else []),
        b.LifecycleDependency(id="dependency",parent=Parent,child=Child,owner_field="owner",parent_state="phase",child_state="phase",prohibited=[Phase.GONE],terminal=[Phase.TERMINAL],ownership=["parents","children"])]
    return b.BehaviorModule(entities=[Parent,Child],enums=[Phase],actions=[selected],safety=policies),selected.name

@pytest.mark.parametrize("mode,code,entity,field",[
    ("binding","SAFETY_PARENT_BINDING_REQUIRED","Parent","phase"),
    ("association","SAFETY_OWNER_ASSOCIATION_REQUIRED","Child","owner"),
    ("combined","SAFETY_OWNER_ASSOCIATION_REQUIRED","Child","owner"),
    ("guard","SAFETY_PARENT_GUARD_REQUIRED","Parent","phase"),
    ("cross","SAFETY_PARENT_GUARD_REQUIRED","Parent","phase"),
    ("terminal_create_no_assoc","SAFETY_OWNER_ASSOCIATION_REQUIRED","Child","owner"),
])
def test_parent_prerequisite_codes_are_structured_and_repeatable(mode,code,entity,field):
    model,name=diagnostic_model(mode)
    analysis=model.safety_analysis.to_dict()
    assert len(analysis["issues"])==1
    issue=analysis["issues"][0]
    assert issue["code"]==code
    assert issue["action"]==name
    assert issue["entity"]==entity and issue["field"]==field
    assert issue["dependency"]==analysis["dependencies"][0]["model_hash"]
    assert diagnostic_model(mode)[0].safety_analysis.to_dict()["issues"]==analysis["issues"]

@pytest.mark.parametrize("mode",["later","good","terminal_assign","terminal_create"])
def test_corrected_parent_prerequisites_and_terminal_exemptions(mode):
    assert diagnostic_model(mode)[0].safety_analysis.to_dict()["issues"]==[]


def test_repairing_one_prerequisite_reveals_only_the_next_established_cause():
    for mode,expected in [("binding",["SAFETY_PARENT_BINDING_REQUIRED"]),("combined",["SAFETY_OWNER_ASSOCIATION_REQUIRED"]),("guard",["SAFETY_PARENT_GUARD_REQUIRED"]),("good",[])]:
        assert [i["code"] for i in diagnostic_model(mode)[0].safety_analysis.to_dict()["issues"]]==expected
