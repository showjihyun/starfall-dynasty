"""SC-70~73 — 스냅샷 손실·대역폭 (AC-18 b~e). **두 출처가 독립일 때만 검증이다**(I-25).

| 출처 | 무엇 |
|------|------|
| 봇 `summary.json` | `snapshots.snapshots_received`, `snapshot_bytes_received` — 소켓에서 **직접** 잰 값 |
| `/debug/stats` 델타 | `snapshots_sent_total`, `snapshot_bytes_total` — 서버가 **소켓에 쓴 뒤** 센 값(`ws.rs:278`) |

**무엇을 쟀는지 반드시 적는다**: 서버는 **WebSocket 페이로드 바이트**를 센다. 봇이 TCP 바이트를
재면 프레임 헤더가 더해진다(15 KiB 에서 4 B, 0.03 % 미만). 봇은 **페이로드 길이**를 센다.

    python tests/e2e/bandwidth.py --bots-out <dir> --stats-before <before.json> [--evidence <p>.json]
    python tests/e2e/bandwidth.py --selftest
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

import db
from poll_until import DEFAULT_STATS, fetch_stats
from three_way import metric

# 계약 §0.3 의 `미검증` 계열. FAIL 과 같은 코드로 내보내면 §0.3 이 나눈 두 칸이 셸에서 합쳐진다.
EXIT_EVIDENCE = 4

# ADR-0011 §2 산출 (fixture 실측 바이트 기반)
ADR_SHIPSTATE_B = 523
ADR_SNAPSHOT_31_B = 16_566
ADR_PER_SESSION_KIB_S = 161.8
ADR_TOTAL_MBIT_S = 42.5
EGRESS_BUDGET_KIB_S = 192  # data/movement/sync-tuning.json

# SC-71 — **모델 정확도 게이트이지 제품 게이트가 아니다**(architect R16 §3). 제품 예산은
# SC-72 가 따로 본다. 20 % 는 계약 SC-71 과 스펙 AC-18(c) 에 이미 있는 수이고 새 임계가 아니다.
SC71_MAX_GAP_PCT = 20.0
# 자명 통과 방어 (a) 의 표본 요건 (계약 §J / architect R16 §4).
SC71_MIN_SESSIONS = 31
SC71_MIN_DURATION_S = 60.0
# (c) 두 수가 독립이라는 주장의 근거. **파일:라인으로 적는다** — SC-63 (d) 와 같은 형태다.
SOURCE_ATTRIBUTION = {
    "bot_bytes": (
        "봇이 수신한 WebSocket 텍스트 프레임의 **페이로드 바이트 길이를 직접 합산**한다 "
        "(tools/bots/src/snapshot.rs — snapshot_bytes_received). 서버 카운터를 읽지 않는다."
    ),
    "server_bytes": (
        "서버가 `sink.send` 가 Ok 를 돌려준 **뒤에** 센다 "
        "(server .. ws.rs:278 `record_message_written`, server ack §1-②). "
        "큐에 넣은 시점이 아니므로 드롭·잘림이 양쪽에서 같이 사라지지 않는다."
    ),
    "why_it_matters": (
        "두 수가 같은 출처에서 나오면 대조가 항진명제다(I-25). 이 칸은 **주장이 아니라 "
        "파일:라인**이어야 한다 — 읽는 사람이 독립성을 직접 확인할 수 있어야 한다."
    ),
}


def judge_sc71(
    *, per_session_kib_s: float | None, adr_gap_pct: float | None,
    sessions: int, duration_s: float | None, bot_bytes: int, sent_bytes,
) -> tuple[str, dict]:
    """SC-71 **단독** verdict. 방어 (a)(b)(c) 를 먼저 보고, 그 다음 20 % 를 본다.

    (a) 가 지금까지 **실제로 열려 있었다**: `duration_s` 나 `sessions` 가 0 이면
    `per_session_kib_s` 가 `None` 이 되고 `adr_gap_pct` 도 `None` 이 되는데, 옛 코드는 그 값을
    출력에 싣기만 하고 verdict 는 다른 세 항목의 불리언에서 받았다 — **아무것도 안 흐른 실행이
    초록을 받는다.**
    """
    detail = {
        "gap_pct_vs_adr": round(adr_gap_pct, 2) if adr_gap_pct is not None else None,
        "max_gap_pct": SC71_MAX_GAP_PCT,
        "per_session_KiB_s": round(per_session_kib_s, 2) if per_session_kib_s else None,
        "adr_per_session_KiB_s": ADR_PER_SESSION_KIB_S,
        "judgment_inputs": {
            "sessions": sessions, "min_sessions": SC71_MIN_SESSIONS,
            "duration_s": duration_s, "min_duration_s": SC71_MIN_DURATION_S,
            "bot_bytes": bot_bytes, "server_bytes": sent_bytes,
        },
        "defense_c_source_attribution": SOURCE_ATTRIBUTION,   # (c)
        "what_this_measures": (
            "**설계 산출이 현실과 맞는가**(ADR-0011 §2 모델)이지 제품이 예산 안인가가 아니다 "
            "— 제품 게이트는 SC-72 다(architect R16 §3)."
        ),
        "remedy_if_over": (
            "FAIL 이면 구제가 정의돼 있다: ADR-0011 §2 의 표를 이번 실측으로 갱신하고 그 변경을 "
            "증거에 인용하면 PASS 로 재판정한다. **ADR 은 architect 소유다 — qa 는 수치와 함께 "
            "넘기고 직접 고치지 않는다.** 갱신 전까지는 비-통과이고 블록 8 을 막는다."
        ),
    }
    # (b) 아무것도 안 흐른 실행
    if not (isinstance(sent_bytes, int) and sent_bytes > 0 and bot_bytes > 0):
        detail["blocked_by"] = "(b) 양쪽 중 한쪽이 0 바이트 — 아무것도 흐르지 않았다"
        return "미검증(표본 없음)", detail
    # (a) 표본 요건
    if per_session_kib_s is None or adr_gap_pct is None:
        detail["blocked_by"] = "(a) per_session_KiB_s 가 None — duration_s 또는 sessions 가 0 이다"
        return "미검증(표본 없음)", detail
    if sessions < SC71_MIN_SESSIONS or (duration_s or 0) < SC71_MIN_DURATION_S:
        detail["blocked_by"] = (
            f"(a) 표본이 계약 창에 못 미친다 — sessions {sessions} < {SC71_MIN_SESSIONS} "
            f"또는 duration {duration_s} < {SC71_MIN_DURATION_S}"
        )
        return "미검증(표본 없음)", detail
    return ("PASS" if adr_gap_pct <= SC71_MAX_GAP_PCT else "FAIL"), detail


def compare(bot_recv: int, server_sent: int, label: str) -> dict:
    delta = server_sent - bot_recv
    pct = (abs(delta) / server_sent * 100.0) if server_sent else None
    return {
        "quantity": label,
        "bots": bot_recv,
        "server_metric": server_sent,
        "delta(server-bots)": delta,
        "delta_pct": round(pct, 3) if pct is not None else None,
    }


def run(args: argparse.Namespace) -> int:
    out = Path(args.bots_out)
    summary = json.loads((out / "summary.json").read_text(encoding="utf-8"))
    snap = summary.get("snapshots", {})
    bot_count = int(snap.get("snapshots_received", 0))
    bot_bytes = int(snap.get("snapshot_bytes_received", 0))
    sessions = int(summary.get("gates", {}).get("sessions_ready", 0))
    duration_s = float(summary.get("duration_secs", 0)) or None

    after = fetch_stats(args.stats)
    before = json.loads(Path(args.stats_before).read_text(encoding="utf-8")) if args.stats_before else {}

    def delta(key: str, sub: str | None = None):
        a, b = metric(after, key, sub), metric(before, key, sub) if before else None
        return a - b if isinstance(a, int) and isinstance(b, int) else a

    sent = delta("snapshots_sent_total")
    sent_bytes = delta("snapshot_bytes_total")
    written_label = delta("messages_written_total", "WORLD_SNAPSHOT")

    rows = [
        compare(bot_count, sent if isinstance(sent, int) else 0, "스냅샷 건수"),
        compare(bot_bytes, sent_bytes if isinstance(sent_bytes, int) else 0, "스냅샷 바이트"),
    ]

    # 세션당 대역폭 (봇 실측 기준)
    per_session_kib_s = None
    total_mbit_s = None
    if duration_s and sessions:
        per_session_kib_s = bot_bytes / sessions / duration_s / 1024.0
        total_mbit_s = sent_bytes * 8 / duration_s / 1e6 if isinstance(sent_bytes, int) else None

    adr_gap_pct = (
        abs(per_session_kib_s - ADR_PER_SESSION_KIB_S) / ADR_PER_SESSION_KIB_S * 100.0
        if per_session_kib_s
        else None
    )

    loss_ok = isinstance(sent, int) and sent == bot_count
    slow_consumer = args.slow_consumer_closes

    # **SC 하나 = verdict 하나** (§7b, architect R16 §5). 묶으면 식에서 빠진 항목이 다른 항목의
    # 초록불을 상속하고 **그 사실이 출력 어디에도 남지 않는다** — SC-71 이 정확히 그 상태였다.
    sc70 = "PASS" if loss_ok else "FAIL"
    if per_session_kib_s is None:
        # 옛 코드는 `budget_ok = per_session_kib_s is None or ...` 라 **표본이 없을 때 통과**했다.
        sc72 = "미검증(표본 없음)"
    else:
        sc72 = "PASS" if per_session_kib_s <= EGRESS_BUDGET_KIB_S else "FAIL"
    budget_ok = sc72 == "PASS"
    sc73 = "PASS" if slow_consumer == 0 else "FAIL"
    sc71, sc71_detail = judge_sc71(
        per_session_kib_s=per_session_kib_s, adr_gap_pct=adr_gap_pct,
        sessions=sessions, duration_s=duration_s, bot_bytes=bot_bytes, sent_bytes=sent_bytes,
    )
    verdicts = {"SC-70": sc70, "SC-71": sc71, "SC-72": sc72, "SC-73": sc73}

    db.emit(
        {
            "item": "SC-70·71·72·73 (AC-18 b~e) 스냅샷 손실·대역폭 — **항목별 독립 verdict**",
            "SC-70_verdict": sc70,
            "SC-71_verdict": sc71,
            "SC-72_verdict": sc72,
            "SC-73_verdict": sc73,
            "measured_what": "봇 = WebSocket 페이로드 길이 / 서버 = 소켓 기록 후 페이로드 바이트 (같은 것을 잰다)",
            "sessions": sessions,
            "duration_s": duration_s,
            "table": rows,
            "SC-70_snapshot_loss_zero": loss_ok,
            "cross_check_written_label": written_label,
            "SC-71_bandwidth": {
                **sc71_detail,
                "total_Mbit_s": round(total_mbit_s, 2) if total_mbit_s else None,
                "adr_total_Mbit_s": ADR_TOTAL_MBIT_S,
            },
            "SC-72_budget": {
                "egress_budget_KiB_s_per_session": EGRESS_BUDGET_KIB_S,
                "within_budget": budget_ok,
                "note": "제품 게이트. SC-71(모델 정확도)과 섞지 않는다 — 재는 양이 다르다",
            },
            "SC-73_queue": {
                "send_queue_bytes": after.get("send_queue_bytes"),
                "send_queue_bytes_max": after.get("send_queue_bytes_max"),
                "slow_consumer_closes": slow_consumer,
                "note": "정상 부하에서 SLOW_CONSUMER 는 0이어야 한다. 0이 아니면 용량이 아니라 소비자가 문제다",
            },
            "observed_ship_bytes_hint": {
                "adr_ShipState_B": ADR_SHIPSTATE_B,
                "adr_31ships_snapshot_B": ADR_SNAPSHOT_31_B,
                "measured_mean_snapshot_B": round(bot_bytes / bot_count) if bot_count else None,
                "max_ships_seen": snap.get("max_ships_seen"),
            },
        },
        args.evidence,
    )
    if "FAIL" in verdicts.values():
        return db.EXIT_FAIL
    if any(v.startswith("미검증") for v in verdicts.values()):
        return EXIT_EVIDENCE
    return db.EXIT_OK


def selftest(args: argparse.Namespace) -> int:
    """계측 산수가 맞는가 — 알려진 값으로 확인한다."""
    # 31 세션이 10초 동안 10 Hz 로 16,566 B 스냅샷을 받는다.
    sessions, duration, hz = 31, 10.0, 10
    bot_bytes = ADR_SNAPSHOT_31_B * hz * sessions * int(duration)
    per = bot_bytes / sessions / duration / 1024.0
    total = bot_bytes * 8 / duration / 1e6
    # 세션당 값은 ADR 과 정확히 같아야 한다. 합계는 ADR 이 COMMAND_RESULT 등을 포함해 조금 크다.
    arith_ok = abs(per - ADR_PER_SESSION_KIB_S) < 0.5 and 38.0 <= total <= ADR_TOTAL_MBIT_S + 0.5

    # ── SC-71 판정과 방어 (a)(b) 의 대조 (architect R16 §6 B-5).
    # **시험하지 않은 방어는 방어가 아니다.** 각 방어가 걸려야 할 입력에서 실제로 걸리는가,
    # 그리고 20 % 가 **실제로 판정 식에 들어갔는가**(19 / 21 양쪽)를 같은 실행에서 보인다.
    def _case(gap_pct=None, *, sessions=31, duration=60.0, bot=1, srv=1):
        return judge_sc71(
            per_session_kib_s=(
                None if gap_pct is None
                else ADR_PER_SESSION_KIB_S * (1.0 + gap_pct / 100.0)
            ),
            adr_gap_pct=gap_pct, sessions=sessions, duration_s=duration,
            bot_bytes=bot, sent_bytes=srv,
        )[0]

    controls = {
        "gap_19pct": _case(19.0),
        "gap_21pct": _case(21.0),
        "gap_exactly_20pct": _case(20.0),
        "duration_zero": _case(None, duration=0.0),
        "sessions_below_31": _case(5.0, sessions=30),
        "duration_below_60": _case(5.0, duration=59.0),
        "zero_bot_bytes": _case(5.0, bot=0),
        "zero_server_bytes": _case(5.0, srv=0),
        "source_attribution_present": sorted(SOURCE_ATTRIBUTION),
    }
    controls_ok = (
        controls["gap_19pct"] == "PASS"
        and controls["gap_21pct"] == "FAIL"          # 20 % 가 실제로 판정 식에 있다
        and controls["gap_exactly_20pct"] == "PASS"  # 경계는 포함(계약 "넘으면")
        and controls["duration_zero"] == "미검증(표본 없음)"
        and controls["sessions_below_31"] == "미검증(표본 없음)"
        and controls["duration_below_60"] == "미검증(표본 없음)"
        and controls["zero_bot_bytes"] == "미검증(표본 없음)"
        and controls["zero_server_bytes"] == "미검증(표본 없음)"
    )
    ok = arith_ok and controls_ok
    db.emit(
        {
            "item": "bandwidth 자체 검증",
            "verdict": "PASS" if ok else "FAIL",
            "arithmetic_ok": arith_ok,
            "sc71_controls": controls,
            "sc71_controls_ok": controls_ok,
            "sc71_controls_meaning": (
                "19 % PASS 와 21 % FAIL 이 같이 나와야 **20 % 가 판정 식에 실제로 들어갔다**는 "
                "뜻이다. 옛 코드에서 SC-71 은 식에 없었고 다른 세 항목의 초록불을 상속했다 — "
                "그 상태에서는 어떤 gap 을 넣어도 verdict 가 같았다. 방어 (a)(b) 는 "
                "**아무것도 안 흐른 실행**과 **창이 못 미친 실행**이 PASS 를 못 받게 한다."
            ),
            "synthetic": "31세션 × 10 Hz × 10초 × 16,566 B",
            "per_session_KiB_s": round(per, 2),
            "adr_per_session_KiB_s": ADR_PER_SESSION_KIB_S,
            "total_Mbit_s": round(total, 2),
            "adr_total_Mbit_s": ADR_TOTAL_MBIT_S,
            "meaning": "ADR-0011 §2 의 산출을 같은 입력으로 재현한다 — 산수가 어긋나면 실측 해석도 어긋난다",
        },
        args.evidence,
    )
    return db.EXIT_OK if ok else db.EXIT_FAIL


def main() -> int:
    ap = argparse.ArgumentParser(description="스냅샷 손실·대역폭 대조")
    ap.add_argument("--bots-out")
    ap.add_argument("--stats", default=DEFAULT_STATS)
    ap.add_argument("--stats-before")
    ap.add_argument("--slow-consumer-closes", type=int, default=0)
    ap.add_argument("--selftest", action="store_true")
    ap.add_argument("--evidence")
    args = ap.parse_args()
    if args.selftest:
        return db.main_guard(lambda: selftest(args))
    if not args.bots_out:
        ap.error("--bots-out 또는 --selftest 가 필요하다")
    return db.main_guard(lambda: run(args))


if __name__ == "__main__":
    sys.exit(main())
