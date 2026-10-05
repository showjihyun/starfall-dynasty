#!/usr/bin/env python
"""사람 Unity 세션 준비 — 서버 기동부터 Editor 기동까지 한 명령으로 한다. **판정 도구가 아니다.**

왜 있나(p1-02 `05_summary.md` §3 기술 부채 7): 사람 세션 준비가 두 슬라이스에서 같은 방식으로 틀렸다.

* **빈 씬** — Editor 를 환경 변수 없이 띄웠다(p1-01 R21, p1-02 SC-68 1차). 그래서 여기서는 Editor 를
  **이 스크립트가** 띄우고, `STARFALL_GREYBOX_AUTOBUILD=1`·`STARFALL_NET_AUTOCONNECT=1`·dev 비밀값을
  **그 자식 프로세스 환경에만** 넣는다. 영구 환경 변수·`os.environ` 은 건드리지 않는다.
* **서버 조기 종료** — 실행 한도가 PC 절전 중에도 흘러 세션 도중 서버가 꺼졌다(SC-68 1차 run1). 한도는
  `--hours`(기본 6) 로 넉넉히 잡고, `status` 가 서버 생존을 보여 준다. 절전 자체는 막지 못한다 — README.

    python tests/e2e/unity_session.py start --slice p1-02-mining --tag SC-68 --new-world
    python tests/e2e/unity_session.py status --slice p1-02-mining --tag SC-68     # Play 뒤 접속 확인
    python tests/e2e/unity_session.py stop   --slice p1-02-mining --tag SC-68     # 서버만 정상 종료
    python tests/e2e/unity_session.py selftest

증거: `_workspace/{slice}/evidence/{tag}/` — `session.json`, `server.log`(종료 시), `serve_stdout.log`,
`unity_editor.log`, `new_world.json`(--new-world). **비밀값은 어디에도 쓰지 않는다**(출처만 적는다).

금지(CLAUDE.md): 서버 하드 킬(stop 파일 → `server_boot.py serve` 가 stdin `shutdown`), Unity 강제 종료,
증거 DB 쓰기(새 월드 INSERT 는 `new_world.py` 가 한다 — 여기서는 읽기 전용 SELECT 만).

종료 코드: 0 = 준비/종료 완료, 1 = 기대와 다름(로그 없음, 서버 비정상 종료 등),
2 = 환경(Editor 가 열려 있음, 바이너리·DB·Unity 없음), 4 = 서버 바이너리에 test-hooks.
"""
from __future__ import annotations

import argparse
import datetime as dt
import json
import os
import re
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
from pathlib import Path
from typing import Callable, Iterable, Mapping

for _stream in (sys.stdout, sys.stderr):
    try:
        _stream.reconfigure(encoding="utf-8", line_buffering=True)  # type: ignore[union-attr]  # cargo 출력과 순서 유지
    except (AttributeError, ValueError):
        pass

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]
CLIENT = REPO / "client"
LOCKFILE = CLIENT / "Temp" / "UnityLockfile"
VERSION_FILE = CLIENT / "ProjectSettings" / "ProjectVersion.txt"
HUB_EDITOR_ROOT = Path(r"C:\Program Files\Unity\Hub\Editor")
SERVER_CONFIG_RS = REPO / "server" / "bins" / "game-server" / "src" / "config.rs"

# 클라이언트가 읽는 변수 이름 — client/Assets/_Project/Scripts/{Greybox/GreyboxSession.cs, Net/StarfallNetHost.cs,
# Net/DevAuthToken.cs, Greybox/TwoSessionHarness.cs}. selftest 가 C# 소스의 상수와 대조한다.
VAR_AUTOBUILD = "STARFALL_GREYBOX_AUTOBUILD"
VAR_AUTOCONNECT = "STARFALL_NET_AUTOCONNECT"
VAR_WS_URL = "STARFALL_WS_URL"
VAR_SECRET = "STARFALL_DEV_AUTH_SECRET"
VAR_TWO_SESSION = "STARFALL_TWO_SESSION_AUTOBUILD"  # 켜면 같은 subject 로 서로를 밀어낸다(절차서 §3.0)
CS_CONSTANTS = {
    VAR_AUTOBUILD: "client/Assets/_Project/Scripts/Greybox/GreyboxSession.cs",
    VAR_AUTOCONNECT: "client/Assets/_Project/Scripts/Net/StarfallNetHost.cs",
    VAR_WS_URL: "client/Assets/_Project/Scripts/Net/StarfallNetHost.cs",
    VAR_SECRET: "client/Assets/_Project/Scripts/Net/DevAuthToken.cs",
    VAR_TWO_SESSION: "client/Assets/_Project/Scripts/Greybox/TwoSessionHarness.cs",
}

TAG = re.compile(r"^[A-Za-z0-9-]{1,40}$")
UUIDV7 = re.compile(r"^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$")

EXIT_OK, EXIT_FAIL, EXIT_ENV, EXIT_HOOKS = 0, 1, 2, 4

