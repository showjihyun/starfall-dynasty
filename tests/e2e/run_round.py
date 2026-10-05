"""라운드 자동 실행기 (qa-throughput R5·R6) — 한 번 실행으로 라운드 판정 **입력**을 모은다. 슬라이스 공용.

    python tests/e2e/run_round.py --slice p1-02 --contract <계약> --round r4
    python tests/e2e/run_round.py --slice p1-02 --contract <계약> --round r4 --only gates,census,filters,sources
    python tests/e2e/run_round.py --slice p1-02 --contract <계약> --round r4 --dry-run
    python tests/e2e/run_round.py selftest

**판정은 하지 않는다.** SC 의 PASS/FAIL 은 리포트가 계약 문구로 정한다. 여기서 쓰는 상태는 "그 단계의
명령이 어떻게 끝났는가" 다: PASS · FAIL · 미검증(환경) · 미검증(사람 대기) · 미검증(조건 미발생) ·
미검증(입력 없음) · 무효(동결 위반) · SKIP(고르지 않음) · DRY(--dry-run).

단계(순서대로. `--only`/`--skip` 으로 고른다. freeze·final 은 `--only` 에 없어도 늘 돈다 — 빼려면 `--skip`):
  freeze   HEAD · porcelain · 소스 sha256 스냅샷 · 증거 DB 지문 · FREEZE 표지 확인
  gates    cargo fmt --check · clippy -D warnings · test --workspace(DB, .env, STARFALL_DB_TESTS=required) · release sim
  census   db_test_census.py (계약 [DB] 이름 ↔ STARFALL_DB_TEST RAN)
  repeat   test --workspace N 회(--repeat, 기본 10)
  unity    EditMode · PlayMode (Editor 가 열려 있으면 미검증(환경)으로 두고 계속)
  filters  cargo_sc_map.py run (계약 필터마다 실행 수·유령·주인 SC)
  sources  check_item_sources.py --slice
  offline  슬라이스 설정의 정적·오프라인 명령(selftest·스키마·경계면 표·golden sha·코드 생성 결정성)
  bots     실서버 봇 케이스(새 월드 · server_boot serve · stdin shutdown)
  judges   파이썬 판정(봇이 만든 월드) + 읽기 전용 SQL
  db_stop  **사람 단계** — DB 정지를 포함하는 케이스. 승인 전에는 멈춘다(아래)
  final    소스 sha256 재대조 · porcelain · 증거 DB 지문 재대조

사람 단계: `--approve db_stop` 이 있으면 바로 돈다. 없으면 터미널이면 묻고, 아니면 `<out>/WAITING_db_stop.txt`
를 쓰고 `<out>/go_db_stop`(또는 `skip_db_stop`) 파일을 `--gate-timeout` 초까지 기다린다. 시간이 다 되면
그 단계는 미검증(사람 대기)로 두고 계속한다.

동결 감시(R6): `--watch-interval`(기본 15 초)마다 소스 mtime 과, 이 실행기의 자손이 아닌 cargo·rustc·Unity
프로세스를 본다. 위반이 겹친 단계는 무효(동결 위반)로 표시하고 계속한다. `_workspace/{slice}/FREEZE` 가
없으면 경고한다.

출력: `<out>/summary.json`(기본 `_workspace/{slice}/evidence/{round}_{YYYYMMDD}/`). 단계마다 다시 쓰고, 끝나면
JSON 뒤에 `DONE` 한 줄을 붙인다. 기다리는 쪽은 `until grep -qx DONE <out>/summary.json; do …; done`.
읽을 때는 `json.JSONDecoder().raw_decode(text)[0]` 또는 `run_round.load_summary(path)`.
진행 상황은 `<out>/progress.txt`.

금지(코드로 막는다): 서버 하드 킬(봇 그룹은 stop 파일 → server_boot 의 stdin `shutdown` 만, 안 내려가면
죽이지 않고 FAIL 로 남긴다) · `docker compose down` · 이 실행기의 증거 DB 쓰기(SQL 은 SELECT/WITH 만,
세션을 read-only 로 연다) · golden 재생 파일 수정(`STARFALL_REPLAY_BLESS` 를 환경에서 지운다) ·
`.env` 값 출력(값은 자식 프로세스 환경에만 넣고 요약·로그에 쓰지 않는다).
봇·판정 단계의 계약 도구는 **그 실행이 만든 새 월드**에만 쓴다(계약이 지명한 SC-25 변조 포함) — 증거 월드
지문을 freeze/final 에서 대조한다.

슬라이스별 값(봇 케이스·판정 명령·증거 월드·오프라인 명령)은 `tests/e2e/round_configs/<슬라이스 폴더>.json`
에 둔다. 이 파일에는 슬라이스 값을 쓰지 않는다.
"""
from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import os
import re
import shlex
import shutil
import subprocess
import sys
import tempfile
import threading
import time
from dataclasses import dataclass, field
from pathlib import Path
from typing import Callable

for _s in (sys.stdout, sys.stderr):
    try:
        _s.reconfigure(encoding="utf-8")
    except (AttributeError, ValueError):
        pass

REPO = Path(__file__).resolve().parents[2]
E2E = REPO / "tests" / "e2e"
EXE_SUFFIX = ".exe" if os.name == "nt" else ""
SERVER_EXE = REPO / "server" / "target" / "debug" / f"starfall-game-server{EXE_SUFFIX}"
BOTS_EXE = REPO / "tools" / "bots" / "target" / "debug" / f"bots{EXE_SUFFIX}"

STAGES = ["freeze", "gates", "census", "repeat", "unity", "filters", "sources", "offline",
          "bots", "judges", "db_stop", "final"]
ALWAYS = {"freeze", "final"}
HUMAN_GATES = {"db_stop": "DB 정지(docker compose stop/start postgres)를 포함한다. 팀에 DB 정지를 공지하고, "
                          "다른 에이전트의 DB 테스트·서버가 돌지 않는지 확인한 뒤 승인한다."}

PASS, FAIL = "PASS", "FAIL"
ENV, HUMAN, NOCOND, NOINPUT = "미검증(환경)", "미검증(사람 대기)", "미검증(조건 미발생)", "미검증(입력 없음)"
INVALID, SKIP, DRY = "무효(동결 위반)", "SKIP", "DRY"
E2E_EXIT = {0: PASS, 1: FAIL, 2: ENV, 3: "FAIL(구현 없음)", 4: NOCOND}

# 동결 대상 소스 — 슬라이스 공용(제품 소스·계약·데이터). qa 도구(tests/e2e)는 넣지 않는다.
SOURCE_ROOTS = {
    "server": None,                                  # target/ 를 뺀 전부(golden 재생 파일 포함)
    "tools/bots": None,
    "client/Assets/_Project": (".cs", ".asmdef", ".uxml", ".uss", ".json", ".unity", ".prefab"),
    "contracts": None,
    "data": None,
}
SKIP_DIRS = {"target", "Library", "Temp", "Logs", "obj", "__pycache__", ".git", "node_modules"}
WATCH_PROCS = ("cargo", "rustc", "unity")

# 단계별 감시 범주 (리더 결정 2026-10-05, R6 완화).
#   client: client/ 를 읽거나 Unity 를 쓰는 단계 — Unity 프로세스(Editor·AssetImportWorker 포함)도 무효 사유다.
#   server: server/·tools/·tests/ 만 쓰는 단계 — Unity 프로세스는 무효 사유가 아니다(위반 기록은 남긴다).
# 소스 내용 변화와 외부 cargo·rustc 는 범주와 관계없이 모든 단계를 무효로 한다.
# offline 은 codegen 결정성 검사(client 의 Generated 와 대조)를 포함해 client 다. filters 의 Unity 맨 이름
# 대조는 client/ 가 아니라 unity 단계가 남긴 결과 xml 을 읽으므로 filters 는 server 다(리더 결정의 목록과 같다).
# freeze·final 은 client/ 소스를 sha 로 읽지만 내용 변화는 sha 대조가 직접 잡으므로 server 로 둔다.
STAGE_WATCH = {"freeze": "server", "gates": "server", "census": "server", "repeat": "server",
               "unity": "client", "filters": "server", "sources": "server", "offline": "client",
               "bots": "server", "judges": "server", "db_stop": "server", "final": "server"}


# ================================================================ 공통

def now_iso() -> str:
    return dt.datetime.now().astimezone().isoformat(timespec="seconds")


def load_dotenv(root: Path = REPO) -> dict:
    """`.env` KEY=VALUE. **값은 자식 환경에만 넣는다 — 어디에도 찍지 않는다.**"""
    env = {}
    p = root / ".env"
    if p.is_file():
        for line in p.read_text(encoding="utf-8", errors="replace").splitlines():
            line = line.strip()
            if not line or line.startswith("#") or "=" not in line:
                continue
            k, v = line.split("=", 1)
            env[k.strip()] = v.strip().strip('"').strip("'")
    return env


def base_env(dotenv: dict) -> dict:
    env = dict(os.environ)
    env.update(dotenv)
    home = Path.home()
    extra = [home / ".cargo" / "bin", home / ".dotnet" / "tools", home / "AppData" / "Local" / "Unity" / "bin"]
    env["PATH"] = os.pathsep.join([str(p) for p in extra if p.exists()] + [env.get("PATH", "")])
    env["PYTHONIOENCODING"] = "utf-8"
    env.setdefault("CARGO_TERM_COLOR", "never")
    env.pop("STARFALL_REPLAY_BLESS", None)            # golden 재생 파일을 다시 만들지 않는다
    return env


