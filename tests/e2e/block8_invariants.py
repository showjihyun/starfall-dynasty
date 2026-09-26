"""블록 8 의 다섯 항목 — **SC-10 · SC-28 · SC-30 · SC-32 · SC-74** 의 판정 도구.

이 다섯은 p1-01 에서 **한 번도 판정된 적이 없다**(architect R18 §2667). 출처 게이트가
`미지명 5건` 으로 빨간불을 켜 온 바로 그 다섯이고, 만기는 *"블록 8 절차서가 명령을 확정하는
시점"* 이다(계약 §570). 이 파일이 그 명령이다.

절차서: `_workspace/p1-01-ship-movement/14_block8_procedure.md`

## 왜 한 파일인가

입력이 셋으로 갈리고 **항목이 그 셋을 따라 갈린다.** 한 입력을 읽는 항목끼리 묶었다:

| 하위 명령 | 항목 | 입력 |
|---|---|---|
| `snapshots --bot-out-dir <DIR>` | **SC-28 · SC-30 · SC-32** | 봇 `--out` 디렉터리 (`snapshots.csv` + `summary.json`) |
| `spawn` | **SC-10** | PostgreSQL `domain_events` + `data/world/systems/*.json` |
| `db-outage --bot-out-dir <DIR> --stats-before <F> --stats-after <F>` | **SC-74** | 봇 산출물 + 중단 창 전후의 `/debug/stats` |
| `selftest` | — | 합성 입력 (양성·음성 대조) |

## 판정하지 않고 "미검증" 을 내는 자리 (계약 §0.3)

**겨냥한 조건이 실제로 발생하지 않았으면 PASS 가 아니다.** 이 파일에서 그 자리는 넷이다:

- **SC-28**: 검사한 간격이 0 이면 `미검증(표본 없음)`. 세션이 스냅샷을 한 건만 받았다는 뜻이다.
- **SC-32**: 이 실행에서 **디스폰이 한 건도 없었으면** `미검증(표본 없음)`. "재등장 0" 은 아무도
  사라지지 않았으면 **항진명제**다. `LINGERING` 을 한 번도 못 봤을 때도 같다.
- **SC-10**: 같은 `actor_id` 의 **두 번째 스폰이 없거나** 두 스폰의 tick 차이가
  `linger_seconds` 이하면 `미검증(표본 없음)`. 30 초 안의 재접속은 **재개이지 스폰이 아니다**
  (계약 게이트 G-j — 그렇게 재면 항진명제다).
- **SC-74**: `/debug/stats` 두 장으로 **DB 가 실제로 멈췄다는 것을 단언하지 못하면**
  `미검증(증거 요건)`. postgres 가 멀쩡한 채로 잰 "스냅샷이 흘렀다" 는 아무것도 뜻하지 않는다.

## 출처 게이트 (P1·P2 — `two_client_view.py raw2` 와 같은 방어)

파일 경로가 아니라 **봇 실행 디렉터리**를 받는다. `summary.json` 이 동반돼야 하고(P1: Unity 는
이 파일을 쓰지 않는다), CSV 의 `observer_actor_id` 집합이 `summary.snapshots_per_bot` 의
집합과 같아야 한다(P2: 다른 실행의 CSV 를 덮어 끼우기).

## 봇의 자체 계수를 **믿지 않는다**

`summary.json` 은 이미 `interval_violations` · `controlled_ship_missing` 을 들고 있다.
**그 수로 판정하지 않는다** — CSV 에서 다시 계산하고, 봇의 수는 **대조로만** 싣는다.
둘이 다르면 그 차이 자체가 발견이다(블록 7 에서 도구·독립 재계산을 둘 다 돌린 것과 같은 규율).

사용법:

    python tests/e2e/block8_invariants.py snapshots --bot-out-dir <DIR> [--evidence <F.json>]
    python tests/e2e/block8_invariants.py spawn [--system cradle] [--evidence <F.json>]
    python tests/e2e/block8_invariants.py db-outage --bot-out-dir <DIR> \
        --stats-before <F.json> --stats-after <F.json> [--evidence <F.json>]
    python tests/e2e/block8_invariants.py selftest

종료 코드: `0` 전부 PASS / `1` FAIL / `2` 사용법·환경 / `4` **미검증**(표본·증거 요건)
"""

from __future__ import annotations

import argparse
import json
import sys
from collections import defaultdict
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import db  # noqa: E402

REPO_ROOT = Path(__file__).resolve().parents[2]

EXIT_EVIDENCE = 4

PASS = "PASS"
FAIL = "FAIL"
NO_SAMPLES = "미검증(표본 없음)"
NO_EVIDENCE = "미검증(증거 요건)"

# 봇 `snapshots.csv` 의 12 열. `two_client_view.py` 의 COLUMNS 와 같아야 한다 — 다르면
# 한쪽만 고쳤을 때 조용히 지나간다(SC-85 가 잡으려는 형태).
COLUMNS = [
    "tick", "observer_actor_id", "ship_id", "presence",
    "px_mm", "py_mm", "pz_mm", "vx_mm_s", "vy_mm_s", "vz_mm_s",
    "render_offset_mm", "render_offset_deg",
]

NL = chr(10)


# ─────────────────────────────────────────────────────────────────────────────
# 봇 산출물 읽기 + 출처 게이트 P1·P2
# ─────────────────────────────────────────────────────────────────────────────

class Rows:
    """`snapshots.csv` 한 장. 관측자별 · tick 별로 갈라 둔다."""

    def __init__(self, header: list[str], rows: list[dict[str, str]]) -> None:
        self.header = header
        self.rows = rows
        # observer -> tick -> [(ship_id, presence)]
        self.by_obs: dict[str, dict[int, list[tuple[str, str]]]] = defaultdict(lambda: defaultdict(list))
        for r in rows:
            self.by_obs[r["observer_actor_id"]][int(r["tick"])].append((r["ship_id"], r["presence"]))

    @property
    def observers(self) -> list[str]:
        return sorted(self.by_obs)


def load_bot_dir(out_dir: Path) -> tuple[Rows, dict]:
    """봇 `--out` 디렉터리를 읽고 P1·P2 를 건다."""
    if not out_dir.is_dir():
        raise db.EnvironmentProblem(f"봇 출력 디렉터리가 없다: {out_dir}")

    summary_path = out_dir / "summary.json"
    csv_path = out_dir / "snapshots.csv"
    # P1: Unity 관측자 세션은 summary.json 을 쓰지 않는다. 없으면 봇 산출물이 아니다.
    if not summary_path.is_file():
        raise db.EnvironmentProblem(
            f"summary.json 이 없다 ({summary_path}). 이 명령은 **봇 실행 디렉터리**를 받는다 — "
            "CSV 파일 경로가 아니다(P1)."
        )
    if not csv_path.is_file():
        raise db.EnvironmentProblem(f"snapshots.csv 가 없다: {csv_path}")

    summary = json.loads(summary_path.read_text(encoding="utf-8"))

    lines = csv_path.read_text(encoding="utf-8").splitlines()
    if not lines:
        raise db.EnvironmentProblem(f"snapshots.csv 가 비어 있다: {csv_path}")
    header = [c.strip() for c in lines[0].split(",")]
    if header != COLUMNS:
        raise db.NotImplementedYet(
            "snapshots.csv 헤더가 계약의 12 열과 다르다." + NL
            + f"  기대: {','.join(COLUMNS)}" + NL
            + f"  실제: {','.join(header)}"
        )
    rows = []
    for line in lines[1:]:
        if not line.strip():
            continue
        parts = line.split(",")
        if len(parts) != len(COLUMNS):
            raise db.NotImplementedYet(f"열 수가 {len(COLUMNS)} 가 아닌 행이 있다: {line[:120]}")
        rows.append(dict(zip(COLUMNS, parts)))

    data = Rows(header, rows)

    # P2: CSV 의 관측자 집합 == summary 가 말하는 관측자 집합.
    declared = {
        b.get("observer_actor_id")
        for b in summary.get("snapshots_per_bot", [])
        if b.get("observer_actor_id")
    }
    seen = set(data.observers)
    if declared and declared != seen:
        raise db.NotImplementedYet(
            "P2 위반 — CSV 의 observer_actor_id 집합이 summary.json 과 다르다." + NL
            + f"  summary: {sorted(declared)}" + NL
            + f"  csv    : {sorted(seen)}"
        )
    return data, summary