if os.name == "nt":
    DETACHED_PROCESS = 0x00000008
    CREATE_NEW_PROCESS_GROUP = 0x00000200
    CREATE_BREAKAWAY_FROM_JOB = 0x01000000
    CREATE_NO_WINDOW = 0x08000000
else:  # selftest 는 ubuntu CI 에서도 돈다
    DETACHED_PROCESS = CREATE_NEW_PROCESS_GROUP = CREATE_BREAKAWAY_FROM_JOB = CREATE_NO_WINDOW = 0


class Stop(Exception):
    """사용자에게 안내하고 멈춘다. code 는 종료 코드."""

    def __init__(self, code: int, message: str):
        super().__init__(message)
        self.code = code


# --------------------------------------------------------------------------- 순수 함수 (selftest 대상)

def build_editor_env(base: Mapping[str, str], server_env: Mapping[str, str], http_addr: str) -> dict[str, str]:
    """Editor 자식 프로세스 환경을 **새 dict 로** 만든다. `base`(보통 os.environ)는 읽기만 한다.

    비밀값은 **서버가 실제로 받은 환경**(`server_boot.load_dotenv()`)에서 가져온다 — 서버와 Editor 가 같은
    값을 쓰는 것을 구성으로 보장한다(.env 와 os.environ 우선순위가 어긋나도 둘이 갈라지지 않는다).
    """
    secret = server_env.get(VAR_SECRET, "")
    if not secret:
        raise Stop(EXIT_ENV, f"{VAR_SECRET} 를 .env 에서 찾지 못했다 — Editor 가 인증하지 못한다")
    env = dict(base)
    env[VAR_AUTOBUILD] = "1"
    env[VAR_AUTOCONNECT] = "1"
    env[VAR_SECRET] = secret
    env[VAR_WS_URL] = f"ws://{http_addr}/ws"
    env.pop(VAR_TWO_SESSION, None)
    return env


def secret_source(base: Mapping[str, str], dotenv_text: str) -> str:
    """비밀값의 **출처만** 말한다(값은 말하지 않는다). `load_dotenv` 는 os.environ 을 .env 보다 앞세운다."""
    if base.get(VAR_SECRET):
        return "os.environ (부모 셸 — .env 보다 앞선다)"
    for line in dotenv_text.splitlines():
        s = line.strip()
        if s.startswith(VAR_SECRET + "=") and s.split("=", 1)[1].strip():
            return ".env"
    return "server_boot.load_dotenv 기본값(dev_only)"


def env_isolation_check(builder: Callable[[], dict[str, str]], probe: Mapping[str, str]) -> list[str]:
    """`builder` 를 부르기 전후로 `probe`(os.environ) 가 같은지와 반환값이 별개 객체인지 본다. 문제 목록을 돌려준다."""
    before = dict(probe)
    child = builder()
    problems = []
    after = dict(probe)
    if before != after:
        changed = sorted(k for k in set(before) | set(after) if before.get(k) != after.get(k))
        problems.append(f"부모 환경이 바뀌었다: {changed}")
    if child is probe:
        problems.append("자식 환경이 부모 환경 객체 그 자체다")
    for k, want in ((VAR_AUTOBUILD, "1"), (VAR_AUTOCONNECT, "1")):
        if child.get(k) != want:
            problems.append(f"자식 환경에 {k}={want} 가 없다")
    if not child.get(VAR_SECRET):
        problems.append(f"자식 환경에 {VAR_SECRET} 가 없다")
    if VAR_TWO_SESSION in child:
        problems.append(f"자식 환경에 {VAR_TWO_SESSION} 가 남아 있다")
    return problems


def leaks_secret(secret: str, *texts: str) -> bool:
    return bool(secret) and any(secret in t for t in texts)


def read_editor_version(version_file: Path) -> str:
    m = re.search(r"^m_EditorVersion:\s*(\S+)\s*$", version_file.read_text(encoding="utf-8"), re.M)
    if not m:
        raise Stop(EXIT_ENV, f"{version_file} 에서 m_EditorVersion 을 못 읽었다")
    return m.group(1)


def find_editor_exe(version_file: Path, hub_root: Path) -> tuple[str, Path]:
    version = read_editor_version(version_file)
    exe = hub_root / version / "Editor" / "Unity.exe"
    if not exe.is_file():
        raise Stop(EXIT_ENV, f"Unity {version} 이 설치돼 있지 않다: {exe} 가 없다 "
                             f"(ProjectVersion.txt 의 버전을 Hub 로 설치한다)")
    return version, exe


def lockfile_state(path: Path, opener: Callable[[Path], None] | None = None) -> str:
    """`absent` | `locked`(Editor 가 쥐고 있다) | `stale`(파일은 있으나 아무도 안 쥠 — 비정상 종료 흔적).

    실행 중인 Editor 는 lockfile 을 공유 금지로 연다 — 읽기 열기가 PermissionError 다(실측 2026-10-05).
    """
    if not path.exists():
        return "absent"
    opener = opener or (lambda p: open(p, "rb").close())
    try:
        opener(path)
    except PermissionError:
        return "locked"
    except OSError:
        return "locked"
    return "stale"


