#!/usr/bin/env bash
export PATH="$HOME/.cargo/bin:$PATH"
cd /c/WorkSpace/SpaceHistoric/server
set -a; . ../.env; set +a
export STARFALL_DB_TESTS=required
OUT="$1"; mkdir -p "$OUT/logs"; : > "$OUT/filters_summary.tsv"
while IFS=$'\t' read -r sc crate filt; do
  f="${filt%%\**}"
  if [ "$sc" = "SC-33" ]; then args=(--test mining_replay); f=""; else args=(); fi
  log="$OUT/logs/${sc}_${crate}_$(echo "$f" | tr ':/' '__').log"
  cargo test -p "$crate" --locked "${args[@]}" -- $f > "$log" 2>&1; rc=$?
  ran=$(grep -E "^test .* \.\.\. (ok|FAILED)" "$log" | wc -l)
  passed=$(grep "^test result" "$log" | awk '{p+=$4} END{print p+0}')
  failed=$(grep "^test result" "$log" | awk '{p+=$6} END{print p+0}')
  names=$(grep -E "^test .* \.\.\. (ok|FAILED)" "$log" | sed -E 's/^test (\S+).*/\1/' | tr '\n' ' ')
  printf "%s\t%s\t%s\trc=%s\tran=%s\tpassed=%s\tfailed=%s\t%s\n" "$sc" "$crate" "$filt" "$rc" "$ran" "$passed" "$failed" "$names" >> "$OUT/filters_summary.tsv"
done < "$CLAUDE_JOB_DIR/tmp/filters.tsv"
echo DONE >> "$OUT/filters_summary.tsv"
