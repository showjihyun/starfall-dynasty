#!/usr/bin/env bash
E="$1"
bash "$CLAUDE_JOB_DIR/tmp/r1_gates.sh" "$E"
bash "$CLAUDE_JOB_DIR/tmp/run_filters.sh" "$E/filters"
echo ALLDONE > "$E/all_done.txt"
