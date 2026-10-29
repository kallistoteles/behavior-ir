#!/usr/bin/env python3
"""Diagnostic Lean attribution only. Success does not issue a Safety Certificate.

Each invocation runs one child. RUSAGE_CHILDREN therefore describes this probe's
Lean process, including kernel-reported maximum RSS. Generic imports are reused;
these timings must not be described as end-to-end or cold certification timings.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import resource
import re
import subprocess
import time


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--original', type=Path, required=True)
    p.add_argument('--output', type=Path, required=True)
    p.add_argument('--toolchain', type=Path, required=True)
    p.add_argument('--variant', choices=['data', 'data-markers', 'checked', 'kernel', 'full', 'noncomputable-data',
        'noncomputable-full', 'noncomputable-kernel', 'wellformed', 'authority', 'lifecycle',
        'unique', 'without-unique', 'eval', 'no-anchors-data', 'no-anchors-checked',
        'no-anchors-kernel', 'raw-nat-data', 'raw-nat-full', 'raw-nat-kernel',
        'typed-cons-data', 'typed-cons-full', 'thin-anchors-data', 'empty-anchors-data',
        'no-anchors-no-hashes-data', 'no-anchors-no-actions-data',
        'no-anchors-no-policies-data', 'no-anchors-factored-hashes-data',
        'no-anchors-factored-hashes-checked', 'no-anchors-factored-hashes-kernel',
        'typed-cons-factored-hashes-full'], required=True)
    p.add_argument('--seconds', type=int, default=60)
    p.add_argument('--depth', type=int, default=4096)
    p.add_argument('--heartbeats', type=int, default=200000)
    p.add_argument('--precompiled-data', type=Path,
        help='For component attribution only: import a previously compiled diagnostic Concrete module')
    a = p.parse_args()
    a.output.mkdir(parents=True, exist_ok=False)
    original = a.original.resolve()
    text = original.read_text()
    prefix = text.split('theorem checked :', 1)[0]
    variants = {
        'data': prefix + 'end Concrete\n',
        'checked': text.split('theorem certified :', 1)[0] + 'end Concrete\n',
        'kernel': text.replace(':= by decide', ':= by decide +kernel'),
        'full': text,
        'wellformed': prefix + 'theorem probe : Safety.wellFormed data = true := by decide\nend Concrete\n',
        'authority': prefix + 'theorem probe : Safety.checkAuthority data = true := by decide\nend Concrete\n',
        'lifecycle': prefix + 'theorem probe : (Safety.checkLifecycleLink data && Safety.checkLifecycle data.lifecycle) = true := by decide\nend Concrete\n',
        'unique': prefix + 'theorem probe : data.actionHashes.Pairwise (· ≠ ·) := by decide\nend Concrete\n',
        'eval': prefix + '#eval Safety.checkModule data\nend Concrete\n',
    }
    check_source = (original.parent/'safety/Check.lean').read_text()
    well_formed = check_source.split('def wellFormed ', 1)[1].split('\ndef checkLifecycleLink', 1)[0]
    without_unique = 'def probeWellFormed ' + well_formed.replace(
        '  decide (m.actionHashes.Pairwise (· ≠ ·)) &&\n', '')
    variants['without-unique'] = (prefix + 'open Safety\n' + without_unique +
        '\ntheorem probe : probeWellFormed data = true := by decide\nend Concrete\n')
    variants['noncomputable-data'] = variants['data'].replace('\ndef ', '\nnoncomputable def ')
    variants['noncomputable-full'] = text.replace('\ndef ', '\nnoncomputable def ')
    variants['noncomputable-kernel'] = variants['kernel'].replace('\ndef ', '\nnoncomputable def ')
    for suffix, existing in [('data', 'data'), ('full', 'full'), ('kernel', 'kernel')]:
        # Only anchor numeral representation changes. nat_lit elaborates a raw
        # Nat literal instead of synthesizing an OfNat instance for every byte.
        anchors, rest = variants[existing].split('namespace Concrete\n', 1)
        anchors = re.sub(r'\b[0-9]+\b', lambda m: '(nat_lit '+m.group()+')', anchors)
        variants['raw-nat-'+suffix] = anchors + 'namespace Concrete\n' + rest
    for suffix in ['data', 'full']:
        anchors, rest = variants[suffix].split('namespace Concrete\n', 1)
        def typed_leaf(match):
            values = match.group(2).split(',')
            tail = '(@List.nil Nat)'
            for value in reversed(values):
                tail = '(@List.cons Nat (nat_lit '+value+') '+tail+')'
            return match.group(1)+tail
        anchors = re.sub(r'(^def (?:inventory|policies)[0-9]+ : List Nat := )\[([0-9,]+)\]',
            typed_leaf, anchors, flags=re.MULTILINE)
        variants['typed-cons-'+suffix] = anchors + 'namespace Concrete\n' + rest
    # Invalid identity data, deliberately useful only for attributing cost to
    # contents versus the unchanged number of anchor declarations.
    variants['empty-anchors-data'] = re.sub(
        r'(^def (?:inventory|policies)[0-9]+ : List Nat := )\[[0-9,]+\]',
        r'\1[]', variants['data'], flags=re.MULTILINE)
    variants['thin-anchors-data'] = re.sub(
        r'(^def (?:inventory|policies)[0-9]+ : List Nat := )\[[0-9,]+\]',
        r'\1[0]', variants['data'], flags=re.MULTILINE)
    # Attribution only: payload fields are not read by checkModule. Dropping them
    # is deliberately NOT a candidate certificate or an identity-preserving fix.
    for suffix, existing in [('data', 'data'), ('checked', 'checked'), ('kernel', 'kernel')]:
        body = variants[existing].split('namespace Concrete\n', 1)[1]
        body = body.replace('ConcreteAnchors.inventory', '([] : List (List Nat))')
        body = body.replace('ConcreteAnchors.policies', '([] : List (List Nat))')
        variants['no-anchors-'+suffix] = 'import safety.Check\nnamespace Concrete\n' + body
    for label, field in [('hashes', 'actionHashes'), ('actions', 'actions'), ('policies', 'policies')]:
        variants['no-anchors-no-'+label+'-data'] = re.sub(
            '^'+field+r' := .*$', field+' := []', variants['no-anchors-data'], flags=re.MULTILINE)
    for label, existing in [('no-anchors-factored-hashes-data', 'no-anchors-data'),
            ('no-anchors-factored-hashes-checked', 'no-anchors-checked'),
            ('no-anchors-factored-hashes-kernel', 'no-anchors-kernel'),
            ('typed-cons-factored-hashes-full', 'typed-cons-full')]:
        base = variants[existing]
        match = re.search(r'^actionHashes := (.*)$', base, flags=re.MULTILINE)
        hashes = json.loads(match.group(1))
        helpers = ''.join('def probeHash'+str(i)+' : List Nat := '+json.dumps(h,separators=(',',':'))+'\n'
            for i,h in enumerate(hashes))
        replacement = '['+','.join('probeHash'+str(i) for i in range(len(hashes)))+']'
        base = base[:match.start(1)]+replacement+base[match.end(1):]
        variants[label] = base.replace('def data :', helpers+'def data :', 1)
    # This command requires Lean.Elab.Command, unlike the production input.
    # Consequently marker timings are instrumentation-only, never a baseline.
    marked = ['import Lean.Elab.Command']
    for number, line in enumerate(variants['data'].splitlines(), 1):
        marked.append(line)
        if number == 2 or (line.startswith('def ') and (number % 256 == 3 or line.startswith('def data'))):
            if not line.startswith('def data'):
                marked.append(f'run_cmd do\n  IO.println s!"[511] line {number} {{← IO.monoMsNow}}"\n  (← IO.getStdout).flush')
    marked.append('run_cmd do IO.println s!"[511] data_complete {← IO.monoMsNow}"')
    variants['data-markers'] = '\n'.join(marked)+'\n'
    selected = variants[a.variant]
    source_name = 'Concrete'
    if a.precompiled_data:
        if a.variant not in ['wellformed','authority','lifecycle','unique','without-unique','eval']:
            raise ValueError('Precompiled data is permitted only for component attribution')
        selected = selected.replace(prefix, 'import Concrete\nnamespace Probe\nopen Concrete\n', 1)
        selected = selected.replace('end Concrete\n', 'end Probe\n')
        source_name = 'Probe'
    source = a.output/(source_name+'.lean')
    source.write_text(selected)
    (a.output/'safety').symlink_to(original.parent/'safety', target_is_directory=True)
    command = [str(a.toolchain/'bin/lean'), f'-DmaxHeartbeats={a.heartbeats}', '-M4096', '-j1',
        f'-DmaxRecDepth={a.depth}', '-o', source_name+'.olean', source_name+'.lean']
    row = {'format': 'behavior.safety_investigation.lean_probe.v1', 'authority': 'DiagnosticOnly',
        'variant': a.variant, 'original': str(original), 'original_sha256': digest(original),
        'source_sha256': digest(source), 'source_bytes': source.stat().st_size,
        'driver_sha256': digest(Path(__file__)), 'command': command, 'cwd': str(a.output.resolve()),
        'wall_budget_seconds': a.seconds, 'virtual_memory_budget_bytes': 8*1024**3,
        'measurement_limits': ['Generic olean imports reused; cache temperature uncontrolled',
            'Heartbeat and recursion high-water counters unavailable',
            'Diagnostic variant is not a library certificate'], 'status': 'Started'}
    def save():
        (a.output/'observation.json').write_text(json.dumps(row, sort_keys=True, indent=2)+'\n')
    save()
    env = os.environ.copy()
    env['LEAN_PATH'] = str(original.parent)
    if a.precompiled_data:
        env['LEAN_PATH'] = str(a.precompiled_data.resolve())+':'+env['LEAN_PATH']
        row['precompiled_data'] = str(a.precompiled_data.resolve())
        row['precompiled_data_sha256'] = digest(a.precompiled_data/'Concrete.olean')
        save()
    def limits():
        resource.setrlimit(resource.RLIMIT_AS, (8*1024**3, 8*1024**3))
    start = time.monotonic()
    with (a.output/'lean.log').open('w') as log:
        proc = subprocess.Popen(command, cwd=a.output, env=env, stdout=log, stderr=subprocess.STDOUT,
            preexec_fn=limits)
        try:
            rc = proc.wait(timeout=a.seconds)
            row['status'] = 'Passed' if rc == 0 else 'Failed'
        except subprocess.TimeoutExpired:
            proc.kill()
            rc = proc.wait()
            row['status'] = 'WallTimeLimit'
    usage = resource.getrusage(resource.RUSAGE_CHILDREN)
    row.update(wall_seconds=time.monotonic()-start, exit_code=rc,
        cpu_user_seconds=usage.ru_utime, cpu_system_seconds=usage.ru_stime,
        max_rss_kib=usage.ru_maxrss, log=(a.output/'lean.log').read_text())
    if (a.output/(source_name+'.olean')).exists():
        row['olean_sha256'] = digest(a.output/(source_name+'.olean'))
    save()
    print(json.dumps({k: row[k] for k in ['variant','status','wall_seconds','cpu_user_seconds',
        'cpu_system_seconds','max_rss_kib','log']}, sort_keys=True), flush=True)


if __name__ == '__main__':
    main()
