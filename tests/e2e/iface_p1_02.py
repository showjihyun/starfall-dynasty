"""p1-02 경계면 3자 비교의 계산부 — `interface_matrix.py --slice p1-02` 가 부른다.

판정 라벨은 `interface_matrix.py` 에 있다(출처 게이트가 도구 이름으로 잇는다 — 계약 §3.2).
이 파일은 라벨을 내지 않는다.

p1-01 판(`interface_matrix.py` 본체)은 `ShipState` 한 원소 표에 맞춰 짜여 있어서, p1-02 의
중첩(`payload.items[]`, `payload.deposits[]`, `payload.historical_event.participants[]` …)을
일반적으로 따라가지 못한다. 그래서 **경로를 재귀로** 따라가는 판을 따로 둔다:

  - 스키마: `allOf`/`$ref`(파일 경계 포함)를 풀어 **경로 → (required, nullable, 정수 범위)**
  - Rust  : 루트 구조체에서 경로 조각마다 필드 타입을 따라 다음 구조체로(`Option<`·`Vec<` 벗김)
  - C#    : 생성 파일의 클래스 블록(중괄호 깊이로 가름)별 `[JsonProperty]` → 같은 방식으로 따라감

행마다 보는 것(계약 §4 의 고정 대상):
  1. 세 쪽에 필드가 있다
  2. **required + nullable**(키는 있어야 하고 값은 null 가능): Rust `Option<T>` + `deserialize_with`
     (없으면 serde 가 빠진 키를 `None` 으로 받아 SC-37 변이가 공짜로 통과한다), `serde(default)` 없음
     / C# `Required.AllowNull`
  3. 선택(required 아님): Rust `Option` 또는 `serde(default)`
  4. 필수·비-null: Rust 비-`Option`, C# `Required.Always`
  5. 정수 범위를 각 언어 타입이 손실 없이 담는다
"""

from __future__ import annotations

import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
GEN = ROOT / "client/Assets/_Project/Scripts/Contracts/Generated"
RUST_SRC = ROOT / "server/crates/contracts/src"

# 이름 → (스키마, Rust 루트 구조체, C# 루트 클래스 | None)
TYPES = {
    "MINE_RESOURCE": ("contracts/commands/MINE_RESOURCE.schema.json", "MineResourceCommand", "MineResourceCommand"),
    "MINERAL_MINED": ("contracts/events/domain/MINERAL_MINED.schema.json", "MineralMinedEvent", "MineralMinedEvent"),
    "MINERAL_DISCOVERED": ("contracts/events/historical/MINERAL_DISCOVERED.schema.json", "MineralDiscoveredEvent", "MineralDiscoveredEvent"),
    "INVENTORY_STATE": ("contracts/messages/INVENTORY_STATE.schema.json", "InventoryStateMessage", "InventoryStateMessage"),
    "DEPOSIT_FIELD_STATE": ("contracts/messages/DEPOSIT_FIELD_STATE.schema.json", "DepositFieldStateMessage", "DepositFieldStateMessage"),
    "HISTORICAL_EVENT_NOTICE": ("contracts/messages/HISTORICAL_EVENT_NOTICE.schema.json", "HistoricalEventNoticeMessage", "HistoricalEventNoticeMessage"),
    # 데이터 4종 — C# 열 없음이 정상(AC-10 d)
    "MINERAL": ("contracts/data/mineral.schema.json", "MineralTable", None),
    "DEPOSIT_FIELD": ("contracts/data/deposit-field.schema.json", "DepositFieldTable", None),
    "MINING_RULES": ("contracts/data/mining-rules.schema.json", "MiningRulesTable", None),
    "SIGNIFICANCE_RULE": ("contracts/data/significance-rule.schema.json", "SignificanceRuleTable", None),
}

