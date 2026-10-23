"""Isolated small-instance adversarial controls for the diagnostic candidate.

Imports only freshly compiled local Concrete/CompleteProbe declarations. Shared
imports are the named candidate's immutable generic helpers, never its instance.
This utility grants no certificate and does not load user-supplied code.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import resource
import subprocess
import time
from candidate_encodings import compact_source


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--baseline', type=Path, required=True)
    parser.add_argument('--generic', type=Path, required=True)
    parser.add_argument('--toolchain', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    original = args.baseline.read_text()
    transformed, metadata = compact_source(original, raw_literals=True, proof_only=True)
    transformed = 'prelude\n'+transformed[:transformed.index('theorem checked :')]+'end Concrete\n'
    claim = re.search(r'theorem certified : (.*?) :=', original).group(1)
    merge = 'prelude\nimport Concrete\nimport MergeWitness\nopen Safety.Candidate\nnamespace MergeProbe\ndef proposed : MergeTrace := .leaf\ntheorem originalUnique : Concrete.data.actionHashes.Pairwise (· ≠ ·) := traceChecked_sound _ proposed (by decide)\nend MergeProbe\n'
    complete = 'prelude\nimport ModelBridge\nimport MergeProbe\nopen MergeProbe\nnamespace CompleteProbe\n'
    complete += '\n'.join('theorem '+name+' : '+prop+' = true := by decide' for name,prop in [
        ('bounds','Safety.Candidate.bounds Concrete.data'), ('authority','Safety.checkAuthority Concrete.data'),
        ('link','Safety.checkLifecycleLink Concrete.data'), ('lifecycle','Safety.checkLifecycle Concrete.data.lifecycle')])+'\n'
    complete += 'theorem checked : Safety.checkModule Concrete.data = true := Safety.Candidate.checked_from_parts _ originalUnique bounds authority link lifecycle\n'
    complete += 'theorem certified : '+claim.replace(' data ', ' Concrete.data ')+' := ⟨rfl,rfl,rfl,rfl,rfl,Safety.module_preserves _ checked,Safety.lifecycle_module_preserves _ checked⟩\nend CompleteProbe\n'
    anchors=transformed[transformed.index('namespace ConcreteAnchors'):transformed.index('end ConcreteAnchors')+len('end ConcreteAnchors')].replace('ConcreteAnchors','Expected')
    audit='prelude\nimport safety.Audit\nimport CompleteProbe\n'+anchors+'\n#safety_audit CompleteProbe.checked : Safety.checkModule Concrete.data = true\n#safety_audit CompleteProbe.certified : '+claim.replace(' data ', ' Concrete.data ').replace('ConcreteAnchors.','Expected.')+'\n'
    denied,count=re.subn(r'\[\(([0-9]+), \[0\]\)\]',r'[(\1, [])]',transformed)
    assert count==1, 'Need independent denied-writer fixture'
    mutations={
        'positive': (transformed,complete),
        'unauthorized_writer':(denied,complete),
        'wrong_identity':(transformed.replace('behaviorHash := [115','behaviorHash := [116',1),complete),
        'proof_hole':(transformed,complete.replace(':= Safety.Candidate.checked_from_parts _ originalUnique bounds authority link lifecycle',':= by sorry')),
        'weakened_statement':(transformed,complete[:complete.index('theorem certified :')]+'theorem certified : True := by trivial\nend CompleteProbe\n'),
    }
    results=[]
    def limits():resource.setrlimit(resource.RLIMIT_AS,(8*1024**3,8*1024**3))
    for name,(data,proof) in mutations.items():
        work=args.output/name; work.mkdir()
        for mod in ['safety','CandidateLemmas.olean','CompactPayload.olean','MergeWitness.olean','ModelBridge.olean']:
            (work/mod).symlink_to((args.generic/mod).resolve(),target_is_directory=mod=='safety')
        sources={'Concrete':data,'MergeProbe':merge,'CompleteProbe':proof,'Verify':audit}
        env={**os.environ,'LEAN_PATH':str(work.resolve()),'PATH':str(args.toolchain/'bin')+':'+os.environ['PATH']}
        row={'case':name,'authority':'DiagnosticOnly','stages':[],'original_instance_importable':False}
        for stage,source in sources.items():
            (work/(stage+'.lean')).write_text(source)
            command=[str(args.toolchain/'bin/lean'),'-DmaxHeartbeats=200000','-DmaxRecDepth=4096','-M4096','-j1']
            if stage!='Verify':command+=['-o',stage+'.olean']
            command+=[stage+'.lean']; start=time.monotonic()
            result=subprocess.run(command,cwd=work,env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True,timeout=180,preexec_fn=limits)
            (work/(stage+'.log')).write_text(result.stdout)
            row['stages'].append({'stage':stage,'exit_code':result.returncode,'wall_seconds':time.monotonic()-start,'log':result.stdout,'source_sha256':hashlib.sha256(source.encode()).hexdigest(),'command':command})
            if result.returncode:break
        row['accepted']=all(s['exit_code']==0 for s in row['stages'])
        assert row['accepted']==(name=='positive'),row
        if name=='proof_hole':assert 'forbidden axioms' in row['stages'][-1]['log'],row
        if name=='weakened_statement':assert 'wrong proposition' in row['stages'][-1]['log'],row
        results.append(row)
        (args.output/'controls.json').write_text(json.dumps({'results':results,'all_checks_passed':True,'certificates_issued':0,'metadata':metadata},indent=2)+'\n')
        print(name,'accepted' if row['accepted'] else 'rejected',flush=True)

if __name__=='__main__':main()