def rel(p) -> str:
    try:
        return Path(p).resolve().relative_to(REPO).as_posix()
    except (ValueError, OSError):
        return str(p)


def load_summary(path) -> dict:
    """summary.json(뒤에 `DONE` 줄이 붙을 수 있다) → dict."""
    return json.JSONDecoder().raw_decode(Path(path).read_text(encoding="utf-8"))[0]


def source_files(root: Path = REPO, roots: dict = SOURCE_ROOTS) -> list[Path]:
    out = []
    for r, exts in roots.items():
        base = root / r
        if not base.exists():
            continue
        for dp, dn, fn in os.walk(base):
            dn[:] = sorted(d for d in dn if d not in SKIP_DIRS)
            for f in sorted(fn):
                if exts is None or f.endswith(exts):
                    out.append(Path(dp) / f)
    return out


def sha_snapshot(files: list[Path], root: Path = REPO) -> dict:
    snap = {}
    for p in files:
        try:
            st = p.stat()
            snap[p.relative_to(root).as_posix()] = (hashlib.sha256(p.read_bytes()).hexdigest(), st.st_mtime)
        except OSError:
            continue
    return snap


def sql_is_read_only(sql: str) -> bool:
    s = re.sub(r"--[^\n]*", "", sql).strip().rstrip(";").strip()
    if ";" in s:
        return False
    return bool(re.match(r"(?is)^(select|with)\b", s)) and not re.search(
        r"(?is)\b(insert|update|delete|truncate|drop|alter|create|grant|revoke|copy|call|do)\b", s)


def compose_argv_is_safe(argv: list[str]) -> bool:
    """이 실행기가 직접 부르는 docker compose 는 `exec`·`ps` 만."""
    if len(argv) >= 3 and Path(argv[0]).stem.lower() == "docker" and argv[1] == "compose":
        return argv[2] in ("exec", "ps")
    return True


# ================================================================ 프로세스 목록 · 동결 감시

def list_processes() -> list[dict]:
    """[{pid, ppid, name, cmd}] — Windows 는 CIM, 그 밖은 ps."""
    try:
        if os.name == "nt":
            ps = ("Get-CimInstance Win32_Process | Select-Object ProcessId,ParentProcessId,Name,CommandLine,CreationDate"
                  " | ConvertTo-Json -Compress")
            out = subprocess.run(["powershell", "-NoProfile", "-NonInteractive", "-Command", ps],
                                 capture_output=True, timeout=60)
            data = json.loads(out.stdout.decode("utf-8", errors="replace") or "[]")
            if isinstance(data, dict):
                data = [data]
            return [{"pid": d.get("ProcessId"), "ppid": d.get("ParentProcessId"), "name": d.get("Name") or "",
                     "cmd": d.get("CommandLine") or ""} for d in data]
        out = subprocess.run(["ps", "-eo", "pid=,ppid=,comm=,args="], capture_output=True, text=True, timeout=30)
        rows = []
        for line in out.stdout.splitlines():
            parts = line.split(None, 3)
            if len(parts) >= 3:
                rows.append({"pid": int(parts[0]), "ppid": int(parts[1]), "name": parts[2],
                             "cmd": parts[3] if len(parts) > 3 else ""})
        return rows
    except (OSError, subprocess.SubprocessError, ValueError):
        return []


def descendants(procs: list[dict], root_pid: int) -> set:
    kids: dict = {}
    for p in procs:
        kids.setdefault(p["ppid"], []).append(p["pid"])
    seen, stack = {root_pid}, [root_pid]
    while stack:
        for c in kids.get(stack.pop(), []):
            if c not in seen:
                seen.add(c)
                stack.append(c)
    return seen


def is_editor(p: dict) -> bool:
    n = Path(p["name"]).stem.lower()
    return n == "unity" and "-batchmode" not in p["cmd"].lower()


class Watcher:
    """15 초 감시 — 소스 mtime(바뀌면 sha 로 내용 확인)과 외부 cargo/rustc/Unity 프로세스.
    위반은 시각 창과 함께 쌓는다. 기준선(baseline)이 None 이면 소스는 보지 않는다(freeze 전)."""

    def __init__(self, files_fn: Callable[[], list], baseline: dict | None, interval: float = 15.0,
                 lister: Callable[[], list] = list_processes, my_pid: int | None = None, root: Path = REPO):
        self.files_fn, self.baseline, self.interval, self.root = files_fn, baseline, interval, root
        self.lister, self.my_pid = lister, my_pid or os.getpid()
        self.violations: list[dict] = []
        self.touched_same_content: list[dict] = []
        self.checks = 0
        self.last_check = time.time()
        self._seen_mtime: dict = {}
        self._seen_proc: dict = {}
        self._stop = threading.Event()
        self._lock = threading.Lock()
        self._t: threading.Thread | None = None

    def start(self) -> None:
        self._t = threading.Thread(target=self._loop, daemon=True)
        self._t.start()

    def stop(self) -> None:
        self._stop.set()
        if self._t:
            self._t.join(timeout=self.interval + 70)
        self.check()

    def _loop(self) -> None:
        while not self._stop.wait(self.interval):
            self.check()

    def check(self) -> None:
        with self._lock:
            t_prev, t = self.last_check, time.time()
            if self.baseline is not None:
                present = set()
                for p in self.files_fn():
                    path = p.relative_to(self.root).as_posix()
                    present.add(path)
                    try:
                        mtime = p.stat().st_mtime
                    except OSError:
                        continue
                    a = self.baseline.get(path)
                    if a is not None and a[1] == mtime:
                        continue                                  # mtime 그대로 — 해시하지 않는다
                    try:
                        sha = hashlib.sha256(p.read_bytes()).hexdigest()
                    except OSError:
                        continue
                    sig = (path, sha, mtime)
                    if sig in self._seen_mtime:
                        continue
                    self._seen_mtime[sig] = True
                    if a is not None and a[0] == sha:            # 다시 쓰였지만 내용은 같다 — 기록만
                        self.touched_same_content.append({"path": path, "at": mtime})
                        continue
                    self.violations.append({"kind": "source_" + ("added" if a is None else "modified"),
                                            "path": path, "at": mtime, "window": [min(mtime, t_prev), t], "seen": t})
                for path in sorted(set(self.baseline) - present):
                    if (path, None, None) not in self._seen_mtime:
                        self._seen_mtime[(path, None, None)] = True
                        self.violations.append({"kind": "source_removed", "path": path, "at": t,
                                                "window": [t_prev, t], "seen": t})
            procs = self.lister()
            if procs:
                mine = descendants(procs, self.my_pid)
                for p in procs:
                    name = Path(p["name"]).stem.lower()
                    if name not in WATCH_PROCS or p["pid"] in mine:   # 정확히 cargo·rustc·Unity (Unity Hub 는 아님)
                        continue
                    key = (p["pid"], name)
                    if key in self._seen_proc:
                        self._seen_proc[key]["window"][1] = t
                        continue
                    v = {"kind": "external_process", "pid": p["pid"], "name": p["name"],
                         "cmd": (p["cmd"] or "")[:300], "editor": is_editor(p), "proc_class": "unity" if name == "unity" else "cargo", "window": [t_prev, t], "seen": t}
                    self._seen_proc[key] = v
                    self.violations.append(v)
            self.checks += 1
            self.last_check = t

    def overlapping(self, start: float, end: float, category: str = "client", relax_unity: bool = True) -> list[dict]:
        """그 시간 창에 겹친 위반 중 그 단계의 감시 범주에서 무효 사유인 것."""
        with self._lock:
            return [v for v in self.violations if v["window"][0] <= end and v["window"][1] >= start
                    and not (relax_unity and category == "server" and v.get("proc_class") == "unity")]


# ================================================================ 실행 문맥

@dataclass
class Cmd:
    name: str
    argv: list
    cwd: str
    rc: object = None
    status: str = ""
    seconds: float = 0.0
    log: str = ""
    missing: list = field(default_factory=list)


@dataclass
class Stage:
    name: str
    status: str = SKIP
    raw_status: str = ""
    started: str = ""
    finished: str = ""
    seconds: float = 0.0
    commands: list = field(default_factory=list)
    metrics: dict = field(default_factory=dict)
    notes: list = field(default_factory=list)
    violations: list = field(default_factory=list)
    t0: float = 0.0
    t1: float = 0.0

    def to_json(self) -> dict:
        return {"name": self.name, "status": self.status, "raw_status": self.raw_status or self.status,
                "started": self.started, "finished": self.finished, "seconds": round(self.seconds, 1),
                "commands": [{"name": c.name, "argv": c.argv, "cwd": c.cwd, "rc": c.rc, "status": c.status,
                              "seconds": round(c.seconds, 1), "log": c.log,
                              **({"missing": c.missing} if c.missing else {})} for c in self.commands],
                "metrics": self.metrics, "notes": self.notes, "violations": self.violations}


