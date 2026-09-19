"""31번째 연결(Unity)의 증거를 로그에서 뽑는다 — §0.6 의 집합에 합칠 `correlation_id` 포함.

client 가 `02_client_ack.md` §1 에서 **문구와 필드 순서를 고정**했다. 이 스크립트는 그 문구만
읽는다. 문구가 바뀌면 여기서 바로 깨지는 것이 맞다(조용히 0건을 반환하지 않는다).

    starfall.net: SESSION_READY session_id=<uuid> correlation_id=<uuid> actor_id=<uuid> \
world_id=<uuid> tick_hz=<int> server_version=<s> attempt=<int>
    starfall.net: dropping N in-flight command(s) on disconnect (no resend, I-23)
    starfall.net: reconnect attempt=<n> delay_ms=<ms> (counter resets only on SESSION_READY)
    starfall.net: closing session_id=<uuid> reason=<CLIENT_CLOSED|EDITOR_RELOAD|PLAYMODE_EXIT|APP_QUIT> code=1000

사용:

    # A 단계 직전 — 31번째 세션의 correlation 을 뽑아 봇 집합에 합칠 파일로
    python tests/e2e/unity_corr.py --out _workspace/p0-02-networking-spike/evidence/unity-corr.txt

    # SC-49/50/51 증거 — 세션·재연결·종료 사유를 한 번에
    python tests/e2e/unity_corr.py --evidence <path>.json
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

import db

DEFAULT_LOG = db.REPO_ROOT / "client/Logs/starfall-net.log"
FALLBACK_LOG = db.REPO_ROOT / "client/Logs/Editor.log"

UUID = r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}"

READY_RE = re.compile(
    r"starfall\.net: SESSION_READY "
    rf"session_id=(?P<session_id>{UUID}) "
    rf"correlation_id=(?P<correlation_id>{UUID}) "
    rf"actor_id=(?P<actor_id>{UUID}) "
    rf"world_id=(?P<world_id>{UUID}) "
    r"tick_hz=(?P<tick_hz>\d+) "
    r"server_version=(?P<server_version>\S+) "
    r"attempt=(?P<attempt>\d+)"
)
DROP_RE = re.compile(
    r"starfall\.net: dropping (?P<n>\d+) in-flight command\(s\) on disconnect \(no resend, I-23\)"
)
RECONNECT_RE = re.compile(
    r"starfall\.net: reconnect attempt=(?P<attempt>\d+) delay_ms=(?P<delay_ms>\d+) "
    r"\(counter resets only on SESSION_READY\)"
)
CLOSING_RE = re.compile(
    rf"starfall\.net: closing session_id=(?P<session_id>{UUID}) "
    r"reason=(?P<reason>[A-Z_]+) code=(?P<code>\d+)"
)


def read_log(path: Path) -> list[str]:
    if not path.is_file():
        raise db.EnvironmentProblem(
            f"로그가 없다: {path}. Unity Editor 에서 PlayMode 접속을 한 번 해야 생긴다 "
            "(03_client_impl.md §1.4)."
        )
    return path.read_text(encoding="utf-8", errors="replace").splitlines()


def run(args: argparse.Namespace) -> int:
    path = Path(args.log) if args.log else DEFAULT_LOG
    if not path.is_file() and FALLBACK_LOG.is_file():
        path = FALLBACK_LOG
    lines = read_log(path)

    ready = [m.groupdict() for m in (READY_RE.search(x) for x in lines) if m]
    drops = [m.groupdict() for m in (DROP_RE.search(x) for x in lines) if m]
    reconnects = [m.groupdict() for m in (RECONNECT_RE.search(x) for x in lines) if m]
    closings = [m.groupdict() for m in (CLOSING_RE.search(x) for x in lines) if m]

    if not ready:
        raise db.NotImplementedYet(
            f"{path} 에 `starfall.net: SESSION_READY …` 줄이 없다. "
            "PlayMode 접속이 일어나지 않았거나 문구가 바뀌었다(02_client_ack.md §1 과 대조)."
        )

    # 최근 것이 마지막에 온다. 중복 correlation 은 한 번만 쓴다.
    correlations, seen = [], set()
    for r in ready:
        c = r["correlation_id"]
        if c not in seen:
            seen.add(c)
            correlations.append(c)

    if args.out:
        out = Path(args.out)
        out.parent.mkdir(parents=True, exist_ok=True)
        take = correlations[-args.last:] if args.last else correlations
        out.write_text("\n".join(take) + "\n", encoding="utf-8")

    tick_hz_values = sorted({int(r["tick_hz"]) for r in ready})
    actors = sorted({r["actor_id"] for r in ready})
    result = {
        "item": "SC-49/50/51 · §0.6 — Unity 세션 로그 추출",
        "log": str(path),
        "session_ready_lines": len(ready),
        "distinct_correlations": len(correlations),
        "correlations": correlations,
        "actor_ids": actors,
        "tick_hz_values": tick_hz_values,
        "tick_hz_is_20": tick_hz_values == [20],
        "attempts": [int(r["attempt"]) for r in ready],
        "sessions": ready[-5:],
        "drop_lines": drops,
        "reconnect_lines": reconnects,
        "closing_lines": closings,
        "closing_reasons": sorted({c["reason"] for c in closings}),
        "notes": [
            "SC-51: attempt 가 SESSION_READY 마다 0 으로 돌아오는지 본다"
            " (카운터는 SESSION_READY 에서만 리셋된다 — ADR-0005 §5).",
            "SC-50: closing reason 이 EDITOR_RELOAD / PLAYMODE_EXIT 이면 DB 의 close_reason 이"
            " CLIENT_CLOSED 가 아닐 수 있다. 그때는 FAIL 이 아니라 재측정이다(도메인 리로드 전제).",
            "A 단계 중에는 client/ 아래 파일을 저장하지 않는다 — 리로드가 31번째 연결을 끊는다.",
        ],
    }
    db.emit(result, args.evidence)
    if args.expect_disjoint_from:
        bots = db.read_correlations(args.expect_disjoint_from)
        overlap = [c for c in correlations if c in set(bots)]
        if overlap:
            print(f"FAIL: 봇 집합과 겹치는 correlation: {overlap}", file=sys.stderr)
            return db.EXIT_FAIL
    return db.EXIT_OK


def main() -> int:
    ap = argparse.ArgumentParser(description="Unity 로그에서 세션 증거 추출")
    ap.add_argument("--log", help=f"기본: {DEFAULT_LOG}")
    ap.add_argument("--out", help="correlation 만 한 줄씩 쓸 파일 (§0.6 합치기용)")
    ap.add_argument("--last", type=int, default=1,
                    help="--out 에 최근 N 개만 쓴다 (기본 1 = 지금 붙어 있는 세션)")
    ap.add_argument("--expect-disjoint-from", help="봇 correlations.txt — 겹치면 FAIL")
    ap.add_argument("--evidence")
    args = ap.parse_args()
    return db.main_guard(lambda: run(args))


if __name__ == "__main__":
    sys.exit(main())
