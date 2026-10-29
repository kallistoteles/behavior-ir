#!/usr/bin/env python3
"""Negative public verification controls against a real positive certificate."""
import argparse
import copy
import json
from pathlib import Path
import behavior as b
from certification_probe import make_fo


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--positive', type=Path, required=True)
    p.add_argument('--output', type=Path, required=True)
    p.add_argument('--request',type=Path,help='Original independent workload request, default released A1')
    p.add_argument('--toolchain', required=True)
    a = p.parse_args()
    a.output.mkdir(parents=True, exist_ok=False)
    if a.request is None:
        model, _ = make_fo(1)
    else:
        from workloads import build_case
        model, _ = build_case(json.loads(a.request.read_text()))
    path=a.positive if a.positive.is_file() else a.positive/'certificate/bundle.json'
    bundle = json.loads(path.read_text())
    assert bundle['evidence']['behavior_hash'] == model.behavior_version
    assert bundle['evidence']['outcome'] == 'Certified under assumptions'
    rows = []
    for name in ['module_identity','missing_action','duplicate_action','missing_operation','extra_operation','missing_policy','forged_proof_integrity','missing_audit','wrong_witness_binding']:
        changed = copy.deepcopy(bundle)
        if name == 'module_identity':
            changed['evidence']['behavior_hash'] = 'sha256:'+'0'*64
        elif name == 'missing_action':
            changed['inventory']['actions'].pop()
        elif name == 'duplicate_action':
            changed['inventory']['actions'].append(copy.deepcopy(changed['inventory']['actions'][0]))
        elif name == 'missing_operation':
            next(action for action in changed['inventory']['actions'] if action['operations'])['operations'].pop()
        elif name == 'extra_operation':
            action=next(action for action in changed['inventory']['actions'] if action['operations']);action['operations'].append(copy.deepcopy(action['operations'][0]))
        elif name == 'missing_audit':
            changed['evidence']['proof']['audit'].pop()
        elif name == 'wrong_witness_binding':
            changed['evidence']['proof']['witness']={'untrusted':'incomplete'}
        elif name == 'missing_policy':
            changed['policies'].pop()
        else:
            changed['evidence']['proof']['audit'][0]['axioms'] = ['sorryAx']
        result = b.verify_safety_evidence(model, changed, output=a.output/name, toolchain=a.toolchain)
        assert result.certificate is None and result.outcome == 'Failed', result.to_dict()
        rows.append({'case': name, 'certificate_present': False, 'assessment': result.to_dict()})
    (a.output/'controls.json').write_text(json.dumps({'format': 'behavior.safety_investigation.controls.v1',
        'package': b.__file__, 'scope': 'Tampered public bundle rejection, not a kernel axiom mutation test',
        'results': rows}, sort_keys=True, indent=2)+'\n')
    print(json.dumps({'controls': len(rows), 'passed': True, 'certificates_issued': 0}))


if __name__ == '__main__':
    main()
