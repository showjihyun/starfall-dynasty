#!/usr/bin/env python
"""AC-2(i) — **로그 소비자가 멈춰도** 서버가 응답하고 `shutdown` 으로 내려가는가.

스펙 `docs/specs/p1-01-ship-movement.md` AC-2 의 (i) 를 그대로 옮긴다. (라운드 3 리포트는 이 항목을
**AC-2(h-log)** 로 불렀다 — 스펙에 (h) 가 둘 있던 때다. architect R3 추가 판정 사안 4 에서 (i) 로 바뀌었다.)

무엇을 재는가
-------------
라운드 2 에서 실제로 터진 버그다: 서버의 `tracing` 기본 writer 가 **이벤트를 낸 스레드에서 동기로**
stdout 에 썼고, 서버를 띄운 qa 드라이버(`server_boot.py`)가 파이프를 드레인하지 않아 4096 B 가 차는
순간 tokio 워커가 하나씩 잠겨 HTTP 도 stdin `shutdown` 도 무응답이 됐다. 이 성질은 `cargo test`
안에 존재할 수 없다(테스트는 tracing 을 초기화하지 않고 `cargo test` 는 stdout 을 계속 읽는다).

**드레인 경로와 미드레인 경로를 분리한다.** `server_boot.py` 의 평소 기동은 이제 드레인하므로 그
경로로는 파이프가 차지 않는다. 이 스크립트만 일부러 **아무도 읽지 않는 파이프**로 서버를 띄운다.

판정 조건 셋 (architect R3, 스펙 AC-2(i))
-----------------------------------------
① **파이프가 실제로 찼음을 단언한다.** 근거는 둘 중 하나다:
   - `PeekNamedPipe` 로 본 **미드레인 누적 바이트 ≥ 파이프 용량**(읽지 않고 잰다), 또는
   - 로그 싱크가 스트림에 남긴 **"[로그 싱크] … N줄을 버렸다 (누적 M)"** 의 M > 0 (`--drop-probe`).
   연결 횟수는 파이프를 채우는 **수단**이지 판정 조건이 아니다. 조건이 안 생겼으면 PASS 도 FAIL 도
   아니고 **종료 코드 4 (관측 조건 미발생)** 다 — "아무 일도 없었는데 초록불"을 막는 자리다.
② **RED 를 먼저 보인다.** 이 스크립트는 바이너리를 `--exe` 로 받는다. 싱크를 끈 바이너리로 돌려
   `--expect fail` 이 맞는지 한 번 보인다(qa r3 §5 에 절차).
③ **stderr 는 파일로 돌린다.** 드레인하지 않는 것은 stdout 하나뿐이어야 한다 — 패닉 메시지처럼 서버가
   stderr 에 쓰는 것이 있으면 교착이 그쪽으로 옮겨 갈 뿐이다. (버린 줄 통지는 stderr 가 아니라 **stdout
   in-band** 로 나온다 — 라운드 3 실측, 스펙 AC-2(i) 정정.)

단계
----
1. 기동: stdout = **읽지 않는 파이프**, stderr = 파일, stdin = 파이프(`shutdown` 용).
2. 준비: `/healthz` 200 을 기다린다(부팅 로그는 파이프 용량보다 작다).
3. 채우기: `bots run --scenario b`(접속→ping→종료 반복)로 로그를 낸다. 파이프가 찰 때까지 최대
   `--fill-rounds` 회. 그동안 `PeekNamedPipe` 로 누적 바이트를 표본한다.
4. 파이프가 찬 상태에서: `/healthz`·`/debug/stats` 를 K 회, 그리고 **새 WebSocket 이
   `SESSION_READY` 까지 가는가**(라운드 2 의 첫 증상은 "TCP 는 붙는데 SESSION_READY 가 없다"였다).
5. 종료: stdin `shutdown`. 기본 모드는 **여전히 아무도 읽지 않는 상태에서** 보낸다.
   시간 안에 안 내려가면 그때 파이프를 읽기 시작해 **드레인이 서버를 풀어 주는지**를 본다 —
   풀리면 원인이 파이프였다는 인과 증거이고, 하드 킬 없이 끝낼 수 있다.
   `--drop-probe` 모드는 4 뒤에 드레인을 재개해 버린 줄 통지를 수집한 뒤 종료한다
   (그래서 이 모드의 종료는 "소비자가 멈춘 채 종료"를 증명하지 않는다 — 기본 모드가 그것을 한다).

종료 코드: 0 = `--expect` 와 일치, 1 = 불일치, 2 = 바이너리·DB·포트 문제(미검증(환경)),
          4 = 관측 조건 미발생(파이프가 차지 않았다 — 수단을 키워 다시 돈다).
"""
from __future__ import annotations

