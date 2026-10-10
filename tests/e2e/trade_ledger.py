"""p1-03 원장·연쇄·보존·포화 항등식 — 계약 SC-30 · 31 · 32 · 33 · 37 · 39 · 118 · 120 · 123.

    python tests/e2e/trade_ledger.py --world <id> shape          # 실행 모양(AC-6(a) 분모) — 판정 아님, 출력만
    python tests/e2e/trade_ledger.py --world <id> ledger         # SC-30  I-80
    python tests/e2e/trade_ledger.py --world <id> chain          # SC-31  I-81
    python tests/e2e/trade_ledger.py --world <id> conservation   # SC-32  I-84 (판매 항 포함)
    python tests/e2e/trade_ledger.py --world <id> conservation --without-sales-term   # 음성 대조(p1-02 식)
    python tests/e2e/trade_ledger.py --world <id> saturation     # SC-33  I-85
    python tests/e2e/trade_ledger.py --world <id> zero-rows      # SC-37  I-93
    python tests/e2e/trade_ledger.py --world <id> accepted --from <봇 수락 command_id 파일>   # SC-39
    python tests/e2e/trade_ledger.py --world <id> load           # SC-118 (부하 월드 — 넷 전부 + 모양)
    python tests/e2e/trade_ledger.py --world <id> market-events  # SC-120 바닥 ≥ 1 · 회복 ≥ 1
    python tests/e2e/trade_ledger.py --world <id> causation      # SC-123 채굴·판매 ↔ processed_commands
    python tests/e2e/trade_ledger.py selftest

**판매가 0 건이면 FAIL 로 인쇄한다**(스펙 I-85 끝 문장, CLAUDE.md). SC-30~33 은 계약 §1-F 머리의
**실행 모양 요건**(판매 actor ≥ 2 · 판매 ≥ 10 · 판매된 광물 ≥ 2 · 구간을 걸친 판매 ≥ 1 · 회복이 끼어든
쌍 ≥ 1)이 미달이면 FAIL 이다(스펙 AC-6(a) 가 FAIL 로 정했다).

**포화 재계산은 서버 값을 입력으로 쓰지 않는다**(계약 SC-33 ⊘). 입력은 이벤트의 `quantity_kg`·`tick`·
`(tick, sequence)` 순서, 그리고 그 시점의 `data/market/*.json` 상수뿐이다. 서버가 쓴 `proceeds_cr`·
`saturation_*`·`price_*` 는 **비교 대상**이다. 가격 식은 스펙 §4.2a·디자인 §3.3 을 파이썬으로 따로 옮긴 것이고,
selftest 가 디자인 §3.6·부록 B 대금 표(계약 §0.6 P1~P10)로 이 구현을 고정한다.

필드 이름은 `contracts/events/domain/MINERAL_SOLD.schema.json` 의 payload 정의에서 확인한다(계약 §0.10 —
디자인 작업 이름으로 쓰면 NULL 이 된다).

종료 코드: db.py 규약 (0 PASS / 1 FAIL / 2 미검증(환경) / 3 구현 없음).
"""

from __future__ import annotations

import argparse
import copy
import json
import random
import sys
from pathlib import Path

import db
import mining_ledger as ml

LABELS = {
    "ledger": {"item": "p1-03 SC-30 (AC-6a) 원장 항등식 I-80 — 인벤토리·지갑·화폐 총량"},
    "chain": {"item": "p1-03 SC-31 (AC-6a) 연쇄 I-81 — 수량·지갑"},
    "conservation": {"item": "p1-03 SC-32 (AC-6a) 질량 보존 I-84 — 판매 항 포함"},
    "conservation_neg": {"item": "참고 — p1-03 SC-32 음성 대조: 판매 항 없는 p1-02 식(FAIL 이어야 한다)"},
    "saturation": {"item": "p1-03 SC-33 (AC-6a) 포화 재계산 I-85"},
    "zero_rows": {"item": "p1-03 SC-37 (AC-6d) 전량 판매 뒤 인벤토리 행 0 으로 존재(DELETE 없음)"},
    "accepted": {"item": "p1-03 SC-39 (AC-7) 기록 지연 구간 전후 수락 판매 전부 DB 에 + 원장"},
    "load": {"item": "p1-03 SC-118 (AC-20b) 부하 월드 항등식 전부 + 실행 모양"},
    "market_events": {"item": "p1-03 SC-120 (AC-20b) 바닥 도달 광물 ≥ 1 · 회복 관찰 광물 ≥ 1"},
    "causation": {"item": "p1-03 SC-123 (AC-21b) causation ↔ processed_commands 양방향 + 종류 일치"},
    "shape": {"item": "참고 — p1-03 실행 모양(AC-6(a) 분모)"},
}

# 계약 §1-F 머리 — 바꾸려면 계약을 먼저 바꾼다(규칙 2).
SHAPE_MIN = {"sellers": 2, "sales": 10, "minerals_sold": 2, "band_crossing_sales": 1, "recovery_pairs": 1}

SOLD_FIELDS = ["market_id", "mineral_id", "pricing_version", "quantity_kg", "proceeds_cr",
               "base_price_cr_per_kg", "floor_price_cr_per_kg", "price_before_cr_per_kg", "price_after_cr_per_kg",
               "saturation_before_kg", "saturation_after_kg", "quantity_before_kg", "quantity_after_kg",
               "wallet_before_cr", "wallet_after_cr"]
SOLD_STR = {"market_id", "mineral_id", "pricing_version"}
COMMAND_OF = {"MINERAL_MINED": "MINE_RESOURCE", "MINERAL_SOLD": "SELL_MINERAL"}


# ── 가격 규칙 (스펙 §4.2a — 독립 구현) ─────────────────────────────────────────────