def _producer(out_dir: Path, summary: dict) -> dict:
    """계약 SC-63 (d) 와 같은 규율 — 무엇이 이 수를 만들었는지 적는다."""
    return {
        "out_dir": str(out_dir),
        "snapshots_csv": str(out_dir / "snapshots.csv"),
        "summary_json": str(out_dir / "summary.json"),
        "stage": summary.get("stage"),
        "url": summary.get("url"),
        "bots": summary.get("bots"),
        "seed": summary.get("seed"),
        "duration_secs": summary.get("duration_secs"),
        "send_hz": summary.get("send_hz"),
    }


# ─────────────────────────────────────────────────────────────────────────────
# SC-28 — 스냅샷 간격
# ─────────────────────────────────────────────────────────────────────────────

def evaluate_sc28(data: Rows, summary: dict) -> tuple[dict, str]:
    """연속 두 스냅샷의 envelope tick 차이가 `snapshot_interval_ticks` 와 같은가.

    **각 세션의 첫 스냅샷은 제외한다** — 스냅샷은 전역 tick 기준(`tick % interval == 0`)으로
    발사되므로 중간에 들어온 세션의 첫 간격은 0~1 tick 이다(계약 SC-28, server ack §1-⑤).
    제외한 건수(세션당 1건)를 반드시 적는다.
    """
    per_bot = {b.get("observer_actor_id"): b for b in summary.get("snapshots_per_bot", [])}
    intervals = [
        b.get("snapshot_interval_ticks")
        for b in summary.get("snapshots_per_bot", [])
        if b.get("snapshot_interval_ticks") is not None
    ]
    expected = intervals[0] if intervals else None

    detail: dict = {
        "expected_interval_ticks": expected,
        "expected_interval_source": "summary.json snapshots_per_bot[].snapshot_interval_ticks "
                                    "(서버가 SESSION_READY 로 알려 준 값 — 이 도구는 숫자를 지어내지 않는다)",
        "per_observer": {},
        "checked_pairs": 0,
        "violating_pairs": 0,
        "excluded_first_snapshots": 0,
        "violations_head": [],
    }
    if expected is None:
        detail["why"] = "summary.json 이 snapshot_interval_ticks 를 싣지 않았다 — 기대값의 출처가 없다"
        return detail, NO_EVIDENCE

    mismatch_with_bot: dict[str, dict] = {}
    for obs in data.observers:
        ticks = sorted(data.by_obs[obs])
        # 세션당 첫 스냅샷 1건을 제외한다 = 첫 tick 을 기준점으로만 쓰고 그 앞 간격은 없다.
        excluded = 1 if ticks else 0
        gaps = [(a, b - a) for a, b in zip(ticks, ticks[1:])]
        bad = [(t, g) for t, g in gaps if g != expected]
        detail["per_observer"][obs] = {
            "snapshot_ticks": len(ticks),
            "checked_pairs": len(gaps),
            "violating_pairs": len(bad),
            "excluded_first_snapshots": excluded,
        }
        detail["checked_pairs"] += len(gaps)
        detail["violating_pairs"] += len(bad)
        detail["excluded_first_snapshots"] += excluded
        detail["violations_head"].extend(
            {"observer": obs, "after_tick": t, "gap_ticks": g} for t, g in bad[:5]
        )
        # 봇의 자체 계수와 대조한다 — 판정에는 쓰지 않는다.
        b = per_bot.get(obs)
        if b is not None and b.get("interval_violations") != len(bad):
            mismatch_with_bot[obs] = {
                "bot_reported_violations": b.get("interval_violations"),
                "recomputed_violations": len(bad),
            }

    detail["violations_head"] = detail["violations_head"][:10]
    detail["bot_self_count_cross_check"] = (
        "일치" if not mismatch_with_bot else mismatch_with_bot
    )
    detail["note"] = (
        "판정은 **CSV 재계산**으로 한다. summary.json 의 interval_violations 는 대조로만 싣는다 — "
        "재는 쪽과 판정하는 쪽이 같은 출처면 대조가 항진명제다."
    )

    # 자명 통과 방어: 검사한 쌍이 0 이면 아무것도 재지 않았다.
    if detail["checked_pairs"] == 0:
        return detail, NO_SAMPLES
    if mismatch_with_bot:
        # 두 출처가 어긋났다 — 어느 쪽이 맞든 이 실행으로는 닫을 수 없다.
        return detail, FAIL
    return detail, (PASS if detail["violating_pairs"] == 0 else FAIL)


# ─────────────────────────────────────────────────────────────────────────────
# SC-30 — controlled_ship_id 가 자기 함선이고 ships 안에 있다
# ─────────────────────────────────────────────────────────────────────────────

def evaluate_sc30(data: Rows, summary: dict) -> tuple[dict, str]:
    per_bot = {b.get("observer_actor_id"): b for b in summary.get("snapshots_per_bot", [])}
    detail: dict = {"per_observer": {}, "ticks_checked": 0, "ticks_missing_controlled": 0,
                    "missing_head": []}

    for obs in data.observers:
        b = per_bot.get(obs) or {}
        controlled = b.get("controlled_ship_id")
        ticks = sorted(data.by_obs[obs])
        if not controlled:
            detail["per_observer"][obs] = {"controlled_ship_id": None,
                                           "why": "summary 가 이 관측자의 controlled_ship_id 를 싣지 않았다"}
            continue
        missing = [t for t in ticks if controlled not in {s for s, _ in data.by_obs[obs][t]}]
        detail["per_observer"][obs] = {
            "controlled_ship_id": controlled,
            "ticks_checked": len(ticks),
            "ticks_missing_controlled": len(missing),
            "bot_reported_missing": b.get("controlled_ship_missing"),
        }
        detail["ticks_checked"] += len(ticks)
        detail["ticks_missing_controlled"] += len(missing)
        detail["missing_head"].extend({"observer": obs, "tick": t} for t in missing[:5])

    detail["missing_head"] = detail["missing_head"][:10]
    detail["note"] = (
        "각 관측자의 `controlled_ship_id` 가 **그 관측자가 받은 모든 스냅샷의 ships 안에** 있는가. "
        "판정은 CSV 재계산이고 봇의 controlled_ship_missing 은 대조로만 싣는다."
    )

    # 자명 통과 방어: controlled_ship_id 가 하나도 없거나 검사한 tick 이 0 이면 잰 것이 없다.
    known = [v for v in detail["per_observer"].values() if v.get("controlled_ship_id")]
    if not known or detail["ticks_checked"] == 0:
        return detail, NO_SAMPLES
    return detail, (PASS if detail["ticks_missing_controlled"] == 0 else FAIL)


# ─────────────────────────────────────────────────────────────────────────────
# SC-32 — LINGERING presence 와 "디스폰 뒤에는 사라진다"
# ─────────────────────────────────────────────────────────────────────────────

ALLOWED_PRESENCE = {"ACTIVE", "LINGERING"}