class Ctx:
    def __init__(self, out: Path, args, cfg: dict, dotenv: dict, dry: bool = False):
        self.out, self.args, self.cfg, self.dry = out, args, cfg, dry
        self.dotenv = dotenv
        self.quiet = False
        self.env = base_env(dotenv)
        self.stage: Stage | None = None
        self.vars: dict = {}
        self.snapshot: dict = {}
        self.fp_before = None
        self.tmp = Path(tempfile.mkdtemp(prefix="run_round_"))
        # 가림 대상은 비밀 성격의 키만 — POSTGRES_USER=starfall 같은 값을 가리면 크레이트 이름까지 망가진다
        self.secret_values = [v for k, v in dotenv.items()
                              if v and len(v) >= 6 and re.search(r"SECRET|PASSWORD|TOKEN|KEY|_URL$", k)]
        self.built_by_prebuild = {str(SERVER_EXE), str(BOTS_EXE)}

    # ---- 경로
    def sdir(self) -> Path:
        d = self.out / self.stage.name
        d.mkdir(parents=True, exist_ok=True)
        return d

    def expand(self, s: str) -> str:
        def rep(m):
            k = m.group(1)
            if k in self.vars:
                return str(self.vars[k])
            return m.group(0)
        return re.sub(r"\{([A-Za-z_][\w.\-]*)\}", rep, s)

    # ---- 명령
    def run(self, name: str, argv: list, *, cwd: Path = REPO, env: dict | None = None, timeout: float | None = None,
            log: Path | None = None, stdout: Path | str | None = None, stderr: Path | None = None,
            exit_map: dict | None = None, extra_env: dict | None = None) -> Cmd:
        argv = [str(a) for a in argv]
        if not compose_argv_is_safe(argv):
            raise RuntimeError(f"금지된 docker compose 명령: {argv[:3]}")
        c = Cmd(name, [rel(a) if ("/" in a or "\\" in a) and Path(a).is_absolute() else a for a in argv], rel(cwd))
        self.stage.commands.append(c)
        if self.dry:
            c.status = DRY
            c.missing = self._missing(argv, cwd)
            return c
        e = dict(env or self.env)
        e.pop("STARFALL_REPLAY_BLESS", None)
        if extra_env:
            e.update(extra_env)
        if log is None and stdout is None:
            log = self.sdir() / f"{name}.log"
        c.log = rel(log) if log else rel(stdout) if stdout not in (None, "-") else rel(stderr) if stderr else ""
        out_f = open(log or stdout, "wb") if (log or stdout not in (None, "-")) else subprocess.DEVNULL
        err_f = subprocess.STDOUT if log else (open(stderr, "wb") if stderr else subprocess.DEVNULL)
        t0 = time.monotonic()
        try:
            proc = subprocess.Popen(argv, cwd=str(cwd), env=e, stdin=subprocess.DEVNULL, stdout=out_f, stderr=err_f)
            try:
                c.rc = proc.wait(timeout=timeout)
            except subprocess.TimeoutExpired:
                self._kill_tree(proc)
                c.rc = "timeout"
        except OSError as ex:
            c.rc = "not-found"
            self.stage.notes.append(f"{name}: 실행 불가 — {ex.__class__.__name__}")
        finally:
            for f in (out_f, err_f):
                if hasattr(f, "close"):
                    f.close()
        c.seconds = time.monotonic() - t0
        m = exit_map or {0: PASS}
        c.status = (FAIL + "(시간 초과)") if c.rc == "timeout" else ENV if c.rc == "not-found" else m.get(c.rc, FAIL)
        self.progress(f"  {name}: rc={c.rc} {c.status} {c.seconds:.0f}s")
        return c

    def _kill_tree(self, proc: subprocess.Popen) -> None:
        """시간 초과한 **도구** 프로세스(cargo·봇·unity CLI)만. 게임 서버는 이 경로로 오지 않는다(봇 그룹 참조)."""
        if os.name == "nt":
            subprocess.run(["taskkill", "/T", "/F", "/PID", str(proc.pid)], capture_output=True)
        else:
            proc.kill()
        try:
            proc.wait(timeout=30)
        except subprocess.TimeoutExpired:
            pass

    def _missing(self, argv: list, cwd: Path) -> list:
        miss = []
        exe = argv[0]
        if not (shutil.which(exe, path=self.env.get("PATH")) or Path(exe).is_file()):
            miss.append(f"실행 파일 없음: {exe}" + (" (bots·서버 빌드 명령이 만든다)" if exe in self.built_by_prebuild else ""))
        for a in argv[1:]:
            if re.search(r"\.(py|exe|ps1|sh)$", a) and not a.startswith("-") and "{" not in a:
                p = Path(a) if Path(a).is_absolute() else (cwd / a)
                if not p.exists() and not (REPO / a).exists():
                    miss.append(f"파일 없음: {a}" + (" (빌드 명령이 만든다)" if str(p) in self.built_by_prebuild else ""))
        if not Path(cwd).exists():
            miss.append(f"작업 디렉터리 없음: {rel(cwd)}")
        return miss

    def py(self, script: str, *args) -> list:
        return [sys.executable, str(E2E / script), *args]

    def psql_select(self, name: str, sql: str, out: Path) -> Cmd:
        if not sql_is_read_only(sql):
            raise RuntimeError(f"읽기 전용이 아닌 SQL 거부: {sql[:80]}")
        argv = ["docker", "compose", "exec", "-T", "postgres", "psql", "-q", "-v", "ON_ERROR_STOP=1", "-U", "starfall",
                "-d", "starfall", "-At", "-F", "|", "-c", "SET default_transaction_read_only = on", "-c", sql]
        return self.run(name, argv, stdout=out, stderr=out.with_suffix(".err"), timeout=120,
                        exit_map={0: PASS, 1: ENV, 2: ENV, 3: FAIL})

    def progress(self, line: str) -> None:
        msg = f"[{dt.datetime.now().strftime('%H:%M:%S')}] {line}"
        if not self.quiet:
            print(msg, flush=True)
        with open(self.out / "progress.txt", "a", encoding="utf-8") as f:
            f.write(msg + "\n")


def aggregate(cmds: list[Cmd]) -> str:
    st = [c.status for c in cmds]
    if not st:
        return NOINPUT
    if any(s == DRY for s in st):
        return DRY
    if any(s.startswith(FAIL) for s in st):
        return FAIL
    if any(s == ENV for s in st):
        return ENV
    if any(s == NOCOND for s in st):
        return NOCOND
    return PASS


# ================================================================ 단계

def test_totals(log: Path) -> dict:
    p = f = i = 0
    failed = []
    if log.is_file():
        for line in log.read_text(encoding="utf-8", errors="replace").splitlines():
            m = re.match(r"^test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored", line)
            if m:
                p, f, i = p + int(m.group(1)), f + int(m.group(2)), i + int(m.group(3))
            m2 = re.match(r"^test (\S+) \.\.\. FAILED", line)
            if m2:
                failed.append(m2.group(1))
    return {"passed": p, "failed": f, "ignored": i, "failed_names": failed}


def stage_freeze(ctx: Ctx) -> None:
    d = ctx.sdir()
    ctx.run("head", ["git", "rev-parse", "HEAD"], stdout=d / "head.txt", stderr=d / "head.err")
    ctx.run("porcelain", ["git", "status", "--porcelain"], stdout=d / "porcelain.txt", stderr=d / "porcelain.err")
    freeze = REPO / "_workspace" / ctx.cfg["slice_dir"] / "FREEZE"
    ctx.stage.metrics["freeze_file"] = {"path": rel(freeze), "present": freeze.is_file()}
    if not freeze.is_file():
        ctx.stage.notes.append(f"경고: {rel(freeze)} 가 없다 — 동결 표지 없이 돈다(R6)")
        ctx.progress(f"!! 경고: {rel(freeze)} 없음")
    if ctx.dry:
        ctx.stage.metrics["source_files"] = len(source_files())
    else:
        ctx.snapshot = sha_snapshot(source_files())
        (d / "src_sha_start.txt").write_text("".join(f"{h}  {p}\n" for p, (h, _) in sorted(ctx.snapshot.items())),
                                             encoding="utf-8", newline="\n")
        ctx.stage.metrics["source_files"] = len(ctx.snapshot)
        ctx.vars["head"] = (d / "head.txt").read_text(encoding="utf-8").strip() if (d / "head.txt").is_file() else ""
    fp_sql = fingerprint_sql(ctx.cfg)
    if fp_sql:
        c = ctx.psql_select("fp_before", fp_sql, d / "fp_before.txt")
        if not ctx.dry and c.status == PASS:
            ctx.fp_before = (d / "fp_before.txt").read_text(encoding="utf-8").strip()
            ctx.stage.metrics["fp_before"] = ctx.fp_before
    ctx.stage.status = aggregate(ctx.stage.commands)


def fingerprint_sql(cfg: dict) -> str | None:
    w = (cfg.get("evidence_db") or {}).get("world_id")
    if not w:
        return None
    if not re.fullmatch(r"[0-9a-f-]{36}", w):
        raise RuntimeError("evidence_db.world_id 형식 오류")
    return ("select (select count(*) from domain_events where world_id = '%s') || '|' || "
            "(select md5(coalesce(string_agg(event_id::text, ',' order by event_id), '')) from domain_events "
            "where world_id = '%s') || '|' || (select count(*) from _sqlx_migrations) || '|' || "
            "(select count(*) from pg_database where datname like 'starfall_test_%%')" % (w, w))


