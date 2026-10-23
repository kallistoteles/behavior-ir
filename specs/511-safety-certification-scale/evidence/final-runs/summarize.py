"""Aggregate complete accepted measurements; no mathematical/certificate authority."""
from pathlib import Path
import hashlib,json,statistics
f=Path(__file__).resolve().parents[2]
profile=json.loads((f/'evidence/evaluation-profile.json').read_text())
manifest=json.loads((f/'evidence/candidate-builds/final-01.json').read_text())
a=json.loads((f/'evidence/final-runs/assessment.json').read_text())
assert a['accepted'] and len(a['attempts'])==60
rows={name['case_repeat']:name['report'] for name in a['attempts']}
def summary(values):return {'min':min(values),'median':statistics.median(values),'max':max(values)}
result={'format':'behavior.safety_remediation.scaling_summary.v1','assessment_sha256':hashlib.sha256((f/'evidence/final-runs/assessment.json').read_bytes()).hexdigest(),'candidate_manifest_sha256':hashlib.sha256((f/'evidence/candidate-builds/final-01.json').read_bytes()).hexdigest(),'cases':[],'roles':120,'accepted_pairs':60,'semantic_reproducibility':[]}
for case in profile['cases']:
 attempts=[rows[f'{case["case"]}-{i+1}'] for i in range(case['repetitions'])]
 hashes=[r['stages'][0]['result']['evidence_sha256'] for r in attempts]
 assert len(set(hashes))==1
 result['semantic_reproducibility'].append({'case':case['case'],'repetitions':3,'producer_consumer_and_repeats_identical':True,'evidence_sha256':hashes[0]})
 row={'case':case['case'],'request':case['request'],'dimensions':case['expected'],'roles':{}}
 for index,role in enumerate(['certification','verification']):
  obs=[r['stages'][index] for r in attempts]
  detail={'wall_seconds':summary([s['wall_seconds'] for s in obs]),'waited_child_cpu_seconds':summary([s['waited_child_cpu_seconds'] for s in obs]),'sampled_tree_resident_kib':summary([s['peak_sampled_tree_resident_kib'] for s in obs]),'sampled_tree_virtual_kib':summary([s['peak_sampled_tree_virtual_kib'] for s in obs]),'tool_seconds':{}}
  for label in ['generic','Concrete','Verify','Fresh','outside_tool_intervals']:
   values=[]
   for r,s in zip(attempts,obs):
    tools=r['tool_stages'][role]
    generic=sum(tools[name]['elapsed_ms'] for name in manifest['proof_sources'])/1000
    other={name:tools[name]['elapsed_ms']/1000 for name in ['Concrete','Verify','Fresh']}
    values.append(generic if label=='generic' else s['wall_seconds']-generic-sum(other.values()) if label=='outside_tool_intervals' else other[label])
   detail['tool_seconds'][label]=summary(values)
  row['roles'][role]=detail
 result['cases'].append(row)
for role_index,role in enumerate(['certification','verification']):
 all_roles=[r['report']['stages'][role_index] for r in a['attempts']]
 result[role]={'wall_seconds':summary([s['wall_seconds'] for s in all_roles]),'maximum_sampled_tree_resident_kib':max(s['peak_sampled_tree_resident_kib'] for s in all_roles)}
result['interpretation']='Whole-chain budgets remain300s/role. Tool timings do not subtract from totals. Concrete combines data elaboration and proof; split prototype timings are separate diagnostic evidence. OS cache uncontrolled, no previous request-result reuse. Sampled RSS/VmSize are not exact peaks. Consumed heartbeat/recursion high-water unavailable. Three repeats are descriptive observations, not a probabilistic or unbounded guarantee.'
out=f/'evidence/final-runs/scaling-summary.json';out.write_text(json.dumps(result,sort_keys=True,indent=2)+'\n')
print('case | certmedian(range) | verifmedian(range) | certgeneric/concrete/audit/fresh/other')
for row in result['cases']:
 c=row['roles']['certification'];v=row['roles']['verification'];w=c['wall_seconds'];z=v['wall_seconds'];t=c['tool_seconds']
 print(f'{row["case"]}: {w["median"]:.2f}({w["min"]:.2f}-{w["max"]:.2f}) | {z["median"]:.2f}({z["min"]:.2f}-{z["max"]:.2f}) | '+ '/'.join(f'{t[k]["median"]:.2f}' for k in ['generic','Concrete','Verify','Fresh','outside_tool_intervals']))
print('maximums',result['certification'],result['verification'])
