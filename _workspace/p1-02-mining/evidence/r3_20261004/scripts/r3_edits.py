import json,sys
src={'simulation.rs':'/c/WorkSpace/SpaceHistoric/server/crates/sim/src/simulation.rs','ws.rs':'/c/WorkSpace/SpaceHistoric/server/crates/gateway/src/ws.rs','runtime.rs':'','ws_integration.rs':''}
cut={'simulation.rs':1850,'ws.rs':786}
cur={k:open('C:/WorkSpace/SpaceHistoric/server/crates/'+p,encoding='utf-8').read() for k,p in [('simulation.rs','sim/src/simulation.rs'),('ws.rs','gateway/src/ws.rs')]}
for line in open(sys.argv[1],encoding='utf-8'):
    try: d=json.loads(line)
    except: continue
    ts=d.get('timestamp','')
    if ts<'2026-10-03T15:00': continue
    m=d.get('message',{})
    if not isinstance(m,dict): continue
    for c in m.get('content',[]) if isinstance(m.get('content'),list) else []:
        if c.get('type')!='tool_use': continue
        i=c.get('input',{}); n=c.get('name')
        fp=i.get('file_path','') or ''
        if n in('Edit','Write','MultiEdit') and any(fp.endswith(k) for k in src):
            k=[k for k in src if fp.endswith(k)][0]
            new=i.get('new_string', i.get('content',''))
            if k in cur:
                pos=cur[k].find(new) if new else -1
                ln=cur[k][:pos].count('\n')+1 if pos>=0 else None
                where = 'TEST' if ln and ln>cut[k] else ('NONTEST' if ln else 'notfound(reverted?)')
            else: ln=None; where='-'
            print(ts,n,k,'line',ln,where,repr((i.get('old_string') or '')[:60]))
        if n=='Bash' and any(k in (i.get('command') or '') for k in ['sed -i','simulation.rs','ws.rs']) and 'sed -i' in (i.get('command') or ''):
            print(ts,'BASH-SED',i['command'][:150])
