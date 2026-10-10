"""계약 §1 방법 칸이 지명한 테스트를 **계약 명령 그대로** 실행해, 필터마다 실행 수와 잡힌 테스트의 주인 SC 를
찍는다. **준비·집계 도구이고 verdict 를 내지 않는다**(판정은 리포트가 계약 문구로 한다). 단, 실행 0 인 이름은
"유령" 으로 따로 찍고 종료 코드 1 을 낸다 — "테스트가 없다" 와 "테스트가 초록이다" 를 섞지 않는다.

    python tests/e2e/cargo_sc_map.py scan     --contract <계약>                       # 추출만(cargo 없음)
    python tests/e2e/cargo_sc_map.py run      --contract <계약> --out <디렉터리>       # 필터마다 cargo test
    python tests/e2e/cargo_sc_map.py map      --contract <계약> --log <cargo 로그>     # 로그 한 벌과 대조(옛 방식)
    python tests/e2e/cargo_sc_map.py selftest

추출 규칙 (p1-02 r1~r3 결함 3건을 고친 판, `04_qa_report_r3.md` §3a):
  1. **맨 이름** — 방법 칸의 백틱 안에 `cargo test -p` 없이 이름만 적힌 토큰도 줍는다. 코드 색인과 대조해
     TEST_FN(테스트 함수) · NON_TEST_IDENT(필드·지표·메시지 이름) · NON_CODE(명시 목록) · GHOST(코드 어디에도
     없음) 로 나눈다. Rust TEST_FN 은 정의 위치의 Cargo.toml 로 크레이트·워크스페이스를 정해 실행 목록에 넣는다.
     Unity TEST_FN 은 cargo 로 돌릴 수 없으므로 `--unity-xml` 이 있으면 그 결과에서 찾는다.
  2. **값을 받는 플래그** — `--test X`·`--features X`·`--bin X` 따위의 값을 필터로 읽지 않는다.
     `--` 뒤의 libtest 인자(`--exact`·`--skip X`)도 따로 둔다.
  3. **열 밀림** — 표 칸은 이스케이프되지 않은 `|` 로만 나눈다(`\\|` 가 든 grep 명령이 앞 칸에 있으면 방법 칸이
     밀린다 — p1-02 계약에서 7 행). 줄 끝 CR 은 지운다. 실행은 셸을 거치지 않고 `subprocess` 인자 목록으로
     넘긴다 — TSV·CRLF·IFS 를 거치는 길 자체가 없다.

주인 SC: 필터가 잡은 테스트마다, **같은 크레이트의 다른 SC 필터**가 그 테스트를 잡는지 본다(cargo 와 같은
부분 문자열 규칙). 잡으면 `also_named_by` 에 그 SC 를 적는다. 이름 없는 필터(`--release` 전체 · `--test X` 전체)는
대상 전체를 돌리므로 주인 대조에서 뺀다(`scope = whole`).

역할(D-1, p1-03 r1 의 SC-40 유령 지명): 지명된 Rust 테스트마다 역할을 **계약 산문이 아니라 Rust 소스
속성**에서 읽는다 — `#[ignore]` 와 **부모의 spawn 리터럴**이 토큰이다(둘 다 오타가 조용히 면제로 바뀌지
않는다: 속성은 cargo 가 거부하고, 리터럴은 실행 인자라서 틀리면 부모가 깨진다).

  RUNNABLE : `#[ignore]` 없음                      → 자기 실행에서 ran ≥ 1
  CHILD    : `#[ignore]` + 같은 파일의 자기 이름 리터럴 → **단독 실행 금지.** `--list --ignored` 존재 1 건 +
                                                   같은 SC 가 지명한 **부모 필터 로그**의 자식 실행 수를 찍는다
  MANUAL   : `#[ignore]` + 부모 리터럴 없음          → `--ignored` 로 돌리고 ran ≥ 1

**분모는 줄지 않는다** — CHILD 도 자기 행으로 남고 다른 조건으로 만족된다. 자식 실행 **기대값은 도구에
박지 않는다**(계약 `⊘` 칸이 요구한다).

자식 입증의 우물은 **표준 게이트 로그**다(`--test-log`, `cargo test --workspace --locked -- --show-output`).
증거가 게이트 전용 실행에만 있으면 CI 는 눈이 먼다. `--show-output` 은 캡처를 **유지한 채** 실행 뒤
`---- <부모> stdout ----` 블록으로 찍으므로 **부모 귀속이 로그 구조에서 나온다**. `--nocapture` 와 raw
write 는 쓰지 않는다 — libtest 가 `test <이름> ... ` 과 `ok\\n` 을 나눠 쓰는 사이에 끼어 **결과 줄을
splice 한다**(architect D-2 실측: 단일 스레드에서 5/5 회). 기대 회차는 **부모 본문의 spawn 호출 수**로
소스에서 센다 — 도구에 수를 박지 않는다.

종료 코드:
  0 = 유령 0 · 실패 0 · 쟀고 통과
  1 = 유령 ≥ 1 · 실패 테스트 ≥ 1 · 자식 회차 불일치 · 자식 pid 충돌
  2 = cargo 를 부를 수 없음 → 미검증(환경)
  4 = **판정하지 않는다** — `#[ignore]` 미분류 · 역할 UNCLASSIFIED · CHILD 의 부모 미지명 · 항등식 불성립
  5 = **미검증(관측 없음)** — `--show-output` 블록 없음 · 블록은 있는데 자식 실행 0 · 기대 회차 미확인 ·
      로그 형태를 못 읽음(`-q`). **"재지 못한 실행" 과 "재서 통과한 실행" 이 같은 라벨을 달면 안 된다.**
"""
from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import time
from dataclasses import asdict, dataclass, field
from pathlib import Path

for _s in (sys.stdout, sys.stderr):
    try:
        _s.reconfigure(encoding="utf-8")
    except (AttributeError, ValueError):
        pass

REPO = Path(__file__).resolve().parents[2]

ROW = re.compile(r"^\|\s*\*{0,2}SC-(\d+)\*{0,2}\s*\|")
CELL_SPLIT = re.compile(r"(?<!\\)\|")
BACKTICK = re.compile(r"`([^`]+)`")
BARE = re.compile(r"^[A-Za-z_][\w:]*\*?$")
TEST_LINE = re.compile(r"^test (\S+?)(?: - should panic)?(?: - compile(?: fail)?)? \.\.\. (ok|FAILED|ignored)")
RESULT_LINE = re.compile(r"^test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored")
METHOD_COL = 3  # | SC | 기준 | 방법 | ⊘ | 담당 | AC | E |

# cargo test 의 값을 받는 플래그(`cargo test --help`). 이 값은 필터가 아니다.
CARGO_VALUE_FLAGS = {"-p", "--package", "--test", "--bench", "--example", "--bin", "--features", "-F",
                     "--target", "--profile", "--manifest-path", "-j", "--jobs", "--color", "--message-format",
                     "--target-dir", "--exclude", "--config", "-Z", "--lockfile-path"}
# libtest(`--` 뒤)의 값을 받는 플래그
LIBTEST_VALUE_FLAGS = {"--skip", "--test-threads", "--format", "--logfile", "--color", "-Z", "--shuffle-seed"}

# 코드 색인에 단어로 나오지만 테스트·식별자가 아닌 것 — 명시 목록만(r3 §3a)
NON_CODE = {"check_violation": "PostgreSQL 오류 조건 이름(SQLSTATE 23514) — 테스트 이름 아님"}

INDEX_ROOTS = ["server", "tools", "client/Assets/_Project", "tests", "contracts"]
SKIP_DIRS = ("target", "Library", "__pycache__", "Generated", "node_modules", ".git")


@dataclass
class Fixes:
    """결함 처리 스위치 — selftest 의 음성 대조가 하나씩 끈다."""
    bare: bool = True          # 결함 1
    value_flags: bool = True   # 결함 2
    cell_split: bool = True    # 결함 3 (열 밀림)
    strip_cr: bool = True      # 결함 3 (CR)
    ignore_role: bool = True   # D-1: 소스 속성에서 역할(RUNNABLE/CHILD/MANUAL)을 읽는다


@dataclass
class Filter:
    sc: str
    source: str                 # cargo | bare
    token: str                  # 계약 백틱 원문
    kind: str = "rust"          # rust | unity
    crate: str = ""
    workspace: str = ""         # 레포 기준 상대 경로(server, tools/bots)
    cargo_flags: list = field(default_factory=list)
    name: str = ""              # 필터 이름("" = 대상 전체)
    libtest_flags: list = field(default_factory=list)
    note: str = ""
    role: str = "RUNNABLE"      # RUNNABLE | CHILD | MANUAL — **소스에서만** 정한다(D-1 §1)
    role_why: str = ""

    @property
    def run_name(self) -> str:
        """cargo 에 넘길 이름 — cargo 는 글롭이 없다. `*` 앞까지(부분 문자열)."""
        return self.name.split("*", 1)[0]

    @property
    def exact(self) -> bool:
        return "--exact" in self.libtest_flags

    def key(self) -> tuple:
        return (self.sc, self.crate, tuple(self.cargo_flags), self.name, tuple(self.libtest_flags))


# ---------------------------------------------------------------- 추출

def split_cells(line: str, fixes: Fixes) -> list[str]:
    return CELL_SPLIT.split(line) if fixes.cell_split else line.split("|")


def contract_rows(text: str, fixes: Fixes = Fixes()) -> list[tuple[str, str, str]]:
    """§1 의 (SC, 방법 칸, 행 전체) — `## 1.` 절 안의 `| SC-nn |` 행만."""
    if fixes.strip_cr:
        text = text.replace("\r", "")
    out, section = [], None
    for line in text.split("\n"):
        if line.startswith("## "):
            section = line[3:].strip()
        m = ROW.match(line)
        if not m or not (section and section.startswith("1.")):
            continue
        cells = split_cells(line, fixes)
        method = cells[METHOD_COL] if len(cells) > METHOD_COL else ""
        out.append((f"SC-{int(m.group(1)):02d}", method, line))
    return out


def parse_cargo(tok: str, fixes: Fixes = Fixes()):
    """`cargo test …` 백틱 하나 → (crate, cargo_flags, name, libtest_flags). cargo test 가 아니면 None."""
    parts = tok.strip().split()
    if parts[:2] != ["cargo", "test"]:
        return None
    crate, cflags, names, lflags = "", [], [], []
    i, after_dd = 2, False
    while i < len(parts):
        p = parts[i]
        if not after_dd and p == "--":
            after_dd = True
        elif not after_dd and p in ("-p", "--package"):
            crate = parts[i + 1] if i + 1 < len(parts) else ""
            i += 1
        elif not after_dd and p.startswith("-"):
            if fixes.value_flags and p in CARGO_VALUE_FLAGS and "=" not in p and i + 1 < len(parts):
                cflags += [p, parts[i + 1]]
                i += 1
            else:
                cflags.append(p)
        elif after_dd and p.startswith("-"):
            if p in LIBTEST_VALUE_FLAGS and i + 1 < len(parts):
                lflags += [p, parts[i + 1]]
                i += 1
            else:
                lflags.append(p)
        else:
            names.append(p)
        i += 1
    return crate, cflags, (names[0] if names else ""), lflags, names[1:]


@dataclass
class CodeIndex:
    test_fns: dict            # 이름 → [(path, lang)]
    idents_text: str          # 주석을 뺀 코드 전체(식별자 단어 검색용)
    # --- D-1: `#[ignore]` 역할 판정의 원천. 계약 산문이 아니라 Rust 속성이 토큰이다.
    ignored: dict = field(default_factory=dict)       # 이름 → [(path, lang)] — `#[test]` + `#[ignore]`
    rs_src: dict = field(default_factory=dict)        # path → 주석을 뺀 소스(부모 리터럴 검색용)
    raw_ignore_attrs: int = 0                         # 주석 뺀 소스의 `#[ignore` 전체 수(항등식 ③ 의 좌변)


_LINE_COMMENT = re.compile(r"//[^\n]*")
_BLOCK_COMMENT = re.compile(r"/\*.*?\*/", re.S)
_PY_COMMENT = re.compile(r"(?m)^\s*#[^\n]*")
RS_TEST = re.compile(r"#\[(?:[\w:]+::)?test[^\]]*\]\s*(?:#\[[^\]]*\]\s*)*(?:pub\s+)?(?:async\s+)?fn\s+(\w+)")
CS_TEST = re.compile(r"\[(?:Test|UnityTest|TestCase[^\]]*|TestCaseSource[^\]]*)\][^\n]*\n(?:\s*\[[^\]]*\][^\n]*\n)*"
                     r"\s*public\s+(?:static\s+)?(?:async\s+)?[\w<>\[\]]+\s+(\w+)")


