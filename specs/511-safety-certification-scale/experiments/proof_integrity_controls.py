#!/usr/bin/env python3
"""Diagnostic mutations; never returns or issues a Safety Certificate."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import resource
import re
import subprocess
import time


def mutations(original):
    """Versioned trusted templates only, not an arbitrary source-editing service."""
    match = re.search(r'(?m)^(theorem checked : (.*?)) := (.*?)\n', original)
    if match is None:
        raise ValueError('Missing concrete checked declaration')
    signature, proposition, body = match.groups()
    def checked(replacement):
        return original[:match.start()]+replacement+'\n'+original[match.end():]
    denied, count = re.subn(r'\[\(([0-9]+), \[0\]\)\]', r'[(\1, [])]', original)
    if count != 1:
        raise ValueError('Use the one-writer positive reference, not an ambiguous control')
    start = original.index('theorem certified :')
    identity = original.replace('behaviorHash := [115', 'behaviorHash := [116', 1)
    payload = original.replace('compactPayload "', 'compactPayload "x', 1)
    if identity == original or payload == original:
        raise ValueError('Expected statement-2 identity/payload template')
    return {
        'unauthorized_writer': denied,
        'proof_hole': checked(signature+' := by sorry'),
        'weakened_statement': original[:start]+'theorem certified : True := by trivial\nend Concrete\n',
        'wrong_identity': identity,
        'wrong_payload': payload,
        'custom_axiom': checked('axiom UntrustedPremise : '+proposition+'\n'+signature+' := UntrustedPremise'),
        'native_axiom': checked(signature+' := True.rec (motive := fun _ => '+proposition+') ('+body+') Lean.trustCompiler').replace('import safety.Check','import Lean\nimport safety.Check',1),
    }


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--positive-workspace', type=Path, required=True)
    p.add_argument('--output', type=Path, required=True)
    p.add_argument('--toolchain', type=Path, required=True)
    a = p.parse_args()
    a.output.mkdir(parents=True, exist_ok=False)
    original = (a.positive_workspace/'Concrete.lean').read_text()
    variants = mutations(original)
    env = os.environ.copy()
    records = []
    def limits():
        resource.setrlimit(resource.RLIMIT_AS, (8*1024**3, 8*1024**3))
    for name, source in variants.items():
        work = a.output/name
        work.mkdir()
        # Never import the original positive Concrete in place of the mutated
        # module being audited. All transitive safety imports resolve via work.
        env['LEAN_PATH'] = str(work.resolve())
        (work/'safety').symlink_to(a.positive_workspace.resolve()/'safety', target_is_directory=True)
        (work/'Concrete.lean').write_text(source)
        (work/'Verify.lean').write_text((a.positive_workspace/'Verify.lean').read_text())
        row = {'case': name, 'source_sha256': hashlib.sha256(source.encode()).hexdigest(),
            'authority': 'NegativeDiagnosticOnly', 'stages': []}
        for stage in ['Concrete', 'Verify']:
            command = [str(a.toolchain/'bin/lean'), '-DmaxHeartbeats=200000', '-M4096', '-j1',
                '-DmaxRecDepth=4096']
            if stage == 'Concrete':
                command += ['-o', 'Concrete.olean']
            command += [stage+'.lean']
            start = time.monotonic()
            result = subprocess.run(command, cwd=work, env=env, capture_output=True,
                text=True, timeout=60, preexec_fn=limits)
            log = result.stdout+result.stderr
            (work/(stage+'.log')).write_text(log)
            row['stages'].append({'stage': stage, 'command': command, 'exit_code': result.returncode,
                'wall_seconds': time.monotonic()-start, 'log': log})
            if result.returncode:
                row['rejected_at'] = stage
                break
        assert 'rejected_at' in row, row
        if name in {'unauthorized_writer', 'wrong_identity'}:
            assert row['rejected_at'] == 'Concrete'
        else:
            assert row['rejected_at'] == 'Verify'
            expected = 'forbidden axioms' if name in {'proof_hole','custom_axiom','native_axiom'} else 'wrong proposition'
            assert expected in row['stages'][-1]['log'], row
        records.append(row)
    (a.output/'controls.json').write_text(json.dumps({'format': 'behavior.safety_investigation.proof_controls.v1',
        'original_source_sha256': hashlib.sha256(original.encode()).hexdigest(),
        'toolchain': str(a.toolchain), 'results': records}, sort_keys=True, indent=2)+'\n')
    print(json.dumps({'controls':len(records),'passed':True,'certificates_issued':0}))


if __name__ == '__main__':
    main()
