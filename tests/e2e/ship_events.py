"""p1-01 도메인 이벤트 검사 — **짝짓기 키가 p0-02와 다르다**.

| 하위 명령 | 계약 항목 | 짝짓기 키 |
|----------|---------|---------|
| `pairs`      | SC-14 · SC-76 (AC-3g / AC-20a) | **`ship_id`** |
| `causation`  | SC-08 · SC-09 · SC-12 · SC-81 (AC-3a·b·e / AC-20f) | `causation_id → event_id` 조인 |
| `shutdown`   | SC-13 (AC-3f) | 종료 디스폰을 (a) 잔류 / (b) 활성(같은 tick 원인)으로 분리 |
| `ledger`     | SC-81 (계약 7차) | 인과 결함(null·self·dangling·타입·순서)을 표 전체에서 모아 **동결 장부 7건과 등식** 판정. 구간을 주면 그 구간 결함 0 |
| `overlap`    | I-29 | 같은 actor 함선의 **수명 구간 겹침**(동시 2척). `pairs` 의 actor 목록은 순차·동시를 못 가른다 |
| `resume`     | SC-78 (AC-20c) | 재개 함선: 세션 쌍 2 : 스폰 1 |
| `types`      | SC-80 (AC-20e) | tick 구간 한정 `distinct event_type` |
| `selftest`   | **SC-86** | 합성 데이터로 "키를 correlation 으로 되돌리면 실패한다"를 보인다 |

**`SHIP_*` 를 `correlation_id` 로 짝지으면 조용히 틀린 답이 나온다**(I-41): 한 함선이 여러 세션을
거칠 수 있고, 두 이벤트의 `correlation_id` 는 각각 **그 이벤트를 일으킨 세션**을 가리킨다.

    python tests/e2e/ship_events.py pairs    [--evidence <p>.json]
    python tests/e2e/ship_events.py causation
    python tests/e2e/ship_events.py resume
    python tests/e2e/ship_events.py types --from-tick <n> [--to-tick <n>]
    python tests/e2e/ship_events.py selftest
"""

from __future__ import annotations

import argparse
import sys

import db

SHIP_EVENTS = ("SHIP_SPAWNED", "SHIP_DESPAWNED")
SESSION_EVENTS = ("SESSION_OPENED", "SESSION_CLOSED")
SLICE_EVENT_TYPES = set(SESSION_EVENTS) | set(SHIP_EVENTS)


def ship_id_expr() -> str:
    """두 함선 이벤트에서 `ship_id` 를 꺼내는 식. 스폰·디스폰 모두 payload 에 있다."""
    return "payload->>'ship_id'"


# ─────────────────────────────────────────────────────────────── pairs
def cmd_pairs(args: argparse.Namespace) -> int:
    db.require_tables("domain_events")
    sid = ship_id_expr()
    where = tick_window(args)

    rows = db.psql_rows(
        f"select {sid}, "
        "count(*) filter (where event_type='SHIP_SPAWNED'), "
        "count(*) filter (where event_type='SHIP_DESPAWNED') "
        f"from domain_events where event_type in {SHIP_EVENTS} {where} "
        f"group by 1 order by 1;"
    )
    ships = [(r[0], int(r[1]), int(r[2])) for r in rows]
    broken = [{"ship_id": s, "spawned": a, "despawned": b} for s, a, b in ships if a != 1 or b != 1]
    spawned = sum(a for _, a, _ in ships)
    despawned = sum(b for _, _, b in ships)

    # actor 당 함선은 최대 1척이어야 한다(I-29) — 같은 구간 안에서.
    dup_actor = db.psql_rows(
        f"select actor_id, count(distinct {sid}) from domain_events "
        f"where event_type='SHIP_SPAWNED' {where} group by 1 having count(distinct {sid}) > 1;"
    )

    ok = bool(ships) and not broken
    db.emit(
        {
            "item": "SC-14/SC-76 (AC-3g/AC-20a) ship_id 기준 함선 이벤트 짝짓기",
            "verdict": "PASS" if ok else ("FAIL" if ships else "FAIL(검사할 함선이 없다)"),
            "pairing_key": "ship_id (correlation_id 아님 — I-41)",
            "ships_checked": len(ships),
            "spawned_total": spawned,
            "despawned_total": despawned,
            "unpaired_or_duplicated": broken,
            "actors_with_more_than_one_ship": [
                {"actor_id": r[0], "ships": int(r[1])} for r in dup_actor
            ],
            "tick_window": args_window(args),
        },
        args.evidence,
    )
    return db.EXIT_OK if ok else db.EXIT_FAIL


