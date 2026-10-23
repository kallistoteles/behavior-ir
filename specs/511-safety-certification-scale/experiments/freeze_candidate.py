#!/usr/bin/env python3
"""Freeze exact inspected build inputs and wheel; no proof or certificate authority."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tomllib


def canonical(value):
    return json.dumps(value,sort_keys=True,separators=(',',':'),allow_nan=False)


def hash_inputs(root, names):
    root=Path(root).resolve();files={}
    for name in sorted(set(names)):
        path=Path(name)
        if path.is_absolute() or '..' in path.parts:
            raise ValueError('Build inputs must be relative within the declared source tree')
        files[str(path)]=hashlib.sha256((root/path).read_bytes()).hexdigest()
    return {'sha256':hashlib.sha256(canonical(files).encode()).hexdigest(),'files':files}


def write_frozen(path, value):
    path=Path(path);path.parent.mkdir(parents=True,exist_ok=True)
    with path.open('x') as handle:handle.write(json.dumps(value,sort_keys=True,indent=2)+'\n')


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source-root',type=Path,required=True)
    parser.add_argument('--wheel',type=Path,required=True)
    parser.add_argument('--inspection',type=Path,required=True)
    parser.add_argument('--consumer-prefix',type=Path,required=True)
    parser.add_argument('--profile',type=Path,required=True)
    parser.add_argument('--output',type=Path,required=True)
    args=parser.parse_args();root=args.source_root.resolve()
    profile_bytes=args.profile.read_bytes();profile_id=hashlib.sha256(profile_bytes).hexdigest()
    expected=args.profile.with_suffix('.sha256').read_text().split()[0]
    if profile_id!=expected:raise ValueError('Original frozen profile identity changed')
    from cold_runner import load_profile
    profile=load_profile(args.profile,expected)
    inspected=json.loads(args.inspection.read_text());record=inspected['record'];proof=record['proof']
    version=tomllib.loads((root/'Cargo.toml').read_text())['workspace']['package']['version']
    pin=json.loads((root/'core-release.json').read_text())
    if (version!='0.14.0' or record['checker_version']!=3 or proof['statement_version']!=2 or
        record['format']!='behavior.safety_evidence.v2' or
        inspected['versions']['binding']['python']!=version or
        inspected['versions']['core']!={'version':pin['version'],'commit':pin['commit']} or
        inspected['versions']['verifier']!='0.10.0' or
        proof['toolchain']['profile']['version']!=profile['approved_dependencies']['lean']):
        raise ValueError('Inspection/package/contract/pin version mismatch')
    names=subprocess.check_output(['git','-C',str(root),'ls-files','-z']).decode().split('\0')
    names=[name for name in names if name and (name.startswith(('crates/','python/behavior/')) or
        name in ['Cargo.toml','Cargo.lock','rust-toolchain.toml','core-release.json','flake.nix','flake.lock','pyproject.toml','README.md','LICENSE','api/public-api.json'])]
    source=hash_inputs(root,names)
    tooling=['scripts/gates.sh','scripts/check-safety-proofs.sh','scripts/release-build.sh','scripts/release-check.sh','release/safety_smoke.py']
    feature='specs/511-safety-certification-scale/experiments/'
    tooling+= [feature+name for name in ['cold_runner.py','cold_worker.py','workloads.py','certification_probe.py','collect_observations.py','freeze_candidate.py','inspect_candidate.py','tests/test_scale_acceptance.py']]
    manifest={'format':'behavior.safety_remediation.candidate_build.v1','state':'Frozen','release':version,
        'source_commit':subprocess.check_output(['git','-C',str(root),'rev-parse','HEAD'],text=True).strip(),
        'source_state':'Reviewed staged precommit input snapshot; no published tag is claimed',
        'source_tree':source,'evaluation_tooling':hash_inputs(root,tooling),
        'source_identity_scope':'All indexed crate/proof/binding code and declared root build inputs; changing evidence/task checkboxes or external analysis prose is not a different compiled input',
        'wheel':str(args.wheel.resolve()),'wheel_sha256':hashlib.sha256(args.wheel.read_bytes()).hexdigest(),
        'consumer_prefix':str(args.consumer_prefix.resolve()),'evaluation_profile_sha256':profile_id,
        'checker_version':3,'statement_version':2,'evidence_format':'behavior.safety_evidence.v2','bundle_format':'behavior.safety_bundle.v2',
        'core':inspected['versions']['core'],'verifier':inspected['versions']['verifier'],
        'proof_sources':proof['sources'],'implementation_identity':proof['implementation_sources'],
        'proof_toolchain':proof['toolchain'],'dependency_identity':record['dependency_identity'],
        'inspection':'Installed API identity inspection only, not proof authority; complete real corpus and gates remain mandatory',
        'expected_requests':{case['case']:{'request':case['request'],'expected':case['expected']} for case in profile['cases']}}
    write_frozen(args.output,manifest)
    print(args.output)

if __name__=='__main__':main()
