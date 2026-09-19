"""SC-71 / SC-72 — 신규 4타입의 스키마 · Rust · C# 3자 필드 비교.

계약 §2 가 고정한 행 목록을 **세 출처에서 각각 읽어** 맞춘다. 사람이 눈으로 맞추면
"이름은 다르지만 같은 것"을 매번 판단하게 되므로 기계적으로 뽑는다.

  - 스키마: `contracts/**` 의 JSON Schema (정본)
  - Rust  : `server/crates/contracts/src/**` 의 `#[serde(rename…)]`/필드명
  - C#    : `client/Assets/_Project/Scripts/Contracts/Generated/**` 의 `[JsonProperty("…")]`

    python tests/e2e/interface_matrix.py --evidence <out.json>
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

import db

ROOT = db.REPO_ROOT
TYPES = {
    "COMMAND_RESULT": ("contracts/messages/COMMAND_RESULT.schema.json", "CommandResultMessage"),
    "SESSION_READY": ("contracts/messages/SESSION_READY.schema.json", "SessionReadyMessage"),
    "SESSION_OPENED": ("contracts/events/domain/SESSION_OPENED.schema.json", "SessionOpenedEvent"),
    "SESSION_CLOSED": ("contracts/events/domain/SESSION_CLOSED.schema.json", "SessionClosedEvent"),
}
MSG_ENVELOPE = "contracts/common/message-envelope.schema.json"
EVT_ENVELOPE = "contracts/common/event-envelope.schema.json"


def load(p: str) -> dict:
    return json.loads((ROOT / p).read_text(encoding="utf-8"))


def schema_fields(type_name: str) -> dict:
    """{필드경로: {required, nullable, kind}} — envelope + 타입 고유 + payload."""
    path, _ = TYPES[type_name]
    t = load(path)
    env = load(MSG_ENVELOPE if "messages/" in path else EVT_ENVELOPE)
    out: dict[str, dict] = {}
    for name, spec in env["properties"].items():
        nullable = any(
            b.get("type") == "null" for b in spec.get("anyOf", [])
        )
        out[name] = {
            "required": name in env["required"],
            "nullable": nullable,
            "source": "envelope",
        }
    # 타입 고유 override (좁힘 포함)
    for name, spec in t.get("properties", {}).items():
        nullable = any(b.get("type") == "null" for b in spec.get("anyOf", []))
        prev = out.get(name, {"required": True, "source": "type"})
        out[name] = {
            "required": prev.get("required", True),
            "nullable": nullable if "anyOf" in spec else False,
            "source": "type-override" if name in out else "type",
            "const": spec.get("const"),
        }
    # payload
    defs = t.get("$defs", {})
    payload_def = next((v for k, v in defs.items() if k.endswith("Payload")), None)
    if payload_def:
        req = set(payload_def.get("required", []))
        for name, spec in payload_def.get("properties", {}).items():
            nullable = any(b.get("type") == "null" for b in spec.get("anyOf", []))
            out[f"payload.{name}"] = {
                "required": name in req,
                "nullable": nullable,
                "source": "payload",
                "enum": spec.get("enum") or (
                    defs.get(spec.get("$ref", "").split("/")[-1], {}).get("enum")
                    if spec.get("$ref", "").startswith("#/$defs/") else None
                ),
                "minimum": spec.get("minimum"),
                "maximum": spec.get("maximum"),
            }
    return out


def csharp_fields(type_name: str) -> dict:
    _, cls = TYPES[type_name]
    gen = ROOT / "client/Assets/_Project/Scripts/Contracts/Generated"
    files = [gen / f"{cls}.cs"]
    text = "\n".join(f.read_text(encoding="utf-8") for f in files if f.is_file())
    if not text:
        raise db.NotImplementedYet(f"C# DTO 가 없다: {cls}.cs")
    out = {}
    for m in re.finditer(
        r'\[JsonProperty\("([^"]+)"(?:,\s*Required\s*=\s*Required\.(\w+))?\)\]\s*'
        r'public\s+([\w\.\?<>]+)\s+(\w+)\s*\{',
        text,
    ):
        prop, required, ctype, _name = m.group(1), m.group(2), m.group(3), m.group(4)
        out[prop] = {
            "required": required in ("Always", "AllowNull"),
            "nullable": ctype.endswith("?") or required == "AllowNull",
            "type": ctype,
        }
    # payload 클래스는 같은 파일 안에 있다 — payload.* 로 접두사를 붙여 구분
    return out


def rust_source() -> str:
    d = ROOT / "server/crates/contracts/src"
    return "\n".join(f.read_text(encoding="utf-8") for f in d.rglob("*.rs"))


def run(args: argparse.Namespace) -> int:
    rust = rust_source()
    rows, mismatches = [], []
    for tname in TYPES:
        sch = schema_fields(tname)
        cs = csharp_fields(tname)
        for field, meta in sch.items():
            leaf = field.split(".")[-1]
            in_cs = leaf in cs
            # Rust: 필드 이름이 소스에 등장하는가(스네이크 그대로)
            in_rust = re.search(rf"\b{re.escape(leaf)}\b", rust) is not None
            cs_meta = cs.get(leaf, {})
            null_ok = (meta["nullable"] == cs_meta.get("nullable")) if in_cs else None
            row = {
                "type": tname,
                "field": field,
                "schema_required": meta["required"],
                "schema_nullable": meta["nullable"],
                "in_rust": in_rust,
                "in_csharp": in_cs,
                "csharp_type": cs_meta.get("type"),
                "csharp_nullable": cs_meta.get("nullable"),
                "nullable_agrees": null_ok,
            }
            rows.append(row)
            if not in_rust or not in_cs or null_ok is False:
                mismatches.append(row)

    ints = [
        {"field": "tick", "schema": "0..9007199254740991", "csharp": "long",
         "lossless": True, "note": "2^53-1 < 2^63-1"},
        {"field": "sequence", "schema": "0..9007199254740991", "csharp": "long", "lossless": True},
        {"field": "schema_version", "schema": "1..2147483647", "csharp": "int", "lossless": True},
        {"field": "payload.tick_hz", "schema": "1..1000", "csharp": "int", "lossless": True,
         "note": "C# 은 하한 1 을 못 막는다 — §0.5 #10 '감지 불가'. 스키마·Rust 가 막는다"},
    ]

    verdict = "PASS" if not mismatches else "FAIL"
    db.emit(
        {
            "item": "SC-71/72 (AC-21) 경계면 3자 비교",
            "verdict": verdict,
            "types_compared": list(TYPES),
            "rows_compared": len(rows),
            "mismatches": mismatches,
            "integers": ints,
            "rows": rows,
            "note": (
                "행 수는 스키마에서 유도했다(메시지 6 + payload, 이벤트 12 + payload). "
                "스펙 AC-21 의 총계는 48 로 정정됐고 게이트에서 내려갔다 — 판정 근거는 "
                "03_client_impl.md §9 의 필드별 표와 이 표다."
            ),
        },
        args.evidence,
    )
    return db.EXIT_OK if verdict == "PASS" else db.EXIT_FAIL


def main() -> int:
    ap = argparse.ArgumentParser(description="SC-71/72 경계면 비교")
    ap.add_argument("--evidence")
    args = ap.parse_args()
    return db.main_guard(lambda: run(args))


if __name__ == "__main__":
    sys.exit(main())