# ─────────────────────────────────────────────────────────── causation
def cmd_causation(args: argparse.Namespace) -> int:
    db.require_tables("domain_events")
    where = tick_window(args, alias="e")

    # 모든 SHIP_* 의 causation_id 가 비-null 이고 실제 이벤트를 가리키는가 (SC-81)
    total = db.scalar_int(
        f"select count(*) from domain_events e where e.event_type in {SHIP_EVENTS} {where};"
    )
    null_cause = db.scalar_int(
        f"select count(*) from domain_events e where e.event_type in {SHIP_EVENTS} {where} "
        "and e.causation_id is null;"
    )
    dangling = db.psql_rows(
        f"select e.event_type, e.event_id::text, e.causation_id::text from domain_events e "
        f"where e.event_type in {SHIP_EVENTS} {where} and e.causation_id is not null "
        "and not exists (select 1 from domain_events c where c.event_id = e.causation_id) "
        "order by e.tick limit 20;"
    )

    # SHIP_SPAWNED.causation = SESSION_OPENED.event_id 이고 원인이 앞선다 (SC-08·SC-09)
    spawn_join = db.psql_rows(
        "select e.event_id::text, c.event_type, "
        "  (c.tick < e.tick or (c.tick = e.tick and c.sequence < e.sequence))::text, "
        "  c.tick::text, e.tick::text "
        f"from domain_events e join domain_events c on c.event_id = e.causation_id "
        f"where e.event_type='SHIP_SPAWNED' {where} order by e.tick limit 200;"
    )
    spawn_wrong_type = [r for r in spawn_join if r[1] != "SESSION_OPENED"]
    spawn_wrong_order = [r for r in spawn_join if r[2] != "true"]

    # SHIP_DESPAWNED.causation = SESSION_CLOSED.event_id, 여러 tick 전일 수 있다 (SC-12)
    despawn_join = db.psql_rows(
        "select e.event_id::text, c.event_type, e.payload->>'despawn_reason', "
        "  (e.tick - c.tick)::text "
        f"from domain_events e join domain_events c on c.event_id = e.causation_id "
        f"where e.event_type='SHIP_DESPAWNED' {where} order by e.tick limit 200;"
    )
    despawn_wrong_type = [r for r in despawn_join if r[1] != "SESSION_CLOSED"]
    lag = [int(r[3]) for r in despawn_join if r[3] is not None]

    ok = (
        total > 0
        and null_cause == 0
        and not dangling
        and bool(spawn_join)
        and not spawn_wrong_type
        and not spawn_wrong_order
        and not despawn_wrong_type
    )
    db.emit(
        {
            "item": "SC-08/09/12/81 (AC-3a·b·e / AC-20f) 인과 조인",
            "verdict": "PASS" if ok else "FAIL",
            "ship_events_checked": total,
            "causation_null": null_cause,
            "causation_dangling": [
                {"event_type": r[0], "event_id": r[1], "causation_id": r[2]} for r in dangling
            ],
            "spawn_joins_checked": len(spawn_join),
            "spawn_cause_wrong_type": len(spawn_wrong_type),
            "spawn_cause_not_earlier": len(spawn_wrong_order),
            "despawn_joins_checked": len(despawn_join),
            "despawn_cause_wrong_type": len(despawn_wrong_type),
            "despawn_cause_tick_lag": {
                "min": min(lag) if lag else None,
                "max": max(lag) if lag else None,
                "note": "상관(같은 세션)과 인과가 서로 다른 시각을 가리키는 첫 데이터 — lag>0 이 그 증거다",
            },
            "despawn_reasons": sorted({r[2] for r in despawn_join if r[2]}),
            "tick_window": args_window(args),
        },
        args.evidence,
    )
    return db.EXIT_OK if ok else db.EXIT_FAIL


