"""p1-02 원장·보존 항등식 — 계약 SC-23 · SC-24 · SC-32 · SC-86 · SC-88 · SC-92.

    python tests/e2e/mining_ledger.py --world <id> ledger          # SC-23  I-54
    python tests/e2e/mining_ledger.py --world <id> conservation    # SC-24  I-55
    python tests/e2e/mining_ledger.py --world <id> conservation --require-regen   # SC-86
    python tests/e2e/mining_ledger.py --world <id> load            # SC-88  (부하 뒤, 키 ≥ 30)
    python tests/e2e/mining_ledger.py --world <id> accepted --from <봇 수락 command_id 파일>  # SC-32
    python tests/e2e/mining_ledger.py --world <id> causation       # SC-92
    python tests/e2e/mining_ledger.py selftest

**채굴이 0 건이면 FAIL 로 인쇄한다**(CLAUDE.md 검증의 규율 — 빈 집합에 대한 전칭명제는 참이다).
결과의 분모(검사한 키·광물·광맥 수)와 판정 근거의 분모(그 키들을 만든 `MINERAL_MINED` 수)를
둘 다 찍는다.

회복량은 이벤트를 `(tick, sequence)` 순으로 되감으며 **스펙 I-69 의 닫힌 식으로 재계산**한다 —
서버의 회복 코드를 부르지 않는다(같은 코드로 재면 자기 일치다). 회복 매개변수(초기 매장량·
`regen_kg`·`regen_interval_s`)는 **그 시점의 `data/`** 에서 읽고 증거에 함께 찍는다(계약 §0.6).

테이블 모양은 ADR-0013 §5a-1 의 요구 집합이다: `inventory_items(world_id, actor_id, mineral_id,
quantity_kg)`, `deposit_states(world_id, deposit_id, remaining_kg, as_of_tick, first_extracted_tick)`,
`processed_commands(world_id, command_id, actor_id, command_type, tick)`. 0002 가 다르게 오면
이 파일이 틀린 것이다 — 경계면 검증에서 맞춘다.

종료 코드: db.py 규약 (0 PASS / 1 FAIL / 2 미검증(환경) / 3 구현 없음).
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

import db

# verdict 라벨 — 출처 게이트가 이 리터럴을 읽는다(계약 §3.2, 슬라이스 표지 §3.3).
LABELS = {
    "ledger": {"item": "p1-02 SC-23 (AC-5a) 원장 항등식 I-54"},
    "conservation": {"item": "p1-02 SC-24 (AC-5a) 보존 법칙 I-55"},
    "regen": {"item": "p1-02 SC-86 (AC-17c) 보존 법칙 + 회복 실제 발생"},
    "load": {"item": "p1-02 SC-88 (AC-17d) 부하 뒤 원장·보존 항등식"},
    "accepted": {"item": "p1-02 SC-32 (AC-6d) 수락된 채굴이 전부 DB 에"},
    "causation": {"item": "p1-02 SC-92 (AC-18b) causation_id ↔ processed_commands 양방향"},
    "discoveries": {"item": "p1-02 SC-90 (AC-17d) MINERAL_DISCOVERED 수 = 채굴된 광물 종류 수 ≤ 4, 고갈 ≥ 1"},
}

# 분모 하한 — 계약 행이 정한 값. 바꾸려면 계약을 먼저 바꾼다(규칙 2).
LEDGER_MIN_KEYS, LEDGER_MIN_EVENTS = 2, 10       # SC-23
LOAD_MIN_KEYS = 30                               # SC-88
CONSERVATION_MIN_MINED_MINERALS = 2              # SC-24


# ── 순수 계산 (selftest 가 DB 없이 부른다) ─────────────────────────────────────────

def regen_effective(remaining: int, as_of: int, t: int, regen_kg: int, interval: int,
                    initial: int) -> int:
    """스펙 I-69: `min(initial, remaining + regen_kg × (⌊t/I⌋ − ⌊as_of/I⌋))`."""
    return min(initial, remaining + regen_kg * (t // interval - as_of // interval))


def check_ledger(events: list[dict], inventory: list[dict], min_keys: int,
                 min_events: int) -> dict:
    """I-54. 키 = (actor_id, mineral_id). events 는 (tick, sequence) 오름차순이어야 한다."""
    by_key: dict[tuple, list[dict]] = {}
    for e in events:
        by_key.setdefault((e["actor_id"], e["mineral_id"]), []).append(e)
    inv = {(r["actor_id"], r["mineral_id"]): r["quantity_kg"] for r in inventory}
    problems, links = [], 0
    for key, evs in sorted(by_key.items()):
        total = sum(e["quantity_kg"] for e in evs)
        if evs[0]["quantity_before_kg"] != 0:
            problems.append({"key": key, "why": "첫 이벤트 quantity_before_kg ≠ 0",
                             "value": evs[0]["quantity_before_kg"]})
        for a, b in zip(evs, evs[1:]):
            links += 1
            if a["quantity_after_kg"] != b["quantity_before_kg"]:
                problems.append({"key": key, "why": "연쇄 끊김 앞.after ≠ 뒤.before",
                                 "at": [b["tick"], b["sequence"]],
                                 "prev_after": a["quantity_after_kg"],
                                 "before": b["quantity_before_kg"]})
        for e in evs:
            if e["quantity_before_kg"] + e["quantity_kg"] != e["quantity_after_kg"]:
                problems.append({"key": key, "why": "before + quantity ≠ after",
                                 "at": [e["tick"], e["sequence"]]})
        last_after = evs[-1]["quantity_after_kg"]
        row = inv.get(key)
        if not (row == total == last_after):
            problems.append({"key": key, "why": "인벤토리 행 = Σ = 최신 after 불성립",
                             "row": row, "sum": total, "last_after": last_after})
    # 이벤트가 없는데 행만 있는 키 — 설명 없는 수량
    for key, q in inv.items():
        if key not in by_key and q != 0:
            problems.append({"key": key, "why": "이벤트 없는 인벤토리 행", "row": q})
    denominators_ok = len(by_key) >= min_keys and len(events) >= min_events
    return {
        "keys_checked": len(by_key),
        "keys_holding": len(by_key) - len({tuple(p["key"]) for p in problems if "key" in p}),
        "events_mineral_mined": len(events),
        "chain_links_checked": links,
        "inventory_rows": len(inv),
        "min_keys": min_keys, "min_events": min_events,
        "denominators_ok": denominators_ok,
        "problems": problems,
        "ok": denominators_ok and not problems,
    }


def check_conservation(events: list[dict], deposit_rows: list[dict], inventory: list[dict],
                       deposits: dict, minerals: dict, tick_hz: int,
                       min_mined_minerals: int, require_regen: bool) -> dict:
    """I-55. deposits: id → {mineral_id, initial_reserve_kg}. minerals: id → {regen_kg, regen_interval_s}."""
    by_dep: dict[str, list[dict]] = {}
    for e in events:
        by_dep.setdefault(e["deposit_id"], []).append(e)
    rows = {r["deposit_id"]: r for r in deposit_rows}
    problems, regen_by_dep, links = [], {}, 0
    for dep_id, evs in sorted(by_dep.items()):
        d = deposits.get(dep_id)
        if d is None:
            problems.append({"deposit": dep_id, "why": "data/ 광맥 표에 없는 deposit_id"})
            continue
        m = minerals[d["mineral_id"]]
        interval = m["regen_interval_s"] * tick_hz
        initial = d["initial_reserve_kg"]
        regen_total = 0
        # 채굴 전 광맥은 초기값이고 상한이 초기값이므로 첫 before 는 초기값이어야 한다.
        if evs[0]["deposit_remaining_before_kg"] != initial:
            problems.append({"deposit": dep_id, "why": "첫 이벤트 before ≠ 초기 매장량",
                             "before": evs[0]["deposit_remaining_before_kg"], "initial": initial})
        for a, b in zip(evs, evs[1:]):
            links += 1
            expect = regen_effective(a["deposit_remaining_after_kg"], a["tick"], b["tick"],
                                     m["regen_kg"], interval, initial)
            if b["deposit_remaining_before_kg"] != expect:
                problems.append({"deposit": dep_id, "why": "회복 재계산 ≠ 뒤.before",
                                 "at": [b["tick"], b["sequence"]], "expected": expect,
                                 "actual": b["deposit_remaining_before_kg"]})
            regen_total += b["deposit_remaining_before_kg"] - a["deposit_remaining_after_kg"]
        for e in evs:
            if e["deposit_remaining_before_kg"] - e["quantity_kg"] != e["deposit_remaining_after_kg"]:
                problems.append({"deposit": dep_id, "why": "광맥 before − quantity ≠ after",
                                 "at": [e["tick"], e["sequence"]]})
        regen_by_dep[dep_id] = regen_total
        row, last = rows.get(dep_id), evs[-1]
        if row is None:
            problems.append({"deposit": dep_id, "why": "채굴됐는데 deposit_states 행이 없다"})
        else:
            if row["remaining_kg"] != last["deposit_remaining_after_kg"] or row["as_of_tick"] != last["tick"]:
                problems.append({"deposit": dep_id, "why": "행 ≠ 최신 이벤트 (remaining, as_of)",
                                 "row": [row["remaining_kg"], row["as_of_tick"]],
                                 "last": [last["deposit_remaining_after_kg"], last["tick"]]})
            if row["first_extracted_tick"] != evs[0]["tick"]:
                problems.append({"deposit": dep_id, "why": "first_extracted_tick ≠ 첫 채굴 tick",
                                 "row": row["first_extracted_tick"], "first": evs[0]["tick"]})
    for dep_id in rows:
        if dep_id not in by_dep:
            problems.append({"deposit": dep_id, "why": "이벤트 없는 deposit_states 행"})

    # 광물별 좌·우변 — 채굴 0 회 광물도 행으로(채굴 수를 옆에)
    per_mineral = []
    for mid in sorted(minerals):
        deps = [k for k, d in deposits.items() if d["mineral_id"] == mid]
        inv_sum = sum(r["quantity_kg"] for r in inventory if r["mineral_id"] == mid)
        remain = sum(rows[k]["remaining_kg"] if k in rows else deposits[k]["initial_reserve_kg"]
                     for k in deps)
        initial = sum(deposits[k]["initial_reserve_kg"] for k in deps)
        regen = sum(regen_by_dep.get(k, 0) for k in deps)
        mined = sum(len(by_dep.get(k, [])) for k in deps)
        lhs, rhs = inv_sum + remain, initial + regen
        per_mineral.append({"mineral_id": mid, "mined_events": mined, "lhs_inventory_plus_remaining": lhs,
                            "rhs_initial_plus_regen": rhs, "regen_kg": regen, "holds": lhs == rhs})
        if lhs != rhs:
            problems.append({"mineral": mid, "why": "Σ인벤토리 + Σ잔량 ≠ Σ초기 + Σ회복", "lhs": lhs, "rhs": rhs})
    mined_minerals = sum(1 for r in per_mineral if r["mined_events"] > 0)
    regen_deposits = sum(1 for v in regen_by_dep.values() if v > 0)
    denominators_ok = mined_minerals >= min_mined_minerals and (regen_deposits >= 1 or not require_regen)
    return {
        "events_mineral_mined": len(events),
        "deposits_mined": len(by_dep),
        "chain_links_checked": links,
        "minerals_with_mining": mined_minerals,
        "min_mined_minerals": min_mined_minerals,
        "deposits_with_actual_regen": regen_deposits,
        "require_regen": require_regen,
        "per_mineral": per_mineral,
        "denominators_ok": denominators_ok,
        "problems": problems,
        "ok": denominators_ok and not problems,
    }


def check_causation(causes: list[str], processed: list[str]) -> dict:
    a, b = set(causes), set(processed)
    ok = bool(a) and bool(b) and a == b and len(causes) == len(a)
    return {"mineral_mined_rows": len(causes), "processed_commands_rows": len(processed),
            "causation_without_command": sorted(a - b)[:20], "command_without_event": sorted(b - a)[:20],
            "missing_counts": [len(a - b), len(b - a)],
            "duplicate_causation": len(causes) - len(a), "ok": ok}


def check_accepted(bot_accepted: list[str], processed: list[str], causes: list[str]) -> dict:
    acc, p, c = set(bot_accepted), set(processed), set(causes)
    return {"bot_accepted": len(acc), "missing_in_processed_commands": sorted(acc - p)[:20],
            "missing_in_mineral_mined": sorted(acc - c)[:20],
            "missing_counts": [len(acc - p), len(acc - c)],
            "ok": bool(acc) and acc <= p and acc <= c}


# ── 데이터·DB 적재 ────────────────────────────────────────────────────────────

def load_data(root: Path) -> tuple[dict, dict, dict]:
    minerals = {}
    for f in sorted((root / "data/minerals").glob("*.json")):
        d = json.loads(f.read_text(encoding="utf-8"))
        minerals[d["id"]] = {"regen_kg": d["regeneration"]["regen_kg"],
                             "regen_interval_s": d["regeneration"]["regen_interval_s"]}
    deposits = {}
    for f in sorted((root / "data/world/deposits").glob("*.json")):
        for d in json.loads(f.read_text(encoding="utf-8"))["deposits"]:
            deposits[d["id"]] = {"mineral_id": d["mineral_id"], "initial_reserve_kg": d["initial_reserve_kg"]}
    return minerals, deposits, {"minerals": minerals, "deposits": deposits}


def _uuid(v: str) -> str:
    db.uuid_array_literal([v])          # 모양 검사 — SQL 에 넣기 전에
    return v


def fetch_events(world: str) -> list[dict]:
    db.require_payload_keys("domain_events", "payload", "MINERAL_MINED",
                            ["mineral_id", "deposit_id", "quantity_kg", "quantity_before_kg",
                             "quantity_after_kg", "deposit_remaining_before_kg", "deposit_remaining_after_kg"],
                            f"world_id = '{_uuid(world)}'", allow_empty=True)
    rows = db.psql_rows_strict(
        "select actor_id, tick, sequence, payload->>'mineral_id', payload->>'deposit_id', "
        "payload->>'quantity_kg', payload->>'quantity_before_kg', payload->>'quantity_after_kg', "
        "payload->>'deposit_remaining_before_kg', payload->>'deposit_remaining_after_kg', causation_id "
        f"from domain_events where world_id = '{_uuid(world)}' and event_type = 'MINERAL_MINED' "
        "order by tick, sequence;")
    out = []
    for r in rows:
        if len(r) != 11 or "" in r[3:10]:
            raise db.NotImplementedYet(f"MINERAL_MINED payload 필드가 비었다(필드 이름 불일치?): {r}")
        out.append({"actor_id": r[0], "tick": int(r[1]), "sequence": int(r[2]), "mineral_id": r[3],
                    "deposit_id": r[4], "quantity_kg": int(r[5]), "quantity_before_kg": int(r[6]),
                    "quantity_after_kg": int(r[7]), "deposit_remaining_before_kg": int(r[8]),
                    "deposit_remaining_after_kg": int(r[9]), "causation_id": r[10]})
    return out


def fetch_inventory(world: str) -> list[dict]:
    db.require_tables("inventory_items")
    return [{"actor_id": a, "mineral_id": m, "quantity_kg": int(q)} for a, m, q in db.psql_rows_strict(
        f"select actor_id, mineral_id, quantity_kg from inventory_items where world_id = '{_uuid(world)}';")]


def fetch_deposit_rows(world: str) -> list[dict]:
    db.require_tables("deposit_states")
    return [{"deposit_id": d, "remaining_kg": int(r), "as_of_tick": int(a), "first_extracted_tick": int(f)}
            for d, r, a, f in db.psql_rows_strict(
                "select deposit_id, remaining_kg, as_of_tick, first_extracted_tick from deposit_states "
                f"where world_id = '{_uuid(world)}';")]


def fetch_processed(world: str) -> list[str]:
    db.require_tables("processed_commands")
    return [r[0] for r in db.psql_rows_strict(
        f"select command_id from processed_commands where world_id = '{_uuid(world)}' "
        "and command_type = 'MINE_RESOURCE';")]


def tick_hz_of(world: str) -> int:
    return db.scalar_int_strict(f"select tick_hz from worlds where world_id = '{_uuid(world)}';")


def check_discoveries(mined_minerals: list[str], discovered_minerals: list[str], depleted_rows: int) -> dict:
    """계약 SC-90 — 순수 함수. `mined_minerals` 는 MINERAL_MINED 의 광물 distinct, `discovered_minerals` 는
    MINERAL_DISCOVERED 행마다의 광물(중복 그대로), `depleted_rows` 는 `deposit_remaining_after_kg = 0` 행 수.
    판정 = 발견 행 수 == 채굴 광물 종류 수 ≤ 4 · 고갈 ≥ 1. ⊘(분모) = 채굴 광물 종류 ≥ 2 · 고갈 ≥ 1."""
    kinds = len(set(mined_minerals))
    ok_counts = len(discovered_minerals) == kinds and kinds <= 4
    denominators_ok = kinds >= 2 and depleted_rows >= 1
    return {"mined_mineral_kinds": kinds, "mined_minerals": sorted(set(mined_minerals)),
            "discovered_rows": len(discovered_minerals), "discovered_minerals": sorted(discovered_minerals),
            "same_set": sorted(set(mined_minerals)) == sorted(discovered_minerals),
            "depleted_rows": depleted_rows, "denominators_ok": denominators_ok,
            "ok": denominators_ok and ok_counts}


def fetch_discovery_facts(world: str) -> tuple[list[str], list[str], int]:
    w = _uuid(world)
    db.require_payload_keys("domain_events", "payload", "MINERAL_MINED",
                            ["mineral_id", "deposit_remaining_after_kg"], f"world_id = '{w}'", allow_empty=True)
    db.require_payload_keys("historical_events", "payload", "MINERAL_DISCOVERED", ["mineral_id"],
                            f"world_id = '{w}'", allow_empty=True)
    mined = [r[0] for r in db.psql_rows_strict(
        f"select distinct payload->>'mineral_id' from domain_events where world_id = '{w}' "
        "and event_type = 'MINERAL_MINED' order by 1;")]
    disc = [r[0] for r in db.psql_rows_strict(
        f"select payload->>'mineral_id' from historical_events where world_id = '{w}' "
        "and event_type = 'MINERAL_DISCOVERED' order by 1;")]
    depleted = db.scalar_int_strict(
        f"select count(*) from domain_events where world_id = '{w}' and event_type = 'MINERAL_MINED' "
        "and (payload->>'deposit_remaining_after_kg')::bigint = 0;")
    return mined, disc, depleted


def verdict(result: dict, label: dict) -> dict:
    out = dict(label)
    if result["ok"]:
        out["verdict"] = "PASS"
    elif not result.get("denominators_ok", True):
        out["verdict"] = "FAIL(분모 미달 — 겨냥한 조건이 이번 실행에서 충분히 일어나지 않았다)"
    else:
        out["verdict"] = "FAIL"
    out.update(result)
    return out


def run(args: argparse.Namespace) -> int:
    root = db.REPO_ROOT
    world = args.world
    if args.mode in ("ledger", "load", "conservation"):
        events = fetch_events(world)
        inv = fetch_inventory(world)
    if args.mode == "ledger":
        res = verdict(check_ledger(events, inv, LEDGER_MIN_KEYS, LEDGER_MIN_EVENTS), LABELS["ledger"])
    elif args.mode == "conservation":
        minerals, deposits, data_echo = load_data(root)
        res = verdict(check_conservation(events, fetch_deposit_rows(world), inv, deposits, minerals,
                                         tick_hz_of(world), CONSERVATION_MIN_MINED_MINERALS,
                                         args.require_regen),
                      LABELS["regen" if args.require_regen else "conservation"])
        res["data_values_at_run"] = data_echo
    elif args.mode == "load":
        minerals, deposits, data_echo = load_data(root)
        led = check_ledger(events, inv, LOAD_MIN_KEYS, LEDGER_MIN_EVENTS)
        con = check_conservation(events, fetch_deposit_rows(world), inv, deposits, minerals,
                                 tick_hz_of(world), CONSERVATION_MIN_MINED_MINERALS, False)
        res = verdict({"ledger": led, "conservation": con, "ok": led["ok"] and con["ok"],
                       "denominators_ok": led["denominators_ok"] and con["denominators_ok"]},
                      LABELS["load"])
        res["data_values_at_run"] = data_echo
    elif args.mode == "discoveries":
        res = verdict(check_discoveries(*fetch_discovery_facts(world)), LABELS["discoveries"])
    elif args.mode == "causation":
        events = fetch_events(world)
        res = verdict(check_causation([e["causation_id"] for e in events], fetch_processed(world)),
                      LABELS["causation"])
    elif args.mode == "accepted":
        if not args.from_file:
            raise db.EnvironmentProblem("accepted 는 --from <봇 수락 command_id 파일> 이 필요하다")
        acc = db.read_correlations(args.from_file)      # 한 줄 하나 UUID, 빈 파일은 환경 문제
        events = fetch_events(world)
        res = verdict(check_accepted(acc, fetch_processed(world), [e["causation_id"] for e in events]),
                      LABELS["accepted"])
    res["world_id"] = world
    db.emit(res, args.evidence)
    return db.EXIT_OK if res["verdict"] == "PASS" else db.EXIT_FAIL


# ── selftest (규칙 6: 걸려야 할 입력에서 걸리는가, 걸리지 말아야 할 입력에서 통과하는가) ──

def _ev(actor, mineral, dep, tick, seq, q, qb, db_, cause=None):
    return {"actor_id": actor, "mineral_id": mineral, "deposit_id": dep, "tick": tick, "sequence": seq,
            "quantity_kg": q, "quantity_before_kg": qb, "quantity_after_kg": qb + q,
            "deposit_remaining_before_kg": db_, "deposit_remaining_after_kg": db_ - q,
            "causation_id": cause or f"c-{actor}-{tick}-{seq}"}


def _world():
    """합성 월드: 광물 2(x: I=10 tick, regen 5 / y: 회복 거의 없음), 광맥 2. tick_hz = 1."""
    minerals = {"x": {"regen_kg": 5, "regen_interval_s": 10}, "y": {"regen_kg": 1, "regen_interval_s": 1000}}
    deposits = {"dx": {"mineral_id": "x", "initial_reserve_kg": 100},
                "dy": {"mineral_id": "y", "initial_reserve_kg": 100}}
    ev = []
    # A 가 dx 를 20 씩 3 번(tick 1, 5, 25) — tick 5→25 에 경계 10·20 두 번 → +10 회복
    ev.append(_ev("A", "x", "dx", 1, 0, 20, 0, 100))
    ev.append(_ev("A", "x", "dx", 5, 0, 20, 20, 80))
    ev.append(_ev("A", "x", "dx", 25, 0, 20, 40, 70))       # before = min(100, 60 + 5×(2−0)) = 70
    # B 가 dy 를 10 씩 7 번
    for i in range(7):
        ev.append(_ev("B", "y", "dy", 30 + i, 0, 10, 10 * i, 100 - 10 * i))
    ev.sort(key=lambda e: (e["tick"], e["sequence"]))
    inv = [{"actor_id": "A", "mineral_id": "x", "quantity_kg": 60},
           {"actor_id": "B", "mineral_id": "y", "quantity_kg": 70}]
    rows = [{"deposit_id": "dx", "remaining_kg": 50, "as_of_tick": 25, "first_extracted_tick": 1},
            {"deposit_id": "dy", "remaining_kg": 30, "as_of_tick": 36, "first_extracted_tick": 30}]
    return minerals, deposits, ev, inv, rows


def selftest() -> int:
    import copy
    minerals, deposits, ev, inv, rows = _world()
    cases = []

    r = check_ledger(ev, inv, LEDGER_MIN_KEYS, LEDGER_MIN_EVENTS)
    cases.append(("원장: 정상 합성 월드 → PASS (키 2 · 이벤트 10)", r["ok"] and r["keys_checked"] == 2
                  and r["events_mineral_mined"] == 10, r))
    r = check_ledger([], [], LEDGER_MIN_KEYS, LEDGER_MIN_EVENTS)
    cases.append(("원장: **채굴 0 건 → FAIL**(0 = 0 은 검사가 아니다)", not r["ok"] and not r["problems"], r))
    bad = copy.deepcopy(ev)
    bad[1]["quantity_before_kg"] += 1
    bad[1]["quantity_after_kg"] += 1
    r = check_ledger(bad, inv, LEDGER_MIN_KEYS, LEDGER_MIN_EVENTS)
    cases.append(("원장: 연쇄 한 칸 어긋남 → FAIL", not r["ok"] and any("연쇄" in p["why"] for p in r["problems"]), r))
    inv_bad = copy.deepcopy(inv)
    inv_bad[0]["quantity_kg"] = 80                       # 변조된 인벤토리(SC-25 의 모양)
    r = check_ledger(ev, inv_bad, LEDGER_MIN_KEYS, LEDGER_MIN_EVENTS)
    cases.append(("원장: 인벤토리 행 변조 → FAIL", not r["ok"], r))
    r = check_ledger(ev, inv + [{"actor_id": "C", "mineral_id": "x", "quantity_kg": 5}],
                     LEDGER_MIN_KEYS, LEDGER_MIN_EVENTS)
    cases.append(("원장: 이벤트 없는 인벤토리 행 → FAIL", not r["ok"], r))

    r = check_conservation(ev, rows, inv, deposits, minerals, 1, 2, True)
    cases.append(("보존: 정상 → PASS, 회복 발생 광맥 1 (dx +10)", r["ok"] and r["deposits_with_actual_regen"] == 1, r))
    bad = copy.deepcopy(ev)
    for e in bad:                                         # 회복을 한 경계 늦게 계산한 서버
        if e["deposit_id"] == "dx" and e["tick"] == 25:
            e["deposit_remaining_before_kg"] = 65
            e["deposit_remaining_after_kg"] = 45
    rows_bad = copy.deepcopy(rows)
    rows_bad[0]["remaining_kg"] = 45
    r = check_conservation(bad, rows_bad, inv, deposits, minerals, 1, 2, True)
    cases.append(("보존: 회복 한 경계 누락 → FAIL(재계산 ≠ before)", not r["ok"]
                  and any("회복 재계산" in p["why"] for p in r["problems"]), r))
    r = check_conservation([], [], [], deposits, minerals, 1, 2, False)
    cases.append(("보존: 채굴 0 건 → FAIL, 광물 행은 채굴 수 0 으로 찍힘", not r["ok"]
                  and all(m["mined_events"] == 0 for m in r["per_mineral"]) and len(r["per_mineral"]) == 2, r))
    no_regen = [e for e in ev if not (e["deposit_id"] == "dx" and e["tick"] == 25)]
    inv_nr = [{"actor_id": "A", "mineral_id": "x", "quantity_kg": 40}, inv[1]]
    rows_nr = [{"deposit_id": "dx", "remaining_kg": 60, "as_of_tick": 5, "first_extracted_tick": 1}, rows[1]]
    r = check_conservation(no_regen, rows_nr, inv_nr, deposits, minerals, 1, 2, True)
    cases.append(("보존 --require-regen: 회복 0 인 실행 → FAIL(분모)", not r["ok"] and not r["problems"], r))
    r = check_discoveries(["a", "b", "c"], ["a", "b", "c"], 2)
    cases.append(("SC-90: 채굴 3 종 = 발견 3 행, 고갈 2 → PASS", r["ok"], r))
    r = check_discoveries(["a", "b"], ["a", "a", "b"], 1)
    cases.append(("SC-90: 한 광물 발견 두 번(3 행 ≠ 2 종) → FAIL", not r["ok"] and r["denominators_ok"], r))
    r = check_discoveries(["a", "b"], ["a", "b"], 0)
    cases.append(("SC-90: 고갈 0 → FAIL(분모 — 고갈 경로 미실행)", not r["ok"] and not r["denominators_ok"], r))
    r = check_discoveries(["a"], ["a"], 3)
    cases.append(("SC-90: 채굴 1 종 → FAIL(분모 — 0 = 0 류)", not r["ok"] and not r["denominators_ok"], r))
    r = check_conservation(no_regen, rows_nr, inv_nr, deposits, minerals, 1, 2, False)
    cases.append(("보존: 같은 입력, --require-regen 없으면 PASS (위 FAIL 이 분모 때문임을 가른다)", r["ok"], r))

    causes = [e["causation_id"] for e in ev]
    r = check_causation(causes, list(causes))
    cases.append(("인과: 양방향 일치 → PASS", r["ok"], r))
    r = check_causation([], [])
    cases.append(("인과: **둘 다 0 행 → FAIL**", not r["ok"], r))
    r = check_causation(causes, causes[:-1])
    cases.append(("인과: 장부 한 행 누락 → FAIL", not r["ok"] and r["missing_counts"] == [1, 0], r))
    r = check_accepted(causes[:3], causes, causes)
    cases.append(("수락: 봇 수락 ⊆ DB → PASS", r["ok"], r))
    r = check_accepted(causes[:3] + ["lost"], causes, causes)
    cases.append(("수락: 봇이 수락받았는데 DB 에 없음 → FAIL", not r["ok"], r))

    err = 'ERROR:  column "quantity_kg" does not exist'
    cases += [("없는 열 조회 → NotImplementedYet (0 행 PASS 가 아니다)",
               isinstance(db.classify_psql_failure(err), db.NotImplementedYet), err),
              ("연결 실패 → 미검증(환경)",
               isinstance(db.classify_psql_failure("service \"postgres\" is not running"), db.EnvironmentProblem), "")]
    fails = 0
    for name, ok, detail in cases:
        print(f"{'OK  ' if ok else 'FAIL'} {name}" + ("" if ok else f" :: {json.dumps(detail, ensure_ascii=False)[:300]}"))
        fails += 0 if ok else 1
    print(f"selftest: {'PASS' if fails == 0 else f'FAIL ({fails})'}  케이스={len(cases)}")
    return 0 if fails == 0 else 1


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--world")
    ap.add_argument("--evidence")
    ap.add_argument("mode", choices=["ledger", "conservation", "load", "accepted", "causation", "discoveries",
                                     "selftest"])
    ap.add_argument("--require-regen", action="store_true")
    ap.add_argument("--from", dest="from_file")
    args = ap.parse_args()
    if args.mode == "selftest":
        return selftest()
    if not args.world:
        ap.error("--world <새 월드 id> 가 필요하다 — 모든 SQL 은 world_id 로 한정한다(계약 §0.1)")
    return db.main_guard(lambda: run(args))


if __name__ == "__main__":
    sys.exit(main())