def _norm(p: str) -> str:
    return p.replace("\\", "/").rstrip("/").lower()


def editors_for_project(procs: Iterable[tuple[int, str]], project: Path) -> list[dict]:
    """Unity.exe 프로세스 (pid, 명령줄) 중 **이 프로젝트**를 연 것. 대화형 Editor 와 임포트 워커를 구분한다."""
    want = _norm(str(project))
    out = []
    for pid, cmd in procs:
        cmd = cmd or ""
        m = re.search(r'-projectPath"?\s+(?:"([^"]+)"|(\S+))', cmd, re.I)
        if not m:
            continue
        if _norm(m.group(1) or m.group(2)) != want:
            continue
        worker = bool(re.search(r"-batchMode", cmd, re.I))
        out.append({"pid": pid, "role": "asset-import-worker" if worker else "editor"})
    return out


def preflight_verdict(lock: str, editors: list[dict]) -> list[str]:
    """Editor 를 띄워도 되는가. 막을 이유 목록(비면 진행)."""
    reasons = []
    main = [e["pid"] for e in editors if e["role"] == "editor"]
    if main:
        reasons.append(f"이 프로젝트를 연 Unity Editor 가 실행 중이다(pid {main})")
    elif editors:
        reasons.append(f"이 프로젝트의 Unity 보조 프로세스가 남아 있다(pid {[e['pid'] for e in editors]})")
    if lock == "locked":
        reasons.append(f"{LOCKFILE.relative_to(REPO).as_posix()} 를 다른 프로세스가 쥐고 있다")
    return reasons


def default_world_id(config_rs: Path = SERVER_CONFIG_RS) -> str:
    m = re.search(r'const DEFAULT_WORLD_ID: &str = "([0-9a-f-]+)"', config_rs.read_text(encoding="utf-8"))
    if not m:
        raise Stop(EXIT_ENV, f"{config_rs} 에서 DEFAULT_WORLD_ID 를 못 찾았다")
    return m.group(1)


def max_seconds(hours: float) -> int:
    if hours <= 0:
        raise Stop(EXIT_FAIL, "--hours 는 0 보다 커야 한다")
    return int(hours * 3600)


def parse_serve_stdout(text: str) -> dict:
    ready = re.search(r"^READY (\S+) pid=(\d+)", text, re.M)
    shut = re.search(r"^SHUTDOWN exit=(-?\d+)", text, re.M)
    return {"addr": ready.group(1) if ready else None,
            "server_pid": int(ready.group(2)) if ready else None,
            "shutdown_exit": int(shut.group(1)) if shut else None,
            "self_exited": "서버가 스스로 종료됐다" in text,
            "hard_terminated": "terminate 했다" in text}


# --------------------------------------------------------------------------- 환경 접근

def evidence_dir(slice_id: str, tag: str) -> Path:
    if not TAG.match(tag):
        raise Stop(EXIT_FAIL, f"--tag 모양이 아니다: {tag!r}")
    sd = REPO / "_workspace" / slice_id
    if not re.match(r"^[A-Za-z0-9._-]+$", slice_id) or not sd.is_dir():
        raise Stop(EXIT_FAIL, f"슬라이스 폴더가 없다: {sd}")
    return sd / "evidence" / tag


def unity_processes() -> list[tuple[int, str]]:
    if os.name != "nt":
        return []
    ps = ("Get-CimInstance Win32_Process -Filter \"Name='Unity.exe'\" | "
          "Select-Object ProcessId,CommandLine | ConvertTo-Json -Compress")
    r = subprocess.run(["powershell", "-NoProfile", "-NonInteractive", "-Command", ps],
                       capture_output=True, text=True, encoding="utf-8", errors="replace", timeout=60)
    if r.returncode != 0:
        raise Stop(EXIT_ENV, f"Unity 프로세스 목록을 못 읽었다: {r.stderr.strip()[:300]}")
    raw = r.stdout.strip()
    if not raw:
        return []
    data = json.loads(raw)
    if isinstance(data, dict):
        data = [data]
    return [(int(d["ProcessId"]), d.get("CommandLine") or "") for d in data]


def pid_alive(pid: int | None) -> bool:
    """tasklist 로만 본다(Windows 에서 os.kill(pid, 0) 은 CTRL_C_EVENT 라 쓰지 않는다)."""
    if not pid:
        return False
    r = subprocess.run(["tasklist", "/FI", f"PID eq {pid}", "/FO", "CSV", "/NH"],
                       capture_output=True, text=True, errors="replace")
    return f'"{pid}"' in r.stdout


def git(*args: str) -> str:
    return subprocess.run(["git", *args], cwd=REPO, capture_output=True, text=True,
                          encoding="utf-8", errors="replace").stdout


def http_stats(addr: str, timeout: float = 3.0) -> dict | None:
    try:
        with urllib.request.urlopen(f"http://{addr}/debug/stats", timeout=timeout) as r:
            return json.loads(r.read().decode("utf-8"))
    except (urllib.error.URLError, OSError, TimeoutError, ValueError):
        return None


