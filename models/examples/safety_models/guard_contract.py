"""Restricted public guard examples; Core and Safety establish different claims."""
from enum import Enum
import behavior as b

VARIANTS=("canonical","permuted","equivalent_unsupported","missing","wrong_parent","insufficient_or")
LOCAL_PROPERTY="After stop, every child of p is terminal (a local query-sensitive postcondition)."

class ParentPhase(Enum):
    OPEN="Open"
    REMOVED="Removed"
    ABANDONED="Abandoned"
class ChildPhase(Enum):
    PLANNED="Planned"
    CREATED="Created"
    ACTIVE="Active"
    TERMINAL="Terminal"
    DONE="Done"
@b.entity
class Parent:
    phase=b.field(ParentPhase)
@b.entity
class Child:
    phase=b.field(ChildPhase)
    owner=b.field(b.Ref[Parent])

TERMINAL=[ChildPhase.DONE,ChildPhase.TERMINAL]
PROHIBITED=[ParentPhase.ABANDONED,ParentPhase.REMOVED]

def no_nonterminal(parent,terminals):
    return b.not_(b.any_(b.select(Child),lambda c:b.and_(c.owner==parent.id,b.not_(c.phase.in_(terminals)))))

def policies(*,parent_writers=(),child_writers=()):
    return [b.FieldOwnership(id="parents",entity=Parent,writers={"phase":parent_writers}),
            b.FieldOwnership(id="children",entity=Child,writers={"phase":child_writers,"owner":[]}),
            b.LifecycleDependency(id="dependency",parent=Parent,child=Child,owner_field="owner",parent_state="phase",child_state="phase",prohibited=PROHIBITED,terminal=TERMINAL,ownership=["parents","children"])]

def build(variant="canonical",*,safety=True):
    if variant not in VARIANTS:raise ValueError(f"Unknown guard variant: {variant}")
    @b.action
    def stop(p:Parent,q:Parent):
        if variant=="equivalent_unsupported":
            guard=b.all_(b.select(Child),lambda c:b.or_(c.owner!=p.id,c.phase.in_(TERMINAL)))
        else:
            guard=no_nonterminal(q if variant=="wrong_parent" else p,list(reversed(TERMINAL)) if variant=="permuted" else TERMINAL)
        if variant=="insufficient_or":guard=b.or_(guard,True)
        if variant!="missing":b.requires(guard)
        b.set_(p.phase,ParentPhase.REMOVED)
        b.ensures(no_nonterminal(p,TERMINAL))
    return b.BehaviorModule(entities=[Parent,Child],enums=[ParentPhase,ChildPhase],actions=[stop],safety=policies(parent_writers=[stop]) if safety else [])

def child_transition_examples():
    rows={}
    for source,target,label in [(ChildPhase.PLANNED,ChildPhase.CREATED,"Planned → Created"),(ChildPhase.TERMINAL,ChildPhase.ACTIVE,"Terminal → Active")]:
        row={}
        for guarded in [False,True]:
            @b.action
            def transition(c:Child,p:Parent):
                b.requires(c.phase==source)
                b.requires(c.owner==p.id)
                if guarded:b.requires(b.not_(p.phase.in_(PROHIBITED)))
                b.set_(c.phase,target)
            model=b.BehaviorModule(entities=[Parent,Child],enums=[ParentPhase,ChildPhase],actions=[transition],safety=policies(child_writers=[transition]))
            analysis=model.safety_analysis.to_dict()
            assert all(op.get("field")!="owner" for a in analysis["inventory"]["actions"] for op in a["operations"])
            row["guarded" if guarded else "unguarded"]=analysis
        row["owner_unchanged"]=True
        rows[label]=row
    return rows
