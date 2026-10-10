set -a; . ./.env; set +a
E=_workspace/p1-03-trading/evidence/phase4_live1
n=111
for spec in sell-dup-reconnect:inner-belt-2 sell-dup-cross-kind:inner-belt-3 cheat-sell-inject:inner-belt-1 cheat-sell-range:inner-belt-2 cheat-sell-fast:inner-belt-3 cheat-sell-overqty:inner-belt-1 cheat-sell-cooldown:inner-belt-2 cheat-sell-unknown:inner-belt-3; do
  c=${spec%%:*}; d=${spec##*:}
  timeout 900 tools/bots/target/debug/bots.exe probe --case $c --deposit $d --label bot-$n --out $E/$c.json > $E/$c.stdout 2> $E/$c.err
  echo "$c rc=$?" >> $E/cases_done.txt
  n=$((n+1))
done
echo DONE >> $E/cases_done.txt
