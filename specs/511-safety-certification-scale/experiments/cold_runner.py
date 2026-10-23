#!/usr/bin/env python3
"""Sequential cold-process observations; certificate authority stays in Rust/Lean.

Commands are trusted harness configuration, never fields taken from evidence.
The workspaces isolate supplied inputs, not OS access by a hostile executable.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import resource
import platform
import signal
import subprocess
import sys
import time


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'), allow_nan=False)


def write_json(path, value):
    temporary = path.with_suffix(path.suffix + '.tmp')
    temporary.write_text(canonical(value) + '\n')
    temporary.replace(path)


def _memory(pid):
    """Linux samples, explicitly not exact/shared-page-adjusted memory peaks."""
    resident = virtual = 0
    pending = [pid]
    seen = set()
    while pending:
        child = pending.pop()
        if child in seen:
            continue
        seen.add(child)
        try:
            fields = dict(line.split(':', 1) for line in Path(f'/proc/{child}/status').read_text().splitlines() if ':' in line)
            resident += int(fields.get('VmRSS', '0 kB').split()[0])
            virtual += int(fields.get('VmSize', '0 kB').split()[0])
            children = Path(f'/proc/{child}/task/{child}/children').read_text()
            pending.extend(int(value) for value in children.split())
        except (OSError, ValueError):
            # A process can exit between samples. The scope remains a sampled lower bound.
            continue
    return resident, virtual


def _execute(command, workspace, hard_timeout):
    start = time.monotonic()
    before = resource.getrusage(resource.RUSAGE_CHILDREN)
    row = {'command':command, 'status':'Failed', 'exit_code':None,
           'actual_limits':{'wall_seconds':hard_timeout}, 'error':None,
           'peak_sampled_tree_resident_kib':0, 'peak_sampled_tree_virtual_kib':0}
    child = None
    try:
        # The process sees the installed package, not python/behavior from a checkout.
        environment = dict(os.environ)
        environment.pop('PYTHONPATH', None)
        environment.pop('PYTHONHOME', None)
        with (workspace/'process.log').open('wb') as log:
            child = subprocess.Popen(command, cwd=workspace, env=environment,
                stdin=subprocess.DEVNULL, stdout=log, stderr=subprocess.STDOUT,
                start_new_session=True)
            while child.poll() is None:
                resident, virtual = _memory(child.pid)
                row['peak_sampled_tree_resident_kib'] = max(row['peak_sampled_tree_resident_kib'], resident)
                row['peak_sampled_tree_virtual_kib'] = max(row['peak_sampled_tree_virtual_kib'], virtual)
                if time.monotonic() - start >= hard_timeout:
                    row['status'] = 'Exhausted'
                    row['error'] = 'External process wall-time limit; required checks may be unrun'
                    os.killpg(child.pid, signal.SIGKILL)
                    break
                time.sleep(.01)
            row['exit_code'] = child.wait()
        if row['status'] != 'Exhausted':
            result_path = workspace/'result.json'
            if result_path.exists():
                if result_path.stat().st_size > 1024 * 1024:
                    raise ValueError('Compact worker result exceeds 1 MiB')
                result = json.loads(result_path.read_text())
                if not isinstance(result, dict):
                    raise ValueError('Worker result must be an object')
                row['result'] = result
                if row['exit_code'] == 0 and result.get('certificate_present') is True:
                    row['status'] = 'Passed'
                elif result.get('certificate_present') is not True:
                    outcome = result.get('outcome')
                    codes = result.get('diagnostic_codes', [])
                    row['status'] = ('Exhausted' if 'SAFETY_PROOF_EXHAUSTED' in codes else
                                     'Inconclusive' if outcome == 'Inconclusive' else
                                     'Unsupported' if outcome == 'Unsupported' else 'Failed')
                    row['error'] = f'No checked certificate: {outcome}'
                else:
                    row['error'] = f'Process exited {row["exit_code"]}'
            else:
                row['error'] = f'Process exited {row["exit_code"]} without result.json'
    except (OSError, ValueError, TypeError) as error:
        row['error'] = f'{type(error).__name__}: {error}'
    finally:
        if child is not None and child.poll() is None:
            try:
                os.killpg(child.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            child.wait()
        after = resource.getrusage(resource.RUSAGE_CHILDREN)
        row['waited_child_cpu_seconds'] = ((after.ru_utime + after.ru_stime) -
                                          (before.ru_utime + before.ru_stime))
        row['process_wall_seconds'] = time.monotonic() - start
    return row


def observe_environment():
    """Actual starting conditions; never inferred consumed proof counters."""
    mem={line.split(':',1)[0]:int(line.split(':',1)[1].split()[0]) for line in Path('/proc/meminfo').read_text().splitlines()}
    cpu=next((line.split(':',1)[1].strip() for line in Path('/proc/cpuinfo').read_text().splitlines() if line.startswith('model name')),None)
    groups=[]
    for line in Path('/proc/self/cgroup').read_text().splitlines():
        if line.startswith('0::'):
            current=Path('/sys/fs/cgroup')/line[3:].lstrip('/')
            while current!=Path('/sys/fs/cgroup') and current.is_relative_to('/sys/fs/cgroup'):
                limits={}
                for name in ['cpu.max','memory.max','cpuset.cpus.effective']:
                    path=current/name
                    limits[name]=path.read_text().strip() if path.exists() else None
                groups.append({'path':str(current),**limits});current=current.parent
    return {'platform':platform.machine()+'-'+platform.system().lower(),'kernel':platform.release(),
        'cpu_model_raw':cpu,'cpu_model':' '.join(cpu.split()) if cpu else None,'affinity':sorted(os.sched_getaffinity(0)),
        'memory_total_kib':mem['MemTotal'],'memory_available_kib':mem['MemAvailable'],
        'swap_total_kib':mem['SwapTotal'],'visible_cgroups':groups,
        'os_page_cache':'uncontrolled','uid':os.geteuid()}


def validate_environment(profile, observed):
    conditions=profile['conditions']
    for key in ['platform','kernel','cpu_model','affinity','memory_total_kib','swap_total_kib']:
        if observed[key]!=conditions[key]:raise ValueError('Reference environment differs: '+key)
    if observed['memory_available_kib']<conditions['minimum_available_memory_kib_at_start']:
        raise ValueError('Insufficient memory at start of reference attempt')
    if any(row['cpu.max'] is not None and not row['cpu.max'].startswith('max ') for row in observed['visible_cgroups']):
        raise ValueError('CPU quota differs from the frozen reference conditions')
    return observed


def run_chain(output, request, producer_command, consumer_command, *, hard_timeout,
              build_identity=None, profile_identity=None, budgets=None):
    """Run trusted producer/checker programs with data-only transfer and no result reuse.

    The worker must write result.json after its public API call finishes. The producer
    additionally writes bundle.json. Only that bundle and the original request are
    supplied to the fresh consumer. A worker result flag is observation, not a certificate.
    """
    if type(hard_timeout) not in (int,float) or not math.isfinite(hard_timeout) or hard_timeout<=0:
        raise ValueError('hard_timeout must be a positive finite number')
    output = Path(output).resolve()
    output.mkdir(parents=True, exist_ok=False)
    start = time.monotonic()
    encoded_request = canonical(request)
    report = {'format':'behavior.safety_remediation.cold_observation.v1',
              'request_sha256':hashlib.sha256(encoded_request.encode()).hexdigest(),
              'build_identity':build_identity, 'profile_identity':profile_identity,
              'status':'Started', 'certificate_present':False,
              'environment_at_start':observe_environment(),
              'stages':[{'role':'certification','status':'NotRun'},
                        {'role':'verification','status':'NotRun'}],
              'measurement':{'previous_request_results_reused':False,
                  'os_cache':'uncontrolled', 'installed_dependencies':'inherited pinned environment; record build manifest',
                  'memory_scope':'sampled sum of Linux process-tree RSS/VmSize; shared pages may be counted repeatedly',
                  'cpu_scope':'delta of waited-child CPU; runs must be sequential',
                  'actual_heartbeats':None, 'actual_recursion_depth':None,
                  'missing_counter_reason':'Pinned tools expose no consumed-count high-water measurement here',
                  'timing_scope':'input staging through worker result/evidence persistence; observation finalization separate',
                  'isolation':'fresh processes/workspaces; producer directory search denied during checking; trusted same-UID worker, not an OS sandbox'}}
    write_json(output/'report.json', report)
    try:
        for index, (role, template) in enumerate(zip(['certification','verification'],
                                                    [producer_command,consumer_command])):
            stage_start = time.monotonic()
            workspace = output/role
            workspace.mkdir()
            write_json(workspace/'request.json', request)
            if index:
                # No producer analysis, compiled proofs, paths or prior acceptance is supplied.
                (workspace/'bundle.json').write_bytes((output/'certification/bundle.json').read_bytes())
            substitutions = {'{request}':str(workspace/'request.json'),
                             '{output}':str(workspace), '{bundle}':str(workspace/'bundle.json')}
            command = [str(part) for part in template]
            for key,value in substitutions.items():
                command = [part.replace(key,value) for part in command]
            producer = output/'certification'
            producer_mode = None
            if index:
                if os.geteuid() == 0:
                    raise ValueError('Unprivileged worker required to enforce producer workspace denial')
                producer_mode = producer.stat().st_mode & 0o7777
                producer.chmod(0)
            try:
                row = _execute(command, workspace, hard_timeout)
            finally:
                if producer_mode is not None:
                    producer.chmod(producer_mode)
            row.update(role=role, wall_seconds=time.monotonic()-stage_start)
            budget = None if budgets is None else budgets.get(role)
            row['budget_seconds'] = budget
            row['performance'] = 'Unmeasured' if budget is None else 'Passed' if row['wall_seconds'] <= budget else 'Failed'
            report['stages'][index] = row
            report['wall_seconds'] = time.monotonic()-start
            write_json(output/'report.json', report)
            if row['status'] != 'Passed':
                report['status'] = row['status']
                break
        else:
            report['status'] = 'Passed'
            report['certificate_present'] = True
    except (OSError, ValueError, TypeError) as error:
        report['status'] = 'Failed'
        report['error'] = f'{type(error).__name__}: {error}'
    finally:
        report['wall_seconds'] = time.monotonic()-start
        report['performance'] = ('Failed' if any(stage.get('performance') == 'Failed' for stage in report['stages'])
                                 else 'Passed' if all(stage.get('performance') == 'Passed' for stage in report['stages'])
                                 else 'Unmeasured')
        write_json(output/'report.json', report)
    return report


def load_profile(path, expected_identity):
    """Immutable acceptance conditions, independently bound by the frozen build."""
    raw=Path(path).read_bytes()
    if hashlib.sha256(raw).hexdigest()!=expected_identity:
        raise ValueError('Evaluation profile identity differs from the frozen build')
    profile=json.loads(raw)
    if profile.get('state')!='Frozen':
        raise ValueError('Evaluation profile must be Frozen')
    import math
    for role in ['certification','verification']:
        budget=profile.get('budgets',{}).get(role)
        if type(budget) not in (int,float) or not math.isfinite(budget) or budget<=0:
            raise ValueError('Mandatory total budgets must be finite positive numbers')
    cases=profile.get('cases',[])
    names=[case.get('case') for case in cases]
    if not cases or len(names)!=len(set(names)) or not {'A256','A512'}.issubset(names):
        raise ValueError('Incomplete or duplicated mandatory corpus')
    if any(type(case.get('repetitions')) is not int or case['repetitions']<3 for case in cases):
        raise ValueError('Every case requires at least three sequential cold pairs')
    return profile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--request', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--python', required=True, help='Installed pinned consumer interpreter')
    parser.add_argument('--toolchain', required=True)
    parser.add_argument('--hard-timeout', type=float, required=True, help='Observation cap, not an acceptance budget')
    parser.add_argument('--build-manifest', type=Path, required=True)
    parser.add_argument('--profile', type=Path)
    args = parser.parse_args()
    request = json.loads(args.request.read_text())
    worker = Path(__file__).with_name('cold_worker.py')
    def command(role):
        return [args.python,str(worker),role,'--request','{request}',
                '--output','{output}','--bundle','{bundle}','--toolchain',args.toolchain]
    manifest = json.loads(args.build_manifest.read_text())
    profile = None if args.profile is None else load_profile(args.profile, manifest['evaluation_profile_sha256'])
    if profile is not None:validate_environment(profile,observe_environment())
    result = run_chain(args.output,request,command('certification'),command('verification'),
                       hard_timeout=args.hard_timeout,build_identity=manifest,
                       profile_identity=None if profile is None else hashlib.sha256(args.profile.read_bytes()).hexdigest(),
                       budgets=None if profile is None else profile['budgets'])
    print(canonical({'status':result['status'],'performance':result['performance'],
                     'wall_seconds':result['wall_seconds'],'report':str(args.output/'report.json')}),flush=True)
    return 0 if result['status']=='Passed' and result['performance']!='Failed' else 1


if __name__ == '__main__':
    raise SystemExit(main())