def evaluate_sc32(data: Rows, summary: dict) -> tuple[dict, str]:
    """계약: 잔류 함선의 presence 가 `LINGERING` 이고, **디스폰된 다음 스냅샷부터 사라진다.**

    ⚠ "사라진 뒤 안 보인다" 는 그대로 재면 **항진명제**다(마지막 등장 이후에는 정의상 안 보인다).
    재는 양은 그 반대다 — **없어졌다가 다시 나타난 건수**. 그리고 그 0 이 뜻을 가지려면
    **이 실행에서 실제로 디스폰이 일어났어야 한다.** 둘을 같이 단언한다.
    """
    presence_seen: dict[str, int] = defaultdict(int)
    reappearances: list[dict] = []
    despawned: list[dict] = []
    lingering_ships: set[str] = set()

    for obs in data.observers:
        ticks = sorted(data.by_obs[obs])
        tick_index = {t: i for i, t in enumerate(ticks)}
        # ship -> 그 ship 이 나타난 스냅샷 인덱스들
        appear: dict[str, list[int]] = defaultdict(list)
        for t in ticks:
            for ship, presence in data.by_obs[obs][t]:
                presence_seen[presence] += 1
                if presence == "LINGERING":
                    lingering_ships.add(ship)
                appear[ship].append(tick_index[t])

        for ship, idxs in appear.items():
            # 재등장 = 연속이 아닌 자리. 한 번이라도 비었다가 다시 나오면 위반이다.
            for a, b in zip(idxs, idxs[1:]):
                if b != a + 1:
                    reappearances.append({
                        "observer": obs, "ship_id": ship,
                        "absent_from_tick": ticks[a + 1], "reappeared_at_tick": ticks[b],
                        "absent_snapshots": b - a - 1,
                    })
            # 디스폰 = 관측 창이 끝나기 전에 사라졌다.
            if idxs[-1] != len(ticks) - 1:
                despawned.append({
                    "observer": obs, "ship_id": ship,
                    "last_seen_tick": ticks[idxs[-1]],
                    "observer_last_tick": ticks[-1],
                })

    unexpected = sorted(set(presence_seen) - ALLOWED_PRESENCE)
    detail = {
        "presence_values_seen": dict(sorted(presence_seen.items())),
        "allowed_presence_values": sorted(ALLOWED_PRESENCE),
        "unexpected_presence_values": unexpected,
        "lingering_ships_seen": len(lingering_ships),
        "despawn_events_observed": len(despawned),
        "despawn_head": despawned[:5],
        "reappearances_after_absence": len(reappearances),
        "reappearance_head": reappearances[:5],
        "bot_reported_lingering_seen": [
            b.get("lingering_seen") for b in summary.get("snapshots_per_bot", [])
        ],
        "note": (
            "판정량은 **재등장 건수**다 — '마지막 등장 이후 안 보인다' 는 정의상 참이라 "
            "그대로 재면 항진명제다. 그리고 `despawn_events_observed == 0` 이면 "
            "**재등장 0 은 아무것도 뜻하지 않으므로** PASS 가 아니라 미검증이다."
        ),
    }

    if unexpected:
        return detail, FAIL
    # ⚠ 순서가 판정이다 — **위반을 먼저 본다.** 재등장은 그 자체가 위반이고, 디스폰 표본이
    # 있었는지와 무관하다. 표본 가드를 먼저 걸면 **가드가 FAIL 을 가린다**: 마지막 스냅샷에서
    # 다시 나타난 함선은 "관측 창 끝까지 보였다" 가 되어 despawned 목록에 안 들어가고,
    # 그러면 진짜 위반이 `미검증(표본 없음)` 으로 인쇄된다. 이 도구의 selftest 가
    # `sc32_negative_control_reappears_after_despawn` 에서 그것을 실제로 잡았다.
    if reappearances:
        return detail, FAIL
    # 겨냥한 조건이 실제로 발생했는가 — 잔류를 봤고, 실제로 사라진 함선이 있었는가.
    if not lingering_ships or not despawned:
        detail["why_not_judged"] = (
            f"LINGERING 관측 {len(lingering_ships)}척 · 디스폰 관측 {len(despawned)}건 — "
            "둘 다 1 이상이어야 이 항목이 무언가를 잰다. 절차서 §B 의 세션 회전을 돌렸는지 확인한다."
        )
        return detail, NO_SAMPLES
    return detail, PASS


# ─────────────────────────────────────────────────────────────────────────────
# SC-32 (디스폰 표본) — `bots resume --gap > linger` 의 관측자 행
# ─────────────────────────────────────────────────────────────────────────────

def evaluate_sc32_resume(doc: dict, linger_seconds: int, tick_hz: int) -> tuple[dict, str]:
    """`bots resume --out <F.json>` 의 `observer_rows` 로 **디스폰을 실제로 관측**한다.

    ⚠ 왜 이 경로가 필요한가 — **`bots run` 의 어떤 단계도 디스폰을 관측하지 못한다.**
    `run` 은 항상 `bot-000..bot-(N-1)` 로 접속하므로, 단계 B(세션 회전)의 다음 주기는 같은
    라벨로 돌아와 **잔류 창 안의 재개**가 되고(디스폰이 일어나지 않는다), 단계 A 는 아무도
    중간에 나가지 않으며, 실행이 끝나면 **지켜볼 관측자가 남지 않는다.**
    `resume` 만이 *"한쪽은 나가 있고 다른 쪽은 계속 보고 있다"* 를 만든다.

    판정 전에 단언하는 것 셋 (전부 없으면 PASS 가 아니다):
      (1) `gap_s > linger_seconds` — 잔류 창 안이면 그것은 **재개이지 디스폰이 아니다**(G-j).
      (2) 관측자가 그 함선의 `LINGERING` 을 **실제로 봤다**.
      (3) 그 함선이 사라진 **뒤에도** 관측자가 스냅샷을 계속 받았다
          (`observer_last_tick` > 함선의 마지막 tick). 아니면 "사라졌다" 와
          "관측자가 먼저 눈을 감았다" 를 구분할 수 없다.
    """
    header = doc.get("observer_rows_header", "")
    rows_raw = doc.get("observer_rows") or []
    gap_s = doc.get("gap_s")
    obs_last = doc.get("observer_last_tick")
    obs_first = doc.get("observer_first_tick")

    detail: dict = {
        "gap_s": gap_s,
        "linger_seconds": linger_seconds,
        "observer_first_tick": obs_first,
        "observer_last_tick": obs_last,
        "subject_ship_rows": len(rows_raw),
        "note": (
            "`observer_rows` 는 **leg1 함선만** 걸러 담긴다(`main.rs:621`). 따라서 이 판정은 "
            "그 한 척의 presence 궤적이고, 관측자가 계속 보고 있었다는 것은 "
            "`observer_last_tick` 으로 단언한다."
        ),
    }

    if header != ",".join(COLUMNS):
        detail["why_not_judged"] = (
            "observer_rows_header 가 계약의 12 열과 다르다." + NL
            + f"  기대: {','.join(COLUMNS)}" + NL + f"  실제: {header}"
        )
        return detail, NO_EVIDENCE

    if gap_s is None or gap_s <= linger_seconds:
        detail["why_not_judged"] = (
            f"`--gap {gap_s}` 이 `linger_seconds` {linger_seconds} 를 넘지 않는다 — "
            "그 재접속은 **재개이고 디스폰이 아니다**(계약 게이트 G-j). "
            f"`--gap {linger_seconds + 5}` 이상으로 다시 돌린다."
        )
        return detail, NO_SAMPLES

    parsed = []
    for line in rows_raw:
        parts = line.split(",")
        if len(parts) != len(COLUMNS):
            detail["why_not_judged"] = f"열 수가 {len(COLUMNS)} 가 아닌 행: {line[:120]}"
            return detail, NO_EVIDENCE
        parsed.append(dict(zip(COLUMNS, parts)))

    if not parsed:
        detail["why_not_judged"] = (
            "관측자가 대상 함선의 행을 한 건도 받지 못했다 — 두 봇이 같은 성계에 있었는지 확인한다"
        )
        return detail, NO_SAMPLES

    ticks = sorted(int(r["tick"]) for r in parsed)
    presence = {}
    for r in parsed:
        presence[r["presence"]] = presence.get(r["presence"], 0) + 1
    interval = tick_hz // 10 if tick_hz >= 10 else 1  # 스냅샷 2 tick @ 20 Hz
    reappear = [
        {"absent_from_tick": a + interval, "reappeared_at_tick": b}
        for a, b in zip(ticks, ticks[1:]) if b - a > interval
    ]
    unexpected = sorted(set(presence) - ALLOWED_PRESENCE)

    detail.update({
        "presence_values_seen": dict(sorted(presence.items())),
        "unexpected_presence_values": unexpected,
        "ship_first_tick": ticks[0],
        "ship_last_tick": ticks[-1],
        "expected_snapshot_interval_ticks": interval,
        "reappearances_after_absence": len(reappear),
        "reappearance_head": reappear[:5],
        "observer_outlived_ship": (obs_last is not None and obs_last > ticks[-1]),
        "observer_ticks_after_ship_vanished": (
            (obs_last - ticks[-1]) if isinstance(obs_last, int) else None
        ),
    })

    if unexpected:
        return detail, FAIL
    # 위반을 먼저 본다 — 표본 가드가 FAIL 을 가리지 않게 (evaluate_sc32 와 같은 순서 규율).
    if reappear:
        return detail, FAIL
    if "LINGERING" not in presence:
        detail["why_not_judged"] = (
            "관측자가 그 함선의 LINGERING 을 한 번도 보지 못했다 — `--settle`/`--listen` 을 늘린다"
        )
        return detail, NO_SAMPLES
    if not detail["observer_outlived_ship"]:
        detail["why_not_judged"] = (
            f"함선의 마지막 tick {ticks[-1]} 이 관측자의 마지막 tick {obs_last} 과 같거나 늦다 — "
            "**관측자가 먼저 눈을 감았다.** 그러면 '디스폰 뒤에 사라졌다' 를 잰 것이 아니다. "
            f"`--listen` 을 늘려(권장 {linger_seconds + 10} 초 이상) 다시 돌린다."
        )
        return detail, NO_SAMPLES
    return detail, PASS


