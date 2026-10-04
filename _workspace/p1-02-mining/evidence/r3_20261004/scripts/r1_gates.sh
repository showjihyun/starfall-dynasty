#!/usr/bin/env bash
# Phase 5 r1 — 서버 게이트(CI 와 같은 명령) + DB 테스트 이름 대조 + 반복 10.
export PATH="$HOME/.cargo/bin:$PATH"
R="$1"
cd /c/WorkSpace/SpaceHistoric/server
set -a; . ../.env; set +a
export STARFALL_DB_TESTS=required
{
  echo "== fmt";    cargo fmt --all --check > "$R/fmt.log" 2>&1; echo "fmt rc=$?"
  echo "== clippy"; cargo clippy --workspace --all-targets -- -D warnings > "$R/clippy.log" 2>&1; echo "clippy rc=$?"
  echo "== test";   cargo test --workspace --locked --no-fail-fast > "$R/test.log" 2>&1; echo "test rc=$?"
  echo "== release sim"; cargo test -p starfall-sim --release --locked > "$R/release.log" 2>&1; echo "release rc=$?"
} > "$R/gates_summary.txt"
cd ..
PYTHONIOENCODING=utf-8 python tests/e2e/db_test_census.py --contract _workspace/p1-02-mining/02_sprint_contract.md \
  --log "$R/test.log" > "$R/census.json" 2>&1; echo "census rc=$?" >> "$R/gates_summary.txt"
bash "$CLAUDE_JOB_DIR/tmp/flaky2.sh" "$R/repeat"
echo DONE >> "$R/gates_summary.txt"