# `fn` 바로 앞의 속성 블록 전체와 함수 이름 — `#[test]`·`#[ignore]` 가 어느 함수에 붙었는지 본다.
RS_ATTR_FN = re.compile(r"((?:#\[[^\]]*\]\s*)+)(?:pub\s+)?(?:async\s+)?fn\s+(\w+)")
RS_IS_TEST_ATTR = re.compile(r"#\[(?:[\w:]+::)?test[\s\]\(]")
RS_IGNORE_ATTR = re.compile(r"#\[ignore\b")
LIST_LINE = re.compile(r"^(\S+): test$")
# 부모 로그에서 "자식이 실제로 돌았다" 고 읽을 수 있는 줄. 두 형태를 센다:
#   ① `test <자식> ... ok`                         — 부모가 자식 stdout 을 그대로 흘릴 때
#   ② `[CHILD] worker=<자식> … :: test result: ok. 1 passed …`  — 오늘 세 부모가 찍는 요약 형태
# **세면 안 되는 것**: `... ignored`(건너뛴 기록) 과 **`0 passed`**. rust 의 음성 대조에서 자식 이름에
# 오타를 내자 determinism 자식이 `0 passed; 2 filtered out` 을 내고도 **exit 0** 이었다 — 종료 상태만
# 보는 검사는 그 입력에서 초록이다. 그래서 "돌았다" 는 **passed ≥ 1** 로만 읽는다.
# `-- --show-output` 이 캡처를 **유지한 채** 통과한 테스트의 출력을 실행 뒤에 찍는 블록 머리.
# architect D-2 실측: 병렬 10 회 + 단일 스레드 3 회에서 오염 0/13. `--nocapture`·raw write 는 libtest 가
# `test <이름> ... ` 과 `ok\n` 을 나눠 쓰는 사이에 끼어들어 **결과 줄을 splice 한다**(단일 스레드에서 5/5).
# 블록 안에서만 세면 **어느 부모의 자식인지가 문자열 추측이 아니라 로그 구조에서 나온다.**
STDOUT_BLOCK = re.compile(r"^---- (\S+) stdout ----\s*$")
BLOCK_END = re.compile(r"^(?:successes:|failures:|test result:|running \d+ tests?\b|\s*Running\b|\s*Doc-tests\b)")
CHILD_OK_LINE = re.compile(r"\.\.\. ok\b")
CHILD_RESULT_PASSED = re.compile(r"test result:.*?\b(\d+) passed")
CHILD_PID = re.compile(r"child_pid=(\d+)")
CHILD_PARENT_PID = re.compile(r"parent_pid=(\d+)")


def child_run_line(line: str) -> bool:
    """이 줄이 **자식이 1 건 이상 돌았다** 는 기록인가. `... ignored` · `0 passed` 는 아니다."""
    if "... ignored" in line:
        return False
    m = CHILD_RESULT_PASSED.search(line)
    if m:
        return int(m.group(1)) >= 1
    return bool(CHILD_OK_LINE.search(line))


def build_index(root: Path = REPO, roots: list[str] = INDEX_ROOTS) -> CodeIndex:
    test_fns: dict = {}
    ignored: dict = {}
    rs_src: dict = {}
    raw_ignore_attrs = 0
    chunks: list[str] = []
    for r in roots:
        base = root / r
        if not base.exists():
            continue
        for dp, dn, fn in os.walk(base):
            dn[:] = [d for d in dn if d not in SKIP_DIRS]
            for f in fn:
                if not f.endswith((".rs", ".cs", ".py", ".json")):
                    continue
                p = Path(dp) / f
                try:
                    src = p.read_text(encoding="utf-8", errors="ignore")
                except OSError:
                    continue
                rel = p.relative_to(root).as_posix()
                if f.endswith(".rs"):
                    for n in RS_TEST.findall(src):
                        test_fns.setdefault(n, []).append((rel, "rust"))
                    src = _LINE_COMMENT.sub("", _BLOCK_COMMENT.sub("", src))
                    # D-1: 속성 블록에서 `#[test]` + `#[ignore]` 를 읽는다. **주석을 뺀 소스로** 센다
                    # (`//!`·`///` 의 이름 언급이 속성으로 읽히지 않도록).
                    attached = 0
                    for attrs, fn_name in RS_ATTR_FN.findall(src):
                        if RS_IS_TEST_ATTR.search(attrs) and RS_IGNORE_ATTR.search(attrs):
                            ignored.setdefault(fn_name, []).append((rel, "rust"))
                            attached += 1
                    raw = len(RS_IGNORE_ATTR.findall(src))
                    raw_ignore_attrs += raw
                    if raw or attached:
                        rs_src[rel] = src
                elif f.endswith(".cs"):
                    for n in CS_TEST.findall(src):
                        test_fns.setdefault(n, []).append((rel, "unity"))
                    src = _LINE_COMMENT.sub("", _BLOCK_COMMENT.sub("", src))
                elif f.endswith(".py"):
                    src = _PY_COMMENT.sub("", src)
                chunks.append(src)
    return CodeIndex(test_fns, "\n".join(chunks), ignored, rs_src, raw_ignore_attrs)


def crate_of(path: str, root: Path = REPO) -> tuple[str, str]:
    """정의 파일 → (패키지 이름, 워크스페이스 상대 경로). 가장 가까운 [package] Cargo.toml 과 [workspace] Cargo.toml."""
    p = (root / path).parent
    pkg, ws = "", ""
    while True:
        ct = p / "Cargo.toml"
        if ct.is_file():
            body = ct.read_text(encoding="utf-8", errors="ignore")
            if not pkg:
                m = re.search(r"(?ms)^\[package\][^\[]*?^name\s*=\s*\"([^\"]+)\"", body)
                if m:
                    pkg = m.group(1)
            if "[workspace]" in body:
                ws = p.relative_to(root).as_posix()
                break
        if p == root or p.parent == p:
            break
        p = p.parent
    return pkg, ws


def workspace_of_crate(crate: str, root: Path = REPO) -> str:
    for ws in ("server", "tools/bots"):
        for ct in (root / ws).rglob("Cargo.toml"):
            if "target" in ct.parts:
                continue
            if re.search(rf'(?m)^name\s*=\s*"{re.escape(crate)}"', ct.read_text(encoding="utf-8", errors="ignore")):
                return ws
    return "server"


def classify_bare(tok: str, idx: CodeIndex) -> tuple[str, list]:
    last = tok.split("::")[-1]
    star = last.endswith("*")
    last = last.rstrip("*")
    if tok in NON_CODE:
        return "NON_CODE", []
    hits = [(n, locs) for n, locs in idx.test_fns.items() if (n.startswith(last) if star else n == last)]
    if hits:
        return "TEST_FN", [loc for _, locs in hits for loc in locs]
    if re.search(r"\b" + re.escape(last) + (r"" if star else r"\b"), idx.idents_text):
        return "NON_TEST_IDENT", []
    return "GHOST", []


def candidate_match(f: Filter, fn_name: str) -> bool:
    """정의된 테스트 **함수 이름**이 이 필터에 잡힐 수 있는가. 색인은 모듈 경로를 모르므로 필터 이름의
    마지막 `::` 조각으로 본다. 틀리는 방향은 **RUNNABLE 쪽**이고(= 돌려 본다), 그때 ran=0 이면 유령으로
    시끄럽게 걸린다 — 조용한 분모 감소가 아니다."""
    if not f.run_name:
        return False
    t = f.run_name.split("::")[-1]
    return fn_name == t if f.exact else (t in fn_name)


def stdout_blocks(log: str) -> list[tuple]:
    """`--show-output` 의 `---- <테스트> stdout ----` 블록들 → [(테스트 이름, 줄 목록)]."""
    out, name, buf = [], None, []
    for line in log.splitlines():
        m = STDOUT_BLOCK.match(line)
        if m:
            if name is not None:
                out.append((name, buf))
            name, buf = m.group(1), []
            continue
        if name is not None:
            if BLOCK_END.match(line):
                out.append((name, buf))
                name, buf = None, []
                continue
            buf.append(line)
    if name is not None:
        out.append((name, buf))
    return out


def fn_body(src: str, name: str) -> str:
    """`fn <name>` 의 중괄호 균형 본문. 못 찾으면 빈 문자열(→ 수를 모른다 = 판정하지 않는다)."""
    m = re.search(r"\bfn\s+" + re.escape(name) + r"\b[^{;]*", src)
    if not m:
        return ""
    i = src.find("{", m.end() - 1)
    if i < 0:
        return ""
    depth = 0
    for j in range(i, len(src)):
        if src[j] == "{":
            depth += 1
        elif src[j] == "}":
            depth -= 1
            if depth == 0:
                return src[i:j + 1]
    return ""


def expected_child_runs(parent: str, child: str, idx: CodeIndex) -> int | None:
    """계약 `⊘` 칸이 요구하는 "2 회" 를 **도구에 박지 않고 소스에서 센다** — 부모 본문의 **spawn 헬퍼
    호출 수**다. 헬퍼는 자식 이름을 리터럴로 든 함수(= 자식을 띄우는 쪽)이고, 호출 수는 실행에
    쓰이는 코드라 **하중을 받는다**(D-1 §1 과 같은 원리). 못 세면 `None` — 그때는 판정하지 않는다."""
    for p, lang in idx.ignored.get(child, []):
        if lang != "rust":
            continue
        src = idx.rs_src.get(p, "")
        body = fn_body(src, parent)
        if not body:
            continue
        lit = f'"{child}"'
        if lit in body:                                    # 부모가 직접 리터럴을 든다
            return body.count(lit)
        helpers = [n for n in re.findall(r"\bfn\s+(\w+)", src)
                   if n not in (parent, child) and lit in fn_body(src, n)]
        counts = [len(re.findall(r"\b" + re.escape(h) + r"\s*\(", body)) for h in helpers]
        counts = [c for c in counts if c]
        if len(counts) == 1:
            return counts[0]
    return None


def parent_of(child: str, idx: CodeIndex) -> dict:
    """자식 이름이 **같은 파일의 문자열 리터럴**로 나오는가 + 그 파일의 부모 후보(같은 파일의
    `#[ignore]` 아닌 테스트 함수). 그 리터럴은 자식을 띄우는 **실행 인자**라서 하중을 받는다 —
    이름이 틀리면 부모가 깨진다. 계약 산문에는 그 하중이 없다(D-1 §1)."""
    files = [p for p, lang in idx.ignored.get(child, []) if lang == "rust"]
    lit_files, parents = [], []
    for p in files:
        if f'"{child}"' not in idx.rs_src.get(p, ""):
            continue
        lit_files.append(p)
        parents += [n for n, locs in idx.test_fns.items()
                    if n != child and n not in idx.ignored and any(q == p for q, lang in locs if lang == "rust")]
    return {"literal_in": sorted(set(lit_files)), "parents": sorted(set(parents))}


def ignore_census(idx: CodeIndex) -> dict:
    """항등식 ③ — 레포의 `#[ignore]` **전체**를 CHILD/MANUAL 로 가른다. 어느 테스트 함수에도 붙이지 못한
    `#[ignore` 는 **미분류**다. 누가 속성을 재포맷해 정규식이 안 먹으면 "위반 0" 이 아니라 미분류 ≥ 1 →
    판정하지 않는다(R28 항등식 ① 과 같은 자기 무력화 방어)."""
    child, manual = [], []
    for n in sorted(idx.ignored):
        (child if parent_of(n, idx)["literal_in"] else manual).append(n)
    attached = sum(len(v) for v in idx.ignored.values())
    return {"raw_ignore_attrs": idx.raw_ignore_attrs, "attached": attached, "child": child, "manual": manual,
            "unclassified": idx.raw_ignore_attrs - attached}


def assign_roles(filters: list[Filter], idx: CodeIndex | None, fixes: Fixes = Fixes()) -> None:
    """Rust 필터마다 역할 하나. **계약 산문을 읽지 않는다** — `#[ignore]` 속성과 부모 리터럴만 본다.
    지명은 전부 줍고(분모를 줄이지 않는다), 가르는 것은 *그 지명을 게이트가 어떻게 만족시키느냐* 다."""
    for f in filters:
        if f.kind != "rust":
            continue
        f.role, f.role_why = "RUNNABLE", "#[ignore] 없음"
        if not fixes.ignore_role:
            f.role_why = "역할 판정 꺼짐(음성 대조)"
            continue
        if idx is None or not f.run_name:
            f.role_why = "대상 전체(이름 없음)" if idx is not None else "색인 없음"
            continue
        ig = [n for n in idx.ignored if candidate_match(f, n)]
        run = [n for n, locs in idx.test_fns.items()
               if n not in idx.ignored and any(lang == "rust" for _, lang in locs) and candidate_match(f, n)]
        if not ig or run:
            if run:
                f.role_why = f"#[ignore] 아닌 테스트 {len(run)} 건을 잡는다"
            continue
        kinds = {("CHILD" if parent_of(n, idx)["literal_in"] else "MANUAL") for n in ig}
        if len(kinds) == 1:
            f.role = kinds.pop()
            f.role_why = ("부모가 같은 파일에서 이름을 리터럴로 들고 있다" if f.role == "CHILD"
                          else "#[ignore] 이고 부모 리터럴 없음 — 사람이 돌린다")
        else:
            f.role, f.role_why = "UNCLASSIFIED", f"#[ignore] {len(ig)} 건이 CHILD·MANUAL 로 갈린다"