def stage_gates(ctx: Ctx) -> None:
    d, srv = ctx.sdir(), REPO / "server"
    env = dict(ctx.env, STARFALL_DB_TESTS="required")
    g = ctx.cfg.get("gates", {})
    ctx.run("fmt", ["cargo", "fmt", "--all", "--check"], cwd=srv, env=env, log=d / "fmt.log", timeout=600)
    ctx.run("clippy", ["cargo", "clippy", "--workspace", "--all-targets", "--", "-D", "warnings"], cwd=srv, env=env,
            log=d / "clippy.log", timeout=3600)
    ctx.run("test", ["cargo", "test", "--workspace", "--locked", "--no-fail-fast"], cwd=srv, env=env,
            log=d / "test.log", timeout=3600)
    for crate in g.get("release_crates", ["starfall-sim"]):
        ctx.run(f"release_{crate}", ["cargo", "test", "-p", crate, "--release", "--locked", "--no-fail-fast"],
                cwd=srv, env=env, log=d / f"release_{crate}.log", timeout=3600)
    if not ctx.dry:
        ctx.stage.metrics["test"] = test_totals(d / "test.log")
        ctx.stage.metrics["release"] = {c: test_totals(d / f"release_{c}.log")
                                        for c in g.get("release_crates", ["starfall-sim"])}
        if ctx.stage.metrics["test"]["passed"] == 0:
            ctx.stage.notes.append("workspace test 의 passed 합이 0 — 실행이 아무것도 재지 않았다")
            ctx.stage.status = FAIL
            return
    ctx.stage.status = aggregate(ctx.stage.commands)


def test_log_path(ctx: Ctx) -> Path:
    return Path(ctx.args.test_log) if ctx.args.test_log else ctx.out / "gates" / "test.log"


def stage_census(ctx: Ctx) -> None:
    d, log = ctx.sdir(), test_log_path(ctx)
    if not ctx.dry and not log.is_file():
        ctx.stage.notes.append(f"입력 없음: {rel(log)} (gates 를 같이 돌리거나 --test-log)")
        ctx.stage.status = NOINPUT
        return
    ctx.run("census", ctx.py("db_test_census.py", "--contract", ctx.args.contract, "--log", log,
                             "--evidence", d / "census.json"), log=d / "census.out", timeout=120)
    if not ctx.dry and (d / "census.json").is_file():
        j = json.loads((d / "census.json").read_text(encoding="utf-8"))
        ctx.stage.metrics = {k: j.get(k) for k in ("verdict", "expected", "expected_and_ran", "ran_lines")}
        ctx.stage.metrics["missing"] = len(j.get("missing", []))
        ctx.stage.metrics["skipped"] = len(j.get("skipped", []))
    ctx.stage.status = aggregate(ctx.stage.commands)


def stage_repeat(ctx: Ctx) -> None:
    d, srv = ctx.sdir(), REPO / "server"
    env = dict(ctx.env, STARFALL_DB_TESTS="required")
    runs = []
    for i in range(1, ctx.args.repeat + 1):
        c = ctx.run(f"run{i}", ["cargo", "test", "--workspace", "--locked", "--no-fail-fast"], cwd=srv, env=env,
                    log=d / f"run{i}.log", timeout=3600)
        if not ctx.dry:
            t = test_totals(d / f"run{i}.log")
            runs.append({"run": i, "rc": c.rc, **t})
    if ctx.dry:
        ctx.stage.status = aggregate(ctx.stage.commands)
        return
    ctx.stage.metrics = {"runs": runs, "passed_set": sorted({r["passed"] for r in runs})}
    if not runs:
        ctx.stage.status = NOINPUT
    elif any(r["rc"] != 0 for r in runs) or len({r["passed"] for r in runs}) != 1 or runs[0]["passed"] == 0:
        ctx.stage.status = FAIL
        ctx.stage.notes.append("rc≠0 이거나 회차마다 passed 수가 다르다 — 재시도로 덮지 말고 비결정성으로 기록")
    else:
        ctx.stage.status = PASS


def junit_counts(xml: Path) -> dict:
    if not xml.is_file():
        return {"present": False}
    body = xml.read_text(encoding="utf-8", errors="replace")
    cases = len(re.findall(r"<testcase\b", body))
    fails = len(re.findall(r"<(failure|error)\b", body))
    skipped = len(re.findall(r"<skipped\b", body))
    return {"present": True, "tests": cases, "failures": fails, "skipped": skipped}


def stage_unity(ctx: Ctx) -> None:
    d = ctx.sdir()
    unity = shutil.which("unity", path=ctx.env.get("PATH"))
    if not ctx.dry:
        editors = [p for p in list_processes() if is_editor(p)]
        if editors:
            ctx.stage.notes.append(f"Unity Editor 가 열려 있다(pid {', '.join(str(p['pid']) for p in editors)}) — "
                                   "단일 인스턴스라 unity test 를 돌릴 수 없다")
            ctx.stage.status = ENV
            return
        if not unity:
            ctx.stage.notes.append("unity CLI 없음")
            ctx.stage.status = ENV
            return
    for mode in ctx.cfg.get("unity_modes", ["EditMode", "PlayMode"]):
        xml = d / f"{mode.lower()}.xml"
        ctx.run(mode, [unity or "unity", "test", "client", "--mode", mode, "--report-format", "junit",
                       "--output", xml], log=d / f"{mode.lower()}.log", timeout=3600)
        if not ctx.dry:
            jc = junit_counts(xml)
            ctx.stage.metrics[mode] = jc
            if not jc.get("present") or jc.get("tests", 0) == 0:
                ctx.stage.commands[-1].status = FAIL
                ctx.stage.notes.append(f"{mode}: 결과 xml 이 없거나 테스트 0 건")
    ctx.stage.status = aggregate(ctx.stage.commands)


def stage_filters(ctx: Ctx) -> None:
    d = ctx.sdir()
    argv = ctx.py("cargo_sc_map.py", "run", "--contract", ctx.args.contract, "--out", d)
    for x in sorted((ctx.out / "unity").glob("*.xml")) if (ctx.out / "unity").is_dir() else []:
        argv += ["--unity-xml", x]
    for x in ctx.args.unity_xml or []:
        argv += ["--unity-xml", x]
    env = dict(ctx.env, STARFALL_DB_TESTS="required")
    ctx.run("cargo_sc_map", argv, env=env, log=d / "cargo_sc_map.out", timeout=4 * 3600,
            exit_map={0: PASS, 1: FAIL, 2: ENV})
    if not ctx.dry and (d / "filters.json").is_file():
        s = json.loads((d / "filters.json").read_text(encoding="utf-8"))["summary"]
        ctx.stage.metrics = {k: s.get(k) for k in ("rows", "filters_rust", "filters_rust_cargo", "filters_rust_bare",
                                                    "filters_unity_bare", "executed", "ran_ge_1", "static_ghosts",
                                                    "ghosts", "failed_tests", "nonzero_rc", "cross_sc", "unity_bare")}
    ctx.stage.status = aggregate(ctx.stage.commands)


def stage_sources(ctx: Ctx) -> None:
    d = ctx.sdir()
    ctx.run("check_item_sources", ctx.py("check_item_sources.py", "--contract", ctx.args.contract,
                                         "--slice", ctx.cfg["slice_label"]), log=d / "check_item_sources.out",
            timeout=600, exit_map={0: PASS, 1: FAIL, 2: ENV, 3: FAIL, 4: FAIL})
    ctx.stage.status = aggregate(ctx.stage.commands)


def run_step(ctx: Ctx, step: dict, d: Path) -> None:
    """설정의 한 단계 — cmd · sql · sha_compare · codegen_compare."""
    t = step.get("type", "cmd")
    name = ctx.expand(step["name"])
    if t == "cmd":
        argv = [ctx.expand(a) for a in step["argv"]]
        argv = [sys.executable if a == "{python}" else a for a in argv]
        if argv and argv[0] == "python":
            argv[0] = sys.executable
        if argv and argv[0] == "bots":
            argv[0] = str(BOTS_EXE)
        cwd = REPO / step.get("cwd", ".")
        so = step.get("stdout")
        kw = {}
        if so:
            kw["stdout"] = "-" if so == "-" else d / ctx.expand(so)
            kw["stderr"] = d / ctx.expand(step.get("stderr", f"{name}.err"))
        else:
            kw["log"] = d / f"{name}.log"
        ctx.run(name, argv, cwd=cwd, timeout=step.get("timeout", 1800),
                exit_map=E2E_EXIT if step.get("exit", "e2e") == "e2e" else {0: PASS}, extra_env=step.get("env"), **kw)
    elif t == "sql":
        worlds = step.get("for_each") or [None]
        for w in worlds:
            wid = ctx.vars.get(f"world.{w}", "{world.%s}" % w) if w else None
            sql = ctx.expand(step["query"]).replace("{w}", wid or "")
            nm = name + (f"_{w}" if w else "")
            ctx.psql_select(nm, sql, d / f"{nm}.txt")
            if step.get("compare_to") and not ctx.dry:
                exp = (REPO / step["compare_to"]).read_text(encoding="utf-8").split()
                got = (d / f"{nm}.txt").read_text(encoding="utf-8").split()
                ok = exp == got
                ctx.stage.commands[-1].status = PASS if ok else FAIL
                ctx.stage.notes.append(f"{nm}: 기준 {rel(REPO / step['compare_to'])} 와 {'같다' if ok else '다르다'}")
    elif t == "sha_compare":
        files = sorted(REPO.glob(step["glob"]))
        c = Cmd(name, ["sha256", step["glob"], "vs", step["baseline"]], ".")
        ctx.stage.commands.append(c)
        if ctx.dry:
            c.status = DRY
            c.missing = [] if (REPO / step["baseline"]).is_file() and files else [f"기준·대상 없음: {step['baseline']}"]
            return
        got = {hashlib.sha256(p.read_bytes()).hexdigest() for p in files}
        base = set(re.findall(r"^([0-9a-f]{64})\b", (REPO / step["baseline"]).read_text(encoding="utf-8"), re.M))
        want = set(step.get("baseline_hashes") or []) or base
        (d / f"{name}.txt").write_text("".join(f"{hashlib.sha256(p.read_bytes()).hexdigest()}  {rel(p)}\n"
                                               for p in files), encoding="utf-8", newline="\n")
        c.log, c.rc = rel(d / f"{name}.txt"), 0
        c.status = PASS if files and got <= want else FAIL
        ctx.stage.metrics[name] = {"files": len(files), "all_in_baseline": got <= want}
    elif t == "codegen_compare":
        gen = REPO / step["generated"]
        outdir = ctx.tmp / "codegen"
        for i in (1, 2):
            sub = outdir / f"run{i}"
            argv = ["dotnet", "run", "ContractsCodegen.cs", "--", "--contracts", str(REPO / "contracts"),
                    "--out", str(sub)]
            ctx.run(f"{name}_run{i}", argv, cwd=REPO / "tools" / "codegen", log=d / f"{name}_run{i}.log",
                    timeout=900)
        if ctx.dry:
            return

        def shas(p: Path) -> dict:
            return {f.name: hashlib.sha256(f.read_bytes()).hexdigest() for f in sorted(p.glob("*.cs"))}
        a, b, g = shas(outdir / "run1"), shas(outdir / "run2"), shas(gen)
        ok = bool(g) and a == b == g
        ctx.stage.metrics[name] = {"generated_files": len(g), "run1_eq_run2": a == b, "eq_committed": a == g}
        c = Cmd(name + "_compare", ["sha256", "tmp/run1", "tmp/run2", rel(gen)], ".", 0, PASS if ok else FAIL)
        ctx.stage.commands.append(c)
    else:
        raise RuntimeError(f"모르는 단계 종류: {t}")


