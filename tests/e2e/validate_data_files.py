#!/usr/bin/env python
"""p1-03 SC-59 (AC-13 d) · SC-58 (AC-13 c) — `data/` 실제 파일 전부가 계약 스키마를 통과하는가,
그리고 `data/**/*.json` 각각이 정확히 하나의 data 종류에 걸리는가.

**대상은 레지스트리에서 유도한다**(스펙 §4.7, ADR-0015). `contracts/registry/types.json` 에서
`kind == "data"` 인 종류마다 `schema`(contracts 기준)와 `data_files`(data 기준 glob) 를 읽는다.
p1-02 까지는 이 파일 안에 손으로 적은 `TARGETS` 목록이 있었다 — 레지스트리와 다른 두 번째 목록은
관측을 앞질러 낡는다(시장·예비 파일이 생겨도 아무것도 빨개지지 않았다).

    python tests/e2e/validate_data_files.py                 # SC-59: 스키마 검증 (12 파일 기대)
    python tests/e2e/validate_data_files.py --glob-census   # SC-58: 파일마다 걸리는 종류 수 = 1
    python tests/e2e/validate_data_files.py --selftest      # 음성 대조 (임시 사본)

**왜 독립으로 재검사하는가.** 서버 기동 경로(AC-2)도 같은 것을 보지만 그건 서버가 자기 자신을
확인하는 것이다 — 이 스크립트는 서버 밖에서 스키마만 본다.

종료 코드: 0 = 오류 0, 1 = 검증 실패, 2 = 환경(jsonschema 없음), 3 = 레지스트리/데이터 디렉터리 없음.
"""
from __future__ import annotations

import argparse
import json
import shutil
import sys
import tempfile
from pathlib import Path

for _stream in (sys.stdout, sys.stderr):
    try:
        _stream.reconfigure(encoding="utf-8")  # type: ignore[union-attr]
    except (AttributeError, ValueError):
        pass

# 계약 SC-59 가 적은 파일 수(2026-10-05). **다르면 FAIL 이 아니라 architect·designer 통지** —
# designer 의 정상적인 추가를 막지 않는다(p1-02 SC-06 선례). 출력에만 찍는다.
EXPECTED_FILES_AT_CONTRACT = 12


def data_kinds(root: Path) -> list[dict]:
    """레지스트리의 data 종류 → [{name, schema(레포 기준), globs(레포 기준), consumers}]."""
    reg = json.loads((root / "contracts/registry/types.json").read_text(encoding="utf-8"))
    out = []
    for t in reg["types"]:
        if t.get("kind") != "data":
            continue
        out.append({
            "name": t["name"],
            "schema": "contracts/" + t["schema"],
            "globs": ["data/" + g for g in t.get("data_files") or []],
            "consumers": list(t.get("consumers") or []),
        })
    return out


def build_registry(contracts: Path):
    """스키마 전부를 $id 로 등록한다 — 상대 $ref 가 파일 경계를 넘어 풀리게."""
    import referencing

    registry = referencing.Registry()
    for path in sorted(contracts.rglob("*.schema.json")):
        doc = json.loads(path.read_text(encoding="utf-8"))
        resource = referencing.Resource.from_contents(doc)
        registry = registry.with_resource(uri=doc["$id"], resource=resource)
        registry = registry.with_resource(uri=path.relative_to(contracts).as_posix(), resource=resource)
    return registry


def count_fields(doc) -> int:
    """문서 안의 필드(키) 수 — 검증기가 파일을 열고 안을 걸었다는 하한(0 파일 검증 → 오류 0 배제)."""
    if isinstance(doc, dict):
        return len(doc) + sum(count_fields(v) for v in doc.values())
    if isinstance(doc, list):
        return sum(count_fields(v) for v in doc)
    return 0


