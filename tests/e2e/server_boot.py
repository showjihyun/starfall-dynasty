#!/usr/bin/env python
"""SC-05 / SC-06 — 서버 기동 경로를 밖에서 몬다.

두 가지 일만 한다.

* `stats` : 실제 `data/` 로 서버를 띄우고 `/debug/stats` 를 받아 적은 뒤 **stdin `shutdown`** 으로
  정상 종료시킨다(하드 킬 금지 — 계약 §0.1). 종료 코드와 왕복 로그를 그대로 남긴다.
* `reject`: `STARFALL_DATA_DIR` 로 **임시 사본**을 가리키고 한 파일만 어긴 뒤 **종료 코드 1** 과
  `기동 거부: 데이터 검증 실패 file=… field=… expected=… actual=… rule=…` 로그를 확인한다.
  **레포의 `data/` 원본은 절대 건드리지 않는다**(designer 소유).

왜 파이썬인가: 셸 인용 규칙 차이로 같은 명령이 다르게 깨지지 않게 하려는 것이고, 서버의 stdin 을
잡고 있어야 정상 종료를 시킬 수 있기 때문이다(`tests/e2e/README.md` 의 대기·셸 절과 같은 이유).

종료 코드: 0 = 기대대로, 1 = 기대와 다름, 2 = 서버 바이너리·DB 에 닿지 못함.
"""
from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
import threading
import time
import urllib.error
import urllib.request
from pathlib import Path

for _stream in (sys.stdout, sys.stderr):
    try:
        _stream.reconfigure(encoding="utf-8")  # type: ignore[union-attr]
    except (AttributeError, ValueError):
        pass

REPO = Path(__file__).resolve().parents[2]
EXE = REPO / "server" / "target" / "debug" / "starfall-game-server.exe"


def load_dotenv() -> dict[str, str]:
    env = dict(os.environ)
    dotenv = REPO / ".env"
    if dotenv.is_file():
        for line in dotenv.read_text(encoding="utf-8").splitlines():
            line = line.strip()
            if not line or line.startswith("#") or "=" not in line:
                continue
            k, v = line.split("=", 1)
            env.setdefault(k.strip(), v.strip())
    env.setdefault("STARFALL_DEV_AUTH_SECRET", "dev_only_not_a_secret")
    return env


def http_json(url: str, timeout: float = 3.0):
    with urllib.request.urlopen(url, timeout=timeout) as r:
        return json.loads(r.read().decode("utf-8"))


def spawn(env: dict[str, str]) -> tuple[subprocess.Popen, list[str]]:
    """서버를 띄우고 **stdout 을 즉시 드레인하는 스레드**를 함께 건다.

    **왜 드레인 스레드가 필요한가 — 이 함수가 라운드 2 에서 서버를 세웠다.**
    `stdout=subprocess.PIPE` 로 띄워 놓고 프로세스가 끝난 뒤에야 `read()` 하면,
    실행 중에는 아무도 파이프를 비우지 않는다. Python `subprocess` 의 익명 파이프 용량은
    **4096 B** 이고, 서버의 `tracing` 기본 writer 는 **이벤트를 낸 스레드에서 동기로** 쓴다.
    그래서 로그가 4 KiB 를 넘는 순간 그 줄을 쓰려던 tokio 워커가 블록되고, 워커가 하나씩
    잠기다가 서버 전체가 무응답이 됐다(라운드 2 §3.1·§4.1 — 멈춘 두 인스턴스의 로그가
    각각 **4033 B / 4100 B** 였다).

    서버는 이제 비블로킹 싱크를 쓰므로 멈추지 않지만, **파이프가 차면 로그 줄을 버린다.**
    드레인하지 않으면 라운드 3 의 실서버 증거가 **4 KiB 에서 조용히 잘린다** — 그래서 여기서 비운다.

    반환하는 리스트는 **살아 있는 버퍼**다: 호출자는 프로세스가 끝난 뒤 `"".join(buf)` 로 전문을 얻는다.
    """
    proc = subprocess.Popen(
        [str(EXE)], cwd=str(REPO), env=env,
        stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
        text=True, encoding="utf-8", errors="replace", bufsize=1,
    )
    buf: list[str] = []

    def _drain() -> None:
        # readline 루프가 EOF 까지 돈다. 데몬이라 프로세스가 죽으면 같이 끝난다.
        try:
            for line in iter(proc.stdout.readline, ""):  # type: ignore[union-attr]
                buf.append(line)
        except (ValueError, OSError):
            pass  # 파이프가 닫히는 중이면 조용히 끝낸다

    threading.Thread(target=_drain, daemon=True, name="server-stdout-drain").start()
    return proc, buf