INT_RANGE = {
    "i8": (-128, 127), "u8": (0, 255), "i16": (-2**15, 2**15 - 1), "u16": (0, 2**16 - 1),
    "i32": (-2**31, 2**31 - 1), "u32": (0, 2**32 - 1), "i64": (-2**63, 2**63 - 1), "u64": (0, 2**64 - 1),
    "int": (-2**31, 2**31 - 1), "long": (-2**63, 2**63 - 1), "uint": (0, 2**32 - 1),
    "ulong": (0, 2**64 - 1), "short": (-2**15, 2**15 - 1), "byte": (0, 255),
}


# ── 스키마 ────────────────────────────────────────────────────────────────────

def _load(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def _deref(spec: dict, base: Path) -> tuple[dict, Path]:
    """`$ref` 하나를 푼다. (스펙, 그 스펙이 사는 파일)."""
    ref = spec.get("$ref")
    if not ref:
        return spec, base
    file_part, _, frag = ref.partition("#")
    target_file = (base.parent / file_part).resolve() if file_part else base
    doc = _load(target_file)
    node = doc
    for seg in [s for s in frag.split("/") if s]:
        node = node[seg]
    # $ref 옆의 제약(minimum 등)은 덮어쓴다
    merged = dict(node)
    merged.update({k: v for k, v in spec.items() if k != "$ref"})
    return _deref(merged, target_file) if "$ref" in node else (merged, target_file)


def _object_shape(spec: dict, base: Path) -> tuple[dict, set]:
    """객체 스펙의 ({이름: (스펙, 그 스펙의 파일)}, required). `allOf` 의 참조 envelope 을 합친다."""
    spec, base = _deref(spec, base)
    props: dict = {}
    req: set = set()
    for part in spec.get("allOf", []):
        p, r = _object_shape(part, base)
        props.update(p)
        req |= r
    for k, v in spec.get("properties", {}).items():
        # 타입 고유 override: const·설명만 있는 스펙은 envelope 의 모양(nullable 등)을 바꾸지 않는다
        if k in props and set(v) <= {"const", "description"}:
            continue
        props[k] = (v, base)
    req |= set(spec.get("required", []))
    return props, req


def _meta(spec: dict, base: Path) -> dict:
    """nullable·정수 범위·배열/객체 여부."""
    nullable = False
    inner = spec
    if "anyOf" in spec:
        alts = spec["anyOf"]
        nullable = any(a.get("type") == "null" for a in alts)
        non_null = [a for a in alts if a.get("type") != "null"]
        inner = non_null[0] if non_null else spec
    inner, ibase = _deref(inner, base)
    t = inner.get("type")
    if isinstance(t, list):
        nullable = nullable or "null" in t
        t = next((x for x in t if x != "null"), None)
    return {"nullable": nullable, "json_type": t, "minimum": inner.get("minimum"),
            "maximum": inner.get("maximum"), "spec": inner, "base": ibase}


def schema_rows(schema_rel: str) -> dict[str, dict]:
    """경로 → 메타. 배열은 `name[]`, 중첩은 `.` 로 잇는다."""
    rows: dict[str, dict] = {}

    def walk(spec: dict, base: Path, prefix: str) -> None:
        props, req = _object_shape(spec, base)
        for name, (pspec, pbase) in props.items():
            m = _meta(pspec, pbase)
            path = f"{prefix}{name}"
            rows[path] = {"required": name in req, **{k: m[k] for k in ("nullable", "json_type", "minimum", "maximum")}}
            if m["json_type"] == "object" or (m["json_type"] is None and (m["spec"].get("properties") or m["spec"].get("allOf"))):
                walk(m["spec"], m["base"], path + ".")
            elif m["json_type"] == "array":
                items, ibase = _deref(m["spec"].get("items", {}), m["base"])
                im = _meta(items, ibase)
                if im["json_type"] == "object" or im["spec"].get("properties") or im["spec"].get("allOf"):
                    walk(im["spec"], im["base"], path + "[].")

    path = (ROOT / schema_rel).resolve()
    walk(_load(path), path, "")
    return rows


# ── Rust ─────────────────────────────────────────────────────────────────────

def rust_structs() -> dict[str, dict[str, dict]]:
    text = "\n".join(p.read_text(encoding="utf-8") for p in sorted(RUST_SRC.rglob("*.rs")))
    out: dict[str, dict[str, dict]] = {}
    for m in re.finditer(r"pub struct (\w+)(?:<[^>]*>)?\s*\{(.*?)\n\}", text, re.S):
        fields: dict[str, dict] = {}
        attrs: list[str] = []
        for line in m.group(2).splitlines():
            s = line.strip()
            if s.startswith("#["):
                attrs.append(s)
                continue
            if s.startswith("///") or s.startswith("//") or not s:
                continue
            fm = re.match(r"pub (\w+)\s*:\s*(.+?),?$", s)
            if fm:
                fields[fm.group(1)] = {"type": fm.group(2).strip().rstrip(","),
                                       "deserialize_with": any("deserialize_with" in a for a in attrs),
                                       "default": any("default" in a for a in attrs)}
            attrs = []
        out[m.group(1)] = fields
    return out


def _rust_inner(ty: str) -> str:
    t = ty
    while True:
        n = re.sub(r"^(Option|Vec|Box)<(.*)>$", r"\2", t.strip())
        if n == t:
            return t.strip()
        t = n


def rust_lookup(structs: dict, root: str, path: str) -> dict | None:
    cur = root
    parts = path.replace("[]", "").split(".")
    field = None
    for i, seg in enumerate(parts):
        field = structs.get(cur, {}).get(seg)
        if field is None:
            return None
        if i < len(parts) - 1:
            cur = _rust_inner(field["type"])
    return field


# ── C# ───────────────────────────────────────────────────────────────────────

CS_PROP = re.compile(r'\[JsonProperty\("([^"]+)"(?:,\s*Required\s*=\s*Required\.(\w+))?\)\]\s*'
                     r'public\s+([\w\.\?\[\]<>]+)\s+(\w+)\s*\{')


def cs_classes(root_cls: str) -> dict[str, dict[str, dict]]:
    """클래스 이름 → {json 이름: {type, required}}. 중첩 클래스는 중괄호 깊이로 가른다.
    같은 이름의 중첩 클래스가 파일마다 있을 수 있으므로 **루트 파일 하나**만 본다."""
    text = (GEN / f"{root_cls}.cs").read_text(encoding="utf-8")
    out: dict[str, dict[str, dict]] = {}
    stack: list[tuple[str, int]] = []   # (클래스, 그 클래스 본문이 시작된 깊이)
    depth = 0
    i = 0
    pending: str | None = None
    while i < len(text):
        m_cls = re.compile(r"class (\w+)").match(text, i)
        if m_cls:
            pending = m_cls.group(1)
            i = m_cls.end()
            continue
        m_prop = CS_PROP.match(text, i)
        if m_prop and stack:
            out.setdefault(stack[-1][0], {})[m_prop.group(1)] = {
                "type": m_prop.group(3), "required": m_prop.group(2)}
            i = m_prop.end() - 1
            continue
        ch = text[i]
        if ch == "{":
            depth += 1
            if pending:
                stack.append((pending, depth))
                out.setdefault(pending, {})
                pending = None
        elif ch == "}":
            if stack and stack[-1][1] == depth:
                stack.pop()
            depth -= 1
        i += 1
    return out


def cs_lookup(classes: dict, root: str, path: str) -> dict | None:
    cur = root
    parts = path.replace("[]", "").split(".")
    prop = None
    for k, seg in enumerate(parts):
        prop = classes.get(cur, {}).get(seg)
        if prop is None:
            return None
        if k < len(parts) - 1:
            base_ty = prop["type"].rstrip("?")
            base_ty = base_ty[:-2] if base_ty.endswith("[]") else base_ty   # 생성기는 배열을 T[] 로 낸다
            cur = re.sub(r"^(List|IReadOnlyList|IList)<(.*)>$", r"\2", base_ty).split(".")[-1]
    return prop


# ── 비교 ─────────────────────────────────────────────────────────────────────

def compare(structs: dict | None = None, cs_loader=None) -> dict:
    """`structs`·`cs_loader` 는 selftest 가 변이를 넣으려고 주입한다(규칙 6)."""
    structs = rust_structs() if structs is None else structs
    cs_loader = cs_classes if cs_loader is None else cs_loader
    rows_out: list[dict] = []
    for tname, (schema, rust_root, cs_root) in TYPES.items():
        classes = cs_loader(cs_root) if cs_root else None
        for path, meta in schema_rows(schema).items():
            rf = rust_lookup(structs, rust_root, path)
            cf = cs_lookup(classes, cs_root, path) if classes is not None else None
            probs: list[str] = []
            if rf is None:
                probs.append("Rust 에 필드가 없다")
            if classes is not None and cf is None:
                probs.append("C# 에 필드가 없다")
            req, null = meta["required"], meta["nullable"]
            if rf is not None:
                ropt = rf["type"].startswith("Option<")
                if req and null:
                    if not ropt:
                        probs.append(f"required+nullable 인데 Rust 가 Option 이 아니다: {rf['type']}")
                    elif not rf["deserialize_with"] or rf["default"]:
                        probs.append("required+nullable 인데 Rust 가 빠진 키를 None 으로 받는다"
                                     "(deserialize_with 없음 또는 serde(default)) — SC-37")
                elif not req:
                    if not (ropt or rf["default"]):
                        probs.append(f"선택 필드인데 Rust 가 Option/serde(default) 가 아니다: {rf['type']}")
                elif ropt:
                    probs.append(f"필수·비-null 인데 Rust 가 Option 이다: {rf['type']}")
            if cf is not None:
                cnull = cf["type"].endswith("?") or cf["required"] == "AllowNull"
                if null and not cnull:
                    probs.append(f"nullable 인데 C# 이 null 을 못 담는다: {cf['type']} Req={cf['required']}")
                if not null and cf["required"] == "AllowNull":
                    probs.append("null 불가인데 C# 이 Required.AllowNull")
                if req and cf["required"] not in ("Always", "AllowNull"):
                    probs.append(f"required 인데 C# Required={cf['required']}")
            # 정수 범위
            lossless = {}
            if meta["json_type"] == "integer" and meta["minimum"] is not None and meta["maximum"] is not None:
                lo, hi = meta["minimum"], meta["maximum"]
                if cf is not None:
                    base = cf["type"].rstrip("?")
                    if base in INT_RANGE:
                        ok = INT_RANGE[base][0] <= lo and hi <= INT_RANGE[base][1]
                        lossless["csharp"] = ok
                        if not ok:
                            probs.append(f"C# {base} 가 {lo}..{hi} 를 못 담는다")
            rows_out.append({"type": tname, "field": path, **{k: meta[k] for k in ("required", "nullable", "json_type", "minimum", "maximum")},
                             "rust": rf["type"] if rf else None,
                             "rust_deserialize_with": rf["deserialize_with"] if rf else None,
                             "csharp": (f"{cf['type']} Req={cf['required']}" if cf else None) if classes is not None else "(데이터 — C# 열 없음)",
                             "lossless": lossless, "problems": probs})
    per_type = {}
    for r in rows_out:
        per_type.setdefault(r["type"], 0)
        per_type[r["type"]] += 1
    return {
        "types": len(TYPES),
        "rows_total": len(rows_out),
        "rows_per_type": per_type,
        "required_nullable_rows": sum(1 for r in rows_out if r["required"] and r["nullable"]),
        "mismatches": [r for r in rows_out if r["problems"]],
        "rows": rows_out,
    }
