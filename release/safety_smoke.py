#!/usr/bin/env python3
"""Public installed-package Safety smoke; proof checking is explicitly requested."""
from __future__ import annotations
import argparse
import json
from pathlib import Path
import sys
import behavior as b

@b.entity
class SafetyNote:
    text=b.field(str)

@b.action
def edit(note:SafetyNote,*,text:b.Input[str]):
    b.set_(note.text,text)

def main() -> int:
    parser=argparse.ArgumentParser(description=__doc__)
    modes=parser.add_mutually_exclusive_group()
    modes.add_argument("--ordinary",action="store_true")
    modes.add_argument("--certify",action="store_true")
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
    if args.certify:
        if args.output is None:parser.error("--certify requires --output")
        try:
            assessment=b.certify_safety(model,output=args.output,toolchain=args.toolchain)
            assert assessment.certificate is not None,assessment.to_dict()
            bundle=b.load_safety_bundle(args.output/"bundle.json")
            assert not bundle.is_verified
            verified=b.verify_safety_evidence(model,bundle,output=args.output/"verified",toolchain=args.toolchain)
            assert verified.certificate is not None,verified.to_dict()
            assert verified.to_dict()["evidence_hash"]==assessment.to_dict()["evidence_hash"]
            row["certified"]=True
        except Exception as error:
            print(f"safety smoke: certification failed: {error}",file=sys.stderr)
            return 1
    print(json.dumps(row,sort_keys=True,separators=(",",":")))
    print("safety smoke: OK",file=sys.stderr)
    return 0

if __name__=="__main__":
    raise SystemExit(main())
