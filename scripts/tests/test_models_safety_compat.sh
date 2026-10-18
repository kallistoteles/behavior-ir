#!/usr/bin/env bash
# The existing compatibility process must apply its full history rules to Safety sidecars.
set -euo pipefail
here="$(cd "$(dirname "$0")/.." && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
python3 - "$here/check-models-compat.py" "$tmp" <<'PY'
import copy,json,subprocess,sys
from pathlib import Path
script=Path(sys.argv[1]);tmp=Path(sys.argv[2]);pin={"commit":"c1","version":"1"}
fmt="behavior.model_compat.v1"
def write(path,data):path.write_text(json.dumps(data,sort_keys=True)+"\n")
def key(v):return {"model_kind":"field_ownership","model_semantic_version":v,"lowerer_version":1,"core_release":pin}
def tuple_(v,cases,sup=()):
 d={**key(v),"cases":cases}
 if sup:d['supersedes']=[key(n) for n in sup]
 return d
def manifest(*tuples):return {"format":fmt,"tuples":list(tuples)}
def repo(name,base):
 r=tmp/name;(r/'models/fixtures').mkdir(parents=True)
 subprocess.run(['git','init','-q','-b','main',str(r)],check=True)
 write(r/'core-release.json',pin);write(r/'models/fixtures/compat.json',manifest())
 write(r/'models/fixtures/safety-compat.json',base)
 subprocess.run(['git','-C',str(r),'add','-A'],check=True)
 subprocess.run(['git','-C',str(r),'-c','user.name=t','-c','user.email=t@t','commit','-qm','base'],check=True)
 subprocess.run(['git','-C',str(r),'checkout','-qb','feature'],check=True)
 return r
def run(r,groups,passed,*args):
 computed=r/'computed.json';write(computed,{"groups":groups})
 legacy=(r/'models/fixtures/compat.json').read_bytes()
 result=subprocess.run([sys.executable,str(script),'--root',str(r),'--safety','--computed',str(computed),*args],capture_output=True,text=True)
 assert (result.returncode==0)==passed,(r.name,args,result.stdout,result.stderr)
 assert (r/'models/fixtures/compat.json').read_bytes()==legacy,'Safety operation modified legacy fixtures'
def computed(v,cases):return {"tuple":key(v),"cases":cases}
a={"a":{"behavior_hash":"b1","policy_set_hash":"p1","inventory_hash":"i1","models":["m1"],"typed_value":0}}
ab={**a,"b":{"behavior_hash":"b2","policy_set_hash":"p2","inventory_hash":"i2","models":["m2"]}}
r=repo('unchanged',manifest(tuple_(1,a)));run(r,[computed(1,a)],True)
r=repo('changed',manifest(tuple_(1,a)));changed=copy.deepcopy(a);changed['a']['policy_set_hash']='edited';write(r/'models/fixtures/safety-compat.json',manifest(tuple_(1,changed)));run(r,[computed(1,changed)],False)
r=repo('types',manifest());typed=copy.deepcopy(a);typed['a']['typed_value']=False;write(r/'models/fixtures/safety-compat.json',manifest(tuple_(1,typed)));run(r,[computed(1,a)],False)
r=repo('unbuilt',manifest(tuple_(1,ab)));write(r/'models/fixtures/safety-compat.json',manifest(tuple_(2,ab,[1]),tuple_(3,a)));run(r,[computed(3,a)],False)
r=repo('history',manifest(tuple_(2,a,[1])));run(r,[computed(2,a)],True)
r=repo('missingcase',manifest(tuple_(1,ab)));write(r/'models/fixtures/safety-compat.json',manifest(tuple_(2,a,[1])));run(r,[computed(2,a)],False)
r=repo('repeat',manifest(tuple_(1,a)));run(r,[computed(2,a)],True,'--write','--supersede');run(r,[computed(3,a)],True,'--write','--supersede');run(r,[computed(3,a)],True)
doc=json.loads((r/'models/fixtures/safety-compat.json').read_text());assert doc['tuples'][0]['supersedes']==[key(1)]
print('test_models_safety_compat: OK')
PY
