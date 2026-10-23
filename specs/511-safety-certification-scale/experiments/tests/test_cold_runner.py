"""Fast process-boundary controls. Fake workers never count as proof evidence."""
from pathlib import Path
import importlib.util
import json
import sys

import pytest

EXPERIMENTS = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('scale_cold_runner', EXPERIMENTS / 'cold_runner.py')
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)


def worker(tmp_path, body):
    tmp_path.mkdir(parents=True, exist_ok=True)
    path = tmp_path / 'trusted_test_worker.py'
    path.write_text('import json, os, pathlib, sys, time\n' + body)
    return [sys.executable, str(path), '{request}', '{output}', '{bundle}']


PRODUCER = '''out=pathlib.Path(sys.argv[2]); out.mkdir(exist_ok=True)
(out/'producer-only').write_text('not consumer input')
(out/'bundle.json').write_text(json.dumps({'producer_pid':os.getpid()}))
(out/'result.json').write_text(json.dumps({'certificate_present':True,'pid':os.getpid(),'cwd':str(pathlib.Path.cwd())}))
'''
CONSUMER = '''out=pathlib.Path(sys.argv[2]); out.mkdir(exist_ok=True)
assert not (pathlib.Path.cwd()/'producer-only').exists()
assert pathlib.Path(sys.argv[3]).parent == pathlib.Path.cwd()
assert json.loads(pathlib.Path(sys.argv[1]).read_text()) == {'case':'tiny'}
(out/'result.json').write_text(json.dumps({'certificate_present':True,'pid':os.getpid(),'cwd':str(pathlib.Path.cwd())}))
'''


def test_separate_processes_workspaces_and_independent_request(tmp_path):
    report = runner.run_chain(tmp_path/'run', {'case':'tiny'},
        worker(tmp_path, PRODUCER), worker(tmp_path/'consumer-script', CONSUMER),
        hard_timeout=5, build_identity={'wheel_sha256':'fixture'}, profile_identity='test')
    assert report['status'] == 'Passed'
    producer, consumer = report['stages']
    assert producer['result']['pid'] != consumer['result']['pid']
    assert producer['result']['cwd'] != consumer['result']['cwd']
    assert producer['wall_seconds'] <= report['wall_seconds']
    assert consumer['wall_seconds'] <= report['wall_seconds']
    assert report['profile_identity'] == 'test'
    assert report['build_identity'] == {'wheel_sha256':'fixture'}
    assert report['measurement']['previous_request_results_reused'] is False
    assert report['measurement']['os_cache'] == 'uncontrolled'
    assert report['measurement']['actual_heartbeats'] is None
    assert json.loads((tmp_path/'run/report.json').read_text())['status'] == 'Passed'


def test_producer_failure_records_unrun_consumer_and_no_certificate(tmp_path):
    report = runner.run_chain(tmp_path/'run', {'case':'tiny'},
        worker(tmp_path, 'sys.exit(7)\n'), ['never-invoked'], hard_timeout=5)
    assert report['status'] == 'Failed'
    assert [s['status'] for s in report['stages']] == ['Failed','NotRun']
    assert report['stages'][0]['exit_code'] == 7
    assert report['certificate_present'] is False
    assert (tmp_path/'run/report.json').is_file()


def test_timeout_kills_group_and_records_unrun_stages(tmp_path):
    report = runner.run_chain(tmp_path/'run', {'case':'tiny'},
        worker(tmp_path, 'time.sleep(20)\n'), ['never-invoked'], hard_timeout=.05)
    assert report['status'] == 'Exhausted'
    assert report['stages'][0]['status'] == 'Exhausted'
    assert report['stages'][1]['status'] == 'NotRun'
    assert report['certificate_present'] is False
    assert report['stages'][0]['actual_limits']['wall_seconds'] == .05


def test_missing_executable_leaves_failure_report(tmp_path):
    report = runner.run_chain(tmp_path/'run', {'case':'tiny'},
        ['/missing-scale-511-executable'], ['never-invoked'], hard_timeout=5)
    assert report['status'] == 'Failed'
    assert report['stages'][0]['error']
    assert report['stages'][1]['status'] == 'NotRun'
    assert report['certificate_present'] is False