def stage_offline(ctx: Ctx) -> None:
    d = ctx.sdir()
    steps = ctx.cfg.get("offline", [])
    if not steps:
        ctx.stage.status = NOINPUT
        ctx.stage.notes.append("설정에 offline 단계가 없다")
        return
    for s in steps:
        run_step(ctx, s, d)
    ctx.stage.status = aggregate(ctx.stage.commands)


# ---------------------------------------------------------------- 봇

def new_world(ctx: Ctx, tag: str, d: Path, var: str) -> str | None:
    d.mkdir(parents=True, exist_ok=True)
    ev = d / "world.json"
    ctx.run(f"{var}_new_world", ctx.py("new_world.py", "--tag", tag, "--evidence", ev), log=d / "world.out",
            timeout=120, exit_map=E2E_EXIT)
    if ctx.dry:
        ctx.vars[f"world.{var}"] = "{world.%s}" % var
        return ctx.vars[f"world.{var}"]
    try:
        w = json.loads(ev.read_text(encoding="utf-8"))["world_id"]
    except (OSError, ValueError, KeyError):
        return None
    ctx.vars[f"world.{var}"] = w
    (d / "world_id").write_text(w + "\n", encoding="utf-8")
    return w


def bot_group(ctx: Ctx, g: dict, d: Path, attempt: int = 0) -> bool:
    """새 월드 1 · 서버 1 회 기동 · 케이스별 봇 신원 분리(r2 run_cases.sh 와 같은 명령). False = 서버가 안 내려감."""
    name = g["name"] + (f"_retry{attempt}" if attempt else "")
    gd = d / name
    gd.mkdir(parents=True, exist_ok=True)
    w = ctx.vars.get(f"world.{g['reuse_world']}") if g.get("reuse_world") else \
        new_world(ctx, ctx.expand(g["tag"]) + (f"-retry{attempt}" if attempt else ""), gd, name)
    if not w:
        ctx.stage.notes.append(f"{name}: 월드를 만들지 못했다")
        return True
    if attempt == 0 and not g.get("reuse_world"):
        ctx.vars[f"world.{g['name']}"] = w
    stop, ready = gd / "stop", gd / "ready"
    for f in (stop, ready):
        f.unlink(missing_ok=True)
    max_s = int(g.get("max_seconds", 1200))
    serve_argv = ctx.py("server_boot.py", "serve", "--world", w, "--stop-file", stop, "--ready-file", ready,
                        "--log", gd / "server.log", "--max-seconds", str(max_s))
    serve_cmd = Cmd(f"{name}_serve", [rel(a) if Path(str(a)).is_absolute() else str(a) for a in serve_argv], ".")
    ctx.stage.commands.append(serve_cmd)
    if ctx.dry:
        serve_cmd.status = DRY
        serve_cmd.missing = ctx._missing([str(a) for a in serve_argv], REPO)
        if not SERVER_EXE.is_file():
            serve_cmd.missing.append(f"서버 바이너리 없음: {rel(SERVER_EXE)} (bots 단계의 빌드 명령이 만든다)")
    else:
        t0 = time.monotonic()
        serve_out = open(gd / "serve.out", "wb")
        serve = subprocess.Popen([str(a) for a in serve_argv], cwd=str(REPO), env=dict(ctx.env, **(g.get("env") or {})),
                                 stdin=subprocess.DEVNULL,
                                 stdout=serve_out, stderr=subprocess.STDOUT)
        deadline = time.monotonic() + 90
        while time.monotonic() < deadline and not ready.exists() and serve.poll() is None:
            time.sleep(0.5)
        if not ready.exists():
            stop.write_text("stop\n", encoding="utf-8")
            try:
                serve.wait(timeout=90)
            except subprocess.TimeoutExpired:
                pass
            serve_out.close()
            serve_cmd.rc, serve_cmd.status, serve_cmd.log = serve.poll(), FAIL, rel(gd / "serve.out")
            ctx.stage.notes.append(f"{name}: 서버가 준비되지 않았다")
            return serve.poll() is not None
    for spec in g["cases"]:
        case, dep, label, *rest = spec.split(":", 3)
        extra = shlex.split((rest[0] if rest else "").replace("WORLD", w))
        ctx.run(f"{name}.{case}", [str(BOTS_EXE), "probe", "--case", case, "--deposit", dep, "--label", label, *extra,
                                   "--out", gd / f"{case}.json"],
                stdout="-", stderr=gd / f"{case}.err", timeout=g.get("case_timeout", 1500), exit_map=E2E_EXIT,
                extra_env=g.get("env"))
    if ctx.dry:
        return True
    stop.write_text("stop\n", encoding="utf-8")       # server_boot 가 stdin `shutdown` 을 보낸다
    try:
        rc = serve.wait(timeout=max_s + 120)
    except subprocess.TimeoutExpired:
        rc = None
    serve_out.close()
    serve_cmd.seconds = time.monotonic() - t0
    serve_cmd.log = rel(gd / "serve.out")
    if rc is None:
        serve_cmd.rc, serve_cmd.status = "running", FAIL
        ctx.stage.notes.append(f"{name}: stop 뒤에도 서버가 안 내려갔다 — 하드 킬하지 않는다. 사람 확인 필요"
                               f"(server_boot pid {serve.pid})")
        return False
    tail = (gd / "serve.out").read_text(encoding="utf-8", errors="replace").strip().splitlines()[-1:]
    serve_cmd.rc = rc
    serve_cmd.status = PASS if rc == 0 else FAIL
    ctx.stage.notes.append(f"{name}: world={w} {tail[0] if tail else ''}")
    # r1 선례: 종료 4(겹침 조건 미발생)는 새 월드로 정해진 횟수만 다시 — 두 결과 모두 남긴다
    if attempt < int(g.get("retry_exit4", 0)) and any(
            c.rc == 4 for c in ctx.stage.commands if c.name.startswith(f"{name}.")):
        ctx.progress(f"  {name}: 종료 4 → 새 월드로 재시도 {attempt + 1}")
        return bot_group(ctx, g, d, attempt + 1)
    return True


def stage_bots(ctx: Ctx) -> None:
    d = ctx.sdir()
    bcfg = ctx.cfg.get("bots") or {}
    if not bcfg.get("steps"):
        ctx.stage.status = NOINPUT
        ctx.stage.notes.append("설정에 bots 단계가 없다")
        return
    if bcfg.get("prebuild", True):
        ctx.run("build_server", ["cargo", "build", "-p", "starfall-game-server"], cwd=REPO / "server",
                log=d / "build_server.log", timeout=1800)
        ctx.run("build_bots", ["cargo", "build"], cwd=REPO / "tools" / "bots", log=d / "build_bots.log", timeout=1800)
        if not ctx.dry and any(c.status != PASS for c in ctx.stage.commands):
            ctx.stage.status = FAIL
            ctx.stage.notes.append("빌드 실패 — 봇 케이스를 돌리지 않는다")
            return
        if not ctx.dry and SERVER_EXE.is_file():
            ctx.stage.metrics["server_sha256"] = hashlib.sha256(SERVER_EXE.read_bytes()).hexdigest()
    run_bot_steps(ctx, bcfg["steps"], d)
    ctx.stage.status = aggregate(ctx.stage.commands)


def run_bot_steps(ctx: Ctx, steps: list, d: Path) -> bool:
    for s in steps:
        t = s.get("type", "cmd")
        if t == "world":
            new_world(ctx, ctx.expand(s["tag"]), d / s["name"], s["name"])
        elif t == "group":
            if not bot_group(ctx, s, d):
                ctx.stage.notes.append("서버가 내려가지 않아 남은 봇 단계를 멈춘다(포트 점유)")
                return False
        else:
            run_step(ctx, s, d)
    return True


