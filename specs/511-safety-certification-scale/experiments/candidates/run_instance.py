"""Instrument existing trusted diagnostic instance sources; never issues certificates."""
import argparse
import os,json,subprocess,time,resource,hashlib
from pathlib import Path
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--workspace',type=Path,required=True)
parser.add_argument('--toolchain',type=Path,required=True)
args=parser.parse_args()
p=args.workspace.resolve()
os.chdir(p)
tool=str(args.toolchain.resolve()/'bin')
env={**os.environ,'LEAN_PATH':str(p),'PATH':tool+':'+os.environ['PATH']}
records=[]
def limit(): resource.setrlimit(resource.RLIMIT_AS,(8*1024**3,8*1024**3))
for name in ['Concrete','MergeProbe','CompleteProbe','CompleteAudit','Fresh']:
 cmd=([tool+'/leanchecker','--fresh','CompleteProbe'] if name=='Fresh' else [tool+'/lean','-DmaxHeartbeats=200000','-DmaxRecDepth=4096','-M4096','-j1']+([] if name=='CompleteAudit' else ['-o',name+'.olean'])+[name+'.lean'])
 start=time.monotonic(); before=resource.getrusage(resource.RUSAGE_CHILDREN)
 try:
  run=subprocess.run(cmd,env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True,timeout=180,preexec_fn=limit)
  rc=run.returncode; log=run.stdout
 except subprocess.TimeoutExpired as exc: rc=None; log=str(exc)
 after=resource.getrusage(resource.RUSAGE_CHILDREN)
 record={'stage':name,'status':'Passed' if rc==0 else 'Failed','exit_code':rc,'wall_seconds':time.monotonic()-start,'child_cpu_seconds':after.ru_utime+after.ru_stime-before.ru_utime-before.ru_stime,'cumulative_waited_child_maxrss_kib':after.ru_maxrss,'command':cmd,'log':log,'source_sha256':hashlib.sha256(Path(name+'.lean').read_bytes()).hexdigest() if name!='Fresh' else None}
 records.append(record); Path(name+'.log').write_text(log)
 Path('instance-observation.json').write_text(json.dumps({'authority':'DiagnosticOnly','reused_generic_imports':True,'stages':records,'status':record['status']},indent=2)+'\n')
 print(name,record['status'],round(record['wall_seconds'],2),flush=True)
 if rc!=0: break