# ─────────────────────────────────────────────────────────────────────────────
# SC-10 — 스폰 위치가 data/ 의 지점이고, 만료 후 재스폰이 같은 자리인가
# ─────────────────────────────────────────────────────────────────────────────

def _spawn_points_mm(system: str) -> tuple[list[tuple[int, int, int]], int, float, Path]:
    """`data/world/systems/<system>.json` 의 points_m × 1000 과 linger_seconds·step."""
    path = REPO_ROOT / "data" / "world" / "systems" / f"{system}.json"
    if not path.is_file():
        raise db.EnvironmentProblem(f"성계 데이터 파일이 없다: {path}")
    doc = json.loads(path.read_text(encoding="utf-8"))
    spawn = doc.get("spawn") or doc.get("spawns") or {}
    points = spawn.get("points_m")
    if not points:
        for v in doc.values():
            if isinstance(v, dict) and "points_m" in v:
                spawn, points = v, v["points_m"]
                break
    if not points:
        raise db.NotImplementedYet(f"{path} 에서 points_m 를 찾지 못했다")
    mm = [tuple(int(round(c * 1000)) for c in p) for p in points]
    presence = doc.get("presence", {})
    return mm, int(presence.get("linger_seconds", 30)), float(spawn.get("radial_offset_step_m", 0.0)), path


def evaluate_sc10(system: str, tick_hz: int, since_tick: int | None = None) -> tuple[dict, str]:
    """⚠ `since_tick` 이 **판정의 범위이고 그것이 이 항목의 전부다** (qa r13 실측으로 추가).

    범위를 안 주면 `domain_events` **전체**를 본다. 그 테이블은 추가 전용이고 슬라이스 전체의
    실행을 담고 있어 **동시 접속 수가 서로 다른 실행이 섞인다.** 무범위로 돌려 보니
    재스폰 불일치 **4건**이 나왔는데, 넷 다 옛 다봇 실행이고 **두 번째 위치가 전부 스폰 지점
    위**였다 — 즉 난수가 아니라 `assignment_rule` 의 **점유 탐침**(`index0+1, …` 로 빈 지점을
    찾는다)이다. 한 건은 SQL 로 확인했다: actor `…0090` 의 2차 스폰 tick 398728 에 1차 지점은
    actor `…0004` 의 함선(396971 스폰 → 399041 디스폰)이 **점유 중이었다.**
    **그러므로 이 항목은 절차서 §A-0 의 저밀도 실행 하나에 범위를 맞춰 판정한다.**
    """
    db.require_tables("domain_events")
    points_mm, linger_seconds, step_m, data_path = _spawn_points_mm(system)
    linger_ticks = linger_seconds * tick_hz

    where = f"and tick >= {int(since_tick)} " if since_tick is not None else ""
    rows = db.psql_rows(
        "select actor_id, tick, "
        "payload->>'position_x_mm', payload->>'position_y_mm', payload->>'position_z_mm' "
        f"from domain_events where event_type='SHIP_SPAWNED' {where}order by tick;"
    )
    spawns = [
        {"actor_id": r[0], "tick": int(r[1]), "pos_mm": (int(r[2]), int(r[3]), int(r[4]))}
        for r in rows if len(r) >= 5 and r[2] not in (None, "")
    ]

    point_set = set(points_mm)
    exact = [s for s in spawns if s["pos_mm"] in point_set]
    offset = [s for s in spawns if s["pos_mm"] not in point_set]

    by_actor: dict[str, list[dict]] = defaultdict(list)
    for s in spawns:
        by_actor[s["actor_id"]].append(s)

    respawn_pairs, respawn_mismatch = [], []
    for actor, lst in by_actor.items():
        lst.sort(key=lambda s: s["tick"])
        for a, b in zip(lst, lst[1:]):
            gap = b["tick"] - a["tick"]
            # G-j: linger_seconds 안의 재접속은 **재개**다. 스폰으로 재면 항진명제가 된다.
            if gap <= linger_ticks:
                continue
            pair = {"actor_id": actor, "first_tick": a["tick"], "second_tick": b["tick"],
                    "gap_ticks": gap, "gap_seconds": round(gap / tick_hz, 2),
                    "first_pos_mm": list(a["pos_mm"]), "second_pos_mm": list(b["pos_mm"]),
                    "same": a["pos_mm"] == b["pos_mm"]}
            respawn_pairs.append(pair)
            if not pair["same"]:
                respawn_mismatch.append(pair)

    detail = {
        "judgment_scope": {
            "since_tick": since_tick,
            "why": (
                "`domain_events` 는 추가 전용이라 범위를 안 주면 **동시 접속 수가 다른 실행이 "
                "섞인다.** 점유가 높았던 실행에서는 `assignment_rule` 의 탐침이 **다른 지점**을 "
                "고르는 것이 정상이므로, 무범위 판정은 정상 동작을 FAIL 로 읽는다."
            ) if since_tick is not None else (
                "⚠ **범위 없음 — 테이블 전체다.** 여러 실행이 섞이므로 이 결과로 판정하지 않는다. "
                "`--since-tick <서버 기동 tick>` 으로 절차서 §A-0 의 실행에 맞춘다."
            ),
        },
        "data_file": str(data_path),
        "spawn_points": len(points_mm),
        "linger_seconds": linger_seconds,
        "linger_ticks_at_hz": {"tick_hz": tick_hz, "linger_ticks": linger_ticks},
        "radial_offset_step_m": step_m,
        "spawns_total": len(spawns),
        "spawns_exactly_on_a_point": len(exact),
        "spawns_not_on_a_point": len(offset),
        "not_on_a_point_head": [
            {"actor_id": s["actor_id"], "tick": s["tick"], "pos_mm": list(s["pos_mm"])}
            for s in offset[:5]
        ],
        "respawn_pairs_beyond_linger": len(respawn_pairs),
        "respawn_pairs_head": respawn_pairs[:5],
        "respawn_position_mismatches": len(respawn_mismatch),
        "respawn_mismatch_head": respawn_mismatch[:5],
        "note": (
            "**후반부(만료 후 재스폰)만 tick 차이 > linger_ticks 인 짝으로 판정한다**(계약 게이트 G-j). "
            "30 초 안의 재접속은 재개이고 SHIP_SPAWNED 를 새로 내지도 않는다 — 그 짝을 세면 항진명제다."
        ),
        "occupancy_caveat": (
            f"스폰 배정은 점유 시 `radial_offset_step_m`({step_m} m) 만큼 **반경 방향으로 밀린다**"
            "(`assignment_rule`). 그러면 위치가 points_m 와 정확히 일치하지 않는 것이 **정상이다.** "
            "따라서 이 항목의 전반부는 **점유가 낮은 실행**에서 재야 한다 — 절차서 §A-0 참조. "
            "spawns_not_on_a_point 가 0 이 아니면 FAIL 로 닫기 전에 그 실행의 동시 접속 수를 본다."
        ),
    }

    if not spawns:
        detail["why_not_judged"] = "domain_events 에 SHIP_SPAWNED 가 없다 — 아무도 접속하지 않았다"
        return detail, NO_SAMPLES
    if not respawn_pairs:
        detail["why_not_judged"] = (
            f"tick 차이가 {linger_ticks} tick({linger_seconds} 초) 를 넘는 같은 actor 의 스폰 짝이 없다. "
            "절차서 §A-0 의 두 번째 실행을 35 초 이상 띄워 돌렸는지 확인한다 — "
            "**이 짝이 없으면 후반부는 판정되지 않는다.**"
        )
        return detail, NO_SAMPLES
    if offset or respawn_mismatch:
        return detail, FAIL
    return detail, PASS