import argparse
import ctypes
import json
import os
import re
import subprocess
import sys
import threading
import time
import urllib.error
import urllib.request
from ctypes import wintypes
from pathlib import Path

for _stream in (sys.stdout, sys.stderr):
    try:
        _stream.reconfigure(encoding="utf-8")  # type: ignore[union-attr]
    except (AttributeError, ValueError):
        pass

sys.path.insert(0, str(Path(__file__).resolve().parent))
from server_boot import load_dotenv  # noqa: E402

REPO = Path(__file__).resolve().parents[2]
DEFAULT_EXE = REPO / "server" / "target" / "debug" / "starfall-game-server.exe"
BOTS = REPO / "tools" / "bots" / "target" / "debug" / "bots.exe"

# Python `subprocess` 가 만드는 익명 파이프의 용량. `GetNamedPipeInfo` 가 0 을 돌려줄 때만 쓴다
# (라운드 2 실측: 멈춘 두 인스턴스의 로그가 4033 B / 4100 B).
FALLBACK_PIPE_CAPACITY = 4096

DROP_NOTICE = re.compile(r"\[로그 싱크\] 소비자가 느려 (\d+)줄을 버렸다 \(누적 (\d+)\)")

# 판정에 필요한 최소 표본 수. 표본 0 건으로 "전부 200" 이 성립하지 않게 한다(계약 §7a).
MIN_HEALTH_SAMPLES = 3


# ── Windows 파이프 계측 ───────────────────────────────────────────────────────

if os.name == "nt":
    import msvcrt

    _k32 = ctypes.WinDLL("kernel32", use_last_error=True)
    _k32.PeekNamedPipe.argtypes = [
        wintypes.HANDLE, wintypes.LPVOID, wintypes.DWORD,
        wintypes.LPDWORD, wintypes.LPDWORD, wintypes.LPDWORD,
    ]
    _k32.PeekNamedPipe.restype = wintypes.BOOL
    _k32.GetNamedPipeInfo.argtypes = [
        wintypes.HANDLE, wintypes.LPDWORD, wintypes.LPDWORD, wintypes.LPDWORD, wintypes.LPDWORD,
    ]
    _k32.GetNamedPipeInfo.restype = wintypes.BOOL


def pipe_handle(fileobj) -> int:
    return msvcrt.get_osfhandle(fileobj.fileno())


def pipe_avail(handle: int) -> int | None:
    """읽지 않고 파이프에 쌓인 바이트 수를 본다. 실패(파이프 닫힘 등)면 None."""
    avail = wintypes.DWORD(0)
    ok = _k32.PeekNamedPipe(handle, None, 0, None, ctypes.byref(avail), None)
    return int(avail.value) if ok else None


def pipe_info(handle: int) -> dict:
    flags, out_sz, in_sz, max_inst = (wintypes.DWORD(0) for _ in range(4))
    ok = _k32.GetNamedPipeInfo(
        handle, ctypes.byref(flags), ctypes.byref(out_sz), ctypes.byref(in_sz), ctypes.byref(max_inst)
    )
    return {
        "ok": bool(ok),
        "out_buffer": int(out_sz.value),
        "in_buffer": int(in_sz.value),
    }


def capacity_from(info: dict, override: int | None) -> tuple[int, str]:
    if override:
        return override, "--pipe-capacity"
    size = max(info.get("in_buffer", 0), info.get("out_buffer", 0))
    if info.get("ok") and size > 0:
        return size, "GetNamedPipeInfo"
    return FALLBACK_PIPE_CAPACITY, "fallback 4096 (GetNamedPipeInfo 가 크기를 주지 않았다)"


