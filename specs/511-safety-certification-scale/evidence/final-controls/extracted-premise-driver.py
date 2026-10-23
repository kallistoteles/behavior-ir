#!/usr/bin/env python3
"""Trusted diagnostic mutations of frozen proof data; never certificate authority."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import resource
import subprocess
import time

DEFINITIONS = {
 'missingGuard': '{ Concrete.data with lifecycle := { Concrete.data.lifecycle with actions := Concrete.data.lifecycle.actions.map (fun a => { a with roots := [] }) } }',
 'wrongStateRole': '{ Concrete.data with lifecycle := { Concrete.data.lifecycle with actions := Concrete.data.lifecycle.actions.map (fun a => { a with parameters := a.parameters.map (fun entry => (entry.1, false)) }) } }',
 'wrongParentProbe': '{ Concrete.data with lifecycle := { Concrete.data.lifecycle with actions := Concrete.data.lifecycle.actions.map (fun a => { a with probes := a.probes.map (fun p => { p with parent := a.parameters.length + 1 }) }) } }',
 'missingLifecycleInventory': '{ Concrete.data with lifecycle := { Concrete.data.lifecycle with actions := [] } }',
 'missingControlPolicy': '{ Concrete.data with policies := Concrete.data.policies.drop 1 }',
 'deniedPermissions': '{ Concrete.data with policies := Concrete.data.policies.map (fun p => { p with writers := p.writers.map (fun entry => (entry.1, [])), creators := [], removers := [] }) }',
 'duplicateFullKeys': '{ Concrete.data with actionHashes := Concrete.data.actionHashes.map (fun _ => Concrete.data.actionHashes.head!) }',
}

def digest(path):
 return hashlib.sha256(path.read_bytes()).hexdigest()

def main():
 parser=argparse.ArgumentParser(description=__doc__)
 parser.add_argument('--positive-workspace',type=Path,required=True)
 parser.add_argument('--bundle',type=Path,required=True)
 parser.add_argument('--build-manifest',type=Path,required=True)
 parser.add_argument('--output',type=Path,required=True)
 args=parser.parse_args()
 build=json.loads(args.build_manifest.read_text())
 bundle=json.loads(args.bundle.read_text())
 expected=build['expected_requests']['D1']['expected']
 record=bundle['evidence']
 assert record['behavior_hash']==expected['behavior_hash']
 assert record['policy_set_hash']==expected['policy_set_hash']
 assert record['outcome']=='Certified under assumptions'
 assert record['proof']['sources']==build['proof_sources']
 assert record['proof']['implementation_sources']==build['implementation_identity']
 assert len(record['proof']['audit'])==16
 root=Path(build['proof_toolchain']['profile']['outPath'])
 work=args.output.resolve();work.mkdir(parents=True,exist_ok=False)
 positive=args.positive_workspace.resolve()
 (work/'safety').symlink_to(positive/'safety',target_is_directory=True)
 for name in ['Concrete.lean','Concrete.olean']:(work/name).symlink_to(positive/name)
 source='prelude\nimport Concrete\nnamespace PremiseControls\ntheorem positive : Safety.checkModule Concrete.data = true := Concrete.checked\n'
 for name,definition in DEFINITIONS.items():
  source+=f'noncomputable def {name} : Safety.ModuleData := {definition}\n'
  source+=f'theorem {name}_rejected : Safety.checkModule {name} = false := by decide\n'
 source+='end PremiseControls\n'
 (work/'PremiseControls.lean').write_text(source)
 audit='prelude\nimport safety.Audit\nimport PremiseControls\n#safety_audit PremiseControls.positive : Safety.checkModule Concrete.data = true\n'
 for name in DEFINITIONS:
  audit+=f'#safety_audit PremiseControls.{name}_rejected : Safety.checkModule PremiseControls.{name} = false\n'
 (work/'PremiseAudit.lean').write_text(audit)
 common=['-DmaxHeartbeats=200000','-M4096','-j1','-DmaxRecDepth=4096']
 observations=[]
 def limits():resource.setrlimit(resource.RLIMIT_AS,(8*1024**3,8*1024**3))
 def run(name,command):
  start=time.monotonic()
  result=subprocess.run(command,cwd=work,env={'PATH':str(root/'bin'),'LEAN_PATH':str(work)},
       capture_output=True,text=True,timeout=180,preexec_fn=limits)
  log=result.stdout+result.stderr;(work/(name+'.log')).write_text(log)
  observations.append({'stage':name,'command':command,'exit_code':result.returncode,
       'wall_seconds':time.monotonic()-start,'log_sha256':digest(work/(name+'.log'))})
  return result.returncode,log
 document={'authority':'Diagnostic proof-premise controls only; no Safety Certificate issued',
  'candidate_manifest_sha256':digest(args.build_manifest),'expected_behavior_hash':expected['behavior_hash'],
  'expected_policy_set_hash':expected['policy_set_hash'],'positive_evidence_hash':record['evidence_hash'],
  'base_concrete_sha256':digest(positive/'Concrete.lean'),'driver_sha256':digest(Path(__file__)),
  'scope':'Perturbed finite premises from actual frozen D1 data. This does not claim the original certified Core module is unsafe; Core extraction and expected-policy coverage remain separate tested/trusted boundaries.',
  'stages':observations,'results':[],'passed':False}
 def persist():
  document['stages']=observations
  (work/'controls.json').write_text(json.dumps(document,sort_keys=True,indent=2)+'\n')
 try:
  rc,_=run('positive-and-refutations',[str(root/'bin/lean'),*common,'-o','PremiseControls.olean','PremiseControls.lean'])
  if rc:raise ValueError('Invalid control preparation: expected positive and false-premise proofs did not compile')
  rc,_=run('axiom-audit',[str(root/'bin/lean'),*common,'PremiseAudit.lean'])
  if rc:raise ValueError('Invalid control preparation: intended-type/axiom audit failed')
  rows=[json.loads(line) for line in (work/'audit.jsonl').read_text().splitlines()]
  assert len(rows)==1+len(DEFINITIONS)
  assert all(row['expected_type_checked'] is True and set(row['axioms'])<= {'propext','Classical.choice','Quot.sound'} for row in rows)
  document['audit']=rows
  rc,_=run('fresh',[str(root/'bin/leanchecker'),'--fresh','PremiseControls'])
  if rc:raise ValueError('Fresh kernel replay of valid diagnostic refutations failed')
  for name in DEFINITIONS:
   path=work/('Attempt_'+name+'.lean')
   path.write_text('prelude\nimport PremiseControls\n'+f'theorem attempted : Safety.checkModule PremiseControls.{name} = true := by decide\n')
   rc,log=run(name,[str(root/'bin/lean'),*common,path.name])
   if rc==0 or "tactic `decide` proved that" not in log.lower() or 'false' not in log:
    raise ValueError('Invalid or accepted negative control: '+name)
   if any(term in log for term in ['maximum recursion','maximum number of heartbeats','unknown identifier','object file']):
    raise ValueError('Control failed for an unintended resource/import reason: '+name)
   document['results'].append({'case':name,'rejected':True,'source_sha256':digest(path),'certificate_present':False})
  document['passed']=True
 finally:persist()
 print(json.dumps({'passed':True,'controls':len(document['results']),'authority':document['authority']}))

if __name__=='__main__':main()
