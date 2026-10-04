import re,subprocess,sys,os
sys.stdout.reconfigure(encoding='utf-8')
text=open('_workspace/p1-02-mining/02_sprint_contract.md',encoding='utf-8').read().replace('\r','')
sec=None; rows=[]
for line in text.split('\n'):
    if line.startswith('## '): sec=line[3:]
    m=re.match(r'^\|\s*\*{0,2}SC-(\d+)',line)
    if m and sec and sec.startswith('1.'):
        cols=[c for c in line.split('|')]
        rows.append((int(m.group(1)),cols[3] if len(cols)>3 else ''))
# collect all fn names in code
names=set()
for root in ['server','tools','client/Assets/_Project/Tests','tests']:
    for dp,dn,fn in os.walk(root):
        if 'target' in dp or 'Library' in dp: continue
        for f in fn:
            if f.endswith(('.rs','.cs','.py')):
                s=open(os.path.join(dp,f),encoding='utf-8',errors='ignore').read()
                names.update(re.findall(r'fn\s+([A-Za-z_][A-Za-z0-9_]*)',s))
                names.update(re.findall(r'void\s+([A-Za-z_][A-Za-z0-9_]*)\s*\(',s))
                names.update(re.findall(r'def\s+([A-Za-z_][A-Za-z0-9_]*)',s))
                names.update(re.findall(r'mod\s+([a-z_][a-z0-9_]*)',s))
for sc,meth in rows:
    for tok in re.findall(r'`([^`]+)`',meth):
        parts=tok.split()
        if tok.startswith('cargo test'):
            cands=[p for p in parts[2:] if not p.startswith('-') and p not in ('test',) and not re.match(r'^starfall',p)]
            cands=[c for c in cands if c!=parts[3]] if len(parts)>3 and parts[2]=='-p' else cands
        elif len(parts)==1 and '_' in tok and re.match(r'^[A-Za-z_][\w:*]*$',tok):
            cands=[tok]
        else: continue
        for c in cands:
            last=c.split('::')[-1].rstrip('*')
            if not last: continue
            hit=any(n.startswith(last) for n in names) if c.endswith('*') or True else last in names
            if not hit: print(f'SC-{sc:02d}\tGHOST?\t{c}\t[{tok}]')