def extract(text: str, idx: CodeIndex | None, fixes: Fixes = Fixes(), root: Path = REPO) -> dict:
    """계약 → {filters, bare, static_ghosts, rows}."""
    filters: list[Filter] = []
    bare: list[dict] = []
    ws_cache: dict = {}
    rows = contract_rows(text, fixes)
    for sc, method, _line in rows:
        for tok in BACKTICK.findall(method):
            t = tok.strip()
            pc = parse_cargo(t, fixes)
            if pc is not None:
                crate, cflags, name, lflags, extra = pc
                if crate not in ws_cache:
                    ws_cache[crate] = workspace_of_crate(crate, root) if crate else "server"
                filters.append(Filter(sc, "cargo", t, "rust", crate, ws_cache[crate], cflags, name, lflags,
                                      note=("추가 위치 인자 무시: " + " ".join(extra)) if extra else ""))
                continue
            if not fixes.bare or t.startswith("cargo") or " " in t:
                continue
            if not BARE.match(t) or ("_" not in t and "::" not in t):
                continue
            if idx is None:
                bare.append({"sc": sc, "token": t, "category": "UNINDEXED"})
                continue
            cat, locs = classify_bare(t, idx)
            bare.append({"sc": sc, "token": t, "category": cat, "defined_in": sorted({p for p, _ in locs})})
            if cat != "TEST_FN":
                continue
            owners = sorted({(crate_of(p, root) if lang == "rust" else ("", "")) + (lang,) for p, lang in locs})
            for crate, ws, lang in owners:
                if lang == "unity":
                    filters.append(Filter(sc, "bare", t, "unity", name=t))
                else:
                    filters.append(Filter(sc, "bare", t, "rust", crate, ws, [], t, [],
                                          note="정의 크레이트 여럿" if len(owners) > 1 else ""))
    # 같은 (SC, 크레이트, 플래그, 이름) 중복 제거 — 순서 보존
    seen, uniq = set(), []
    for f in filters:
        if f.key() in seen:
            continue
        seen.add(f.key())
        uniq.append(f)
    assign_roles(uniq, idx, fixes)
    return {"rows": len(rows), "filters": uniq, "bare": bare,
            "static_ghosts": [b for b in bare if b["category"] == "GHOST"],
            "ignore_census": ignore_census(idx) if idx is not None else None}


# ---------------------------------------------------------------- 실행

def load_dotenv(root: Path = REPO) -> dict:
    """`.env` 의 KEY=VALUE — **값은 어디에도 찍지 않는다**."""
    env = {}
    p = root / ".env"
    if p.is_file():
        for line in p.read_text(encoding="utf-8", errors="replace").splitlines():
            line = line.strip()
            if not line or line.startswith("#") or "=" not in line:
                continue
            k, v = line.split("=", 1)
            env[k.strip()] = v.strip().strip('"').strip("'")
    return env


def cargo_env(root: Path = REPO) -> dict:
    env = dict(os.environ)
    env.update(load_dotenv(root))
    env["STARFALL_DB_TESTS"] = "required"
    home = Path.home()
    extra = [str(home / ".cargo" / "bin")]
    env["PATH"] = os.pathsep.join(extra + [env.get("PATH", "")])
    env.setdefault("CARGO_TERM_COLOR", "never")
    return env


def mode_of(role: str) -> str:
    """역할 → 게이트가 그 지명을 만족시키는 방법. **CHILD 에 `--ignored` 를 붙여 단독 실행하지 않는다** —
    실측으로 `0 passed; 1 failed`(부모가 넣는 환경 변수 없이 패닉)였다. 존재는 `--list` 로 본다(D-1 §2)."""
    return {"CHILD": "list", "MANUAL": "ignored"}.get(role, "run")


def build_argv(f: Filter, cargo: list[str], mode: str = "run", show_output: bool = False) -> list[str]:
    argv = list(cargo) + ["test", "-p", f.crate]
    if "--locked" not in f.cargo_flags:
        argv.append("--locked")
    argv += f.cargo_flags + ["--"]
    if f.run_name:
        argv.append(f.run_name)
    argv += f.libtest_flags
    if mode in ("list", "ignored") and "--ignored" not in argv:
        argv.append("--ignored")
    if mode == "list":
        argv.append("--list")
    # 부모가 자식 관측을 `println!` 로 찍는다 → libtest 가 **성공한 테스트의 출력을 캡처한다.**
    # `--show-output` 은 캡처를 **유지한 채** 실행 뒤 블록으로 찍는다(`--nocapture` 는 실행 중에 섞여
    # 나와 libtest 의 결과 줄을 splice 한다 — architect D-2 실측). 이 플래그가 없으면 블록이 없고,
    # 그때 CHILD 입증은 **PASS 도 FAIL 도 아니라 미검증**이다.
    if show_output and "--show-output" not in argv:
        argv.append("--show-output")
    return argv


def parents_needing_show_output(filters: list[Filter], idx: CodeIndex | None) -> set:
    """CHILD 행의 **부모 필터**. 표준 게이트 로그(`--test-log`)가 없을 때의 대체 우물이다 —
    주 우물은 표준 게이트 로그다(증거가 게이트 전용 실행에만 있으면 CI 는 눈이 먼다)."""
    need = set()
    if idx is None:
        return need
    for f in filters:
        if f.kind != "rust" or f.role != "CHILD":
            continue
        cand = parent_of(f.name, idx)["parents"]
        for g in filters:
            if g.kind != "rust" or g is f or g.sc != f.sc or g.role == "CHILD":
                continue
            if any(candidate_match(g, c) for c in cand):
                need.add(g.key())
    return need


def parse_test_log(log: str) -> dict:
    tests = [(m.group(1), m.group(2)) for m in (TEST_LINE.match(l) for l in log.splitlines()) if m]
    p = fl = ig = 0
    for m in (RESULT_LINE.match(l) for l in log.splitlines()):
        if m:
            p, fl, ig = p + int(m.group(1)), fl + int(m.group(2)), ig + int(m.group(3))
    return {"tests": tests, "passed": p, "failed": fl, "ignored": ig}


def filter_matches(f: Filter, test_path: str) -> bool:
    if not f.run_name:
        return False
    return test_path == f.run_name if f.exact else f.run_name in test_path


def run_filters(filters: list[Filter], out: Path, cargo: list[str] | None = None, env: dict | None = None,
                root: Path = REPO, timeout: int = 1800, progress=print, idx: CodeIndex | None = None,
                show_output: bool = True, test_log: Path | None = None) -> list[dict]:
    cargo = cargo or ["cargo"]
    env = env if env is not None else cargo_env(root)
    logs = out / "logs"
    logs.mkdir(parents=True, exist_ok=True)
    results = []
    rust = [f for f in filters if f.kind == "rust"]
    # CHILD 는 **부모 로그를 읽어야** 하므로 맨 뒤로 보낸다(부모가 먼저 돌아 로그를 남긴다).
    rust = [f for f in rust if f.role != "CHILD"] + [f for f in rust if f.role == "CHILD"]
    need_so = parents_needing_show_output(rust, idx) if show_output else set()
    for i, f in enumerate(rust, 1):
        mode = mode_of(f.role)
        nc = f.key() in need_so
        argv = build_argv(f, cargo, mode, show_output=nc)
        slug = re.sub(r"[^\w.-]+", "_", f"{f.sc}_{f.crate}_{'_'.join(f.cargo_flags)}_{f.name or 'ALL'}")[:150]
        log_path = logs / f"{i:03d}_{slug}.log"
        t0 = time.monotonic()
        try:
            proc = subprocess.run(argv, cwd=str(root / (f.workspace or "server")), env=env, stdin=subprocess.DEVNULL,
                                  stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=timeout)
            rc, text = proc.returncode, proc.stdout.decode("utf-8", errors="replace")
        except subprocess.TimeoutExpired as e:
            rc, text = "timeout", (e.stdout or b"").decode("utf-8", errors="replace")
        log_path.write_text(text, encoding="utf-8")
        parsed = parse_test_log(text)
        ran = [(n, s) for n, s in parsed["tests"] if s in ("ok", "FAILED")]
        rec = {**_fdict(f), "mode": mode, "show_output": nc,
               "argv": argv[len(cargo):], "cwd": f.workspace or "server", "rc": rc,
               "seconds": round(time.monotonic() - t0, 1), "log": str(log_path),
               "ran": len(ran), "ok": sum(1 for _, s in ran if s == "ok"),
               "failed": [n for n, s in ran if s == "FAILED"],
               "ignored": [n for n, s in parsed["tests"] if s == "ignored"],
               "names": [n for n, _ in ran], "ghost": len(ran) == 0}
        # `-q` 음성 대조(architect D-2 부수 발견): `cargo test -q` 는 `test X ... ok` 줄을 아예 안 찍고
        # 점 진행만 찍는다. 그러면 **모든 필터가 유령**으로 인쇄되고, 다음 사람이 거기 맞춰 집계를
        # 느슨하게 고치면 전부 초록이 된다. 결과 줄은 통과를 말하는데 테스트 줄이 0 이면 **로그 형태를
        # 못 읽은 것**이고, 그때는 유령이 아니라 **미검증**이다.
        rec["log_form_unreadable"] = (parsed["passed"] + parsed["failed"] >= 1 and not parsed["tests"])
        if mode == "list":
            # CHILD ①: 존재를 **실행 없이** 기계가 본다. `--list --ignored` 는 `<이름>: test` 를 찍는다.
            rec["listed"] = len([m for m in (LIST_LINE.match(l) for l in text.splitlines()) if m])
            rec["ghost"] = rec["listed"] == 0
        results.append(rec)
        if rec["log_form_unreadable"]:
            rec["ghost"] = False      # 유령으로 인쇄하지 않는다 — 못 읽은 것과 없는 것은 다르다
        tail = f"listed={rec['listed']}" if mode == "list" else f"ran={len(ran)}"
        progress(f"[{i}/{len(rust)}] {f.sc} {f.crate} {' '.join(f.cargo_flags)} {f.name or '(전체)'} "
                 f"[{f.role}/{mode}] → {tail} rc={rc}")
    attach_child_evidence(results, idx, test_log)
    attach_ownership(results)
    return results


def count_child_in_blocks(log: str, parent: str, child: str) -> dict:
    """표준 게이트 로그에서 `---- <부모> stdout ----` 블록 안의 자식 실행 수·pid."""
    runs, pids, parent_pids, blocks = 0, [], [], []
    for bname, lines in stdout_blocks(log):
        if not (bname == parent or bname.endswith("::" + parent)):
            continue
        blocks.append(bname)
        for line in lines:
            if child not in line or not child_run_line(line):
                continue
            runs += 1
            m = CHILD_PID.search(line)
            if m:
                pids.append(m.group(1))
            m = CHILD_PARENT_PID.search(line)
            if m:
                parent_pids.append(m.group(1))
    return {"runs": runs, "pids": pids, "blocks": blocks,
            "distinct_pids": len(set(pids)),
            "pid_equals_parent": sorted({p for p, q in zip(pids, parent_pids) if p == q})}


def parent_census(idx: CodeIndex, test_log: Path | None) -> dict:
    """**분모 3** — 레포의 `#[ignore]` 자식을 spawn 하는 부모 전체(계약이 지명하든 않든)를 표준 게이트
    로그에 대고 센다. 리더가 실측으로 닫은 두 걱정(`--workspace` 전달 · 로그 크기)을 **단언으로 남기는**
    자리다: 세 부모 각각 자식 실행 수가 **소스가 요구하는 회차와 정확히 같은지**. 일부만 찍히면
    이 로그로는 판정할 수 없다 — 유령이 아니라 **미검증**이다.

    계약이 지명하지 않은 부모는 **판정 대상이 아니라 로그 건전성의 증거**다(만기 #5 분모 3 을 줄이지 않는다)."""
    cen = ignore_census(idx)
    rows = []
    log = None
    if test_log is not None:
        try:
            log = test_log.read_text(encoding="utf-8", errors="replace")
        except OSError:
            log = None
    for child in cen["child"]:
        parents = [p for p in parent_of(child, idx)["parents"] if expected_child_runs(p, child, idx)]
        exp = next((expected_child_runs(p, child, idx) for p in parents), None)
        row = {"child": child, "parents": parents, "expected": exp, "runs": None, "distinct_pids": None,
               "pids": None, "measurable": None, "pid_equals_parent": None}
        if log is not None and parents:
            c = count_child_in_blocks(log, parents[0], child)
            row.update(runs=c["runs"], pids=c["pids"], distinct_pids=c["distinct_pids"],
                       pid_equals_parent=c["pid_equals_parent"], measurable=bool(c["blocks"]))
        rows.append(row)
    partial = [r["child"] for r in rows
               if log is None or not r["measurable"] or r["expected"] is None or r["runs"] != r["expected"]]
    pid_bad = [(r["child"], r["pids"]) for r in rows
               if r["runs"] and (r["distinct_pids"] != r["runs"] or r["pid_equals_parent"])]
    return {"denominator": len(cen["child"]), "log": test_log.as_posix() if test_log else None,
            "rows": rows, "partial": partial, "pid_bad": pid_bad}