# ────────────────────────────────────────────────────────────── ledger
# SC-81 (계약 7차): 표 전체의 인과 결함 집합이 **동결된 결함 장부**(스펙 §11-8)와 같아야 한다.
# 장부 행은 지우지도 고치지도 않는다(원칙 5). 자기 루프는 순환이 아니라 "원인 미상(결함 기록)".
FROZEN_DEFECT_LEDGER = {
    "01a0c46d-412c-75c4-a72e-5eb9b6eda64b",  # tick 395902 seq 8  (qa R3 RED 인스턴스 종료 스윕)
    "01a0c46d-412c-75c4-a72e-5ebab4dbe3a7",  # tick 395902 seq 9
    "01a0c46d-412c-75c4-a72e-5ebbfc8128c6",  # tick 395902 seq 10
    "01a0c46d-412c-75c4-a72e-5ebc0f2f7da9",  # tick 395902 seq 11
    "01a0c46d-412c-75c4-a72e-5ebd1f652604",  # tick 395902 seq 12
    "01a0c46d-412c-75c4-a72e-5ebe4e8056ac",  # tick 395902 seq 13
    "01a0c48c-6427-744b-9d9a-25dcd98df09f",  # tick 430030 seq 0   (qa R3 §6.7 재현, actor …0044)
}


def session_causation_defects(where: str, cte: str = "") -> list[dict]:
    """세션 이벤트의 인과 (계약 7차, 스펙 I-30·AC-3(h)).
    - `SESSION_OPENED` 의 원인은 null 이다.
    - `SESSION_CLOSED{SUPERSEDED}` 의 원인은 **새 세션의 `SESSION_OPENED`** — 같은 actor, 다른 correlation,
      **같은 tick, 더 작은 sequence**, != self.
    - 그 밖의 사유의 `SESSION_CLOSED` 원인은 null 이다(전송 계층의 사실이라 원인 이벤트가 없다)."""
    rows = db.psql_rows(
        cte + "select e.event_id::text, e.event_type, e.tick::text, e.sequence::text, "
        "  coalesce(e.payload->>'close_reason',''), coalesce(c.event_type, ''), "
        "  case "
        "    when e.event_type = 'SESSION_OPENED' and e.causation_id is not null then 'opened_cause_not_null' "
        "    when e.event_type = 'SESSION_CLOSED' and coalesce(e.payload->>'close_reason','') <> 'SUPERSEDED' "
        "         and e.causation_id is not null then 'closed_cause_not_null' "
        "    when e.event_type = 'SESSION_CLOSED' and e.payload->>'close_reason' = 'SUPERSEDED' then "
        "      case "
        "        when e.causation_id is null then 'superseded_cause_null' "
        "        when e.causation_id = e.event_id then 'self' "
        "        when c.event_id is null then 'dangling' "
        "        when c.event_type <> 'SESSION_OPENED' then 'wrong_type' "
        "        when c.actor_id is distinct from e.actor_id then 'actor_mismatch' "
        "        when c.correlation_id = e.correlation_id then 'same_session' "
        "        when not (c.tick = e.tick and c.sequence < e.sequence) then 'not_same_tick_earlier' "
        "        else '' end "
        "    else '' end "
        "from domain_events e left join domain_events c on c.event_id = e.causation_id "
        f"where e.event_type in {SESSION_EVENTS} {where};"
    )
    return [{"event_id": r[0], "event_type": r[1], "tick": int(r[2]), "sequence": int(r[3]),
             "close_reason": r[4], "cause_type": r[5], "defect": r[6]} for r in rows if r[6]]


