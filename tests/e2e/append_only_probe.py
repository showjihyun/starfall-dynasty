"""SC-11 / SC-12 / SC-13 (AC-4) — append-only 트리거와 UNIQUE 제약을 실제로 때려 본다.

**탐침 규칙 (계약 §B — 어기면 SC-59 를 QA 가 스스로 깨뜨린다):**

1. 서버가 **떠 있지 않을 때** 실행한다. 도는 중에 넣으면 서버가 같은 tick 을 쓰려다
   가짜 `UNIQUE` 위반을 일으킨다.
2. 탐침 `tick` = `max(tick) + 1` (이 실행이 쓰지 않는 전용 값), `sequence` 는 **0부터 연속**.
3. 실행 후 서버를 다시 기동하면 `start_tick = probe_tick + 1` 이 된다 — **정상이다**
   (ADR-0006 §2.3). 리포트에 적지 않으면 다음 사람이 버그로 본다.
4. 그래서 이 블록은 **모든 부하·왕복 측정이 끝난 뒤**에 돌린다.

    python tests/e2e/append_only_probe.py --evidence <path>
"""

from __future__ import annotations

import argparse
import sys
import uuid

import db

WORLD_ID = "01a0b1c2-3d4e-7f01-8a2b-9c0d1e2f3a4b"  # 스파이크 월드 (ADR-0007 §2 시드)


def insert_sql(event_id: str, tick: int, sequence: int) -> str:
    return (
        "insert into domain_events (event_id, event_type, schema_version, world_id, tick, "
        "sequence, occurred_at, recorded_at, correlation_id, causation_id, actor_id, payload) "
        f"values ('{event_id}', 'QA_APPEND_ONLY_PROBE', 1, '{WORLD_ID}', {tick}, {sequence}, "
        "'3800-01-01T00:00:00Z', now(), "
        f"'{uuid.uuid4()}', null, null, '{{}}'::jsonb) "
        "on conflict (event_id) do nothing;"
    )


