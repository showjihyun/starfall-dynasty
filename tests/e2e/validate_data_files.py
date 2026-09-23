#!/usr/bin/env python
"""SC-07 (AC-21 d) — 실제 `data/` 3파일이 계약 스키마를 통과하는가.

**왜 독립으로 재검사하는가.** architect 실측(2026-09-19)은 `sync-tuning.json` 이 6건
실패였고 D-1 로 해소됐다. `data/` 는 designer 소유이고 살아 있으므로 **회귀가 조용히
돌아올 수 있다.** 서버 기동 경로(AC-2)도 같은 것을 보지만, 그건 서버가 자기 자신을
확인하는 것이다(I-25) — 이 스크립트는 서버 밖에서 스키마만 본다.

파일은 **파일명이 아니라 디렉토리 규약**으로 찾는다: designer 가 함선 클래스를 한 벌 더
추가하면 그 파일도 자동으로 검사 대상이 된다.

종료 코드: 0 = 오류 0, 1 = 검증 실패, 3 = 스키마/데이터 디렉토리 없음.
"""
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

for _stream in (sys.stdout, sys.stderr):
    try:
        _stream.reconfigure(encoding="utf-8")  # type: ignore[union-attr]
    except (AttributeError, ValueError):
        pass

# (데이터 glob, 스키마 상대 경로)
TARGETS = [
    ("data/ships/*.json", "contracts/data/ship-class.schema.json"),
    ("data/world/systems/*.json", "contracts/data/star-system.schema.json"),
    ("data/movement/sync-tuning.json", "contracts/data/sync-tuning.schema.json"),
]


def build_registry(contracts: Path):
    """스키마 전부를 $id 로 등록한다 — 상대 $ref 가 파일 경계를 넘어 풀리게."""
    import referencing
    import referencing.jsonschema

    registry = referencing.Registry()
    for path in sorted(contracts.rglob("*.schema.json")):
        doc = json.loads(path.read_text(encoding="utf-8"))
        resource = referencing.Resource.from_contents(doc)
        registry = registry.with_resource(uri=doc["$id"], resource=resource)
        # 상대 $ref("../common/primitives.schema.json#/$defs/X") 를 위해 파일 경로로도 건다.
        registry = registry.with_resource(
            uri=path.relative_to(contracts).as_posix(), resource=resource
        )
    return registry


def main() -> int:
    ap = argparse.ArgumentParser(description="SC-07 data/ 3파일 스키마 검증")
    ap.add_argument("--root", default=".")
    ap.add_argument("--evidence")
    args = ap.parse_args()

    try:
        from jsonschema import Draft202012Validator
    except ImportError:
        print("미검증(환경): jsonschema 가 없다 — pip install jsonschema", file=sys.stderr)
        return 2

    root = Path(args.root).resolve()
    contracts = root / "contracts"
    if not contracts.is_dir() or not (root / "data").is_dir():
        print(f"FAIL(구현 없음): {contracts} 또는 {root/'data'} 가 없다", file=sys.stderr)
        return 3

    registry = build_registry(contracts)
    rows, errors_total, files_checked = [], 0, 0

    for glob, schema_rel in TARGETS:
        schema_path = contracts.parent / schema_rel
        schema = json.loads(schema_path.read_text(encoding="utf-8"))
        validator = Draft202012Validator(schema, registry=registry)
        matched = sorted(root.glob(glob))
        if not matched:
            rows.append({"glob": glob, "schema": schema_rel, "files": 0,
                         "errors": 1, "detail": ["대상 파일이 하나도 없다"]})
            errors_total += 1
            continue
        for f in matched:
            files_checked += 1
            doc = json.loads(f.read_text(encoding="utf-8"))
            errs = sorted(validator.iter_errors(doc), key=lambda e: list(e.absolute_path))
            errors_total += len(errs)
            rows.append({
                "file": f.relative_to(root).as_posix(),
                "schema": schema_rel,
                "errors": len(errs),
                "detail": [f"/{'/'.join(str(p) for p in e.absolute_path)}: {e.message}" for e in errs],
            })

    result = {
        "item": "SC-07 / AC-21(d) — data/ 실제 파일의 계약 스키마 검증",
        "verdict": "PASS" if errors_total == 0 else "FAIL",
        "files_checked": files_checked,
        "errors_total": errors_total,
        "rows": rows,
        "note": "architect 실측 2026-09-19 은 sync-tuning.json 6건 실패였다(D-1 로 해소). 여기서 0 이면 회귀 없음.",
    }
    text = json.dumps(result, indent=2, ensure_ascii=False)
    print(text)
    if args.evidence:
        Path(args.evidence).write_text(text + "\n", encoding="utf-8")
    return 0 if errors_total == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
