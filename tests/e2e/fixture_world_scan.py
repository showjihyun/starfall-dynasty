#!/usr/bin/env python
"""SC-84 (AC-21 c) — fixture 의 `world_id` <-> `tick_hz` 조합이 I-19 와 모순되지 않는가.

I-19: `occurred_at` 은 `tick` 에서만 파생되고, 파생 상수(`tick_hz`·`calendar_epoch`·
`calendar_scale`)는 `worlds` 행에 묶여 **그 월드의 수명 동안 불변**이다. 따라서
**같은 `world_id` 를 쓰는 계약 fixture 는 같은 `tick_hz` 를 써야 한다.**

DB 를 보지 않는다 — `contracts/fixtures/` 전수 스캔이다(서버가 꺼져 있어도 돈다).
`world_id` 와 `tick_hz` 는 fixture 안 어디에 있어도 찾는다(envelope·payload·중첩 무관).

종료 코드: 0 = 위반 0, 1 = 위반 있음, 3 = fixture 디렉토리 없음.
"""
from __future__ import annotations

import argparse
import json
import sys
from collections import defaultdict
from pathlib import Path

# Windows 콘솔 기본 코드 페이지(cp949)에서 한글·em dash 가 깨진다 (db.py 와 같은 처리).
for _stream in (sys.stdout, sys.stderr):
    try:
        _stream.reconfigure(encoding="utf-8")  # type: ignore[union-attr]
    except (AttributeError, ValueError):
        pass


def walk(node, out: dict[str, set]):
    """fixture 안의 모든 `world_id` / `tick_hz` 를 깊이 무관하게 모은다."""
    if isinstance(node, dict):
        for k, v in node.items():
            if k in ("world_id", "tick_hz") and not isinstance(v, (dict, list)):
                out[k].add(v)
            walk(v, out)
    elif isinstance(node, list):
        for v in node:
            walk(v, out)


def main() -> int:
    ap = argparse.ArgumentParser(description="SC-84 fixture world_id<->tick_hz 스캔 (I-19)")
    ap.add_argument("--root", default=".", help="레포 루트")
    ap.add_argument("--evidence", help="JSON 을 이 경로에도 쓴다")
    args = ap.parse_args()

    root = Path(args.root).resolve()
    fixtures = root / "contracts" / "fixtures"
    if not fixtures.is_dir():
        print(f"FAIL(구현 없음): {fixtures} 가 없다", file=sys.stderr)
        return 3

    # 유효 fixture 만 본다. 반례(invalid/)는 일부러 망가뜨린 것이라 I-19 의 대상이 아니다.
    files = sorted(p for p in fixtures.rglob("*.json") if "invalid" not in p.parts)

    by_world: dict[str, dict[int, list[str]]] = defaultdict(lambda: defaultdict(list))
    scanned, with_world, with_tick_hz = 0, 0, 0
    multi_valued: list[str] = []

    for f in files:
        scanned += 1
        found: dict[str, set] = defaultdict(set)
        walk(json.loads(f.read_text(encoding="utf-8")), found)
        rel = f.relative_to(fixtures).as_posix()
        worlds, hzs = found.get("world_id", set()), found.get("tick_hz", set())
        if len(worlds) > 1 or len(hzs) > 1:
            multi_valued.append(rel)  # 한 파일 안에서 갈라지면 그 자체가 위반 후보
        if worlds:
            with_world += 1
        if hzs:
            with_tick_hz += 1
        for w in worlds:
            for hz in hzs:
                by_world[str(w)][hz].append(rel)

    violations = []
    for world, hz_map in sorted(by_world.items()):
        if len(hz_map) > 1:
            violations.append({"world_id": world,
                               "tick_hz_values": {str(hz): sorted(f) for hz, f in sorted(hz_map.items())}})

    result = {
        "item": "SC-84 / AC-21(c) — fixture world_id<->tick_hz (I-19)",
        "verdict": "PASS" if not violations and not multi_valued else "FAIL",
        "fixtures_scanned": scanned,
        "fixtures_with_world_id": with_world,
        "fixtures_with_tick_hz": with_tick_hz,
        "pairs_observed": sum(len(h) for h in by_world.values()),
        "world_ids": {w: {"tick_hz": sorted(h.keys()), "fixtures": sum(len(v) for v in h.values())}
                      for w, h in sorted(by_world.items())},
        "files_with_conflicting_values_inside_one_fixture": multi_valued,
        "violations": violations,
    }
    text = json.dumps(result, indent=2, ensure_ascii=False)
    print(text)
    if args.evidence:
        Path(args.evidence).write_text(text + "\n", encoding="utf-8")
    return 0 if result["verdict"] == "PASS" else 1


if __name__ == "__main__":
    sys.exit(main())
