"""`tools/bots` 의 Rust probe 라벨을 **팔 단위로** 읽는다 (architect R28 Q-1·Q-3·Q-5).

## 왜 정규식 한 줄로 안 되는가

옛 `RUST_LABEL` 은 한 줄짜리 `=> "…"` 만 잡았고, 실제 구조는 이렇다:

    Self::CheatRange => {
        // 주석이 여기 들어간다
        "SC-24/SC-67 (AC-5c/AC-17b): …"
    }

**주석을 먼저 지우지 않으면 17 팔 중 14 만 잡힌다**(`CheatRange`·`CheatSeq`·`CheatFlood` 가
`=> {` 와 문자열 사이에 주석을 갖고 있다 — architect 실측). 그래서 순서가 정해져 있다:
**① 주석 줄 제거 → ② 팔 경계로 쪼개기 → ③ 팔 안의 문자열 리터럴.**

## 이을 키는 §3.1 표가 아니라 **케이스 이름**이다

만기 행은 §3.1 을 가리켰지만 실측이 막았다 — §3.1 의 치트 7종 이름 중 **5개가
`tools/bots/src` 에 존재하지 않는다.** 손으로 유지하는 대응표가 필요해지고, 그것은 지워진
`RUST_KNOWN_MISMATCHES = 8` 과 **같은 종류의 부패**다.

대신 **`as_str()` 의 변이→CLI 이름 맵**을 소스에서 읽고, 계약 §1 행의 **백틱 span 안 토큰**과
**완전히 같을 때만** 잇는다. **부분문자열로 이으면 샌다** — `cheat-range` 는
`cheat-range-turn` 의 접두다(Q-6 (d)).

## 종류 판정 — 명시가 있으면 명시, 없으면 표식

`ContractRef::Verdict` / `ContractRef::Reference` 가 팔에 있으면 **그것이 정본**이다.
아직 없는 동안은 **관측 표식**(`관측용`·`참고`·`참조`)으로 가른다 — 규칙 7 보충이 이미 그
구분을 쓰고 있다. **표식도 명시도 없으면 verdict 로 본다**(주장하는 쪽을 기본값으로 둔다 —
`미분류`를 통과로 읽는 것이 이 슬라이스가 반복해 데인 형태다).
"""

from __future__ import annotations

import re

# `ContractRef::Verdict` / `::Reference` — rust 가 넣으면 이것이 정본이다.
CONTRACT_REF = re.compile(r"ContractRef::(Verdict|Reference)")
# **Q-8: 비수식 `Verdict(`/`Reference(` 는 받지 않는다.** 정규식을 넓히면 **나중에 들어올 다른
# enum 이 공급하는 같은 토큰**을 verdict 주장으로 읽는다 — 없앤 `참고` 부분문자열과 같은 느슨함이다.
#
# 이것이 합성 대조가 아니라 **실제로 일어난 모양**이다: rust 가 `use ContractRef::{Reference,
# Verdict};` 를 넣자 수식 적중이 **0/17** 이 됐고, **위반도 0 이므로 게이트가 "위반 0 · exit 0" 을
# 인쇄할 수 있었다 — 팔을 하나도 보지 못한 채로.** 항등식 ①(변이==팔)이 그것을 잡았다.
UNQUALIFIED_REF = re.compile(r'(?<![\w:])(Verdict|Reference)\s*\(')
# 명시가 없는 동안의 대체 — 계약 §7b 규칙 7 보충이 쓰는 표식.
OBSERVATION_MARKS = ("관측용", "참고", "참조")
SC_NUM = re.compile(r"SC-(\d+)")
SC_LIST = re.compile(r"SC-(\d+)((?:\s*[/·,]\s*(?:SC-)?\d+)+)")
BARE_NUM = re.compile(r"\d+")
STRING_LIT = re.compile(r'"((?:[^"\\]|\\.)*)"')
ARM_HEAD = re.compile(r"^\s*Self::(\w+)\s*=>", re.M)
# 백틱 span 안에서 CLI 케이스 이름 모양의 토큰만 꺼낸다.
CASE_TOKEN = re.compile(r"[a-z][a-z0-9]*(?:-[a-z0-9]+)*")
CONTRACT_ROW = re.compile(r"^\|\s*\*{0,2}SC-(\d+)\*{0,2}\s*\|")


def strip_line_comments(text: str) -> str:
    """`//` 로 시작하는 줄을 지운다. **파싱 전에 반드시 먼저 한다**(위 docstring)."""
    return "\n".join(l for l in text.splitlines() if not l.lstrip().startswith("//"))


def expand_sc(text: str) -> set[int]:
    """`SC-11/12/13` · `SC-24(c)(d)·SC-67` 처럼 **접두사를 한 번만 쓰는 열거**까지 펼친다."""
    out: set[int] = set()
    for first, rest in SC_LIST.findall(text):
        out.add(int(first))
        out.update(int(n) for n in BARE_NUM.findall(rest))
    out.update(int(m) for m in SC_NUM.findall(text))
    return out


def _block(src: str, header: str, after: int = 0) -> str:
    i = src.index(header, after)
    return src[i:src.index("\n    }", i)]


def case_names(src: str) -> dict[str, str]:
    """`as_str()` 의 **변이 → CLI 이름**. 이것이 계약과 잇는 유일한 키다."""
    stripped = strip_line_comments(src)
    blk = _block(stripped, "pub fn as_str(self) -> &'static str {",
                 stripped.index("pub enum ProbeCase"))
    return {v: n for v, n in re.findall(r"Self::(\w+)\s*=>\s*\"([^\"]+)\"", blk)}


