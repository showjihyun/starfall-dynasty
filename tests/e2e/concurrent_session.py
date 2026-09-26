#!/usr/bin/env python
"""SC-88 — 같은 actor 의 동시 세션 (스펙 AC-3(h), I-29, 사용자 결정 5 = (B) 나중 접속이 이어받는다).

두 하위 명령:

* `run`  : **떠 있는 서버**에 대해 같은 라벨의 봇 둘을 겹쳐 붙인다(S1 → 2 s 뒤 S2).
  - 라운드 A: 둘 다 닫고 잔류 창(+여유)이 지날 때까지 기다린 뒤 `ships_active` 를 본다(유령 `ACTIVE` 0).
  - 라운드 B: 다른 라벨로 한 번 더 겹쳐 붙이고 **S2 를 살려 둔 채** 끝낸다 — 그 뒤 qa 가 서버를 내리면
    넘겨받은 함선이 `SERVER_SHUTDOWN` 으로 디스폰되어 (e) 의 "종료 디스폰 원인이 실제 SESSION_CLOSED" 가
    **실제로 발생한 조건에서** 판정된다(계약 §7a — 디스폰 0 건이면 "전부" 는 아무것도 뜻하지 않는다).
* `check`: 서버를 내린 뒤 DB 로 (a)(b)(e) 를 판정하고, `run` 의 봇 출력으로 (c) 를 판정한다.

**(d) "밀려난 클라이언트가 재접속하지 않는다" 는 여기서 판정하지 않는다** — 봇은 원래 재접속하지 않으므로
봇을 S1 로 두면 항진명제다. 블록 6 에서 Unity S1 + 봇 S2 로 잰다(계약 SC-88).

종료 코드: 0 = 성립, 1 = 불성립, 2 = 서버·DB 에 닿지 못함.
"""
from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
import time
import urllib.request
from pathlib import Path

for _stream in (sys.stdout, sys.stderr):
    try:
        _stream.reconfigure(encoding="utf-8")  # type: ignore[union-attr]
    except (AttributeError, ValueError):
        pass

sys.path.insert(0, str(Path(__file__).resolve().parent))
import db  # noqa: E402
from server_boot import load_dotenv  # noqa: E402

REPO = Path(__file__).resolve().parents[2]
BOTS = REPO / "tools" / "bots" / "target" / "debug" / "bots.exe"
CLOSE = re.compile(r"close: code=Some\((\d+)\).*initiator=(\w+)")
ACTOR = re.compile(r"actor_id=([0-9a-f-]{36})")
CORR = re.compile(r"correlation_id=Some\(([0-9a-f-]{36})\)")
SHIP = re.compile(r"controlled_ship_id=Some\(\"?([0-9a-f-]{36})")


def stats(addr: str) -> dict:
    with urllib.request.urlopen(f"http://{addr}/debug/stats", timeout=5) as r:
        return json.loads(r.read().decode("utf-8"))


def gauges(addr: str) -> dict:
    d = stats(addr)
    return {k: d[k] for k in ("tick", "ws_connections", "ships_active", "ships_lingering")}


def pair(addr: str, label: str, s1_secs: int, s2_secs: int, env: dict, ev: Path, tag: str) -> dict:
    """S1 을 띄우고 2 s 뒤 같은 라벨로 S2. 각 시점 게이지를 남긴다."""
    snaps = {"before": gauges(addr)}
    f1 = open(ev / f"{tag}-s1.log", "wb")
    p1 = subprocess.Popen([str(BOTS), "probe", "--case", "fly", "--count", str(s1_secs), "--label", label],
                          cwd=str(REPO), env=env, stdout=f1, stderr=subprocess.STDOUT)
    time.sleep(2.0)
    snaps["s1_only"] = gauges(addr)
    f2 = open(ev / f"{tag}-s2.log", "wb")
    p2 = subprocess.Popen([str(BOTS), "probe", "--case", "fly", "--count", str(s2_secs), "--label", label],
                          cwd=str(REPO), env=env, stdout=f2, stderr=subprocess.STDOUT)
    time.sleep(2.0)
    snaps["after_s2"] = gauges(addr)
    return {"label": label, "p1": p1, "p2": p2, "f1": f1, "f2": f2, "snaps": snaps}


