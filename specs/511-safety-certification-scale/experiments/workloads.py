"""Neutral public-API construction only; Rust owns Safety meaning and coverage checks."""
from enum import Enum
import json

import behavior as b
from certification_probe import make_fo


def _integer(request,key,default,minimum=1):
    value=request.get(key,default)
    if type(value) is not int or value < minimum:
        raise ValueError(f'{key} must be an integer >= {minimum}')
    return value


def _action(name,annotations,fn):
    fn.__name__=name
    fn.__annotations__=annotations
    return b.action(fn)


def _lifecycle(request):
    count=_integer(request,'dependencies',1)
    width=_integer(request,'guard_width',2)
    fo_actions=_integer(request,'fo_actions',0,0)
    generated=request.get('generated',False)
    if type(generated) is not bool:
        raise ValueError('generated must be boolean')
    ParentPhase=Enum('ParentPhase',{'OPEN':'Open',**{f'BLOCKED_{i}':f'Blocked{i}' for i in range(width)}})
    ChildPhase=Enum('ChildPhase',{'ACTIVE':'Active',**{f'TERMINAL_{i}':f'Terminal{i}' for i in range(width)}})
    Parent=b.entity(type('Parent',(),{'phase':b.field(ParentPhase)}))
    children=[b.entity(type(f'Child_{i}',(),{'phase':b.field(ChildPhase),'owner':b.field(b.Ref[Parent])})) for i in range(count)]
    prohibited=[getattr(ParentPhase,f'BLOCKED_{i}') for i in range(width)]
    terminal=[getattr(ChildPhase,f'TERMINAL_{i}') for i in range(width)]

    def stop(parent):
        for Child in children:
            b.requires(b.not_(b.any_(b.select(Child),lambda child:b.and_(child.owner==parent.id,b.not_(child.phase.in_(terminal))))))
        b.set_(parent.phase,prohibited[0])
    stop=_action('stop',{'parent':Parent},stop)
    def reopen(parent):b.set_(parent.phase,ParentPhase.OPEN)
    reopen=_action('reopen',{'parent':Parent},reopen)
    def create_parent(*,new_id):b.create(Parent,id=new_id,phase=ParentPhase.OPEN)
    create_parent=_action('create_parent',{'new_id':b.Input[b.Id[Parent]]},create_parent)
    def remove_parent(parent):b.remove(parent)
    remove_parent=_action('remove_parent',{'parent':Parent},remove_parent)
    actions=[stop,reopen,create_parent,remove_parent]
    expected={name:1 for name in ['stop','reopen','create_parent','remove_parent']}
    controls=[b.FieldOwnership(id='parent_control',entity=Parent,writers={'phase':[stop,reopen]},creators=[create_parent],removers=[remove_parent])]
    dependencies=[]
    def child_actions(Child,index):
        def activate(child,parent):
            b.requires(child.owner==parent.id)
            b.requires(b.not_(parent.phase.in_(prohibited)))
            b.set_(child.phase,ChildPhase.ACTIVE)
        activate=_action(f'activate_{index}',{'child':Child,'parent':Parent},activate)
        def finish(child):b.set_(child.phase,terminal[0])
        finish=_action(f'finish_{index}',{'child':Child},finish)
        def create_child(parent,*,new_id):
            b.requires(b.not_(parent.phase.in_(prohibited)))
            b.create(Child,id=new_id,owner=parent.id,phase=ChildPhase.ACTIVE)
        create_child=_action(f'create_child_{index}',{'parent':Parent,'new_id':b.Input[b.Id[Child]]},create_child)
        def remove_child(child):b.remove(child)
        remove_child=_action(f'remove_child_{index}',{'child':Child},remove_child)
        return activate,finish,create_child,remove_child
    for index,Child in enumerate(children):
        activate,finish,create_child,remove_child=child_actions(Child,index)
        actions.extend([activate,finish,create_child,remove_child])
        expected.update({f'{kind}_{index}':1 for kind in ['activate','finish','create_child','remove_child']})
        control=f'child_control_{index}'
        controls.append(b.FieldOwnership(id=control,entity=Child,writers={'owner':[],'phase':[activate,finish]},creators=[create_child],removers=[remove_child]))
        dependencies.append(b.LifecycleDependency(id=f'dependency_{index}',parent=Parent,child=Child,
            owner_field='owner',parent_state='phase',child_state='phase',prohibited=prohibited,
            terminal=terminal,ownership=['parent_control',control]))
    entities=[Parent,*children];enums=[ParentPhase,ChildPhase];models=[]
    if generated:
        TaskPhase=Enum('TaskPhase',{'OPEN':'Open','DONE':'Done'})
        Task=b.entity(type('Task',(),{'phase':b.field(TaskPhase)}))
        machine=b.StateMachine('work',subject=Task,state_field='phase',entry_states={TaskPhase.OPEN})
        machine.transition('finish',from_={TaskPhase.OPEN},to=TaskPhase.DONE)
        models.append(machine);entities.append(Task);enums.append(TaskPhase)
        controls.append(b.FieldOwnership(id='task_control',entity=Task,writers={'phase':[b.GeneratedAction('work','finish')]}))
        expected['work__finish']=1
    if fo_actions:
        Auxiliary=b.entity(type('Auxiliary',(),{'value':b.field(int)}))
        entities.append(Auxiliary);edits=[]
        def edit_action(index):
            def edit(item):b.set_(item.value,index)
            return _action(f'edit_{index}',{'item':Auxiliary},edit)
        for index in range(fo_actions):
            edits.append(edit_action(index));expected[f'edit_{index}']=1
        actions.extend(edits)
        controls.append(b.FieldOwnership(id='auxiliary_control',entity=Auxiliary,writers={'value':edits}))
    return b.BehaviorModule(entities=entities,enums=enums,actions=actions,models=models,safety=[*controls,*dependencies]),expected


