#!/usr/bin/env python3
"""Installed identity inspection only. Deliberately cannot issue a certificate."""
import argparse
from importlib import metadata
import json
from pathlib import Path
import sys

import behavior as b
from cold_worker import parse_installed_wheel_hash


@b.entity
class SafetyBuildProbe:
    value=b.field(int)


@b.action
def inspect_update(probe:SafetyBuildProbe):
    b.set_(probe.value,probe.value)


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--toolchain',required=True)
    parser.add_argument('--expected-wheel',type=Path,required=True)
    args=parser.parse_args()
    import hashlib
    digest=hashlib.sha256(args.expected_wheel.read_bytes()).hexdigest()
    installed=parse_installed_wheel_hash(metadata.distribution('behavior').read_text('direct_url.json'))
    if installed!=digest:raise ValueError('Installed wheel differs from candidate artifact')
    if not Path(b.__file__).resolve().is_relative_to(Path(sys.prefix).resolve()):
        raise ValueError('Inspection loaded a source/foreign package instead of the installed wheel')
    args.output.mkdir(parents=True,exist_ok=False)
    model=b.BehaviorModule(entities=[SafetyBuildProbe],actions=[inspect_update],safety=[
        b.FieldOwnership(id='inspection',entity=SafetyBuildProbe,writers={'value':[inspect_update]})])
    result=b.certify_safety(model,output=args.output/'proof',toolchain=args.toolchain,
                           limits={'max_heartbeats':1})
    if result.certificate is not None:
        raise ValueError('Identity inspection unexpectedly completed proof checking; inspect its controls')
    record=result.to_dict()
    if record['outcome']!='Inconclusive' or 'SAFETY_PROOF_EXHAUSTED' not in record['diagnostic_codes']:
        raise ValueError('Identity inspection did not fail at its deliberate proof resource control')
    document={'inspection_not_certification':True,'versions':b.versions(),'record':record,
              'installed_wheel_sha256':installed,'package':b.__file__,'prefix':sys.prefix}
    (args.output/'inspection.json').write_text(json.dumps(document,indent=2,sort_keys=True)+'\n')
    print(args.output/'inspection.json')

if __name__=='__main__':main()
