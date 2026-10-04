"""r3 고친 스캔: 계약 §1 방법 칸에서 (a) `cargo test -p <crate> [--플래그 [값]] <필터>` 와 (b) 맨 백틱 이름을 모두 줍는다.
cargo_sc_map.py 의 결함(맨 이름 누락, `--test X` 뒤 필터 누락)을 고친 판. 출력:
  - <out>/scan_filters.tsv : sc  crate  flags  filter  source(cargo|bare)
  - <out>/scan_bare.tsv    : sc  token  category(TEST_FN|NON_TEST_IDENT|GHOST)
GHOST = 코드(server·tools·client Tests/Scripts·tests·contracts) 어디에도 단어로 나오지 않는 맨 이름."""
import os, re, sys
sys.stdout.reconfigure(encoding='utf-8')
contract, out = sys.argv[1], sys.argv[2]
text = open(contract, encoding='utf-8').read().replace('\r', '')
code = []; rs_files = {}
NON_CODE = {'check_violation': 'PostgreSQL 오류 조건 이름(SQLSTATE 23514) — 테스트 이름 아님'}
for root in ['server', 'tools', 'client/Assets/_Project', 'tests', 'contracts']:
    for dp, dn, fn in os.walk(root):
        if any(x in dp for x in ('target', 'Library', '__pycache__', 'Generated')): continue
        for f in fn:
            if f.endswith(('.rs', '.cs', '.py', '.json')):
                src = open(os.path.join(dp, f), encoding='utf-8', errors='ignore').read(); code.append(src)
                if f.endswith('.rs'): rs_files[os.path.join(dp, f).replace(os.sep, '/')] = src
code = '\n'.join(code)
test_fns = set(re.findall(r'#\[(?:tokio::)?test[^\]]*\]\s*(?:#\[[^\]]*\]\s*)*(?:async\s+)?fn\s+(\w+)', code)) \
         | set(re.findall(r'\[Test\][^\n]*\n\s*public\s+(?:async\s+)?\w+\s+(\w+)', code))
sec = None; filters = []; bare = []
CARGO = re.compile(r'^cargo test -p ([\w-]+)((?:\s+--[\w-]+(?:\s+(?!-)[\w-]+)?)*)\s+(\S+)$')
for line in text.split('\n'):
    if line.startswith('## '): sec = line[3:]
    m = re.match(r'^\|\s*\*{0,2}SC-(\d+)', line)
    if not (m and sec and sec.startswith('1.')): continue
    sc = f'SC-{int(m.group(1)):02d}'
    meth = line.split('|')[3]
    crate = None
    for tok in re.findall(r'`([^`]+)`', meth):
        cm = CARGO.match(tok.strip())
        if cm:
            crate = cm.group(1); flags = cm.group(2).strip(); filt = cm.group(3)
            if filt.startswith('--'):  # 필터 없이 플래그만(예: --test mining_replay)
                flags = (flags + ' ' + filt).strip(); filt = ''
            if flags.endswith('--test') and filt: flags, filt = flags + ' ' + filt, ''
            filters.append((sc, crate, flags, filt, 'cargo')); continue
        if tok.startswith('cargo') or ' ' in tok.strip(): continue
        t = tok.strip()
        if not re.match(r'^[A-Za-z_][\w:]*\*?$', t) or '_' not in t: continue
        last = t.split('::')[-1].rstrip('*')
        hit = (lambda n: n.startswith(last)) if t.endswith('*') else (lambda n: n == last)
        if t in NON_CODE: cat = 'NON_CODE'
        elif any(hit(n) for n in test_fns): cat = 'TEST_FN'
        elif re.search(r'\b' + re.escape(last), code): cat = 'NON_TEST_IDENT'
        else: cat = 'GHOST'
        bare.append((sc, t, cat))
        if cat == 'TEST_FN':
            owner = None
            for path, src in rs_files.items():
                if re.search(r'fn\s+' + re.escape(last), src):
                    mm = re.match(r'server/(?:crates|bins)/([\w-]+)/', path)
                    owner = {'game-server': 'starfall-game-server'}.get(mm.group(1), 'starfall-' + mm.group(1)) if mm else None
                    break
            filters.append((sc, owner or 'UNITY', '', t, 'bare'))
os.makedirs(out, exist_ok=True)
with open(f'{out}/scan_filters.tsv', 'w', encoding='utf-8', newline='\n') as f:
    for r in filters: f.write('\t'.join(r) + '\n')
with open(f'{out}/scan_bare.tsv', 'w', encoding='utf-8', newline='\n') as f:
    for r in bare: f.write('\t'.join(r) + '\n')
g = [b for b in bare if b[2] == 'GHOST']
bt = [b for b in bare if b[2] == 'TEST_FN']
print(f'cargo_filters={sum(1 for r in filters if r[4]=="cargo")} bare_tokens={len(bare)} bare_test_fn={len(bt)} ghost={len(g)}')
for b in bt: print('BARE_TEST_FN', *b)
for b in g: print('GHOST', *b)
