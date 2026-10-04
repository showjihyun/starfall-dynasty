import hashlib, shutil, subprocess, sys, os, json
R='C:/WorkSpace/SpaceHistoric/server/'
OUT='C:/WorkSpace/SpaceHistoric/_workspace/p1-02-mining/evidence/r3_20261004/mutations/'
BK='C:/Users/CHOISOOYEON/.claude/jobs/0063ccb8/tmp/mut_backup/'
os.makedirs(BK, exist_ok=True)
env=dict(os.environ); env['PATH']=os.path.expanduser('~/.cargo/bin')+os.pathsep+env['PATH']
def sha(p): return hashlib.sha256(open(p,'rb').read()).hexdigest()
M=[
 ('MA_threshold_plus1','crates/gateway/src/ws.rs',
  b'stats.recording_lag() > RECORDING_BACKLOG_LIMIT)', b'stats.recording_lag() > RECORDING_BACKLOG_LIMIT + 1)',
  ['-p','starfall-gateway','--lib','--','recording_lag_boundary']),
 ('MB_gate_all_commands','crates/gateway/src/ws.rs',
  b'if matches!(command, InboundCommand::MineResource(_))\r\n        && (stats.persist_halted()',
  b'if (true || matches!(command, InboundCommand::MineResource(_)))\r\n        && (stats.persist_halted()',
  ['-p','starfall-gateway','--lib','--','recording_lag_boundary']),
 ('MC_depletion_dead','crates/sim/src/simulation.rs',
  b'if remaining_before <= 0 {', b'if false && remaining_before <= 0 {',
  ['-p','starfall-sim','--lib','--','mining::reject_order','mining::each_rejection_reason']),
 ('MD_capacity_writes_deposit','crates/sim/src/simulation.rs',
  b'        else {\r\n            reject(outcome, ids, RejectReasonCode::CapacityExceeded);',
  b'        else {\r\n            self.deposit_states.insert(deposit_id.clone(), DepositRuntimeState { remaining_kg: remaining_before, as_of_tick: tick_number, first_extracted_tick: existing_first_extracted_tick.unwrap_or(tick_number) });\r\n            reject(outcome, ids, RejectReasonCode::CapacityExceeded);',
  ['-p','starfall-sim','--lib','--','mining::each_rejection_reason']),
]
sel=sys.argv[1:] or [m[0] for m in M]
summary=[]
for name,f,old,new,args in M:
    if name not in sel: continue
    p=R+f; before=sha(p); data=open(p,'rb').read()
    if b'\r\n' not in data[:2000]:
        old=old.replace(b'\r\n',b'\n'); new=new.replace(b'\r\n',b'\n')
    n=data.count(old)
    if n!=1: summary.append(f'{name}\tSKIP occurrences={n}'); continue
    shutil.copy2(p,BK+name+'.orig')
    try:
        open(p,'wb').write(data.replace(old,new))
        r=subprocess.run(['cargo','test','--locked']+args,cwd=R,env=env,capture_output=True)
        log=r.stdout.decode('utf-8','replace')+r.stderr.decode('utf-8','replace')
        open(OUT+name+'.log','w',encoding='utf-8').write(log)
    finally:
        open(p,'wb').write(data)
    after=sha(p)
    failed=[l for l in log.splitlines() if l.endswith('FAILED') and l.startswith('test ')]
    summary.append(f'{name}\t{f}\trc={r.returncode}\tfailed={failed}\tsha_before={before}\tsha_after={after}\trestored={before==after}')
    # green after restore
    r2=subprocess.run(['cargo','test','--locked']+args,cwd=R,env=env,capture_output=True)
    log2=r2.stdout.decode('utf-8','replace')+r2.stderr.decode('utf-8','replace')
    open(OUT+name+'.restored.log','w',encoding='utf-8').write(log2)
    res=[l for l in log2.splitlines() if l.startswith('test result')]
    summary.append(f'{name}\tRESTORED rc={r2.returncode}\t{res}')
open(OUT+'summary.txt','a',encoding='utf-8').write('\n'.join(summary)+'\n')
print('\n'.join(summary))