def stage_judges(ctx: Ctx) -> None:
    d = ctx.sdir()
    steps = ctx.cfg.get("judges", [])
    if not steps:
        ctx.stage.status = NOINPUT
        return
    needed = sorted({m for s in steps for m in re.findall(r"\{world\.([\w-]+)\}", json.dumps(s, ensure_ascii=False))}
                    | {w for s in steps for w in s.get("for_each", [])})
    if not ctx.dry:
        bots_dir = ctx.out / "bots"
        for w in needed:
            if f"world.{w}" not in ctx.vars and (bots_dir / w / "world_id").is_file():
                ctx.vars[f"world.{w}"] = (bots_dir / w / "world_id").read_text(encoding="utf-8").strip()
        missing = [w for w in needed if f"world.{w}" not in ctx.vars]
        if missing:
            ctx.stage.notes.append(f"입력 없음: 봇 월드 {missing} (bots 단계를 같이 돌린다)")
            ctx.stage.status = NOINPUT
            return
        ctx.stage.metrics["worlds"] = {w: ctx.vars[f"world.{w}"] for w in needed}
    for s in steps:
        run_step(ctx, s, d)
    ctx.stage.status = aggregate(ctx.stage.commands)


def stage_db_stop(ctx: Ctx) -> None:
    d = ctx.sdir()
    cfg = ctx.cfg.get("db_stop") or {}
    if not cfg.get("steps"):
        ctx.stage.status = NOINPUT
        return
    fp_sql = fingerprint_sql(ctx.cfg)
    if fp_sql:
        ctx.psql_select("fp_before_db_stop", fp_sql, d / "fp_before.txt")
    ok = run_bot_steps(ctx, cfg["steps"], d)
    if fp_sql and ok:
        ctx.run("compose_ps", ["docker", "compose", "ps", "--format", "{{.Name}} {{.Status}}"],
                stdout=d / "compose_ps.txt", stderr=d / "compose_ps.err", timeout=60, exit_map={0: PASS})
        ctx.psql_select("fp_after_db_stop", fp_sql, d / "fp_after.txt")
        if not ctx.dry:
            a = (d / "fp_before.txt").read_text(encoding="utf-8").strip() if (d / "fp_before.txt").is_file() else ""
            b = (d / "fp_after.txt").read_text(encoding="utf-8").strip() if (d / "fp_after.txt").is_file() else ""
            ctx.stage.metrics["fp"] = {"before": a, "after": b, "same": bool(a) and a == b}
            if a and b and a != b:
                ctx.stage.commands[-1].status = FAIL
                ctx.stage.notes.append("DB 정지 전후 증거 DB 지문이 다르다")
    ctx.stage.status = aggregate(ctx.stage.commands)


def stage_final(ctx: Ctx) -> None:
    d = ctx.sdir()
    ctx.run("porcelain_end", ["git", "status", "--porcelain"], stdout=d / "porcelain_end.txt",
            stderr=d / "porcelain_end.err")
    if not ctx.dry:
        if ctx.snapshot:
            end = sha_snapshot(source_files())
            (d / "src_sha_end.txt").write_text("".join(f"{h}  {p}\n" for p, (h, _) in sorted(end.items())),
                                               encoding="utf-8", newline="\n")
            changed = sorted(p for p in set(end) | set(ctx.snapshot)
                             if (end.get(p) or (None,))[0] != (ctx.snapshot.get(p) or (None,))[0])
            ctx.stage.metrics["source_files"] = len(end)
            ctx.stage.metrics["source_changed"] = changed
            c = Cmd("src_sha_compare", ["sha256", "freeze/src_sha_start.txt", "final/src_sha_end.txt"], ".", 0,
                    PASS if not changed else FAIL, log=rel(d / "src_sha_end.txt"))
            ctx.stage.commands.append(c)
        else:
            ctx.stage.notes.append("freeze 스냅샷이 없어 sha 재대조를 못 했다")
    fp_sql = fingerprint_sql(ctx.cfg)
    if fp_sql:
        c = ctx.psql_select("fp_after", fp_sql, d / "fp_after.txt")
        if not ctx.dry and c.status == PASS:
            after = (d / "fp_after.txt").read_text(encoding="utf-8").strip()
            ctx.stage.metrics["fp"] = {"before": ctx.fp_before, "after": after,
                                       "same": ctx.fp_before is not None and after == ctx.fp_before}
            if ctx.fp_before is None:
                ctx.stage.notes.append("freeze 의 지문이 없어 대조 못 함")
                c.status = ENV
            elif after != ctx.fp_before:
                c.status = FAIL
                ctx.stage.notes.append("증거 DB 지문이 시작과 다르다")
    ctx.stage.status = aggregate(ctx.stage.commands)


STAGE_FUNCS: dict = {"freeze": stage_freeze, "gates": stage_gates, "census": stage_census, "repeat": stage_repeat,
                     "unity": stage_unity, "filters": stage_filters, "sources": stage_sources,
                     "offline": stage_offline, "bots": stage_bots, "judges": stage_judges, "db_stop": stage_db_stop,
                     "final": stage_final}


# ================================================================ 실행기

def select_stages(only: str | None, skip: str | None, all_stages: list = STAGES) -> list[str]:
    o = {s.strip() for s in only.split(",") if s.strip()} if only else None
    k = {s.strip() for s in skip.split(",") if s.strip()} if skip else set()
    bad = ((o or set()) | k) - set(all_stages)
    if bad:
        raise SystemExit(f"모르는 단계: {sorted(bad)} (가능: {', '.join(all_stages)})")
    return [s for s in all_stages if (o is None or s in o or s in ALWAYS) and s not in k]


def human_gate(ctx: Ctx, name: str, message: str, approve: set, timeout: float, interactive: bool) -> str:
    """'go' | 'skip' | 'timeout'"""
    if name in approve:
        return "go"
    if ctx.dry:
        return "go"
    notice = (f"사람 단계 '{name}': {message}\n승인: 파일 {rel(ctx.out / ('go_' + name))} 를 만든다. "
              f"건너뜀: {rel(ctx.out / ('skip_' + name))}. 제한 {int(timeout)} 초.\n")
    (ctx.out / f"WAITING_{name}.txt").write_text(notice, encoding="utf-8")
    ctx.progress(f"!! 사람 단계 대기: {name} — {message}")
    if interactive:
        ans = input(f"{notice}지금 진행할까? [y/N] ").strip().lower()
        return "go" if ans in ("y", "yes") else "skip"
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if (ctx.out / f"go_{name}").exists():
            return "go"
        if (ctx.out / f"skip_{name}").exists():
            return "skip"
        time.sleep(1.0)
    return "timeout"


class Runner:
    def __init__(self, ctx: Ctx, selected: list[str], funcs: dict = STAGE_FUNCS, watcher: Watcher | None = None,
                 approve: set | None = None, gate_timeout: float = 1800.0, gates: dict = HUMAN_GATES,
                 all_stages: list = STAGES, interactive: bool = False, stage_watch: dict | None = None,
                 relax_unity: bool = True):
        self.ctx, self.selected, self.funcs, self.watcher = ctx, selected, funcs, watcher
        self.approve, self.gate_timeout, self.gates = approve or set(), gate_timeout, gates
        self.interactive = interactive
        self.stage_watch = STAGE_WATCH if stage_watch is None else stage_watch
        self.relax_unity = relax_unity
        self.all_stages = all_stages
        self.stages: dict = {s: Stage(s) for s in all_stages}
        self.started = now_iso()
        self.meta: dict = {}

    def summary(self, done: bool) -> dict:
        st = [self.stages[s] for s in self.all_stages]
        counts: dict = {}
        for s in st:
            counts[s.status] = counts.get(s.status, 0) + 1
        w = self.watcher
        return {"tool": "tests/e2e/run_round.py", "format": 1, **self.meta,
                "started": self.started, "finished": now_iso() if done else None, "complete": done,
                "watch": {"enabled": w is not None, "interval_s": w.interval if w else None,
                          "checks": w.checks if w else 0, "violations": w.violations if w else [],
                          "touched_same_content": w.touched_same_content if w else [],
                          "relax_unity": self.relax_unity,
                          "stage_category": {s: self.stage_watch.get(s, "client") for s in self.all_stages}},
                "counts": counts, "stages": [s.to_json() for s in st]}

    def write(self, done: bool) -> None:
        text = json.dumps(self.summary(done), ensure_ascii=False, indent=2)
        for v in self.ctx.secret_values:           # 방어: .env 값이 어떤 경로로든 들어가면 가린다
            text = text.replace(v, "<redacted>")
        p = self.ctx.out / "summary.json"
        tmp = p.with_suffix(".json.tmp")
        tmp.write_text(text + ("\nDONE\n" if done else "\n"), encoding="utf-8", newline="\n")
        os.replace(tmp, p)

    def run(self) -> dict:
        ctx = self.ctx
        if self.watcher:
            self.watcher.start()
        self.write(False)
        try:
            for name in self.all_stages:
                stg = self.stages[name]
                if name not in self.selected:
                    continue
                ctx.stage = stg
                ctx.vars["E"] = str(ctx.out / name)
                stg.started, stg.t0 = now_iso(), time.time()
                ctx.progress(f"== {name}")
                gate = self.gates.get(name)
                decision = human_gate(ctx, name, gate, self.approve, self.gate_timeout,
                                      self.interactive) if gate else "go"
                if decision != "go":
                    stg.status = HUMAN
                    stg.notes.append(f"사람 단계 승인 없음({decision}) — 돌리지 않았다")
                else:
                    try:
                        self.funcs[name](ctx)
                    except Exception as ex:  # 한 단계의 도구 오류가 라운드 전체를 멈추지 않게
                        stg.status = FAIL
                        stg.notes.append(f"실행기 예외: {ex.__class__.__name__}: {ex}")
                stg.t1 = time.time()
                stg.finished, stg.seconds = now_iso(), stg.t1 - stg.t0
                if self.watcher and not ctx.dry:
                    self.watcher.check()
                    v = self.watcher.overlapping(stg.t0, stg.t1, self.stage_watch.get(name, "client"), self.relax_unity)
                    if v:
                        stg.violations = v
                        stg.raw_status = stg.status
                        stg.status = INVALID
                ctx.progress(f"== {name}: {stg.status}" + (f" (원래 {stg.raw_status})" if stg.raw_status else ""))
                self.write(False)
        finally:
            if self.watcher:
                self.watcher.stop()
                # 마지막 점검에서 드러난 위반도 단계에 반영한다
                for s in self.stages.values():
                    if s.t1 and s.status != INVALID:
                        v = self.watcher.overlapping(s.t0, s.t1, self.stage_watch.get(s.name, "client"), self.relax_unity)
                        if v:
                            s.violations, s.raw_status, s.status = v, s.status, INVALID
            self.write(True)
            ctx.progress("DONE")
            shutil.rmtree(ctx.tmp, ignore_errors=True)
        return self.summary(True)


