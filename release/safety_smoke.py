#!/usr/bin/env python3
"""Public installed-package Safety smoke; proof checking is explicitly requested."""
from __future__ import annotations
import argparse
import json
import os
import subprocess
from pathlib import Path
import sys
import behavior as b

@b.entity
class SafetyNote:
    text=b.field(str)

@b.action
def edit(note:SafetyNote,*,text:b.Input[str]):
    b.set_(note.text,text)

def verify_in_consumer(bundle: dict, producer: Path, toolchain: str | None) -> dict:
    """Data-only installed subprocess; producer work products are inaccessible."""
    if os.geteuid()==0:
        raise RuntimeError('Unprivileged consumer required for producer workspace denial')
    consumer=producer.parent/(producer.name+'-consumer')
    consumer.mkdir(exist_ok=False)
    supplied=consumer/'bundle.json';supplied.write_text(json.dumps(bundle))
    command=[sys.executable,str(Path(__file__).resolve()),'--verify-bundle',str(supplied.resolve()),
             '--output',str((consumer/'checked').resolve())]
    if toolchain is not None:command+=['--toolchain',toolchain]
    mode=producer.stat().st_mode & 0o7777
    env={key:value for key,value in os.environ.items() if key not in ['PYTHONPATH','PYTHONHOME']}
    producer.chmod(0)
    try:
        result=subprocess.run(command,cwd=consumer,env=env,check=True,text=True,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
    finally:
        producer.chmod(mode)
    return json.loads(result.stdout)


def main() -> int:
    parser=argparse.ArgumentParser(description=__doc__)
    modes=parser.add_mutually_exclusive_group()
    modes.add_argument("--ordinary",action="store_true")
    modes.add_argument("--certify",action="store_true")
    modes.add_argument("--verify-bundle",type=Path)
    parser.add_argument("--toolchain")
    parser.add_argument("--output",type=Path)
    args=parser.parse_args()
    model=b.BehaviorModule(entities=[SafetyNote],actions=[edit],safety=[b.FieldOwnership(
        id="notes",entity=SafetyNote,writers={"text":[edit]})])
    assert model.safety_analysis.status=="Lowered" and not model.safety_analysis.is_certified
    seed=[{"entity":"SafetyNote","value":{"id":"n1","text":"first"}}]
    store=b.Store.create(b.InMemoryBackend(),model,b.Store.genesis_for(model,seed))
    result=store.evaluate(model,"edit",bindings={"note":"n1"},input={"text":"second"},commit_time="2026-10-18T00:00:00Z")
    assert result.decision.result=="ALLOW" and result.bundle is not None
    store.commit(model,result.bundle)
    assert store.load("SafetyNote","n1")["value"]["text"]=="second"
    plain=b.BehaviorModule.from_wire_json(model.to_wire_json())
    assert plain.safety_analysis is None
    assert b.replay_data(store).ok and b.replay_behavior(store,[plain]).ok
    row={"behavior_hash":model.behavior_version,"ordinary_execution":"Passed","core_only_replay":"Passed","certified":False}
    if args.verify_bundle:
        if args.output is None:parser.error('--verify-bundle requires --output')
        bundle=b.load_safety_bundle(args.verify_bundle)
        result=b.verify_safety_evidence(model,bundle,output=args.output,toolchain=args.toolchain)
        assert result.certificate is not None,result.to_dict()
        record=result.to_dict()
        assert record['format']=='behavior.safety_evidence.v2'
        assert record['checker_version']==3 and record['proof']['statement_version']==2
        assert len(record['proof']['audit'])==16
        row['evidence_hash']=record['evidence_hash'];row['certified']=True
    if args.certify:
        if args.output is None:parser.error("--certify requires --output")
        try:
            assessment=b.certify_safety(model,output=args.output,toolchain=args.toolchain)
            assert assessment.certificate is not None,assessment.to_dict()
            bundle=b.load_safety_bundle(args.output/"bundle.json")
            assert not bundle.is_verified
            assert bundle.to_dict()['format']=='behavior.safety_bundle.v2'
            verified=verify_in_consumer(bundle.to_dict(),args.output,args.toolchain)
            assert verified["evidence_hash"]==assessment.to_dict()["evidence_hash"]
            row["certified"]=True
        except Exception as error:
            print(f"safety smoke: certification failed: {error}",file=sys.stderr)
            return 1
    print(json.dumps(row,sort_keys=True,separators=(",",":")))
    print("safety smoke: OK",file=sys.stderr)
    return 0

if __name__=="__main__":
    raise SystemExit(main())