def causation_defects(where: str, cte: str = "") -> list[dict]:
    """`SHIP_*` 의 인과 결함 — **존재만 보지 않는다**(존재만 보는 조인은 자기 참조를 통과시킨다, 실측).
    결함 = null · 자기 참조 · 원인 없음 · 원인 타입 틀림 · 원인이 먼저가 아님."""
    rows = db.psql_rows(
        cte + "select e.event_id::text, e.event_type, e.tick::text, e.sequence::text, "
        "  coalesce(c.event_type, ''), "
        "  case "
        "    when e.causation_id is null then 'null' "
        "    when e.causation_id = e.event_id then 'self' "
        "    when c.event_id is null then 'dangling' "
        "    when (e.event_type = 'SHIP_SPAWNED' and c.event_type <> 'SESSION_OPENED') "
        "      or (e.event_type = 'SHIP_DESPAWNED' and c.event_type <> 'SESSION_CLOSED') then 'wrong_type' "
        "    when not (c.tick < e.tick or (c.tick = e.tick and c.sequence < e.sequence)) then 'not_earlier' "
        "    else '' end "
        "from domain_events e left join domain_events c on c.event_id = e.causation_id "
        f"where e.event_type in {SHIP_EVENTS} {where};"
    )
    return [{"event_id": r[0], "event_type": r[1], "tick": int(r[2]), "sequence": int(r[3]),
             "cause_type": r[4], "defect": r[5]} for r in rows if r[5]]


def cmd_ledger(args: argparse.Namespace) -> int:
    """SC-81 — (b) 표 전체 결함 집합 == 장부, (c) 지정 구간 결함 0. 구간을 주면 (c) 만 본다."""
    db.require_tables("domain_events")
    where = tick_window(args, alias="e")
    checked = db.scalar_int(f"select count(*) from domain_events e where e.event_type in {SHIP_EVENTS} {where};")
    checked_sessions = db.scalar_int(
        f"select count(*) from domain_events e where e.event_type in {SESSION_EVENTS} {where};")
    superseded = db.scalar_int(
        "select count(*) from domain_events e where e.event_type = 'SESSION_CLOSED' "
        f"and e.payload->>'close_reason' = 'SUPERSEDED' {where};")
    defects = causation_defects(where) + session_causation_defects(where)
    found = {d["event_id"] for d in defects}
    windowed = getattr(args, "from_tick", None) is not None or getattr(args, "to_tick", None) is not None
    if windowed:
        ok = checked > 0 and not defects
        rule = "(c) 구간 결함 0"
    else:
        ok = checked > 0 and found == FROZEN_DEFECT_LEDGER
        rule = "(b) 표 전체 결함 집합 == 동결 장부 7건"
    db.emit(
        {
            "item": "SC-81 (AC-20f, I-30) 인과 — 타입 + ≠ self + 순서, 동결된 결함 장부",
            "verdict": "PASS" if ok else "FAIL",
            "rule": rule,
            "ship_events_checked": checked,
            "session_events_checked": checked_sessions,
            "superseded_closes_checked": superseded,
            "defects_found": len(defects),
            "defects_by_kind": {k: sum(1 for d in defects if d["defect"] == k)
                                for k in sorted({d["defect"] for d in defects})},
            "new_defects_not_in_ledger": sorted(found - FROZEN_DEFECT_LEDGER),
            "ledger_rows_missing": [] if windowed else sorted(FROZEN_DEFECT_LEDGER - found),
            "defects": defects[:50],
            "tick_window": args_window(args),
        },
        args.evidence,
    )
    return db.EXIT_OK if ok else db.EXIT_FAIL


