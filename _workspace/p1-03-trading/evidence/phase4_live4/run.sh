E=_workspace/p1-03-trading/evidence/phase4_live4
for s in sell-dup sell-cas sell-cas-market sell-recover; do
  timeout 2400 python tests/e2e/restart_cases.py --out $E/$s $s > $E/$s.out 2>&1
  echo "$s rc=$?" >> $E/done.txt
done
echo DONE >> $E/done.txt