# ─────────────────────────────────────────────────────────────────────────────
# SC-74 — DB 중단 구간에도 스냅샷이 흐르는가
# ─────────────────────────────────────────────────────────────────────────────

def _server_initiated_closes(out_dir: Path) -> tuple[int | None, dict]:
    """"끊긴 연결 0" 의 **출처**. qa r13 실측으로 고쳤다.

    처음에는 `summary.get("server_initiated_closes")` 를 봤는데 **`summary.json` 에 그 키가 없다.**
    봇은 그 수를 stdout 에만 찍고(`report.rs:62,158` 의 `gates`) 파일로 내보내지 않는다.
    그래서 값이 늘 `None` 이었고, `None > 0` 이 거짓이라 **그 절반의 판정이 한 번도 평가되지
    않은 채 PASS 가 인쇄됐다** — 계약 §7a 가 막으려는 바로 그 형태다.

    진짜 출처는 `sessions.json` 의 **세션별 `close_initiator`** 다. 읽을 수 없으면
    **0 으로 가정하지 않고 `None` 을 돌려주고, 호출자가 `미검증(증거 요건)` 으로 닫는다.**
    """
    path = out_dir / "sessions.json"
    if not path.is_file():
        return None, {"source": str(path), "why": "sessions.json 이 없다 — 끊긴 연결 수의 출처가 없다"}
    try:
        rows = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        return None, {"source": str(path), "why": f"sessions.json 을 읽지 못했다: {exc}"}
    if not isinstance(rows, list) or not rows:
        return None, {"source": str(path), "why": "sessions.json 이 비었거나 배열이 아니다"}
    initiators = [r.get("close_initiator") for r in rows]
    if any(i is None for i in initiators):
        return None, {"source": str(path), "sessions": len(rows),
                      "why": "close_initiator 가 없는 세션이 있다 — 0 으로 가정하지 않는다"}
    server = [r for r in rows if r.get("close_initiator") != "client"]
    return len(server), {
        "source": str(path),
        "sessions": len(rows),
        "close_initiators": {i: initiators.count(i) for i in sorted(set(initiators))},
        "server_initiated": [
            {"bot": r.get("bot"), "close_initiator": r.get("close_initiator"),
             "close_code": r.get("close_code"), "peer_close_code": r.get("peer_close_code"),
             "close_reason_text": r.get("close_reason_text")}
            for r in server[:5]
        ],
    }


def evaluate_sc74(data: Rows, summary: dict, before: dict, after: dict,
                  out_dir: Path | None = None) -> tuple[dict, str]:
    """D 단계: postgres 를 멈춘 구간에 봇이 받은 스냅샷 수 > 0 이고 끊긴 연결이 0 인가.

    ⚠ **DB 가 실제로 멈췄다는 것을 먼저 단언한다.** postgres 가 멀쩡한 채로 잰
    "스냅샷이 흘렀다" 는 아무것도 뜻하지 않는다 — 이 슬라이스가 네 번 데인 형태다.
    단언의 근거는 중단 창 전후 `/debug/stats` 의 `persist_backlog` 증가
    (또는 `domain_events_persist_failed_total` 증가)다.
    """
    t0, t1 = before.get("tick"), after.get("tick")
    backlog_before = before.get("persist_backlog")
    backlog_after = after.get("persist_backlog")
    failed_before = before.get("domain_events_persist_failed_total", 0)
    failed_after = after.get("domain_events_persist_failed_total", 0)
    committed_before = before.get("last_committed_tick")
    committed_after = after.get("last_committed_tick")

    detail: dict = {
        "outage_tick_window": {"from_stats_before_tick": t0, "to_stats_after_tick": t1},
        "outage_actually_happened": {
            "persist_backlog_before": backlog_before,
            "persist_backlog_after": backlog_after,
            "domain_events_persist_failed_before": failed_before,
            "domain_events_persist_failed_after": failed_after,
            "last_committed_tick_before": committed_before,
            "last_committed_tick_after": committed_after,
            "why": (
                "DB 가 멈췄으면 영속화가 밀린다 — `persist_backlog` 가 늘거나 "
                "`domain_events_persist_failed_total` 이 는다. 둘 다 그대로면 "
                "**postgres 가 실제로 멈추지 않은 것**이고, 그때 '스냅샷이 흘렀다' 는 항진명제다."
            ),
        },
        "snapshots_in_window_per_observer": {},
        "observers_with_zero_in_window": [],
    }

    if t0 is None or t1 is None or t1 <= t0:
        detail["why_not_judged"] = (
            "중단 창의 tick 범위를 두 stats 에서 읽지 못했다 "
            f"(before.tick={t0}, after.tick={t1}). --stats-before 를 **멈추기 직전**, "
            "--stats-after 를 **재개 직후**에 받았는지 확인한다."
        )
        return detail, NO_EVIDENCE

    backlog_grew = (
        isinstance(backlog_before, int) and isinstance(backlog_after, int)
        and backlog_after > backlog_before
    )
    failed_grew = (
        isinstance(failed_before, int) and isinstance(failed_after, int)
        and failed_after > failed_before
    )
    detail["outage_actually_happened"]["asserted"] = bool(backlog_grew or failed_grew)
    if not (backlog_grew or failed_grew):
        detail["why_not_judged"] = (
            "중단 창 전후로 `persist_backlog` 도 `domain_events_persist_failed_total` 도 늘지 않았다 — "
            "**DB 가 멈췄다는 증거가 없다.** postgres 를 실제로 멈췄는지, 그리고 멈춘 구간 안에서 "
            "stats 를 받았는지 확인한다(절차서 §D)."
        )
        return detail, NO_EVIDENCE

    for obs in data.observers:
        n = sum(1 for t in data.by_obs[obs] if t0 <= t <= t1)
        detail["snapshots_in_window_per_observer"][obs] = n
        if n == 0:
            detail["observers_with_zero_in_window"].append(obs)

    errors = [
        {"observer_actor_id": b.get("observer_actor_id"), "errors": b.get("errors")}
        for b in summary.get("snapshots_per_bot", []) if b.get("errors")
    ]
    detail["bots_with_errors"] = errors
    closes, close_detail = _server_initiated_closes(out_dir) if out_dir else (None, {"why": "out_dir 미지정"})
    detail["server_initiated_closes"] = closes
    detail["server_initiated_closes_evidence"] = close_detail
    detail["note"] = (
        "판정 둘: 중단 창 안에서 **모든 봇이** 스냅샷을 1건 이상 받았는가, 그리고 끊긴 연결이 0 인가. "
        "끊긴 연결 수는 `sessions.json` 의 세션별 `close_initiator` 에서 읽는다 — "
        "`summary.json` 에는 그 수가 **없다**(봇이 stdout 에만 찍는다)."
    )

    if not data.observers:
        detail["why_not_judged"] = "CSV 에 관측자가 없다"
        return detail, NO_SAMPLES
    # 읽지 못한 값을 0 으로 가정하지 않는다 — 그러면 그 절반이 평가되지 않은 채 PASS 가 난다.
    if closes is None:
        detail["why_not_judged"] = (
            "끊긴 연결 수를 읽지 못했다 — 판정 둘 중 하나가 평가되지 않는다. "
            f"{close_detail.get('why')}"
        )
        return detail, NO_EVIDENCE
    if detail["observers_with_zero_in_window"] or errors or closes > 0:
        return detail, FAIL
    return detail, PASS


