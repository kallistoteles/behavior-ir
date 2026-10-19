"""Restricted VM/Resource fixture, not a prescribed application data model.

Every fixture Resource has an immutable required Ref[VM]. Nonterminal Resources
require a VM neither Removed nor Abandoned. Terminal Resources still require an
existing VM. I-5b/I-6 remain unmapped; live and nonterminal are not equated.
Pool-owned resources are valid application requirements outside this pattern.
"""
from enum import Enum
from behavior import (BehaviorModule, FieldOwnership, LifecycleDependency, Id, Input, Ref,
    Store, InMemoryBackend, action, entity, field, requires, set_, create, remove, select,
    any_, not_, and_)

class VMPhase(Enum):
    OPEN="Open"
    REMOVED="Removed"
    ABANDONED="Abandoned"
class ResourcePhase(Enum):
    ACTIVE="Active"
    TERMINAL="Terminal"
@entity
class VM:
    phase=field(VMPhase)
@entity
class Resource:
    phase=field(ResourcePhase)
    owner=field(Ref[VM])

@action
def stop(vm:VM):
    requires(not_(any_(select(Resource),lambda resource:and_(resource.owner==vm.id,not_(resource.phase.in_([ResourcePhase.TERMINAL]))))))
    set_(vm.phase,VMPhase.REMOVED)
@action
def reopen(vm:VM):
    set_(vm.phase,VMPhase.OPEN)
@action
def activate(resource:Resource,vm:VM):
    requires(resource.owner==vm.id)
    requires(not_(vm.phase.in_([VMPhase.ABANDONED,VMPhase.REMOVED])))
    set_(resource.phase,ResourcePhase.ACTIVE)
@action
def finish(resource:Resource):
    set_(resource.phase,ResourcePhase.TERMINAL)
@action
def create_child(vm:VM,*,new_id:Input[Id[Resource]]):
    requires(not_(vm.phase.in_([VMPhase.ABANDONED,VMPhase.REMOVED])))
    create(Resource,id=new_id,owner=vm.id,phase=ResourcePhase.ACTIVE)
@action
def create_parent(*,new_id:Input[Id[VM]]):
    create(VM,id=new_id,phase=VMPhase.OPEN)
@action
def remove_child(resource:Resource):
    remove(resource)
@action
def remove_parent(vm:VM):
    remove(vm)
@action
def extra(resource:Resource):
    set_(resource.owner,resource.owner)

ACTIONS=[stop,reopen,activate,finish,create_child,create_parent,remove_child,remove_parent]
def policies():
    return [FieldOwnership(id="parents",entity=VM,writers={"phase":[stop,reopen]},creators=[create_parent],removers=[remove_parent]),
            FieldOwnership(id="children",entity=Resource,writers={"phase":[activate,finish],"owner":[]},creators=[create_child],removers=[remove_child]),
            LifecycleDependency(id="relation",parent=VM,child=Resource,owner_field="owner",parent_state="phase",child_state="phase",
                prohibited=[VMPhase.ABANDONED,VMPhase.REMOVED],terminal=[ResourcePhase.TERMINAL],ownership=["parents","children"])]
def build(*,safety=True,extra_writer=False):
    return BehaviorModule(entities=[Resource,VM],enums=[VMPhase,ResourcePhase],actions=ACTIONS+([extra] if extra_writer else []),safety=policies() if safety else [])

def step(store,model,name,bindings=None,inputs=None,allow=True):
    result=store.evaluate(model,name,bindings=bindings or {},input=inputs or {},commit_time="2026-10-18T00:00:00Z")
    assert (result.decision.result=="ALLOW")==allow, (name,result.decision)
    if allow:
        assert result.bundle is not None
        store.commit(model,result.bundle)
    return result

def history():
    model=build()
    store=Store.create(InMemoryBackend(),model,Store.genesis_for(model,[]))
    step(store,model,"create_parent",inputs={"new_id":"p1"})
    step(store,model,"create_child",{"vm":"p1"},{"new_id":"c1"})
    step(store,model,"stop",{"vm":"p1"},allow=False)
    step(store,model,"finish",{"resource":"c1"})
    step(store,model,"activate",{"resource":"c1","vm":"p1"})
    step(store,model,"finish",{"resource":"c1"})
    step(store,model,"stop",{"vm":"p1"})
    # Terminal children can refer to an existing Removed parent, never a deleted one.
    step(store,model,"activate",{"resource":"c1","vm":"p1"},allow=False)
    step(store,model,"create_child",{"vm":"p1"},{"new_id":"c2"},allow=False)
    candidate=store.evaluate(model,"remove_parent",bindings={"vm":"p1"},commit_time="2026-10-18T00:00:00Z")
    assert candidate.decision.result!="ALLOW" or candidate.bundle is None
    step(store,model,"remove_child",{"resource":"c1"})
    step(store,model,"remove_parent",{"vm":"p1"})
    return model,store

def exercise():
    model,store=history()
    assert store.current().position==8
    return {"passed":True,"committed":8,"boundary":"Core-only transitions; no initial-state certification"}
