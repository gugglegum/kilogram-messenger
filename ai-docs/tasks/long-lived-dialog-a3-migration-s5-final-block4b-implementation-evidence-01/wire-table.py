from pathlib import Path
import re,sys
source=Path('apps/kilogram-cli/tests/support/migration_s54_admission_schema.rs').read_text(encoding='utf-8')
rows=[]
for name,body in re.findall(r'wire!\((\w+)\s*\{(.*?)\}\);',source,re.S):
 fields=[]
 for key,kind in re.findall(r'pub\s+(\w+):\s*(Vec<[^>]+>|Option<[^>]+>|\w+)',body):
  def wire(t):return {'String':'s','PathBuf':'p','u32':'u','u64':'u','usize':'u','bool':'b','Value':'j'}.get(t,t)
  desc='a'+wire(kind[4:-1]) if kind.startswith('Vec<') else 'n'+wire(kind[7:-1]) if kind.startswith('Option<') else wire(kind)
  if key=='raw_compiler_artifact_json':desc='araw'
  fields.append(f"{key}='{desc}'")
 assert fields;rows.append(' '+name+'=@{'+(';'.join(fields))+'}')
assert len(rows)==37,len(rows)
table='$script:S54Schema=@{\n'+'\n'.join(rows)+'\n}'
p=Path('scripts/invoke-migration-s54-whole.ps1');text=p.read_text(encoding='utf-8');match=re.search(r'\$script:S54Schema=@\{.*?^\}',text,re.M|re.S);assert match
if '--write' in sys.argv:p.write_text(text[:match.start()]+table+text[match.end():],encoding='utf-8')
else:assert match.group(0)==table,'closed wire table drift'
print('closed_wire_types='+str(len(rows)))
