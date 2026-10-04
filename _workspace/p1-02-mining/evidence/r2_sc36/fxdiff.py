import json,os,sys
def flat(o,p=""):
    out={}
    if isinstance(o,dict):
        for k,v in o.items(): out.update(flat(v,f"{p}.{k}" if p else k))
    elif isinstance(o,list):
        out[p+"#len"]=len(o)
        for i,v in enumerate(o): out.update(flat(v,f"{p}[{i}]"))
    else: out[p]=o
    return out
new=sys.argv[1:]
for t in ["MINE_RESOURCE","MINERAL_MINED","MINERAL_DISCOVERED","INVENTORY_STATE","DEPOSIT_FIELD_STATE","HISTORICAL_EVENT_NOTICE","MINERAL","DEPOSIT_FIELD","MINING_RULES","SIGNIFICANCE_RULE"]:
    valid=[f for f in sorted(os.listdir(t)) if f.endswith(".json")]
    vs={f:flat(json.load(open(f"{t}/{f}",encoding="utf-8"))) for f in valid}
    for f in sorted(os.listdir(f"{t}/invalid")):
        inv=flat(json.load(open(f"{t}/invalid/{f}",encoding="utf-8")))
        best=None
        for vf,v in vs.items():
            keys=set(v)|set(inv)
            d=[k for k in keys if v.get(k,"<absent>")!=inv.get(k,"<absent>")]
            if best is None or len(d)<len(best[1]): best=(vf,d)
        vf,d=best
        desc="; ".join(f"{k}: {vs[vf].get(k,'<absent>')!r}->{inv.get(k,'<absent>')!r}" for k in sorted(d)[:4])
        print(f"{t}\t{f}\tvs {vf}\t{len(d)}\t{desc[:160]}")
