#!/usr/bin/env python3
"""Archive compact observations without duplicating large generated proof inputs."""
import argparse
import hashlib
import json
from pathlib import Path


def canonical(value):
    return json.dumps(value,sort_keys=True,separators=(',',':'),allow_nan=False)


def tool_stage_observations(case_root, sources):
    """Archive actual stage records; never select a passing retry or invent counters."""
    stages={}
    for role in ['certification','verification']:
        workspaces=list((Path(case_root)/role/'proof').glob('proof-*'))
        if len(workspaces)>1:raise ValueError('Ambiguous repeated proof workspaces in one cold role')
        role_stages={}
        for name in list(sources)+['Concrete','Verify','Fresh']:
            path=None if not workspaces else workspaces[0]/(name+'.resources.json')
            role_stages[name]={'status':'NotRun'} if path is None or not path.exists() else json.loads(path.read_text())
        stages[role]=role_stages
    return stages


def assess_cold(reports, profile, build):
    """Operational completeness, not a certificate constructor or proof authority."""
    expected={f'{case["case"]}-{rep+1}':case for case in profile['cases'] for rep in range(case['repetitions'])}
    seen=set();errors=[];attempts=[]
    required=['inventory_comparison','generic_proof','concrete_proof','theorem_types','axiom_audit','fresh_replay']
    for name,report in reports:
        attempts.append({'case_repeat':name,'report':report})
        if name in seen or name not in expected:
            errors.append(f'{name}: duplicate or unexpected attempt');continue
        seen.add(name);case=expected[name]
        request_identity=hashlib.sha256(canonical(case['request']).encode()).hexdigest()
        if (report.get('profile_identity')!=build['evaluation_profile_sha256'] or
            canonical(report.get('build_identity'))!=canonical(build) or
            report.get('request_sha256')!=request_identity):
            errors.append(f'{name}: changed profile, build or request identity')
        if (report.get('status')!='Passed' or report.get('performance')!='Passed' or
            report.get('certificate_present') is not True):
            errors.append(f'{name}: incomplete proof or failed operational budget')
        if 'tool_stages' in report and any(stage.get('status')!='Passed' for roles in report['tool_stages'].values() for stage in roles.values()):
            errors.append(f'{name}: failed or unrun actual tool stage')
        stages=report.get('stages',[])
        if len(stages)!=2 or [stage.get('role') for stage in stages]!=['certification','verification']:
            errors.append(f'{name}: missing construction/checking role');continue
        for stage in stages:
            result=stage.get('result',{})
            if (stage.get('status')!='Passed' or stage.get('performance')!='Passed' or
                canonical(result.get('dimensions'))!=canonical(case['expected']) or
                any(result.get('obligations',{}).get(key)!='Passed' for key in required)):
                errors.append(f'{name}/{stage.get("role")}: incomplete expected input or proof obligations')
            if case['expected'].get('lifecycle_dependencies',0) and result.get('obligations',{}).get('lifecycle_preservation')!='Passed':
                errors.append(f'{name}: missing lifecycle preservation')
            for observed_key,manifest_key in [('installed_wheel_sha256','wheel_sha256'),('proof_source_identities','proof_sources'),('implementation_identity','implementation_identity')]:
                if manifest_key in build and canonical(result.get(observed_key))!=canonical(build[manifest_key]):errors.append(f'{name}: different installed build/proof identity')
            for key in ['checker_version','statement_version','evidence_format']:
                if key in build and result.get(key)!=build[key]:errors.append(f'{name}: different {key}')
        identities=[stage.get('result',{}).get('evidence_sha256') for stage in stages]
        if not identities[0] or identities[0]!=identities[1]:errors.append(f'{name}: construction/checking semantic evidence differs')
    for name in sorted(set(expected)-seen):errors.append(f'{name}: mandatory report absent')
    return {'format':'behavior.safety_remediation.corpus_assessment.v1','accepted':not errors,
            'expected_attempts':len(expected),'observed_attempts':len(reports),'errors':errors,'attempts':attempts}


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('root', type=Path)
    p.add_argument('output', type=Path)
    p.add_argument('--profile',type=Path)
    p.add_argument('--build-manifest',type=Path)
    a = p.parse_args()
    if a.profile is not None or a.build_manifest is not None:
        if a.profile is None or a.build_manifest is None:p.error('cold assessment needs both --profile and --build-manifest')
        from cold_runner import load_profile
        build=json.loads(a.build_manifest.read_text())
        profile=load_profile(a.profile,build['evaluation_profile_sha256'])
        reports=[]
        for path in sorted(a.root.glob('*/report.json')):
            row=json.loads(path.read_text());row['raw_report_sha256']=hashlib.sha256(path.read_bytes()).hexdigest()
            row['tool_stages']=tool_stage_observations(path.parent,build['proof_sources'])
            reports.append((path.parent.name,row))
        result=assess_cold(reports,profile,build)
        a.output.parent.mkdir(parents=True,exist_ok=True)
        a.output.write_text(json.dumps(result,sort_keys=True,indent=2)+'\n')
        print(f'Observed {len(reports)} attempts; accepted={result["accepted"]}')
        return 0 if result['accepted'] else 1
    observations = []
    for path in sorted(a.root.glob('*/observation.json')):
        row = json.loads(path.read_text())
        # Incomplete historical attempts remain visible, never silently selected away.
        common = {k: v for k, v in row.items() if k not in {
            'assessment', 'independent_expected_action_operations', 'verification'}}
        common['case'] = path.parent.name
        common['raw_observation_sha256'] = hashlib.sha256(path.read_bytes()).hexdigest()
        generated = path.parent/'Concrete.lean'
        if row.get('variant') == 'data-markers' and generated.exists():
            source = generated.read_text()
            if 'import Lean.Elab.Command' not in source:
                common['validity'] = 'Discarded: marker command lacks required Lean.Elab.Command import'
        if 'assessment' in row:
            result = row['assessment']
            common['assessment'] = {k: result.get(k) for k in ['outcome', 'obligations', 'diagnostics']}
            common['proof_files'] = []
            for generated in sorted(path.parent.glob('certificate/proof-*/Concrete.lean')):
                common['proof_files'].append({'source': str(generated),
                    'bytes': generated.stat().st_size,
                    'sha256': hashlib.sha256(generated.read_bytes()).hexdigest(),
                    'stage_resources': {p.name: json.loads(p.read_text())
                        for p in sorted(generated.parent.glob('*.resources.json'))}})
            audit = result.get('proof', {}).get('audit', [])
            if audit:
                common['audit'] = audit
        observations.append(common)
    a.output.parent.mkdir(parents=True, exist_ok=True)
    a.output.write_text(json.dumps({'format': 'behavior.safety_investigation.archive.v1',
        'observations': observations}, sort_keys=True, indent=2)+'\n')
    print(f'Archived {len(observations)} completed observations to {a.output}')


if __name__ == '__main__':
    raise SystemExit(main())