def build_case(request):
    if not isinstance(request,dict):raise ValueError('Case must be an object')
    kind=request.get('kind')
    allowed={'fo':{'kind','actions','operations','policies','padding'},
             'lifecycle':{'kind','dependencies','guard_width','generated','fo_actions'}}
    if kind not in allowed or set(request)-allowed[kind]:
        raise ValueError('Unknown or executable case configuration')
    if kind=='fo':
        return make_fo(_integer(request,'actions',1),_integer(request,'operations',1),
                       _integer(request,'policies',1),_integer(request,'padding',0,0))
    return _lifecycle(request)


def describe_case(model,expected):
    analysis=model.safety_analysis.to_dict()
    actions=analysis['inventory']['actions']
    actual={action['name']:len(action['operations']) for action in actions}
    if actual!=expected:
        raise ValueError(f'Independent expected inventory differs: {expected!r} != {actual!r}')
    if len({action['hash'] for action in actions}) != len(actions):
        raise ValueError('Duplicate full action hashes in inventory')
    policies=analysis['policies']
    ld=[item['document'] for item in policies if item['document']['kind']=='LifecycleDependency']
    # These are measurements, not a Python reimplementation of Safety semantics.
    return {'actions':len(expected),'operations':sum(expected.values()),
        'ownership_policies':sum(item['document']['kind']=='FieldOwnership' for item in policies),
        'lifecycle_dependencies':len(ld),
        'required_precondition_roots':sum(len(item['preconditions']) for item in actions),
        'guard_literal_width':max((len(item['terminal']) for item in ld),default=0),
        'encoded_inventory_bytes':len(json.dumps(analysis['inventory'],sort_keys=True,separators=(',',':')).encode()),
        'behavior_hash':model.behavior_version,'policy_set_hash':analysis['policy_set_hash'],
        'analysis_issues':analysis['issues'],'expected_action_operations':expected}
