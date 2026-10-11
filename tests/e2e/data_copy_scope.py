#!/usr/bin/env python
"""p1-03 SC-57 (AC-13 b, I-91, ADR-0015) — 클라이언트 데이터 사본에 서버 전용 종류의 파일이 0 인가.

클라이언트 EditMode 테스트(SC-56)와 **독립된 구현**이다 — 같은 규칙을 다른 코드로 한 번 더 잰다.

    기대 집합 = 레지스트리에서 `kind == data` 이고 `consumers` 에 `client` 가 있는 종류의
                `data_files` glob 이 `data/` 에서 잡는 파일 (data 기준 상대 경로)
    서버 전용 = `consumers` 에 `client` 가 없는 data 종류의 glob 이 잡는 경로
    사본     = `client/Assets/_Project/Data/**/*.json`

판정: 사본에 서버 전용 종류의 파일 0 · 사본 전용(기대 밖) 0 · 누락 0 · 바이트 동일 — 그리고 기대 집합
크기 > 0, 사본 파일 수 > 0 (빈 집합 배제). 만기 "서버 전용 필드 분모 2 → 0" 의 실행 증거
(`world/deposits/cradle.json` 이 v2 이고 `mineral_id`·`initial_reserve_kg` 키가 사본에 0 개)도 같이 센다.

    python tests/e2e/data_copy_scope.py [--copy <사본 디렉터리>] [--evidence <json>]
    python tests/e2e/data_copy_scope.py --selftest

종료 코드: 0 = PASS, 1 = FAIL, 3 = 레지스트리/디렉터리 없음.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import sys
import tempfile
from pathlib import Path

for _s in (sys.stdout, sys.stderr):
    try:
        _s.reconfigure(encoding="utf-8")
    except (AttributeError, ValueError):
        pass

REPO = Path(__file__).resolve().parents[2]
COPY_REL = "client/Assets/_Project/Data"
SERVER_ONLY_KEYS = ("mineral_id", "initial_reserve_kg")  # p1-02 만기 표의 분모 2 (광맥 표 서버 전용 필드)


def kinds(root: Path) -> list[dict]:
    reg = json.loads((root / "contracts/registry/types.json").read_text(encoding="utf-8"))
    return [t for t in reg["types"] if t.get("kind") == "data"]


def files_of(data_root: Path, globs: list[str]) -> set[str]:
    out: set[str] = set()
    for g in globs:
        out |= {p.relative_to(data_root).as_posix() for p in data_root.glob(g) if p.is_file()}
    return out


def sha(p: Path) -> str:
    return hashlib.sha256(p.read_bytes()).hexdigest()


def count_keys(doc, names) -> int:
    if isinstance(doc, dict):
        return sum(1 for k in doc if k in names) + sum(count_keys(v, names) for v in doc.values())
    if isinstance(doc, list):
        return sum(count_keys(v, names) for v in doc)
    return 0


def judge(root: Path, copy_dir: Path) -> dict:
    data_root = root / "data"
    ks = kinds(root)
    client_kinds = [k for k in ks if "client" in (k.get("consumers") or [])]
    server_only_kinds = [k for k in ks if "client" not in (k.get("consumers") or [])]
    expected = set()
    for k in client_kinds:
        expected |= files_of(data_root, k.get("data_files") or [])
    copy = {p.relative_to(copy_dir).as_posix() for p in copy_dir.rglob("*.json")} if copy_dir.is_dir() else set()
    # 서버 전용: 사본 쪽 경로가 서버 전용 glob 에 걸리는가(원본에 없는 이름이어도 잡는다)
    server_only_in_copy = sorted(files_of(copy_dir, [g for k in server_only_kinds for g in (k.get("data_files") or [])])) \
        if copy_dir.is_dir() else []
    copy_only = sorted(copy - expected)
    missing = sorted(expected - copy)
    differ = sorted(f for f in expected & copy if sha(data_root / f) != sha(copy_dir / f))
    leaked_keys = 0
    for f in sorted(copy):
        if f.startswith("world/deposits/"):
            try:
                leaked_keys += count_keys(json.loads((copy_dir / f).read_text(encoding="utf-8")), SERVER_ONLY_KEYS)
            except json.JSONDecodeError:
                leaked_keys += 1
    ok = (len(expected) > 0 and len(copy) > 0 and not server_only_in_copy and not copy_only
          and not missing and not differ and leaked_keys == 0)
    return {
        "item": "p1-03 SC-57 (AC-13b) — 클라이언트 사본에 서버 전용 종류 파일 0 (레지스트리 유도, 독립 구현)",
        "verdict": "PASS" if ok else "FAIL",
        "copy_dir": copy_dir.as_posix(),
        "kinds_total": len(ks),
        "client_kinds": sorted(k["name"] for k in client_kinds),
        "server_only_kinds": sorted(k["name"] for k in server_only_kinds),
        "expected_files": len(expected),
        "copy_files": len(copy),
        "server_only_in_copy": server_only_in_copy,
        "copy_only": copy_only,
        "missing": missing,
        "bytes_differ": differ,
        "server_only_keys_in_copied_deposit_table": leaked_keys,
        "note": "기대 0 또는 사본 0 이면 FAIL(빈 집합 배제)",
    }


def selftest(root: Path) -> int:
    checks = []
    with tempfile.TemporaryDirectory() as td:
        good = Path(td) / "good"
        # 레지스트리에서 유도한 올바른 사본을 만든다
        ks = kinds(root)
        exp = set()
        for k in ks:
            if "client" in (k.get("consumers") or []):
                exp |= files_of(root / "data", k.get("data_files") or [])
        for f in exp:
            (good / f).parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(root / "data" / f, good / f)
        r0 = judge(root, good)
        checks.append(("양성: 레지스트리대로 만든 사본은 PASS", r0["verdict"] == "PASS"))

        bad = Path(td) / "bad"
        shutil.copytree(good, bad)
        (bad / "world/deposit-reserves").mkdir(parents=True, exist_ok=True)
        shutil.copyfile(root / "data/world/deposit-reserves/cradle.json", bad / "world/deposit-reserves/cradle.json")
        r1 = judge(root, bad)
        checks.append(("음성: 사본에 예비 파일 → FAIL(server_only_in_copy 1)",
                       r1["verdict"] == "FAIL" and r1["server_only_in_copy"] == ["world/deposit-reserves/cradle.json"]))

        bad2 = Path(td) / "bad2"
        shutil.copytree(good, bad2)
        (bad2 / "history/rules").mkdir(parents=True, exist_ok=True)
        shutil.copyfile(root / "data/history/rules/mineral-discovery.json", bad2 / "history/rules/mineral-discovery.json")
        r2 = judge(root, bad2)
        checks.append(("음성: 사본에 역사 규칙 → FAIL", r2["verdict"] == "FAIL" and len(r2["server_only_in_copy"]) == 1))

        bad3 = Path(td) / "bad3"
        shutil.copytree(good, bad3)
        p = bad3 / "world/deposits/cradle.json"
        doc = json.loads(p.read_text(encoding="utf-8"))
        doc["deposits"][0]["mineral_id"] = "ferrosite"
        p.write_text(json.dumps(doc), encoding="utf-8")
        r3 = judge(root, bad3)
        checks.append(("음성: 공개 광맥 표에 mineral_id 되돌림 → FAIL(키 1)",
                       r3["verdict"] == "FAIL" and r3["server_only_keys_in_copied_deposit_table"] == 1))

        r4 = judge(root, Path(td) / "nothing")
        checks.append(("음성: 사본 디렉터리 없음 → FAIL(0 = 0 아님)", r4["verdict"] == "FAIL"))
    bad_n = 0
    for name, ok in checks:
        print(("ok   " if ok else "FAIL ") + name)
        bad_n += 0 if ok else 1
    print(f"selftest {len(checks) - bad_n}/{len(checks)}")
    return 0 if bad_n == 0 else 1


def main() -> int:
    ap = argparse.ArgumentParser(description="p1-03 SC-57 사본 범위")
    ap.add_argument("--root", default=str(REPO))
    ap.add_argument("--copy")
    ap.add_argument("--evidence")
    ap.add_argument("--selftest", action="store_true")
    a = ap.parse_args()
    root = Path(a.root).resolve()
    if not (root / "contracts/registry/types.json").is_file():
        print("FAIL(구현 없음): 레지스트리 없음", file=sys.stderr)
        return 3
    if a.selftest:
        return selftest(root)
    res = judge(root, Path(a.copy) if a.copy else root / COPY_REL)
    text = json.dumps(res, indent=2, ensure_ascii=False)
    print(text)
    if a.evidence:
        Path(a.evidence).write_text(text + "\n", encoding="utf-8")
    return 0 if res["verdict"] == "PASS" else 1


if __name__ == "__main__":
    sys.exit(main())
