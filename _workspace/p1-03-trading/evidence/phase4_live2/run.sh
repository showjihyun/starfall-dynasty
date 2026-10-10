set -a; . ./.env; set +a
E=_workspace/p1-03-trading/evidence/phase4_live2
W=$(cat $E/world_id)
timeout 1500 tools/bots/target/debug/bots.exe probe --case trade-cycle --deposit inner-belt-1 --label bot-100 --label-b bot-101 --label-c bot-102 --world $W --out $E/trade-cycle.json > $E/trade-cycle.stdout 2> $E/trade-cycle.err
echo "rc=$?" > $E/done.txt