def _drain_settle(proc: subprocess.Popen, wait_s: float = 2.0) -> None:
    """드레인 스레드가 마지막 줄까지 읽을 시간을 유계로 준다.

    무한 대기하지 않는다 — 그러면 고치려던 결함(소비자가 생산자를 세운다)으로 되돌아간다.
    """
    deadline = time.monotonic() + wait_s
    while proc.poll() is None and time.monotonic() < deadline:
        time.sleep(0.05)
    time.sleep(0.2)


def graceful_shutdown(proc: subprocess.Popen, wait_s: float = 20.0) -> int:
    """stdin 에 `shutdown` 한 줄. 하드 킬은 최후의 수단이고 그 사실을 알린다."""
    try:
        assert proc.stdin is not None
        proc.stdin.write("shutdown\n")
        proc.stdin.flush()
    except (BrokenPipeError, OSError):
        pass
    try:
        proc.wait(timeout=wait_s)
    except subprocess.TimeoutExpired:
        print("!! 정상 종료가 시간 안에 끝나지 않아 terminate 했다 — 이 사실을 리포트에 적어야 한다",
              file=sys.stderr)
        proc.terminate()
        proc.wait(timeout=10)
    return proc.returncode


def cmd_stats(args) -> int:
    env = load_dotenv()
    addr = env.get("STARFALL_HTTP_ADDR", "127.0.0.1:8080")
    base = f"http://{addr}"
    if not EXE.is_file():
        print(f"미검증(환경): {EXE} 가 없다 — cargo build -p starfall-game-server", file=sys.stderr)
        return 2

    proc, log_buf = spawn(env)
    stats, boot_log = None, []
    deadline = time.monotonic() + args.timeout
    try:
        while time.monotonic() < deadline:
            if proc.poll() is not None:
                break
            try:
                stats = http_json(f"{base}/debug/stats")
                break
            except (urllib.error.URLError, OSError, TimeoutError):
                time.sleep(0.25)
        if stats is None:
            print("FAIL: /debug/stats 에 닿지 못했다", file=sys.stderr)
            return 1
        rc = graceful_shutdown(proc)
    finally:
        if proc.poll() is None:
            proc.kill()
    # 드레인 스레드가 이미 읽었다 — 여기서 read() 하면 빈 문자열이다.
    _drain_settle(proc)
    out = "".join(log_buf)
    boot_log = out.splitlines()

    print("### /debug/stats (서버 기동 직후, 접속 0건)")
    print(json.dumps(stats, indent=2, ensure_ascii=False))
    print(f"\n### stdin `shutdown` 후 종료 코드 = {rc}")
    print("\n### 서버 로그 (전문)")
    for line in boot_log:
        print(line)
    if args.evidence:
        Path(args.evidence).write_text(
            json.dumps({"stats": stats, "shutdown_exit_code": rc, "log": boot_log},
                       indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    return 0 if rc == 0 else 1


def mutate(doc, key: str, value):
    """중첩 무관하게 첫 번째 `key` 를 `value` 로 바꾼다. 못 찾으면 False."""
    if isinstance(doc, dict):
        for k in list(doc):
            if k == key:
                doc[k] = value
                return True
            if mutate(doc[k], key, value):
                return True
    elif isinstance(doc, list):
        for item in doc:
            if mutate(item, key, value):
                return True
    return False


def find_parent_with(doc, key: str):
    if isinstance(doc, dict):
        if key in doc:
            return doc
        for v in doc.values():
            found = find_parent_with(v, key)
            if found is not None:
                return found
    elif isinstance(doc, list):
        for v in doc:
            found = find_parent_with(v, key)
            if found is not None:
                return found
    return None


# (이름, 대상 파일 glob, 변형 함수, 기대 rule)
def _case_hard_radius(docs):
    return mutate(docs["system"], "hard_boundary_radius_m", 20000.1)


def _case_empty_points(docs):
    p = find_parent_with(docs["system"], "points_m")
    if p is None:
        return False
    p["points_m"] = []
    return True


def _case_point_count(docs):
    p = find_parent_with(docs["system"], "point_count")
    if p is None:
        return False
    p["point_count"] = p["point_count"] + 1
    return True


def _case_non_integer_ratio(docs):
    return mutate(docs["tuning"], "snapshot_hz", 7)


def _case_resume_above_linger(docs):
    p = find_parent_with(docs["system"], "reconnect_resume_window_seconds")
    if p is None:
        p = find_parent_with(docs["tuning"], "reconnect_resume_window_seconds")
    if p is None:
        return False
    p["reconnect_resume_window_seconds"] = p.get("linger_seconds", 30) + 5
    return True


def _case_spawn_outside(docs):
    p = find_parent_with(docs["system"], "points_m")
    if p is None or not p["points_m"]:
        return False
    first = p["points_m"][0]
    if isinstance(first, dict):
        for axis in ("x", "x_m"):
            if axis in first:
                first[axis] = 999999.0
                return True
        k = sorted(first)[0]
        first[k] = 999999.0
        return True
    if isinstance(first, list):
        first[0] = 999999.0
        return True
    return False


def _case_thrust_order(docs):
    p = find_parent_with(docs["ship"], "main_thrust_mps2")
    if p is None:
        return False
    p["main_thrust_mps2"] = 0.1
    return True


def _case_duplicate_class_id(docs):
    # 같은 id 를 가진 두 번째 함선 클래스 파일을 만든다 (호출부에서 처리).
    return "duplicate"


CASES = [
    ("1-hard-radius-above-ceiling", _case_hard_radius, "schema"),
    ("2-spawn-points-empty", _case_empty_points, "schema"),
    ("3-point-count-mismatch", _case_point_count, "derived"),
    ("4-tick-snapshot-ratio-not-integer", _case_non_integer_ratio, "derived"),
    ("5-resume-window-above-linger", _case_resume_above_linger, "derived"),
    ("6-spawn-point-outside-hard-boundary", _case_spawn_outside, "derived"),
    ("7-main-thrust-below-lateral-or-reverse", _case_thrust_order, "derived"),
    ("8-duplicate-ship-class-id", _case_duplicate_class_id, "derived"),
]


def cmd_reject(args) -> int:
    env0 = load_dotenv()
    if not EXE.is_file():
        print(f"미검증(환경): {EXE} 가 없다", file=sys.stderr)
        return 2
    work = Path(args.workdir).resolve()
    results = []

    for name, fn, expected_rule in CASES:
        case_dir = work / name
        if case_dir.exists():
            shutil.rmtree(case_dir)
        shutil.copytree(REPO / "data", case_dir)

        ship_files = sorted((case_dir / "ships").glob("*.json"))
        system_files = sorted((case_dir / "world" / "systems").glob("*.json"))
        tuning_file = case_dir / "movement" / "sync-tuning.json"
        docs = {
            "ship": json.loads(ship_files[0].read_text(encoding="utf-8")),
            "system": json.loads(system_files[0].read_text(encoding="utf-8")),
            "tuning": json.loads(tuning_file.read_text(encoding="utf-8")),
        }
        outcome = fn(docs)
        if outcome == "duplicate":
            dup = json.loads(ship_files[0].read_text(encoding="utf-8"))
            (case_dir / "ships" / "zz-duplicate.json").write_text(
                json.dumps(dup, indent=2), encoding="utf-8")
        elif not outcome:
            results.append({"case": name, "verdict": "SKIPPED(변형 지점을 못 찾음)"})
            continue
        else:
            ship_files[0].write_text(json.dumps(docs["ship"], indent=2), encoding="utf-8")
            system_files[0].write_text(json.dumps(docs["system"], indent=2), encoding="utf-8")
            tuning_file.write_text(json.dumps(docs["tuning"], indent=2), encoding="utf-8")

        env = dict(env0)
        env["STARFALL_DATA_DIR"] = str(case_dir)
        proc = subprocess.run([str(EXE)], cwd=str(REPO), env=env, input="",
                              capture_output=True, text=True, encoding="utf-8",
                              errors="replace", timeout=args.timeout)
        log = (proc.stdout or "") + (proc.stderr or "")
        reject_lines = [ln for ln in log.splitlines() if "기동 거부" in ln]
        has_file = any("file=" in ln for ln in reject_lines)
        has_field = any("field=" in ln for ln in reject_lines)
        has_rule = any(f"rule={expected_rule}" in ln for ln in reject_lines)
        ok = proc.returncode == 1 and bool(reject_lines)
        results.append({
            "case": name,
            "expected_rule": expected_rule,
            "exit_code": proc.returncode,
            "reject_log_lines": reject_lines[:3],
            "has_file": has_file, "has_field": has_field, "rule_matches": has_rule,
            "verdict": "PASS" if ok else "FAIL",
        })

    total = len(results)
    passed = sum(1 for r in results if r["verdict"] == "PASS")
    summary = {
        "item": "SC-06 / AC-2(b~g) — 기동 거부 8종",
        "cases_run": total,
        "passed": passed,
        "verdict": "PASS" if passed == total and total == len(CASES) else "FAIL",
        "results": results,
    }
    text = json.dumps(summary, indent=2, ensure_ascii=False)
    print(text)
    if args.evidence:
        Path(args.evidence).write_text(text + "\n", encoding="utf-8")
    return 0 if summary["verdict"] == "PASS" else 1



def cmd_serve(args) -> int:
    """서버를 띄우고 **중지 파일이 생길 때까지** 붙잡고 있는다 (블록 3~5 관측용).

    왜 필요한가: 관측 중에는 서버가 살아 있어야 하고, 끝낼 때는 **stdin `shutdown`** 이어야 한다
    (하드 킬 금지 — 계약 §0.1). 배경 프로세스로 띄워도 stdin 을 잡고 있는 주체가 있어야 그게 된다.
    """
    env = load_dotenv()
    addr = env.get("STARFALL_HTTP_ADDR", "127.0.0.1:8080")
    if not EXE.is_file():
        print(f"미검증(환경): {EXE} 가 없다", file=sys.stderr)
        return 2
    stop = Path(args.stop_file)
    if stop.exists():
        stop.unlink()
    log_path = Path(args.log) if args.log else None

    proc, log_buf = spawn(env)
    deadline = time.monotonic() + args.ready_timeout
    ready = False
    while time.monotonic() < deadline:
        if proc.poll() is not None:
            break
        try:
            http_json(f"http://{addr}/debug/stats")
            ready = True
            break
        except (urllib.error.URLError, OSError, TimeoutError):
            time.sleep(0.25)
    if not ready:
        if proc.poll() is None:
            proc.kill()
        print("FAIL: 서버가 준비되지 않았다", file=sys.stderr)
        return 1

    Path(args.ready_file).write_text(addr + "\n", encoding="utf-8")
    print(f"READY {addr} pid={proc.pid}", flush=True)

    limit = time.monotonic() + args.max_seconds
    while not stop.exists() and time.monotonic() < limit:
        if proc.poll() is not None:
            print("!! 서버가 스스로 종료됐다", file=sys.stderr)
            break
        time.sleep(0.25)

    rc = graceful_shutdown(proc) if proc.poll() is None else proc.returncode
    _drain_settle(proc)
    out = "".join(log_buf)
    if log_path:
        log_path.write_text(out, encoding="utf-8")
    print(f"SHUTDOWN exit={rc}")
    try:
        Path(args.ready_file).unlink()
    except OSError:
        pass
    return 0 if rc == 0 else 1


def main() -> int:
    ap = argparse.ArgumentParser(description="SC-05/SC-06 서버 기동 경로")
    sub = ap.add_subparsers(dest="cmd", required=True)
    s = sub.add_parser("stats", help="SC-05: 실제 data/ 로 띄우고 /debug/stats 를 받는다")
    s.add_argument("--timeout", type=float, default=30.0)
    s.add_argument("--evidence")
    s.set_defaults(func=cmd_stats)
    r = sub.add_parser("reject", help="SC-06: 기동 거부 8종")
    r.add_argument("--workdir", required=True, help="임시 data 사본을 둘 디렉토리")
    r.add_argument("--timeout", type=float, default=60.0)
    r.add_argument("--evidence")
    r.set_defaults(func=cmd_reject)
    v = sub.add_parser("serve", help="서버를 띄우고 중지 파일이 생길 때까지 붙잡는다 (블록 3~5)")
    v.add_argument("--stop-file", required=True)
    v.add_argument("--ready-file", required=True)
    v.add_argument("--log")
    v.add_argument("--ready-timeout", type=float, default=40.0)
    v.add_argument("--max-seconds", type=float, default=900.0)
    v.set_defaults(func=cmd_serve)
    args = ap.parse_args()
    return args.func(args)


if __name__ == "__main__":
    sys.exit(main())