def attach_child_evidence(results: list[dict], idx: CodeIndex | None, test_log: Path | None = None) -> None:
    """CHILD 행마다 ② 자식 실행 수. **증거의 우물은 `--test-log`(표준 게이트 로그)** 다 — 증거가
    게이트 전용 실행에만 있으면 CI 는 눈이 먼다(architect D-2). 없으면 부모 필터 로그로 떨어진다.

    세는 자리는 `---- <부모> stdout ----` **블록 안**이고, 그래서 **부모 귀속이 로그 구조에서 나온다.**
    기대 회차는 **부모 본문의 spawn 호출 수**로 소스에서 센다 — 도구에 2 를 박지 않는다.
    부모가 같은 SC 에 지명돼 있지 않으면 판정하지 않는다(exit 4)."""
    ext_text = None
    if test_log is not None:
        try:
            ext_text = test_log.read_text(encoding="utf-8", errors="replace")
        except OSError:
            ext_text = None
    for r in results:
        if r.get("role") != "CHILD":
            continue
        child = r["name"]
        cand = parent_of(child, idx)["parents"] if idx is not None else []
        sibs = []
        for o in results:
            if o is r or o["sc"] != r["sc"] or o.get("role") == "CHILD":
                continue
            g = Filter(o["sc"], o["source"], o["token"], o["kind"], o["crate"], o["workspace"],
                       o["cargo_flags"], o["name"], o["libtest_flags"])
            if any(candidate_match(g, c) for c in cand):
                sibs.append(o)
        # 이 SC 의 부모 필터가 **어느 부모 테스트 함수**를 지명했는가(블록 이름과 맞출 대상)
        named_parents = sorted({c for c in cand for o in sibs
                                if candidate_match(Filter(o["sc"], o["source"], o["token"], o["kind"], o["crate"],
                                                          o["workspace"], o["cargo_flags"], o["name"],
                                                          o["libtest_flags"]), c)})
        if ext_text is not None:
            src_name, texts = f"test-log:{test_log.as_posix()}", [ext_text]
        else:
            src_name, texts = "filter-log", []
            for o in sibs:
                try:
                    texts.append(Path(o["log"]).read_text(encoding="utf-8", errors="replace"))
                except OSError:
                    pass
        runs, mentions, pids, parent_pids = 0, 0, [], []
        blocks_seen, parent_ran = [], False
        for body in texts:
            mentions += sum(1 for line in body.splitlines() if child in line)
            parsed = parse_test_log(body)
            parent_ran = parent_ran or any(n == p or n.endswith("::" + p) for n, _ in parsed["tests"]
                                           for p in named_parents)
            for bname, lines in stdout_blocks(body):
                if not any(bname == p or bname.endswith("::" + p) for p in named_parents):
                    continue
                blocks_seen.append(bname)
                for line in lines:
                    if child not in line or not child_run_line(line):
                        continue
                    runs += 1
                    # rust 가 자식 pid ≠ 부모 pid 를 단언하고 그 pid 를 찍는다. 두 줄의 `child_pid` 가
                    # **서로 달라야** "서로 다른 프로세스에서 2 회" 가 기계로 확인된다 — 같은 pid 두 줄은
                    # 한 프로세스를 두 번 적은 것이다.
                    m = CHILD_PID.search(line)
                    if m:
                        pids.append(m.group(1))
                    m = CHILD_PARENT_PID.search(line)
                    if m:
                        parent_pids.append(m.group(1))
        r["parent_candidates"] = cand
        r["parent_filters"] = [{"sc": o["sc"], "name": o["name"], "log": o["log"]} for o in sibs]
        r["parent_named"] = bool(sibs)
        r["named_parents"] = named_parents
        r["child_evidence_source"] = src_name
        r["child_runs"] = runs
        # 이름이 **언급된** 수와 **돌았다고 읽힌** 수를 따로 찍는다. 둘이 다르면 그 차이가 건너뜀 기록이고,
        # mentions ≥ 1 인데 runs 0 이면 "이름은 있고 실행 기록은 없다" 다(D-1 후속 §A 가 정정한 그 문장).
        r["child_mentions"] = mentions
        r["child_pids"] = pids
        r["distinct_child_pids"] = len(set(pids))
        r["pid_equals_parent"] = sorted({p for p, q in zip(pids, parent_pids) if p == q})
        r["stdout_blocks"] = blocks_seen
        r["expected_child_runs"] = next((e for p in named_parents
                                         if (e := expected_child_runs(p, child, idx)) is not None), None) \
            if idx is not None else None
        # **측정 경로가 열려 있었는가.** 부모가 돌았는데 그 부모의 `stdout` 블록이 없으면
        # `--show-output` 이 빠진 것이고, 그 실행은 "자식이 안 돌았다" 가 아니라 **재지 못한 실행**이다.
        r["parent_ran_in_log"] = parent_ran
        r["child_measurable"] = bool(sibs) and parent_ran and bool(blocks_seen)


def _fdict(f: Filter) -> dict:
    d = asdict(f)
    d["run_name"] = f.run_name
    return d


def attach_ownership(results: list[dict]) -> None:
    """잡힌 테스트마다 같은 크레이트의 **다른 SC** 필터가 그 테스트를 잡는지."""
    fl = [(r["sc"], Filter(r["sc"], r["source"], r["token"], r["kind"], r["crate"], r["workspace"],
                           r["cargo_flags"], r["name"], r["libtest_flags"])) for r in results]
    for r in results:
        if not r["name"]:
            r["scope"] = "whole"
            r["also_named_by"] = {}
            continue
        r["scope"] = "named"
        also = {}
        for t in r["names"]:
            others = sorted({sc for sc, g in fl if sc != r["sc"] and g.crate == r["crate"] and filter_matches(g, t)})
            if others:
                also[t] = others
        r["also_named_by"] = also


def unity_bare_check(filters: list[Filter], xmls: list[Path]) -> list[dict]:
    """Unity 맨 이름 → junit/NUnit xml 의 testcase 이름에서 찾는다(접두 `*` 허용)."""
    cases = []
    for x in xmls:
        body = x.read_text(encoding="utf-8", errors="replace")
        for m in re.finditer(r'<test-?case\b[^>]*\bname="([^"]+)"[^>]*?(?:result="(\w+)")?', body):
            cases.append((m.group(1), m.group(2) or "", x.name))
    out = []
    for f in filters:
        if f.kind != "unity":
            continue
        pre = f.name.rstrip("*")
        hit = [c for c in cases if (c[0].split("(")[0].endswith(pre) or c[0].split(".")[-1].startswith(pre))
               if f.name.endswith("*") or c[0].split("(")[0].split(".")[-1] == pre]
        out.append({"sc": f.sc, "name": f.name, "matched": len(hit),
                    "results": sorted({c[1] for c in hit}), "ghost": len(hit) == 0})
    return out


# ---------------------------------------------------------------- 옛 방식(로그 한 벌)

def map_log(ext: dict, log: str) -> dict:
    tests = parse_test_log(log)["tests"]
    rows: dict = {}
    for f in ext["filters"]:
        if f.kind != "rust":
            continue
        m = [(n, s) for n, s in tests if (filter_matches(f, n) if f.run_name else True)]
        rows.setdefault(f.sc, []).append({"crate": f.crate, "filter": f.name, "flags": f.cargo_flags,
                                          "source": f.source, "matched": len(m),
                                          "ok": sum(1 for _, s in m if s == "ok"),
                                          "failed": [n for n, s in m if s == "FAILED"],
                                          "names": [n for n, _ in m][:12]})
    return {"log_test_lines": len(tests), "rows": rows,
            "zero_match_filters": [(k, i["filter"]) for k, v in rows.items() for i in v if i["matched"] == 0]}


# ---------------------------------------------------------------- 보고

def summarize(ext: dict, results: list[dict] | None, unity: list[dict] | None = None,
              pcensus: dict | None = None) -> dict:
    fl = ext["filters"]
    s = {"rows": ext["rows"],
         "filters_rust": sum(1 for f in fl if f.kind == "rust"),
         "filters_rust_cargo": sum(1 for f in fl if f.kind == "rust" and f.source == "cargo"),
         "filters_rust_bare": sum(1 for f in fl if f.kind == "rust" and f.source == "bare"),
         "filters_unity_bare": sum(1 for f in fl if f.kind == "unity"),
         "bare_tokens": len(ext["bare"]),
         "bare_by_category": {c: sum(1 for b in ext["bare"] if b["category"] == c)
                              for c in sorted({b["category"] for b in ext["bare"]})},
         "static_ghosts": [(b["sc"], b["token"]) for b in ext["static_ghosts"]]}
    # D-1 §1 — 역할은 소스에서 읽은다. 분모(filters_rust)는 줄지 않는다: CHILD 도 자기 행으로 남는다.
    rust = [f for f in fl if f.kind == "rust"]
    s["roles"] = {k: sum(1 for f in rust if f.role == k) for k in ROLES}
    # D-1 추가 만기 2 — **닫지 말고 센다**: 산문(백틱 맨 이름)에 적힌 RUNNABLE 테스트 이름.
    # 이 수가 출력에 없으면 "구멍이 있다" 와 "구멍이 없다" 가 같은 출력이다.
    s["prose_runnable"] = {"rust": sum(1 for f in rust if f.source == "bare" and f.role == "RUNNABLE"),
                           "unity_bare": sum(1 for f in fl if f.kind == "unity")}
    s["ignore_census"] = ext.get("ignore_census")
    if pcensus is not None:
        s["parent_census"] = pcensus
    if results is not None:
        child = [r for r in results if r.get("role") == "CHILD"]
        s.update({"executed": len(results),
                  "ran_ge_1": sum(1 for r in results if r["ran"] >= 1 or r.get("listed", 0) >= 1),
                  "ran_ge_1_self": sum(1 for r in results if r["ran"] >= 1),
                  "child_proven": sum(1 for r in child if r.get("listed", 0) >= 1),
                  "child_rows": [{"sc": r["sc"], "name": r["name"], "listed": r.get("listed"),
                                  "child_runs": r.get("child_runs"), "child_mentions": r.get("child_mentions"),
                                  "child_measurable": r.get("child_measurable"),
                                  "expected_child_runs": r.get("expected_child_runs"),
                                  "child_evidence_source": r.get("child_evidence_source"),
                                  "stdout_blocks": r.get("stdout_blocks"),
                                  "parent_ran_in_log": r.get("parent_ran_in_log"),
                                  "child_pids": r.get("child_pids"),
                                  "distinct_child_pids": r.get("distinct_child_pids"),
                                  "pid_equals_parent": r.get("pid_equals_parent"),
                                  "parent_named": r.get("parent_named"),
                                  "parent_filters": [p["name"] for p in r.get("parent_filters", [])]}
                                 for r in child],
                  "child_parent_missing": [(r["sc"], r["name"]) for r in child if not r.get("parent_named")],
                  # 측정 경로가 닫혀 있었다 → **미검증**(exit 4). PASS 도 FAIL 도 아니다.
                  "child_unmeasured": [(r["sc"], r["name"]) for r in child
                                       if r.get("parent_named") and not r.get("child_measurable")],
                  # 측정은 했고 실행 증거가 0 → **PASS 가 아니라 미검증**(리더 판정).
                  # "재지 못한 실행" 과 "재서 통과한 실행" 이 같은 라벨을 달면 안 된다.
                  "child_unobserved": [(r["sc"], r["name"]) for r in child
                                       if r.get("child_measurable") and not r.get("child_runs")],
                  # 쟀고 ≥1 인데 **소스가 요구하는 회차와 다르다** → PASS 아님(exit 1).
                  # 기대값은 도구가 아니라 부모 본문의 spawn 호출 수에서 나온다.
                  "child_runs_mismatch": [(r["sc"], r["name"], r.get("child_runs"), r.get("expected_child_runs"))
                                          for r in child
                                          if r.get("child_measurable") and r.get("child_runs")
                                          and r.get("expected_child_runs") is not None
                                          and r["child_runs"] != r["expected_child_runs"]],
                  # 기대 회차를 소스에서 못 셌다 → 판정하지 않는다
                  "child_expected_unknown": [(r["sc"], r["name"]) for r in child
                                             if r.get("child_measurable")
                                             and r.get("expected_child_runs") is None],
                  # `-q` 따위로 로그 형태를 못 읽었다 → 유령 66 이 아니라 미검증
                  "log_form_unreadable": [(r["sc"], r["name"] or "(전체)") for r in results
                                          if r.get("log_form_unreadable")],
                  # 같은 pid 두 줄 = 한 프로세스를 두 번 적은 것. "서로 다른 프로세스" 의 기계 확인.
                  # pid 를 든 줄끼리만 본다 — 자식 stdout 을 그대로 흘린 줄에는 pid 가 없고, 그것은 충돌이 아니다
                  "child_pid_collisions": [(r["sc"], r["name"], r.get("child_pids")) for r in child
                                           if r.get("child_measurable") and r.get("child_pids")
                                           and r.get("distinct_child_pids", 0) < len(r.get("child_pids") or [])],
                  "child_pid_equals_parent": [(r["sc"], r["name"], r.get("pid_equals_parent")) for r in child
                                              if r.get("pid_equals_parent")],
                  "ghosts": [(r["sc"], r["crate"], r["token"]) for r in results if r["ghost"]],
                  "failed_tests": sorted({(r["sc"], n) for r in results for n in r["failed"]}),
                  "nonzero_rc": [(r["sc"], r["token"], r["rc"]) for r in results if r["rc"] != 0],
                  "cross_sc": [(r["sc"], t, o) for r in results for t, o in r.get("also_named_by", {}).items()]})
    if unity is not None:
        s["unity_bare"] = {"checked": len(unity), "ghosts": [(u["sc"], u["name"]) for u in unity if u["ghost"]]}
    # 항등식 셋을 **값째로** 요약에 넣는다 — 리포트가 읽는 출력은 `summary.json` 이고,
    # `cargo_sc_map.out` 에만 있으면 그 자리에는 수가 없다(architect 만기: "여덟 수가 출력에 있어야 한다").
    s["identities"] = [{"name": n, "lhs": lhs, "holds": ok} for n, lhs, ok in identities(s)]
    s["undecidable"] = undecidable(s)
    s["unmeasured"] = unmeasured(s)
    s["verdict_exit"] = exit_code(s)
    return s


ROLES = ("RUNNABLE", "CHILD", "MANUAL", "UNCLASSIFIED")