# ================================================================ 설정

def resolve_slice(slice_arg: str) -> tuple[str, str]:
    """`p1-02` 또는 `p1-02-mining` → (폴더 이름, 라벨 `p1-02`)."""
    ws = REPO / "_workspace"
    if (ws / slice_arg).is_dir():
        d = slice_arg
    else:
        c = sorted(p.name for p in ws.glob(f"{slice_arg}-*") if p.is_dir())
        if len(c) != 1:
            raise SystemExit(f"슬라이스 폴더를 하나로 정할 수 없다: {slice_arg} → {c}")
        d = c[0]
    m = re.match(r"^(p\d+-\d+)", d)
    return d, (m.group(1) if m else d)


def load_config(path: Path | None, slice_dir: str, label: str) -> dict:
    p = path or (E2E / "round_configs" / f"{slice_dir}.json")
    cfg = json.loads(p.read_text(encoding="utf-8")) if p.is_file() else {}
    cfg["_config_path"] = rel(p) if p.is_file() else None
    cfg["slice_dir"] = slice_dir
    cfg.setdefault("slice_label", label)
    return cfg


# ================================================================ selftest

def selftest() -> int:
    tmp = Path(tempfile.mkdtemp(prefix="run_round_selftest_"))
    src = tmp / "src"
    src.mkdir()
    (src / "a.rs").write_text("fn a() {}\n", encoding="utf-8")
    (src / "b.rs").write_text("fn b() {}\n", encoding="utf-8")
    py = sys.executable
    secret = "s3cr3t-value-for-selftest"

    class A:  # args 대역
        contract, test_log, unity_xml, repeat = "x.md", None, [], 1

    def files_fn():
        return sorted(src.glob("*.rs"))

    def snap_fn():
        return sha_snapshot(files_fn(), tmp)

    def mk_ctx(name: str) -> Ctx:
        out = tmp / name
        out.mkdir()
        c = Ctx(out, A(), {"slice_dir": "selftest", "slice_label": "p0-00"}, {"FAKE_SECRET": secret})
        c.quiet = True
        return c

    def cmd_stage(argv, exit_map=None):
        def f(ctx):
            ctx.run("c", argv, exit_map=exit_map)
            ctx.stage.status = aggregate(ctx.stage.commands)
        return f

    toucher = f"import time,pathlib;time.sleep(0.6);pathlib.Path(r'{src / 'a.rs'}').write_text('fn a2() {{}}\\n');time.sleep(0.6)"
    marker = tmp / "gate_ran.txt"
    funcs = {
        "ok": cmd_stage([py, "-c", "print('ok')"]),
        "bad": cmd_stage([py, "-c", "import sys;sys.exit(1)"]),
        "touch": cmd_stage([py, "-c", toucher]),
        "ok2": cmd_stage([py, "-c", "import time;time.sleep(0.3)"]),
        "env": cmd_stage([py, "-c", "import sys;sys.exit(2)"], E2E_EXIT),
        "gated": cmd_stage([py, "-c", f"open(r'{marker}','w').write('x')"]),
        "leak": cmd_stage([py, "-c", "import os;print(os.environ.get('FAKE_SECRET'))"]),
    }
    order = list(funcs)

    def run_case(name, watch: bool, lister=lambda: [], approve=None, stage_watch=None, relax=True):
        ctx = mk_ctx(name)
        (src / "a.rs").write_text("fn a() {}\n", encoding="utf-8")
        base = snap_fn()
        w = Watcher(files_fn, base, interval=0.2, lister=lister, root=tmp) if watch else None
        r = Runner(ctx, order, funcs=funcs, watcher=w, approve=approve or set(), gate_timeout=0.5,
                   gates={"gated": "selftest 사람 단계"}, all_stages=order,
                   stage_watch=stage_watch if stage_watch is not None else {}, relax_unity=relax)
        mid = {}
        orig_write = r.write

        def spy(done):
            orig_write(done)
            if not done and "after_ok" not in mid and r.stages["ok"].status == PASS:
                mid["after_ok"] = (ctx.out / "summary.json").read_text(encoding="utf-8")
        r.write = spy
        r.run()
        return ctx, (ctx.out / "summary.json").read_text(encoding="utf-8")

    ctx1, text1 = run_case("watch_on", True)
    s1 = load_summary(ctx1.out / "summary.json")
    st1 = {s["name"]: s for s in s1["stages"]}
    ctx2, text2 = run_case("watch_off", False)
    st2 = {s["name"]: s for s in load_summary(ctx2.out / "summary.json")["stages"]}
    marker_after_unapproved = marker.exists()
    ctx3, _ = run_case("approved", False, approve={"gated"})
    st3 = {s["name"]: s for s in load_summary(ctx3.out / "summary.json")["stages"]}
    marker_after_approved = marker.exists()

    ext_pid = 999999
    ctx4, _ = run_case("ext_proc", True, lister=lambda: [{"pid": ext_pid, "ppid": 1, "name": "cargo.exe",
                                                          "cmd": "cargo check"}])
    st4 = {s["name"]: s for s in load_summary(ctx4.out / "summary.json")["stages"]}
    ctx5, _ = run_case("own_proc", True, lister=lambda: [{"pid": os.getpid(), "ppid": 1, "name": "python", "cmd": ""},
                                                         {"pid": 424242, "ppid": os.getpid(), "name": "cargo.exe",
                                                          "cmd": "cargo test"},
                                                         {"pid": 31337, "ppid": 1, "name": "Unity Hub.exe", "cmd": ""}])
    st5 = {s["name"]: s for s in load_summary(ctx5.out / "summary.json")["stages"]}

    # R6 완화: client/ 를 보는 Editor 가 떠 있을 때 — server 범주(ok)는 유효, client 범주(ok2)는 무효
    editor = [{"pid": 51515, "ppid": 1, "name": "Unity.exe", "cmd": "Unity.exe -projectPath C:/x/client"},
              {"pid": 51516, "ppid": 1, "name": "Unity.exe",
               "cmd": "Unity.exe -batchMode -name AssetImportWorker0 -projectPath C:/x/client"}]
    cats = {"ok": "server", "bad": "server", "touch": "server", "ok2": "client", "env": "server",
            "gated": "server", "leak": "server"}
    ctx7, _ = run_case("editor_relaxed", True, lister=lambda: editor, stage_watch=cats)
    st7 = {s["name"]: s for s in load_summary(ctx7.out / "summary.json")["stages"]}
    ctx8, _ = run_case("editor_strict", True, lister=lambda: editor, stage_watch=cats, relax=False)
    st8 = {s["name"]: s for s in load_summary(ctx8.out / "summary.json")["stages"]}
    ctx9, _ = run_case("editor_plus_cargo", True, lister=lambda: editor + [
        {"pid": 61616, "ppid": 1, "name": "cargo.exe", "cmd": "cargo check"}], stage_watch=cats)
    st9 = {s["name"]: s for s in load_summary(ctx9.out / "summary.json")["stages"]}

    # 방어 가림: 단계 메모에 비밀값이 들어간 경우
    rctx = mk_ctx("redact")
    rr = Runner(rctx, ["ok"], funcs={"ok": lambda c: c.stage.notes.append(f"oops {secret}")}, all_stages=["ok"])
    rr.run()
    rtext = (rctx.out / "summary.json").read_text(encoding="utf-8")
    redacted_ok = secret not in rtext and "oops <redacted>" in rtext

    # dry-run: 없는 실행 파일 · 없는 스크립트를 찾아낸다
    dctx = mk_ctx("dry")
    dctx.dry = True
    dctx.stage = Stage("d")
    dc = dctx.run("x", ["definitely-not-a-real-exe-zz", str(E2E / "no_such_script_zz.py")])
    dc_ok = dctx.run("y", [py, str(E2E / "run_round.py")])

    lines = text1.rstrip("\n").split("\n")
    keys_ok = all(k in s1 for k in ("started", "finished", "complete", "watch", "counts", "stages")) and all(
        all(k in s for k in ("name", "status", "commands", "seconds", "started", "finished")) for s in s1["stages"]) \
        and all(all(k in c for k in ("argv", "rc", "seconds", "log", "status")) for s in s1["stages"]
                for c in s["commands"])
    cases = [
        ("summary.json 마지막 줄이 정확히 DONE", lines[-1] == "DONE"),
        ("DONE 줄은 하나뿐이고 JSON 은 raw_decode 로 읽힌다", lines.count("DONE") == 1 and s1["complete"] is True),
        ("단계·명령 키(상태·명령·시간·로그)", keys_ok),
        ("성공 단계 → PASS", st1["ok"]["status"] == PASS),
        ("실패 단계 → FAIL 이고 다음 단계도 돈다", st1["bad"]["status"] == FAIL and st1["ok2"]["started"]),
        ("종료 2(e2e 규약) → 미검증(환경)", st1["env"]["status"] == ENV),
        ("동결 위반(실행 중 소스 수정) → 그 단계 무효, 원래 상태 보존",
         st1["touch"]["status"] == INVALID and st1["touch"]["raw_status"] == PASS
         and any(v.get("path", "").endswith("a.rs") for v in st1["touch"]["violations"])),
        ("위반 뒤 단계는 무효가 아니다(겹친 단계만)", st1["ok2"]["status"] == PASS and st1["ok"]["status"] == PASS),
        ("음성 대조: 감시를 끄면 같은 수정이 무효로 잡히지 않는다", st2["touch"]["status"] == PASS),
        ("외부 cargo 프로세스 → 겹친 단계 무효", st4["ok"]["status"] == INVALID
         and any(v["kind"] == "external_process" for v in st4["ok"]["violations"])),
        ("음성 대조: 실행기의 자손 cargo · 외부 Unity Hub 는 위반이 아니다", st5["ok"]["status"] == PASS),
        ("사람 단계: 승인 없으면 멈췄다가 미검증(사람 대기), 명령은 안 돈다",
         st1["gated"]["status"] == HUMAN and not marker_after_unapproved and (ctx1.out / "WAITING_gated.txt").exists()),
        ("사람 단계: 승인(--approve)이면 돈다", st3["gated"]["status"] == PASS and marker_after_approved),
        (".env 값은 자식 환경에는 들어가고(자식 로그로 확인) summary 에는 나오지 않는다",
         secret in (ctx1.out / "leak" / "c.log").read_text(encoding="utf-8") and secret not in text1
         and secret not in text2),
        ("방어: 요약 문구에 .env 값이 섞여도 가린다", redacted_ok),
        ("dry-run: 없는 실행 파일·스크립트를 찍는다", len(dc.missing) == 2 and not dc_ok.missing),
        ("R6 완화: Editor·AssetImportWorker 가 떠 있어도 server 범주 단계는 유효",
         st7["ok"]["status"] == PASS and st7["leak"]["status"] == PASS),
        ("R6 완화: 같은 Editor 가 client 범주 단계(unity·codegen)는 무효로 한다",
         st7["ok2"]["status"] == INVALID and all(v.get("proc_class") == "unity" for v in st7["ok2"]["violations"])),
        ("R6 완화: server 단계도 소스 변화는 여전히 무효", st7["touch"]["status"] == INVALID),
        ("음성 대조: 완화를 끄면(--strict-unity) Editor 가 server 단계도 무효로 한다",
         st8["ok"]["status"] == INVALID and st8["leak"]["status"] == INVALID),
        ("R6 완화와 무관하게 외부 cargo 는 server 단계도 무효", st9["ok"]["status"] == INVALID
         and any(v.get("proc_class") == "cargo" for v in st9["ok"]["violations"])),
        ("모든 실행 단계에 감시 범주가 명시돼 있다", set(STAGE_WATCH) == set(STAGES)
         and set(STAGE_WATCH.values()) <= {"server", "client"}
         and STAGE_WATCH["unity"] == STAGE_WATCH["offline"] == "client"
         and all(STAGE_WATCH[s] == "server" for s in ("gates", "census", "repeat", "filters", "sources",
                                                        "bots", "judges"))),
        ("--only 는 freeze·final 을 늘 포함", select_stages("gates,census", None) == ["freeze", "gates", "census", "final"]),
        ("--skip 은 freeze 도 뺄 수 있다", "freeze" not in select_stages(None, "freeze,bots")),
        ("SQL 가드: SELECT 만", sql_is_read_only("select 1") and not sql_is_read_only("update x set y=1")
         and not sql_is_read_only("select 1; delete from t") and not sql_is_read_only("with a as (delete from t) select 1")),
        ("compose 가드: down 금지", not compose_argv_is_safe(["docker", "compose", "down", "-v"])
         and compose_argv_is_safe(["docker", "compose", "exec", "-T", "postgres"])),
        ("STARFALL_REPLAY_BLESS 는 자식 환경에서 지운다", "STARFALL_REPLAY_BLESS" not in base_env({"STARFALL_REPLAY_BLESS": "1"})),
    ]
    # 중간 스냅샷에 DONE 이 없다(기다리는 쪽이 일찍 깨지 않는다)
    ctx6 = mk_ctx("mid")
    r6 = Runner(ctx6, ["ok"], funcs={"ok": funcs["ok"]}, all_stages=["ok"])
    r6.write(False)
    cases.append(("중간 summary 에는 DONE 줄이 없다",
                  "DONE" not in (ctx6.out / "summary.json").read_text(encoding="utf-8").split("\n")))
    shutil.rmtree(tmp, ignore_errors=True)
    bad = 0
    for name, ok in cases:
        print(("OK   " if ok else "FAIL ") + name)
        bad += 0 if ok else 1
    print(f"selftest: {'PASS' if not bad else f'FAIL ({bad})'}  케이스={len(cases)}")
    return 0 if not bad else 1