def popen_detached(cmd: list[str], **kw) -> tuple[subprocess.Popen, str]:
    """부모(이 스크립트·에이전트 셸)가 끝나도 살아남게 띄운다. 잡 이탈이 막혀 있으면 그것 없이 띄우고 알린다."""
    if os.name != "nt":
        return subprocess.Popen(cmd, start_new_session=True, **kw), "posix-new-session"
    base = DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP
    try:
        return subprocess.Popen(cmd, creationflags=base | CREATE_BREAKAWAY_FROM_JOB, **kw), "detached+breakaway"
    except OSError:
        return subprocess.Popen(cmd, creationflags=base, **kw), "detached(잡 이탈 거부됨 — 부모 셸 종료 시 함께 끝날 수 있다)"


def write_session(path: Path, doc: dict, secret: str) -> None:
    text = json.dumps(doc, indent=2, ensure_ascii=False) + "\n"
    if leaks_secret(secret, text):  # 마지막 방어선 — 일어나면 안 된다
        raise Stop(EXIT_FAIL, "session.json 에 비밀값이 섞였다 — 쓰지 않는다")
    path.write_text(text, encoding="utf-8")


def load_session(ev: Path) -> dict:
    p = ev / "session.json"
    if not p.is_file():
        raise Stop(EXIT_FAIL, f"{p} 가 없다 — start 를 먼저 한다")
    return json.loads(p.read_text(encoding="utf-8"))


def rel(p: Path) -> str:
    try:
        return p.resolve().relative_to(REPO).as_posix()
    except ValueError:
        return str(p)


# --------------------------------------------------------------------------- 서브커맨드