class PeekSampler(threading.Thread):
    """`PeekNamedPipe` 표본을 100 ms 마다 남긴다. 파이프를 **읽지 않는다.**"""

    def __init__(self, handle: int, t0: float, period: float = 0.1) -> None:
        super().__init__(daemon=True, name="peek-sampler")
        self.handle, self.t0, self.period = handle, t0, period
        self.samples: list[tuple[float, int]] = []
        self.stop = threading.Event()

    def run(self) -> None:
        while not self.stop.is_set():
            v = pipe_avail(self.handle)
            if v is None:
                return
            self.samples.append((round(time.monotonic() - self.t0, 3), v))
            self.stop.wait(self.period)

    def latest(self) -> int:
        return self.samples[-1][1] if self.samples else 0

    def max(self) -> int:
        return max((v for _, v in self.samples), default=0)


class LateReader(threading.Thread):
    """**드레인 재개**용. 시작하기 전까지는 아무것도 읽지 않는다."""

    def __init__(self, fileobj) -> None:
        super().__init__(daemon=True, name="late-reader")
        self.fileobj = fileobj
        self.buf = bytearray()
        self.lock = threading.Lock()

    def run(self) -> None:
        fd = self.fileobj.fileno()
        while True:
            try:
                chunk = os.read(fd, 65536)
            except OSError:
                return
            if not chunk:
                return
            with self.lock:
                self.buf.extend(chunk)

    def text(self) -> str:
        with self.lock:
            return bytes(self.buf).decode("utf-8", errors="replace")

    def resume(self) -> bool:
        """한 번만 시작한다. 이미 시작했으면 False."""
        if self.ident is not None:
            return False
        self.start()
        return True


# ── HTTP·봇 ──────────────────────────────────────────────────────────────────

def http_code(url: str, timeout: float = 3.0) -> str:
    try:
        with urllib.request.urlopen(url, timeout=timeout) as r:
            r.read()
            return str(r.status)
    except urllib.error.HTTPError as e:
        return str(e.code)
    except (urllib.error.URLError, OSError, TimeoutError):
        return "000"


def run_bounded(cmd: list[str], env: dict[str, str], timeout: float, log: Path) -> dict:
    """봇은 **수단**이다. 서버가 멎으면 봇도 멎으므로 시간을 묶고, 넘기면 봇(클라이언트)만 끝낸다."""
    t = time.monotonic()
    with open(log, "wb") as f:
        p = subprocess.Popen(cmd, cwd=str(REPO), env=env, stdout=f, stderr=subprocess.STDOUT)
        try:
            rc = p.wait(timeout=timeout)
            timed_out = False
        except subprocess.TimeoutExpired:
            p.kill()
            rc = p.wait(timeout=10)
            timed_out = True
    return {"cmd": " ".join(cmd[1:]), "rc": rc, "timed_out": timed_out,
            "elapsed_s": round(time.monotonic() - t, 2), "log": log.name}


# ── 판정 (순수 함수 — `--selftest` 가 합성 관측으로 검사한다) ────────────────────

def evaluate(obs: dict) -> dict:
    """관측 → 판정. **조건이 안 생겼으면 성질을 판정하지 않는다.**

    `obs` 키: pipe_capacity, pipe_max_avail, dropped_total(None 가능), health(list of (healthz, stats)),
    session_ready_under_full(bool|None), shutdown_rc(None 가능), shutdown_without_drain(bool),
    drop_probe(bool).
    """
    full_by_peek = obs["pipe_max_avail"] >= obs["pipe_capacity"] > 0
    dropped = obs.get("dropped_total")
    full_by_drop = dropped is not None and dropped > 0
    condition = full_by_peek or full_by_drop
    basis = []
    if full_by_peek:
        basis.append(f"peek {obs['pipe_max_avail']} B >= 용량 {obs['pipe_capacity']} B")
    if full_by_drop:
        basis.append(f"싱크가 버린 줄 누적 {dropped}")

    health = obs["health"]
    n = len(health)
    healthz_ok = sum(1 for h, _ in health if h == "200")
    stats_ok = sum(1 for _, s in health if s == "200")
    health_ok = n >= MIN_HEALTH_SAMPLES and healthz_ok == n and stats_ok == n
    ready_ok = obs.get("session_ready_under_full") is True
    rc = obs.get("shutdown_rc")
    # 기본 모드는 "소비자가 멈춘 채" 내려가야 한다. drop-probe 모드는 드레인 재개 뒤 종료다.
    shutdown_ok = rc == 0 and (obs.get("drop_probe") or obs.get("shutdown_without_drain") is True)

    reasons = []
    if not health_ok:
        reasons.append(f"health {healthz_ok}/{n} · stats {stats_ok}/{n} (최소 표본 {MIN_HEALTH_SAMPLES})")
    if not ready_ok:
        reasons.append(f"파이프가 찬 상태에서 SESSION_READY 도달 = {obs.get('session_ready_under_full')}")
    if not shutdown_ok:
        reasons.append(
            f"shutdown rc={rc} without_drain={obs.get('shutdown_without_drain')} drop_probe={obs.get('drop_probe')}"
        )

    if not condition:
        verdict = "CONDITION_NOT_MET"
    elif health_ok and ready_ok and shutdown_ok:
        verdict = "PASS"
    else:
        verdict = "FAIL"
    return {
        "verdict": verdict,
        "condition_occurred": condition,
        "condition_basis": basis,
        "health_samples": n, "healthz_200": healthz_ok, "stats_200": stats_ok,
        "session_ready_under_full": obs.get("session_ready_under_full"),
        "shutdown_ok": shutdown_ok,
        "fail_reasons": reasons,
    }


