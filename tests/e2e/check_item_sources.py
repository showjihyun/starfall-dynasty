"""**출처 대조** — 어떤 SC 번호로 verdict 를 내는 도구가 **계약이 그 항목에 지명한 도구인가.**

계약 §7b 규칙 7(18차 신설, architect R18 제안). 게이트의 **세 번째 방향**이다:

| 방향 | 묻는 것 | 도구 |
|------|--------|------|
| 순방향 | 계약의 항목이 판정·대기로 분류됐는가 | `check_contract_items.py` (세기만 한다) |
| 역방향 | 리포트가 판정한 번호가 계약에 **있는가** | `check_contract_items.py` |
| **출처** | 그 번호로 verdict 를 내는 **도구**가 계약이 지명한 도구인가 | **이 파일** |

## 왜 세 번째가 필요한가 — 앞의 둘이 원리적으로 못 잡는다

`append_only_probe.py` 가 `"item": "SC-11/12/13 (AC-4) append-only …"` 로 verdict 를 냈다.
**그 셋은 p0-02 의 번호다.** p1-01 계약에서 SC-11 은 *잔류 창 안 재접속*, SC-12 는
*`LINGER_EXPIRED` 디스폰*, SC-13 은 *`SERVER_SHUTDOWN` 디스폰*이다.

**역방향 게이트는 이것을 못 잡는다** — *"판정된 번호가 계약에 존재하는가"* 를 묻는데
**SC-11·12·13 은 전부 존재한다.** 게이트는 초록이고 귀속은 거짓이다.
**번호의 존재만 보는 검사는 번호의 의미가 뒤바뀐 경우를 통과시킨다.**

순방향도 못 잡는다 — 그 행들은 다른 도구가 정상적으로 판정하고 있었다.

## 어디서 "지명"을 읽는가

계약이 이미 데이터를 들고 있다. 두 곳을 합쳐 읽는다:

1. **§1 항목 행** — 그 행 안에 나오는 모든 `*.py` 파일명(검증 항목 칸·검증 방법 칸 모두).
2. **§3.2 도구 표** — `| 스크립트 | 항목 |` 행. `SC-61~65` 같은 범위를 펼친다.

둘 중 하나라도 그 (도구, SC) 짝을 허용하면 통과다. **느슨한 쪽이 옳다** — 이 검사가 잡으려는
것은 *남의 번호*이지 *문서화 누락*이 아니고, 엄하게 걸면 사람이 검사를 끄게 된다.

    python tests/e2e/check_item_sources.py --contract <02_*.md> [--tools-dir tests/e2e]
    python tests/e2e/check_item_sources.py --selftest

종료 코드: 0 통과 / **1 출처 위반**(남의 번호로 verdict) / **3 미지명만** / 2 사용법 오류
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

# Rust 팔 파서 (architect R28 Q-1). 같은 폴더에 있고, **SC 번호를 라벨로 쓰지 않는다**
# — 출처 게이트가 자기 모듈을 위반으로 세지 않게.
import rust_case_refs as RCR

for _stream in (sys.stdout, sys.stderr):
    try:
        _stream.reconfigure(encoding="utf-8")
    except (AttributeError, OSError):
        pass

PY_NAME = re.compile(r"([A-Za-z_][A-Za-z0-9_]*\.py)")
SC_NUM = re.compile(r"SC-(\d+)")
# `SC-61~65` · `SC-82~84` 같은 범위.
SC_RANGE = re.compile(r"SC-(\d+)\s*[~-]\s*(\d+)")
# `SC-11/12/13` · `SC-70·71·72·73` · `SC-08/09/12/81` 처럼 **접두사를 한 번만 쓰는 열거.**
# 이것을 안 펼치면 첫 번호만 걸리고 나머지는 조용히 빠진다 — 실제 라벨의 절반이 이 형태다.
SC_LIST = re.compile(r"SC-(\d+)((?:\s*[/·,]\s*(?:SC-)?\d+)+)")
BARE_NUM = re.compile(r"\d+")
# 분모를 셀 때 **판정 수단이 지명됐는가**를 본다. `.py` 만 세면 이 레포 항목의 절반 이상이
# "미지명"으로 나오는데, 그것들은 지명이 없는 게 아니라 **수단이 파이썬이 아닌 것**이다.
MEANS = (
    re.compile(r"cargo\s+(test|fmt|clippy|run)"),
    re.compile(r"unity\s+test|EditMode|PlayMode"),
    re.compile(r"psql|SQL|select\s", re.I),
    re.compile(r"bots\s+(run|probe|resume|token|subjects)"),
    re.compile(r"dotnet\s+run|ContractsCodegen"),
    re.compile(r"\.mp4|영상|녹화|육안"),
    re.compile(r"/debug/stats|summary\.json|Profiler"),
    re.compile(r"grep"),
    re.compile("tools/bots|probe|봇 2대|봇 두 대"),
)
# verdict 를 내는 파이썬 라벨. `"item": "..."` 와 `item = f"..."` 두 형태를 본다.
ITEM_LABEL = re.compile(r'"item"\s*:\s*(?:f?")([^"]*)"|^\s*item\s*=\s*f?"([^"]*)"', re.M)
# 규칙 7 의 대상은 verdict 라벨이다 — 관측 표식은 위반이 아니다(R23).
OBSERVATION_MARKS = ("관측용", "참고", "참조")

CONTRACT_ROW = re.compile(r"^\|\s*\*{0,2}SC-(\d+)\*{0,2}\s*\|", re.M)
# **E-2 (architect R19)**: 도구가 없는 것이 정당한 항목(사람이 보는 육안·영상 항목)은 계약이
# 그 사실을 **명시**해야 한다. 명시가 없는 미지명은 *비어 있음*과 구별되지 않는다.
NO_TOOL_MARK = "도구 없음(사람 관찰)"
# 종료 코드: 0 통과 / **1 출처 위반** / **3 미지명만** / 2 사용법 오류.
# 이 레포엔 이미 `4 = 미검증(판정 기준 미지정)` 관례가 있으므로 새 규약이 아니다.
EXIT_SOURCE_VIOLATION = 1
EXIT_UNNAMED_ONLY = 3
# **분류 불가·항등식 깨짐 → 4** (architect R28 Q-3·Q-4). 판정을 시도하지 않았다는 뜻이고
# **exit 1(거짓 주장)·exit 3(문서 공백)과 섞으면 셋이 한 빨간불로 합쳐진다**(F-1).
EXIT_UNDECIDABLE = 4
# 기본 실행이 제외를 적을 때 같이 찍는 수. **이 수가 출력에 있어야 만기가 지났는지·
# 늘었는지가 그 자리에서 읽힌다**(architect R23). `--include-rust` 실행으로 갱신한다.
def expand(text: str) -> set[int]:
    """문자열 안의 SC 번호를 범위까지 펼쳐 모은다."""
    out: set[int] = set()
    for lo, hi in SC_RANGE.findall(text):
        lo_i, hi_i = int(lo), int(hi)
        if lo_i <= hi_i <= lo_i + 40:      # 오탈자로 1..9999 를 펼치지 않는다
            out.update(range(lo_i, hi_i + 1))
    for first, rest in SC_LIST.findall(text):
        out.add(int(first))
        out.update(int(n) for n in BARE_NUM.findall(rest))
    out.update(int(m) for m in SC_NUM.findall(text))
    return out


def contract_allowed(contract_text: str) -> dict[str, set[int]]:
    """도구 파일명 → 그 도구가 verdict 를 내도 되는 SC 번호."""
    allowed: dict[str, set[int]] = {}
    in_tools_table = False
    for line in contract_text.splitlines():
        if line.startswith("#"):
            # **어느 절을 읽고 있는지 추적한다.** 이것이 없으면 §9 이력 표의 행이
            # "도구 + SC 번호"를 함께 담고 있다는 이유만으로 **지명으로 읽힌다** —
            # 실제로 그랬고, 이력 한 줄을 추가하자 미지명 5건이 조용히 0 이 됐다.
            # **게이트가 자기 문서의 산문을 근거로 초록이 되는 형태다.**
            in_tools_table = line.lstrip("# ").startswith("3.2")
            continue
        if not line.lstrip().startswith("|"):
            continue
        tools = set(PY_NAME.findall(line))
        if not tools:
            continue
        m = CONTRACT_ROW.match(line.strip())
        if m:
            # §1 항목 행: 그 행이 지명한 도구는 **그 행의 번호**를 낼 수 있다.
            nums = {int(m.group(1))}
        elif in_tools_table:
            # §3.2 도구 표: 항목 칸의 번호(범위 포함) 전부.
            nums = expand(line)
        else:
            continue          # §9 이력·§7b 예시 등 — 지명이 아니다
        for t in tools:
            allowed.setdefault(t, set()).update(nums)
    return allowed


def emitted(tool_path: Path) -> set[int]:
    """그 도구가 **verdict 라벨로** 쓰는 SC 번호."""
    out: set[int] = set()
    for a, b in ITEM_LABEL.findall(tool_path.read_text(encoding="utf-8")):
        out |= expand(a or b)
    return out


def rust_check(contract_text: str, all_rs: dict) -> dict:
    """Rust 팔을 **케이스 이름**으로 계약 §1 에 잇는다 (architect R28 Q-2·Q-4·Q-5).

    **`PY_NAME` 경로를 타지 않는다** — 그것은 `.py` 만 잡으므로 `.rs` 는 원리적으로 지명될 수
    없다(그래서 옛 게이트에서 Rust 는 "지명 0" 상태로 검사됐다). 대신 계약 §1 행의 백틱 span 안
    토큰과 `as_str()` 의 CLI 이름을 **완전히 같을 때만** 잇는다.

    **미지명 verdict 팔은 exit 1 이다(3 이 아니다)** — §1 미지명(3)은 *계약이 도구를 안 적었다*는
    문서 공백이고, 미지명 verdict 팔은 *도구가 근거 없이 판정을 주장한다*는 **거짓 주장**이다.
    방향이 반대이고 후자는 "남의 번호"와 같은 무게다.
    """
    scen = all_rs.get("scenario.rs", "")
    main = all_rs.get("main.rs", "")
    if not scen:
        return {"skipped": "scenario.rs 가 없다"}
    ids = RCR.identities(scen, main, all_rs)
    names = RCR.case_names(scen)
    arms = RCR.contract_arms(scen)
    named = RCR.contract_named_cases(contract_text, set(names.values()))

    unclassifiable = [a["variant"] for a in arms if a["kind"] == "unclassifiable"]
    unqualified = [a["variant"] for a in arms if a["kind"] == "unqualified"]   # Q-8
    # **다섯 수 중 3 과 4 를 가른다** (리더 R29): 3 은 *계약이 그 케이스를 아예 안 적었다*,
    # 4 는 *적었는데 그 번호를 주지 않았다*. 합치면 **"몇 건이 위반이고 몇 건이 옳은 라벨인가"**
    # 가 출력에서 갈리지 않는다.
    unnamed_verdict_arms = []   # (케이스, [SC…]) — 계약 §1 에 그 케이스 이름이 없다
    violations = []             # (케이스, SC) — 이름은 있는데 그 번호를 안 줬다
    for a in arms:
        if a["kind"] != "verdict":
            continue
        case = names.get(a["variant"]) or a["variant"]
        if case not in named:
            unnamed_verdict_arms.append((case, a["sc"]))
            continue
        for n in a["sc"]:
            if n not in named[case]:
                violations.append((case, n))
    # Q-5 유령 지명: 계약이 백틱으로 적은 케이스 이름이 `as_str()` 에 없으면 위반
    ghosts = sorted(set(named) - set(names.values()))
    return {
        "identities": ids,
        "arms_total": len(arms),
        "verdict_arms": sum(1 for a in arms if a["kind"] == "verdict"),
        "reference_arms": sum(1 for a in arms if a["kind"] == "reference"),
        # Q-9: **여섯째 수.** `ContractRef` 명시로 종류가 정해진 팔 수. `< 팔 수` 면 exit 4 —
        # 대체 사슬(관측 표식 → 기본 verdict)은 **지우지 않는다.** 바꾸는 것은 그 단계가
        # **조용한 것**이고, 수 하나가 소리 나게 만든다.
        "explicit_arms": sum(1 for a in arms if a.get("decided_by") == "explicit"),
        "decided_by": {k: sum(1 for a in arms if a.get("decided_by") == k)
                       for k in ("explicit", "mark", "default", "unqualified")},
        "unclassifiable_arms": unclassifiable,
        "unqualified_arms": unqualified,
        "unnamed_verdict_arms": sorted(unnamed_verdict_arms),
        "contract_named_cases": {k: sorted(v) for k, v in sorted(named.items())},
        "violations": sorted(violations),
        "ghost_named_cases": ghosts,
    }


def rust_exit_code(res: dict) -> int:
    """항등식 → 분류 → 위반 **순서로** 본다.

    **항등식을 팔 루프보다 먼저 본다** — architect 시제품이 검사를 뒤에 뒀다가 깨진 입력에서
    `KeyError` 로 죽었고, **죽는 것과 exit 4 는 CI 에서 다르게 읽힌다.**
    """
    if res.get("skipped"):
        return 0
    if (not res["identities"]["ok"] or res["unclassifiable_arms"]
            or res["unqualified_arms"]
            # Q-9: 명시가 팔 수보다 적으면 **게이트가 무엇으로 판정했는지 모르는 상태**다.
            or res["explicit_arms"] < res["arms_total"]):
        return EXIT_UNDECIDABLE
    if res["violations"] or res["ghost_named_cases"] or res["unnamed_verdict_arms"]:
        return EXIT_SOURCE_VIOLATION
    return 0


def exit_code(bad: list, unmarked: set) -> int:
    """**빨간불 둘을 같은 코드로 내보내지 않는다** (architect R20 F-1).

    미지명 36 때문에 상시 exit 1 인 동안 **진짜 출처 위반이 새로 생겨도 종료 코드가 안 바뀐다** —
    CI·절차·사람 모두 "그 빨간불은 아는 것"으로 읽고 지나간다. **계약 §7b 규칙 5 와 글자 그대로
    같은 논리다**: 거기선 한 불리언이 여러 항목을 삼켰고 여기선 한 종료 코드가 두 실패 종류를
    삼킨다. 출처 위반이 더 무거우므로 둘 다면 1 이다.
    """
    if bad:
        return EXIT_SOURCE_VIOLATION
    if unmarked:
        return EXIT_UNNAMED_ONLY
    return 0


def coverage(contract_text: str) -> tuple[set[int], set[int], set[int]]:
    """(계약 전체 SC, 도구가 지명된 SC, **명시 없이 지명이 빠진 SC**).

    **이 검사 자신의 자명 통과 방어다** (architect R19 E-2). 출처 게이트는 §3.2 표와 §1 행이
    지명한 것만 볼 수 있으므로, **표가 비면 아무것도 못 보면서 exit 0 을 낸다** — 그 상태가
    *검사가 없는 상태*와 구분되지 않는다. 그래서 매 실행에 **분모를 찍고**, 지명이 빠진
    항목은 계약에 `도구 없음(사람 관찰)` 로 **명시돼 있어야** 한다. 명시 없는 미지명은 위반이다.

    **세 방향 중 이것만 자기 분모를 안 찍고 있었다** — 순·역방향은 "계약 항목 90 · 판정 행 N"
    을 찍는다.
    """
    all_sc: set[int] = set()
    named: set[int] = set()
    exempt: set[int] = set()
    other_means: set[int] = set()
    for line in contract_text.splitlines():
        s = line.strip()
        m = CONTRACT_ROW.match(s)
        if not m:
            continue
        n = int(m.group(1))
        all_sc.add(n)
        if NO_TOOL_MARK in s:
            exempt.add(n)
        elif any(rx.search(s) for rx in MEANS):
            # 파이썬이 아닌 판정 수단이 지명돼 있다. 출처 대조 대상은 아니지만 **분모에서는
            # 지명된 것으로 센다** — 세지 않으면 "지명이 비었다"와 구분되지 않는다.
            other_means.add(n)
    for tool_nums in contract_allowed(contract_text).values():
        named |= tool_nums
    unmarked = all_sc - named - exempt - other_means
    return all_sc, named | exempt | other_means, unmarked


def check(contract_text: str, tools: dict[str, str],
          refused: list[str] | None = None) -> list[tuple[str, int]]:
    """(도구, 남의 번호) 목록. 빈 목록이 통과다.

    `.rs` 는 **이 경로로 판정하지 않는다** — `refused` 에 이름만 담고 건너뛴다(R28 Q-2).
    """
    allowed = contract_allowed(contract_text)
    bad: list[tuple[str, int]] = []
    if refused is None:
        refused = []
    for name, source in sorted(tools.items()):
        used: set[int] = set()
        if name.endswith(".rs"):
            # **이 경로로 `.rs` 를 판정하지 않는다** (architect R28 Q-2). 옛 한 줄 정규식은
            # 17 팔 중 1 개만 보았고, `PY_NAME` 이 `.py` 만 잡아 **옳은 라벨이어도 지명될 수
            # 없다.** 그 상태로 통과시키면 `--tools-dir tools/bots/src` 가 **거짓 초록**을 낸다.
            # Rust 는 `rust_check()` 가 **케이스 이름**으로 판정한다.
            refused.append(name)
            continue
        for a, b in ITEM_LABEL.findall(source):
            used |= expand(a or b)
        for n in sorted(used - allowed.get(name, set())):
            bad.append((name, n))
    return bad



# 합성 소스 조립용. 모듈 수준에 두면 `_rs` 와 `rust_selftest` 가 같이 쓴다.
NL = chr(10)


def _rs(arms: list[tuple[str, str]], variants: list[str] | None = None,
        names: list[tuple[str, str]] | None = None) -> str:
    """합성 `scenario.rs`. `arms` = (변이, 팔 본문). 본문을 그대로 넣으므로 주석·다줄·복수
    리터럴을 **그 모양대로** 시험할 수 있다."""
    names = names or [(v, v.lower()) for v, _ in arms]
    variants = variants or [v for v, _ in arms]
    out = ["pub enum ProbeCase {"] + ["    %s," % v for v in variants] + ["}", ""]
    out += ["impl ProbeCase {", "    pub fn as_str(self) -> &'static str {", "        match self {"]
    out += ['            Self::%s => "%s",' % (v, n) for v, n in names]
    out += ["        }", "    }", "", "    pub fn contract_item(self) -> &'static str {",
            "        match self {"]
    for v, body in arms:
        out.append("            Self::%s => %s" % (v, body))
    out += ["        }", "    }", "}"]
    return NL.join(out) + NL


def rust_selftest() -> int:
    """**R28 Q-6 — 여덟 케이스가 고치기 전 입력에서 실제로 빨간불을 켜는가.**

    (a) 가 제일 중요하다 — **오늘 정규식이 놓치던 정확한 모양**(`=> {` + 주석 + 문자열)이고,
    이 케이스가 없으면 다음 회귀가 안 보인다.
    """
    S1 = "## 1. 검증 항목" + NL
    OUT = "## 7b. 자명 통과 시험" + NL
    row = lambda n, cell: "| SC-%d | 무엇 | %s | qa | AC-x | E6 |%s" % (n, cell, NL)
    multiline = '{' + NL + '                // 주석이 여기 들어간다' + NL         + '                "SC-24 (AC-5c): 범위 초과"' + NL + '            }'
    cases = []

    # (a) 다줄 + 주석 — 잡히는가. 주석을 안 지우면 이 팔의 리터럴이 안 보인다.
    src = _rs([("CheatRange", multiline)], names=[("CheatRange", "cheat-range")])
    arms = RCR.contract_arms(src)
    cases.append(("(a) `=> {` + 주석 + 문자열을 잡는다",
                  len(arms) == 1 and arms[0]["kind"] == "verdict" and arms[0]["sc"] == [24],
                  "팔=%s" % arms))
    # 같은 입력에서 **주석을 지우지 않으면** 리터럴이 하나 더 보인다(대조)
    naive = len(RCR.STRING_LIT.findall(src))
    stripped = len(RCR.STRING_LIT.findall(RCR.strip_line_comments(src)))
    cases.append(("(a') 주석 제거가 실제로 무언가를 지운다",
                  naive >= stripped, "naive=%d stripped=%d" % (naive, stripped)))

    # (b) verdict 팔에 남의 번호 → 위반
    src = _rs([("CheatRange", 'ContractRef::Verdict("SC-24/SC-99 (AC-5c): 범위 초과")')],
              names=[("CheatRange", "cheat-range")])
    res = rust_check(S1 + row(24, "`probe --case cheat-range`"), {"scenario.rs": src, "main.rs": ""})
    cases.append(("(b) verdict 팔의 남의 번호 → 위반",
                  ("cheat-range", 99) in res["violations"], str(res["violations"])))

    # (c) 같은 번호를 Reference 로 → 위반 아님
    src = _rs([("CheatRange", 'ContractRef::Reference("SC-24/SC-99 재현용")')],
              names=[("CheatRange", "cheat-range")])
    res = rust_check(S1 + row(24, "`probe --case cheat-range`"), {"scenario.rs": src, "main.rs": ""})
    cases.append(("(c) 같은 번호를 Reference 로 → 위반 없음",
                  res["violations"] == [] and res["reference_arms"] == 1, str(res)[:60]))

    # (d) 접두 누출 — 계약이 `cheat-range` 만 적을 때 `cheat-range-turn` 은 미지명
    src = _rs([("CheatRangeTurn", 'ContractRef::Verdict("SC-67 (AC-17b)")')],
              names=[("CheatRangeTurn", "cheat-range-turn")])
    res = rust_check(S1 + row(24, "`probe --case cheat-range`"), {"scenario.rs": src, "main.rs": ""})
    # 계약이 `cheat-range` 만 적었으므로 `cheat-range-turn` 은 **이름 자체가 미지명 = ③** 이다.
    # 부분문자열로 이었다면 여기서 ④ 도 ③ 도 비어 exit 0 이 났을 것이다.
    cases.append(("(d) 접두 누출 없음 — cheat-range 가 cheat-range-turn 을 지명하지 않는다",
                  res["unnamed_verdict_arms"] == [("cheat-range-turn", [67])]
                  and rust_exit_code(res) == EXIT_SOURCE_VIOLATION,
                  "③=%s ④=%s" % (res["unnamed_verdict_arms"], res["violations"])))

    # (e) 팔 수 != 변이 수 → 항등식 깨짐 → exit 4
    src = _rs([("CheatRange", 'ContractRef::Verdict("SC-24")')], variants=["CheatRange", "Orphan"],
              names=[("CheatRange", "cheat-range"), ("Orphan", "orphan")])
    res = rust_check(S1 + row(24, "`probe --case cheat-range`"), {"scenario.rs": src, "main.rs": ""})
    cases.append(("(e) 팔 없는 변이 → exit 4 (이름으로 인쇄)",
                  rust_exit_code(res) == EXIT_UNDECIDABLE
                  and res["identities"]["identity_1_variants_vs_arms"]["variants_without_arm"] == ["Orphan"],
                  str(res["identities"]["identity_1_variants_vs_arms"])))

    # (f) 백틱 **밖** 케이스 이름은 지명이 아니다
    src = _rs([("Binary", 'ContractRef::Verdict("SC-25 (AC-8b)")')], names=[("Binary", "binary")])
    res = rust_check(S1 + row(25, "binary 프레임도 같은 예산"), {"scenario.rs": src, "main.rs": ""})
    cases.append(("(f) 백틱 밖 `binary` 는 지명이 아니다",
                  res["unnamed_verdict_arms"] == [("binary", [25])]
                  and rust_exit_code(res) == EXIT_SOURCE_VIOLATION,
                  "③=%s ④=%s" % (res["unnamed_verdict_arms"], res["violations"])))

    # (g) 한 팔에 문자열 둘 → 분류 불가 → exit 4
    two = '{' + NL + '                ContractRef::Verdict("첫 리터럴")' + NL         + '                "SC-11 둘째"' + NL + '            }'
    src = _rs([("CheatRange", two)], names=[("CheatRange", "cheat-range")])
    res = rust_check(S1 + row(24, "`probe --case cheat-range`"), {"scenario.rs": src, "main.rs": ""})
    cases.append(("(g) 한 팔에 문자열 둘 → exit 4",
                  rust_exit_code(res) == EXIT_UNDECIDABLE and res["unclassifiable_arms"] == ["CheatRange"],
                  str(res["unclassifiable_arms"])))

    # (i) **Q-8 — 비수식 종류 토큰 → exit 4.** 합성 최소 입력이다(파일 전체를 fixture 로 박으면
    #     `scenario.rs` 가 바뀔 때마다 낡는다 — architect·리더 둘 다 짚었다).
    #     **이것은 실제로 일어난 모양이다**: `use ContractRef::{Reference, Verdict};` 한 줄이
    #     수식 적중을 0/17 로 만들었고, **위반도 0 이라 "위반 0 · exit 0" 이 인쇄될 수 있었다.**
    src = ("use ContractRef::{Reference, Verdict};" + NL
           + _rs([("CheatRange", 'Verdict("SC-24 (AC-5c): 범위 초과")')],
                 names=[("CheatRange", "cheat-range")]))
    res = rust_check(S1 + row(24, "`probe --case cheat-range`"), {"scenario.rs": src, "main.rs": ""})
    cases.append(("(i) Q-8 비수식 `Verdict(` → exit 4 (조용한 exit 0 이 아니다)",
                  rust_exit_code(res) == EXIT_UNDECIDABLE and res["unqualified_arms"] == ["CheatRange"],
                  "code=%s unqualified=%s" % (rust_exit_code(res), res["unqualified_arms"])))
    # (i') 같은 입력을 **수식**으로 고치면 통과한다 — 음성 대조. 없으면 (i) 가
    #      "무조건 exit 4" 와 구분되지 않는다.
    src_ok = _rs([("CheatRange", 'ContractRef::Verdict("SC-24 (AC-5c): 범위 초과")')],
                 names=[("CheatRange", "cheat-range")])
    res_ok = rust_check(S1 + row(24, "`probe --case cheat-range`"),
                        {"scenario.rs": src_ok, "main.rs": ""})
    cases.append(("(i') 수식으로 고치면 exit 0 — (i) 가 무조건 4 를 내는 게 아니다",
                  rust_exit_code(res_ok) == 0 and res_ok["verdict_arms"] == 1,
                  "code=%s" % rust_exit_code(res_ok)))

    # (j) ③ 과 ④ 를 가른다 — 계약이 케이스를 **아예 안 적은** 경우는 ③ 이다
    src = _rs([("TickBurst", 'ContractRef::Verdict("SC-89 (g)")')],
              names=[("TickBurst", "tick-burst")])
    res = rust_check(S1 + row(24, "`probe --case cheat-range`"), {"scenario.rs": src, "main.rs": ""})
    cases.append(("(j) 계약이 케이스를 안 적었으면 ③(미지명 verdict 팔) — ④ 가 아니다",
                  res["unnamed_verdict_arms"] == [("tick-burst", [89])] and res["violations"] == [],
                  "③=%s ④=%s" % (res["unnamed_verdict_arms"], res["violations"])))

    # (k) **Q-9 — `ContractRef` 명시가 없으면 exit 4.** 기본값 verdict 는 남겨 두지만
    #     **조용하지 않게** 만든다. 항등식 ① 은 이 팔을 못 잡는다(팔 수는 17 그대로다).
    src = _rs([("CheatRange", '"SC-24 (AC-5c): 범위 초과"')],
              names=[("CheatRange", "cheat-range")])
    res = rust_check(S1 + row(24, "`probe --case cheat-range`"), {"scenario.rs": src, "main.rs": ""})
    cases.append(("(k) Q-9 ContractRef 없는 팔 → exit 4 (기본 verdict 로 조용히 통과하지 않는다)",
                  rust_exit_code(res) == EXIT_UNDECIDABLE
                  and res["explicit_arms"] == 0 and res["decided_by"]["default"] == 1
                  and res["identities"]["ok"],      # ① 은 성립한다 — 그래서 여섯째 수가 필요하다
                  "code=%s 명시=%s/%s ①=%s" % (rust_exit_code(res), res["explicit_arms"],
                                               res["arms_total"], res["identities"]["ok"])))

    # (l) **architect 가 실측한 정확한 구멍**: verdict 주장 하나가 `참고:` 두 글자로 면제되고
    #     항등식은 성립하고 출력에 표시가 없던 경로. 이제 exit 4 다.
    src = _rs([("TickBurst", '"참고: SC-89 (g) 재현용"')], names=[("TickBurst", "tick-burst")])
    res = rust_check(S1 + row(89, "`probe --case tick-burst`"), {"scenario.rs": src, "main.rs": ""})
    cases.append(("(l) Q-9 `참고:` 산문만으로 reference 가 된 팔 → exit 4",
                  rust_exit_code(res) == EXIT_UNDECIDABLE
                  and res["decided_by"]["mark"] == 1 and res["identities"]["ok"],
                  "code=%s 표식=%s ①=%s" % (rust_exit_code(res), res["decided_by"]["mark"],
                                            res["identities"]["ok"])))

    # (h) §1 **밖** 항목행은 지명하지 않는다
    src = _rs([("CheatRange", 'ContractRef::Verdict("SC-24 (AC-5c)")')], names=[("CheatRange", "cheat-range")])
    res = rust_check(OUT + row(24, "`probe --case cheat-range`"), {"scenario.rs": src, "main.rs": ""})
    cases.append(("(h) §1 밖 `| SC-24 |` 행은 지명이 아니다",
                  res["unnamed_verdict_arms"] == [("cheat-range", [24])]
                  and rust_exit_code(res) == EXIT_SOURCE_VIOLATION,
                  "③=%s ④=%s" % (res["unnamed_verdict_arms"], res["violations"])))

    failures = 0
    for name, ok, detail in cases:
        print("%s [Rust] %s%s" % ("OK  " if ok else "FAIL", name, "" if ok else " :: " + detail))
        if not ok:
            failures += 1
    return failures


def selftest() -> int:
    """**규칙 6 을 이 검사 자신에게 적용한다** — 라벨을 일부러 어긋나게 한 입력에서
    실제로 걸리는가, 그리고 맞는 입력을 통과시키는가(음성 대조)."""
    contract = (
        "## 1. 검증 항목\n"
        "| SC-11 | 잔류 창 안 재접속 | `bots resume` + `resume_check.py` | qa | AC-3 | E6 |\n"
        "| SC-79 | sequence 빈틈 | p0-02 SQL 그대로 | qa | AC-20(d) | E2 |\n"
        "| SC-61 | A 가 움직이면 B 가 본다 | B 의 CSV | qa | AC-16(a) | E6 |\n"
        # **§3.2 제목이 있어야 그 아래 표가 지명으로 읽힌다** (절 추적 도입 뒤의 요건)
        "### 3.2 e2e 스크립트 확장\n"
        "| `check_sequence_gaps.py` · `append_only_probe.py` | SC-79, SC-82~84 | 재사용 |\n"
        "| `two_client_view.py` (신규) | SC-61~65 | 두 CSV 조인 |\n"
    )
    cases = [
        (
            "위반 — append_only_probe 가 SC-11 로 verdict 를 낸다 (실제 사례)",
            {"append_only_probe.py": '"item": "SC-11/12/13 (AC-4) append-only"'},
            [("append_only_probe.py", 11), ("append_only_probe.py", 12),
             ("append_only_probe.py", 13)],
        ),
        (
            "깨끗 — §3.2 표가 지명한 번호",
            {"check_sequence_gaps.py": '"item": "SC-79 (AC-20d) sequence 빈틈 검사"'},
            [],
        ),
        (
            "깨끗 — §1 행이 그 도구를 지명한다",
            {"resume_check.py": '"item": "SC-11 재개 적분 연속성"'},
            [],
        ),
        (
            "깨끗 — 범위 표기를 펼친다 (SC-61~65 안의 63)",
            {"two_client_view.py": '"item": "SC-63 봇 2대 원시 대조"'},
            [],
        ),
        (
            "위반 — 범위 밖 (SC-61~65 인데 SC-70 을 낸다)",
            {"two_client_view.py": '"item": "SC-70 스냅샷 손실"'},
            [("two_client_view.py", 70)],
        ),
        (
            "`item = f\"…\"` 형태도 본다",
            {"poll_until.py": 'item = f"SC-62 (AC-17b) 마지막 봇 종료"'},
            [("poll_until.py", 62)],
        ),
        (
            "SC 번호가 없는 라벨은 아무것도 주장하지 않는다",
            {"poll_until.py": '"item": "계약 외 검사 — 대기 헬퍼"'},
            [],
        ),
    ]
    # -- E-3 (architect R19): **E-2 의 방어 자신에 규칙 6 을 돌린다.**
    #    지명이 사라진 입력에서 실제로 걸리는가, 그리고 명시가 있으면 통과하는가.
    row_named = '| SC-11 | 재접속 | `resume_check.py` | qa | AC-3 | E6 |' + chr(10)
    cov_full = row_named + '| SC-59 | 육안 부호 관찰 | mp4 4개 촬영 | client | AC-14 | E3 |' + chr(10)
    cov_missing = row_named + '| SC-59 | 부호 관찰 | 사람이 본다 | client | AC-14 | E3 |' + chr(10)
    cov_marked = row_named + '| SC-59 | 부호 관찰 | ' + NO_TOOL_MARK + ' | client | AC-14 | E3 |' + chr(10)
    # 수단 표기별 대조. **분모를 넓힐 때마다 여기 한 줄을 늘린다** — R21 뒤 `grep` 과
    # `tools/bots probe` 를 분모가 못 읽어 7건이 미지명으로 잘못 남았고, 그것은 **계약의
    # 누락이 아니라 분모를 세는 쪽의 결함**이었다. 대조가 없으면 다음 표기에서 또 같은 일이
    # 일어나고, 그때도 "계약이 안 적었다"로 읽힌다.
    def _row(sc, method):
        return f"| SC-{sc} | 무엇을 재는가 | {method} | qa | AC-x | E6 |" + chr(10)
    means_cases = [
        ("cargo", _row(16, "`cargo test -p starfall-sim` 해당 테스트"), set()),
        ("unity EditMode", _row(54, "`unity test client --mode EditMode`"), set()),
        ("grep", _row(3, "**grep 이 판정한다**(정밀 grep + 양성 대조)"), set()),
        ("봇 probe", _row(23, "`tools/bots` 의 `probe` 시나리오 `cheat-position`"), set()),
        ("봇 2대", _row(25, "`tools/bots` 두 대(20 Hz vs 200 Hz)의 이동 거리 대조"), set()),
        ("SQL", _row(57, "SQL(ship_id 기준)"), set()),
        ("수단이 없다", _row(99, "무엇을 재는지만 적혀 있다"), {99}),
    ]
    cov_cases = [
        ('지명이 다 있으면 미지명 0', cov_full, set()),
        ('지명이 사라지면 그 항목이 드러난다', cov_missing, {59}),
        ('명시가 있으면 통과', cov_marked, set()),
    ] + means_cases + [
        # **§9 이력 표를 지명으로 읽지 않는다.** 이력 행은 도구 이름과 SC 번호를 같이 담으므로,
        # 절을 안 보면 **이력 한 줄을 적는 것만으로 미지명이 사라진다** — 실제로 그랬다.
        ("§9 이력 행은 지명이 아니다",
         "## 1. 검증 항목" + chr(10)
         + "| SC-99 | 무엇을 재는가 | 수단이 없다 | qa | AC-x | E6 |" + chr(10)
         + "## 9. 계약 변경 이력" + chr(10)
         + "| 2026-09-25 (20차) | `check_item_sources.py` 로 SC-99 를 확인했다 | 근거 |" + chr(10),
         {99}),
    ]
    failures = 0
    for name, text, expected in cov_cases:
        _all, _cov, unmarked = coverage(text)
        ok = unmarked == expected
        print(f"{'OK  ' if ok else 'FAIL'} [분모] {name} :: 미지명={sorted(unmarked)} 기대={sorted(expected)}")
        if not ok:
            failures += 1

    # -- F-1 (architect R20): **종료 코드가 두 실패를 가르는가.** 이 대조가 없으면
    #    "출력에서는 구분된다"가 "코드에서도 구분된다"로 읽힌다 — 그 둘은 다르다.
    code_cases = [
        ("출처 위반 → 1", [("x.py", 11)], set(), EXIT_SOURCE_VIOLATION),
        ("미지명만 → 3", [], {59}, EXIT_UNNAMED_ONLY),
        ("둘 다 → 1 (출처 위반이 더 무겁다)", [("x.py", 11)], {59}, EXIT_SOURCE_VIOLATION),
        ("둘 다 없음 → 0", [], set(), 0),
    ]
    for name, bad_in, unmarked_in, want in code_cases:
        got_code = exit_code(bad_in, unmarked_in)
        ok = got_code == want
        print(f"{'OK  ' if ok else 'FAIL'} [종료코드] {name} :: {got_code} 기대={want}")
        if not ok:
            failures += 1

    for name, tools, expected in cases:
        got = check(contract, tools)
        ok = got == expected
        print(f"{'OK  ' if ok else 'FAIL'} {name} :: 잡은 것={got} 기대={expected}")
        if not ok:
            failures += 1
    failures += rust_selftest()
    total = len(cases) + len(cov_cases) + len(code_cases) + 14
    print(f"selftest: {'PASS' if failures == 0 else f'FAIL ({failures})'}  케이스={total} (출처 {len(cases)} + 분모 {len(cov_cases)} + 종료코드 {len(code_cases)} + Rust 14)")
    return 0 if failures == 0 else 1


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--contract")
    ap.add_argument("--tools-dir", action="append", default=None,
                    help="여러 번 줄 수 있다. 기본값: tests/e2e 와 tools/bots/src")
    ap.add_argument("--no-rust", action="store_true",
                    help="Rust 판정을 끈다. 끄면 항등식도 안 찍힌다 — 그 사실이 출력에 남는다")
    ap.add_argument("--include-rust", action="store_true",
                    help="tools/bots/src 의 Rust 라벨도 훑는다(규칙 8 만기 항목, 기본 꺼짐)")
    ap.add_argument("--selftest", action="store_true")
    args = ap.parse_args()

    if args.selftest:
        return selftest()
    if not args.contract:
        ap.print_usage()
        print("오류: --contract 또는 --selftest 가 필요하다", file=sys.stderr)
        return 2

    contract_text = Path(args.contract).read_text(encoding="utf-8")
    # **Rust 는 이제 별도 경로로 판정한다** (architect R28 Q-2·Q-7). `.rs` 는 `PY_NAME`
    # (`.py` 만 잡는다) 로 지명될 수 없으므로 `dirs` 에 넣어 같은 검사를 돌릴 수 없다 —
    # **케이스 이름**으로 잇는 `rust_check()` 가 그 일을 한다. `dirs` 는 파이썬 전용이다.
    dirs = args.tools_dir or ["tests/e2e"]
    tools: dict[str, str] = {}
    for d in dirs:
        base = Path(d)
        for f in sorted(list(base.glob("*.py")) + list(base.glob("*.rs"))):
            if f.name == Path(__file__).name:
                continue
            tools[f.name] = f.read_text(encoding="utf-8")
    refused: list[str] = []
    bad = check(contract_text, tools, refused)

    # ── Rust (architect R28 Q-7). **기본으로 켠다** — rust 의 라벨 강등(R-1·R-2)이 들어온 것을
    #    실행으로 확인했다(verdict 6 / reference 11). `--no-rust` 로 끌 수 있지만 **두 항등식은
    #    항상 찍는다** — 끄는 것과 안 적는 것은 다르다(R23).
    # **`--no-rust` 여도 두 항등식은 찍는다** (architect R28 Q-7). 끄는 것이 *판정*을 끄는 것이지
    # *보고*를 끄는 것이 아니다 — 안 찍으면 "끈 것"과 "볼 게 없던 것"이 같은 출력이 된다.
    rs_dir = Path("tools/bots/src")
    all_rs = (
        {f.name: f.read_text(encoding="utf-8") for f in sorted(rs_dir.glob("*.rs"))}
        if rs_dir.is_dir() else {}
    )
    rust_res = rust_check(contract_text, all_rs) if all_rs else {"skipped": "tools/bots/src 가 없다"}
    rust_code = 0 if args.no_rust else rust_exit_code(rust_res)

    allowed = contract_allowed(contract_text)
    all_sc, covered, unmarked = coverage(contract_text)
    # **분모를 먼저 찍는다.** 이 줄이 없으면 "지명이 하나도 없어서 볼 게 없었다"와
    # "전부 지명됐고 위반이 없다"가 같은 출력이 된다.
    print(f"계약 항목: {len(all_sc)}")
    print(f"도구가 지명된 항목: {len(covered)} / {len(all_sc)}  (도구 {len(allowed)}개)")
    kinds: dict[str, int] = {}
    for n in tools:
        kinds[n.rsplit(".", 1)[-1]] = kinds.get(n.rsplit(".", 1)[-1], 0) + 1
    print(f"검사한 도구: {len(tools)} ({', '.join(f'{k} {v}' for k, v in sorted(kinds.items()))})")
    # **무엇을 훑지 않았는지를 적는다** (architect R23). 끄는 선택은 유지하되 **제외가 명시적으로
    # 비어 있게** 만든다 — `도구 없음(사람 관찰)` 이 빈칸과 다른 것과 같은 이치다.
    # **개수를 같이 찍는 것이 핵심이다**: 만기가 지났는지, 수가 늘었는지가 그 자리에서 읽힌다.
    # qa r13: 여기도 `and not args.tools_dir` 가 붙어 있었다 — `--tools-dir` 를 주면
    # **제외 안내가 사라져** 그 실행의 출력만 보는 사람은 Rust 가 범위에 있었다고 읽는다.
    # 끄는 것과 가리는 것은 다르다(이 파일이 §3.2 빈 표에서 스스로 진단한 상태다).
    if refused:
        print("**거부**: `.rs` %d개는 이 경로로 판정하지 않는다 — %s (Rust 는 케이스 이름으로 "
              "판정한다. R28 Q-2)" % (len(refused), ", ".join(refused)))
        print("   ⚠ **검사해 달라고 받은 것을 검사하지 못했으므로 exit 4 다** — "
              "받은 파일을 건너뛰고 `위반 없음`을 인쇄하면 **11개를 주고 0개를 검사한 실행이 "
              "통과로 읽힌다.**")
    if args.no_rust:
        print("**Rust**: `--no-rust` — **판정을 끈 것이고 보고를 끈 것이 아니다.** "
              "아래 항등식은 그대로 찍히지만 **exit 코드는 Rust 위반에 대해 아무것도 말하지 않는다.**")
    if rust_res:
        if rust_res.get("skipped"):
            print("**Rust**: " + str(rust_res["skipped"]))
        else:
            ids = rust_res["identities"]
            i1 = ids["identity_1_variants_vs_arms"]
            i2 = ids["identity_2_sc_lines"]
            # **항등식은 항상 찍는다.** 이 두 줄이 없으면 exit 0 이 "라벨이 옳다"로 읽힌다.
            extra1 = ""
            if i1["variants_without_arm"]:
                extra1 += "  팔 없는 변이=" + str(i1["variants_without_arm"])
            if i1["arms_without_variant"]:
                extra1 += "  변이 없는 팔=" + str(i1["arms_without_variant"])
            print("Rust 항등식 ① 변이 %d == 팔 %d: %s%s"
                  % (i1["variants"], i1["arms"], "OK" if i1["ok"] else "FAIL", extra1))
            print("Rust 항등식 ② 주석 아닌 SC- 줄 %d == 판정 팔 %d + 도움말 %d: %s"
                  % (i2["non_comment_sc_lines_all_rs"], i2["contract_item_arms"],
                     i2["main_usage_sc_lines"], "OK" if i2["ok"] else "FAIL"))
            print("Rust 팔: verdict %d · reference %d · 분류 불가 %d%s · **비수식 %d**%s"
                  % (rust_res["verdict_arms"], rust_res["reference_arms"],
                     len(rust_res["unclassifiable_arms"]),
                     " " + str(rust_res["unclassifiable_arms"]) if rust_res["unclassifiable_arms"] else "",
                     len(rust_res["unqualified_arms"]),
                     " " + str(rust_res["unqualified_arms"]) if rust_res["unqualified_arms"] else ""))
            # **3 과 4 를 따로 찍는다** — 합치면 위반과 옳은 라벨이 안 갈린다.
            db = rust_res["decided_by"]
            print("Rust **ContractRef 명시 팔: %d / %d**  (표식 %d · 기본 %d · 비수식 %d)"
                  % (rust_res["explicit_arms"], rust_res["arms_total"],
                     db["mark"], db["default"], db["unqualified"]))
            if rust_res["explicit_arms"] < rust_res["arms_total"]:
                print("   ⚠ **명시가 팔 수보다 적다 → exit 4.** 산문(`참고:` 등)으로 종류가 정해진 팔은")
                print("   **항등식 ①에 안 걸린다** — ①은 *팔 수*를 세고 *무엇이 종류를 정했는가*는 세지 않는다.")
            print("Rust ③ 미지명 verdict 팔 %d · ④ .rs 출처 위반 %d · ⑤ 유령 지명 %d"
                  % (len(rust_res["unnamed_verdict_arms"]), len(rust_res["violations"]),
                     len(rust_res["ghost_named_cases"])))
            if rust_res["unnamed_verdict_arms"]:
                print()
                print("!! 위반 ③ — **계약 §1 이 그 케이스 이름을 아예 적지 않았다**(verdict 주장 근거 0):")
                for case, scs in rust_res["unnamed_verdict_arms"]:
                    print("   %s  ->  SC-%s" % (case, ", SC-".join(str(n) for n in scs)))
            if rust_res["unqualified_arms"]:
                print()
                print("!! 비수식 종류 토큰 — **`ContractRef::` 없이 `Verdict(`/`Reference(`** (R28 Q-8).")
                print("   " + ", ".join(rust_res["unqualified_arms"]))
                print("   정규식을 넓히지 않는다 — 맨 토큰은 **나중에 들어올 다른 enum 이 공급할 수 있다.**")
            if rust_res["violations"]:
                print()
                print("!! 위반 — **근거 없이 판정을 주장하는 Rust 팔** (계약 §1 이 그 케이스에 그 번호를 주지 않았다):")
                for case, n in rust_res["violations"]:
                    print("   %s  ->  SC-%d" % (case, n))
            if rust_res["ghost_named_cases"]:
                print()
                print("!! 위반 — **유령 지명** (계약이 백틱으로 적은 케이스가 as_str() 에 없다):")
                print("   " + ", ".join(rust_res["ghost_named_cases"]))
            if rust_res["unclassifiable_arms"]:
                print()
                print("!! 분류 불가 팔 — 한 팔에 문자열이 1개가 아니다. **첫 리터럴만 읽으면")
                print("   둘째의 SC 번호가 안 보이고 exit 0 이 난다**(R28 Q-6 (g)).")

    if unmarked:
        print()
        print(f"!! 위반 — 도구 지명도 `{NO_TOOL_MARK}` 명시도 없는 항목 {len(unmarked)}건:")
        print("   " + ", ".join(f"SC-{n}" for n in sorted(unmarked)))
        print()
        print("   지명이 없으면 이 검사는 그 항목에 대해 아무것도 볼 수 없고,")
        print("   그 상태는 검사가 없는 상태와 구분되지 않는다. 사람이 보는 항목이면")
        print(f"   계약 행에 `{NO_TOOL_MARK}` 을 적어 **명시적으로 비어 있게** 만든다.")
    if bad:
        print()
        print("!! 위반 — 계약이 지명하지 않은 번호로 verdict 를 내는 도구:")
        for name, n in bad:
            print(f"   {name}  ->  SC-{n}")
        print()
        print("번호의 존재만 보는 역방향 게이트는 이것을 통과시킨다 — 그래서 이 검사가 있다.")
    code = exit_code(bad, unmarked)
    # **무거운 쪽이 이긴다**: 4(판정 불가) > 1(거짓 주장) > 3(문서 공백).
    if refused:
        code = EXIT_UNDECIDABLE
    if rust_code == EXIT_UNDECIDABLE or code == EXIT_UNDECIDABLE:
        code = EXIT_UNDECIDABLE
    elif rust_code == EXIT_SOURCE_VIOLATION or code == EXIT_SOURCE_VIOLATION:
        code = EXIT_SOURCE_VIOLATION
    if code == EXIT_SOURCE_VIOLATION:
        print()
        print("종료 코드 1 = **출처 위반**. 미지명(코드 3)과 다른 코드다 — 섞으면 상시 빨간불이")
        print("새 위반을 가린다(architect R20 F-1, 계약 §7b 규칙 5 와 같은 논리).")
    elif code == EXIT_UNNAMED_ONLY:
        print()
        print("종료 코드 3 = **미지명만**(출처 위반 0). 계약 §7b 규칙 8 의 만기 항목이다.")
    else:
        print("위반 없음 — 모든 verdict 라벨이 계약이 지명한 번호이고, 미지명 항목이 전부 명시돼 있다.")
    return code


if __name__ == "__main__":
    sys.exit(main())
