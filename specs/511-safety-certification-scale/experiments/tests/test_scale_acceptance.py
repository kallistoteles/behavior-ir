"""Mandatory real corpus execution. Never included in the fast pytest gate.

Requires the frozen installed candidate, original profile, fresh output root and
sequential execution. Missing configuration is a failure, never a skipped proof.
"""
import hashlib
import json
import os
from pathlib import Path
import sys

import pytest

EXPERIMENTS=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(EXPERIMENTS))
import cold_runner

FEATURE=EXPERIMENTS.parent
PROFILE=FEATURE/'evidence/evaluation-profile.json'
CORPUS=json.loads(PROFILE.read_text())['cases']
PARAMETERS=[(case,rep) for case in CORPUS for rep in range(case['repetitions'])]


@pytest.mark.parametrize('case,repetition', PARAMETERS,
                         ids=[f'{case["case"]}-{rep+1}' for case,rep in PARAMETERS])
def test_frozen_case_certifies_and_independently_checks(case,repetition):
    # Explicit required deployment/evaluation inputs. No fake worker or source-package fallback.
    python=os.environ['BEHAVIOR_SAFETY_SCALE_PYTHON']
    toolchain=os.environ['BEHAVIOR_SAFETY_SCALE_TOOLCHAIN']
    manifest_path=Path(os.environ['BEHAVIOR_SAFETY_SCALE_BUILD'])
    output=Path(os.environ['BEHAVIOR_SAFETY_SCALE_OUTPUT'])/f'{case["case"]}-{repetition+1}'
    manifest=json.loads(manifest_path.read_text())
    profile=cold_runner.load_profile(PROFILE, manifest['evaluation_profile_sha256'])
    cold_runner.validate_environment(profile,cold_runner.observe_environment())
    assert manifest['state']=='Frozen'
    assert manifest['checker_version']==3 and manifest['statement_version']==2
    assert manifest['release']=='0.14.0'
    assert hashlib.sha256(Path(manifest['wheel']).read_bytes()).hexdigest()==manifest['wheel_sha256']
    worker=EXPERIMENTS/'cold_worker.py'
    def command(role):
        return [python,str(worker),role,'--request','{request}','--output','{output}',
                '--bundle','{bundle}','--toolchain',toolchain]
    report=cold_runner.run_chain(output,case['request'],command('certification'),command('verification'),
        hard_timeout=profile['observation_hard_timeout_seconds_per_role'],build_identity=manifest,
        profile_identity=manifest['evaluation_profile_sha256'],budgets=profile['budgets'])
    assert report['status']=='Passed',report
    assert report['certificate_present'] is True
    assert report['performance']=='Passed',report
    for stage in report['stages']:
        assert stage['result']['dimensions']==case['expected']
        assert stage['result']['installed_wheel_sha256']==manifest['wheel_sha256']
        assert stage['result']['proof_source_identities']==manifest['proof_sources']
        assert stage['result']['implementation_identity']==manifest['implementation_identity']
        assert Path(stage['result']['package']).resolve().is_relative_to(Path(manifest['consumer_prefix']).resolve())
        assert all(stage['result']['obligations'].get(k)=='Passed' for k in
            ['inventory_comparison','generic_proof','concrete_proof','theorem_types','axiom_audit','fresh_replay'])
    assert report['stages'][0]['result']['evidence_sha256']==report['stages'][1]['result']['evidence_sha256']