def identities(s: dict) -> list[tuple]:
    """D-1 §4 의 항등식 셋. **매 실행 출력에 찍는다** — 수가 출력에 없으면 그 만기는 닫히지 않는다.
    (이름, 좌변 문자열, 성립 여부)."""
    r = s.get("roles") or {}
    c = s.get("ignore_census") or {}
    out = [("① Rust 필터 = RUNNABLE + CHILD + MANUAL + UNCLASSIFIED",
            f"{s['filters_rust']} = {r.get('RUNNABLE')} + {r.get('CHILD')} + {r.get('MANUAL')} + "
            f"{r.get('UNCLASSIFIED')}",
            s["filters_rust"] == sum(r.get(k, 0) for k in ROLES))]
    if "executed" in s:
        # **로그 형태를 못 읽은 필터를 따로 센다.** 그 항을 빼면 `-q` 가 항등식 불성립(= 판정 보류)으로
        # 보이는데, 실제 사유는 "못 읽었다"(= 미검증)다. 두 사유가 섞이면 다음 사람이 엉뚱한 데를 고친다.
        nr = len(s.get("log_form_unreadable") or [])
        out.append(("② 자기 실행(ran≥1) + 자식 입증(listed≥1) + 로그 못 읽음 = Rust 필터",
                    f"{s['ran_ge_1_self']} + {s['child_proven']} + {nr} = {s['filters_rust']}",
                    s["ran_ge_1_self"] + s["child_proven"] + nr == s["filters_rust"]))
    if c:
        out.append(("③ 레포 #[ignore] = CHILD + MANUAL + 미분류",
                    f"{c['raw_ignore_attrs']} = {len(c['child'])} + {len(c['manual'])} + {c['unclassified']}",
                    c["raw_ignore_attrs"] == len(c["child"]) + len(c["manual"]) + c["unclassified"]))
    return out


def undecidable(summary: dict) -> list[str]:
    """**판정하지 않는다**(exit 4) — 분류가 깨졌으면 "위반 0" 이 아니라 멈춘다."""
    why = []
    c = summary.get("ignore_census") or {}
    if c.get("unclassified"):
        why.append(f"#[ignore] 미분류 {c['unclassified']} 건(속성을 테스트 함수에 붙이지 못했다 — 매크로·cfg?)")
    if (summary.get("roles") or {}).get("UNCLASSIFIED"):
        why.append(f"역할 UNCLASSIFIED 필터 {summary['roles']['UNCLASSIFIED']} 건")
    for sc, n in summary.get("child_parent_missing") or []:
        why.append(f"CHILD {sc} `{n}` 의 부모가 같은 SC 에 지명되지 않았다 — 자식 실행 수를 읽을 자리가 없다")
    for name, lhs, ok in identities(summary):
        if not ok:
            why.append(f"항등식 {name} 불성립: {lhs}")
    return why


def unmeasured(summary: dict) -> list[str]:
    """**미검증(관측 없음)**(exit 5) — 깨진 것은 없고 **관측 경로가 끊겼다.** PASS 는 "쟀고 통과했다" 를
    뜻하므로 여기 해당하면 PASS 가 아니다. FAIL 도 아니다(깨진 것이 없다)."""
    why = []
    for sc, n in summary.get("child_unmeasured") or []:
        why.append(f"CHILD {sc} `{n}` — 부모가 돌았는데 `---- <부모> stdout ----` 블록이 없다. "
                   f"표준 게이트 명령에 `-- --show-output` 이 빠졌다(`--nocapture` 는 splice 때문에 쓰지 않는다)")
    for sc, n in summary.get("child_unobserved") or []:
        why.append(f"CHILD {sc} `{n}` — 블록은 있는데 자식 실행 줄이 0 이다. 계약 SC 가 요구한 "
                   f"\"서로 다른 프로세스에서 2 회\" 중 **한 쪽을 이 실행이 재지 못했다**")
    for sc, n in summary.get("child_expected_unknown") or []:
        why.append(f"CHILD {sc} `{n}` — 기대 회차를 부모 본문에서 못 셌다(spawn 헬퍼 미확인)")
    pc = summary.get("parent_census") or {}
    for ch in pc.get("partial") or []:
        r = next((x for x in pc["rows"] if x["child"] == ch), {})
        why.append(f"부모 센서스 {pc.get('denominator')} 중 `{ch}` — 표준 게이트 로그의 자식 실행 수 "
                   f"{r.get('runs')} ≠ 소스가 요구하는 {r.get('expected')}. **일부만 찍힌 로그로는 판정할 수 "
                   f"없다**(`-- --show-output` 이 모든 테스트 바이너리에 전달됐는지)")
    for sc, n in summary.get("log_form_unreadable") or []:
        why.append(f"{sc} `{n}` — 결과 줄은 통과를 말하는데 `test … ok` 줄이 0 이다. 로그 형태를 못 읽었다"
                   f"(`-q`?). **유령이 아니라 미검증이다** — 유령으로 인쇄하면 다음 사람이 집계를 느슨하게 고친다")
    return why


def exit_code(summary: dict) -> int:
    if undecidable(summary):
        return 4
    bad = summary["static_ghosts"] or summary.get("ghosts") or summary.get("failed_tests") \
        or summary.get("nonzero_rc") or (summary.get("unity_bare") or {}).get("ghosts") \
        or summary.get("child_pid_collisions") or summary.get("child_pid_equals_parent") \
        or summary.get("child_runs_mismatch") or (summary.get("parent_census") or {}).get("pid_bad")
    if bad:
        return 1
    return 5 if unmeasured(summary) else 0


def write_outputs(out: Path, ext: dict, results, summary, unity=None) -> None:
    out.mkdir(parents=True, exist_ok=True)
    doc = {"summary": summary, "bare": ext["bare"],
           "filters": results if results is not None else [_fdict(f) for f in ext["filters"]]}
    if unity is not None:
        doc["unity_bare"] = unity
    (out / "filters.json").write_text(json.dumps(doc, ensure_ascii=False, indent=2), encoding="utf-8")
    if results is not None:
        lines = ["sc\tcrate\tflags\tfilter\tsource\trole\tmode\tlisted\tchild_runs\trc\tran\tfailed\tscope"
                 "\talso_named_by\tnames"]
        for r in results:
            lines.append("\t".join([r["sc"], r["crate"], " ".join(r["cargo_flags"]) or ".", r["name"] or ".",
                                    r["source"], r.get("role", "?"), r.get("mode", "?"),
                                    str(r.get("listed", ".")), str(r.get("child_runs", ".")),
                                    str(r["rc"]), str(r["ran"]), str(len(r["failed"])), r["scope"],
                                    json.dumps(r["also_named_by"], ensure_ascii=False) if r["also_named_by"] else ".",
                                    " ".join(r["names"][:20])]))
        (out / "filters_summary.tsv").write_text("\n".join(lines) + "\n", encoding="utf-8", newline="\n")


def print_summary(s: dict) -> None:
    print(f"계약 §1 행 {s['rows']} · Rust 필터 {s['filters_rust']}(cargo {s['filters_rust_cargo']} + 맨 이름 "
          f"{s['filters_rust_bare']}) · Unity 맨 이름 {s['filters_unity_bare']} · 맨 토큰 {s['bare_tokens']} "
          f"{s['bare_by_category']}")
    for sc, t in s["static_ghosts"]:
        print(f"  유령(코드에 없음): {sc} {t}")
    r, c = s.get("roles") or {}, s.get("ignore_census") or {}
    print(f"역할(소스에서) RUNNABLE {r.get('RUNNABLE')} · CHILD {r.get('CHILD')} · MANUAL {r.get('MANUAL')} · "
          f"UNCLASSIFIED {r.get('UNCLASSIFIED')}")
    if c:
        print(f"레포 #[ignore] {c['raw_ignore_attrs']}(붙은 것 {c['attached']}) · CHILD {c['child']} · "
              f"MANUAL {c['manual']} · 미분류 {c['unclassified']}")
    pr = s.get("prose_runnable") or {}
    print(f"산문(백틱 맨 이름) RUNNABLE 분모 — Rust {pr.get('rust')} · Unity 맨 이름 {pr.get('unity_bare')} "
          f"(D-1 추가 만기 2: 닫지 않고 센다. 이 이름들은 돌고 초록이 되어 분자를 부풀릴 수 있다)")
    for row in s.get("child_rows") or []:
        print(f"  CHILD {row['sc']} `{row['name']}`: --list --ignored 존재 {row['listed']} · "
              f"증거 우물 {row['child_evidence_source']} · "
              f"측정 경로(--show-output 블록) {'열림' if row['child_measurable'] else '닫힘'} "
              f"{row['stdout_blocks']} · "
              f"자식 실행 수 {row['child_runs']} / 소스가 요구하는 회차 {row['expected_child_runs']} "
              f"(이름 언급 {row['child_mentions']}) · "
              f"자식 pid {row['child_pids']} → 서로 다른 pid {row['distinct_child_pids']} · "
              f"부모 필터 {row['parent_filters'] or '없음'}")
    for sc, n, got, exp in s.get("child_runs_mismatch") or []:
        print(f"  자식 회차 불일치: {sc} `{n}` 로그 {got} 회 ≠ 부모 본문의 spawn 호출 {exp} 회")
    for sc, n, pids in s.get("child_pid_collisions") or []:
        print(f"  자식 pid 충돌: {sc} `{n}` {pids} — 같은 프로세스를 두 번 적었다. "
              f"\"서로 다른 프로세스에서 2 회\" 가 성립하지 않는다")
    for sc, n, pids in s.get("child_pid_equals_parent") or []:
        print(f"  자식 pid = 부모 pid: {sc} `{n}` {pids} — 별도 프로세스가 아니다")
    pc = s.get("parent_census")
    if pc:
        print(f"부모 센서스 — 분모 {pc['denominator']}(레포의 `#[ignore]` 자식을 spawn 하는 부모 전체) · "
              f"로그 {pc['log']}")
        for r in pc["rows"]:
            print(f"  {r['child']} ← {r['parents']}: 자식 실행 {r['runs']} / 소스 요구 {r['expected']} · "
                  f"pid {r['pids']} 서로 다른 pid {r['distinct_pids']} · 블록 "
                  f"{'있음' if r['measurable'] else '없음'}")
        print(f"  일부만 찍힌 부모 {pc['partial'] or '없음'} · pid 이상 {pc['pid_bad'] or '없음'}")
    for name, lhs, ok in identities(s):
        print(f"  항등식 {name}: {lhs}  {'성립' if ok else '불성립'}")
    for w in undecidable(s):
        print(f"  판정 보류(exit 4): {w}")
    for w in unmeasured(s):
        print(f"  미검증(관측 없음, exit 5): {w}")
    if "executed" in s:
        print(f"실행 {s['executed']} · 실행 ≥ 1 = {s['ran_ge_1']} · 유령(실행 0) {len(s['ghosts'])} · "
              f"실패 테스트 {len(s['failed_tests'])} · rc≠0 {len(s['nonzero_rc'])} · 다른 SC 도 잡는 테스트 {len(s['cross_sc'])}")
        for g in s["ghosts"]:
            print(f"  유령(실행 0): {g[0]} {g[1]} `{g[2]}`")
        for sc, t, o in s["cross_sc"]:
            print(f"  교차: {sc} 의 필터가 잡은 {t} — {', '.join(o)} 도 지명")
    if "unity_bare" in s:
        print(f"Unity 맨 이름 {s['unity_bare']['checked']} · xml 에 없음 {len(s['unity_bare']['ghosts'])}")


# ---------------------------------------------------------------- selftest