def cmd_run(args) -> int:
    env = load_dotenv()
    addr = env.get("STARFALL_HTTP_ADDR", "127.0.0.1:8080")
    try:
        start = stats(addr)
    except OSError as e:
        print(f"미검증(환경): 서버에 닿지 못했다: {e}", file=sys.stderr)
        return 2
    ev = Path(args.evidence)
    ev.mkdir(parents=True, exist_ok=True)
    out = {"start_tick": start["start_tick"], "run_from_tick": start["tick"]}

    # 라운드 A — 둘 다 닫히고 잔류 창이 지나도록.
    a = pair(addr, args.label_a, 12, 6, env, ev, "A")
    a["p1"].wait(timeout=120)
    a["p2"].wait(timeout=120)
    a["snaps"]["both_closed"] = gauges(addr)
    time.sleep(args.linger_wait)
    a["snaps"]["after_linger"] = gauges(addr)
    out["A"] = {"label": a["label"], "snaps": a["snaps"]}

    # 라운드 B — S2 를 살려 둔 채 끝낸다(종료 스윕에서 넘겨받은 함선이 디스폰되도록).
    b = pair(addr, args.label_b, 8, args.b_s2_secs, env, ev, "B")
    b["p1"].wait(timeout=120)
    b["snaps"]["s1_closed"] = gauges(addr)
    out["B"] = {"label": b["label"], "snaps": b["snaps"], "s2_pid": b["p2"].pid,
                "note": "S2 가 살아 있는 동안 qa 가 stdin shutdown 을 보낸다"}
    (ev / "run.json").write_text(json.dumps(out, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps(out, ensure_ascii=False, indent=2))
    print(f"\n>>> 이제 서버를 정상 종료하라(라운드 B 의 S2 가 {args.b_s2_secs} s 동안 살아 있다). "
          f"그 뒤: concurrent_session.py check --evidence {ev}")
    return 0


def bot_facts(log: Path) -> dict:
    t = log.read_bytes().decode("utf-8", errors="replace")
    c = CLOSE.search(t)
    a = ACTOR.search(t)
    k = CORR.search(t)
    sh = SHIP.search(t)
    return {"close_code": int(c.group(1)) if c else None, "initiator": c.group(2) if c else None,
            "actor_id": a.group(1) if a else None, "correlation_id": k.group(1) if k else None,
            "controlled_ship_id": sh.group(1) if sh else None}


def cmd_check(args) -> int:
    ev = Path(args.evidence)
    run = json.loads((ev / "run.json").read_text(encoding="utf-8"))
    frm = run["run_from_tick"]
    result: dict = {"from_tick": frm, "rounds": {}}
    ok_all = True
    for tag in ("A", "B"):
        s1, s2 = bot_facts(ev / f"{tag}-s1.log"), bot_facts(ev / f"{tag}-s2.log")
        actor = s1["actor_id"]
        rows = db.psql_rows(
            "select tick::text, sequence::text, event_type, event_id::text, coalesce(causation_id::text,''), "
            "  correlation_id::text, coalesce(payload->>'close_reason',''), coalesce(payload->>'ship_id',''), "
            "  coalesce(payload->>'despawn_reason','') "
            f"from domain_events where actor_id = '{actor}' and tick >= {frm} order by tick, sequence;"
        )
        ev_rows = [dict(zip(("tick", "seq", "type", "id", "cause", "corr", "close_reason", "ship", "despawn"), r))
                   for r in rows]
        spawns = [r for r in ev_rows if r["type"] == "SHIP_SPAWNED"]
        opened = [r for r in ev_rows if r["type"] == "SESSION_OPENED"]
        sup = [r for r in ev_rows if r["type"] == "SESSION_CLOSED" and r["close_reason"] == "SUPERSEDED"]
        by_id = {r["id"]: r for r in ev_rows}
        # (b) SUPERSEDED 닫힘의 원인 = 같은 tick 의 새 SESSION_OPENED, sequence 가 더 작다
        b_ok = len(sup) == 1 and sup[0]["corr"] == s1["correlation_id"] and sup[0]["cause"] in by_id \
            and by_id[sup[0]["cause"]]["type"] == "SESSION_OPENED" \
            and by_id[sup[0]["cause"]]["corr"] == s2["correlation_id"] \
            and by_id[sup[0]["cause"]]["tick"] == sup[0]["tick"] \
            and int(by_id[sup[0]["cause"]]["seq"]) < int(sup[0]["seq"])
        # (a)·③ 함선 1척: 이 라운드에 스폰은 S1 의 1건뿐이고 S2 는 **같은 함선**을 조종한다(이어받음)
        same_ship = (s1["controlled_ship_id"] is not None
                     and s1["controlled_ship_id"] == s2["controlled_ship_id"]
                     and len(spawns) == 1 and spawns[0]["ship"] == s1["controlled_ship_id"])
        a_ok = len(spawns) == 1 and len(opened) == 2 and same_ship
        # (c) S1 연결은 서버가 4001 로 닫았다
        c_ok = s1["close_code"] == 4001 and s1["initiator"] == "server"
        rnd = {"actor_id": actor, "s1": s1, "s2": s2, "spawns": len(spawns), "session_opened": len(opened),
               "superseded_rows": sup, "a_one_ship": a_ok, "b_superseded_caused_by_new_open": b_ok,
               "c_close_4001": c_ok, "events": ev_rows}
        if tag == "A":
            g = run["A"]["snaps"]
            # 격리 전제: 라운드 A 시작 전에 활성 함선이 없어야 (e) 의 게이지 판정이 이 actor 를 가리킨다.
            rnd["e_no_ghost_after_linger"] = g["before"]["ships_active"] == 0 and g["after_linger"]["ships_active"] == 0
            rnd["gauges"] = g
            ok_all &= rnd["e_no_ghost_after_linger"]
        else:
            desp = [r for r in ev_rows if r["type"] == "SHIP_DESPAWNED" and r["despawn"] == "SERVER_SHUTDOWN"]
            cause_ok = bool(desp) and all(r["cause"] in by_id and by_id[r["cause"]]["type"] == "SESSION_CLOSED"
                                          and r["cause"] != r["id"] for r in desp)
            rnd["shutdown_despawns"] = len(desp)
            rnd["e_shutdown_cause_real_session_closed"] = cause_ok
            ok_all &= cause_ok
        ok_all &= a_ok and b_ok and c_ok
        result["rounds"][tag] = rnd
    result["d_not_judged_here"] = "봇은 재접속하지 않는다 — (d) 는 Unity S1 로(블록 6)"
    result["verdict"] = "PASS" if ok_all else "FAIL"
    (ev / "check.json").write_text(json.dumps(result, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps({k: v for k, v in result.items() if k != "rounds"}, ensure_ascii=False))
    for tag, r in result["rounds"].items():
        print(tag, {k: v for k, v in r.items() if k not in ("events", "superseded_rows", "gauges")})
    print("다음도 함께 돌린다: ship_events.py overlap / ledger --from-tick", frm)
    return 0 if ok_all else 1


def main() -> int:
    ap = argparse.ArgumentParser(description="SC-88: 같은 actor 동시 세션 (B) 넘겨받기")
    sub = ap.add_subparsers(dest="cmd", required=True)
    r = sub.add_parser("run")
    r.add_argument("--label-a", default="bot-050")
    r.add_argument("--label-b", default="bot-051")
    r.add_argument("--linger-wait", type=float, default=35.0, help="잔류 창 30 s + 여유 (G-j)")
    r.add_argument("--b-s2-secs", type=int, default=60)
    r.add_argument("--evidence", required=True)
    c = sub.add_parser("check")
    c.add_argument("--evidence", required=True)
    args = ap.parse_args()
    return db.main_guard(lambda: cmd_run(args) if args.cmd == "run" else cmd_check(args))


if __name__ == "__main__":
    sys.exit(main())
