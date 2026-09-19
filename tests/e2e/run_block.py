"""스프린트 계약 §4 의 **실행 블록**을 순서대로 돌리고 증거를 한 곳에 모은다.

    python tests/e2e/run_block.py preflight
    python tests/e2e/run_block.py load        [--unity-corr <file>] [--duration 60]
    python tests/e2e/run_block.py durability  [--duration 150]
    python tests/e2e/run_block.py probes

증거는 `_workspace/p0-02-networking-spike/evidence/<block>/` 아래에 쌓인다. 각 단계는
`step-*.json` 과 원본 stdout 을 남기고, 마지막에 `manifest.json` 으로 요약한다.

**이 스크립트는 판정하지 않는다.** 판정은 QA 가 리포트에서 한다. 여기서 하는 일은
(1) 게이트 순서를 지키는 것, (2) 실행 중에만 볼 수 있는 것을 놓치지 않는 것,
(3) 증거를 다시 만들 수 있는 형태로 남기는 것뿐이다.
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import time
import urllib.error
import urllib.request
from datetime import datetime, timezone
from pathlib import Path

import db

REPO = db.REPO_ROOT
EVIDENCE_ROOT = REPO / "_workspace/p0-02-networking-spike/evidence"
BOTS_DIR = REPO / "tools/bots"
BOTS_EXE = BOTS_DIR / "target/debug/bots.exe"
E2E = Path(__file__).resolve().parent
STATS_URL = "http://127.0.0.1:8080/debug/stats"
HEALTH_URL = "http://127.0.0.1:8080/healthz"


def arg_dict(args) -> dict:
    """`vars(args)` 에는 `args.fn`(함수)이 들어 있어 JSON 직렬화가 터진다.
    라운드 1에서 실제로 manifest 쓰기가 실패했다 — 측정은 끝난 뒤였지만 증거 요약이 안 남았다."""
    return {k: v for k, v in vars(args).items() if isinstance(v, (str, int, float, bool, type(None)))}


def now_iso() -> str:
    return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def write(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


def http_json(url: str, timeout: float = 2.0):
    try:
        with urllib.request.urlopen(url, timeout=timeout) as r:  # noqa: S310
            return json.loads(r.read().decode("utf-8"))
    except (urllib.error.URLError, json.JSONDecodeError, TimeoutError):
        return None


def run_cmd(cmd: list[str], *, cwd: Path = REPO, timeout: int = 600) -> dict:
    started = time.perf_counter()
    try:
        proc = subprocess.run(
            cmd, cwd=cwd, capture_output=True, text=True, timeout=timeout,
            encoding="utf-8", errors="replace",
        )
        return {
            "cmd": " ".join(cmd), "exit": proc.returncode,
            "stdout": proc.stdout, "stderr": proc.stderr,
            "elapsed_s": round(time.perf_counter() - started, 2),
        }
    except FileNotFoundError as exc:
        return {"cmd": " ".join(cmd), "exit": 127, "stdout": "", "stderr": str(exc),
                "elapsed_s": 0.0}
    except subprocess.TimeoutExpired:
        return {"cmd": " ".join(cmd), "exit": 124, "stdout": "", "stderr": "timeout",
                "elapsed_s": round(time.perf_counter() - started, 2)}


def step(manifest: list, out: Path, name: str, result: dict) -> dict:
    result = {"step": name, "at": now_iso(), **result}
    write(out / f"step-{name}.json", json.dumps(result, indent=2, ensure_ascii=False))
    manifest.append({k: v for k, v in result.items() if k not in ("stdout", "stderr")})
    marker = "ok" if result.get("exit", 0) == 0 else f"exit={result.get('exit')}"
    print(f"  [{name}] {marker} ({result.get('elapsed_s', 0)}s)")
    return result


# --------------------------------------------------------------------------- preflight


def cmd_preflight(args: argparse.Namespace) -> int:
    out = EVIDENCE_ROOT / "preflight"
    manifest: list = []
    print("== preflight ==")

    step(manifest, out, "docker-ps", run_cmd(
        ["docker", "compose", "ps", "--format", "{{.Service}}\t{{.Status}}"], timeout=60))

    try:
        version = db.psql("select version();")
        tables = db.psql(
            "select string_agg(tablename, ',' order by tablename) "
            "from pg_tables where schemaname='public';")
        dbinfo = {"exit": 0, "postgres": version, "public_tables": tables,
                  "domain_events": db.table_exists("domain_events"),
                  "worlds": db.table_exists("worlds")}
    except (db.EnvironmentProblem, db.NotImplementedYet) as exc:
        dbinfo = {"exit": 2, "error": str(exc)}
    step(manifest, out, "db", dbinfo)

    health = http_json(HEALTH_URL)
    stats = http_json(STATS_URL)
    keys = sorted(stats.keys()) if isinstance(stats, dict) else None
    # server 정본은 최상위 키 31개다(03_server_impl.md §5). 수가 다르면 스크립트가 보는 모양이
    # 서버와 어긋난 것이니 대조부터 한다.
    step(manifest, out, "server", {
        "exit": 0 if health else 2,
        "healthz": health,
        "debug_stats_key_count": len(keys) if keys else None,
        "debug_stats_keys": keys,
        "expected_key_count": 31,
        "persist_backlog": (stats or {}).get("persist_backlog"),
        "persist_backlog_limit": (stats or {}).get("persist_backlog_limit"),
        "accepting_connections": (stats or {}).get("accepting_connections"),
        "note": "healthz 가 null 이면 서버가 떠 있지 않다 → 부하 블록은 E5(미검증(환경)) 또는 FAIL(구현 없음)",
    })

    step(manifest, out, "bots-binary", {
        "exit": 0 if BOTS_EXE.is_file() else 2,
        "path": str(BOTS_EXE),
        "hint": "없으면: cd tools/bots && cargo build",
    })
    step(manifest, out, "bots-selftest", run_cmd(
        ["cargo", "test", "--offline"], cwd=BOTS_DIR, timeout=900))
    step(manifest, out, "occurred-at-selftest", run_cmd(
        [sys.executable, str(E2E / "check_occurred_at.py"), "--selftest"], timeout=60))

    write(out / "manifest.json", json.dumps(
        {"block": "preflight", "at": now_iso(), "steps": manifest}, indent=2, ensure_ascii=False))
    print(f"evidence -> {out}")
    return db.EXIT_OK


# --------------------------------------------------------------------------- load (블록 5)


def wait_for_lines(path: Path, want: int, timeout: float) -> list[str]:
    deadline = time.perf_counter() + timeout
    lines: list[str] = []
    while time.perf_counter() < deadline:
        if path.is_file():
            lines = [x.strip() for x in path.read_text(encoding="utf-8").splitlines() if x.strip()]
            if len(lines) >= want:
                return lines
        time.sleep(0.2)
    return lines


def merged_corr_file(out: Path, live: Path, unity: str | None) -> Path:
    """봇 집합 + (있으면) Unity 의 correlation 을 합친 파일. §0.6 의 31 이 여기서 나온다."""
    values = []
    if live.is_file():
        values += [x.strip() for x in live.read_text(encoding="utf-8").splitlines() if x.strip()]
    if unity:
        p = Path(unity)
        if p.is_file():
            values += [x.strip() for x in p.read_text(encoding="utf-8").splitlines() if x.strip()]
    merged = out / "correlations.merged.txt"
    seen, ordered = set(), []
    for v in values:
        if v not in seen:
            seen.add(v)
            ordered.append(v)
    write(merged, "\n".join(ordered) + ("\n" if ordered else ""))
    return merged


def cmd_load(args: argparse.Namespace) -> int:
    out = EVIDENCE_ROOT / "load"
    manifest: list = []
    print("== 블록 5: 부하 A → B → C ==")
    if not BOTS_EXE.is_file():
        print("bots 바이너리가 없다. cd tools/bots && cargo build", file=sys.stderr)
        return db.EXIT_ENV

    stats_before = http_json(STATS_URL)
    write(out / "stats-before.json", json.dumps(stats_before, indent=2, ensure_ascii=False))
    step(manifest, out, "stats-before", {"exit": 0 if stats_before else 2})

    # --- A 단계 -------------------------------------------------------------
    a_out = out / "a"
    live = a_out / "correlations.live.txt"
    a_cmd = [
        str(BOTS_EXE), "run", "--scenario", "a",
        "--bots", str(args.bots), "--seed", str(args.seed),
        "--duration", str(args.duration), "--ramp", str(args.ramp),
        "--interval", str(args.interval), "--url", args.url,
        "--out", str(a_out), "--live-corr", str(live),
    ]
    print(f"  A 단계 시작 ({args.duration}s + ramp {args.ramp}s) …")
    a_started = time.perf_counter()
    a_proc = subprocess.Popen(
        a_cmd, cwd=REPO, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
        text=True, encoding="utf-8", errors="replace")

    # SC-61: **A 가 도는 동안**에만 잴 수 있다. 끝나면 증명할 수 없다.
    expected_live = args.bots
    lines = wait_for_lines(live, expected_live, timeout=args.ramp + 20)
    mid_corr = merged_corr_file(out / "mid", live, args.unity_corr)
    time.sleep(max(0.0, args.mid_at - (time.perf_counter() - a_started)))
    mid = run_cmd([sys.executable, str(E2E / "check_in_flight.py"),
                   "--corr", str(mid_corr),
                   "--evidence", str(out / "step-SC-61.json")], timeout=120)
    mid["live_correlations_at_query"] = len(lines)
    mid["a_running"] = a_proc.poll() is None
    step(manifest, out, "SC-61-in-flight-visibility", mid)

    a_stdout, _ = a_proc.communicate(timeout=args.duration + args.ramp + 180)
    write(a_out / "stdout.txt", a_stdout or "")
    step(manifest, out, "A-bots-run", {
        "exit": a_proc.returncode, "cmd": " ".join(a_cmd),
        "elapsed_s": round(time.perf_counter() - a_started, 2), "stdout": a_stdout})

    # SC-62: 마지막 봇이 끊긴 **직후**부터 잰다. 여기서 다른 일을 먼저 하면 측정이 망가진다.
    # SC-62 의 기대 행 수는 **봇 세션만** 2배로 잡는다. Unity hold 세션은 이 시점에 아직
    # 열려 있어 SESSION_CLOSED 가 존재할 수 없다 — 31×2=62 를 기다리면 영영 도달하지 않는다
    # (라운드 1에서 실제로 15초 타임아웃 후 거짓 FAIL 이 나왔다).
    bot_corr = a_out / "correlations.txt"
    a_corr = merged_corr_file(out / "a-merged", bot_corr, args.unity_corr)
    n_bots = len([x for x in bot_corr.read_text(encoding="utf-8").splitlines() if x.strip()])
    n_unity = len([x for x in a_corr.read_text(encoding="utf-8").splitlines() if x.strip()]) - n_bots
    expect_rows = n_bots * 2 + n_unity  # 봇은 OPENED+CLOSED, 아직 열린 Unity 는 OPENED 만
    step(manifest, out, "SC-62-visibility-deadline", run_cmd(
        [sys.executable, str(E2E / "poll_until.py"), "rows", "--corr", str(a_corr),
         "--expect", str(expect_rows), "--evidence", str(out / "step-SC-62.json")], timeout=120))

    step(manifest, out, "SC-56-three-way", run_cmd(
        [sys.executable, str(E2E / "three_way.py"), "--bots-out", str(a_out),
         "--stats-before", str(out / "stats-before.json"),
         "--evidence", str(out / "step-SC-56.json")], timeout=180))

    # --- B 단계 -------------------------------------------------------------
    b_out = out / "b"
    print("  B 단계 (회전) …")
    step(manifest, out, "B-bots-run", run_cmd(
        [str(BOTS_EXE), "run", "--scenario", "b", "--bots", str(args.bots),
         "--seed", str(args.seed), "--cycles", str(args.cycles), "--pings", str(args.pings),
         "--url", args.url, "--out", str(b_out)], timeout=900))

    # A+B 합집합으로 기록 무결성 (SC-57/58/59/60)
    ab = out / "ab"
    ab_values = []
    for f in (a_out / "correlations.txt", b_out / "correlations.txt"):
        if f.is_file():
            ab_values += [x.strip() for x in f.read_text(encoding="utf-8").splitlines() if x.strip()]
    if args.unity_corr and Path(args.unity_corr).is_file():
        ab_values += [x.strip() for x in Path(args.unity_corr).read_text(encoding="utf-8").splitlines() if x.strip()]
    ab_file = ab / "correlations.txt"
    write(ab_file, "\n".join(dict.fromkeys(ab_values)) + "\n")

    # 주의: Unity hold 세션이 아직 열려 있으면 그 correlation 은 CLOSED 가 없어 FAIL 로 보인다.
    # 그 경우 Unity 를 RELEASE 한 뒤 `check_sessions.py` 를 다시 돌려 판정한다(라운드 1에서 그렇게 했다).
    step(manifest, out, "SC-57-58-session-pairs", run_cmd(
        [sys.executable, str(E2E / "check_sessions.py"), "--corr", str(ab_file),
         "--evidence", str(out / "step-SC-57-58.json")], timeout=180))
    step(manifest, out, "SC-59-sequence-gaps", run_cmd(
        [sys.executable, str(E2E / "check_sequence_gaps.py"),
         "--evidence", str(out / "step-SC-59.json")], timeout=180))
    step(manifest, out, "SC-60-occurred-at", run_cmd(
        [sys.executable, str(E2E / "check_occurred_at.py"), "--corr", str(ab_file),
         "--sample", "10", "--evidence", str(out / "step-SC-60.json")], timeout=180))

    # --- C 단계 -------------------------------------------------------------
    c_out = out / "c"
    print("  C 단계 (백프레셔) …")
    step(manifest, out, "C-bots-run", run_cmd(
        [str(BOTS_EXE), "run", "--scenario", "c", "--bots", str(args.bots),
         "--seed", str(args.seed), "--duration", str(args.duration),
         "--ramp", str(args.ramp), "--interval", str(args.interval),
         "--burst", str(args.burst), "--url", args.url, "--out", str(c_out)],
        timeout=args.duration + 300))

    stats_after = http_json(STATS_URL)
    write(out / "stats-after.json", json.dumps(stats_after, indent=2, ensure_ascii=False))
    step(manifest, out, "stats-after", {"exit": 0 if stats_after else 2})

    write(out / "manifest.json", json.dumps({
        "block": "load", "at": now_iso(),
        "config": arg_dict(args),
        "note": "SC-63~65(백프레셔)는 a/summary.json 과 c/summary.json 의 p99 를 비교해 QA 가 판정한다",
        "steps": manifest,
    }, indent=2, ensure_ascii=False))
    print(f"evidence -> {out}")
    return db.EXIT_OK


# --------------------------------------------------------------------------- durability (블록 6)


def cmd_durability(args: argparse.Namespace) -> int:
    out = EVIDENCE_ROOT / "durability"
    manifest: list = []
    print("== 블록 6: D 기록 내구성 (AC-19) ==")
    if not BOTS_EXE.is_file():
        print("bots 바이너리가 없다. cd tools/bots && cargo build", file=sys.stderr)
        return db.EXIT_ENV

    d_out = out / "d"
    live = d_out / "correlations.live.txt"
    d_cmd = [
        str(BOTS_EXE), "run", "--scenario", "d", "--bots", str(args.bots),
        "--seed", str(args.seed), "--duration", str(args.duration),
        "--ramp", str(args.ramp), "--interval", str(args.interval),
        "--url", args.url, "--out", str(d_out), "--live-corr", str(live),
    ]
    print(f"  D 부하 시작 ({args.duration}s) …")
    started = time.perf_counter()
    proc = subprocess.Popen(d_cmd, cwd=REPO, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                            text=True, encoding="utf-8", errors="replace")
    wait_for_lines(live, args.bots, timeout=args.ramp + 20)
    time.sleep(args.settle)

    outage_started = now_iso()
    step(manifest, out, "stop-postgres", run_cmd(
        ["docker", "compose", "stop", "postgres"], timeout=120))

    # SC-66: 시간이 아니라 **상태**로 기다린다. 600 tick = 20 Hz × 30초 (ADR-0007 §4).
    step(manifest, out, "SC-66-wait-backlog", run_cmd(
        [sys.executable, str(E2E / "poll_until.py"), "backlog", "--above", "600",
         "--timeout", "120", "--evidence", str(out / "step-SC-66-backlog.json")], timeout=180))

    # SC-66: 그 상태에서 새 연결이 503 recording_backlog 인가.
    step(manifest, out, "SC-66-new-connection-503", run_cmd(
        ["curl.exe", "-s", "-o", str(out / "ws-503-body.txt"),
         "-w", "http_code=%{http_code}\\n",
         "-H", "Connection: Upgrade", "-H", "Upgrade: websocket",
         "-H", "Sec-WebSocket-Version: 13",
         "-H", "Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==",
         "-H", f"Authorization: Bearer {args.token}" if args.token else "X-No-Token: 1",
         args.ws_http_url], timeout=60))

    stats_during = http_json(STATS_URL)
    write(out / "stats-during-outage.json", json.dumps(stats_during, indent=2, ensure_ascii=False))
    step(manifest, out, "SC-67-stats-during-outage", {
        "exit": 0 if stats_during else 2,
        "ws_connections": (stats_during or {}).get("ws_connections"),
        "note": "기존 세션이 유지되는가 — 봇 쪽 증거는 commands.csv 의 중단 구간 왕복이다",
    })

    step(manifest, out, "start-postgres", run_cmd(
        ["docker", "compose", "start", "postgres"], timeout=180))
    outage_ended = now_iso()
    step(manifest, out, "pg-ready", run_cmd(
        ["docker", "compose", "exec", "-T", "postgres",
         "pg_isready", "-U", "starfall", "-d", "starfall", "-t", "60"], timeout=120))

    # SC-68: 백로그가 **정상 대역(0~20)** 으로 돌아가는가. 0 이 아닌 것이 이상이 아니다(server §5).
    step(manifest, out, "SC-68-backlog-drained", run_cmd(
        [sys.executable, str(E2E / "poll_until.py"), "backlog", "--target", "20",
         "--timeout", "180", "--evidence", str(out / "step-SC-68.json")], timeout=240))

    stdout, _ = proc.communicate(timeout=args.duration + 300)
    write(d_out / "stdout.txt", stdout or "")
    step(manifest, out, "D-bots-run", {
        "exit": proc.returncode, "cmd": " ".join(d_cmd),
        "elapsed_s": round(time.perf_counter() - started, 2), "stdout": stdout})

    # SC-69: 중단 구간 이벤트가 한 건도 유실되지 않았는가
    corr = d_out / "correlations.txt"
    step(manifest, out, "SC-69-no-loss", run_cmd(
        [sys.executable, str(E2E / "check_sessions.py"), "--corr", str(corr),
         "--evidence", str(out / "step-SC-69.json")], timeout=180))

    write(out / "manifest.json", json.dumps({
        "block": "durability", "at": now_iso(),
        "outage_started": outage_started, "outage_ended": outage_ended,
        "config": arg_dict(args),
        "note": (
            "SC-67 의 봇 쪽 증거: d/commands.csv 에서 중단 구간(위 두 시각)에 sent 된 명령의 "
            "왕복 성공 건수 > 0. 봇의 *_us 는 summary.json 의 clock_base_unix_ms 를 더해 벽시계로 옮긴다."
        ),
        "steps": manifest,
    }, indent=2, ensure_ascii=False))
    print(f"evidence -> {out}")
    return db.EXIT_OK


# --------------------------------------------------------------------------- probes


PROBE_CASES = ["auth-ok", "order", "duplicate", "inflight", "oversize", "binary",
               "slow-consumer", "idle"]


def cmd_probes(args: argparse.Namespace) -> int:
    out = EVIDENCE_ROOT / "probes"
    manifest: list = []
    print("== 서버 단독 항목 probe (SC-14·19·20·21·22·24·25·26) ==")
    if not BOTS_EXE.is_file():
        print("bots 바이너리가 없다. cd tools/bots && cargo build", file=sys.stderr)
        return db.EXIT_ENV
    cases = args.cases.split(",") if args.cases else PROBE_CASES
    # 신원은 server 정본의 `bot-NNN` 을 쓴다(§3.3). case 마다 다른 봇을 써 세션이 섞이지 않게 한다.
    for idx, case in enumerate(cases):
        label = f"bot-{idx:03d}"
        print(f"  probe {case} ({label}) …")
        r = run_cmd([str(BOTS_EXE), "probe", "--case", case, "--url", args.url,
                     "--label", label], timeout=180)
        write(out / f"probe-{case}.txt", (r.get("stdout") or "") + (r.get("stderr") or ""))
        step(manifest, out, f"probe-{case}", r)
    write(out / "manifest.json", json.dumps(
        {"block": "probes", "at": now_iso(), "cases": cases, "steps": manifest},
        indent=2, ensure_ascii=False))
    print(f"evidence -> {out}")
    return db.EXIT_OK


def main() -> int:
    ap = argparse.ArgumentParser(description="p0-02 검증 블록 러너")
    sub = ap.add_subparsers(dest="block", required=True)

    pf = sub.add_parser("preflight", help="환경·도구 점검 (E-코드 판단용)")
    pf.set_defaults(fn=cmd_preflight)

    for name, fn in (("load", cmd_load), ("durability", cmd_durability)):
        p = sub.add_parser(name)
        p.add_argument("--url", default="ws://127.0.0.1:8080/ws")
        p.add_argument("--bots", type=int, default=30)
        p.add_argument("--seed", type=int, default=42)
        p.add_argument("--duration", type=int, default=60 if name == "load" else 150)
        p.add_argument("--ramp", type=int, default=5)
        p.add_argument("--interval", type=int, default=500)
        p.add_argument("--unity-corr", help="Unity 세션의 correlation_id 파일 (§0.6 의 31번째)")
        p.set_defaults(fn=fn)
        if name == "load":
            p.add_argument("--cycles", type=int, default=5)
            p.add_argument("--pings", type=int, default=3)
            p.add_argument("--burst", type=int, default=2000)
            p.add_argument("--mid-at", type=float, default=25.0,
                           help="A 시작 후 몇 초에 SC-61 을 조회할지")
        else:
            p.add_argument("--settle", type=float, default=20.0,
                           help="중단 전에 정상 상태를 유지할 시간")
            p.add_argument("--token", help="503 확인용 유효 개발 토큰 (bots token 출력)")
            p.add_argument("--ws-http-url", default="http://127.0.0.1:8080/ws")

    pr = sub.add_parser("probes")
    pr.add_argument("--url", default="ws://127.0.0.1:8080/ws")
    pr.add_argument("--cases", help="쉼표 목록 (기본: 전부)")
    pr.set_defaults(fn=cmd_probes)

    args = ap.parse_args()
    return db.main_guard(lambda: args.fn(args))


if __name__ == "__main__":
    sys.exit(main())