# ─────────────────────────────────────────────────────────────────────────────
# 하위 명령
# ─────────────────────────────────────────────────────────────────────────────

def _exit_for(verdicts: list[str]) -> int:
    if any(v == FAIL for v in verdicts):
        return db.EXIT_FAIL
    if any(v.startswith("미검증") for v in verdicts):
        return EXIT_EVIDENCE
    return db.EXIT_OK


def cmd_snapshots(args: argparse.Namespace) -> int:
    out_dir = Path(args.bot_out_dir)
    data, summary = load_bot_dir(out_dir)
    d28, v28 = evaluate_sc28(data, summary)
    d30, v30 = evaluate_sc30(data, summary)
    d32, v32 = evaluate_sc32(data, summary)
    db.emit({
        "item": "SC-28 / SC-30 / SC-32 (AC-7 a/c/e) 봇 스냅샷 불변식 — 블록 8",
        "SC-28_verdict": v28,
        "SC-30_verdict": v30,
        "SC-32_verdict": v32,
        "producer": _producer(out_dir, summary),
        "structural": {
            "observers_in_csv": data.observers,
            "observer_count": len(data.observers),
            "rows_total": len(data.rows),
        },
        "SC-28": d28,
        "SC-30": d30,
        "SC-32": d32,
    }, args.evidence)
    return _exit_for([v28, v30, v32])


def cmd_spawn(args: argparse.Namespace) -> int:
    detail, verdict = evaluate_sc10(args.system, args.tick_hz, args.since_tick)
    db.emit({
        "item": "SC-10 (AC-3 c) 스폰 위치의 데이터 일치와 만료 후 재스폰 결정성 — 블록 8",
        "SC-10_verdict": verdict,
        "SC-10": detail,
    }, args.evidence)
    return _exit_for([verdict])


def cmd_db_outage(args: argparse.Namespace) -> int:
    out_dir = Path(args.bot_out_dir)
    data, summary = load_bot_dir(out_dir)
    before = json.loads(Path(args.stats_before).read_text(encoding="utf-8"))
    after = json.loads(Path(args.stats_after).read_text(encoding="utf-8"))
    detail, verdict = evaluate_sc74(data, summary, before, after, out_dir)
    db.emit({
        "item": "SC-74 (AC-18 f) DB 중단 구간의 스냅샷 지속 — 블록 8 D 단계",
        "SC-74_verdict": verdict,
        "producer": _producer(out_dir, summary),
        "stats_before": str(args.stats_before),
        "stats_after": str(args.stats_after),
        "SC-74": detail,
    }, args.evidence)
    return _exit_for([verdict])


# ─────────────────────────────────────────────────────────────────────────────
# selftest — **각 판정이 걸려야 할 입력에서 걸리는가**
# ─────────────────────────────────────────────────────────────────────────────

_OBS_A = "aaaa0000-0000-7000-8000-000000000001"
_OBS_B = "bbbb0000-0000-7000-8000-000000000002"
_SHIP_A = "01a0c000-0000-7000-8000-00000000000a"
_SHIP_B = "01a0c000-0000-7000-8000-00000000000b"


def _write_dir(root: Path, rows: list[tuple[int, str, str, str]], *,
               per_bot: list[dict], interval: int | None = 2,
               sessions: list[dict] | None = None) -> Path:
    root.mkdir(parents=True, exist_ok=True)
    with (root / "snapshots.csv").open("w", encoding="utf-8", newline="") as f:
        f.write(",".join(COLUMNS) + NL)
        for tick, obs, ship, presence in rows:
            f.write(f"{tick},{obs},{ship},{presence},0,0,{tick * 1000},0,0,0,0,0" + NL)
    for b in per_bot:
        b.setdefault("snapshot_interval_ticks", interval)
    (root / "summary.json").write_text(json.dumps({
        "stage": "a-steady", "url": "ws://127.0.0.1:8080/ws", "bots": len(per_bot),
        "seed": 42, "duration_secs": 60,
        "snapshots_per_bot": per_bot,
    }), encoding="utf-8")
    if sessions is not None:
        (root / "sessions.json").write_text(json.dumps(sessions), encoding="utf-8")
    return root


def _good_rows(ticks: list[int]) -> list[tuple[int, str, str, str]]:
    out = []
    for t in ticks:
        out.append((t, _OBS_A, _SHIP_A, "ACTIVE"))
        out.append((t, _OBS_A, _SHIP_B, "ACTIVE"))
        out.append((t, _OBS_B, _SHIP_B, "ACTIVE"))
        out.append((t, _OBS_B, _SHIP_A, "ACTIVE"))
    return out


def _both_bots() -> list[dict]:
    return [
        {"observer_actor_id": _OBS_A, "controlled_ship_id": _SHIP_A, "interval_violations": 0,
         "controlled_ship_missing": 0, "lingering_seen": True, "errors": []},
        {"observer_actor_id": _OBS_B, "controlled_ship_id": _SHIP_B, "interval_violations": 0,
         "controlled_ship_missing": 0, "lingering_seen": True, "errors": []},
    ]


def _despawn_rows(ticks: list[int]) -> list[tuple[int, str, str, str]]:
    """SHIP_B 가 중간에 LINGERING 이 됐다가 사라진다 — SC-32 가 무언가를 재는 입력."""
    out = []
    cut = len(ticks) // 2
    for i, t in enumerate(ticks):
        out.append((t, _OBS_A, _SHIP_A, "ACTIVE"))
        out.append((t, _OBS_B, _SHIP_B, "ACTIVE"))
        out.append((t, _OBS_B, _SHIP_A, "ACTIVE"))
        if i < cut:
            out.append((t, _OBS_A, _SHIP_B, "ACTIVE" if i < cut - 2 else "LINGERING"))
    return out


def cmd_resume_despawn(args: argparse.Namespace) -> int:
    doc = json.loads(Path(args.resume_json).read_text(encoding="utf-8"))
    _, linger_seconds, _, data_path = _spawn_points_mm(args.system)
    detail, verdict = evaluate_sc32_resume(doc, linger_seconds, args.tick_hz)
    db.emit({
        "item": "SC-32 (AC-7 e) 잔류 presence 와 디스폰 후 소멸 — 블록 8 (resume 관측자 경로)",
        "SC-32_verdict": verdict,
        "producer": {
            "resume_json": str(args.resume_json),
            "label": doc.get("label"), "observer": doc.get("observer"),
            "gap_s": doc.get("gap_s"), "listen_s": doc.get("listen_s"),
            "data_file": str(data_path),
        },
        "SC-32": detail,
    }, args.evidence)
    return _exit_for([verdict])