def cmd_causation_selftest(args: argparse.Namespace) -> int:
    """SC-81 인과 검사 자체의 검증 — **DB 에 쓰지 않고** 합성 행을 CTE 로 `domain_events` 에 덮어
    같은 SQL 을 돌린다. 올바른 넘겨받기(SUPERSEDED)는 결함 0, 틀린 모양은 전부 정해진 결함 종류로 잡혀야 한다.
    계약 7차 뒤 실데이터에 `SUPERSEDED` 가 아직 0건일 때도 이 경로가 **무언가를 가르는지**를 보인다(§7a)."""
    A, B = "00000000-0000-7000-8000-00000000000a", "00000000-0000-7000-8000-00000000000b"
    S1, S2 = "00000000-0000-7000-8000-0000000000c1", "00000000-0000-7000-8000-0000000000c2"

    def ev(n: int) -> str:
        return f"00000000-0000-7000-8000-{n:012d}"

    # (event_id, type, tick, seq, close_reason, causation, correlation, actor, 기대 결함)
    rows = [
        (ev(1), "SESSION_OPENED", 10, 0, None, None, S1, A, ""),
        (ev(2), "SESSION_OPENED", 20, 0, None, None, S2, A, ""),
        (ev(3), "SESSION_CLOSED", 20, 1, "SUPERSEDED", ev(2), S1, A, ""),           # 올바른 넘겨받기
        (ev(4), "SESSION_CLOSED", 30, 0, "CLIENT_CLOSED", None, S2, A, ""),         # 올바른 일반 닫힘
        (ev(5), "SESSION_CLOSED", 40, 1, "SUPERSEDED", None, S1, A, "superseded_cause_null"),
        (ev(6), "SESSION_CLOSED", 40, 2, "SUPERSEDED", ev(6), S1, A, "self"),
        (ev(7), "SESSION_CLOSED", 40, 3, "SUPERSEDED", ev(4), S1, A, "wrong_type"),
        (ev(8), "SESSION_OPENED", 50, 5, None, None, S2, B, ""),
        (ev(9), "SESSION_CLOSED", 50, 6, "SUPERSEDED", ev(8), S1, A, "actor_mismatch"),
        (ev(10), "SESSION_OPENED", 60, 0, None, None, S1, A, ""),
        (ev(11), "SESSION_CLOSED", 60, 1, "SUPERSEDED", ev(10), S1, A, "same_session"),
        (ev(12), "SESSION_OPENED", 70, 3, None, None, S2, A, ""),
        (ev(13), "SESSION_CLOSED", 70, 2, "SUPERSEDED", ev(12), S1, A, "not_same_tick_earlier"),
        (ev(14), "SESSION_CLOSED", 80, 0, "IDLE_TIMEOUT", ev(1), S1, A, "closed_cause_not_null"),
        (ev(15), "SESSION_OPENED", 90, 0, None, ev(4), S2, A, "opened_cause_not_null"),
        (ev(16), "SHIP_DESPAWNED", 95, 0, None, ev(16), S1, A, "self"),
        (ev(17), "SHIP_SPAWNED", 96, 0, None, ev(4), S1, A, "wrong_type"),
    ]

    def lit(v):
        return "null" if v is None else f"'{v}'"

    values = ", ".join(
        f"({lit(e)}::uuid, {lit(t)}, {tk}::bigint, {sq}::int, "
        f"{lit('{}' if cr is None else '{\"close_reason\": \"' + cr + '\"}')}::jsonb, "
        f"{lit(ca)}::uuid, {lit(co)}::uuid, {lit(ac)}::uuid)"
        for e, t, tk, sq, cr, ca, co, ac, _ in rows
    )
    cte = ("with domain_events(event_id, event_type, tick, sequence, payload, causation_id, correlation_id, actor_id) "
           f"as (values {values}) ")
    found = {d["event_id"]: d["defect"] for d in causation_defects("", cte) + session_causation_defects("", cte)}
    expected = {e: want for e, *_, want in rows if want}
    mismatches = [{"event_id": e, "expected": expected.get(e, ""), "found": found.get(e, "")}
                  for e in sorted(set(found) | set(expected)) if found.get(e, "") != expected.get(e, "")]
    ok = not mismatches and len(expected) == 10
    db.emit(
        {
            "item": "SC-81 인과 검사 자체 검증(합성, DB 쓰기 없음)",
            "verdict": "PASS" if ok else "FAIL",
            "synthetic_rows": len(rows),
            "expected_defects": len(expected),
            "found_defects": len(found),
            "mismatches": mismatches,
            "cases": {e: want or "(정상)" for e, *_, want in rows},
        },
        args.evidence,
    )
    return db.EXIT_OK if ok else db.EXIT_FAIL