def test_consumer_failure_is_not_hidden_by_producer_success(tmp_path):
    report = runner.run_chain(tmp_path/'run', {'case':'tiny'},
        worker(tmp_path, PRODUCER), worker(tmp_path/'consumer-script','sys.exit(9)\n'), hard_timeout=5)
    assert report['status'] == 'Failed'
    assert report['stages'][1]['exit_code'] == 9
    assert report['certificate_present'] is False


def test_existing_output_cannot_reuse_old_result(tmp_path):
    output = tmp_path/'run'; output.mkdir()
    (output/'report.json').write_text('{"status":"Passed"}')
    with pytest.raises(FileExistsError):
        runner.run_chain(output, {'case':'tiny'}, ['never-invoked'], ['never-invoked'], hard_timeout=5)
    assert json.loads((output/'report.json').read_text()) == {'status':'Passed'}


def test_nonzero_inconclusive_worker_result_is_preserved(tmp_path):
    body = "out=pathlib.Path(sys.argv[2]); (out/'result.json').write_text(json.dumps({'certificate_present':False,'outcome':'Inconclusive'})); sys.exit(1)\n"
    report=runner.run_chain(tmp_path/'run',{'case':'tiny'},worker(tmp_path,body),['never-invoked'],hard_timeout=5)
    assert report['status']=='Inconclusive'
    assert report['stages'][0]['result']['outcome']=='Inconclusive'
    assert report['stages'][1]['status']=='NotRun'
    assert report['certificate_present'] is False


def test_completed_proof_over_soft_budget_keeps_validity_separate(tmp_path):
    report=runner.run_chain(tmp_path/'slow',{'case':'tiny'},worker(tmp_path,PRODUCER),
        worker(tmp_path/'consumer-script',CONSUMER),hard_timeout=5,
        budgets={'certification':0.000001,'verification':0.000001})
    assert report['status']=='Passed'
    assert report['certificate_present'] is True
    assert report['performance']=='Failed'


def test_frozen_profile_is_bound_before_execution(tmp_path):
    original=EXPERIMENTS.parent/'evidence/evaluation-profile.json'
    import hashlib
    digest=hashlib.sha256(original.read_bytes()).hexdigest()
    loaded=runner.load_profile(original,digest)
    assert loaded['state']=='Frozen'
    value=json.loads(original.read_text());value['budgets']['verification']+=1
    changed=tmp_path/'profile.json';changed.write_text(json.dumps(value))
    with pytest.raises(ValueError,match='identity'):
        runner.load_profile(changed,digest)
    value['state']='Draft';changed.write_text(json.dumps(value))
    with pytest.raises(ValueError):
        runner.load_profile(changed,hashlib.sha256(changed.read_bytes()).hexdigest())


def test_consumer_cannot_read_producer_workspace(tmp_path):
    body=CONSUMER+'''\nproducer=pathlib.Path.cwd().parent/'certification'
try:
    (producer/'producer-only').read_text()
except PermissionError:
    pass
else:
    raise AssertionError('producer work products accessible during checking')
'''
    report=runner.run_chain(tmp_path/'isolated',{'case':'tiny'},worker(tmp_path,PRODUCER),
        worker(tmp_path/'consumer-script',body),hard_timeout=5)
    assert report['status']=='Passed',report


@pytest.mark.skipif(sys.platform != "linux", reason="Linux reference environment observation")
def test_environment_observation_is_actual_and_separate_from_budget():
    observation=runner.observe_environment()
    assert observation['platform']==__import__('platform').machine()+'-linux'
    assert observation['affinity']==sorted(__import__('os').sched_getaffinity(0))
    assert type(observation['memory_available_kib']) is int
    assert observation['memory_available_kib']>0
    assert 'configured_heartbeats' not in observation


@pytest.mark.parametrize('bad',[0,-1,float('nan'),float('inf'),True])
def test_external_limits_are_finite_positive_numbers(tmp_path,bad):
    with pytest.raises(ValueError):
        runner.run_chain(tmp_path/'invalid',{},[],[],hard_timeout=bad)