def cmd_selftest(args: argparse.Namespace) -> int:
    import tempfile

    tmp = Path(tempfile.mkdtemp(prefix="starfall-block8-"))
    ticks = [100 + 2 * i for i in range(20)]
    results: dict[str, str] = {}
    obs: dict[str, object] = {}

    # ── SC-28 ────────────────────────────────────────────────────────────────
    d, s = load_bot_dir(_write_dir(tmp / "ok", _good_rows(ticks), per_bot=_both_bots()))
    det, v = evaluate_sc28(d, s)
    results["sc28_clean"] = v
    obs["sc28_clean_checked_pairs"] = det["checked_pairs"]
    obs["sc28_clean_excluded_first"] = det["excluded_first_snapshots"]

    bad_ticks = ticks[:10] + [ticks[9] + 6] + [ticks[9] + 6 + 2 * i for i in range(1, 9)]
    d, s = load_bot_dir(_write_dir(tmp / "gap", _good_rows(bad_ticks), per_bot=[
        {"observer_actor_id": _OBS_A, "controlled_ship_id": _SHIP_A, "interval_violations": 1,
         "controlled_ship_missing": 0, "lingering_seen": True, "errors": []},
        {"observer_actor_id": _OBS_B, "controlled_ship_id": _SHIP_B, "interval_violations": 1,
         "controlled_ship_missing": 0, "lingering_seen": True, "errors": []},
    ]))
    det, v = evaluate_sc28(d, s)
    results["sc28_negative_control_gap_of_6"] = v
    obs["sc28_violating_pairs_on_that_input"] = det["violating_pairs"]

    # 한 세션이 스냅샷을 1건만 받으면 검사할 쌍이 없다 → 미검증
    d, s = load_bot_dir(_write_dir(tmp / "single", _good_rows(ticks[:1]), per_bot=_both_bots()))
    results["sc28_single_snapshot"] = evaluate_sc28(d, s)[1]

    # 봇의 자체 계수와 재계산이 어긋나면 닫지 않는다
    d, s = load_bot_dir(_write_dir(tmp / "liar", _good_rows(ticks), per_bot=[
        {"observer_actor_id": _OBS_A, "controlled_ship_id": _SHIP_A, "interval_violations": 7,
         "controlled_ship_missing": 0, "lingering_seen": True, "errors": []},
        {"observer_actor_id": _OBS_B, "controlled_ship_id": _SHIP_B, "interval_violations": 0,
         "controlled_ship_missing": 0, "lingering_seen": True, "errors": []},
    ]))
    results["sc28_bot_counter_disagrees"] = evaluate_sc28(d, s)[1]

    # ── SC-30 ────────────────────────────────────────────────────────────────
    d, s = load_bot_dir(_write_dir(tmp / "ok30", _good_rows(ticks), per_bot=_both_bots()))
    det, v = evaluate_sc30(d, s)
    results["sc30_clean"] = v
    obs["sc30_ticks_checked"] = det["ticks_checked"]

    drop = [r for r in _good_rows(ticks) if not (r[1] == _OBS_A and r[2] == _SHIP_A and r[0] == ticks[5])]
    d, s = load_bot_dir(_write_dir(tmp / "miss30", drop, per_bot=_both_bots()))
    det, v = evaluate_sc30(d, s)
    results["sc30_negative_control_own_ship_absent"] = v
    obs["sc30_ticks_missing_on_that_input"] = det["ticks_missing_controlled"]

    # ── SC-32 ────────────────────────────────────────────────────────────────
    # 아무도 사라지지 않은 실행 → 재등장 0 이지만 **항진명제**라 미검증이어야 한다
    d, s = load_bot_dir(_write_dir(tmp / "nodespawn", _good_rows(ticks), per_bot=_both_bots()))
    det, v = evaluate_sc32(d, s)
    results["sc32_no_despawn_is_not_pass"] = v
    obs["sc32_despawn_events_on_that_input"] = det["despawn_events_observed"]

    d, s = load_bot_dir(_write_dir(tmp / "despawn", _despawn_rows(ticks), per_bot=_both_bots()))
    det, v = evaluate_sc32(d, s)
    results["sc32_clean_with_real_despawn"] = v
    obs["sc32_despawn_events_observed"] = det["despawn_events_observed"]
    obs["sc32_lingering_ships_seen"] = det["lingering_ships_seen"]

    reappear = _despawn_rows(ticks) + [(ticks[-1], _OBS_A, _SHIP_B, "ACTIVE")]
    d, s = load_bot_dir(_write_dir(tmp / "reappear", reappear, per_bot=_both_bots()))
    det, v = evaluate_sc32(d, s)
    results["sc32_negative_control_reappears_after_despawn"] = v
    obs["sc32_reappearances_on_that_input"] = det["reappearances_after_absence"]

    bad_presence = _despawn_rows(ticks) + [(ticks[0], _OBS_A, _SHIP_B, "GHOST")]
    d, s = load_bot_dir(_write_dir(tmp / "ghost", bad_presence, per_bot=_both_bots()))
    results["sc32_negative_control_unknown_presence"] = evaluate_sc32(d, s)[1]

    # ── P1 · P2 출처 게이트 ──────────────────────────────────────────────────
    nosum = tmp / "nosummary"
    nosum.mkdir(parents=True, exist_ok=True)
    (nosum / "snapshots.csv").write_text(",".join(COLUMNS) + NL, encoding="utf-8")
    try:
        load_bot_dir(nosum)
        results["p1_no_summary"] = "accepted  <-- 방어가 안 걸렸다"
    except db.EnvironmentProblem:
        results["p1_no_summary"] = "rejected(EnvironmentProblem)"

    try:
        load_bot_dir(_write_dir(tmp / "p2", _good_rows(ticks), per_bot=[
            {"observer_actor_id": "cccc0000-0000-7000-8000-000000000003",
             "controlled_ship_id": _SHIP_A, "interval_violations": 0,
             "controlled_ship_missing": 0, "lingering_seen": True, "errors": []},
        ]))
        results["p2_observer_set_mismatch"] = "accepted  <-- 방어가 안 걸렸다"
    except db.NotImplementedYet:
        results["p2_observer_set_mismatch"] = "rejected(NotImplementedYet)"

    # ── SC-74 ────────────────────────────────────────────────────────────────
    client_closed = [{"bot": "bot-000", "close_initiator": "client", "close_code": 1000},
                     {"bot": "bot-001", "close_initiator": "client", "close_code": 1000}]
    dir_d = _write_dir(tmp / "d", _good_rows(ticks), per_bot=_both_bots(), sessions=client_closed)
    d, s = load_bot_dir(dir_d)
    grew = {"tick": ticks[0], "persist_backlog": 3, "domain_events_persist_failed_total": 0,
            "last_committed_tick": 90}
    grew_after = {"tick": ticks[-1], "persist_backlog": 240,
                  "domain_events_persist_failed_total": 0, "last_committed_tick": 90}
    det, v = evaluate_sc74(d, s, grew, grew_after, dir_d)
    results["sc74_clean_with_real_outage"] = v
    obs["sc74_snapshots_in_window"] = det["snapshots_in_window_per_observer"]

    flat_after = dict(grew_after, persist_backlog=3)
    results["sc74_negative_control_db_never_stopped"] = evaluate_sc74(d, s, grew, flat_after, dir_d)[1]

    # 중단 창에 스냅샷이 한 건도 없으면 FAIL
    late = {"tick": ticks[-1] + 100, "persist_backlog": 3,
            "domain_events_persist_failed_total": 0, "last_committed_tick": 90}
    late_after = dict(late, tick=ticks[-1] + 200, persist_backlog=240)
    det, v = evaluate_sc74(d, s, late, late_after, dir_d)
    results["sc74_negative_control_no_snapshots_in_window"] = v

    # **끊긴 연결이 있으면 FAIL** — 이 절반이 실제로 판정되는가(r13 에서 조용히 통과했던 자리)
    srv = [{"bot": "bot-000", "close_initiator": "client", "close_code": 1000},
           {"bot": "bot-001", "close_initiator": "server", "close_code": 1011,
            "peer_close_code": 1011, "close_reason_text": "SLOW_CONSUMER"}]
    dir_srv = _write_dir(tmp / "d_srv", _good_rows(ticks), per_bot=_both_bots(), sessions=srv)
    d2, s2 = load_bot_dir(dir_srv)
    det, v = evaluate_sc74(d2, s2, grew, grew_after, dir_srv)
    results["sc74_negative_control_server_initiated_close"] = v
    obs["sc74_server_closes_on_that_input"] = det["server_initiated_closes"]

    # **출처가 없으면 0 으로 가정하지 않는다** — 미검증이지 PASS 가 아니다
    dir_nos = _write_dir(tmp / "d_nosess", _good_rows(ticks), per_bot=_both_bots())
    d3, s3 = load_bot_dir(dir_nos)
    results["sc74_no_sessions_json_is_not_pass"] = evaluate_sc74(d3, s3, grew, grew_after, dir_nos)[1]

    # ── SC-32 resume 경로 ────────────────────────────────────────────────────
    hdr = ",".join(COLUMNS)

    def _resume_doc(*, gap_s, ship_ticks, presences, obs_last):
        return {
            "label": "bot-007", "observer": "bot-008", "gap_s": gap_s, "listen_s": 40.0,
            "observer_rows_header": hdr,
            "observer_rows": [
                f"{tk},{_OBS_B},{_SHIP_A},{pr},0,0,{tk * 1000},0,0,0,0,0"
                for tk, pr in zip(ship_ticks, presences)
            ],
            "observer_first_tick": ship_ticks[0] if ship_ticks else None,
            "observer_last_tick": obs_last,
        }

    live = [100 + 2 * i for i in range(10)]
    pres = ["ACTIVE"] * 5 + ["LINGERING"] * 5
    det, v = evaluate_sc32_resume(_resume_doc(gap_s=35.0, ship_ticks=live, presences=pres,
                                              obs_last=live[-1] + 40), 30, 20)
    results["sc32r_clean_despawn_observed"] = v
    obs["sc32r_observer_ticks_after_vanish"] = det["observer_ticks_after_ship_vanished"]

    results["sc32r_negative_control_gap_inside_linger"] = evaluate_sc32_resume(
        _resume_doc(gap_s=6.0, ship_ticks=live, presences=pres, obs_last=live[-1] + 40), 30, 20)[1]

    results["sc32r_no_lingering_seen"] = evaluate_sc32_resume(
        _resume_doc(gap_s=35.0, ship_ticks=live, presences=["ACTIVE"] * 10,
                    obs_last=live[-1] + 40), 30, 20)[1]

    results["sc32r_observer_closed_eyes_first"] = evaluate_sc32_resume(
        _resume_doc(gap_s=35.0, ship_ticks=live, presences=pres, obs_last=live[-1]), 30, 20)[1]

    gapped = live[:5] + [live[4] + 10] + [live[4] + 10 + 2 * i for i in range(1, 5)]
    det, v = evaluate_sc32_resume(_resume_doc(gap_s=35.0, ship_ticks=gapped, presences=pres,
                                              obs_last=gapped[-1] + 40), 30, 20)
    results["sc32r_negative_control_reappears"] = v
    obs["sc32r_reappearances_on_that_input"] = det["reappearances_after_absence"]

    expected = {
        "sc32r_clean_despawn_observed": PASS,
        "sc32r_negative_control_gap_inside_linger": NO_SAMPLES,
        "sc32r_no_lingering_seen": NO_SAMPLES,
        "sc32r_observer_closed_eyes_first": NO_SAMPLES,
        "sc32r_negative_control_reappears": FAIL,
        "sc28_clean": PASS,
        "sc28_negative_control_gap_of_6": FAIL,
        "sc28_single_snapshot": NO_SAMPLES,
        "sc28_bot_counter_disagrees": FAIL,
        "sc30_clean": PASS,
        "sc30_negative_control_own_ship_absent": FAIL,
        "sc32_no_despawn_is_not_pass": NO_SAMPLES,
        "sc32_clean_with_real_despawn": PASS,
        "sc32_negative_control_reappears_after_despawn": FAIL,
        "sc32_negative_control_unknown_presence": FAIL,
        "p1_no_summary": "rejected(EnvironmentProblem)",
        "p2_observer_set_mismatch": "rejected(NotImplementedYet)",
        "sc74_clean_with_real_outage": PASS,
        "sc74_negative_control_db_never_stopped": NO_EVIDENCE,
        "sc74_negative_control_no_snapshots_in_window": FAIL,
        "sc74_negative_control_server_initiated_close": FAIL,
        "sc74_no_sessions_json_is_not_pass": NO_EVIDENCE,
    }
    failures = {k: {"got": results.get(k), "expected": e}
                for k, e in expected.items() if results.get(k) != e}

    # 겨냥한 조건이 실제로 발생했다는 단언 — 음성 대조가 "0 건" 위에서 FAIL 을 낸 것이 아님을 보인다.
    occurred = {
        "sc28_violating_pairs_on_that_input": obs.get("sc28_violating_pairs_on_that_input"),
        "sc30_ticks_missing_on_that_input": obs.get("sc30_ticks_missing_on_that_input"),
        "sc32_reappearances_on_that_input": obs.get("sc32_reappearances_on_that_input"),
        "sc32_despawn_events_observed": obs.get("sc32_despawn_events_observed"),
        "sc32_despawn_events_on_no_despawn_input": obs.get("sc32_despawn_events_on_that_input"),
    }
    occurred_ok = (
        (occurred["sc28_violating_pairs_on_that_input"] or 0) > 0
        and (occurred["sc30_ticks_missing_on_that_input"] or 0) > 0
        and (occurred["sc32_reappearances_on_that_input"] or 0) > 0
        and (occurred["sc32_despawn_events_observed"] or 0) > 0
        and occurred["sc32_despawn_events_on_no_despawn_input"] == 0
        and (obs.get("sc32r_reappearances_on_that_input") or 0) > 0
        and (obs.get("sc32r_observer_ticks_after_vanish") or 0) > 0
        and (obs.get("sc74_server_closes_on_that_input") or 0) > 0
    )

    db.emit({
        "item": "block8_invariants selftest",
        "cases": results,
        "observations": obs,
        "expected_mismatches": failures,
        "targeted_condition_actually_occurred": occurred,
        "targeted_condition_ok": occurred_ok,
        "meaning": (
            "음성 대조가 **겨냥한 결함이 실제로 들어 있는 입력에서** FAIL 을 내야 한다 — "
            "`*_on_that_input` 이 0 이면 그 FAIL 은 아무것도 뜻하지 않는다. "
            "`sc32_no_despawn_is_not_pass` 는 반대 방향이다: **디스폰이 0 건인 입력에서 "
            "PASS 가 아니라 미검증이 나와야 한다** — 이 항목의 자명 통과가 바로 거기 있다."
        ),
    }, args.evidence)
    return db.EXIT_OK if (not failures and occurred_ok) else db.EXIT_FAIL


