#!/usr/bin/env bash
# ws_integration 비결정성 재현 — --workspace 10 회. 회차마다 그 시간 창에 바뀐 server/ 소스를 기록해
# 편집과 겹친 회차를 판정에서 뺀다(첫 시도에서 7 회가 편집과 겹쳐 무효였다).
export PATH="$HOME/.cargo/bin:$PATH"
cd /c/WorkSpace/SpaceHistoric/server
set -a; . ../.env; set +a
export STARFALL_DB_TESTS=required
D="$1"; mkdir -p "$D"; : > "$D/summary.txt"
git -C .. rev-parse HEAD > "$D/head.txt"
for i in $(seq 1 10); do
  t0=$(date '+%Y-%m-%d %H:%M:%S')
  cargo test --workspace --locked --no-fail-fast > "$D/run$i.log" 2>&1
  rc=$?
  edits=$(find . -path ./target -prune -o -type f \( -name '*.rs' -o -name '*.toml' -o -name '*.sql' \) -newermt "$t0" -print | tr '\n' ' ')
  p=$(grep "^test result" "$D/run$i.log" | awk '{p+=$4} END{print p+0}')
  f=$(grep -E '^test .* FAILED$' "$D/run$i.log" | sed 's/^test //;s/ \.\.\. FAILED//' | tr '\n' ' ')
  ce=$(grep -c '^error\[' "$D/run$i.log")
  echo "run$i start=$t0 rc=$rc passed=$p compile_errors=$ce failed=[$f] edits_during=[$edits]" >> "$D/summary.txt"
done
echo DONE >> "$D/summary.txt"
