set -a; . ./.env; set +a
E=_workspace/p1-03-trading/evidence/phase4_live3/rerun
B=tools/bots/target/debug/bots.exe
group() { # name cmds...
  g=$1; shift
  mkdir -p $E/$g
  python tests/e2e/new_world.py --tag p4-live3b-$g --evidence $E/$g/world.json > $E/$g/world.out 2>&1
  W=$(python -c "import json;print(json.load(open('$E/$g/world.json',encoding='utf-8'))['world_id'])")
  echo $W > $E/$g/world_id
  rm -f $E/$g/stop $E/$g/ready
  python tests/e2e/server_boot.py serve --world $W --stop-file $E/$g/stop --ready-file $E/$g/ready --log $E/$g/server.log --max-seconds 3000 > $E/$g/serve.out 2>&1 &
  for i in $(seq 1 60); do [ -f $E/$g/ready ] && break; sleep 1; done
  for c in "$@"; do eval "$c"; echo "$g: rc=$?" >> $E/done.txt; done
  touch $E/$g/stop; wait
}
group leak 'timeout 1200 $B probe --case leak-scan-market --deposit inner-belt-1 --label bot-120 --label-b bot-121 --world $W --out $E/leak/leak.json > $E/leak/leak.stdout 2> $E/leak/leak.err'
#group trace 'timeout 1200 $B probe --case trace-sell-a --deposit inner-belt-1 --label bot-130 --world $W --out $E/trace/a.json > $E/trace/a.stdout 2> $E/trace/a.err' 'timeout 600 $B probe --case trace-sell-b --deposit inner-belt-2 --label bot-131 --seller-label bot-130 --world $W --out $E/trace/b.json > $E/trace/b.stdout 2> $E/trace/b.err'
#group race 'timeout 900 $B probe --case race-sell-same-tick --deposit inner-belt-1 --label bot-140 --label-b bot-141 --world $W --out $E/race/race.json > $E/race/race.stdout 2> $E/race/race.err'
echo DONE >> $E/done.txt
