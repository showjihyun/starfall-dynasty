"""p1-02 재기동·재접속 계열 채굴 케이스 오케스트레이터 — **준비·조율 도구이고 verdict 를 내지 않는다.**

판정은 각 케이스의 마지막 단계 봇(`bots probe --case …`)이 낸다(계약이 판정 도구로 지명). 이 도구가
하는 일은 봇이 할 수 없는 것뿐이다: 새 월드 만들기, 서버 기동·**stdin `shutdown`** 종료(하드 킬
금지)·재기동, 기동 전 동결·바이너리 훅 검사(`server_boot.spawn`), 그리고 SC-25 의 **인벤토리 변조**
(`UPDATE inventory_items` — 계약 SC-25 방법 칸 "qa 가 `UPDATE inventory ...`", 이 실행이 만든 새
월드의 한 행만).

    python tests/e2e/restart_cases.py --out <DIR> dup       # SC-78 (b)(c) · SC-108 (재기동 변형 포함)
    python tests/e2e/restart_cases.py --out <DIR> cas       # SC-25 (대조 실행 + 변조 실행) · SC-26

각 단계의 봇 JSON·종료 코드·서버 로그를 `<DIR>` 에 남기고, 마지막에 `summary.json` 에 **봇이 낸**
판정을 모은다(이 도구가 판정을 바꾸지 않는다).
"""
from __future__ import annotations

import argparse
import json
import subprocess
import sys
import time
import urllib.error
from pathlib import Path

import db
import server_boot as sb

REPO = sb.REPO
BOTS = REPO / "tools" / "bots" / "target" / "debug" / "bots.exe"


def new_world(out: Path, tag: str) -> str:
    ev = out / f"world_{tag}.json"
    subprocess.run([sys.executable, str(REPO / "tests/e2e/new_world.py"), "--tag", tag, "--evidence", str(ev)],
                   check=True, capture_output=True, text=True, encoding="utf-8", errors="replace")
    return json.loads(ev.read_text(encoding="utf-8"))["world_id"]


class Server:
    def __init__(self, world: str, log: Path):
        self.env = sb.load_dotenv()
        self.env["STARFALL_WORLD_ID"] = world
        self.addr = self.env.get("STARFALL_HTTP_ADDR", "127.0.0.1:8080")
        self.log = log
        self.proc, self.buf = sb.spawn(self.env)   # 동결·훅 검사 포함
        deadline = time.monotonic() + 40
        while time.monotonic() < deadline and self.proc.poll() is None:
            try:
                sb.http_json(f"http://{self.addr}/debug/stats")
                return
            except (urllib.error.URLError, OSError, TimeoutError):
                time.sleep(0.3)
        raise RuntimeError("서버가 준비되지 않았다: " + "".join(self.buf)[-2000:])

    def _save(self) -> None:
        sb._drain_settle(self.proc)
        self.log.write_text("".join(self.buf), encoding="utf-8")

    def stop(self) -> int:
        rc = sb.graceful_shutdown(self.proc)
        self._save()
        return rc

    def wait_exit(self, timeout: float) -> int | None:
        try:
            self.proc.wait(timeout=timeout)
        except subprocess.TimeoutExpired:
            return None
        self._save()
        return self.proc.returncode


def bot(out: Path, name: str, case: str, deposit: str, label: str, *extra: str) -> tuple[int, dict]:
    """봇 한 단계. 종료 코드와 JSON 을 그대로 돌려준다(해석하지 않는다)."""
    j = out / f"{name}.json"
    cmd = [str(BOTS), "probe", "--case", case, "--deposit", deposit, "--label", label, *extra, "--out", str(j)]
    env = sb.load_dotenv()
    p = subprocess.run(cmd, cwd=str(REPO), env=env, capture_output=True, text=True, encoding="utf-8",
                       errors="replace", timeout=900)
    (out / f"{name}.err").write_text(p.stderr, encoding="utf-8")
    data = json.loads(j.read_text(encoding="utf-8")) if j.exists() else {"error": "봇 JSON 없음"}
    print(f"  {name}: exit={p.returncode} verdict={data.get('verdict')}", flush=True)
    return p.returncode, data


def scenario_dup(out: Path) -> dict:
    res: dict = {}
    w = new_world(out, "Q2-dup")
    res["world_id"] = w
    s = Server(w, out / "server_1.log")
    try:
        res["mine-dup-reconnect"] = bot(out, "mine-dup-reconnect", "mine-dup-reconnect", "vela-orbit-2", "bot-040")
        res["mine-dup-restart.run"] = bot(out, "mine-dup-restart.run", "mine-dup-restart", "vela-orbit-1", "bot-050",
                                          "--phase", "run", "--state", str(out / "restart_state.json"), "--world", w)
        res["mine-dup-cross-actor.run"] = bot(out, "mine-dup-cross-actor.run", "mine-dup-cross-actor", "inner-belt-1",
                                              "bot-051", "--label-b", "bot-052", "--phase", "run",
                                              "--state", str(out / "cross_state.json"), "--world", w)
    finally:
        res["server_1_exit"] = s.stop()
    s2 = Server(w, out / "server_2.log")
    try:
        # 재기동 로그 줄("tick 을 이어서 시작한다 … last_tick=Some(")은 기동 직후 이미 버퍼에 있다.
        s2.log.write_text("".join(s2.buf), encoding="utf-8")
        res["mine-dup-restart.resume"] = bot(out, "mine-dup-restart.resume", "mine-dup-restart", "vela-orbit-1",
                                             "bot-050", "--phase", "resume", "--state",
                                             str(out / "restart_state.json"), "--world", w,
                                             "--restart-log", str(s2.log))
        res["mine-dup-cross-actor.resume"] = bot(out, "mine-dup-cross-actor.resume", "mine-dup-cross-actor",
                                                 "inner-belt-1", "bot-051", "--label-b", "bot-053", "--phase",
                                                 "resume", "--state", str(out / "cross_state.json"), "--world", w,
                                                 "--restart-log", str(s2.log))
    finally:
        res["server_2_exit"] = s2.stop()
    return res