# ── 본 실행 ──────────────────────────────────────────────────────────────────

def cmd_run(args) -> int:
    exe = Path(args.exe).resolve()
    if not exe.is_file():
        print(f"미검증(환경): {exe} 가 없다", file=sys.stderr)
        return 2
    if not BOTS.is_file():
        print(f"미검증(환경): {BOTS} 가 없다 — tools/bots 에서 cargo build", file=sys.stderr)
        return 2
    env = load_dotenv()
    env.pop("RUST_LOG", None)
    if args.rust_log:
        env["RUST_LOG"] = args.rust_log
    addr = env.get("STARFALL_HTTP_ADDR", "127.0.0.1:8080")
    base = f"http://{addr}"
    ws = f"ws://{addr}/ws"
    if http_code(f"{base}/healthz", timeout=1.0) != "000":
        print(f"미검증(환경): {addr} 에 이미 무언가가 응답한다 — 다른 서버가 떠 있다", file=sys.stderr)
        return 2

    ev = Path(args.evidence).resolve() / args.label
    ev.mkdir(parents=True, exist_ok=True)
    timeline: list[dict] = []
    t0 = time.monotonic()

    def mark(event: str, **kw) -> None:
        row = {"t_s": round(time.monotonic() - t0, 3), "event": event, **kw}
        timeline.append(row)
        print(json.dumps(row, ensure_ascii=False), flush=True)

    stderr_path = ev / "server-stderr.log"
    stderr_f = open(stderr_path, "wb")  # ③ stderr 는 파일로 — 교착이 옮겨 갈 자리를 없앤다
    proc = subprocess.Popen(
        [str(exe)], cwd=str(REPO), env=env,
        stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=stderr_f, bufsize=0,
    )
    handle = pipe_handle(proc.stdout)
    info = pipe_info(handle)
    capacity, cap_src = capacity_from(info, args.pipe_capacity)
    mark("spawned", pid=proc.pid, exe=str(exe), exe_mtime=time.strftime(
        "%Y-%m-%dT%H:%M:%S", time.localtime(exe.stat().st_mtime)), rust_log=env.get("RUST_LOG"),
        pipe_info=info, pipe_capacity=capacity, capacity_source=cap_src)
    sampler = PeekSampler(handle, t0)
    sampler.start()
    reader = LateReader(proc.stdout)

    # 2. 준비
    deadline = time.monotonic() + args.ready_timeout
    ready = False
    while time.monotonic() < deadline and proc.poll() is None:
        if http_code(f"{base}/healthz", timeout=1.0) == "200":
            ready = True
            break
        time.sleep(0.25)
    mark("ready" if ready else "not_ready", peek=sampler.latest())
    if not ready:
        reader.resume()
        if proc.poll() is None:
            proc.stdin.write(b"shutdown\n")
            proc.stdin.flush()
            try:
                proc.wait(timeout=20)
            except subprocess.TimeoutExpired:
                proc.kill()
        reader.join(timeout=5)
        (ev / "server-stdout.log").write_text(reader.text(), encoding="utf-8")
        stderr_f.close()
        print("미검증(환경): 서버가 준비되지 않았다 — DB(docker compose) 를 확인하라. 로그는 증거 폴더", file=sys.stderr)
        return 2

    # 3. 채우기 — 수단. 판정 조건은 아래 peek 이다.
    fills = []
    for rnd in range(1, args.fill_rounds + 1):
        r = run_bounded(
            [str(BOTS), "run", "--scenario", "b", "--bots", str(args.fill_bots),
             "--cycles", str(args.fill_cycles), "--pings", "1", "--url", ws,
             "--out", str(ev / f"fill-{rnd}")],
            env, args.fill_timeout, ev / f"fill-{rnd}.log",
        )
        r["round"] = rnd
        r["peek_after"] = sampler.latest()
        fills.append(r)
        mark("fill_round", **r)
        if sampler.max() >= capacity:
            break
    mark("fill_done", peek_now=sampler.latest(), peek_max=sampler.max(), capacity=capacity)

    # 4. 파이프가 찬 상태에서의 응답성
    health: list[tuple[str, str]] = []
    for i in range(args.health_samples):
        h = http_code(f"{base}/healthz")
        s = http_code(f"{base}/debug/stats")
        health.append((h, s))
        mark("health", i=i, healthz=h, stats=s, peek=sampler.latest())
        time.sleep(args.health_interval)
    probe = run_bounded(
        [str(BOTS), "probe", "--case", "auth-ok", "--url", ws, "--label", args.probe_label],
        env, args.probe_timeout, ev / "probe-auth-ok.log",
    )
    probe_text = (ev / "probe-auth-ok.log").read_bytes().decode("utf-8", errors="replace")
    session_ready = (not probe["timed_out"]) and "session_id=" in probe_text
    mark("probe_session_ready", ok=session_ready, **probe, peek=sampler.latest())

    # 5. 종료
    dropped_total = None
    if args.drop_probe:
        reader.resume()
        mark("drain_resumed", reason="--drop-probe: 버린 줄 통지 수집")
        wait_until = time.monotonic() + args.drop_wait
        while time.monotonic() < wait_until and not DROP_NOTICE.search(reader.text()):
            time.sleep(0.2)
        found = DROP_NOTICE.findall(reader.text())
        dropped_total = max((int(c) for _, c in found), default=0)
        mark("drop_notice", notices=len(found), dropped_total=dropped_total)

    sampler.stop.set()
    t_sd = time.monotonic()
    try:
        proc.stdin.write(b"shutdown\n")
        proc.stdin.flush()
    except OSError as e:
        mark("shutdown_write_failed", error=str(e))
    mark("shutdown_sent", reader_running=reader.is_alive())
    exited_without_drain = False
    released_by_drain = None
    hard_killed = False
    try:
        proc.wait(timeout=args.shutdown_timeout)
        exited_without_drain = reader.ident is None
    except subprocess.TimeoutExpired:
        mark("shutdown_timeout", waited_s=args.shutdown_timeout)
        if reader.ident is None:
            # 드레인이 서버를 풀어 주는가 — 풀리면 원인이 파이프였다는 인과 증거다.
            reader.resume()
            mark("drain_resumed", reason="shutdown 이 시간 안에 안 먹었다 — 소비자를 되살려 본다")
            try:
                proc.wait(timeout=args.shutdown_timeout)
                released_by_drain = True
            except subprocess.TimeoutExpired:
                released_by_drain = False
        if proc.poll() is None:
            hard_killed = True
            mark("HARD_KILL", note="드레인으로도 풀리지 않았다 — 이 사실 자체가 판정 대상이다")
            proc.kill()
            proc.wait(timeout=10)
    rc = proc.returncode
    mark("exited", rc=rc, elapsed_s=round(time.monotonic() - t_sd, 2),
         without_drain=exited_without_drain, released_by_drain=released_by_drain, hard_killed=hard_killed)

    # 남은 stdout — 종료 뒤에는 읽어도 판정에 영향이 없다.
    reader.resume()
    reader.join(timeout=10)
    stderr_f.close()
    out_text = reader.text()
    (ev / "server-stdout.log").write_text(out_text, encoding="utf-8")
    notices = DROP_NOTICE.findall(out_text)
    if notices:
        dropped_total = max(dropped_total or 0, max(int(c) for _, c in notices))
    with open(ev / "peek.csv", "w", encoding="utf-8") as f:
        f.write("t_s,avail_bytes\n")
        for t, v in sampler.samples:
            f.write(f"{t},{v}\n")

    obs = {
        "pipe_capacity": capacity,
        "pipe_max_avail": sampler.max(),
        "dropped_total": dropped_total,
        "health": health,
        "session_ready_under_full": session_ready,
        "shutdown_rc": rc if not hard_killed else None,
        "shutdown_without_drain": exited_without_drain,
        "drop_probe": args.drop_probe,
    }
    result = evaluate(obs)
    first_full = next((t for t, v in sampler.samples if v >= capacity), None)
    summary = {
        "label": args.label, "expect": args.expect, "mode": "drop-probe" if args.drop_probe else "strict",
        "exe": str(exe), "rust_log": env.get("RUST_LOG"),
        "pipe": {"capacity": capacity, "capacity_source": cap_src, "info": info,
                 "max_avail": sampler.max(), "first_full_at_s": first_full,
                 "samples": len(sampler.samples)},
        "fills": fills, "probe": probe, "health": health,
        "stdout_bytes_after": len(out_text.encode("utf-8")),
        "stderr_bytes": stderr_path.stat().st_size,
        "dropped_total": dropped_total, "drop_notices": len(notices),
        "shutdown": {"rc": rc, "without_drain": exited_without_drain,
                     "released_by_drain": released_by_drain, "hard_killed": hard_killed},
        **result,
        "timeline": timeline,
    }
    (ev / "verdict.json").write_text(json.dumps(summary, ensure_ascii=False, indent=2), encoding="utf-8")
    print(f"VERDICT {result['verdict']} (expect {args.expect}) condition={result['condition_basis']} "
          f"reasons={result['fail_reasons']}")
    if result["verdict"] == "CONDITION_NOT_MET":
        return 4
    matched = (result["verdict"] == "PASS") == (args.expect == "pass")
    return 0 if matched else 1


