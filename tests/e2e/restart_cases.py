"""재기동·재접속·변조 계열 오케스트레이터. p1-02 채굴(dup·cas)은 **준비·조율만** 하고 판정은 봇이 낸다.
p1-03 거래(sell-dup·sell-cas·sell-cas-market·sell-recover)는 **이 도구가 판정한다**(계약 §3.2 — SC-34·35·36·51·112,
`summary.json` 의 `judgement*`). selftest: `python tests/e2e/restart_cases.py selftest`.

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


# ── p1-03 거래 (계약 SC-34 · 35 · 36 · 51 · 112) ─────────────────────────────────
#
# p1-02 와 달리 **이 도구가 판정한다**(계약 §3.2 — 이 도구가 지명 도구). 봇 단계(`sell-*`)는 관측용
# Reference 이고, 판정 입력은 봇 사실 JSON + 서버 종료 코드·로그 + SQL(새 월드 한정, SELECT 만 — 변조
# UPDATE 한 문장 제외).

P103_LABELS = {
    "sell-dup": {"item": "p1-03 SC-112 (AC-18g·AC-5c) 정상 종료 → 재기동 → 같은 판매 재전송 → DUPLICATE_COMMAND_ID, 지갑·processed_commands 불변"},
    "sell-cas": {"item": "p1-03 SC-34 (AC-6b) 지갑 행 SQL 변조 뒤 판매 → 서버 스스로 정지, 그 판매 미커밋 (대조 실행 생존)"},
    "sell-cas-reload": {"item": "p1-03 SC-36 (AC-6c) 재기동하면 변조 값으로 적재, 다음 판매의 wallet_before = 변조 값"},
    "sell-cas-market": {"item": "p1-03 SC-35 (AC-6b) market_states 행 SQL 변조 뒤 판매 → 서버 스스로 정지, 그 판매 미커밋"},
    "sell-recover": {"item": "p1-03 SC-51 (AC-11d) 정상 재기동 → 포화·마지막 거래 보존, 재기동 뒤 첫 배수 tick 에 식대로 회복"},
}
RECOVERY_KG_FERROSITE = 1000      # 디자인 §3.6 (계약 §0.6) — 회복량/60 s
RECOVERY_INTERVAL_TICKS = 1200    # 60 s × 20 Hz
HALT_LOG_MARKERS = ("비교 후 쓰기", "CompareAndSetMismatch", "compare_and_set", "CAS")


def _sell_ok(facts: dict, i: int) -> bool:
    s = (facts.get("sells") or [])[i:i + 1]
    return bool(s) and any(r.get("status") == "ACCEPTED" for r in s[0].get("results") or [])


def judge_sell_dup_restart(run: dict, resume: dict, pc_before: int | None, pc_after: int | None,
                           server1_exit: int | None) -> dict:
    """⊘: 첫 전송 수락·지갑 증가, 정상 종료(exit 0). 판정: 재전송 사유 DUPLICATE, 재기동 뒤 첫 지갑 = 재기동 전 잔액,
    재전송 뒤 영수증 0, processed_commands 행 수 불변."""
    bal = (run.get("wallets") or [{}])[-1].get("balance_cr")
    first_wallet = (resume.get("wallets") or [{}])[0].get("balance_cr")
    rs = ((resume.get("sells") or [{}])[0].get("results") or [{}])[0]
    cond = _sell_ok(run, 0) and (bal or 0) > 0 and server1_exit == 0
    ok = (rs.get("reason") == "DUPLICATE_COMMAND_ID" and first_wallet == bal and not resume.get("receipts")
          and pc_before is not None and pc_before == pc_after)
    return {"verdict": ("PASS" if ok else "FAIL") if cond else "미검증(조건 미발생)",
            "first_sell_accepted": _sell_ok(run, 0), "balance_before_restart": bal, "server1_exit": server1_exit,
            "resend_reason": rs.get("reason"), "wallet_after_restart": first_wallet,
            "receipts_after_resend": len(resume.get("receipts") or []),
            "processed_commands_before_after": [pc_before, pc_after]}


def judge_sell_cas(ctrl: dict, tamp: dict, which: str) -> dict:
    """⊘: 변조 전 판매 수락·커밋(행 ≥ 1), 변조 값 ≠ 원래 값. 판정: 서버 스스로 0 아닌 코드로 종료,
    로그에 비교 후 쓰기 실패와 그 표, 변조 뒤 판매의 MINERAL_SOLD 가 DB 에 없음. 양성 짝: 대조 실행 생존(exit 0)."""
    cond = (tamp.get("pre_committed_sales", 0) >= 1 and tamp.get("tampered_value") is not None
            and tamp.get("tampered_value") != tamp.get("original_value"))
    log = tamp.get("halt_log_lines") or []
    ok = (tamp.get("server_halted_itself") is True and tamp.get("server_exit_code") not in (0, None)
          and any(which in l for l in log) and tamp.get("sold_rows_after_tamper") == 0
          and ctrl.get("server_exit_code") == 0 and ctrl.get("sold_rows_after_tamper", 0) >= 1)
    return {"verdict": ("PASS" if ok else "FAIL") if cond else "미검증(조건 미발생)",
            "tampered_table": which, "control": {k: ctrl.get(k) for k in ("server_exit_code", "sold_rows_after_tamper")},
            "tamper": {k: tamp.get(k) for k in ("pre_committed_sales", "original_value", "tampered_value",
                                                  "server_halted_itself", "server_exit_code", "sold_rows_after_tamper")},
            "halt_log_lines": log[:5]}


def judge_sell_cas_reload(tamp: dict, reload_facts: dict, wallet_before_db: int | None,
                          new_sold_rows: int | None = None, reload_server_exit: int | None = None) -> dict:
    """판정: 재기동 뒤 판매가 수락되고 **커밋됐고**(새 MINERAL_SOLD 1 행 — 수락 결과만으론 부족하다, 2026-10-07
    실측: 수락 뒤 커밋 실패로 서버가 정지한 사례), 그 행의 wallet_before = 변조 값, 서버 정상 종료."""
    cond = tamp.get("tampered_value") is not None and tamp.get("server_halted_itself") is True
    ok = (_sell_ok(reload_facts, 0) and new_sold_rows == 1 and wallet_before_db == tamp.get("tampered_value")
          and reload_server_exit == 0)
    return {"verdict": ("PASS" if ok else "FAIL") if cond else "미검증(조건 미발생)",
            "tampered_value": tamp.get("tampered_value"), "reload_sell_accepted": _sell_ok(reload_facts, 0),
            "mineral_sold_wallet_before_cr": wallet_before_db, "new_sold_rows_after_restart": new_sold_rows,
            "reload_server_exit": reload_server_exit}


def judge_sell_recover(run: dict, resume: dict) -> dict:
    """⊘: 재기동 직전 포화 > 0(회복이 아직 끝나지 않음). 판정: 재기동 뒤 첫 MARKET_STATE 의 포화·last_sale =
    재기동 전 마지막 값, 그다음 바뀐 값 = 식 max(0, s − 1000 × (⌊t/1200⌋ − ⌊as_of/1200⌋)) — 실제 tick 으로."""
    pre = (run.get("ferrosite_market") or [None])[-1]
    post = resume.get("ferrosite_market") or []
    first = post[0] if post else None
    nxt = next((m for m in post[1:] if first and m["saturation_kg"] != first["saturation_kg"]), None)
    cond = bool(pre) and pre["saturation_kg"] > 0 and first is not None
    expect = None
    if first and nxt:
        as_of = first["tick"]
        expect = max(0, first["saturation_kg"] - RECOVERY_KG_FERROSITE *
                     (nxt["tick"] // RECOVERY_INTERVAL_TICKS - as_of // RECOVERY_INTERVAL_TICKS))
    ok = (cond and first["saturation_kg"] == pre["saturation_kg"] and first.get("last_sale") == pre.get("last_sale")
          and nxt is not None and nxt["saturation_kg"] == expect)
    return {"verdict": ("PASS" if ok else "FAIL") if cond else "미검증(조건 미발생)",
            "before_restart_last": pre, "after_restart_first": first, "after_restart_next_change": nxt,
            "expected_next_by_formula": expect,
            "note": "식의 as_of 는 재기동 뒤 첫 메시지 tick(그 시점 실효 포화) — 배수 경계를 실제 tick 으로 센다"}


def _sold_rows(w: str) -> int:
    return db.scalar_int_strict(f"select count(*) from domain_events where world_id = '{w}' and event_type = 'MINERAL_SOLD';")


def _bot_json(out: Path, name: str) -> dict:
    p = out / f"{name}.json"
    return json.loads(p.read_text(encoding="utf-8")) if p.exists() else {}


def scenario_sell_dup(out: Path) -> dict:
    res: dict = {}
    w = new_world(out, "p103-sell-dup")
    res["world_id"] = w
    state = out / "sell_dup_state.json"
    s = Server(w, out / "server_1.log")
    try:
        res["run"] = bot(out, "sell-dup-restart.run", "sell-dup-restart", "inner-belt-1", "bot-150",
                         "--phase", "run", "--state", str(state), "--world", w)
        pc_before = db.scalar_int_strict(f"select count(*) from processed_commands where world_id = '{w}';")
    finally:
        res["server_1_exit"] = s.stop()
    s2 = Server(w, out / "server_2.log")
    try:
        res["resume"] = bot(out, "sell-dup-restart.resume", "sell-dup-restart", "inner-belt-1", "bot-150",
                            "--phase", "resume", "--state", str(state), "--world", w)
        time.sleep(2)
        pc_after = db.scalar_int_strict(f"select count(*) from processed_commands where world_id = '{w}';")
    finally:
        res["server_2_exit"] = s2.stop()
    res["judgement"] = {**P103_LABELS["sell-dup"], **judge_sell_dup_restart(
        res["run"][1], res["resume"][1], pc_before, pc_after, res["server_1_exit"])}
    return res


def _sell_cas_run(out: Path, tag: str, tamper: str | None) -> dict:
    """tamper: None(대조) | "wallets" | "market_states"."""
    res: dict = {}
    w = new_world(out, tag)
    res["world_id"] = w
    ready, tampered = out / f"{tag}.ready", out / f"{tag}.tampered"
    for f in (ready, tampered):
        f.unlink(missing_ok=True)
    s = Server(w, out / f"{tag}_server.log")
    p = subprocess.Popen([str(BOTS), "probe", "--case", "sell-cas-halt", "--deposit", "inner-belt-1", "--label",
                          "bot-160", "--world", w, "--ready-file", str(ready), "--tampered-file", str(tampered),
                          "--out", str(out / f"{tag}_bot.json")],
                         cwd=str(REPO), env=sb.load_dotenv(), stdout=subprocess.DEVNULL, stderr=subprocess.PIPE,
                         text=True, encoding="utf-8", errors="replace")
    deadline = time.monotonic() + 600
    while not ready.exists() and time.monotonic() < deadline and p.poll() is None:
        time.sleep(0.3)
    if not ready.exists():
        res["error"] = "봇이 ready 를 만들지 않았다"
    else:
        deadline2 = time.monotonic() + 10
        committed = 0
        while time.monotonic() < deadline2 and committed == 0:
            committed = _sold_rows(w)
            time.sleep(0.3)
        res["pre_committed_sales"] = committed
        if tamper == "wallets":
            rows = db.psql_rows_strict(f"select actor_id, balance_cr from wallets where world_id = '{w}';")
            if len(rows) == 1:
                a, b = rows[0][0], int(rows[0][1])
                res["original_value"], res["tampered_value"] = b, b + 777
                db.psql_strict(f"update wallets set balance_cr = {b + 777} where world_id = '{w}' and actor_id = '{a}';")
        elif tamper == "market_states":
            rows = db.psql_rows_strict(f"select market_id, mineral_id, saturation_kg from market_states where world_id = '{w}';")
            if len(rows) == 1:
                mk, mn, sat = rows[0][0], rows[0][1], int(rows[0][2])
                res["original_value"], res["tampered_value"] = sat, sat + 77
                db.psql_strict(f"update market_states set saturation_kg = {sat + 77} where world_id = '{w}' "
                               f"and market_id = '{mk}' and mineral_id = '{mn}';")
        tampered.write_text("go\n", encoding="utf-8")
    try:
        p.wait(timeout=900)
    except subprocess.TimeoutExpired:
        res["bot_timeout"] = True
    res["bot_run_exit"] = p.returncode
    if tamper:
        rc = s.wait_exit(60)
        res["server_halted_itself"] = rc is not None
        res["server_exit_code"] = rc if rc is not None else s.stop()
    else:
        res["server_exit_code"] = s.stop()
    try:
        res["sold_rows_after_tamper"] = _sold_rows(w) - res.get("pre_committed_sales", 0)
    except db.QueryError as e:
        res["sold_rows_after_tamper"] = None
        res["sql_error"] = str(e)
    log = (out / f"{tag}_server.log").read_text(encoding="utf-8", errors="replace").splitlines()
    res["halt_log_lines"] = [l for l in log if any(m in l for m in HALT_LOG_MARKERS)][-5:]
    res["bot_facts"] = _bot_json(out, f"{tag}_bot")
    return res


def scenario_sell_cas(out: Path, market: bool) -> dict:
    res: dict = {"control": _sell_cas_run(out, "p103-cas-control", None)}
    table = "market_states" if market else "wallets"
    res["tamper"] = _sell_cas_run(out, "p103-cas-market" if market else "p103-cas-wallet", table)
    key = "sell-cas-market" if market else "sell-cas"
    res["judgement"] = {**P103_LABELS[key], **judge_sell_cas(res["control"], res["tamper"], table)}
    if not market:
        t = res["tamper"]
        s = Server(t["world_id"], out / "p103-cas-reload_server.log")
        before = _sold_rows(t["world_id"])
        try:
            res["reload"] = bot(out, "sell-cas-reload", "sell-cas-reload", "inner-belt-1", "bot-160", "--world", t["world_id"])
            time.sleep(2)
        finally:
            rc = s.wait_exit(1)            # 스스로 멈췄는가(커밋 실패 정지)
            res["reload_server_exit"] = rc if rc is not None else s.stop()
        new_rows = _sold_rows(t["world_id"]) - before
        wb = db.psql_rows_strict(
            f"select (payload->>'wallet_before_cr')::bigint from domain_events where world_id = '{t['world_id']}' "
            "and event_type = 'MINERAL_SOLD' order by tick desc, sequence desc limit 1;") if new_rows > 0 else []
        res["judgement_reload"] = {**P103_LABELS["sell-cas-reload"], **judge_sell_cas_reload(
            t, res["reload"][1], int(wb[0][0]) if wb else None, new_rows, res["reload_server_exit"])}
    return res


def scenario_sell_recover(out: Path) -> dict:
    res: dict = {}
    w = new_world(out, "p103-sell-recover")
    res["world_id"] = w
    s = Server(w, out / "server_1.log")
    try:
        res["run"] = bot(out, "sell-recover.run", "sell-recover-restart", "inner-belt-1", "bot-170",
                         "--phase", "run", "--world", w)
    finally:
        res["server_1_exit"] = s.stop()
    s2 = Server(w, out / "server_2.log")
    try:
        res["resume"] = bot(out, "sell-recover.resume", "sell-recover-restart", "inner-belt-1", "bot-171",
                            "--phase", "resume", "--world", w)
    finally:
        res["server_2_exit"] = s2.stop()
    res["judgement"] = {**P103_LABELS["sell-recover"], **judge_sell_recover(res["run"][1], res["resume"][1])}
    return res


def selftest_p103() -> int:
    run = {"sells": [{"results": [{"status": "ACCEPTED"}]}], "wallets": [{"balance_cr": 0}, {"balance_cr": 2000}]}
    resume = {"sells": [{"results": [{"status": "REJECTED", "reason": "DUPLICATE_COMMAND_ID"}]}],
              "wallets": [{"balance_cr": 2000}], "receipts": []}
    pre = {"tick": 5000, "saturation_kg": 13000, "last_sale": {"actor_id": "a", "quantity_kg": 14000, "tick": 3000}}
    cases = [
        ("dup: 정상 → PASS", judge_sell_dup_restart(run, resume, 5, 5, 0)["verdict"] == "PASS"),
        ("dup 음성: 재기동 뒤 영수증 1(두 번 반영) → FAIL",
         judge_sell_dup_restart(run, {**resume, "receipts": [{}]}, 5, 5, 0)["verdict"] == "FAIL"),
        ("dup 음성: 첫 판매 거절 → 미검증",
         judge_sell_dup_restart({**run, "sells": [{"results": [{"status": "REJECTED"}]}]}, resume, 5, 5, 0)["verdict"].startswith("미검증")),
        ("dup 음성: 장부 행 증가 → FAIL", judge_sell_dup_restart(run, resume, 5, 6, 0)["verdict"] == "FAIL"),
        ("cas: 정지·미커밋·대조 생존 → PASS", judge_sell_cas(
            {"server_exit_code": 0, "sold_rows_after_tamper": 1},
            {"pre_committed_sales": 1, "original_value": 2000, "tampered_value": 2777, "server_halted_itself": True,
             "server_exit_code": 3, "sold_rows_after_tamper": 0, "halt_log_lines": ["CAS 실패 table=wallets"]},
            "wallets")["verdict"] == "PASS"),
        ("cas 음성: 다른 이유로 죽음(로그에 표 이름 없음) → FAIL", judge_sell_cas(
            {"server_exit_code": 0, "sold_rows_after_tamper": 1},
            {"pre_committed_sales": 1, "original_value": 2000, "tampered_value": 2777, "server_halted_itself": True,
             "server_exit_code": 3, "sold_rows_after_tamper": 0, "halt_log_lines": ["panic"]},
            "wallets")["verdict"] == "FAIL"),
        ("cas 음성: 변조 전 판매 미커밋 → 미검증", judge_sell_cas(
            {"server_exit_code": 0}, {"pre_committed_sales": 0, "original_value": 1, "tampered_value": 2}, "wallets")["verdict"].startswith("미검증")),
        ("cas-reload: 새 판매 1 행·wallet_before = 변조 값·정상 종료 → PASS", judge_sell_cas_reload(
            {"tampered_value": 2777, "server_halted_itself": True}, {"sells": [{"results": [{"status": "ACCEPTED"}]}]},
            2777, 1, 0)["verdict"] == "PASS"),
        ("cas-reload 음성: 수락은 됐는데 커밋 안 됨(새 행 0, 서버 정지) → FAIL", judge_sell_cas_reload(
            {"tampered_value": 2777, "server_halted_itself": True}, {"sells": [{"results": [{"status": "ACCEPTED"}]}]},
            None, 0, 1)["verdict"] == "FAIL"),
        ("recover: 보존 + 식대로 → PASS", judge_sell_recover(
            {"ferrosite_market": [pre]},
            {"ferrosite_market": [{**pre, "tick": 5010}, {**pre, "tick": 6000, "saturation_kg": 12000}]})["verdict"] == "PASS"),
        ("recover 음성: 재기동 뒤 포화가 0 으로(보존 실패) → FAIL", judge_sell_recover(
            {"ferrosite_market": [pre]},
            {"ferrosite_market": [{**pre, "tick": 5010, "saturation_kg": 0}, {**pre, "tick": 6000, "saturation_kg": 0}]})["verdict"] == "FAIL"),
        ("recover 음성: 배수 경계에서 두 번 깎임(식과 다름) → FAIL", judge_sell_recover(
            {"ferrosite_market": [pre]},
            {"ferrosite_market": [{**pre, "tick": 5010}, {**pre, "tick": 6000, "saturation_kg": 11000}]})["verdict"] == "FAIL"),
    ]
    bad = 0
    for n, ok in cases:
        print(("OK   " if ok else "FAIL ") + n)
        bad += 0 if ok else 1
    print(f"selftest p1-03: {len(cases) - bad}/{len(cases)}")
    return 0 if bad == 0 else 1


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--out", default="")
    ap.add_argument("scenario", choices=["dup", "cas", "sell-dup", "sell-cas", "sell-cas-market", "sell-recover",
                                         "selftest"])
    a = ap.parse_args()
    if a.scenario == "selftest":
        return selftest_p103()
    out = Path(a.out)
    out.mkdir(parents=True, exist_ok=True)
    if not BOTS.is_file():
        print(f"미검증(환경): {BOTS} 가 없다 — tools/bots 에서 cargo build", file=sys.stderr)
        return 2
    res = {"dup": scenario_dup, "cas": scenario_cas, "sell-dup": scenario_sell_dup,
           "sell-cas": lambda o: scenario_sell_cas(o, False), "sell-cas-market": lambda o: scenario_sell_cas(o, True),
           "sell-recover": scenario_sell_recover}[a.scenario](out)
    res["provenance"] = sb.source_provenance()
    (out / "summary.json").write_text(json.dumps(res, ensure_ascii=False, indent=2, default=str), encoding="utf-8")
    print(json.dumps({k: (v[1].get("verdict") if isinstance(v, tuple) else None) for k, v in res.items()
                      if isinstance(v, tuple)}, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    sys.exit(main())