# ────────────────────────────────────────────────────────────── overlap
def cmd_overlap(args: argparse.Namespace) -> int:
    """I-29 — 한 `actor_id` 는 **동시에** 최대 1척. `pairs` 의 `actors_with_more_than_one_ship` 은
    순차(만료 뒤 새 스폰 — 정상)와 동시(위반)를 구분하지 못해 **라운드 3 에서 동시 위반을 정보로 흘려보냈다.**
    여기서는 함선 수명 구간 [스폰 tick, 디스폰 tick) 이 겹치는 같은 actor 의 함선 쌍만 센다.
    디스폰이 없는 함선은 구간 끝을 열어 둔다(아직 살아 있다).
    """
    db.require_tables("domain_events")
    sid = ship_id_expr()
    where = tick_window(args)
    rows = db.psql_rows(
        "with life as ("
        f"  select actor_id::text a, {sid} s, "
        "    min(tick) filter (where event_type='SHIP_SPAWNED') t0, "
        "    max(tick) filter (where event_type='SHIP_DESPAWNED') t1 "
        f"  from domain_events where event_type in {SHIP_EVENTS} {where} group by 1, 2) "
        "select x.a, x.s, x.t0::text, coalesce(x.t1::text,''), y.s, y.t0::text, coalesce(y.t1::text,'') "
        "from life x join life y on x.a = y.a and x.s < y.s "
        "where x.t0 is not null and y.t0 is not null "
        "  and x.t0 < coalesce(y.t1, 9223372036854775807) and y.t0 < coalesce(x.t1, 9223372036854775807) "
        "order by x.a, x.t0;"
    )
    overlaps = [{"actor_id": r[0], "ship_a": r[1], "a_spawn": int(r[2]), "a_despawn": int(r[3]) if r[3] else None,
                 "ship_b": r[4], "b_spawn": int(r[5]), "b_despawn": int(r[6]) if r[6] else None} for r in rows]
    ships = db.scalar_int(
        f"select count(distinct {sid}) from domain_events where event_type='SHIP_SPAWNED' {where};"
    )
    ok = ships > 0 and not overlaps
    db.emit(
        {
            "item": "I-29 actor 당 동시 1척 (수명 구간 겹침)",
            "verdict": "PASS" if ok else ("FAIL" if ships else "FAIL(검사할 함선이 없다)"),
            "ships_checked": ships,
            "concurrent_overlaps": overlaps,
            "tick_window": args_window(args),
        },
        args.evidence,
    )
    return db.EXIT_OK if ok else db.EXIT_FAIL


# ────────────────────────────────────────────────────────────── shutdown
def cmd_shutdown(args: argparse.Namespace) -> int:
    """SC-13 (AC-3f, I-41): 정상 종료 스윕의 `SHIP_DESPAWNED{SERVER_SHUTDOWN}` 을 **(a) 잔류 / (b) 활성**으로 나눈다.

    (a) 잔류 함선의 원인은 **여러 tick 전**의 `SESSION_CLOSED` 다(잔류를 시작시킨 것).
    (b) 활성 함선의 원인은 **같은 tick** 에 스윕이 먼저 발행한 `SESSION_CLOSED` 이고 `sequence` 가 작다
        (ADR-0011 §6 — 스윕을 함선부터 돌면 원인이 아직 없다).
    **둘 다 1건 이상이어야 판정한다**(계약 §7a — 한쪽이 0이면 그 경로는 타지 않은 것이다).
    """
    db.require_tables("domain_events")
    where = tick_window(args, alias="d")
    rows = db.psql_rows(
        "select d.payload->>'ship_id', d.tick::text, d.sequence::text, c.event_type, c.tick::text, "
        "  c.sequence::text, (c.tick < d.tick or (c.tick = d.tick and c.sequence < d.sequence))::text "
        "from domain_events d left join domain_events c on c.event_id = d.causation_id "
        f"where d.event_type='SHIP_DESPAWNED' and d.payload->>'despawn_reason'='SERVER_SHUTDOWN' {where} "
        "order by d.tick, d.sequence;"
    )
    a_rows, b_rows, bad = [], [], []
    for sid, dt, ds, ctype, ct, cs, earlier in rows:
        rec = {"ship_id": sid, "tick": int(dt), "sequence": int(ds), "cause_type": ctype,
               "cause_tick": int(ct) if ct else None, "cause_sequence": int(cs) if cs else None,
               "cause_earlier": earlier == "true"}
        if ctype != "SESSION_CLOSED" or earlier != "true":
            bad.append(rec)
        (b_rows if rec["cause_tick"] == rec["tick"] else a_rows).append(rec)
    ok = bool(a_rows) and bool(b_rows) and not bad
    db.emit(
        {
            "item": "SC-13 (AC-3f) 정상 종료 디스폰 — (a) 잔류 / (b) 활성",
            "verdict": "PASS" if ok else "FAIL",
            "server_shutdown_despawns": len(rows),
            "a_lingering": len(a_rows),
            "b_active_same_tick": len(b_rows),
            "cause_wrong_or_not_earlier": bad,
            "rows": a_rows + b_rows,
            "note": "(a)(b) 중 하나라도 0 이면 그 경로는 이 실행에서 타지 않았다 — PASS 아님",
            "tick_window": args_window(args),
        },
        args.evidence,
    )
    return db.EXIT_OK if ok else db.EXIT_FAIL