FAKE_CARGO = r'''
import json, os, sys
TESTS = {"starfall-sim": ["mining::reject_does_not_start_cooldown", "mining::each_rejection_reason_leaves_x",
                          "mining::each_rejection_reason_leaves_x_reasons_3_6_7"],
         "starfall-gateway": ["ws::tests::recording_lag_boundary_rejects", "mine_resource_counted"],
         "starfall-replay": ["demo_replay_worker", "sc_two_process_demo", "demo_live_smoke"]}
INTEG = {("starfall-gateway", "ws_integration"): ["mine_resource_counted"]}
IGNORED = {"demo_replay_worker", "demo_live_smoke"}
# 부모가 성공 경로에서 자식 흔적을 찍는 **두 형태** + 세면 안 되는 미끼 둘.
# 수는 도구에 박힌 것이 아니라 이 로그에서 나온다.
PARENT_PRINTS_CHILD = {
    "sc_two_process_demo": [
        "[CHILD] worker=demo_replay_worker run=1 child_pid=11 parent_pid=99 :: test result: ok. 1 passed; 0 failed",
        "[CHILD] worker=demo_replay_worker run=2 child_pid=22 parent_pid=99 :: test result: ok. 1 passed; 0 failed",
        "test demo_replay_worker ... ignored, 부모가 자식 프로세스로만 실행한다",   # 미끼 ①: 건너뜀 기록
        # 미끼 ②: rust 가 자식 이름에 오타를 냈을 때 determinism 자식이 낸 모양 — exit 0 인데 0 건이다
        "[CHILD] worker=demo_replay_worker run=3 child_pid=33 parent_pid=99 :: "
        "test result: ok. 0 passed; 0 failed; 0 ignored; 2 filtered out",
    ],
}
# 다른 테스트의 `stdout` 블록에 **같은 자식 이름**이 흘러도 세지 않는다 — 블록 앵커가 그것을 가른다
DECOY_BLOCKS = {"unrelated_test": [
    "[CHILD] worker=demo_replay_worker run=9 child_pid=44 parent_pid=99 :: test result: ok. 1 passed; 0 failed",
]}
# `println!` 은 libtest 가 캡처한다 — **`--nocapture` 가 없으면 이 줄들은 로그에 안 남는다.**
# 진짜 cargo 가 그렇게 동작하는 것을 실측했다(trading_replay 부모: 플래그 있으면 2 줄, 없으면 0 줄).
# 케이스마다 자기 픽스처를 주입한다(`FAKE_CARGO_EXTRA` = JSON 파일) — 내장 dict 를 고치지 않는다
_x = os.environ.get("FAKE_CARGO_EXTRA")
if _x:
    _c = json.load(open(_x, encoding="utf-8"))
    TESTS.update(_c.get("tests", {}))
    IGNORED |= set(_c.get("ignored", []))
    PARENT_PRINTS_CHILD.update(_c.get("prints", {}))
    DECOY_BLOCKS.clear()
    DECOY_BLOCKS.update(_c.get("decoys", {}))
a = sys.argv[1:]
assert a[0] == "test", a
crate = a[a.index("-p") + 1]
dd = a.index("--")
cflags, rest = a[:dd], a[dd + 1:]
names = [x for x in rest if not x.startswith("-")]
# 진짜 cargo 처럼 `--` 앞의 위치 인자도 테스트 이름 필터다(값 플래그의 값은 제외)
skip = False
for x in cflags[1:]:
    if skip:
        skip = False
    elif x in ("-p", "--test", "--features"):
        skip = True
    elif not x.startswith("-"):
        names.append(x)
tests = TESTS.get(crate, [])
if "--test" in cflags:
    tests = INTEG.get((crate, cflags[cflags.index("--test") + 1]), None)
    if tests is None:
        print("error: no test target named `%s`" % cflags[cflags.index("--test") + 1]); sys.exit(101)
sel = [t for t in tests if not names or any(n in t for n in names)]
want_ignored = "--ignored" in rest
pool = [t for t in sel if (t.split("::")[-1] in IGNORED) == want_ignored]
if "--list" in rest:
    for t in pool:
        print("%s: test" % t)
    print("%d test%s, 0 benchmarks" % (len(pool), "" if len(pool) == 1 else "s"))
    sys.exit(0)
skipped = [t for t in sel if t not in pool]
quiet = "-q" in cflags or "--quiet" in cflags
print("")
print("running %d tests" % len(sel))
if quiet:
    # 진짜 `cargo test -q` 처럼 **`test X ... ok` 줄을 아예 안 찍는다**(점 진행만). 실측으로 확인했다.
    print("".join("i" if t in skipped else "." for t in sel))
else:
    for t in pool:
        print("test %s ... ok" % t)
    for t in skipped:
        print("test %s ... ignored" % t)
    # `--show-output` 은 캡처를 유지한 채 **실행 뒤** `---- <테스트> stdout ----` 블록으로 찍는다
    shown = [(t, PARENT_PRINTS_CHILD[t.split("::")[-1]]) for t in pool if t.split("::")[-1] in PARENT_PRINTS_CHILD]
    # **다른 테스트의 블록**에 같은 자식 이름을 흘리는 미끼. 블록 앵커가 없으면 이것까지 세게 된다.
    shown += [(k, v) for k, v in DECOY_BLOCKS.items()] if shown else []
    if "--show-output" in rest and shown:
        print("")
        print("successes:")
        print("")
        for t, lines in shown:
            print("---- %s stdout ----" % t)
            for line in lines:
                print(line)
            print("")
print("")
print("test result: ok. %d passed; 0 failed; %d ignored; 0 measured; %d filtered out"
      % (len(pool), len(skipped), len(tests) - len(sel)))
'''


def _legacy_shell_transport(filters: list[Filter]) -> list[Filter]:
    """옛 길(r2·r3 1차)을 그대로 흉내 낸다: Windows Python 이 TSV 를 CRLF 로 쓰고, bash `read` 가 IFS 탭으로
    빈 칸을 접는다. 결함 3 의 음성 대조 전용."""
    blob = "".join("\t".join([f.sc, f.crate, " ".join(f.cargo_flags), f.name]) + "\r\n" for f in filters)
    out = []
    for line in blob.split("\n"):
        if not line:
            continue
        cols = re.split(r"\t+", line)          # IFS 공백류 문자는 연속이면 하나로 접힌다
        cols += [""] * (4 - len(cols))
        sc, crate, flags, name = cols[:4]
        # 따옴표 없는 `$flags` 의 단어 나눔은 IFS(공백·탭·줄바꿈)로만 — CR 은 IFS 가 아니라 남는다
        out.append(Filter(sc, "cargo", "", "rust", crate, "server", [x for x in re.split(r"[ \t\n]+", flags) if x],
                          name, []))
    return out