def cmd_start(args) -> int:
    sys.path.insert(0, str(HERE))
    import server_boot  # noqa: E402 — load_dotenv·binary_has_test_hooks·EXE 를 같은 정의로 쓴다

    ev = evidence_dir(args.slice, args.tag)
    started = dt.datetime.now().astimezone().isoformat(timespec="seconds")
    if args.new_world and args.world:
        raise Stop(EXIT_FAIL, "--new-world 와 --world 는 함께 쓰지 않는다")

    # 1. Editor 가 열려 있으면 멈춘다. 죽이지 않는다.
    lock = lockfile_state(LOCKFILE)
    editors = unity_processes()
    mine = editors_for_project(editors, CLIENT)
    reasons = preflight_verdict(lock, mine)
    print(f"[1] Editor 점검: lockfile={lock} · 이 프로젝트의 Unity 프로세스={mine}")
    if reasons and not args.skip_editor:
        for r in reasons:
            print(f"    - {r}")
        print("멈춤: Unity Editor 를 닫아 주세요. 닫은 뒤 같은 명령을 다시 실행합니다."
              " (이 스크립트는 Unity 프로세스를 종료하지 않습니다)")
        return EXIT_ENV
    if lock == "stale":
        print("    참고: lockfile 이 남아 있지만 아무도 쥐고 있지 않다(이전 비정상 종료 흔적) — Unity 가 스스로 정리한다")

    ev.mkdir(parents=True, exist_ok=True)
    stop_file, ready_file = ev / "server.stop", ev / "server.ready"
    if ready_file.exists():
        raise Stop(EXIT_FAIL, f"{rel(ready_file)} 가 있다 — 이 태그의 서버가 아직 떠 있다. 먼저 stop 한다")

    server_env = server_boot.load_dotenv()
    addr = server_env.get("STARFALL_HTTP_ADDR", "127.0.0.1:8080")
    if http_stats(addr) is not None:
        raise Stop(EXIT_ENV, f"{addr} 에 이미 서버가 떠 있다 — 그 서버를 먼저 정상 종료한다")

    # 5(앞당김). Editor 실행 파일 — 서버를 띄운 뒤에 없다는 것을 알면 서버만 남는다.
    version, exe = (None, None)
    if not args.skip_editor:
        version, exe = find_editor_exe(VERSION_FILE, HUB_EDITOR_ROOT)
        print(f"[5] Editor: {exe} (ProjectVersion {version})")

    # 2. 깨끗한 서버 바이너리.
    print("[2] cargo build -p starfall-game-server --locked")
    b = subprocess.run(["cargo", "build", "-p", "starfall-game-server", "--locked"], cwd=REPO / "server")
    if b.returncode != 0:
        raise Stop(EXIT_ENV, f"서버 빌드 실패(exit {b.returncode})")
    if server_boot.binary_has_test_hooks(server_boot.EXE.read_bytes()):
        raise Stop(EXIT_HOOKS, f"{server_boot.EXE} 에 test-hooks 주입점이 있다 — 빌드 결과가 기대와 다르다")
    print("    test-hooks 없음")

    # 3. 월드.
    if args.new_world:
        nw = ev / "new_world.json"
        r = subprocess.run([sys.executable, str(HERE / "new_world.py"), "--tag", args.tag, "--evidence", str(nw)],
                           cwd=REPO, capture_output=True, text=True, encoding="utf-8", errors="replace")
        if r.returncode != 0 or not nw.is_file():
            raise Stop(EXIT_ENV if r.returncode == 2 else EXIT_FAIL,
                       f"new_world.py exit {r.returncode}: {r.stderr.strip()[:400]}")
        world = json.loads(nw.read_text(encoding="utf-8"))["world_id"]
        world_src = f"new_world.py → {rel(nw)}"
    elif args.world:
        if not UUIDV7.match(args.world):
            raise Stop(EXIT_FAIL, f"--world 가 UUIDv7 모양이 아니다: {args.world}")
        import db  # noqa: E402
        if db.scalar_int_strict(f"select count(*) from worlds where world_id = '{args.world}';") != 1:
            raise Stop(EXIT_FAIL, f"worlds 에 {args.world} 가 없다")
        world, world_src = args.world, "--world"
    else:
        world = server_env.get("STARFALL_WORLD_ID") or default_world_id()
        world_src = "환경/서버 기본값(config.rs DEFAULT_WORLD_ID)"
    print(f"[3] 월드 {world} ({world_src})")

    # 4. 서버 — serve 를 분리 프로세스로. serve 가 서버 stdin 을 쥐고, stop 파일이 생기면 `shutdown` 을 보낸다.
    limit = max_seconds(args.hours)
    serve_out = ev / "serve_stdout.log"
    server_log = ev / "server.log"
    cmd = [sys.executable, str(HERE / "server_boot.py"), "serve", "--stop-file", str(stop_file),
           "--ready-file", str(ready_file), "--log", str(server_log), "--ready-timeout", "60",
           "--max-seconds", str(limit), "--world", world]
    fh = open(serve_out, "w", encoding="utf-8")
    serve, detach_mode = popen_detached(cmd, cwd=REPO, stdin=subprocess.DEVNULL, stdout=fh,
                                        stderr=subprocess.STDOUT, env=dict(os.environ))
    fh.close()
    deadline = time.monotonic() + 120
    while time.monotonic() < deadline and not ready_file.exists() and serve.poll() is None:
        time.sleep(0.5)
    parsed = parse_serve_stdout(serve_out.read_text(encoding="utf-8", errors="replace"))
    if not ready_file.exists() or parsed["server_pid"] is None:
        raise Stop(EXIT_FAIL, f"서버가 READY 에 이르지 못했다(serve exit={serve.poll()}) — {rel(serve_out)} 를 본다")
    print(f"[4] READY {addr} server pid={parsed['server_pid']} serve pid={serve.pid} ({detach_mode}) "
          f"한도 {limit}s = {args.hours}h")

    doc = {
        "purpose": "사람 Unity 세션 준비(unity_session.py) — 판정 아님",
        "slice": args.slice, "tag": args.tag, "started_at": started,
        "world_id": world, "world_source": world_src,
        "server": {"addr": addr, "pid": parsed["server_pid"], "serve_pid": serve.pid, "detach": detach_mode,
                   "max_seconds": limit, "stop_file": rel(stop_file), "ready_file": rel(ready_file),
                   "serve_stdout": rel(serve_out), "log": rel(server_log), "binary": rel(server_boot.EXE),
                   "test_hooks": False},
        "editor": None,
        "git": {"head": git("rev-parse", "HEAD").strip(),
                "porcelain": [l for l in git("status", "--porcelain").splitlines() if l]},
        "preflight": {"lockfile": lock, "unity_processes_for_project": mine},
        "secret": {"variable": VAR_SECRET, "source": secret_source(os.environ, (REPO / ".env").read_text(
            encoding="utf-8") if (REPO / ".env").is_file() else ""), "injected_into": "Editor 자식 프로세스 환경만",
            "value": "기록하지 않음"},
    }
    secret = server_env.get(VAR_SECRET, "")
    session_path = ev / "session.json"

    if args.skip_editor:
        doc["editor"] = {"skipped": True, "reason": "--skip-editor (도구 자체 점검용)"}
        write_session(session_path, doc, secret)
        print(f"[8] {rel(session_path)} — Editor 는 띄우지 않았다(--skip-editor)")
        return EXIT_OK

    # 6. Editor — 환경 변수는 이 자식에게만.
    editor_log = (ev / "unity_editor.log").resolve()
    if editor_log.exists():
        editor_log.rename(editor_log.with_name(f"unity_editor.{int(time.time())}.log"))
    child_env = build_editor_env(os.environ, server_env, addr)
    ecmd = [str(exe), "-projectPath", str(CLIENT.resolve()), "-logFile", str(editor_log)]
    editor, emode = popen_detached(ecmd, cwd=str(CLIENT), env=child_env, stdin=subprocess.DEVNULL,
                                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    del child_env
    print(f"[6] Editor 기동 pid={editor.pid} ({emode}) — {VAR_AUTOBUILD}=1 {VAR_AUTOCONNECT}=1 "
          f"{VAR_WS_URL}=ws://{addr}/ws {VAR_SECRET}=<.env, 자식 환경에만>")

    # 7. 로그 파일이 실제로 생겼는가(R21: Unity 는 경로가 이상해도 조용히 엉뚱한 곳에 쓴다).
    deadline = time.monotonic() + 90
    while time.monotonic() < deadline and not (editor_log.exists() and editor_log.stat().st_size > 0):
        if editor.poll() is not None:
            break
        time.sleep(1)
    log_ok = editor_log.exists() and editor_log.stat().st_size > 0
    doc["editor"] = {"exe": str(exe), "version": version, "pid": editor.pid, "detach": emode,
                     "log": rel(editor_log), "log_created": log_ok, "alive_after_wait": editor.poll() is None,
                     "env_injected": [VAR_AUTOBUILD, VAR_AUTOCONNECT, VAR_WS_URL, VAR_SECRET],
                     "env_removed": [VAR_TWO_SESSION]}
    write_session(session_path, doc, secret)
    print(f"[7] Editor 로그 {'생김' if log_ok else '없음'}: {editor_log}")
    print(f"[8] {rel(session_path)}")
    if not log_ok:
        print("FAIL: Editor 로그 파일이 생기지 않았다 — Editor 화면을 확인한다(서버는 떠 있다; 끝낼 때 stop)",
              file=sys.stderr)
        return EXIT_FAIL
    print("준비 끝. Editor 가 열리면 Play 를 누르고, 그 뒤 `status` 로 live_connections 를 확인한다.")
    return EXIT_OK


def cmd_status(args) -> int:
    ev = evidence_dir(args.slice, args.tag)
    s = load_session(ev)
    addr, world = s["server"]["addr"], s["world_id"]
    serve_alive = pid_alive(s["server"].get("serve_pid"))
    server_alive = pid_alive(s["server"].get("pid"))
    stats = http_stats(addr)
    out: dict = {"world_id": world, "server_pid_alive": server_alive, "serve_pid_alive": serve_alive,
                 "ready_file": (REPO / s["server"]["ready_file"]).exists()}
    if stats is None:
        out["stats"] = "응답 없음"
    else:
        out["stats"] = {k: stats.get(k) for k in ("live_connections", "ws_connections",
                                                  "sessions_opened_total", "sessions_closed_total")}
    if s.get("editor") and s["editor"].get("pid"):
        out["editor_pid_alive"] = pid_alive(s["editor"]["pid"])
    sys.path.insert(0, str(HERE))
    import db  # noqa: E402
    if not UUIDV7.match(world):
        raise Stop(EXIT_FAIL, f"session.json 의 world_id 가 UUIDv7 모양이 아니다: {world}")
    try:
        for table in ("historical_events", "domain_events"):
            rows = db.psql_rows_strict(f"select event_type, count(*) from {table} "
                                       f"where world_id = '{world}' group by 1 order by 1;")
            out[table] = {r[0]: int(r[1]) for r in rows}
    except db.EnvironmentProblem as e:
        out["db"] = f"미검증(환경): {e}"
    print(json.dumps(out, indent=2, ensure_ascii=False))
    return EXIT_OK if stats is not None else EXIT_FAIL


def cmd_stop(args) -> int:
    ev = evidence_dir(args.slice, args.tag)
    s = load_session(ev)
    secret = ""  # session.json 에 원래 비밀값이 없다 — write_session 의 방어선은 start 에서만 의미가 있다
    stop_file, ready_file = REPO / s["server"]["stop_file"], REPO / s["server"]["ready_file"]
    serve_out = REPO / s["server"]["serve_stdout"]
    server_log = REPO / s["server"]["log"]
    serve_pid = s["server"].get("serve_pid")
    already = not pid_alive(serve_pid)
    stop_file.write_text(dt.datetime.now().astimezone().isoformat(timespec="seconds") + "\n", encoding="utf-8")
    print(f"stop 파일 {rel(stop_file)} — serve 가 서버 stdin 에 `shutdown` 을 보낸다"
          + (" (serve 가 이미 끝나 있었다)" if already else ""))
    deadline = time.monotonic() + 90
    while time.monotonic() < deadline and pid_alive(serve_pid):
        time.sleep(0.5)
    still = pid_alive(serve_pid)
    parsed = parse_serve_stdout(serve_out.read_text(encoding="utf-8", errors="replace")
                                if serve_out.is_file() else "")
    result = {"stopped_at": dt.datetime.now().astimezone().isoformat(timespec="seconds"),
              "serve_was_already_exited": already, "serve_still_alive": still,
              "shutdown_exit": parsed["shutdown_exit"], "server_self_exited_before_stop": parsed["self_exited"],
              "hard_terminated": parsed["hard_terminated"], "server_pid_alive": pid_alive(s["server"].get("pid")),
              "server_log": rel(server_log), "server_log_exists": server_log.is_file(),
              "ready_file_removed": not ready_file.exists(), "editor": "사용자에게 맡김(이 스크립트는 닫지 않는다)"}
    s["stop"] = result
    write_session(ev / "session.json", s, secret)
    print(json.dumps(result, indent=2, ensure_ascii=False))
    ok = (not still and parsed["shutdown_exit"] == 0 and not parsed["hard_terminated"]
          and not parsed["self_exited"] and server_log.is_file())
    if still:
        print("!! serve 가 90 s 안에 끝나지 않았다 — 강제 종료하지 않는다. serve_stdout.log 를 본다", file=sys.stderr)
    return EXIT_OK if ok else EXIT_FAIL


# --------------------------------------------------------------------------- selftest

def cmd_selftest(_args) -> int:
    cases: list[tuple[str, bool]] = []
    fake_secret = "selftest-SECRET-7f3a91"
    server_env = {VAR_SECRET: fake_secret, "STARFALL_HTTP_ADDR": "127.0.0.1:8080"}

    # (1) 환경 격리 — 실제 os.environ 으로 건다.
    os.environ.pop("__UNITY_SESSION_PROBE__", None)
    snap = dict(os.environ)
    probs = env_isolation_check(lambda: build_editor_env(os.environ, server_env, "127.0.0.1:8080"), os.environ)
    cases.append(("환경 구성: os.environ 불변, 자식에 AUTOBUILD·AUTOCONNECT·비밀값", not probs and snap == dict(os.environ)))
    child = build_editor_env({VAR_TWO_SESSION: "1", "PATH": "x"}, server_env, "127.0.0.1:9999")
    cases.append(("자식 환경: 비밀값은 서버 환경과 같은 값", child[VAR_SECRET] == fake_secret))
    cases.append(("자식 환경: TWO_SESSION 제거, WS URL 이 서버 주소", VAR_TWO_SESSION not in child
                  and child[VAR_WS_URL] == "ws://127.0.0.1:9999/ws" and child["PATH"] == "x"))
    cases.append(("비밀값이 프로세스 환경에 원래 없었다(누출 검사가 의미 있다)", fake_secret not in os.environ.values()))
    try:
        build_editor_env({}, {}, "a")
        cases.append(("비밀값 없으면 멈춘다", False))
    except Stop as e:
        cases.append(("비밀값 없으면 멈춘다", e.code == EXIT_ENV))

    # (2) 음성 대조 — 부모 환경을 고치는 구성 함수를 같은 검사기가 잡는가.
    probe = {"PATH": "x"}

    def leaky() -> dict[str, str]:
        probe[VAR_SECRET] = fake_secret  # 이 결함(영구·부모 환경 주입)이 R21 류 사고의 반대편이다
        return build_editor_env(probe, server_env, "a")
    cases.append(("음성 대조: 부모 환경을 고치는 구성 함수는 걸린다", any("부모 환경이 바뀌었다" in p
                                                         for p in env_isolation_check(leaky, probe))))
    cases.append(("음성 대조: AUTOBUILD 빠진 환경은 걸린다",
                  any(VAR_AUTOBUILD in p for p in env_isolation_check(lambda: {VAR_SECRET: "s"}, {}))))

    # (3) 비밀값 누출 — session.json 직렬화.
    doc = {"secret": {"source": secret_source({}, f"{VAR_SECRET}={fake_secret}\n"), "value": "기록하지 않음"}}
    text = json.dumps(doc, ensure_ascii=False)
    cases.append(("session 기록에 비밀값 없음, 출처는 .env", not leaks_secret(fake_secret, text)
                  and doc["secret"]["source"] == ".env"))
    cases.append(("음성 대조: 비밀값이 든 기록은 누출로 걸린다", leaks_secret(fake_secret, text + fake_secret)))
    with tempfile.TemporaryDirectory() as td:
        try:
            write_session(Path(td) / "s.json", {"x": "a" + fake_secret}, fake_secret)
            cases.append(("write_session 이 비밀값 섞인 문서를 거부한다", False))
        except Stop:
            cases.append(("write_session 이 비밀값 섞인 문서를 거부한다", not (Path(td) / "s.json").exists()))

    # (4) Editor 경로 — 버전 파일에서.
    with tempfile.TemporaryDirectory() as td:
        t = Path(td)
        vf = t / "ProjectVersion.txt"
        vf.write_text("m_EditorVersion: 6000.9.9f1\nm_EditorVersionWithRevision: 6000.9.9f1 (abc)\n", encoding="utf-8")
        hub = t / "Hub" / "Editor"
        try:
            find_editor_exe(vf, hub)
            cases.append(("Editor 미설치 버전은 멈춘다", False))
        except Stop as e:
            cases.append(("Editor 미설치 버전은 멈춘다(메시지에 버전)", "6000.9.9f1" in str(e)))
        (hub / "6000.9.9f1" / "Editor").mkdir(parents=True)
        (hub / "6000.9.9f1" / "Editor" / "Unity.exe").write_bytes(b"")
        (hub / "6000.1.0f1" / "Editor").mkdir(parents=True)
        (hub / "6000.1.0f1" / "Editor" / "Unity.exe").write_bytes(b"")
        v, exe = find_editor_exe(vf, hub)
        cases.append(("Editor 경로를 버전 파일의 버전으로 고른다(다른 설치 무시)",
                      v == "6000.9.9f1" and exe == hub / "6000.9.9f1" / "Editor" / "Unity.exe"))
    real_v = read_editor_version(VERSION_FILE)
    cases.append(("레포 ProjectVersion.txt 를 읽는다", bool(re.match(r"^\d{4}\.\d+\.\d+[abfp]\d+$", real_v))))

    # (5) lockfile·실행 중 Editor 분기.
    with tempfile.TemporaryDirectory() as td:
        lf = Path(td) / "UnityLockfile"
        cases.append(("lockfile 없음 → absent", lockfile_state(lf) == "absent"))
        lf.write_bytes(b"")
        cases.append(("아무도 안 쥔 lockfile → stale(진행)", lockfile_state(lf) == "stale"))

        def denied(_p: Path) -> None:
            raise PermissionError(13, "sharing violation")
        cases.append(("공유 거부된 lockfile → locked", lockfile_state(lf, denied) == "locked"))
    proj = Path(r"C:\WorkSpace\SpaceHistoric\client")
    procs = [
        (43528, r'"C:\Program Files\Unity\Hub\Editor\6000.6.1f1\Editor\Unity.exe" -projectPath '
                r'C:\WorkSpace\SpaceHistoric\client -logFile C:\x\unity_editor.log'),
        (32852, r'"...\Unity.exe" "-adb2" "-batchMode" "-name" "AssetImportWorkerHW0" "-projectPath" '
                r'"C:/WorkSpace/SpaceHistoric/client" "-logFile" "Logs/AssetImportWorkerHW0.log"'),
        (11111, r'"...\Unity.exe" -projectPath "D:\Other Game\client"'),
        (22222, ""),
    ]
    found = editors_for_project(procs, proj)
    cases.append(("실행 중 Editor 감지: 같은 프로젝트(대소문자·구분자 무관)만, 역할 구분",
                  found == [{"pid": 43528, "role": "editor"}, {"pid": 32852, "role": "asset-import-worker"}]))
    cases.append(("Editor 실행 중 → 멈춤", any("실행 중" in r for r in preflight_verdict("locked", found))))
    cases.append(("lockfile 만 잠김 → 멈춤", len(preflight_verdict("locked", [])) == 1))
    cases.append(("음성 대조: 다른 프로젝트의 Unity·stale lockfile 은 막지 않는다",
                  preflight_verdict("stale", editors_for_project(procs[2:], proj)) == []))

    # (6) 기타 순수 부분.
    cases.append(("--hours 6 → 21600 s", max_seconds(6) == 21600))
    cases.append(("서버 기본 월드 id 를 config.rs 에서 읽는다", bool(UUIDV7.match(default_world_id()))))
    p = parse_serve_stdout("마이그레이션 동결 확인: ...\nREADY 127.0.0.1:8080 pid=4242\nSHUTDOWN exit=0\n")
    cases.append(("serve 출력 파싱", p["server_pid"] == 4242 and p["shutdown_exit"] == 0 and not p["hard_terminated"]))
    p2 = parse_serve_stdout("READY a pid=1\n!! 서버가 스스로 종료됐다\nSHUTDOWN exit=1\n")
    cases.append(("음성 대조: 서버 자진 종료는 표시된다", p2["self_exited"] and p2["shutdown_exit"] == 1))
    drift = []
    for var, f in CS_CONSTANTS.items():
        src = (REPO / f).read_text(encoding="utf-8")
        if f'"{var}"' not in src:
            drift.append(var)
    cases.append((f"변수 이름이 C# 상수와 같다{(' — 어긋남 ' + str(drift)) if drift else ''}", not drift))

    fails = 0
    for name, ok in cases:
        print(f"{'OK  ' if ok else 'FAIL'} {name}")
        fails += 0 if ok else 1
    print(f"selftest: {'PASS' if fails == 0 else f'FAIL ({fails})'}  케이스={len(cases)}")
    return 0 if fails == 0 else 1


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    st = sub.add_parser("start", help="Editor 확인 → 빌드 → (새 월드) → 서버 → Editor")
    st.add_argument("--slice", required=True)
    st.add_argument("--tag", required=True)
    st.add_argument("--new-world", action="store_true")
    st.add_argument("--world")
    st.add_argument("--hours", type=float, default=6.0, help="서버 실행 한도(시간, 기본 6)")
    st.add_argument("--skip-editor", action="store_true",
                    help="도구 자체 점검용: Editor 를 띄우지 않고, 열린 Editor 가 있어도 서버까지만 띄운다")
    st.set_defaults(func=cmd_start)
    for name, fn, h in (("status", cmd_status, "서버 접속 수·이 월드의 이벤트 개수"),
                        ("stop", cmd_stop, "stop 파일로 서버 정상 종료(Editor 는 사용자에게)")):
        p = sub.add_parser(name, help=h)
        p.add_argument("--slice", required=True)
        p.add_argument("--tag", required=True)
        p.set_defaults(func=fn)
    sub.add_parser("selftest", help="Unity·서버 없이 순수 부분을 양성·음성 대조로 건다").set_defaults(func=cmd_selftest)
    args = ap.parse_args()
    try:
        return args.func(args)
    except Stop as e:
        print(f"멈춤: {e}", file=sys.stderr)
        return e.code


if __name__ == "__main__":
    sys.exit(main())