def main() -> int:
    ap = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)

    p = sub.add_parser("snapshots", help="SC-28 · SC-30 · SC-32")
    p.add_argument("--bot-out-dir", required=True)
    p.add_argument("--evidence")
    p.set_defaults(fn=cmd_snapshots)

    p = sub.add_parser("spawn", help="SC-10")
    p.add_argument("--system", default="cradle")
    p.add_argument("--tick-hz", type=int, default=20)
    p.add_argument("--since-tick", type=int, default=None,
                   help="이 tick 이후의 SHIP_SPAWNED 만 본다 — **절차서 §A-0 의 서버 기동 tick 을 준다.** "
                        "생략하면 테이블 전체(여러 실행이 섞여 점유 탐침이 FAIL 로 읽힌다)")
    p.add_argument("--evidence")
    p.set_defaults(fn=cmd_spawn)

    p = sub.add_parser("db-outage", help="SC-74")
    p.add_argument("--bot-out-dir", required=True)
    p.add_argument("--stats-before", required=True)
    p.add_argument("--stats-after", required=True)
    p.add_argument("--evidence")
    p.set_defaults(fn=cmd_db_outage)

    p = sub.add_parser("resume-despawn", help="SC-32 (디스폰 표본 — bots resume 경로)")
    p.add_argument("--resume-json", required=True)
    p.add_argument("--system", default="cradle")
    p.add_argument("--tick-hz", type=int, default=20)
    p.add_argument("--evidence")
    p.set_defaults(fn=cmd_resume_despawn)

    p = sub.add_parser("selftest")
    p.add_argument("--evidence")
    p.set_defaults(fn=cmd_selftest)

    args = ap.parse_args()
    return db.main_guard(lambda: args.fn(args))


if __name__ == "__main__":
    raise SystemExit(main())
