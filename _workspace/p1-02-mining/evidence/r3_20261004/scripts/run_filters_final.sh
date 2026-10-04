#!/usr/bin/env bash
# 고친 스캔(scan_filters.tsv)의 필터를 계약 명령 그대로 실행한다: `*` 앞까지(cargo 는 글롭이 없다), 플래그는 그대로.
# 입력의 CR 은 지우고, 빈 칸은 '.' 로 채운다(IFS 탭이 연속 탭을 접어 열이 밀린다 — 1차 실행에서 겪음).
export PATH="$HOME/.cargo/bin:$PATH"
IN="$1"; OUT="$2"; mkdir -p "$OUT/logs"; : > "$OUT/filters_summary.tsv"
cd /c/WorkSpace/SpaceHistoric/server
set -a; . ../.env; set +a
export STARFALL_DB_TESTS=required
tr -d '\r' < "$IN" | awk -F'\t' -v OFS='\t' '{for(i=1;i<=5;i++) if($i=="") $i="."; print}' |
while IFS=$'\t' read -r sc crate flags filt src; do
  [ "$crate" = "UNITY" ] && continue
  [ "$flags" = "." ] && flags=""; [ "$filt" = "." ] && filt=""
  f="${filt%%\**}"
  log="$OUT/logs/${sc}_${crate}_$(echo "${flags}_$f" | tr ':/ ' '___').log"
  cargo test -p "$crate" --locked $flags -- $f > "$log" 2>&1 < /dev/null; rc=$?
  ran=$(grep -E "^test .* \.\.\. (ok|FAILED)" "$log" | wc -l)
  failed=$(grep "^test result" "$log" | awk '{p+=$6} END{print p+0}')
  names=$(grep -E "^test .* \.\.\. (ok|FAILED)" "$log" | sed -E 's/^test (\S+).*/\1/' | head -20 | tr '\n' ' ')
  printf "%s\t%s\t%s\t%s\t%s\trc=%s\tran=%s\tfailed=%s\t%s\n" "$sc" "$crate" "${flags:-.}" "${filt:-.}" "$src" "$rc" "$ran" "$failed" "$names" >> "$OUT/filters_summary.tsv"
done
echo DONE >> "$OUT/filters_summary.tsv"