def market_consts(market: dict, mineral_id: str, tick_hz: int) -> dict:
    r = market["rules"]
    li = next(x for x in market["listings"] if x["mineral_id"] == mineral_id)
    floor_band = -(-(100 - r["price_floor_pct"]) // r["price_drop_pct_per_band"])     # ceil
    return {"base": li["base_price_cr_per_kg"], "band_kg": li["band_kg"], "recovery_kg": li["recovery_kg"],
            "drop": r["price_drop_pct_per_band"], "floor_pct": r["price_floor_pct"], "floor_band": floor_band,
            "cap": floor_band * li["band_kg"], "interval": r["recovery_interval_s"] * tick_hz,
            "pricing_version": market["pricing_version"]}


def band(c: dict, s: int) -> int:
    return min(s // c["band_kg"], c["floor_band"])


def price(c: dict, b: int) -> int:
    return c["base"] * max(c["floor_pct"], 100 - b * c["drop"]) // 100


def sell(c: dict, s: int, q: int) -> tuple[int, int, int]:
    """(대금, 판매 뒤 포화, 반복 수). 디자인 §3.3 의 의사 코드 그대로."""
    proceeds, it = 0, 0
    while q > 0:
        it += 1
        b = band(c, s)
        if b == c["floor_band"]:
            proceeds += q * price(c, c["floor_band"])
            s, q = c["cap"], 0
        else:
            take = min(q, (b + 1) * c["band_kg"] - s)
            proceeds += take * price(c, b)
            s, q = s + take, q - take
    return proceeds, s, it


def recover(c: dict, s: int, as_of: int, t: int) -> int:
    """I-87: max(0, s − recovery_kg × (⌊t/I⌋ − ⌊as_of/I⌋))."""
    return max(0, s - c["recovery_kg"] * (t // c["interval"] - as_of // c["interval"]))


def crosses_band(c: dict, before: int, after: int) -> bool:
    return after > before and band(c, before) != band(c, after - 1)


# ── 순수 판정 (selftest 가 DB 없이 부른다) ───────────────────────────────────────

def _by(key, evs):
    out: dict = {}
    for e in evs:
        out.setdefault(key(e), []).append(e)
    return out


def shape(sold: list[dict], consts: dict) -> dict:
    """consts: (market_id, mineral_id) → market_consts."""
    crossing = sum(1 for e in sold if crosses_band(consts[(e["market_id"], e["mineral_id"])],
                                                    e["saturation_before_kg"], e["saturation_after_kg"]))
    pairs = 0
    for key, evs in _by(lambda e: (e["market_id"], e["mineral_id"]), sold).items():
        c = consts[key]
        for a, b in zip(evs, evs[1:]):
            if b["tick"] // c["interval"] > a["tick"] // c["interval"] and b["saturation_before_kg"] < a["saturation_after_kg"]:
                pairs += 1
    got = {"sellers": len({e["actor_id"] for e in sold}), "sales": len(sold),
           "minerals_sold": len({e["mineral_id"] for e in sold}), "band_crossing_sales": crossing,
           "recovery_pairs": pairs}
    short = {k: [got[k], v] for k, v in SHAPE_MIN.items() if got[k] < v}
    return {"shape": got, "shape_min": SHAPE_MIN, "shape_short": short, "shape_ok": not short}


def check_ledger(mined: list[dict], sold: list[dict], inventory: list[dict], wallets: list[dict]) -> dict:
    problems = []
    inv = {(r["actor_id"], r["mineral_id"]): r["quantity_kg"] for r in inventory}
    keys = {(e["actor_id"], e["mineral_id"]) for e in mined + sold}
    for key in sorted(keys):
        m = sum(e["quantity_kg"] for e in mined if (e["actor_id"], e["mineral_id"]) == key)
        s = sum(e["quantity_kg"] for e in sold if (e["actor_id"], e["mineral_id"]) == key)
        evs = sorted([e for e in mined + sold if (e["actor_id"], e["mineral_id"]) == key],
                     key=lambda e: (e["tick"], e["sequence"]))
        last = evs[-1]["quantity_after_kg"]
        row = inv.get(key)
        if not (row == m - s == last):
            problems.append({"key": key, "why": "인벤토리 행 = Σ채굴 − Σ판매 = 최신 after 불성립",
                             "row": row, "mined_minus_sold": m - s, "last_after": last})
    for key, q in inv.items():
        if key not in keys and q != 0:
            problems.append({"key": key, "why": "이벤트 없는 인벤토리 행", "row": q})
    wal = {r["actor_id"]: r["balance_cr"] for r in wallets}
    sellers = sorted({e["actor_id"] for e in sold})
    for a in sellers:
        evs = [e for e in sold if e["actor_id"] == a]
        total = sum(e["proceeds_cr"] for e in evs)
        last = evs[-1]["wallet_after_cr"]
        if not (wal.get(a, 0) == total == last):
            problems.append({"actor": a, "why": "지갑 행 = Σ대금 = 최신 wallet_after 불성립",
                             "row": wal.get(a), "sum": total, "last_after": last})
    for a, b in wal.items():
        if a not in sellers and b != 0:
            problems.append({"actor": a, "why": "판매 없는 지갑 행", "row": b})
    money, proceeds = sum(wal.values()), sum(e["proceeds_cr"] for e in sold)
    if money != proceeds:
        problems.append({"why": "화폐 총량 Σ지갑 ≠ Σ대금", "wallets": money, "proceeds": proceeds})
    return {"inventory_keys_checked": len(keys), "events_mineral_mined": len(mined), "events_mineral_sold": len(sold),
            "wallets_checked": len(wal), "sellers": len(sellers), "money_supply_cr": money,
            "sum_proceeds_cr": proceeds, "denominators_ok": len(sold) > 0, "problems": problems,
            "ok": len(sold) > 0 and not problems}


def check_chain(mined: list[dict], sold: list[dict]) -> dict:
    problems, qlinks, wlinks, transitions = [], 0, 0, 0
    for key, evs in _by(lambda e: (e["actor_id"], e["mineral_id"]),
                        sorted(mined + sold, key=lambda e: (e["tick"], e["sequence"]))).items():
        if evs[0]["quantity_before_kg"] != 0:
            problems.append({"key": key, "why": "첫 이벤트 quantity_before ≠ 0"})
        for a, b in zip(evs, evs[1:]):
            qlinks += 1
            transitions += a["event_type"] != b["event_type"]
            if a["quantity_after_kg"] != b["quantity_before_kg"]:
                problems.append({"key": key, "why": "앞.quantity_after ≠ 뒤.quantity_before", "at": [b["tick"], b["sequence"]]})
    for e in sold:
        if e["quantity_after_kg"] != e["quantity_before_kg"] - e["quantity_kg"]:
            problems.append({"at": [e["tick"], e["sequence"]], "why": "판매 after ≠ before − quantity"})
        if e["wallet_after_cr"] != e["wallet_before_cr"] + e["proceeds_cr"]:
            problems.append({"at": [e["tick"], e["sequence"]], "why": "wallet_after ≠ before + proceeds"})
    for e in mined:
        if e["quantity_after_kg"] != e["quantity_before_kg"] + e["quantity_kg"]:
            problems.append({"at": [e["tick"], e["sequence"]], "why": "채굴 after ≠ before + quantity"})
    for actor, evs in _by(lambda e: e["actor_id"], sold).items():
        if evs[0]["wallet_before_cr"] != 0:
            problems.append({"actor": actor, "why": "첫 판매 wallet_before ≠ 0"})
        for a, b in zip(evs, evs[1:]):
            wlinks += 1
            if a["wallet_after_cr"] != b["wallet_before_cr"]:
                problems.append({"actor": actor, "why": "앞.wallet_after ≠ 뒤.wallet_before", "at": [b["tick"], b["sequence"]]})
    denom = qlinks > 0 and transitions > 0 and len(sold) > 0
    return {"quantity_links": qlinks, "mine_sell_transitions": transitions, "wallet_links": wlinks,
            "events_mineral_sold": len(sold), "denominators_ok": denom, "problems": problems,
            "ok": denom and not problems}


def check_conservation(mined, sold, deposit_rows, inventory, deposits, minerals, tick_hz,
                       with_sales_term: bool = True) -> dict:
    base = ml.check_conservation(mined, deposit_rows, inventory, deposits, minerals, tick_hz, 0, False)
    problems = [p for p in base["problems"] if "mineral" not in p]
    per = []
    for row in base["per_mineral"]:
        mid = row["mineral_id"]
        sold_kg = sum(e["quantity_kg"] for e in sold if e["mineral_id"] == mid)
        lhs = row["lhs_inventory_plus_remaining"] + (sold_kg if with_sales_term else 0)
        rhs = row["rhs_initial_plus_regen"]
        per.append({"mineral_id": mid, "mined_events": row["mined_events"],
                    "sold_events": sum(1 for e in sold if e["mineral_id"] == mid), "sold_kg": sold_kg,
                    "inventory_plus_remaining": row["lhs_inventory_plus_remaining"], "lhs": lhs,
                    "rhs_initial_plus_regen": rhs, "regen_kg": row["regen_kg"], "holds": lhs == rhs})
        if lhs != rhs:
            problems.append({"mineral": mid, "why": "Σ인벤토리 + Σ잔량 + Σ판매 ≠ Σ초기 + Σ회복", "lhs": lhs, "rhs": rhs})
    sold_minerals = sum(1 for r in per if r["sold_kg"] > 0)
    denom = sold_minerals >= 2
    return {"with_sales_term": with_sales_term, "events_mineral_mined": len(mined), "events_mineral_sold": len(sold),
            "minerals_with_sales": sold_minerals, "deposit_chain_links": base["chain_links_checked"],
            "per_mineral": per, "denominators_ok": denom, "problems": problems, "ok": denom and not problems}


def check_saturation(sold: list[dict], consts: dict, market_rows: list[dict]) -> dict:
    problems, checked, pairs = [], 0, 0
    rows = {(r["market_id"], r["mineral_id"]): r for r in market_rows}
    groups = _by(lambda e: (e["market_id"], e["mineral_id"]), sold)
    for key, evs in sorted(groups.items()):
        c = consts[key]
        s, as_of = 0, None
        for e in evs:
            checked += 1
            at = [e["tick"], e["sequence"]]
            s_eff = 0 if as_of is None else recover(c, s, as_of, e["tick"])
            if as_of is not None and e["tick"] // c["interval"] > as_of // c["interval"] and s_eff < s:
                pairs += 1
            exp_proceeds, exp_after, _ = sell(c, s_eff, e["quantity_kg"])
            expect = {"saturation_before_kg": s_eff, "saturation_after_kg": exp_after, "proceeds_cr": exp_proceeds,
                      "price_before_cr_per_kg": price(c, band(c, s_eff)),
                      "price_after_cr_per_kg": price(c, band(c, exp_after)),
                      "base_price_cr_per_kg": c["base"], "floor_price_cr_per_kg": price(c, c["floor_band"]),
                      "pricing_version": c["pricing_version"]}
            for f, v in expect.items():
                if e[f] != v:
                    problems.append({"key": key, "at": at, "field": f, "recomputed": v, "server": e[f]})
            s, as_of = exp_after, e["tick"]          # 재계산 값으로 이어 간다 — 서버 값으로 갈아타지 않는다
        last = evs[-1]
        row = rows.get(key)
        if row is None:
            problems.append({"key": key, "why": "판매가 있는데 market_states 행이 없다"})
        elif (row["saturation_kg"], row["as_of_tick"], row["last_seller_actor_id"], row["last_sale_quantity_kg"]) != \
                (s, last["tick"], last["actor_id"], last["quantity_kg"]):
            problems.append({"key": key, "why": "market_states 행 ≠ 마지막 판매(재계산 포화·tick·판매자·kg)",
                             "row": [row["saturation_kg"], row["as_of_tick"], row["last_seller_actor_id"], row["last_sale_quantity_kg"]],
                             "expected": [s, last["tick"], last["actor_id"], last["quantity_kg"]]})
    for key in rows:
        if key not in groups:
            problems.append({"key": key, "why": "판매 없는 market_states 행"})
    denom = checked > 0
    return {"sales_recomputed": checked, "market_mineral_keys": len(groups), "recovery_pairs": pairs,
            "market_rows": len(rows), "denominators_ok": denom, "problems": problems[:50],
            "problem_count": len(problems), "ok": denom and not problems}


def check_zero_rows(sold: list[dict], inventory: list[dict]) -> dict:
    inv = {(r["actor_id"], r["mineral_id"]): r["quantity_kg"] for r in inventory}
    full = [e for e in sold if e["quantity_after_kg"] == 0]
    # 그 키의 마지막 이벤트가 그 전량 판매여야 행이 0 이다 — 뒤에 채굴이 있으면 행은 0 이 아니어도 된다.
    last_full = {}
    for e in full:
        last_full[(e["actor_id"], e["mineral_id"])] = e
    problems = []
    zero_rows = sum(1 for q in inv.values() if q == 0)
    for key, e in last_full.items():
        if key not in inv:
            problems.append({"key": key, "why": "전량 판매 뒤 인벤토리 행이 없다(DELETE 경로)"})
    denom = len(full) > 0 and zero_rows > 0
    return {"full_sales": len(full), "keys_after_full_sale": len(last_full), "inventory_rows": len(inv),
            "zero_quantity_rows": zero_rows, "denominators_ok": denom, "problems": problems,
            "ok": denom and not problems}


def check_causation(mined: list[dict], sold: list[dict], processed: list[dict]) -> dict:
    ev = {e["causation_id"]: e["event_type"] for e in mined + sold}
    pc = {p["command_id"]: p["command_type"] for p in processed}
    dup = len(mined) + len(sold) - len(ev)
    a, b = set(ev), set(pc)
    wrong = sorted(k for k in a & b if COMMAND_OF.get(ev[k]) != pc[k])
    sell_rows = sum(1 for t in pc.values() if t == "SELL_MINERAL")
    denom = len(mined) > 0 and len(sold) > 0 and sell_rows > 0
    ok = denom and a == b and not wrong and dup == 0
    return {"mined_rows": len(mined), "sold_rows": len(sold), "processed_rows": len(pc),
            "processed_sell_rows": sell_rows, "missing_counts": [len(a - b), len(b - a)],
            "event_without_command": sorted(a - b)[:20], "command_without_event": sorted(b - a)[:20],
            "kind_mismatch": wrong[:20], "duplicate_causation": dup, "denominators_ok": denom, "ok": ok}


def check_market_events(sold: list[dict], consts: dict) -> dict:
    floor_minerals = sorted({e["mineral_id"] for e in sold if e["price_after_cr_per_kg"] == e["floor_price_cr_per_kg"]})
    rec = set()
    for key, evs in _by(lambda e: (e["market_id"], e["mineral_id"]), sold).items():
        c = consts[key]
        for a, b in zip(evs, evs[1:]):
            if b["tick"] // c["interval"] > a["tick"] // c["interval"] and b["saturation_before_kg"] < a["saturation_after_kg"]:
                rec.add(key[1])
    per = {}
    for e in sold:
        per[e["mineral_id"]] = per.get(e["mineral_id"], 0) + e["quantity_kg"]
    ok = len(floor_minerals) >= 1 and len(rec) >= 1
    return {"sales": len(sold), "sold_kg_by_mineral": per, "floor_reached_minerals": floor_minerals,
            "recovery_observed_minerals": sorted(rec), "denominators_ok": ok, "ok": ok}


def check_accepted(bot_accepted: list[str], processed: list[dict], sold: list[dict]) -> dict:
    acc = set(bot_accepted)
    p = {x["command_id"] for x in processed if x["command_type"] == "SELL_MINERAL"}
    c = {e["causation_id"] for e in sold}
    return {"bot_accepted_sales": len(acc), "missing_in_processed_commands": sorted(acc - p)[:20],
            "missing_in_mineral_sold": sorted(acc - c)[:20], "missing_counts": [len(acc - p), len(acc - c)],
            "denominators_ok": bool(acc), "ok": bool(acc) and acc <= p and acc <= c}


# ── 데이터·DB 적재 ────────────────────────────────────────────────────────────

def schema_payload_props(root: Path) -> set[str]:
    s = json.loads((root / "contracts/events/domain/MINERAL_SOLD.schema.json").read_text(encoding="utf-8"))
    return set(s["$defs"]["MineralSoldPayload"]["properties"])


def load_data(root: Path) -> tuple[dict, dict, dict, dict]:
    """광물(회복), 광맥(공개부 + 예비 합침 — ADR-0015), 시장(파일 id → 표)."""
    minerals = {}
    for f in sorted((root / "data/minerals").glob("*.json")):
        d = json.loads(f.read_text(encoding="utf-8"))
        minerals[d["id"]] = {"regen_kg": d["regeneration"]["regen_kg"], "regen_interval_s": d["regeneration"]["regen_interval_s"]}
    reserves = {}
    for f in sorted((root / "data/world/deposit-reserves").glob("*.json")):
        d = json.loads(f.read_text(encoding="utf-8"))
        for r in d.get("reserves", d.get("deposits", [])):
            reserves[r["deposit_id"]] = r
    deposits = {}
    for f in sorted((root / "data/world/deposits").glob("*.json")):
        for d in json.loads(f.read_text(encoding="utf-8"))["deposits"]:
            r = reserves[d["id"]]
            deposits[d["id"]] = {"mineral_id": r["mineral_id"], "initial_reserve_kg": r["initial_reserve_kg"]}
    markets = {}
    for f in sorted((root / "data/market").glob("*.json")):
        m = json.loads(f.read_text(encoding="utf-8"))
        markets[m["id"]] = m
    echo = {"minerals": minerals, "deposits": deposits,
            "markets": {k: {"pricing_version": v["pricing_version"], "rules": v["rules"], "listings": v["listings"]}
                        for k, v in markets.items()}}
    return minerals, deposits, markets, echo


def consts_for(sold: list[dict], markets: dict, tick_hz: int) -> dict:
    out = {}
    for e in sold:
        key = (e["market_id"], e["mineral_id"])
        if key not in out:
            if e["market_id"] not in markets:
                raise db.NotImplementedYet(f"data/market 에 없는 market_id: {e['market_id']}")
            out[key] = market_consts(markets[e["market_id"]], e["mineral_id"], tick_hz)
    return out


def fetch_mined(world: str) -> list[dict]:
    out = ml.fetch_events(world)
    for e in out:
        e["event_type"] = "MINERAL_MINED"
    return out


def fetch_sold(world: str) -> list[dict]:
    w = ml._uuid(world)
    props = schema_payload_props(db.REPO_ROOT)
    missing = [f for f in SOLD_FIELDS if f not in props]
    if missing:
        raise db.NotImplementedYet(f"계약 스키마에 없는 필드 이름을 쓰려 했다(§0.10): {missing}")
    db.require_payload_keys("domain_events", "payload", "MINERAL_SOLD", SOLD_FIELDS, f"world_id = '{w}'", allow_empty=True)
    cols = ", ".join(f"payload->>'{f}'" for f in SOLD_FIELDS)
    rows = db.psql_rows_strict(
        f"select actor_id, tick, sequence, causation_id, {cols} from domain_events "
        f"where world_id = '{w}' and event_type = 'MINERAL_SOLD' order by tick, sequence;")
    out = []
    for r in rows:
        if len(r) != 4 + len(SOLD_FIELDS) or "" in r:
            raise db.NotImplementedYet(f"MINERAL_SOLD 행에 빈 필드(이름 불일치?): {r}")
        e = {"event_type": "MINERAL_SOLD", "actor_id": r[0], "tick": int(r[1]), "sequence": int(r[2]), "causation_id": r[3]}
        for f, v in zip(SOLD_FIELDS, r[4:]):
            e[f] = v if f in SOLD_STR else int(v)
        out.append(e)
    return out


def fetch_wallets(world: str) -> list[dict]:
    db.require_tables("wallets")
    return [{"actor_id": a, "balance_cr": int(b)} for a, b in db.psql_rows_strict(
        f"select actor_id, balance_cr from wallets where world_id = '{ml._uuid(world)}';")]


def fetch_market_rows(world: str) -> list[dict]:
    db.require_tables("market_states")
    return [{"market_id": m, "mineral_id": n, "saturation_kg": int(s), "as_of_tick": int(t),
             "last_seller_actor_id": a, "last_sale_quantity_kg": int(q)}
            for m, n, s, t, a, q in db.psql_rows_strict(
                "select market_id, mineral_id, saturation_kg, as_of_tick, last_seller_actor_id, last_sale_quantity_kg "
                f"from market_states where world_id = '{ml._uuid(world)}';")]


def fetch_processed(world: str) -> list[dict]:
    db.require_tables("processed_commands")
    return [{"command_id": c, "command_type": t} for c, t in db.psql_rows_strict(
        f"select command_id, command_type from processed_commands where world_id = '{ml._uuid(world)}';")]


def verdict(result: dict, label: dict, need_shape: dict | None = None) -> dict:
    out = dict(label)
    shape_ok = True
    if need_shape is not None:
        out.update(need_shape)
        shape_ok = need_shape["shape_ok"]
    if result["ok"] and shape_ok:
        out["verdict"] = "PASS"
    elif not result.get("denominators_ok", True) or not shape_ok:
        out["verdict"] = "FAIL(분모 미달 — 겨냥한 조건이 이번 실행에서 충분히 일어나지 않았다)"
    else:
        out["verdict"] = "FAIL"
    out.update(result)
    return out


def run(a: argparse.Namespace) -> int:
    root, w = db.REPO_ROOT, a.world
    minerals, deposits, markets, echo = load_data(root)
    hz = ml.tick_hz_of(w)
    mined, sold = fetch_mined(w), fetch_sold(w)
    consts = consts_for(sold, markets, hz)
    shp = shape(sold, consts)
    if a.mode == "shape":
        res = dict(LABELS["shape"], verdict="참고", **shp, ok=True)
    elif a.mode == "ledger":
        res = verdict(check_ledger(mined, sold, ml.fetch_inventory(w), fetch_wallets(w)), LABELS["ledger"], shp)
    elif a.mode == "chain":
        res = verdict(check_chain(mined, sold), LABELS["chain"], shp)
    elif a.mode == "conservation":
        r = check_conservation(mined, sold, ml.fetch_deposit_rows(w), ml.fetch_inventory(w), deposits, minerals, hz,
                               not a.without_sales_term)
        res = verdict(r, LABELS["conservation_neg" if a.without_sales_term else "conservation"],
                      None if a.without_sales_term else shp)
    elif a.mode == "saturation":
        res = verdict(check_saturation(sold, consts, fetch_market_rows(w)), LABELS["saturation"], shp)
    elif a.mode == "zero-rows":
        res = verdict(check_zero_rows(sold, ml.fetch_inventory(w)), LABELS["zero_rows"])
    elif a.mode == "accepted":
        if not a.from_file:
            raise db.EnvironmentProblem("accepted 는 --from <봇 수락 SELL command_id 파일> 이 필요하다")
        acc = check_accepted(db.read_correlations(a.from_file), fetch_processed(w), sold)
        led = check_ledger(mined, sold, ml.fetch_inventory(w), fetch_wallets(w))
        res = verdict({"accepted": acc, "ledger": led, "ok": acc["ok"] and led["ok"],
                       "denominators_ok": acc["denominators_ok"] and led["denominators_ok"]}, LABELS["accepted"])
    elif a.mode == "load":
        inv = ml.fetch_inventory(w)
        parts = {"ledger": check_ledger(mined, sold, inv, fetch_wallets(w)), "chain": check_chain(mined, sold),
                 "conservation": check_conservation(mined, sold, ml.fetch_deposit_rows(w), inv, deposits, minerals, hz),
                 "saturation": check_saturation(sold, consts, fetch_market_rows(w))}
        res = verdict({**parts, "ok": all(p["ok"] for p in parts.values()),
                       "denominators_ok": all(p["denominators_ok"] for p in parts.values())}, LABELS["load"], shp)
    elif a.mode == "market-events":
        res = verdict(check_market_events(sold, consts), LABELS["market_events"])
    elif a.mode == "causation":
        res = verdict(check_causation(mined, sold, fetch_processed(w)), LABELS["causation"])
    else:
        raise SystemExit(f"모르는 mode {a.mode}")
    res["world_id"] = w
    res["data_values_at_run"] = echo
    db.emit(res, a.evidence)
    return db.EXIT_OK if res["verdict"] in ("PASS", "참고") else db.EXIT_FAIL


# ── selftest ────────────────────────────────────────────────────────────────

def _market() -> dict:
    """data/market/cradle-exchange.json 의 값을 **디자인 §3.6 에서** 옮긴 합성 표(파일을 읽지 않는다)."""
    return {"id": "m", "pricing_version": "m@1",
            "rules": {"price_drop_pct_per_band": 10, "price_floor_pct": 30, "recovery_interval_s": 60},
            "listings": [{"mineral_id": "ferrosite", "base_price_cr_per_kg": 10, "band_kg": 2000, "recovery_kg": 1000},
                         {"mineral_id": "glacine", "base_price_cr_per_kg": 14, "band_kg": 1500, "recovery_kg": 750},
                         {"mineral_id": "cobaltine", "base_price_cr_per_kg": 40, "band_kg": 600, "recovery_kg": 300},
                         {"mineral_id": "starfall-glass", "base_price_cr_per_kg": 250, "band_kg": 100, "recovery_kg": 50}]}


# 계약 §0.6 P1~P10 — 디자인 §3.6·부록 B (테스트 출력이 아니다)
DESIGN_TABLE = [("ferrosite", 0, 12000, 90000), ("ferrosite", 0, 14000, 98000), ("glacine", 0, 10000, 96500),
                ("glacine", 0, 10500, 99000), ("cobaltine", 0, 3200, 100000), ("cobaltine", 0, 2400, 81600),
                ("starfall-glass", 0, 500, 100000), ("starfall-glass", 0, 700, 122500),
                ("ferrosite", 0, 2000, 20000), ("ferrosite", 2000, 2000, 18000)]


def _sale(c, actor, mineral, tick, seq, q, s_before, qb, wb, market="m"):
    p, after, _ = sell(c, s_before, q)
    return {"event_type": "MINERAL_SOLD", "actor_id": actor, "tick": tick, "sequence": seq,
            "causation_id": f"s-{actor}-{tick}-{seq}", "market_id": market, "mineral_id": mineral,
            "pricing_version": c["pricing_version"], "quantity_kg": q, "proceeds_cr": p,
            "base_price_cr_per_kg": c["base"], "floor_price_cr_per_kg": price(c, c["floor_band"]),
            "price_before_cr_per_kg": price(c, band(c, s_before)), "price_after_cr_per_kg": price(c, band(c, after)),
            "saturation_before_kg": s_before, "saturation_after_kg": after,
            "quantity_before_kg": qb, "quantity_after_kg": qb - q, "wallet_before_cr": wb, "wallet_after_cr": wb + p}


def _mine(actor, mineral, dep, tick, seq, q, qb, dbefore):
    e = ml._ev(actor, mineral, dep, tick, seq, q, qb, dbefore)
    e["event_type"] = "MINERAL_MINED"
    return e


def _world():
    """tick_hz 1 → 회복 간격 60 tick. A·B 가 ferrosite, C 가 glacine 을 캐고 판다."""
    mk = _market()
    fe, gl = market_consts(mk, "ferrosite", 1), market_consts(mk, "glacine", 1)
    minerals = {"ferrosite": {"regen_kg": 1, "regen_interval_s": 100000},
                "glacine": {"regen_kg": 1, "regen_interval_s": 100000}}
    deposits = {"ib1": {"mineral_id": "ferrosite", "initial_reserve_kg": 50000},
                "vo2": {"mineral_id": "glacine", "initial_reserve_kg": 50000}}
    mined, sold = [], []
    dep = {"ib1": 50000, "vo2": 50000}
    hold, wal, sat, as_of = {}, {}, {}, {}
    plan = [("A", "ferrosite", "ib1", 10), ("B", "ferrosite", "ib1", 70), ("A", "ferrosite", "ib1", 130),
            ("B", "ferrosite", "ib1", 200), ("A", "ferrosite", "ib1", 260), ("B", "ferrosite", "ib1", 330),
            ("C", "glacine", "vo2", 340), ("A", "ferrosite", "ib1", 400), ("B", "ferrosite", "ib1", 460),
            ("C", "glacine", "vo2", 520)]
    for actor, mineral, d, t in plan:
        q = 2000
        mined.append(_mine(actor, mineral, d, t, 0, q, hold.get((actor, mineral), 0), dep[d]))
        dep[d] -= q
        hold[(actor, mineral)] = hold.get((actor, mineral), 0) + q
        c = fe if mineral == "ferrosite" else gl
        s = 0 if mineral not in as_of else recover(c, sat[mineral], as_of[mineral], t + 5)
        e = _sale(c, actor, mineral, t + 5, 0, q, s, hold[(actor, mineral)], wal.get(actor, 0))
        sold.append(e)
        hold[(actor, mineral)] -= q
        wal[actor] = e["wallet_after_cr"]
        sat[mineral], as_of[mineral] = e["saturation_after_kg"], t + 5
    inv = [{"actor_id": a, "mineral_id": m, "quantity_kg": q} for (a, m), q in hold.items()]
    wallets = [{"actor_id": a, "balance_cr": b} for a, b in wal.items()]
    rows = [{"deposit_id": "ib1", "remaining_kg": dep["ib1"], "as_of_tick": max(e["tick"] for e in mined if e["deposit_id"] == "ib1"),
             "first_extracted_tick": 10},
            {"deposit_id": "vo2", "remaining_kg": dep["vo2"], "as_of_tick": 520, "first_extracted_tick": 340}]
    mrows = []
    for mineral in ("ferrosite", "glacine"):
        last = [e for e in sold if e["mineral_id"] == mineral][-1]
        mrows.append({"market_id": "m", "mineral_id": mineral, "saturation_kg": last["saturation_after_kg"],
                      "as_of_tick": last["tick"], "last_seller_actor_id": last["actor_id"],
                      "last_sale_quantity_kg": last["quantity_kg"]})
    processed = [{"command_id": e["causation_id"], "command_type": COMMAND_OF[e["event_type"]]} for e in mined + sold]
    consts = {("m", "ferrosite"): fe, ("m", "glacine"): gl}
    return dict(mined=mined, sold=sold, inv=inv, wallets=wallets, rows=rows, mrows=mrows, processed=processed,
                consts=consts, minerals=minerals, deposits=deposits)


def selftest() -> int:
    W = _world()
    cases = []
    mk = _market()
    # 가격 구현을 디자인 표로 고정
    got = [sell(market_consts(mk, m, 20), s, q)[0] for m, s, q, _ in DESIGN_TABLE]
    cases.append(("가격: 디자인 §3.6·부록 B 대금 표 P1~P10 일치", got == [p for *_, p in DESIGN_TABLE], got))
    fe = market_consts(mk, "ferrosite", 20)
    cases.append(("가격: 2^31−1 kg 반복 ≤ floor_band + 1 = 8", sell(fe, 0, 2**31 - 1)[2] <= fe["floor_band"] + 1, None))
    cases.append(("회복 T-8: 14,000 → 배수 1 → 13,000(시세 4) → 12,000",
                  recover(fe, 14000, 1199, 1200) == 13000 and price(fe, band(fe, 13000)) == 4
                  and recover(fe, 13000, 1200, 2400) == 12000, None))
    rnd = random.Random(7)
    ok_split = True
    for _ in range(200):
        s0, total = rnd.randrange(0, 14001), rnd.randrange(1, 20000)
        cut = rnd.randrange(0, total + 1)
        p1, s1, _ = sell(fe, s0, cut) if cut else (0, s0, 0)
        p2, _, _ = sell(fe, s1, total - cut) if total - cut else (0, s1, 0)
        ok_split &= (p1 + p2) == sell(fe, s0, total)[0]
    cases.append(("가격: 분할 무관(시드 7, 200 회) — 재계산기 자체의 성질", ok_split, None))

    sh = shape(W["sold"], W["consts"])
    cases.append(("모양: 합성 월드는 요건 충족(판매 10, actor 3, 광물 2, 걸침·회복 쌍 ≥ 1)", sh["shape_ok"], sh))
    sh0 = shape([], W["consts"])
    cases.append(("모양: 판매 0 → 요건 미달", not sh0["shape_ok"], sh0))

    r = check_ledger(W["mined"], W["sold"], W["inv"], W["wallets"])
    cases.append(("원장: 정상 → PASS", r["ok"], r))
    r = check_ledger(W["mined"], [], W["inv"], [])
    cases.append(("원장: **판매 0 건 → FAIL**", not r["ok"], r))
    wb = copy.deepcopy(W["wallets"])
    wb[0]["balance_cr"] += 1
    r = check_ledger(W["mined"], W["sold"], W["inv"], wb)
    cases.append(("원장: 지갑 행 변조 → FAIL(+ 화폐 총량)", not r["ok"] and any("화폐" in p.get("why", "") for p in r["problems"]), r))
    ib = copy.deepcopy(W["inv"])
    ib[0]["quantity_kg"] += 5
    r = check_ledger(W["mined"], W["sold"], ib, W["wallets"])
    cases.append(("원장: 인벤토리 변조 → FAIL", not r["ok"], r))

    r = check_chain(W["mined"], W["sold"])
    cases.append(("연쇄: 정상 → PASS(채굴→판매 전이 ≥ 1)", r["ok"] and r["mine_sell_transitions"] > 0, r))
    sb = copy.deepcopy(W["sold"])
    sb[3]["wallet_before_cr"] += 1
    sb[3]["wallet_after_cr"] += 1
    r = check_chain(W["mined"], sb)
    cases.append(("연쇄: 지갑 연쇄 한 칸 어긋남 → FAIL", not r["ok"], r))

    r = check_conservation(W["mined"], W["sold"], W["rows"], W["inv"], W["deposits"], W["minerals"], 1)
    cases.append(("보존: 판매 항 포함 → PASS", r["ok"], r))
    r = check_conservation(W["mined"], W["sold"], W["rows"], W["inv"], W["deposits"], W["minerals"], 1, False)
    cases.append(("보존 음성 대조: 판매 항 없는 p1-02 식 → FAIL(스펙 §10-17)", not r["ok"]
                  and any(p.get("mineral") for p in r["problems"]), r))

    r = check_saturation(W["sold"], W["consts"], W["mrows"])
    cases.append(("포화: 정상 → PASS, 회복 쌍 ≥ 1", r["ok"] and r["recovery_pairs"] >= 1, r))
    sb = copy.deepcopy(W["sold"])
    sb[2]["proceeds_cr"] += 1
    r = check_saturation(sb, W["consts"], W["mrows"])
    cases.append(("포화: 서버 대금 1 cr 틀림 → FAIL(서버 값을 입력으로 쓰지 않는다)", not r["ok"]
                  and any(p.get("field") == "proceeds_cr" for p in r["problems"]), r))
    sb = copy.deepcopy(W["sold"])
    sb[2]["saturation_before_kg"] = sb[1]["saturation_after_kg"]           # 회복을 빠뜨린 서버
    r = check_saturation(sb, W["consts"], W["mrows"])
    cases.append(("포화: 회복 누락 → FAIL", not r["ok"], r))
    mb = copy.deepcopy(W["mrows"])
    mb[0]["last_seller_actor_id"] = "Z"
    r = check_saturation(W["sold"], W["consts"], mb)
    cases.append(("포화: market_states 마지막 판매자 불일치 → FAIL", not r["ok"], r))
    r = check_saturation([], W["consts"], [])
    cases.append(("포화: 판매 0 → FAIL", not r["ok"], r))

    r = check_zero_rows(W["sold"], W["inv"])
    cases.append(("0 행: 전량 판매 뒤 행 0 유지 → PASS", r["ok"], r))
    r = check_zero_rows(W["sold"], [x for x in W["inv"] if x["quantity_kg"] != 0])
    cases.append(("0 행: 행이 지워짐 → FAIL", not r["ok"], r))

    r = check_causation(W["mined"], W["sold"], W["processed"])
    cases.append(("인과: 양방향·종류 일치 → PASS", r["ok"], r))
    pb = copy.deepcopy(W["processed"])
    pb[-1]["command_type"] = "MINE_RESOURCE"
    r = check_causation(W["mined"], W["sold"], pb)
    cases.append(("인과: 판매 장부 행의 종류가 MINE → FAIL", not r["ok"] and r["kind_mismatch"], r))
    r = check_causation(W["mined"], [], [p for p in W["processed"] if p["command_type"] == "MINE_RESOURCE"])
    cases.append(("인과: 판매 0 → FAIL(분모)", not r["ok"], r))

    r = check_market_events(W["sold"], W["consts"])
    cases.append(("시장 사건: 합성 월드엔 바닥 없음 → FAIL(조건 미발생)", not r["ok"] and not r["floor_reached_minerals"], r))
    c = W["consts"][("m", "ferrosite")]
    crash = [_sale(c, "A", "ferrosite", 10, 0, 14000, 0, 14000, 0),
             _sale(c, "B", "ferrosite", 1300, 0, 2000, recover(c, 14000, 10, 1300), 2000, 0)]
    r = check_market_events(crash, W["consts"])
    cases.append(("시장 사건: 바닥 + 회복 뒤 판매 → PASS", r["ok"], r))

    r = check_accepted([e["causation_id"] for e in W["sold"]][:3], W["processed"], W["sold"])
    cases.append(("수락: 봇 수락 ⊆ DB → PASS", r["ok"], r))
    r = check_accepted(["lost"], W["processed"], W["sold"])
    cases.append(("수락: DB 에 없는 수락 → FAIL", not r["ok"], r))

    props = schema_payload_props(db.REPO_ROOT)
    cases.append(("§0.10: 쓰는 필드 이름이 전부 계약 스키마 payload 에 있다", set(SOLD_FIELDS) <= props,
                  sorted(set(SOLD_FIELDS) - props)))
    cases.append(("§0.10: 디자인 작업 이름(inventory_before_kg)은 스키마에 없다", "inventory_before_kg" not in props, None))
    fails = 0
    for name, ok, detail in cases:
        print(f"{'OK  ' if ok else 'FAIL'} {name}" + ("" if ok else f" :: {json.dumps(detail, ensure_ascii=False, default=str)[:400]}"))
        fails += 0 if ok else 1
    print(f"selftest: {'PASS' if fails == 0 else f'FAIL ({fails})'}  케이스={len(cases)}")
    return 0 if fails == 0 else 1


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("mode", choices=["shape", "ledger", "chain", "conservation", "saturation", "zero-rows", "accepted",
                                     "load", "market-events", "causation", "selftest"])
    ap.add_argument("--world")
    ap.add_argument("--evidence")
    ap.add_argument("--from", dest="from_file")
    ap.add_argument("--without-sales-term", action="store_true")
    a = ap.parse_args()
    if a.mode == "selftest":
        return selftest()
    if not a.world:
        ap.error("--world <새 월드 id> 가 필요하다 — 모든 SQL 은 world_id 로 한정한다(계약 §0.1)")
    return db.main_guard(lambda: run(a))


if __name__ == "__main__":
    sys.exit(main())
