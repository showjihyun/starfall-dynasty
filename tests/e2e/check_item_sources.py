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
CONTRACT_ROW = re.compile(r"^\|\s*\*{0,2}SC-(\d+)\*{0,2}\s*\|", re.M)
# **E-2 (architect R19)**: 도구가 없는 것이 정당한 항목(사람이 보는 육안·영상 항목)은 계약이
# 그 사실을 **명시**해야 한다. 명시가 없는 미지명은 *비어 있음*과 구별되지 않는다.
NO_TOOL_MARK = "도구 없음(사람 관찰)"
# 종료 코드: 0 통과 / **1 출처 위반** / **3 미지명만** / 2 사용법 오류.
# 이 레포엔 이미 `4 = 미검증(판정 기준 미지정)` 관례가 있으므로 새 규약이 아니다.
EXIT_SOURCE_VIOLATION = 1
EXIT_UNNAMED_ONLY = 3
# 기본 실행이 제외를 적을 때 같이 찍는 수. **이 수가 출력에 있어야 만기가 지났는지·
# 늘었는지가 그 자리에서 읽힌다**(architect R23). `--include-rust` 실행으로 갱신한다.
RUST_KNOWN_MISMATCHES = 8
RUST_KNOWN_LIST = "SC-14·19·20·21·24·25·26·67"
# **규칙 7 의 대상은 verdict 라벨이다** (architect R23). 로그가 관련 항목을 *가리키는*
# 표식은 금지가 아니라 표시의 문제다 — 이것을 위반으로 잡으면 게이트가 "로그에서 SC
# 번호를 전부 빼라"는 압력을 만들고, 그러면 라벨 불일치와 함께 **증거에서 항목으로
# 가는 길도 사라진다**. 고치는 것이 재는 것을 망가뜨리는 형태다.
OBSERVATION_MARKS = ("관측용", "참고", "참조")
# 분모를 셀 때 **판정 수단이 지명됐는가**를 본다. `.py` 만 세면 이 레포 항목의 절반 이상이
# "미지명"으로 나오는데, 그것들은 지명이 없는 게 아니라 **수단이 파이썬이 아닌 것**이다
# (server 게이트는 `cargo test`, client 는 `unity test`·EditMode, 기록 무결성은 SQL).
# 출처 **대조**는 파이썬 도구에만 걸리지만, 출처 **분모**는 수단 전체를 세야 뜻이 있다.
MEANS = (
    re.compile(r"cargo\s+(test|fmt|clippy|run)"),
    re.compile(r"unity\s+test|EditMode|PlayMode"),
    re.compile(r"psql|SQL|select\s", re.I),
    re.compile(r"bots\s+(run|probe|resume|token|subjects)"),
    re.compile(r"dotnet\s+run|ContractsCodegen"),
    re.compile(r"\.mp4|영상|녹화|육안"),
    re.compile(r"/debug/stats|summary\.json|Profiler"),
    # R21 이후: architect 가 쓴 수단 표기 둘을 분모가 못 읽고 있었다 — **분모를 세는
    # 쪽의 결함이지 계약의 누락이 아니었다.** 14건 중 7건이 이 형태였다.
    re.compile(r"grep"),
    re.compile("tools/bots|probe|봇 2대|봇 두 대"),
)
# verdict 를 내는 라벨. `"item": "..."` 와 `item = f"..."` 두 형태를 본다.
ITEM_LABEL = re.compile(r'"item"\s*:\s*(?:f?")([^"]*)"|^\s*item\s*=\s*f?"([^"]*)"', re.M)
# **Rust 도구도 SC 라벨을 출력한다** — `tools/bots/src/scenario.rs` 의 probe 설명 문자열이
# 그것이다. 파이썬만 훑으면 그 라벨은 영원히 안 보이고, **안 보이는 것은 검사가 없는 것과
# 같다**(규칙 7 주석의 분모 논리). 주석(`//`)은 제외한다 — 주석은 verdict 를 인쇄하지 않는다.
RUST_LABEL = re.compile(r'^\s*(?!//)[^\n]*?=>\s*"([^"]*SC-\d[^"]*)"', re.M)


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


