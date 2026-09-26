"""SC-83 (p1-01, AC-21 b) — 신규 7타입의 스키마 · Rust · C# 3자 **필드별** 비교.
p0-02 의 SC-71/72(4타입)도 회귀로 함께 돌린다.

계약 §2 가 고정한 행 목록을 **세 출처에서 각각 읽어** 맞춘다. 사람이 눈으로 맞추면
"이름은 다르지만 같은 것"을 매번 판단하게 되므로 기계적으로 뽑는다.

  - 스키마: `contracts/**` 의 JSON Schema (정본). `$ref` 는 `primitives.schema.json` 까지 따라간다
  - Rust  : `server/crates/contracts/src/**` 의 `pub struct` 필드와 `bounded_int_newtype!` 범위
  - C#    : `client/Assets/_Project/Scripts/Contracts/Generated/**` 의 `[JsonProperty("…")]`

**총계가 아니라 표가 근거다**(AC-21 b). 그래서 `--evidence` 는 행을 전부 쓴다.

계약 §2 가 정한 표의 모양:
  * envelope 필드 전부(**`payload` 컨테이너 포함**) + payload 자체 필드
  * 배열 원소 타입 `ShipState` 는 **별도 표**
  * 데이터 3종(`SHIP_CLASS`·`STAR_SYSTEM`·`SYNC_TUNING`)은 **C# 열이 없다**(AC-10 d — 생성 안 함)
  * **정수 행**: 각 언어 타입이 스키마 범위를 손실 없이 담는가
  * **좁힘 행**: `SHIP_SPAWNED`·`SHIP_DESPAWNED` 의 `actor_id`·`causation_id` 가 비-null
    (Rust 비-`Option` / C# `Guid` + `Required.Always`)

    python tests/e2e/interface_matrix.py --evidence <out.json>
    python tests/e2e/interface_matrix.py --markdown <out.md>      # 사람이 읽는 표
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

import db

ROOT = db.REPO_ROOT
GEN = ROOT / "client/Assets/_Project/Scripts/Contracts/Generated"
RUST_SRC = ROOT / "server/crates/contracts/src"
PRIMITIVES = "contracts/common/primitives.schema.json"

# name -> (schema path, envelope path | None, Rust struct, C# class | None)
TYPES: dict[str, tuple[str, str | None, str, str | None]] = {
    # --- p0-02 의 4타입 (회귀) ---
    "COMMAND_RESULT": ("contracts/messages/COMMAND_RESULT.schema.json",
                       "contracts/common/message-envelope.schema.json",
                       "CommandResultMessage", "CommandResultMessage"),
    "SESSION_READY": ("contracts/messages/SESSION_READY.schema.json",
                      "contracts/common/message-envelope.schema.json",
                      "SessionReadyMessage", "SessionReadyMessage"),
    "SESSION_OPENED": ("contracts/events/domain/SESSION_OPENED.schema.json",
                       "contracts/common/event-envelope.schema.json",
                       "SessionOpenedEvent", "SessionOpenedEvent"),
    "SESSION_CLOSED": ("contracts/events/domain/SESSION_CLOSED.schema.json",
                       "contracts/common/event-envelope.schema.json",
                       "SessionClosedEvent", "SessionClosedEvent"),
    # --- p1-01 의 신규 7타입 ---
    "SET_SHIP_CONTROL": ("contracts/commands/SET_SHIP_CONTROL.schema.json",
                         "contracts/common/command-envelope.schema.json",
                         "SetShipControlCommand", "SetShipControlCommand"),
    "WORLD_SNAPSHOT": ("contracts/messages/WORLD_SNAPSHOT.schema.json",
                       "contracts/common/message-envelope.schema.json",
                       "WorldSnapshotMessage", "WorldSnapshotMessage"),
    "SHIP_SPAWNED": ("contracts/events/domain/SHIP_SPAWNED.schema.json",
                     "contracts/common/event-envelope.schema.json",
                     "ShipSpawnedEvent", "ShipSpawnedEvent"),
    "SHIP_DESPAWNED": ("contracts/events/domain/SHIP_DESPAWNED.schema.json",
                       "contracts/common/event-envelope.schema.json",
                       "ShipDespawnedEvent", "ShipDespawnedEvent"),
    # 데이터 3종 — C# 열 없음이 정상이다(AC-10 d)
    "SHIP_CLASS": ("contracts/data/ship-class.schema.json", None, "ShipClassTable", None),
    "STAR_SYSTEM": ("contracts/data/star-system.schema.json", None, "StarSystemTable", None),
    "SYNC_TUNING": ("contracts/data/sync-tuning.schema.json", None, "SyncTuningTable", None),
}

NEW_SEVEN = ["SET_SHIP_CONTROL", "WORLD_SNAPSHOT", "SHIP_SPAWNED", "SHIP_DESPAWNED",
             "SHIP_CLASS", "STAR_SYSTEM", "SYNC_TUNING"]

# 계약 §2 의 "좁힘 행": 이 (타입, 필드) 는 envelope 이 널 가능이어도 비-null 이어야 한다.
NARROWED = {
    ("SHIP_SPAWNED", "actor_id"), ("SHIP_SPAWNED", "causation_id"),
    ("SHIP_DESPAWNED", "actor_id"), ("SHIP_DESPAWNED", "causation_id"),
}

# C# 정수 타입의 표현 범위.
CS_INT_RANGE = {
    "int": (-2**31, 2**31 - 1),
    "long": (-2**63, 2**63 - 1),
    "uint": (0, 2**32 - 1),
    "ulong": (0, 2**64 - 1),
    "short": (-2**15, 2**15 - 1),
    "byte": (0, 255),
}
RUST_INT_RANGE = {
    "i8": (-128, 127), "u8": (0, 255),
    "i16": (-2**15, 2**15 - 1), "u16": (0, 2**16 - 1),
    "i32": (-2**31, 2**31 - 1), "u32": (0, 2**32 - 1),
    "i64": (-2**63, 2**63 - 1), "u64": (0, 2**64 - 1),
}


def load(rel: str) -> dict:
    return json.loads((ROOT / rel).read_text(encoding="utf-8"))


# ---------------------------------------------------------------------------
# 스키마
# ---------------------------------------------------------------------------

def resolve(spec: dict, own_defs: dict, prim: dict) -> dict:
    """`$ref` 를 한 단계 따라간다. `#/$defs/X` 와 `…primitives.schema.json#/$defs/X` 둘 다."""
    ref = spec.get("$ref")
    if not ref:
        return spec
    frag = ref.split("#/$defs/")[-1]
    if ref.startswith("#/$defs/"):
        target = own_defs.get(frag, {})
    elif "primitives.schema.json" in ref:
        target = prim.get("$defs", {}).get(frag, {})
    else:
        return spec
    merged = dict(target)
    merged.update({k: v for k, v in spec.items() if k != "$ref"})
    merged["_ref"] = frag
    return merged


def field_meta(name: str, spec: dict, required: set[str], own_defs: dict,
               prim: dict, source: str) -> dict:
    r = resolve(spec, own_defs, prim)
    branches = r.get("anyOf") or r.get("oneOf") or []
    nullable = any(b.get("type") == "null" for b in branches)
    if not nullable and r.get("type") == "null":
        nullable = True
    # anyOf 안의 비-null 가지를 실제 타입으로 본다
    real = r
    for b in branches:
        if b.get("type") != "null":
            real = resolve(b, own_defs, prim)
            break
    return {
        "source": source,
        "required": name in required,
        "nullable": nullable,
        "json_type": real.get("type"),
        "ref": real.get("_ref") or r.get("_ref"),
        "minimum": real.get("minimum"),
        "maximum": real.get("maximum"),
        "enum": real.get("enum"),
        "const": r.get("const"),
        "items_ref": (real.get("items") or {}).get("$ref", "").split("/")[-1] or None,
    }


def schema_rows(tname: str, prim: dict) -> tuple[dict, dict]:
    """(주 표 행, $defs 에서 뽑은 배열 원소 표들)."""
    spath, epath, _, _ = TYPES[tname]
    t = load(spath)
    defs = t.get("$defs", {})
    rows: dict[str, dict] = {}

    if epath:
        env = load(epath)
        env_defs = env.get("$defs", {})
        env_req = set(env.get("required", []))
        for name, spec in env["properties"].items():
            rows[name] = field_meta(name, spec, env_req, env_defs, prim, "envelope")
        # 타입 고유 override (const 태그, 좁힘 등)
        t_req = set(t.get("required", [])) or env_req
        for name, spec in t.get("properties", {}).items():
            if name == "payload":
                rows["payload"] = field_meta(name, spec, t_req | env_req, defs, prim,
                                             "envelope(payload 컨테이너)")
                continue
            prev = rows.get(name)
            m = field_meta(name, spec, t_req | env_req, defs, prim,
                           "type-override" if prev else "type")
            if prev and not spec.get("anyOf"):
                m["required"] = prev["required"] or m["required"]
            rows[name] = m
        payload_def = next((v for k, v in defs.items() if k.endswith("Payload")), None)
        if payload_def:
            preq = set(payload_def.get("required", []))
            for name, spec in payload_def.get("properties", {}).items():
                rows[f"payload.{name}"] = field_meta(name, spec, preq, defs, prim, "payload")
    else:
        # 데이터 타입: 스키마 최상위가 곧 표. 중첩 객체는 한 단계 펼친다.
        treq = set(t.get("required", []))
        for name, spec in t.get("properties", {}).items():
            m = field_meta(name, spec, treq, defs, prim, "data")
            rows[name] = m
            sub = resolve(spec, defs, prim)
            if sub.get("type") == "object" and sub.get("properties"):
                sreq = set(sub.get("required", []))
                for sname, sspec in sub["properties"].items():
                    rows[f"{name}.{sname}"] = field_meta(sname, sspec, sreq, defs, prim, "data.nested")

    # 배열 원소 표 (계약 §2: ShipState 는 별도 표)
    element_tables: dict[str, dict] = {}
    for row in list(rows.values()):
        ir = row.get("items_ref")
        if ir and ir in defs:
            d = defs[ir]
            dreq = set(d.get("required", []))
            element_tables[ir] = {
                n: field_meta(n, s, dreq, defs, prim, f"{ir}[]")
                for n, s in d.get("properties", {}).items()
            }
    return rows, element_tables


# ---------------------------------------------------------------------------
# Rust
# ---------------------------------------------------------------------------

def rust_index() -> tuple[dict[str, dict[str, str]], dict[str, tuple[str, int, int]]]:
    """(struct -> {field: 타입문자열}, newtype -> (기본타입, min, max))."""
    text = "\n".join(p.read_text(encoding="utf-8") for p in sorted(RUST_SRC.rglob("*.rs")))
    structs: dict[str, dict[str, str]] = {}
    for m in re.finditer(r"pub struct (\w+)\s*\{(.*?)\n\}", text, re.S):
        name, body = m.group(1), m.group(2)
        fields: dict[str, str] = {}
        attrs: list[str] = []
        for line in body.splitlines():
            s = line.strip()
            if s.startswith("#["):
                attrs.append(s)
                continue
            fm = re.match(r"pub (\w+)\s*:\s*([^,\n]+)", s)
            if fm:
                ty = fm.group(2).strip().rstrip(",")
                # `#[serde(default)]` 는 "없어도 된다"를 `Option` 없이 표현한다.
                # 이걸 모르면 `Vec<T> + serde(default)` 가 거짓 불일치로 잡힌다.
                if any("default" in a for a in attrs):
                    ty += "  /*serde(default)*/"
                fields[fm.group(1)] = ty
            attrs = []
        structs[name] = fields

    newtypes: dict[str, tuple[str, int, int]] = {}
    for m in re.finditer(
        r"bounded_int_newtype!\(\s*(\w+)\s*,\s*(\w+)\s*,\s*([^,]+?)\s*,\s*([^,]+?)\s*,",
        text, re.S,
    ):
        name, prim = m.group(1), m.group(2)

        def num(tok: str, prim=prim) -> int:
            tok = tok.strip().replace("_", "")
            if "::MAX" in tok:
                return RUST_INT_RANGE[prim][1]
            if "::MIN" in tok:
                return RUST_INT_RANGE[prim][0]
            return int(re.sub(r"[a-z]\d*$", "", tok))

        newtypes[name] = (prim, num(m.group(3)), num(m.group(4)))
    for m in re.finditer(r"safe_u64_newtype!\(\s*(\w+)", text):
        newtypes[m.group(1)] = ("u64", 0, 9007199254740991)
    for m in re.finditer(r"pub struct (\w+)\((?:pub )?(u\d+|i\d+)\)", text):
        newtypes.setdefault(m.group(1), (m.group(2), *RUST_INT_RANGE[m.group(2)]))
    return structs, newtypes


def rust_lookup(structs: dict, tname: str, field: str) -> tuple[bool, str | None]:
    """계약 표의 행 경로를 Rust 구조체 필드로 찾는다."""
    _, _, rust_struct, _ = TYPES[tname]
    parts = field.split(".")
    if parts[0].endswith("[]"):  # 배열 원소 표 — 원소 구조체를 직접 본다
        elem = parts[0][:-2]
        f = structs.get(elem, {}).get(parts[1])
        return (f is not None), f
    if parts[0] == "payload" and len(parts) > 1:
        payload_ty = structs.get(rust_struct, {}).get("payload")
        if not payload_ty:
            return False, None
        base = payload_ty.replace("Option<", "").replace(">", "").strip()
        f = structs.get(base, {}).get(parts[1])
        return (f is not None), f
    if len(parts) == 2:  # 데이터 타입의 중첩
        outer = structs.get(rust_struct, {}).get(parts[0])
        if not outer:
            return False, None
        base = re.sub(r"^(Option|Vec)<|>$", "", outer).strip()
        f = structs.get(base, {}).get(parts[1])
        return (f is not None), f
    f = structs.get(rust_struct, {}).get(parts[0])
    return (f is not None), f


# ---------------------------------------------------------------------------
# C#
# ---------------------------------------------------------------------------

CS_RE = re.compile(
    r'\[JsonProperty\("([^"]+)"(?:,\s*Required\s*=\s*Required\.(\w+))?\)\]\s*'
    r'public\s+([\w\.\?\[\]<>]+)\s+(\w+)\s*\{'
)


def csharp_index(cls: str | None) -> dict[str, dict] | None:
    if cls is None:
        return None
    path = GEN / f"{cls}.cs"
    if not path.is_file():
        return {}
    text = path.read_text(encoding="utf-8")
    # ShipState 중첩 클래스는 별도로 잘라 둔다.
    nested_start = text.find("class ShipState")
    main_text = text[:nested_start] if nested_start != -1 else text
    out = {}
    for m in CS_RE.finditer(main_text):
        prop, required, ctype = m.group(1), m.group(2), m.group(3)
        out[prop] = {"required": required, "type": ctype,
                     "nullable": ctype.endswith("?") or required == "AllowNull"}
    return out


def csharp_shipstate() -> dict[str, dict]:
    path = GEN / "WorldSnapshotMessage.cs"
    if not path.is_file():
        return {}
    text = path.read_text(encoding="utf-8")
    i = text.find("class ShipState")
    if i == -1:
        return {}
    out = {}
    for m in CS_RE.finditer(text[i:]):
        prop, required, ctype = m.group(1), m.group(2), m.group(3)
        out[prop] = {"required": required, "type": ctype,
                     "nullable": ctype.endswith("?") or required == "AllowNull"}
    return out


# ---------------------------------------------------------------------------
# 비교
# ---------------------------------------------------------------------------

def cmp_row(tname: str, field: str, meta: dict, structs: dict, newtypes: dict,
            cs: dict | None, cs_key: str | None = None) -> dict:
    leaf = cs_key if cs_key is not None else field.split(".")[-1]
    in_rust, rust_ty = rust_lookup(structs, tname, field)
    rust_default = bool(rust_ty and "serde(default)" in rust_ty)
    rust_ty_clean = re.sub(r"\s*/\*serde\(default\)\*/", "", rust_ty).strip() if rust_ty else None
    rust_opt = bool(rust_ty_clean and rust_ty_clean.startswith("Option<"))
    rust_base = re.sub(r"^Option<|>$", "", rust_ty_clean).strip() if rust_ty_clean else None

    cs_has = cs is not None and leaf in cs
    cs_meta = (cs or {}).get(leaf, {})

    problems: list[str] = []
    if not in_rust:
        problems.append("Rust 에 필드가 없다")
    if cs is not None and not cs_has:
        problems.append("C# 에 필드가 없다")

    # **`Option<T>` 는 "널 가능"과 "선택(없어도 됨)" 둘 다를 표현한다.**
    # 이 둘을 섞으면 데이터 테이블의 선택 필드 39건이 전부 거짓 불일치로 나온다
    # (qa 가 라운드 1 에서 실제로 겪었다 — 총계만 보고 FAIL 로 적었으면 server 에게
    # 고칠 것이 없는 수정 요청 39건을 보냈을 것이다). 그래서 기대값은 OR 이다.
    optional_in_rust = meta["nullable"] or not meta["required"]
    if in_rust and optional_in_rust and not (rust_opt or rust_default):
        problems.append(
            f"선택/널 불일치: 스키마 nullable={meta['nullable']} required={meta['required']} "
            f"→ Option 또는 serde(default) 기대 / Rust={rust_ty_clean}")
    if in_rust and not optional_in_rust and rust_opt:
        problems.append(
            f"좁힘 누락: 스키마가 required 이고 널 불가인데 Rust 가 Option 이다 / Rust={rust_ty_clean}")
    if cs_has:
        cs_nullable = cs_meta["nullable"]
        if cs_nullable != meta["nullable"]:
            problems.append(
                f"널 가능 불일치: 스키마 nullable={meta['nullable']} / C# nullable={cs_nullable}")
        if meta["required"] and cs_meta.get("required") not in ("Always", "AllowNull"):
            problems.append(f"필수 불일치: 스키마 required=True / C# Required={cs_meta.get('required')}")

    # 좁힘 행
    narrowed = (tname, field) in NARROWED
    if narrowed:
        if meta["nullable"]:
            problems.append("좁힘 행인데 스키마가 널 가능이다")
        if in_rust and rust_opt:
            problems.append("좁힘 행인데 Rust 가 Option 이다")
        if cs_has and (cs_meta["type"].endswith("?") or cs_meta.get("required") != "Always"):
            problems.append("좁힘 행인데 C# 이 Guid + Required.Always 가 아니다")

    # 정수 행 — 스키마 범위를 손실 없이 담는가
    integer_row, lossless_rust, lossless_cs = False, None, None
    if meta["json_type"] == "integer" and meta["minimum"] is not None \
            and meta["maximum"] is not None:
        integer_row = True
        lo, hi = meta["minimum"], meta["maximum"]
        if rust_base:
            rng = newtypes.get(rust_base) or (
                (rust_base, *RUST_INT_RANGE[rust_base]) if rust_base in RUST_INT_RANGE else None)
            if rng:
                prim = rng[0] if rng[0] in RUST_INT_RANGE else rust_base
                plo, phi = RUST_INT_RANGE.get(prim, (None, None))
                lossless_rust = plo is not None and plo <= lo and hi <= phi
                if lossless_rust is False:
                    problems.append(f"Rust {rust_base}({prim}) 가 스키마 범위 {lo}..{hi} 를 못 담는다")
        if cs_has:
            base = cs_meta["type"].rstrip("?")
            rng = CS_INT_RANGE.get(base)
            if rng:
                lossless_cs = rng[0] <= lo and hi <= rng[1]
                if not lossless_cs:
                    problems.append(f"C# {base} 가 스키마 범위 {lo}..{hi} 를 못 담는다")
            else:
                lossless_cs = None

    return {
        "type": tname, "field": field, "source": meta["source"],
        "schema_type": meta["json_type"], "schema_ref": meta["ref"],
        "schema_required": meta["required"], "schema_nullable": meta["nullable"],
        "schema_min": meta["minimum"], "schema_max": meta["maximum"],
        "rust_present": in_rust, "rust_type": rust_ty, "rust_option": rust_opt,
        "csharp_present": None if cs is None else cs_has,
        "csharp_type": cs_meta.get("type"), "csharp_required": cs_meta.get("required"),
        "csharp_nullable": cs_meta.get("nullable") if cs_has else None,
        "narrowed_row": narrowed, "integer_row": integer_row,
        "lossless_rust": lossless_rust, "lossless_csharp": lossless_cs,
        "problems": problems,
    }


def md_table(title: str, rows: list[dict], with_cs: bool) -> str:
    head = "| 타입 | 필드 | 출처 | 스키마(type / req / null / 범위) | Rust | C# |"
    sep = "|---|---|---|---|---|---|"
    if not with_cs:
        head = "| 타입 | 필드 | 출처 | 스키마(type / req / null / 범위) | Rust |"
        sep = "|---|---|---|---|---|"
    out = [f"#### {title} — 행 {len(rows)}개", "", head, sep]
    for r in rows:
        rng = f" {r['schema_min']}..{r['schema_max']}" if r["schema_min"] is not None else ""
        sch = f"{r['schema_type']} / req={r['schema_required']} / null={r['schema_nullable']}{rng}"
        rust = f"`{r['rust_type']}`" if r["rust_present"] else "**없음**"
        cells = [r["type"], f"`{r['field']}`", r["source"], sch, rust]
        if with_cs:
            cells.append(
                f"`{r['csharp_type']}` Req={r['csharp_required']}" if r["csharp_present"] else "**없음**")
        flag = " ⚠" if r["problems"] else ""
        out.append("| " + " | ".join(str(c) for c in cells) + f" |{flag}")
    return "\n".join(out)


def run(args: argparse.Namespace) -> int:
    prim = load(PRIMITIVES)
    structs, newtypes = rust_index()

    main_rows: list[dict] = []
    element_rows: list[dict] = []
    data_rows: list[dict] = []

    for tname, (_, epath, _, cs_cls) in TYPES.items():
        rows, elements = schema_rows(tname, prim)
        cs = csharp_index(cs_cls)
        bucket = main_rows if epath else data_rows
        for field, meta in rows.items():
            bucket.append(cmp_row(tname, field, meta, structs, newtypes, cs))
        for elem_name, elem_fields in elements.items():
            elem_cs = csharp_shipstate() if elem_name == "ShipState" and cs_cls else None

            # 배열 원소의 Rust 구조체는 이름이 같다고 본다 (ShipState).
            # 경로를 `ShipState[].<field>` 로 넘기면 `rust_lookup` 이 그 구조체를 직접 찾는다.
            for fname, fmeta in elem_fields.items():
                element_rows.append(
                    cmp_row(tname, f"{elem_name}[].{fname}", fmeta, structs,
                            newtypes, elem_cs, cs_key=fname))

    all_rows = main_rows + element_rows + data_rows
    new7 = [r for r in all_rows if r["type"] in NEW_SEVEN]
    mismatches = [r for r in all_rows if r["problems"]]
    new7_mismatches = [r for r in new7 if r["problems"]]

    verdict = "PASS" if not new7_mismatches else "FAIL"
    summary = {
        "item": "SC-83 / AC-21(b) — 신규 7타입 경계면 3자 필드별 비교",
        "verdict": verdict,
        "new_seven": NEW_SEVEN,
        "rows_compared_total": len(all_rows),
        "rows_compared_new_seven": len(new7),
        "rows_envelope_and_payload": len([r for r in new7 if r in main_rows]),
        "rows_array_element_ShipState": len([r for r in new7 if r in element_rows]),
        "rows_data_types_no_csharp_column": len([r for r in new7 if r in data_rows]),
        "mismatches_new_seven": len(new7_mismatches),
        "mismatches_including_p0_02_regression": len(mismatches),
        "integer_rows": len([r for r in new7 if r["integer_row"]]),
        "integer_rows_lossless_rust": len([r for r in new7 if r["integer_row"] and r["lossless_rust"]]),
        "integer_rows_lossless_csharp": len(
            [r for r in new7 if r["integer_row"] and r["lossless_csharp"]]),
        "narrowed_rows": len([r for r in new7 if r["narrowed_row"]]),
        "narrowed_rows_ok": len([r for r in new7 if r["narrowed_row"] and not r["problems"]]),
        "mismatch_detail": new7_mismatches,
        "rows": all_rows,
    }
    db.emit(summary, args.evidence)

    if args.markdown:
        parts = [
            "# SC-83 경계면 비교표 (스키마 / Rust / C#)",
            "",
            f"- 판정: **{verdict}** — 신규 7타입 대조 행 **{summary['rows_compared_new_seven']}개**, "
            f"불일치 **{summary['mismatches_new_seven']}건**",
            f"- 정수 행 {summary['integer_rows']}개 (Rust 무손실 {summary['integer_rows_lossless_rust']} / "
            f"C# 무손실 {summary['integer_rows_lossless_csharp']})",
            f"- 좁힘 행 {summary['narrowed_rows']}개 (통과 {summary['narrowed_rows_ok']})",
            "",
            md_table("와이어 4타입 — envelope + payload", [r for r in main_rows if r["type"] in NEW_SEVEN], True),
            "",
            md_table("배열 원소 ShipState (별도 표)", element_rows, True),
            "",
            md_table("데이터 3타입 — C# 열 없음이 정상 (AC-10 d)", data_rows, False),
            "",
            md_table("p0-02 4타입 (회귀)", [r for r in main_rows if r["type"] not in NEW_SEVEN], True),
        ]
        Path(args.markdown).write_text("\n".join(parts) + "\n", encoding="utf-8")

    return db.EXIT_OK if verdict == "PASS" else db.EXIT_FAIL


def main() -> int:
    ap = argparse.ArgumentParser(description="SC-83 경계면 3자 비교")
    ap.add_argument("--evidence")
    ap.add_argument("--markdown", help="사람이 읽는 표를 이 경로에 쓴다")
    args = ap.parse_args()
    return db.main_guard(lambda: run(args))


if __name__ == "__main__":
    sys.exit(main())