def selftest() -> int:
    tmp = Path(tempfile.mkdtemp(prefix="cargo_sc_map_selftest_"))
    # 합성 코드 색인: Rust 테스트 3, Unity 테스트 1, 필드 이름 1, 주석에만 나오는 이름 1
    (tmp / "server" / "crates" / "sim" / "src").mkdir(parents=True)
    (tmp / "server" / "Cargo.toml").write_text('[workspace]\nmembers = ["crates/*"]\n', encoding="utf-8")
    (tmp / "server" / "crates" / "sim" / "Cargo.toml").write_text('[package]\nname = "starfall-sim"\n', encoding="utf-8")
    (tmp / "server" / "crates" / "sim" / "src" / "lib.rs").write_text(
        "pub struct S { pub quantity_kg: i64 }\n// halted_only_in_comment 은 주석이다\n"
        "#[cfg(test)]\nmod mining {\n  #[test]\n  fn reject_does_not_start_cooldown() {}\n"
        "  #[test]\n  fn each_rejection_reason_leaves_x() {}\n  #[tokio::test]\n  async fn each_rejection_reason_leaves_x_reasons_3_6_7() {}\n}\n",
        encoding="utf-8")
    (tmp / "server" / "crates" / "gw" / "src").mkdir(parents=True)
    (tmp / "server" / "crates" / "gw" / "Cargo.toml").write_text('[package]\nname = "starfall-gateway"\n', encoding="utf-8")
    (tmp / "server" / "crates" / "gw" / "src" / "ws.rs").write_text(
        "#[test]\nfn recording_lag_boundary_rejects() {}\n#[tokio::test]\nasync fn mine_resource_counted() {}\n", encoding="utf-8")
    # D-1: CHILD(부모가 spawn 하는 `#[ignore]`) 와 MANUAL(부모 리터럴 없음) 을 한 크레이트에 둔다.
    (tmp / "server" / "crates" / "replay" / "tests").mkdir(parents=True)
    (tmp / "server" / "crates" / "replay" / "Cargo.toml").write_text('[package]\nname = "starfall-replay"\n',
                                                                     encoding="utf-8")
    (tmp / "server" / "crates" / "replay" / "tests" / "replay.rs").write_text(
        '//! 주석에서 demo_replay_worker 를 언급해도 리터럴이 아니다.\n'
        '#[test]\n#[ignore = "부모(sc_two_process_demo) 가 자식 프로세스로만 실행한다"]\n'
        'fn demo_replay_worker() {}\n\n'
        'fn spawn_child() {\n  let _ = ["demo_replay_worker", "--exact", "--ignored"];\n}\n\n'
        '#[test]\nfn sc_two_process_demo() {\n  spawn_child();\n  spawn_child();\n}\n\n'
        '#[test]\n#[ignore = "실서버가 필요하다"]\nfn demo_live_smoke() {}\n', encoding="utf-8")
    (tmp / "client" / "Assets" / "_Project" / "Tests").mkdir(parents=True)
    (tmp / "client" / "Assets" / "_Project" / "Tests" / "T.cs").write_text(
        "class T {\n  [Test]\n  public void PilotLabel_LastFourChars() {}\n}\n", encoding="utf-8")
    idx = build_index(tmp)
    fake = tmp / "fake_cargo.py"
    fake.write_text(FAKE_CARGO, encoding="utf-8")
    cargo = [sys.executable, str(fake)]

    # 결함 1: 맨 이름(+ 유령), 결함 2: --test X 뒤 이름, 결함 3: `\|` 가 앞 칸에 있어 방법 칸이 밀림 + CRLF
    contract = ("## 1. 검증 항목\r\n"
                "| SC | 기준 | 방법 | ⊘ |\r\n"
                "| SC-28 | x | `cargo test -p starfall-gateway recording_lag_boundary_*` + `halted_rejects_ghost` | y |\r\n"
                "| SC-09 | x | `cargo test -p starfall-gateway --test ws_integration mine_resource_counted` | y |\r\n"
                "| SC-41 | `grep -nE \"a\\|b\"` 로 본다 | `cargo test -p starfall-sim mining::each_rejection_reason` + "
                "`each_rejection_reason_leaves_x_reasons_3_6_7` · `quantity_kg` · `halted_only_in_comment` | z |\r\n"
                "| SC-11 | x | `cargo test -p starfall-sim mining::reject_*` | y |\r\n"
                "| SC-12 | x | `cargo test -p starfall-sim mining::reject_does_not_start_cooldown` | y |\r\n"
                "| SC-70 | x | `PilotLabel_LastFourChars` (Unity) | y |\r\n"
                "| SC-71 | x | `cargo test -p starfall-sim --release` | y |\r\n"
                "| SC-42 | x | `cargo test -p starfall-replay sc_two_process_demo` (자식 `demo_replay_worker` 는 "
                "`#[ignore]`, 부모가 부른다) · `demo_replay_worker` | y |\r\n"
                "| SC-43 | x | `demo_live_smoke` | y |\r\n"
                "## 2. 다른 절\r\n| SC-99 | x | `cargo test -p starfall-sim not_section_1` | y |\r\n")

    def names(ext, kind="rust"):
        return sorted((f.sc, f.name) for f in ext["filters"] if f.kind == kind)

    full = extract(contract, idx, Fixes(), tmp)
    off1 = extract(contract, idx, Fixes(bare=False), tmp)
    off2 = extract(contract, idx, Fixes(value_flags=False), tmp)
    off3 = extract(contract, idx, Fixes(cell_split=False), tmp)
    off_role = extract(contract, idx, Fixes(ignore_role=False), tmp)
    res = run_filters(full["filters"], tmp / "out", cargo=cargo, env=dict(os.environ), root=tmp,
                      progress=lambda *_: None, idx=idx)
    res_role_off = run_filters(off_role["filters"], tmp / "out_role_off", cargo=cargo, env=dict(os.environ),
                               root=tmp, progress=lambda *_: None, idx=idx)
    sm_role_off = summarize(off_role, res_role_off)
    # 음성 대조: `--show-output` 을 빼면 블록이 없다 → **미검증(exit 5)**, PASS 도 FAIL 도 아니다
    res_so_off = run_filters(extract(contract, idx, Fixes(), tmp)["filters"], tmp / "out_so_off", cargo=cargo,
                             env=dict(os.environ), root=tmp, progress=lambda *_: None, idx=idx, show_output=False)
    sm_so_off = summarize(full, res_so_off)

    def mini(tag: str, spawn_calls: int, lines: list, show_output: bool = True, extra_cargo: str = "") -> tuple:
        """픽스처 하나짜리 미니 레포 — CHILD 결과 네 갈래를 **입력을 바꿔** 확인한다.
        기대 회차는 부모 본문의 spawn 호출 수에서 나오므로 `spawn_calls` 가 그 입력이다."""
        d = Path(tempfile.mkdtemp(prefix=f"csm_{tag}_"))
        crate, child, parent = f"starfall-{tag}", f"{tag}_worker", f"sc_{tag}"
        (d / "server" / "crates" / "x" / "tests").mkdir(parents=True)
        (d / "server" / "Cargo.toml").write_text('[workspace]\nmembers = ["crates/*"]\n', encoding="utf-8")
        (d / "server" / "crates" / "x" / "Cargo.toml").write_text(f'[package]\nname = "{crate}"\n', encoding="utf-8")
        calls = "\n".join(["  spawn_x();"] * spawn_calls)
        (d / "server" / "crates" / "x" / "tests" / "t.rs").write_text(
            f'#[test]\n#[ignore = "부모가 자식 프로세스로만 실행한다"]\nfn {child}() {{}}\n\n'
            f'fn spawn_x() {{ let _ = "{child}"; }}\n\n'
            f'#[test]\nfn {parent}() {{\n{calls}\n}}\n', encoding="utf-8")
        cfg = d / "fake.json"
        cfg.write_text(json.dumps({"tests": {crate: [child, parent]}, "ignored": [child],
                                   "prints": {parent: lines}}), encoding="utf-8")
        i = build_index(d)
        e = extract(f"## 1. x\n| SC-50 | x | `cargo test -p {crate}{extra_cargo} {parent}` · `{child}` | y |\n",
                    i, Fixes(), d)
        r = run_filters(e["filters"], d / "out", cargo=cargo, env={**os.environ, "FAKE_CARGO_EXTRA": str(cfg)},
                        root=d, progress=lambda *_: None, idx=i, show_output=show_output)
        sm = summarize(e, r)
        shutil.rmtree(d, ignore_errors=True)
        return sm, next((x for x in r if x["role"] == "CHILD"), None), r

    CH = "[CHILD] worker={w} run={n} child_pid={p} parent_pid=99 :: test result: ok. 1 passed; 0 failed"

    # 부모 센서스(분모 3) — 자식 셋을 가진 합성 레포 + **표준 게이트 로그를 글자로 만들어** 먹인다
    pc_root = Path(tempfile.mkdtemp(prefix="csm_pcen_"))
    (pc_root / "server" / "crates" / "x" / "tests").mkdir(parents=True)
    (pc_root / "server" / "Cargo.toml").write_text('[workspace]\nmembers = ["crates/*"]\n', encoding="utf-8")
    (pc_root / "server" / "crates" / "x" / "Cargo.toml").write_text('[package]\nname = "starfall-pc"\n',
                                                                    encoding="utf-8")
    for tag in ("a", "b", "c"):
        (pc_root / "server" / "crates" / "x" / "tests" / f"{tag}.rs").write_text(
            f'#[test]\n#[ignore = "부모가 자식 프로세스로만 실행한다"]\nfn {tag}_worker() {{}}\n\n'
            f'fn spawn_{tag}() {{ let _ = "{tag}_worker"; }}\n\n'
            f'#[test]\nfn p_{tag}() {{\n  spawn_{tag}();\n  spawn_{tag}();\n}}\n', encoding="utf-8")
    pc_idx = build_index(pc_root)
    pc_ext = extract("## 1. x\n| SC-60 | x | `cargo test -p starfall-pc p_a` | y |\n", pc_idx, Fixes(), pc_root)

    def gate_log(*entries) -> Path:
        """`--show-output` 형태의 표준 게이트 로그를 글자로 만든다(블록 머리 + 자식 줄)."""
        body = ["", "running 6 tests", "", "successes:", ""]
        for parent, lines in entries:
            body += [f"---- {parent} stdout ----", *lines, ""]
        body += ["test result: ok. 6 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out"]
        p = pc_root / f"gate_{len(list(pc_root.glob('gate_*.log')))}.log"
        p.write_text("\n".join(body), encoding="utf-8")
        return p

    def two(tag, p1, p2):
        return (f"p_{tag}", [CH.format(w=f"{tag}_worker", n=1, p=p1), CH.format(w=f"{tag}_worker", n=2, p=p2)])

    pcen_ok = parent_census(pc_idx, gate_log(two("a", "11", "12"), two("b", "21", "22"), two("c", "31", "32")))
    pcen_part = parent_census(pc_idx, gate_log(two("a", "11", "12")))          # 한 부모만 찍혔다
    pcen_one = parent_census(pc_idx, gate_log(("p_a", [CH.format(w="a_worker", n=1, p="11")])))
    pcen_three = parent_census(pc_idx, gate_log(("p_a", [CH.format(w="a_worker", n=i, p=str(10 + i))
                                                         for i in (1, 2, 3)])))
    pcen_samepid = parent_census(pc_idx, gate_log(two("a", "11", "11"), two("b", "21", "22"), two("c", "31", "32")))
    shutil.rmtree(pc_root, ignore_errors=True)
    pass_sm, pass_ch, _ = mini("okcase", 2, [CH.format(w="okcase_worker", n=1, p="11"),
                                             CH.format(w="okcase_worker", n=2, p="22")])
    coll_sm, coll_ch, _ = mini("collide", 2, [CH.format(w="collide_worker", n=1, p="77"),
                                              CH.format(w="collide_worker", n=2, p="77")])
    one_sm, one_ch, _ = mini("onerun", 2, [CH.format(w="onerun_worker", n=1, p="11")])
    # 소스가 **3 회**를 요구하는 입력 — 기대값을 2 로 박은 판은 여기서 깨진다
    three_sm, three_ch, _ = mini("threecalls", 3, [CH.format(w="threecalls_worker", n=i, p=str(10 + i))
                                                   for i in (1, 2, 3)])
    noobs_sm, noobs_ch, _ = mini("noobs", 2, ["[note] 부모가 뭔가 찍지만 자식 흔적은 없다"])
    verb_sm, verb_ch, _ = mini("verbatim", 2, ["test verbatim_worker ... ok", "test verbatim_worker ... ok"])
    quiet_sm, _, quiet_res = mini("quietcase", 2, [CH.format(w="quietcase_worker", n=1, p="11"),
                                                   CH.format(w="quietcase_worker", n=2, p="22")],
                                  extra_cargo=" -q")
    nosh_sm, nosh_ch, _ = mini("noshow", 2, [CH.format(w="noshow_worker", n=1, p="11"),
                                             CH.format(w="noshow_worker", n=2, p="22")], show_output=False)
    by = {(r["sc"], r["name"]): r for r in res}
    named_cargo = [f for f in full["filters"] if f.kind == "rust" and f.source == "cargo" and f.name]
    legacy = _legacy_shell_transport(named_cargo)
    legacy_res = run_filters(legacy, tmp / "out_legacy", cargo=cargo, env=dict(os.environ), root=tmp,
                             progress=lambda *_: None)
    # 같은 입력 필터를 위치로 짝짓는다 — 키가 없어서 0 으로 읽히는 공짜 통과를 막는다
    assert len(legacy_res) == len(named_cargo), (len(legacy_res), len(named_cargo))
    lby = {(f.sc, f.name): r for f, r in zip(named_cargo, legacy_res)}
    sm = summarize(full, res)

    cases = [
        # 결함 1
        ("결함1: 맨 이름 TEST_FN 을 실행 목록에 넣는다(SC-41 `…_reasons_3_6_7`)",
         ("SC-41", "each_rejection_reason_leaves_x_reasons_3_6_7") in names(full)),
        ("결함1: 코드에 없는 맨 이름 = 정적 유령(SC-28 `halted_rejects_ghost`)",
         [("SC-28", "halted_rejects_ghost"), ("SC-41", "halted_only_in_comment")]
         == [(b["sc"], b["token"]) for b in full["static_ghosts"]]),
        ("결함1: 주석에만 나오는 이름도 유령이다(주석 제외 색인)",
         any(b["token"] == "halted_only_in_comment" and b["category"] == "GHOST" for b in full["bare"])),
        ("결함1: 필드 이름은 NON_TEST_IDENT 이고 실행 목록에 없다",
         any(b["token"] == "quantity_kg" and b["category"] == "NON_TEST_IDENT" for b in full["bare"])
         and ("SC-41", "quantity_kg") not in names(full)),
        ("결함1 음성 대조: 맨 이름 처리를 끄면 유령도 TEST_FN 도 못 줍는다",
         not off1["static_ghosts"] and ("SC-41", "each_rejection_reason_leaves_x_reasons_3_6_7") not in names(off1)),
        # 결함 2
        ("결함2: `--test ws_integration X` → 플래그 값과 이름을 나눈다",
         any(f.sc == "SC-09" and f.cargo_flags == ["--test", "ws_integration"] and f.name == "mine_resource_counted"
             for f in full["filters"])),
        ("결함2: 그 필터가 실제로 1 건 실행된다", by[("SC-09", "mine_resource_counted")]["ran"] == 1),
        ("결함2 음성 대조: 값 플래그 처리를 끄면 이름이 `ws_integration` 으로 바뀐다",
         any(f.sc == "SC-09" and f.name == "ws_integration" for f in off2["filters"])
         and not any(f.sc == "SC-09" and f.name == "mine_resource_counted" for f in off2["filters"])),
        # 결함 3
        ("결함3: `\\|` 가 앞 칸에 있어도 방법 칸을 읽는다(SC-41 cargo 필터)",
         ("SC-41", "mining::each_rejection_reason") in names(full)),
        ("결함3 음성 대조: 이스케이프 무시 split 이면 SC-41 필터를 잃는다",
         ("SC-41", "mining::each_rejection_reason") not in names(off3)),
        ("결함3: CRLF 계약에서도 직접 subprocess 경로는 SC-11·12·28 필터를 각 ≥ 1 실행",
         all(by[k]["ran"] >= 1 for k in [("SC-11", "mining::reject_*"),
                                         ("SC-12", "mining::reject_does_not_start_cooldown"),
                                         ("SC-28", "recording_lag_boundary_*")])),
        ("결함3 음성 대조: 옛 TSV(CRLF)+IFS 길이면 같은 필터가 0 건(이름 끝 CR · 빈 칸 접힘)",
         lby[("SC-12", "mining::reject_does_not_start_cooldown")]["ran"] == 0
         and lby[("SC-11", "mining::reject_*")]["ran"] == 0
         and all(by[k]["ran"] >= 1 for k in lby)),
        # 유령·주인
        ("§1 밖의 행은 줍지 않는다", not any(f.sc == "SC-99" for f in full["filters"])),
        ("이름 없는 `--release` 필터는 대상 전체·주인 대조 제외",
         by[("SC-71", "")]["scope"] == "whole" and by[("SC-71", "")]["ran"] == 3),
        ("주인: SC-11 `mining::reject_*` 가 잡은 테스트는 SC-12 도 지명 → 교차로 찍힌다",
         by[("SC-11", "mining::reject_*")]["also_named_by"] == {"mining::reject_does_not_start_cooldown": ["SC-12"]}),
        ("주인: SC-41 필터 2 개가 잡은 테스트는 같은 SC 라 교차 아님",
         by[("SC-41", "mining::each_rejection_reason")]["also_named_by"] == {}),
        ("Unity 맨 이름은 cargo 로 돌리지 않는다", names(full, "unity") == [("SC-70", "PilotLabel_LastFourChars")]
         and not any(r["kind"] == "unity" for r in res)),
        # D-1 역할 분류
        ("D-1: 역할을 소스에서 읽는다 — CHILD 1(`#[ignore]`+부모 리터럴) · MANUAL 1 · 나머지 RUNNABLE",
         sm["roles"]["CHILD"] == 1 and sm["roles"]["MANUAL"] == 1 and sm["roles"]["UNCLASSIFIED"] == 0
         and by[("SC-42", "demo_replay_worker")]["role"] == "CHILD"
         and by[("SC-43", "demo_live_smoke")]["role"] == "MANUAL"
         and by[("SC-42", "sc_two_process_demo")]["role"] == "RUNNABLE"),
        ("D-1: CHILD 는 단독 실행하지 않는다 — `--list --ignored` 로 존재 1, argv 에 `--list` 가 있다",
         by[("SC-42", "demo_replay_worker")]["mode"] == "list"
         and by[("SC-42", "demo_replay_worker")]["listed"] == 1
         and "--list" in by[("SC-42", "demo_replay_worker")]["argv"]
         and not by[("SC-42", "demo_replay_worker")]["ghost"]),
        ("D-1: CHILD 의 자식 실행 수를 **부모 로그에서 세어** 찍는다(도구에 기대값을 박지 않는다). "
         "미끼 둘(`... ignored` · **`0 passed`**)은 안 센다 — 부모 블록의 자식 이름 4 줄 중 2 줄만 실행이다",
         by[("SC-42", "demo_replay_worker")]["child_runs"] == 2
         and by[("SC-42", "demo_replay_worker")]["parent_filters"][0]["name"] == "sc_two_process_demo"
         and by[("SC-42", "demo_replay_worker")]["parent_named"]),
        ("D-2: 부모 필터를 `--show-output` 으로 돌린다(`--nocapture` 는 splice 때문에 쓰지 않는다) — "
         "`---- <부모> stdout ----` 블록 **안에서만** 세므로 부모 귀속이 로그 구조에서 나온다",
         by[("SC-42", "sc_two_process_demo")]["show_output"] is True
         and "--show-output" in by[("SC-42", "sc_two_process_demo")]["argv"]
         and "--nocapture" not in by[("SC-42", "sc_two_process_demo")]["argv"]
         and by[("SC-42", "demo_replay_worker")]["stdout_blocks"] == ["sc_two_process_demo"]
         and by[("SC-42", "demo_replay_worker")]["child_measurable"] is True
         # 표준 게이트 로그가 주 우물이고, 없으면 필터 로그로 떨어진다
         and by[("SC-42", "demo_replay_worker")]["child_evidence_source"] == "filter-log"
         # 필요한 로그에만 붙인다 — 관계없는 필터는 그대로다(로그가 커지는 대가를 안 치른다)
         and by[("SC-12", "mining::reject_does_not_start_cooldown")]["show_output"] is False
         and sum(1 for r in res if r.get("show_output")) == 1),
        ("D-2: **다른 테스트의 블록**에 같은 자식 이름이 흘러도 세지 않는다 — 언급 5 ≠ 실행 2",
         by[("SC-42", "demo_replay_worker")]["child_mentions"] == 5
         and by[("SC-42", "demo_replay_worker")]["child_runs"] == 2
         and "unrelated_test" not in by[("SC-42", "demo_replay_worker")]["stdout_blocks"]),
        ("D-2: 기대 회차를 **부모 본문의 spawn 호출 수**로 센다 — 도구에 2 를 박지 않는다. "
         "소스가 3 회를 요구하는 입력에서는 3 이 나오고 그때도 PASS 다",
         by[("SC-42", "demo_replay_worker")]["expected_child_runs"] == 2
         and sm["child_runs_mismatch"] == [] and sm["child_expected_unknown"] == []
         and three_ch["expected_child_runs"] == 3 and three_ch["child_runs"] == 3
         and exit_code(three_sm) == 0),
        ("리더 지시: `[CHILD] … child_pid` 두 개가 **서로 다른지** 센다 — \"서로 다른 프로세스에서 2 회\"",
         by[("SC-42", "demo_replay_worker")]["child_pids"] == ["11", "22"]
         and by[("SC-42", "demo_replay_worker")]["distinct_child_pids"] == 2
         and by[("SC-42", "demo_replay_worker")]["pid_equals_parent"] == []
         and sm["child_pid_collisions"] == [] and sm["child_pid_equals_parent"] == []),
        ("리더 지시 음성 대조: `--show-output` 을 빼면 **미검증(exit 5)** 이다 — PASS 도 FAIL 도 아니다. "
         "같은 입력에 플래그만 켠 짝이 exit 0 이므로 \"무조건 미검증\" 이 아니다",
         sm_so_off["child_unmeasured"] == [("SC-42", "demo_replay_worker")]
         and sm_so_off["child_unobserved"] == []          # "자식이 안 돌았다 → FAIL" 로 두지 않는다
         and any("show-output" in w for w in unmeasured(sm_so_off))
         and nosh_ch["child_measurable"] is False and nosh_ch["child_runs"] == 0
         and exit_code(nosh_sm) == 5
         # 그리고 플래그가 있는 판은 미검증이 아니다 — 0 == 0 통과가 아님을 양쪽으로 못박는다
         and sm["child_unmeasured"] == [] and sm["unmeasured"] == []),
        # 리더가 요구한 네 갈래 — **입력을 바꿔** 결과가 갈리는지 본다(두 번째가 없으면 첫째가 "무조건 미검증")
        ("갈래 ①: 자식 실행 2 · 두 pid 다름 · 소스 요구 2 → **PASS**(exit 0)",
         pass_ch["child_runs"] == 2 and pass_ch["distinct_child_pids"] == 2
         and pass_ch["expected_child_runs"] == 2 and exit_code(pass_sm) == 0
         and pass_sm["unmeasured"] == [] and pass_sm["undecidable"] == []),
        ("갈래 ②: 자식 실행 0(블록은 있다) → **미검증(exit 5)**. PASS 가 아니다",
         noobs_ch["child_measurable"] is True and noobs_ch["child_runs"] == 0
         and noobs_sm["child_unobserved"] == [("SC-50", "noobs_worker")]
         and exit_code(noobs_sm) == 5),
        ("갈래 ③: 자식 실행 2 · **두 pid 같음** → PASS 아님(exit 1). 한 프로세스를 두 번 적었다",
         coll_ch["child_runs"] == 2 and coll_ch["distinct_child_pids"] == 1
         and coll_sm["child_pid_collisions"] == [("SC-50", "collide_worker", ["77", "77"])]
         and exit_code(coll_sm) == 1),
        ("갈래 ④: 자식 실행 1 · 소스 요구 2 → PASS 아님(exit 1). 기대값은 도구가 아니라 소스가 든다",
         one_ch["child_runs"] == 1 and one_ch["expected_child_runs"] == 2
         and one_sm["child_runs_mismatch"] == [("SC-50", "onerun_worker", 1, 2)]
         and exit_code(one_sm) == 1),
        ("자식 stdout 을 그대로 흘린 `test X ... ok` 형태도 센다(pid 없음 → 충돌 아님)",
         verb_ch["child_runs"] == 2 and verb_ch["child_pids"] == []
         and verb_sm["child_pid_collisions"] == [] and exit_code(verb_sm) == 0),
        ("D-2 부수 발견: `-q` 로그(점 진행, `test … ok` 0 줄)는 **유령이 아니라 미검증** — "
         "유령으로 인쇄하면 다음 사람이 집계를 느슨하게 고친다",
         all(r["log_form_unreadable"] for r in quiet_res if r["mode"] == "run")
         and not any(r["ghost"] for r in quiet_res)
         and quiet_sm["ghosts"] == [] and quiet_sm["log_form_unreadable"]
         and exit_code(quiet_sm) == 5),
        # 리더 지시: `--workspace` 전달은 실측으로 닫혔다 — **그 걱정을 단언으로 남긴다**
        ("부모 센서스 분모 3(계약이 지명하든 않든) · 세 부모 각각 **정확히** 소스 요구 회차 → 통과",
         pcen_ok["denominator"] == 3 and pcen_ok["partial"] == [] and pcen_ok["pid_bad"] == []
         and [r["runs"] for r in pcen_ok["rows"]] == [2, 2, 2]
         and [r["expected"] for r in pcen_ok["rows"]] == [2, 2, 2]),
        ("부모 센서스 음성 대조 ①: 한 부모만 찍힌 로그는 **일부만 찍힘 → 미검증**(PASS 가 아니다). "
         "분모 3 은 줄지 않는다",
         pcen_part["denominator"] == 3 and sorted(pcen_part["partial"]) == ["b_worker", "c_worker"]
         and exit_code(summarize(pc_ext, None, None, pcen_part)) == 5),
        ("부모 센서스 음성 대조 ②: 회차가 **1 이나 3** 이면 통과하지 않는다 — 전달이 부분이면 걸린다",
         "a_worker" in pcen_one["partial"] and pcen_one["rows"][0]["runs"] == 1
         and "a_worker" in pcen_three["partial"] and pcen_three["rows"][0]["runs"] == 3
         # 같은 자식이 **정확히 2** 일 때는 partial 에 없다 — "무조건 partial" 이 아님을 못박는다
         and "a_worker" not in pcen_ok["partial"]),
        ("부모 센서스 음성 대조 ③: 같은 pid 두 줄은 `pid_bad` → exit 1(미검증이 아니다)",
         pcen_samepid["pid_bad"] and pcen_samepid["partial"] == []
         and exit_code(summarize(pc_ext, None, None, pcen_samepid)) == 1),
        ("항등식 셋이 **값째로** 요약에 올라온다(리포트가 읽는 자리는 summary.json 이다)",
         len(sm["identities"]) == 3 and all(x["holds"] for x in sm["identities"])
         and all(x["lhs"] and x["name"] for x in sm["identities"])),
        ("D-1: MANUAL 은 `--ignored` 로 돌려 ran ≥ 1",
         by[("SC-43", "demo_live_smoke")]["mode"] == "ignored"
         and "--ignored" in by[("SC-43", "demo_live_smoke")]["argv"]
         and by[("SC-43", "demo_live_smoke")]["ran"] >= 1),
        ("D-1: 분모가 줄지 않는다 — executed == Rust 필터, 항등식 ①②③ 성립, 판정 보류 없음",
         sm["executed"] == sm["filters_rust"] and sm["ran_ge_1"] == sm["filters_rust"]
         and all(ok for _, _, ok in identities(sm)) and undecidable(sm) == []),
        ("D-1: 항등식 ③ 은 레포 전체 `#[ignore]` 를 센다(CHILD 1 + MANUAL 1 + 미분류 0)",
         sm["ignore_census"] == {"raw_ignore_attrs": 2, "attached": 2, "child": ["demo_replay_worker"],
                                 "manual": ["demo_live_smoke"], "unclassified": 0}),
        ("D-1 추가 만기 2: 산문 RUNNABLE 분모를 센다 — Rust 맨 이름 3 중 RUNNABLE 1(CHILD 1·MANUAL 1 제외), "
         "Unity 1",
         sm["filters_rust_bare"] == 3 and sm["prose_runnable"] == {"rust": 1, "unity_bare": 1}),
        ("D-1 음성 대조: 역할 판정을 끄면 CHILD·MANUAL 이 **유령으로 되돌아온다**(끈 판이 초록이면 검출기가 죽었다)",
         sm_role_off["roles"]["CHILD"] == 0
         and {g[0] for g in sm_role_off["ghosts"]} >= {"SC-42", "SC-43"}
         and len(sm_role_off["ghosts"]) > len(sm["ghosts"])
         and not sm["ghosts"]),
        ("유령 1(정적) → 종료 코드 1", exit_code(sm) == 1),
        ("음성 대조: 입력이 0 이면(§1 없음) 행 0 — 0 = 0 을 통과로 읽지 않게 rows 를 찍는다",
         extract("## 2. x\n| SC-01 | a | `cargo test -p a b` | c |\n", idx, Fixes(), tmp)["rows"] == 0),
    ]
    # 실행 0 유령: 존재하지 않는 이름을 cargo 필터로 지명
    g = run_filters([Filter("SC-77", "cargo", "", "rust", "starfall-sim", "", [], "mining::no_such_test", [])],
                    tmp / "out_g", cargo=cargo, env=dict(os.environ), root=tmp, progress=lambda *_: None)
    cases.append(("실행 0 인 cargo 필터 = 유령(rc 0 이어도)", g[0]["ghost"] and g[0]["rc"] == 0))

    # exit 4 ①: CHILD 인데 부모가 **같은 SC 에 지명되지 않았다** → 자식 실행 수를 읽을 자리가 없다
    orphan = extract("## 1. x\n| SC-44 | x | `demo_replay_worker` | y |\n", idx, Fixes(), tmp)
    o_res = run_filters(orphan["filters"], tmp / "out_orphan", cargo=cargo, env=dict(os.environ), root=tmp,
                        progress=lambda *_: None, idx=idx)
    o_sm = summarize(orphan, o_res)
    cases.append(("exit 4 ①: 부모가 같은 SC 에 없는 CHILD 는 **판정하지 않는다**(통과도 실패도 아니다)",
                  o_sm["child_parent_missing"] == [("SC-44", "demo_replay_worker")] and exit_code(o_sm) == 4))
    # 같은 자식인데 부모 로그가 없으면 0 이다 — 2 를 도구에 박으면 여기서 깨진다(기대값 하드코딩 금지)
    cases.append(("D-1 ⑥: 자식 실행 수는 **세서** 나온다 — 부모 로그가 없으면 2 가 아니라 0",
                  o_res[0]["child_runs"] == 0 and o_res[0]["parent_filters"] == []
                  and by[("SC-42", "demo_replay_worker")]["child_runs"] == 2))

    # exit 4 ②: 자기 무력화 방어 — `#[ignore` 를 어느 테스트 함수에도 붙이지 못하면 "위반 0" 이 아니라 멈춘다
    tmp2 = Path(tempfile.mkdtemp(prefix="cargo_sc_map_selftest_u_"))
    (tmp2 / "server" / "crates" / "x" / "tests").mkdir(parents=True)
    (tmp2 / "server" / "Cargo.toml").write_text('[workspace]\nmembers = ["crates/*"]\n', encoding="utf-8")
    (tmp2 / "server" / "crates" / "x" / "Cargo.toml").write_text('[package]\nname = "starfall-x"\n', encoding="utf-8")
    (tmp2 / "server" / "crates" / "x" / "tests" / "t.rs").write_text(
        '#[test]\n#[ignore = "x"]\n#[allow(dead_code)]\npub(crate) fn weird_form() {}\n', encoding="utf-8")
    idx2 = build_index(tmp2)
    u_sm = summarize(extract("## 1. x\n| SC-45 | x | `cargo test -p starfall-x weird_form` | y |\n", idx2,
                             Fixes(), tmp2), None)
    cases.append(("exit 4 ②: 정규식이 못 읽는 `#[ignore]` 형태는 미분류 ≥ 1 → 판정 보류(R28 항등식 ① 과 같은 장치)",
                  u_sm["ignore_census"]["unclassified"] == 1 and exit_code(u_sm) == 4
                  and any("미분류" in w for w in undecidable(u_sm))))
    shutil.rmtree(tmp2, ignore_errors=True)
    shutil.rmtree(tmp, ignore_errors=True)
    bad = 0
    for name, ok in cases:
        print(("OK   " if ok else "FAIL ") + name)
        bad += 0 if ok else 1
    print(f"selftest: {'PASS' if not bad else f'FAIL ({bad})'}  케이스={len(cases)}")
    return 0 if not bad else 1