def check(contract_text: str, tools: dict[str, str]) -> list[tuple[str, int]]:
    """(도구, 남의 번호) 목록. 빈 목록이 통과다."""
    allowed = contract_allowed(contract_text)
    bad: list[tuple[str, int]] = []
    for name, source in sorted(tools.items()):
        used: set[int] = set()
        if name.endswith(".rs"):
            for lbl in RUST_LABEL.findall(source):
                if any(mark in lbl for mark in OBSERVATION_MARKS):
                    continue          # verdict 가 아님이 문자열 안에 있다
                used |= expand(lbl)
        else:
            for a, b in ITEM_LABEL.findall(source):
                used |= expand(a or b)
        for n in sorted(used - allowed.get(name, set())):
            bad.append((name, n))
    return bad


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
        (
            "관측 표식은 위반이 아니다 — `관측용` 이 문자열 안에 있다 (architect R23)",
            {"scenario.rs": '    Self::Fly => "SC-70 관측용: 스냅샷 주기 전송",'},
            [],
        ),
        (
            "같은 파일의 **verdict 라벨**은 잡는다 — 표식이 없으면 주장이다",
            {"scenario.rs": '    Self::Binary => "SC-70 (AC-8b): 바이너리 프레임도 같은 예산",'},
            [("scenario.rs", 70)],
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
    total = len(cases) + len(cov_cases) + len(code_cases)
    print(f"selftest: {'PASS' if failures == 0 else f'FAIL ({failures})'}  케이스={total} (출처 {len(cases)} + 분모 {len(cov_cases)} + 종료코드 {len(code_cases)})")
    return 0 if failures == 0 else 1


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--contract")
    ap.add_argument("--tools-dir", action="append", default=None,
                    help="여러 번 줄 수 있다. 기본값: tests/e2e 와 tools/bots/src")
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
    # **Rust 훑기는 아직 기본값이 아니다 (규칙 8 만기 항목).** `scenario.rs` 의 probe 설명
    # 문자열이 **p0-02 번호 11개**를 쓰고 있어, 지금 기본으로 켜면 게이트가 **상시 exit 1** 이
    # 되고 **새로 생기는 파이썬 위반을 가린다** — F-1 이 막으려던 바로 그 형태다. 라벨을 고치고
    # §3.1(봇 하네스)의 지명을 게이트가 읽게 만든 뒤 기본값으로 올린다.
    dirs = args.tools_dir or ["tests/e2e"]
    if args.include_rust and not args.tools_dir:
        dirs = dirs + ["tools/bots/src"]
    tools: dict[str, str] = {}
    for d in dirs:
        base = Path(d)
        for f in sorted(list(base.glob("*.py")) + list(base.glob("*.rs"))):
            if f.name == Path(__file__).name:
                continue
            tools[f.name] = f.read_text(encoding="utf-8")
    bad = check(contract_text, tools)

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
    if not args.include_rust and not args.tools_dir:
        print(f"**제외**: Rust 도구(`tools/bots/src`)는 이 실행에서 검사하지 않았다 — "
              f"알려진 라벨 불일치 **{RUST_KNOWN_MISMATCHES}건**, "
              f"계약 §7b 규칙 8 만기(블록 8 실행 전). "
              f"번호: {RUST_KNOWN_LIST}. `--include-rust` 로 본다.")
    # **검사하지 않은 것을 적는다** (architect R23). `--include-rust` 기본 꺼짐의 근거는
    # "켜면 상시 exit 1 이 되어 새 파이썬 위반을 가린다" 였는데, **F-1 의 해법은 끄는 것이
    # 아니라 가르는 것**이었다(exit 1 / exit 3). 끄기만 하면 가려지는 정도가 아니라
    # **사라진다** — 기본 출력에 Rust 가 한 글자도 없으면 `85/90` 을 읽는 사람은 그게 전부라고
    # 읽는다. 이 검사 자신이 §3.2 빈 표에서 진단한 상태다.
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