# ────────────────────────────────────────────────────────────── resume
def cmd_resume(args: argparse.Namespace) -> int:
    """재개된 함선: **세션 쌍은 2개 이상인데 스폰은 1건**(I-29·I-41)."""
    db.require_tables("domain_events")
    sid = ship_id_expr()
    where = tick_window(args)

    # 함선별 스폰 수와, 그 함선 actor 의 세션 수
    rows = db.psql_rows(
        f"select {sid} as ship, max(actor_id::text), "
        "count(*) filter (where event_type='SHIP_SPAWNED') "
        f"from domain_events where event_type in {SHIP_EVENTS} {where} group by 1 order by 1;"
    )
    resumed = []
    for ship, actor, spawns in rows:
        sessions = db.scalar_int(
            "select count(*) from domain_events "
            f"where event_type='SESSION_OPENED' and actor_id = '{actor}'{tick_window(args)};"
        )
        if sessions >= 2:
            resumed.append(
                {"ship_id": ship, "actor_id": actor, "sessions": sessions, "spawns": int(spawns)}
            )

    violations = [r for r in resumed if r["spawns"] != 1]
    ok = bool(resumed) and not violations
    db.emit(
        {
            "item": "SC-78 (AC-20c) 재개 함선: 세션 2 : 스폰 1",
            "verdict": "PASS" if ok else ("FAIL" if resumed else "미검증 — 재개 사례가 없다"),
            "ships_checked": len(rows),
            "resumed_ships": resumed,
            "violations": violations,
            "meaning": "세션이 2개 이상인데 스폰이 1건이면 같은 함선을 이어 탄 것이다(I-29).",
            "tick_window": args_window(args),
        },
        args.evidence,
    )
    if not resumed:
        return db.EXIT_FAIL
    return db.EXIT_OK if ok else db.EXIT_FAIL


# ─────────────────────────────────────────────────────────────── types
def cmd_types(args: argparse.Namespace) -> int:
    """SC-80: 이동은 도메인 이벤트를 만들지 않는다(I-31). **위치 시계열이 없다**."""
    db.require_tables("domain_events")
    where = tick_window(args)
    rows = db.psql_rows(
        f"select event_type, count(*) from domain_events where true {where} group by 1 order by 1;"
    )
    found = {r[0]: int(r[1]) for r in rows}
    extra = sorted(set(found) - SLICE_EVENT_TYPES)
    # 위치·상태 시계열을 뜻하는 이름이 하나라도 있는가
    suspicious = [t for t in found if any(k in t.upper() for k in ("POSITION", "MOVE", "STATE", "SNAPSHOT", "TICK"))]
    ok = bool(found) and not suspicious and (args.allow_extra or not extra)
    db.emit(
        {
            "item": "SC-80 (AC-20e) domain_events 에 위치 시계열이 없다",
            "verdict": "PASS" if ok else "FAIL",
            "tick_window": args_window(args),
            "window_note": (
                "tick 구간을 주지 않으면 전수다. 이 PC 에는 p0-02 가 정당하게 남긴 "
                "QA_APPEND_ONLY_PROBE 행이 있을 수 있으므로 `--from-tick` 또는 "
                "`docker compose down -v` 게이트 중 무엇을 썼는지 리포트에 적는다(계약 §0.2)."
            ),
            "event_types": found,
            "expected_types": sorted(SLICE_EVENT_TYPES),
            "unexpected_types": extra,
            "position_timeseries_like": suspicious,
        },
        args.evidence,
    )
    return db.EXIT_OK if ok else db.EXIT_FAIL