def validate(root: Path) -> dict:
    from jsonschema import Draft202012Validator

    kinds = data_kinds(root)
    registry = build_registry(root / "contracts")
    rows, errors_total, files_checked, fields_total = [], 0, 0, 0
    if not kinds:
        errors_total += 1
        rows.append({"detail": ["레지스트리에 data 종류가 0 — 표 파싱 사망"]})
    for k in kinds:
        schema = json.loads((root / k["schema"]).read_text(encoding="utf-8"))
        validator = Draft202012Validator(schema, registry=registry)
        if not k["globs"]:
            rows.append({"kind": k["name"], "errors": 1, "detail": ["data_files 가 비어 있다"]})
            errors_total += 1
            continue
        for glob in k["globs"]:
            matched = sorted(root.glob(glob))
            if not matched:
                rows.append({"kind": k["name"], "glob": glob, "files": 0, "errors": 1,
                             "detail": ["대상 파일이 하나도 없다"]})
                errors_total += 1
                continue
            for f in matched:
                files_checked += 1
                try:
                    doc = json.loads(f.read_text(encoding="utf-8"))
                except json.JSONDecodeError as e:
                    rows.append({"file": f.relative_to(root).as_posix(), "errors": 1, "detail": [f"JSON: {e}"]})
                    errors_total += 1
                    continue
                fields = count_fields(doc)
                fields_total += fields
                errs = sorted(validator.iter_errors(doc), key=lambda e: list(e.absolute_path))
                errors_total += len(errs)
                rows.append({
                    "file": f.relative_to(root).as_posix(), "kind": k["name"], "schema": k["schema"],
                    "fields_checked": fields, "errors": len(errs),
                    "detail": [f"/{'/'.join(str(p) for p in e.absolute_path)}: {e.message}" for e in errs],
                })
    return {
        "item": "p1-03 SC-59 (AC-13d) — data/ 실제 파일 전부의 계약 스키마 검증, 대상은 레지스트리 data_files 유도",
        "verdict": "PASS" if errors_total == 0 and files_checked > 0 else "FAIL",
        "kinds": len(kinds),
        "globs": sum(len(k["globs"]) for k in kinds),
        "files_checked": files_checked,
        "files_expected_at_contract": EXPECTED_FILES_AT_CONTRACT,
        "files_differ_from_contract": files_checked != EXPECTED_FILES_AT_CONTRACT,
        "fields_checked": fields_total,
        "errors_total": errors_total,
        "rows": rows,
    }


def glob_census(root: Path) -> dict:
    """SC-58: data/**/*.json 각각이 걸리는 data 종류 glob 수. 0 개·2 개 이상 = FAIL."""
    kinds = data_kinds(root)
    hits: dict[str, list[str]] = {}
    for k in kinds:
        for glob in k["globs"]:
            for f in root.glob(glob):
                hits.setdefault(f.relative_to(root).as_posix(), []).append(k["name"])
    files = sorted(p.relative_to(root).as_posix() for p in (root / "data").rglob("*.json"))
    rows = [{"file": f, "kinds": sorted(hits.get(f, []))} for f in files]
    zero = [r["file"] for r in rows if not r["kinds"]]
    multi = [r for r in rows if len(r["kinds"]) > 1]
    ok = bool(files) and not zero and not multi
    return {
        "item": "p1-03 SC-58 (AC-13c) — data/**/*.json 각각이 정확히 하나의 data 종류 glob 에 걸린다",
        "verdict": "PASS" if ok else "FAIL",
        "files": len(files),
        "kinds": len(kinds),
        "zero_match": zero,
        "multi_match": multi,
        "note": "파일 0 이면 FAIL(빈 집합의 전칭명제)" if not files else "",
        "rows": rows,
    }


def _copy_tree(root: Path, dst: Path) -> None:
    shutil.copytree(root / "contracts", dst / "contracts")
    shutil.copytree(root / "data", dst / "data")