def run(args: argparse.Namespace) -> int:
    db.require_tables("worlds", "domain_events")

    server_up = False
    try:
        import urllib.request

        with urllib.request.urlopen("http://127.0.0.1:8080/healthz", timeout=1):  # noqa: S310
            server_up = True
    except Exception:  # noqa: BLE001 — 안 떠 있는 것이 정상이다
        server_up = False
    if server_up and not args.force:
        raise db.EnvironmentProblem(
            "서버가 떠 있다. 탐침은 서버 정지 상태에서 넣어야 한다(계약 §B 규칙 1). "
            "정말 진행하려면 --force."
        )

    before = db.scalar_int("select count(*) from domain_events;")
    probe_tick = db.scalar_int("select coalesce(max(tick), -1) + 1 from domain_events;")
    event_id = str(uuid.uuid4())

    steps = []

    # 탐침 행 1건 (sequence 0 — count=1, max(seq)+1=1, min=0 이므로 SC-59 를 통과한다)
    out = db.psql(insert_sql(event_id, probe_tick, 0))
    after_insert = db.scalar_int("select count(*) from domain_events;")
    steps.append({"step": "탐침 행 삽입", "tick": probe_tick, "sequence": 0,
                  "rows": after_insert, "output": out})
    if after_insert != before + 1:
        db.emit({"item": "SC-90 (AC-20) append-only 탐침", "SC-90_verdict": "FAIL",
                 "reason": "탐침 행을 넣지 못했다", "steps": steps}, args.evidence)
        return db.EXIT_FAIL

    # SC-11 (a): UPDATE 가 막히는가
    upd = db.psql("update domain_events set event_type='X' where event_type='QA_APPEND_ONLY_PROBE';")
    rows_after_update = db.scalar_int("select count(*) from domain_events;")
    update_blocked = "append-only" in upd.lower()
    steps.append({"step": "UPDATE 시도", "blocked": update_blocked,
                  "rows": rows_after_update, "output": upd})

    # SC-11 (b): DELETE 가 막히는가
    dele = db.psql("delete from domain_events where event_type='QA_APPEND_ONLY_PROBE';")
    rows_after_delete = db.scalar_int("select count(*) from domain_events;")
    delete_blocked = "append-only" in dele.lower()
    steps.append({"step": "DELETE 시도", "blocked": delete_blocked,
                  "rows": rows_after_delete, "output": dele})

    # SC-12: 같은 event_id 재삽입 → 행 수 증가 0
    again = db.psql(insert_sql(event_id, probe_tick, 0))
    rows_after_reinsert = db.scalar_int("select count(*) from domain_events;")
    reinsert_noop = rows_after_reinsert == rows_after_delete
    steps.append({"step": "같은 event_id 재삽입", "no_new_row": reinsert_noop,
                  "rows": rows_after_reinsert, "output": again})

    # (e): 다른 event_id + 같은 (world_id, tick, sequence) → UNIQUE 위반
    #
    # **E-1 (architect R19)**: 옛 판정은 `"duplicate key" in dup or "unique" in dup` 였다 —
    # **어느 제약이든 위반되기만 하면 참이다.** (d) 와 (e) 의 입력 차이는 `event_id` 뿐이라,
    # (e) 가 `event_id` **PK**(`domain_events_pkey`)에 걸려도 같은 문자열이 나온다. 그러면
    # 두 절이 **같은 제약을 두 번 재고** (e) 가 이름붙인 성질(= `(world_id,tick,sequence)`
    # 유일성)은 한 번도 검사되지 않는다. §7b(1) 을 (e) 에 돌린 결과다.
    # → **제약의 이름**을 본다. 이름은 DB 에서 읽는다(하드코딩하면 마이그레이션과 갈린다).
    uniq_name = db.psql(
        "select conname from pg_constraint "
        "where conrelid = 'domain_events'::regclass and contype = 'u' "
        "and pg_get_constraintdef(oid) like '%(world_id, tick, sequence)%';"
    ).strip()
    pk_name = db.psql(
        "select conname from pg_constraint "
        "where conrelid = 'domain_events'::regclass and contype = 'p';"
    ).strip()
    dup = db.psql(insert_sql(str(uuid.uuid4()), probe_tick, 0))
    rows_after_dup = db.scalar_int("select count(*) from domain_events;")
    unique_violated = bool(uniq_name) and uniq_name in dup
    hit_pk_instead = bool(pk_name) and pk_name in dup
    steps.append({"step": "다른 event_id + 같은 (world_id,tick,sequence)",
                  "unique_violation": unique_violated,
                  "expected_constraint": uniq_name,
                  "pk_constraint": pk_name,
                  "hit_pk_instead": hit_pk_instead,
                  "why_name_not_substring": (
                      "문자열 `unique`/`duplicate key` 로 보면 event_id PK 위반도 참이 된다 — "
                      "그러면 (d) 와 (e) 가 같은 제약을 두 번 재고 (e) 는 아무것도 검사하지 "
                      "않는다(architect R19 E-1)"
                  ),
                  "rows": rows_after_dup, "output": dup})

    ok = (
        update_blocked
        and delete_blocked
        and rows_after_update == before + 1
        and rows_after_delete == before + 1
        and reinsert_noop
        and unique_violated
        and not hit_pk_instead        # (e) 가 (d) 와 같은 제약을 잰 것이 아님을 단언한다
        and rows_after_dup == before + 1
    )
    db.emit(
        {
            # **D-2 (architect R18)**: 이 라벨은 **p0-02 의 번호였다.** p1-01 계약에서 SC-11·12·13
            # 은 전혀 다른 것을 가리키고(잔류 창 재접속 · LINGER_EXPIRED 디스폰 · SERVER_SHUTDOWN
            # 디스폰), AC-4 도 이 슬라이스에선 적분·경계다. **라벨로 집계하면 하드 게이트 셋이
            # 치르지 않은 PASS 를 받는다.** qa 가 리포트 12건을 훑어 **수확된 곳이 없음**을
            # 확인했으므로 정정할 과거 PASS 는 없다 — 라벨만 고친다(12_qa_report_r12.md §4 발견 6).
            "item": "SC-90 (AC-20) append-only · 멱등 삽입 · UNIQUE",
            "SC-90_verdict": "PASS" if ok else "FAIL",
            "label_history": (
                "17차까지 이 라벨은 `SC-11/12/13 (AC-4)` 였다 — **p0-02 의 번호다.** "
                "18차에 계약 SC-90 을 신설해 집을 줬다(계약 §9 18차)."
            ),
            "rows_before": before,
            "probe_tick": probe_tick,
            "probe_sequence": 0,
            "probe_event_id": event_id,
            "steps": steps,
            "note": (
                f"다음 서버 기동은 start_tick = {probe_tick + 1} 에서 시작한다 "
                "(ADR-0006 §2.3 tick 재개). 정상이며 tick 은 뒤로 가지 않는다."
            ),
        },
        args.evidence,
    )
    return db.EXIT_OK if ok else db.EXIT_FAIL


def main() -> int:
    ap = argparse.ArgumentParser(description="append-only / UNIQUE 탐침 (서버 정지 상태에서)")
    ap.add_argument("--force", action="store_true", help="서버가 떠 있어도 진행한다(권장하지 않음)")
    ap.add_argument("--evidence")
    args = ap.parse_args()
    return db.main_guard(lambda: run(args))


if __name__ == "__main__":
    sys.exit(main())