# ---------------------------------------------------------------- main

def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("mode", nargs="?", choices=["scan", "run", "map", "selftest"])
    ap.add_argument("--contract")
    ap.add_argument("--out", help="run/scan: filters.json·filters_summary.tsv·logs/ 를 둘 디렉터리")
    ap.add_argument("--log", help="map: cargo test 로그 한 벌")
    ap.add_argument("--test-log", help="run: CHILD 자식 입증을 읽을 **표준 게이트 로그**"
                                       "(`cargo test --workspace --locked -- --show-output`). 주 우물이다")
    ap.add_argument("--json", help="map: 결과 JSON")
    ap.add_argument("--unity-xml", action="append", default=[], help="Unity 맨 이름을 찾을 결과 xml(여러 번)")
    ap.add_argument("--timeout", type=int, default=1800, help="필터 하나의 cargo 제한 시간(초)")
    a = ap.parse_args()
    mode = a.mode or ("map" if a.log else None)
    if mode == "selftest":
        return selftest()
    if not mode or not a.contract:
        ap.error("scan|run|map 과 --contract 가 필요하다")
    text = Path(a.contract).read_text(encoding="utf-8")
    idx = build_index()
    ext = extract(text, idx)
    if mode == "map":
        if not a.log:
            ap.error("map 은 --log 가 필요하다")
        r = map_log(ext, Path(a.log).read_text(encoding="utf-8", errors="replace"))
        if a.json:
            Path(a.json).write_text(json.dumps(r, ensure_ascii=False, indent=2), encoding="utf-8")
        print(f"로그 테스트 줄 {r['log_test_lines']} · SC {len(r['rows'])} · 매칭 0 필터 {len(r['zero_match_filters'])}")
        for k, f in r["zero_match_filters"]:
            print(f"  매칭 0: {k} {f}")
        print_summary(summarize(ext, None))
        return 0
    unity = None
    if mode == "scan":
        if a.unity_xml:
            unity = unity_bare_check(ext["filters"], [Path(x) for x in a.unity_xml])
        s = summarize(ext, None, unity)
        if a.out:
            write_outputs(Path(a.out), ext, None, s, unity)
        print_summary(s)
        for f in ext["filters"]:
            print(f"  {f.sc}\t{f.kind}\t{f.crate}\t{' '.join(f.cargo_flags) or '.'}\t{f.name or '(전체)'}\t{f.source}")
        return exit_code(s)
    # run
    if not a.out:
        ap.error("run 은 --out 이 필요하다")
    if not shutil.which("cargo", path=cargo_env()["PATH"]):
        print("미검증(환경): cargo 가 없다", file=sys.stderr)
        return 2
    out = Path(a.out)
    tl = Path(a.test_log) if a.test_log else None
    results = run_filters(ext["filters"], out, timeout=a.timeout, idx=idx, test_log=tl)
    if a.unity_xml:
        unity = unity_bare_check(ext["filters"], [Path(x) for x in a.unity_xml])
    s = summarize(ext, results, unity, parent_census(idx, tl))
    write_outputs(out, ext, results, s, unity)
    print_summary(s)
    return exit_code(s)


if __name__ == "__main__":
    sys.exit(main())