# ================================================================ main

def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("mode", nargs="?", choices=["selftest"])
    ap.add_argument("--slice", help="p1-02 또는 p1-02-mining")
    ap.add_argument("--contract")
    ap.add_argument("--round", help="r1, r2, … (출력 폴더 이름)")
    ap.add_argument("--out", help="기본 _workspace/{slice}/evidence/{round}_{YYYYMMDD}")
    ap.add_argument("--config", help="기본 tests/e2e/round_configs/{slice 폴더}.json")
    ap.add_argument("--only")
    ap.add_argument("--skip")
    ap.add_argument("--repeat", type=int, default=10)
    ap.add_argument("--test-log", help="census·bots(SC-07) 가 쓸 cargo test 로그(gates 를 안 돌릴 때)")
    ap.add_argument("--unity-xml", action="append", default=[])
    ap.add_argument("--approve", action="append", default=[], help="미리 승인할 사람 단계(예: db_stop)")
    ap.add_argument("--gate-timeout", type=float, default=1800.0)
    ap.add_argument("--watch-interval", type=float, default=15.0)
    ap.add_argument("--no-watch", action="store_true")
    ap.add_argument("--strict-unity", action="store_true",
                    help="R6 완화를 끈다 — Unity 프로세스도 모든 단계의 무효 사유(STAGE_WATCH 무시)")
    ap.add_argument("--dry-run", action="store_true")
    a = ap.parse_args()
    if a.mode == "selftest":
        return selftest()
    if not (a.slice and a.contract and a.round):
        ap.error("--slice --contract --round 가 필요하다")
    slice_dir, label = resolve_slice(a.slice)
    cfg = load_config(Path(a.config) if a.config else None, slice_dir, label)
    a.contract = str(Path(a.contract).resolve())
    if not Path(a.contract).is_file():
        ap.error(f"계약 파일 없음: {a.contract}")
    out = Path(a.out) if a.out else REPO / "_workspace" / slice_dir / "evidence" / \
        f"{a.round}_{dt.date.today().strftime('%Y%m%d')}"
    out = out.resolve()
    if (out / "summary.json").exists():
        ap.error(f"이미 summary.json 이 있다(증거를 덮지 않는다): {out}")
    out.mkdir(parents=True, exist_ok=True)
    selected = select_stages(a.only, a.skip)
    ctx = Ctx(out, a, cfg, load_dotenv(), dry=a.dry_run)
    ctx.vars.update({"round": a.round, "contract": a.contract, "slice_label": cfg["slice_label"],
                     "tmp": str(ctx.tmp), "test_log": str(test_log_path(ctx)), "python": sys.executable})
    watcher = None
    if not a.no_watch and not a.dry_run:
        watcher = Watcher(source_files, None, interval=a.watch_interval)
    runner = Runner(ctx, selected, watcher=watcher, approve=set(a.approve), gate_timeout=a.gate_timeout,
                    interactive=bool(sys.stdin and sys.stdin.isatty()), relax_unity=not a.strict_unity)
    runner.meta = {"slice": slice_dir, "slice_label": cfg["slice_label"], "round": a.round,
                   "contract": rel(a.contract), "config": cfg.get("_config_path"), "out": rel(out),
                   "selected": selected, "dry_run": a.dry_run,
                   "argv": [rel(x) if os.path.isabs(x) else x for x in sys.argv[1:]],
                   "env_keys_loaded": sorted(ctx.dotenv)}
    if watcher:
        # 기준선은 freeze 단계의 스냅샷 — freeze 가 끝난 뒤 감시를 붙인다
        orig = STAGE_FUNCS["freeze"]

        def freeze_then_watch(c):
            orig(c)
            watcher.baseline = c.snapshot or sha_snapshot(source_files())
        runner.funcs = dict(STAGE_FUNCS, freeze=freeze_then_watch)
        if "freeze" not in selected:
            watcher.baseline = sha_snapshot(source_files())
    s = runner.run()
    print(json.dumps(s["counts"], ensure_ascii=False))
    if a.dry_run:
        miss = [(st["name"], c["name"], m) for st in s["stages"] for c in st["commands"] for m in c.get("missing", [])]
        for m in miss:
            print("  없음:", *m)
        return 1 if any("빌드" not in m[2] for m in miss) else 0
    return 0 if not any(st["status"] not in (PASS, SKIP) for st in s["stages"]) else 1


if __name__ == "__main__":
    sys.exit(main())
