"""Two independently checked child types share one parent lifecycle."""
from .ld_parent_child import (Parent, Child, ParentPhase, ChildPhase, ACTIONS,
    create_parent, remove_parent, activate, finish, create_child, remove_child, reopen, extra, step)
from behavior import (BehaviorModule, FieldOwnership, LifecycleDependency, Ref, Id, Input,
    Store, InMemoryBackend, entity, field, action, requires, not_, any_, and_, select, set_, create, remove)

@entity
class OtherChild:
    phase=field(ChildPhase)
    owner=field(Ref[Parent])
@action
def stop(p:Parent):
    requires(not_(any_(select(Child),lambda c:and_(c.owner==p.id,not_(c.phase.in_([ChildPhase.TERMINAL]))))))
    requires(not_(any_(select(OtherChild),lambda c:and_(c.owner==p.id,not_(c.phase.in_([ChildPhase.TERMINAL]))))))
    set_(p.phase,ParentPhase.GONE)
@action
def other_activate(c:OtherChild,p:Parent):
    requires(c.owner==p.id)
    requires(not_(p.phase.in_([ParentPhase.ABANDONED,ParentPhase.GONE])))
    set_(c.phase,ChildPhase.ACTIVE)
@action
def other_finish(c:OtherChild):
    set_(c.phase,ChildPhase.TERMINAL)
@action
def other_create_child(p:Parent,*,new_id:Input[Id[OtherChild]]):
    requires(not_(p.phase.in_([ParentPhase.ABANDONED,ParentPhase.GONE])))
    create(OtherChild,id=new_id,owner=p.id,phase=ChildPhase.ACTIVE)
@action
def other_remove_child(c:OtherChild):
    remove(c)

def build(*,safety=True,extra_writer=False):
    controls=[FieldOwnership(id="parents",entity=Parent,writers={"phase":[stop,reopen]},creators=[create_parent],removers=[remove_parent]),
        FieldOwnership(id="children",entity=Child,writers={"phase":[activate,finish],"owner":[]},creators=[create_child],removers=[remove_child]),
        FieldOwnership(id="other_children",entity=OtherChild,writers={"phase":[other_activate,other_finish],"owner":[]},creators=[other_create_child],removers=[other_remove_child])]
    deps=[LifecycleDependency(id=id,parent=Parent,child=child,owner_field="owner",parent_state="phase",child_state="phase",
        prohibited=[ParentPhase.ABANDONED,ParentPhase.GONE],terminal=[ChildPhase.TERMINAL],ownership=["parents",control])
        for id,child,control in [("relation",Child,"children"),("other_relation",OtherChild,"other_children")]]
    actions=[stop,*ACTIONS[1:],other_activate,other_finish,other_create_child,other_remove_child]
    return BehaviorModule(entities=[Parent,Child,OtherChild],enums=[ParentPhase,ChildPhase],actions=actions+([extra] if extra_writer else []),safety=[*controls,*deps] if safety else [])
def history():
    model=build()
    store=Store.create(InMemoryBackend(),model,Store.genesis_for(model,[]))
    step(store,model,"create_parent",inputs={"new_id":"p1"})
    step(store,model,"create_child",{"p":"p1"},{"new_id":"c1"})
    step(store,model,"other_create_child",{"p":"p1"},{"new_id":"c2"})
    step(store,model,"stop",{"p":"p1"},allow=False)
    step(store,model,"finish",{"c":"c1"})
    step(store,model,"stop",{"p":"p1"},allow=False)
    step(store,model,"other_finish",{"c":"c2"})
    step(store,model,"stop",{"p":"p1"})
    assert store.load("Parent","p1")["value"]["phase"]=="Gone"
    assert store.load("Child","c1")["value"]["phase"]=="Terminal"
    assert store.load("OtherChild","c2")["value"]["phase"]=="Terminal"
    return model,store
def exercise():
    _,store=history()
    assert store.current().position==6
    return {"passed":True,"committed":6,"dependencies":2,"boundary":"Core-only shared parent"}
