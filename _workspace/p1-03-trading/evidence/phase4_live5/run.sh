set -a; . ./.env; set +a
E=_workspace/p1-03-trading/evidence/phase4_live5
python tests/e2e/new_world.py --tag p4-live5-load --evidence $E/world.json > $E/world.out 2>&1
W=$(python -c "import json;print(json.load(open('$E/world.json',encoding='utf-8'))['world_id'])"); echo $W > $E/world_id
rm -f $E/stop $E/ready
python tests/e2e/server_boot.py serve --world $W --stop-file $E/stop --ready-file $E/ready --log $E/server.log --max-seconds 3600 > $E/serve.out 2>&1 &
for i in $(seq 1 60); do [ -f $E/ready ] && break; sleep 1; done
STARFALL_SELL_LOAD_SECS=1200 timeout 3000 tools/bots/target/debug/bots.exe probe --case sell-load --deposit inner-belt-1 --label bot-500 --world $W --out $E/load.json > $E/load.stdout 2> $E/load.err
echo "load rc=$?" >> $E/done.txt
touch $E/stop; wait
echo DONE >> $E/done.txt