# ───────────────────────────────────────────────────────────── selftest
def cmd_selftest(args: argparse.Namespace) -> int:
    """**SC-86**: 짝짓기 키를 `correlation_id` 로 되돌리면 재개 시나리오에서 실패한다.

    DB 없이 합성 데이터로 돌린다 — 이 검사가 무언가를 막는다는 것을 스스로 증명한다.
    """
    # 한 함선이 두 세션을 거친 경우: 스폰은 세션1의 correlation, 디스폰은 세션2의 correlation.
    events = [
        {"type": "SHIP_SPAWNED", "ship_id": "ship-1", "correlation_id": "corr-A"},
        {"type": "SHIP_DESPAWNED", "ship_id": "ship-1", "correlation_id": "corr-B"},
    ]

    def pair_by(key: str):
        groups: dict[str, dict[str, int]] = {}
        for e in events:
            g = groups.setdefault(e[key], {"SHIP_SPAWNED": 0, "SHIP_DESPAWNED": 0})
            g[e["type"]] += 1
        return {k: v for k, v in groups.items() if v["SHIP_SPAWNED"] != 1 or v["SHIP_DESPAWNED"] != 1}

    by_ship = pair_by("ship_id")
    by_corr = pair_by("correlation_id")

    ok = (not by_ship) and bool(by_corr)
    db.emit(
        {
            "item": "SC-86 짝짓기 키 자체 검증 (§0.6)",
            "verdict": "PASS" if ok else "FAIL",
            "scenario": "함선 1척이 세션 2개를 거친다(재개). 스폰과 디스폰의 correlation 이 다르다",
            "paired_by_ship_id": {"broken_groups": by_ship, "result": "짝이 맞는다" if not by_ship else "깨진다"},
            "paired_by_correlation_id": {
                "broken_groups": by_corr,
                "result": "깨진다(기대)" if by_corr else "깨지지 않았다 — 검사가 무의미하다",
            },
            "meaning": (
                "p0-02 의 correlation 기반 SQL 을 그대로 쓰면 재개 함선에서 '짝 없는 이벤트 2건'이 "
                "나온다. 이 스크립트가 ship_id 로 짝짓는 이유이고, 그 선택이 실제로 무언가를 막는다."
            ),
        },
        args.evidence,
    )
    return db.EXIT_OK if ok else db.EXIT_FAIL


# ─────────────────────────────────────────────────────────────── 공통
def tick_window(args: argparse.Namespace, alias: str = "") -> str:
    a = f"{alias}." if alias else ""
    parts = []
    if getattr(args, "from_tick", None) is not None:
        parts.append(f" and {a}tick >= {int(args.from_tick)}")
    if getattr(args, "to_tick", None) is not None:
        parts.append(f" and {a}tick <= {int(args.to_tick)}")
    return "".join(parts)


def args_window(args: argparse.Namespace) -> dict:
    return {
        "from_tick": getattr(args, "from_tick", None),
        "to_tick": getattr(args, "to_tick", None),
        "mode": "tick 구간 한정" if getattr(args, "from_tick", None) is not None else "전수",
    }


def main() -> int:
    ap = argparse.ArgumentParser(description="p1-01 도메인 이벤트 검사")
    sub = ap.add_subparsers(dest="cmd", required=True)
    for name, fn in (
        ("pairs", cmd_pairs),
        ("causation", cmd_causation),
        ("shutdown", cmd_shutdown),
        ("overlap", cmd_overlap),
        ("ledger", cmd_ledger),
        ("causation-selftest", cmd_causation_selftest),
        ("resume", cmd_resume),
        ("types", cmd_types),
        ("selftest", cmd_selftest),
    ):
        p = sub.add_parser(name)
        p.add_argument("--evidence")
        if name not in ("selftest", "causation-selftest"):
            p.add_argument("--from-tick", type=int, dest="from_tick")
            p.add_argument("--to-tick", type=int, dest="to_tick")
        if name == "types":
            p.add_argument("--allow-extra", action="store_true",
                           help="이 슬라이스 밖 타입(p0-02 탐침 등)을 위반으로 보지 않는다")
        p.set_defaults(fn=fn)
    args = ap.parse_args()
    return db.main_guard(lambda: args.fn(args))


if __name__ == "__main__":
    sys.exit(main())
