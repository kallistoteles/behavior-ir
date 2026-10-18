"""Restricted immutable parent/child reference; all runtime behavior is Core IR."""
from enum import Enum
from behavior import (BehaviorModule, FieldOwnership, LifecycleDependency, Id, Input, Ref,
    Store, InMemoryBackend, action, entity, field, requires, set_, create, remove, select,
    any_, not_, and_)

class ParentPhase(Enum):
    OPEN="Open"
    GONE="Gone"
    ABANDONED="Abandoned"
class ChildPhase(Enum):
    ACTIVE="Active"
    TERMINAL="Terminal"
@entity
class Parent:
    phase=field(ParentPhase)
@entity
class Child:
    phase=field(ChildPhase)
    owner=field(Ref[Parent])

@action
def stop(p:Parent):
    requires(not_(any_(select(Child),lambda c:and_(c.owner==p.id,not_(c.phase.in_([ChildPhase.TERMINAL]))))))
    set_(p.phase,ParentPhase.GONE)
@action
def reopen(p:Parent):
    set_(p.phase,ParentPhase.OPEN)
@action
def activate(c:Child,p:Parent):
    requires(c.owner==p.id)
    requires(not_(p.phase.in_([ParentPhase.ABANDONED,ParentPhase.GONE])))
    set_(c.phase,ChildPhase.ACTIVE)
@action
def finish(c:Child):
    set_(c.phase,ChildPhase.TERMINAL)
@action
def create_child(p:Parent,*,new_id:Input[Id[Child]]):
    requires(not_(p.phase.in_([ParentPhase.ABANDONED,ParentPhase.GONE])))
    create(Child,id=new_id,owner=p.id,phase=ChildPhase.ACTIVE)
@action
def create_parent(*,new_id:Input[Id[Parent]]):
    create(Parent,id=new_id,phase=ParentPhase.OPEN)
@action
def remove_child(c:Child):
    remove(c)
@action
def remove_parent(p:Parent):
    remove(p)
@action
def extra(c:Child):
    set_(c.owner,c.owner)

ACTIONS=[stop,reopen,activate,finish,create_child,create_parent,remove_child,remove_parent]
def policies():
    return [FieldOwnership(id="parents",entity=Parent,writers={"phase":[stop,reopen]},creators=[create_parent],removers=[remove_parent]),
            FieldOwnership(id="children",entity=Child,writers={"phase":[activate,finish],"owner":[]},creators=[create_child],removers=[remove_child]),
            LifecycleDependency(id="relation",parent=Parent,child=Child,owner_field="owner",parent_state="phase",child_state="phase",
                prohibited=[ParentPhase.ABANDONED,ParentPhase.GONE],terminal=[ChildPhase.TERMINAL],ownership=["parents","children"])]
def build(*,safety=True,extra_writer=False):
    return BehaviorModule(entities=[Parent,Child],enums=[ParentPhase,ChildPhase],actions=ACTIONS+([extra] if extra_writer else []),safety=policies() if safety else [])

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
    step(store,model,"create_child",{"p":"p1"},{"new_id":"c1"})
    step(store,model,"stop",{"p":"p1"},allow=False)
    step(store,model,"finish",{"c":"c1"})
    step(store,model,"activate",{"c":"c1","p":"p1"})
    step(store,model,"finish",{"c":"c1"})
    step(store,model,"stop",{"p":"p1"})
    # Terminal children can refer to an existing Gone parent, never a deleted one.
    step(store,model,"activate",{"c":"c1","p":"p1"},allow=False)
    step(store,model,"create_child",{"p":"p1"},{"new_id":"c2"},allow=False)
    candidate=store.evaluate(model,"remove_parent",bindings={"p":"p1"},commit_time="2026-10-18T00:00:00Z")
    assert candidate.decision.result!="ALLOW" or candidate.bundle is None
    step(store,model,"remove_child",{"c":"c1"})
    step(store,model,"remove_parent",{"p":"p1"})
    return model,store

def exercise():
    model,store=history()
    assert store.current().position==8
    return {"passed":True,"committed":8,"boundary":"Core-only transitions; no initial-state certification"}