# ── 도구 자체 검증 (서버 불필요) ───────────────────────────────────────────────

_WRITER = (
    "import sys,time\n"
    "n=int(sys.argv[1])\n"
    "sys.stdout.buffer.write(b'x'*n); sys.stdout.buffer.flush()\n"
    "time.sleep(float(sys.argv[2]))\n"
)


def _peek_child(nbytes: int, hold: float = 3.0) -> tuple[int, int, dict]:
    """자식이 nbytes 를 쓰고 hold 초 버틴다. 우리는 **읽지 않고** peek 만 한다."""
    p = subprocess.Popen([sys.executable, "-c", _WRITER, str(nbytes), str(hold)],
                         stdout=subprocess.PIPE, bufsize=0)
    h = pipe_handle(p.stdout)
    info = pipe_info(h)
    cap, _ = capacity_from(info, None)
    time.sleep(1.0)
    avail = pipe_avail(h) or 0
    p.stdout.read()  # 이제 비워 준다(자식이 끝날 수 있게)
    p.wait(timeout=hold + 10)
    return avail, cap, info


def cmd_selftest(_args) -> int:
    """계약 §3.3·§7a: **이 도구가 틀렸을 때 빨간불이 켜지는가.**

    1. 판정 함수: 입력 0 · 조건 미발생 · 표본 부족 · 하드 킬 · 드레인 뒤 종료 가 PASS 를 받지 않는다.
    2. ① 검출기: 파이프에 조금만 쓰면 "차지 않음", 넘치게 쓰면 "참" 을 실제 파이프로 구분한다.
    """
    fails = 0

    def check(name: str, cond: bool) -> None:
        nonlocal fails
        print(f"{'ok  ' if cond else 'FAIL'} {name}")
        fails += 0 if cond else 1

    good = {"pipe_capacity": 4096, "pipe_max_avail": 4200, "dropped_total": None,
            "health": [("200", "200")] * 5, "session_ready_under_full": True,
            "shutdown_rc": 0, "shutdown_without_drain": True, "drop_probe": False}
    check("정상 관측은 PASS", evaluate(good)["verdict"] == "PASS")
    zero = {"pipe_capacity": 0, "pipe_max_avail": 0, "dropped_total": None, "health": [],
            "session_ready_under_full": None, "shutdown_rc": None,
            "shutdown_without_drain": False, "drop_probe": False}
    check("입력이 전부 0 이면 PASS 가 아니다", evaluate(zero)["verdict"] != "PASS")
    check("파이프가 안 찼으면 CONDITION_NOT_MET",
          evaluate({**good, "pipe_max_avail": 1200})["verdict"] == "CONDITION_NOT_MET")
    check("버린 줄 > 0 은 peek 없이도 조건 성립",
          evaluate({**good, "pipe_max_avail": 0, "dropped_total": 17, "drop_probe": True})["verdict"] == "PASS")
    check("버린 줄 0 은 조건이 아니다",
          evaluate({**good, "pipe_max_avail": 0, "dropped_total": 0})["verdict"] == "CONDITION_NOT_MET")
    check("health 표본 부족(2)이면 PASS 가 아니다",
          evaluate({**good, "health": [("200", "200")] * 2})["verdict"] == "FAIL")
    check("healthz 000 한 번이면 FAIL",
          evaluate({**good, "health": [("200", "200")] * 4 + [("000", "200")]})["verdict"] == "FAIL")
    check("SESSION_READY 미도달이면 FAIL",
          evaluate({**good, "session_ready_under_full": False})["verdict"] == "FAIL")
    check("하드 킬(rc=None)이면 FAIL", evaluate({**good, "shutdown_rc": None})["verdict"] == "FAIL")
    check("rc=1 이면 FAIL", evaluate({**good, "shutdown_rc": 1})["verdict"] == "FAIL")
    check("기본 모드에서 드레인 뒤에야 내려갔으면 FAIL",
          evaluate({**good, "shutdown_without_drain": False})["verdict"] == "FAIL")

    small, cap, info = _peek_child(512)
    print(f"     peek(512 B 씀) = {small}, 용량 = {cap}, info = {info}")
    check("512 B 만 쓴 파이프는 '참' 이 아니다", small < cap and small == 512)
    big, cap2, _ = _peek_child(64 * 1024)
    print(f"     peek(64 KiB 씀, 막힘) = {big}, 용량 = {cap2}")
    check("64 KiB 를 쓰려다 막힌 파이프는 peek >= 용량", big >= cap2)
    print(f"selftest: {'PASS' if fails == 0 else f'FAIL ({fails})'}")
    return 0 if fails == 0 else 1


