#!/usr/bin/env python3
"""Neutral public-API investigation driver; never changes certification semantics.

Run with an isolated installation of the published wheel. Outputs are observations,
not certificates issued by this driver; certificate authority remains in the library.
"""
import argparse
import hashlib
import json
import platform
from pathlib import Path
import resource
import sys
import time

import behavior as b


def make_fo(actions, operations=1, policies=1, padding=0):
    fields = [f'value_{i:04d}' + 'x' * padding for i in range(operations)]
    item = b.entity(type('Item', (), {name: b.field(int) for name in fields}))
    edits = []
    expected = {}
    for index in range(actions):
        def make(index):
            def edit(item):
                for offset, field in enumerate(fields):
                    b.set_(getattr(item, field), index + offset)
            edit.__name__ = f'edit_{index:04d}' + 'x' * padding
            edit.__annotations__ = {'item': item}
            return b.action(edit)
        action = make(index)
        edits.append(action)
        expected[action.name] = operations
    if policies > operations:
        raise ValueError('Each distinct partition policy needs at least one protected field')
    controls = [b.FieldOwnership(id=f'control_{i}', entity=item,
        writers={field: edits for j, field in enumerate(fields) if j % policies == i})
        for i in range(policies)]
    return b.BehaviorModule(entities=[item], actions=edits, safety=controls), expected


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--actions', type=int, default=1)
    parser.add_argument('--operations', type=int, default=1)
    parser.add_argument('--policies', type=int, default=1)
    parser.add_argument('--padding', type=int, default=0)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--toolchain', required=True)
    parser.add_argument('--expect', choices=['certified', 'inconclusive'])
    parser.add_argument('--verify', action='store_true')
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    row = {'format': 'behavior.safety_investigation.observation.v1',
        'kind': 'UnmodifiedPublishedCertification', 'argv': sys.argv,
        'driver_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        'package': b.__file__, 'versions': b.versions(), 'platform': platform.platform(),
        'python': sys.version, 'requested': vars(args) | {'output': str(args.output)},
        'toolchain': args.toolchain, 'status': 'Started', 'certificate_present': False}
    def save():
        (args.output/'observation.json').write_text(json.dumps(row, sort_keys=True, indent=2)+'\n')
    save()
    start = time.monotonic()
    model, expected = make_fo(args.actions, args.operations, args.policies, args.padding)
    row['authoring_analysis_seconds'] = time.monotonic() - start
    analysis = model.safety_analysis.to_dict()
    inventory = analysis['inventory']['actions']
    actual = {action['name']: len(action['operations']) for action in inventory}
    assert actual == expected, (actual, expected)
    assert analysis['issues'] == [], analysis['issues']
    wire = model.to_wire_json()
    (args.output/'module.json').write_text(wire if isinstance(wire,str) else json.dumps(wire))
    (args.output/'analysis.json').write_text(json.dumps(analysis, sort_keys=True, indent=2)+'\n')
    row.update(behavior_hash=model.behavior_version, policy_set_hash=analysis['policy_set_hash'],
        checker_version=analysis['checker_version'], actions=len(inventory),
        operations=sum(actual.values()), ownership_policies=len(analysis['policies']),
        lifecycle_dependencies=0, inventory_json_bytes=len(json.dumps(analysis['inventory'],sort_keys=True,separators=(',',':')).encode()),
        independent_expected_action_operations=expected, analysis_issues=[])
    save()
    start = time.monotonic()
    cpu_before = resource.getrusage(resource.RUSAGE_CHILDREN)
    assessment = b.certify_safety(model, output=args.output/'certificate', toolchain=args.toolchain)
    cpu_after = resource.getrusage(resource.RUSAGE_CHILDREN)
    row.update(certification_wall_seconds=time.monotonic()-start,
        observed_child_cpu_seconds=(cpu_after.ru_utime+cpu_after.ru_stime)-(cpu_before.ru_utime+cpu_before.ru_stime),
        certificate_present=assessment.certificate is not None, assessment=assessment.to_dict(), status='Completed')
    row['measurement_limits'] = ['Child CPU is waited-child aggregate, not per-stage CPU',
        'Library memory records are direct-child 20ms samples, not exact peaks',
        'No actual heartbeat or recursion high-water reading is claimed',
        'Cache temperature is uncontrolled; no cold/warm claim']
    if args.verify and assessment.certificate is not None:
        start = time.monotonic()
        result = b.verify_safety_evidence(model, args.output/'certificate/bundle.json',
            output=args.output/'verified', toolchain=args.toolchain)
        row['verification_wall_seconds'] = time.monotonic()-start
        row['verification'] = result.to_dict()
        assert result.certificate is not None
    save()
    print(json.dumps({k: row[k] for k in ['status','actions','operations','ownership_policies',
        'behavior_hash','certificate_present','certification_wall_seconds']},sort_keys=True),flush=True)
    print(json.dumps({'outcome': row['assessment']['outcome'],
        'diagnostics': row['assessment'].get('diagnostics'),
        'obligations': row['assessment']['obligations']},sort_keys=True),flush=True)
    actual = 'certified' if row['certificate_present'] else 'inconclusive' if row['assessment']['outcome']=='Inconclusive' else 'other'
    return 1 if args.expect and actual != args.expect else 0


if __name__ == '__main__':
    raise SystemExit(main())
