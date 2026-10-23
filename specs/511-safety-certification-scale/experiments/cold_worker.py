#!/usr/bin/env python3
"""Trusted worker: original request -> public Rust certificate/checking API."""
import argparse
import hashlib
from importlib import metadata
import sys
import json
from pathlib import Path
import shutil

import behavior as b
from workloads import build_case, describe_case


def parse_installed_wheel_hash(text):
    try:
        digest=json.loads(text)['archive_info']['hashes']['sha256']
        if not isinstance(digest,str) or len(digest)!=64 or any(c not in '0123456789abcdef' for c in digest):
            raise ValueError('Invalid installed wheel digest')
        return digest
    except (TypeError,KeyError,json.JSONDecodeError) as error:
        raise ValueError('A local installed wheel archive identity is required, not an editable package') from error


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('role',choices=['certification','verification'])
    parser.add_argument('--request',type=Path,required=True)
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--bundle',type=Path,required=True)
    parser.add_argument('--toolchain',required=True)
    args=parser.parse_args()
    wheel_sha256=parse_installed_wheel_hash(metadata.distribution('behavior').read_text('direct_url.json'))
    model,expected=build_case(json.loads(args.request.read_text()))
    dimensions=describe_case(model,expected)
    # Both workers reconstruct the complete expected policy set from original authoring inputs.
    if args.role=='certification':
        result=b.certify_safety(model,output=args.output/'proof',toolchain=args.toolchain)
    else:
        result=b.verify_safety_evidence(model,args.bundle,output=args.output/'proof',toolchain=args.toolchain)
    record=result.to_dict()
    if result.certificate is not None and args.role=='certification':
        shutil.copyfile(args.output/'proof/bundle.json',args.output/'bundle.json')
    summary={'certificate_present':result.certificate is not None,'outcome':record['outcome'],
        'dimensions':dimensions,'versions':b.versions(),'obligations':record['obligations'],
        'diagnostic_codes':[item['code'] for item in record.get('diagnostics',[])],
        'package':b.__file__,'role':args.role,'installed_wheel_sha256':wheel_sha256,'prefix':sys.prefix,
        'checker_version':record['checker_version'],'statement_version':record['proof']['statement_version'],
        'evidence_format':record['format'],'proof_source_identities':record['proof']['sources'],
        'implementation_identity':record['proof']['implementation_sources'],
        'evidence_sha256':hashlib.sha256((args.output/'proof/evidence.json').read_bytes()).hexdigest()}
    (args.output/'result.json').write_text(json.dumps(summary,sort_keys=True,indent=2)+'\n')
    return 0 if result.certificate is not None else 1


if __name__=='__main__':
    raise SystemExit(main())