def main() -> int:
    ap = argparse.ArgumentParser(description="AC-2(i): 드레인하지 않는 stdout 파이프에서도 서버가 선다")
    sub = ap.add_subparsers(dest="cmd", required=True)
    r = sub.add_parser("run", help="실서버를 띄운다 — 측정이다(빌드 중에 돌리지 않는다, G-c)")
    r.add_argument("--exe", default=str(DEFAULT_EXE), help="서버 바이너리. RED 는 싱크를 끈 바이너리")
    r.add_argument("--label", required=True, help="증거 하위 폴더 이름 (예: red, green, green-drop)")
    r.add_argument("--evidence", required=True)
    r.add_argument("--expect", choices=["pass", "fail"], required=True)
    r.add_argument("--rust-log", help="서버 RUST_LOG. 기본은 서버 기본 필터(환경의 RUST_LOG 는 지운다)")
    r.add_argument("--drop-probe", action="store_true",
                   help="4단계 뒤 드레인을 재개해 '버린 줄' 통지를 수집한 뒤 종료")
    r.add_argument("--drop-wait", type=float, default=15.0)
    r.add_argument("--pipe-capacity", type=int, help="용량을 직접 준다(기본: GetNamedPipeInfo)")
    r.add_argument("--ready-timeout", type=float, default=40.0)
    r.add_argument("--fill-rounds", type=int, default=3)
    r.add_argument("--fill-bots", type=int, default=4)
    r.add_argument("--fill-cycles", type=int, default=5)
    r.add_argument("--fill-timeout", type=float, default=60.0)
    r.add_argument("--health-samples", type=int, default=10)
    r.add_argument("--health-interval", type=float, default=0.5)
    r.add_argument("--probe-label", default="bot-090")
    r.add_argument("--probe-timeout", type=float, default=20.0)
    r.add_argument("--shutdown-timeout", type=float, default=20.0)
    sub.add_parser("selftest", help="판정 함수와 ① 검출기를 서버 없이 검사")
    args = ap.parse_args()
    if os.name != "nt":
        print("미검증(환경): PeekNamedPipe 계측은 Windows 전용이다", file=sys.stderr)
        return 2
    return cmd_run(args) if args.cmd == "run" else cmd_selftest(args)


if __name__ == "__main__":
    sys.exit(main())
