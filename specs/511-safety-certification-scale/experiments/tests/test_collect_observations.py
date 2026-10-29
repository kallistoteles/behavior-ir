"""Report aggregation controls. Synthetic flags are not mathematical evidence."""
import copy
import hashlib
import json
from pathlib import Path
import sys

sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
import collect_observations as collector


def corpus():
    profile={'cases':[{'case':name,'repetitions':3,'request':{'actions':count},'expected':{'actions':count}}
                      for name,count in [('A256',256),('A512',512)]]}
    build={'state':'Frozen','evaluation_profile_sha256':'profile'}
    reports=[]
    for case in profile['cases']:
        for rep in range(3):
            result={'dimensions':case['expected'],'obligations':{name:'Passed' for name in
                ['inventory_comparison','generic_proof','concrete_proof','theorem_types','axiom_audit','fresh_replay']},
                'evidence_sha256':'same'}
            report={'status':'Passed','certificate_present':True,'performance':'Passed','profile_identity':'profile',
                    'build_identity':build,'request_sha256':hashlib.sha256(json.dumps(case['request'],sort_keys=True,separators=(',',':')).encode()).hexdigest(),
                    'stages':[dict(role=role,status='Passed',performance='Passed',result=copy.deepcopy(result)) for role in ['certification','verification']]}
            reports.append((f'{case["case"]}-{rep+1}',report))
    return profile,build,reports


def test_complete_exact_repeated_reports_are_required():
    profile,build,reports=corpus()
    assert collector.assess_cold(reports,profile,build)['accepted']
    for changed in [reports[:-1],reports+reports[:1]]:
        result=collector.assess_cold(changed,profile,build)
        assert not result['accepted']
        assert result['errors']


def test_failed_started_unrun_and_budget_misses_are_retained():
    profile,build,reports=corpus()
    for field,value in [('status','Started'),('performance','Failed'),('certificate_present',False)]:
        changed=copy.deepcopy(reports);changed[0][1][field]=value
        result=collector.assess_cold(changed,profile,build)
        assert not result['accepted']
        assert len(result['attempts'])==6


def test_changed_profile_build_counts_and_missing_obligations_cannot_pass():
    profile,build,reports=corpus()
    for field,value in [('profile_identity','changed'),('build_identity',{'state':'different'})]:
        changed=copy.deepcopy(reports);changed[0][1][field]=value
        assert not collector.assess_cold(changed,profile,build)['accepted']
    changed=copy.deepcopy(reports)
    changed[0][1]['stages'][0]['result']['obligations'].pop('fresh_replay')
    assert not collector.assess_cold(changed,profile,build)['accepted']
    changed=copy.deepcopy(reports)
    changed[0][1]['stages'][0]['result']['dimensions']['actions']=False
    assert not collector.assess_cold(changed,profile,build)['accepted']


def test_tool_stage_archive_preserves_unrun_and_applied_limits(tmp_path):
    work=tmp_path/'certification/proof/proof-1-0';work.mkdir(parents=True)
    (work/'Data.resources.json').write_text(json.dumps({'status':'Passed','elapsed_ms':12,'configured':{'max_heartbeats':200000}}))
    stages=collector.tool_stage_observations(tmp_path,{'Data':'identity'})
    assert stages['certification']['Data']['status']=='Passed'
    assert stages['certification']['Data']['configured']['max_heartbeats']==200000
    assert stages['certification']['Fresh']['status']=='NotRun'
    assert stages['verification']['Concrete']['status']=='NotRun'
    second=work.parent/'proof-2-0';second.mkdir()
    with __import__('pytest').raises(ValueError):collector.tool_stage_observations(tmp_path,{'Data':'identity'})