def contract_arms(src: str) -> list[dict]:
    """`contract_item()` 의 팔마다 `{variant, kind, literals, sc}`.

    **`literals` 가 1개가 아니면 `kind = "unclassifiable"`** — 팔의 첫 리터럴만 읽으면
    둘째의 `SC-11` 이 보이지 않고 exit 0 이 난다(architect 실측, Q-6 (g)).
    """
    stripped = strip_line_comments(src)
    blk = _block(stripped, "pub fn contract_item(self)")
    parts = re.split(r"\n(?=\s*Self::\w+\s*=>)", blk)[1:]
    arms = []
    for part in parts:
        m = ARM_HEAD.match(part) or re.match(r"\s*Self::(\w+)\s*=>", part)
        if not m:
            continue
        lits = [g for g in STRING_LIT.findall(part)]
        explicit = CONTRACT_REF.search(part)
        unqualified = UNQUALIFIED_REF.search(part)
        if len(lits) != 1:
            kind = "unclassifiable"
        elif unqualified and not explicit:
            # Q-8: 종류를 주장하려 했으나 **수식이 아니다.** 추측하지 않고 판정을 멈춘다.
            kind = "unqualified"
        elif explicit:
            kind = explicit.group(1).lower()          # verdict | reference
        elif any(mark in lits[0] for mark in OBSERVATION_MARKS):
            kind = "reference"
        else:
            kind = "verdict"                          # 주장하는 쪽을 기본값으로
        # **무엇이 종류를 정했는가** (Q-9). 종류만으로는 *"명시가 있었는가"* 를 알 수 없고,
        # 항등식 ① 은 **팔 수**를 세므로 산문으로 분류된 팔도 팔로 세어져 통과한다.
        # 그 둘째 질문을 재는 수가 없어서 **`참고:` 두 글자로 verdict 주장 하나가 조용히
        # 면제되는 경로**가 남아 있었다(architect R28 후속 4 실측).
        decided_by = ("explicit" if explicit
                      else "unqualified" if unqualified
                      else "mark" if (lits and any(mk in lits[0] for mk in OBSERVATION_MARKS))
                      else "default")
        arms.append({
            "variant": m.group(1),
            "kind": kind,
            "decided_by": decided_by,
            "literals": lits,
            "sc": sorted(expand_sc(lits[0])) if lits else [],
        })
    return arms


def contract_named_cases(contract_text: str, known: set[str]) -> dict[str, set[int]]:
    """계약 **§1 항목 행의 백틱 span 안 토큰**이 케이스 이름과 **완전히 같을 때만** 잇는다.

    **§1 밖 행을 세지 않는다** (Q-2·Q-6 (h)): `| SC-nn |` 모양은 §7b 논의 표에도 있고
    (실측: `SC-55` 가 §1 과 §7b 두 곳), 그것을 지명으로 읽으면 **논의 표에 도구 이름을
    적는 순간 지명이 생긴다.**

    **백틱 밖 토큰도 세지 않는다** (Q-6 (f)): `order`·`binary`·`idle`·`fly`·`duplicate` 는
    영어 단어이고 계약 산문에 그냥 나온다.
    """
    out: dict[str, set[int]] = {}
    section = None
    for line in contract_text.splitlines():
        if line.startswith("## "):        # 최상위 절만. `### A.`~`L.` 은 §1 의 하위다
            section = line[3:].strip()
        m = CONTRACT_ROW.match(line.strip())
        if not m or not (section and section.startswith("1.")):
            continue
        n = int(m.group(1))
        for span in re.findall(r"`([^`]+)`", line):
            for tok in CASE_TOKEN.findall(span):
                if tok in known:
                    out.setdefault(tok, set()).add(n)
    return out


def identities(scenario_src: str, main_src: str, all_rs: dict[str, str] | None = None) -> dict:
    """**판정 전에** 보는 두 항등식 (Q-3).

    `1 / 22` 가 뜻을 잃은 것은 수가 작아서가 아니라 **22 가 판정 라벨 17 과 도움말 5 를
    섞었기 때문**이다. 그래서 두 종류를 **갈라서** 찍는다.
    """
    names = case_names(scenario_src)
    arms = contract_arms(scenario_src)
    arm_variants = {a["variant"] for a in arms}
    missing = sorted(set(names) - arm_variants)
    extra = sorted(arm_variants - set(names))
    # **분모는 `tools/bots/src` 전체다** — `scenario.rs` 만 세면 22 가 아니라 17 이 나오고,
    # 그러면 항등식이 "도움말 5 줄"을 설명하지 못해 늘 깨진 것으로 읽힌다(내가 그렇게 짰다).
    sources = all_rs if all_rs is not None else {"scenario.rs": scenario_src, "main.rs": main_src}
    sc_lines = [
        l for src in sources.values()
        for l in strip_line_comments(src).splitlines() if "SC-" in l
    ]
    usage = ""
    if "const USAGE" in main_src:
        i = main_src.index("const USAGE")
        usage = main_src[i:main_src.index('"#;', i)]
    usage_lines = [l for l in usage.splitlines() if "SC-" in l]
    id1 = len(names) == len(arms) and not missing and not extra
    id2 = len(sc_lines) == len(arms) + len(usage_lines)
    return {
        "identity_1_variants_vs_arms": {
            "variants": len(names), "arms": len(arms),
            "variants_without_arm": missing,          # **이름으로 인쇄한다**
            "arms_without_variant": extra,
            "ok": id1,
        },
        "identity_2_sc_lines": {
            "non_comment_sc_lines_all_rs": len(sc_lines),
            "contract_item_arms": len(arms),
            "main_usage_sc_lines": len(usage_lines),
            "ok": id2,
            "note": "판정 라벨과 도움말을 갈라서 센다 — 섞으면 분수가 뜻을 잃는다",
        },
        "ok": id1 and id2,
    }