def _inventory_row(w: str) -> tuple[str, str, int]:
    rows = db.psql_rows_strict(f"select actor_id, mineral_id, quantity_kg from inventory_items where world_id = '{w}';")
    if len(rows) != 1:
        raise db.QueryError(f"인벤토리 행이 정확히 1 이어야 한다(변조 대상 특정): {rows}")
    a, m, q = rows[0]
    return a, m, int(q)


def _cas_run(out: Path, tag: str, tamper: bool) -> dict:
    res: dict = {}
    w = new_world(out, tag)
    res["world_id"] = w
    ready, tampered = out / f"{tag}.ready", out / f"{tag}.tampered"
    for f in (ready, tampered):
        f.unlink(missing_ok=True)
    s = Server(w, out / f"{tag}_server.log")
    state = out / f"{tag}_state.json"
    p = subprocess.Popen([str(BOTS), "probe", "--case", "cas-halt", "--deposit", "vela-orbit-2", "--label", "bot-060",
                          "--phase", "run", "--state", str(state), "--world", w, "--ready-file", str(ready),
                          "--tampered-file", str(tampered), "--out", str(out / f"{tag}_bot.json")],
                         cwd=str(REPO), env=sb.load_dotenv(), stdout=subprocess.DEVNULL, stderr=subprocess.PIPE,
                         text=True, encoding="utf-8", errors="replace")
    deadline = time.monotonic() + 400
    while not ready.exists() and time.monotonic() < deadline and p.poll() is None:
        time.sleep(0.3)
    if not ready.exists():
        res["error"] = "봇이 ready 를 만들지 않았다"
    else:
        # 변조 **전** 채굴이 커밋됐다(계약 SC-25 ⊘): processed_commands 에 1 행.
        deadline2 = time.monotonic() + 10
        committed = 0
        while time.monotonic() < deadline2 and committed == 0:
            committed = db.scalar_int_strict(f"select count(*) from processed_commands where world_id = '{w}';")
            time.sleep(0.3)
        res["x1_committed_rows"] = committed
        actor, mineral, q = _inventory_row(w)
        res["pre_tamper"] = {"actor_id": actor, "mineral_id": mineral, "quantity_kg": q}
        if tamper:
            tv = q + 777
            db.psql_strict(f"update inventory_items set quantity_kg = {tv} where world_id = '{w}' "
                           f"and actor_id = '{actor}' and mineral_id = '{mineral}';")
            res["tampered_value"] = tv
        tampered.write_text("go\n", encoding="utf-8")
    p.wait(timeout=600)
    res["bot_run_exit"] = p.returncode
    if tamper:
        rc = s.wait_exit(60)
        res["server_exit_code"] = rc if rc is not None else s.stop()
        res["server_halted_itself"] = rc is not None
    else:
        res["server_exit_code"] = s.stop()
    res["state"] = str(state)
    res["log"] = str(s.log)
    return res


def scenario_cas(out: Path) -> dict:
    res: dict = {}
    res["control"] = _cas_run(out, "cas-control", tamper=False)
    res["tamper"] = _cas_run(out, "cas-tamper", tamper=True)
    t, c = res["tamper"], res["control"]
    tv = t.get("tampered_value")
    res["cas-halt"] = bot(out, "cas-halt.judge", "cas-halt", "vela-orbit-2", "bot-060", "--phase", "judge",
                          "--world", t["world_id"], "--state", t["state"], "--server-log", t["log"],
                          "--exit-code", str(t["server_exit_code"]), "--control-state", c["state"],
                          "--control-log", c["log"], "--control-exit-code", str(c["server_exit_code"]),
                          *(["--tampered-value", str(tv)] if tv is not None else []))
    # SC-26: 같은(정지한) 월드로 재기동 → 적재 → 채굴.
    s = Server(t["world_id"], out / "cas-reload_server.log")
    try:
        s.log.write_text("".join(s.buf), encoding="utf-8")
        res["cas-reload"] = bot(out, "cas-reload", "cas-reload", "vela-orbit-2", "bot-060", "--world", t["world_id"],
                                "--halt-state", t["state"], "--restart-log", str(s.log),
                                "--exit-code", str(t["server_exit_code"]),
                                *(["--tampered-value", str(tv)] if tv is not None else []))
    finally:
        res["reload_server_exit"] = s.stop()
    return res


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--out", required=True)
    ap.add_argument("scenario", choices=["dup", "cas"])
    a = ap.parse_args()
    out = Path(a.out)
    out.mkdir(parents=True, exist_ok=True)
    if not BOTS.is_file():
        print(f"미검증(환경): {BOTS} 가 없다 — tools/bots 에서 cargo build", file=sys.stderr)
        return 2
    res = scenario_dup(out) if a.scenario == "dup" else scenario_cas(out)
    res["provenance"] = sb.source_provenance()
    (out / "summary.json").write_text(json.dumps(res, ensure_ascii=False, indent=2, default=str), encoding="utf-8")
    print(json.dumps({k: (v[1].get("verdict") if isinstance(v, tuple) else None) for k, v in res.items()
                      if isinstance(v, tuple)}, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    sys.exit(main())