def selftest(root: Path) -> int:
    """규칙 6 — 방어가 걸려야 할 입력에서 실제로 걸리는가. 임시 사본만 바꾼다(원본 data/ 불변)."""
    checks = []
    with tempfile.TemporaryDirectory() as td:
        base = Path(td) / "base"
        _copy_tree(root, base)
        v0, c0 = validate(base), glob_census(base)
        checks.append(("양성: 무변경 사본은 스키마 오류 0", v0["errors_total"] == 0 and v0["files_checked"] > 0))
        checks.append(("양성: 무변경 사본은 glob census 통과", c0["verdict"] == "PASS"))
        checks.append(("유도: 시장·예비 파일이 대상에 있다",
                       {"data/market/cradle-exchange.json", "data/world/deposit-reserves/cradle.json"}
                       <= {r.get("file") for r in v0["rows"]}))

        # 음성 1: 시장 기준가를 소수로 → 정확히 1 오류(AC-13(d) 음성 대조)
        neg = Path(td) / "neg1"
        _copy_tree(root, neg)
        mp = neg / "data/market/cradle-exchange.json"
        doc = json.loads(mp.read_text(encoding="utf-8"))
        doc["listings"][0]["base_price_cr_per_kg"] = 10.5
        mp.write_text(json.dumps(doc), encoding="utf-8")
        v1 = validate(neg)
        checks.append(("음성: 시장 기준가 소수 → 1 오류", v1["errors_total"] == 1))

        # 음성 2: 아무 glob 에도 안 걸리는 파일 → census FAIL
        neg2 = Path(td) / "neg2"
        _copy_tree(root, neg2)
        (neg2 / "data/unused").mkdir()
        (neg2 / "data/unused/x.json").write_text("{}", encoding="utf-8")
        c2 = glob_census(neg2)
        checks.append(("음성: 걸리는 종류 0 개 파일 → FAIL", c2["verdict"] == "FAIL" and c2["zero_match"] == ["data/unused/x.json"]))

        # 음성 3: 두 종류에 걸리는 파일 → census FAIL (레지스트리에 겹치는 glob 하나 추가)
        neg3 = Path(td) / "neg3"
        _copy_tree(root, neg3)
        rp = neg3 / "contracts/registry/types.json"
        reg = json.loads(rp.read_text(encoding="utf-8"))
        for t in reg["types"]:
            if t["name"] == "MINERAL":
                t["data_files"] = list(t["data_files"]) + ["market/*.json"]
        rp.write_text(json.dumps(reg), encoding="utf-8")
        c3 = glob_census(neg3)
        checks.append(("음성: 두 종류에 걸리는 파일 → FAIL", c3["verdict"] == "FAIL" and len(c3["multi_match"]) == 1))

        # 음성 4: 레지스트리 data_files 를 비우면 검증도 FAIL (유도가 실제로 레지스트리를 읽는다)
        neg4 = Path(td) / "neg4"
        _copy_tree(root, neg4)
        rp = neg4 / "contracts/registry/types.json"
        reg = json.loads(rp.read_text(encoding="utf-8"))
        for t in reg["types"]:
            if t["name"] == "MARKET":
                t["data_files"] = ["market/nothing-*.json"]
        rp.write_text(json.dumps(reg), encoding="utf-8")
        v4 = validate(neg4)
        checks.append(("음성: 레지스트리 glob 이 아무것도 안 잡으면 FAIL", v4["verdict"] == "FAIL"))

    bad = 0
    for name, ok in checks:
        print(("ok   " if ok else "FAIL ") + name)
        bad += 0 if ok else 1
    print(f"selftest {len(checks) - bad}/{len(checks)}")
    return 0 if bad == 0 else 1


def main() -> int:
    ap = argparse.ArgumentParser(description="p1-03 SC-58·59 data/ 검증(레지스트리 유도)")
    ap.add_argument("--root", default=".")
    ap.add_argument("--evidence")
    ap.add_argument("--glob-census", action="store_true")
    ap.add_argument("--selftest", action="store_true")
    args = ap.parse_args()

    try:
        import jsonschema  # noqa: F401
    except ImportError:
        print("미검증(환경): jsonschema 가 없다 — pip install jsonschema", file=sys.stderr)
        return 2

    root = Path(args.root).resolve()
    if not (root / "contracts/registry/types.json").is_file() or not (root / "data").is_dir():
        print(f"FAIL(구현 없음): {root}/contracts/registry/types.json 또는 {root}/data 가 없다", file=sys.stderr)
        return 3
    if args.selftest:
        return selftest(root)

    result = glob_census(root) if args.glob_census else validate(root)
    text = json.dumps(result, indent=2, ensure_ascii=False)
    print(text)
    if args.evidence:
        Path(args.evidence).write_text(text + "\n", encoding="utf-8")
    return 0 if result["verdict"] == "PASS" else 1


if __name__ == "__main__":
    sys.exit(main())
