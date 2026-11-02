from pathlib import Path
import re
source=Path('apps/kilogram-cli/tests/support/migration_s54_admission_schema.rs').read_text()
rows=[]
for name,body in re.findall(r'wire!\((\w+)\s*\{(.*?)\}\);',source,re.S):
 fields=[]
 for key,kind in re.findall(r'pub\s+(\w+):\s*(Vec<[^>]+>|Option<[^>]+>|\w+)',body):
  def wire(t):return {'String':'s','PathBuf':'p','u32':'u','u64':'u','usize':'u','bool':'b','Value':'j'}.get(t,t)
  if kind.startswith('Vec<'):desc='a'+wire(kind[4:-1])
  elif kind.startswith('Option<'):desc='n'+wire(kind[7:-1])
  else:desc=wire(kind)
  if key=='raw_compiler_artifact_json':desc='araw'
  fields.append(f"{key}='{desc}'")
 assert fields,name
 rows.append(' '+name+'=@{'+(';'.join(fields))+'}')
assert len(rows)==33,len(rows)
table='$script:S54Schema=@{\n'+'\n'.join(rows)+'\n}'
p=Path('scripts/invoke-migration-s54-whole.ps1');t=p.read_text()
actual=re.search(r'\$script:S54Schema=@\{.*?^\}',t,re.M|re.S)
assert actual and actual.group(0)==table,'Rust/PowerShell wire table drift'
print('closed_types='+str(len(rows)))
