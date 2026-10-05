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

종료 코드: 0 = 유령 0 · 실패 0 / 1 = 유령 ≥ 1 또는 실패 테스트 ≥ 1 / 2 = cargo 를 부를 수 없음(미검증(환경)).
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


_LINE_COMMENT = re.compile(r"//[^\n]*")
_BLOCK_COMMENT = re.compile(r"/\*.*?\*/", re.S)
_PY_COMMENT = re.compile(r"(?m)^\s*#[^\n]*")
RS_TEST = re.compile(r"#\[(?:[\w:]+::)?test[^\]]*\]\s*(?:#\[[^\]]*\]\s*)*(?:pub\s+)?(?:async\s+)?fn\s+(\w+)")
CS_TEST = re.compile(r"\[(?:Test|UnityTest|TestCase[^\]]*|TestCaseSource[^\]]*)\][^\n]*\n(?:\s*\[[^\]]*\][^\n]*\n)*"
                     r"\s*public\s+(?:static\s+)?(?:async\s+)?[\w<>\[\]]+\s+(\w+)")


def build_index(root: Path = REPO, roots: list[str] = INDEX_ROOTS) -> CodeIndex:
    test_fns: dict = {}
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
                elif f.endswith(".cs"):
                    for n in CS_TEST.findall(src):
                        test_fns.setdefault(n, []).append((rel, "unity"))
                    src = _LINE_COMMENT.sub("", _BLOCK_COMMENT.sub("", src))
                elif f.endswith(".py"):
                    src = _PY_COMMENT.sub("", src)
                chunks.append(src)
    return CodeIndex(test_fns, "\n".join(chunks))


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
    return {"rows": len(rows), "filters": uniq, "bare": bare,
            "static_ghosts": [b for b in bare if b["category"] == "GHOST"]}


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


def build_argv(f: Filter, cargo: list[str]) -> list[str]:
    argv = list(cargo) + ["test", "-p", f.crate]
    if "--locked" not in f.cargo_flags:
        argv.append("--locked")
    argv += f.cargo_flags + ["--"]
    if f.run_name:
        argv.append(f.run_name)
    argv += f.libtest_flags
    return argv


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
                root: Path = REPO, timeout: int = 1800, progress=print) -> list[dict]:
    cargo = cargo or ["cargo"]
    env = env if env is not None else cargo_env(root)
    logs = out / "logs"
    logs.mkdir(parents=True, exist_ok=True)
    results = []
    rust = [f for f in filters if f.kind == "rust"]
    for i, f in enumerate(rust, 1):
        argv = build_argv(f, cargo)
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
        results.append({**_fdict(f), "argv": argv[len(cargo):], "cwd": f.workspace or "server", "rc": rc,
                        "seconds": round(time.monotonic() - t0, 1), "log": str(log_path),
                        "ran": len(ran), "ok": sum(1 for _, s in ran if s == "ok"),
                        "failed": [n for n, s in ran if s == "FAILED"],
                        "ignored": [n for n, s in parsed["tests"] if s == "ignored"],
                        "names": [n for n, _ in ran], "ghost": len(ran) == 0})
        progress(f"[{i}/{len(rust)}] {f.sc} {f.crate} {' '.join(f.cargo_flags)} {f.name or '(전체)'} → ran={len(ran)} rc={rc}")
    attach_ownership(results)
    return results


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

def summarize(ext: dict, results: list[dict] | None, unity: list[dict] | None = None) -> dict:
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
    if results is not None:
        s.update({"executed": len(results),
                  "ran_ge_1": sum(1 for r in results if r["ran"] >= 1),
                  "ghosts": [(r["sc"], r["crate"], r["token"]) for r in results if r["ghost"]],
                  "failed_tests": sorted({(r["sc"], n) for r in results for n in r["failed"]}),
                  "nonzero_rc": [(r["sc"], r["token"], r["rc"]) for r in results if r["rc"] != 0],
                  "cross_sc": [(r["sc"], t, o) for r in results for t, o in r.get("also_named_by", {}).items()]})
    if unity is not None:
        s["unity_bare"] = {"checked": len(unity), "ghosts": [(u["sc"], u["name"]) for u in unity if u["ghost"]]}
    return s


def exit_code(summary: dict) -> int:
    bad = summary["static_ghosts"] or summary.get("ghosts") or summary.get("failed_tests") \
        or summary.get("nonzero_rc") or (summary.get("unity_bare") or {}).get("ghosts")
    return 1 if bad else 0


def write_outputs(out: Path, ext: dict, results, summary, unity=None) -> None:
    out.mkdir(parents=True, exist_ok=True)
    doc = {"summary": summary, "bare": ext["bare"],
           "filters": results if results is not None else [_fdict(f) for f in ext["filters"]]}
    if unity is not None:
        doc["unity_bare"] = unity
    (out / "filters.json").write_text(json.dumps(doc, ensure_ascii=False, indent=2), encoding="utf-8")
    if results is not None:
        lines = ["sc\tcrate\tflags\tfilter\tsource\trc\tran\tfailed\tscope\talso_named_by\tnames"]
        for r in results:
            lines.append("\t".join([r["sc"], r["crate"], " ".join(r["cargo_flags"]) or ".", r["name"] or ".",
                                    r["source"], str(r["rc"]), str(r["ran"]), str(len(r["failed"])), r["scope"],
                                    json.dumps(r["also_named_by"], ensure_ascii=False) if r["also_named_by"] else ".",
                                    " ".join(r["names"][:20])]))
        (out / "filters_summary.tsv").write_text("\n".join(lines) + "\n", encoding="utf-8", newline="\n")


def print_summary(s: dict) -> None:
    print(f"계약 §1 행 {s['rows']} · Rust 필터 {s['filters_rust']}(cargo {s['filters_rust_cargo']} + 맨 이름 "
          f"{s['filters_rust_bare']}) · Unity 맨 이름 {s['filters_unity_bare']} · 맨 토큰 {s['bare_tokens']} "
          f"{s['bare_by_category']}")
    for sc, t in s["static_ghosts"]:
        print(f"  유령(코드에 없음): {sc} {t}")
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
import sys
TESTS = {"starfall-sim": ["mining::reject_does_not_start_cooldown", "mining::each_rejection_reason_leaves_x",
                          "mining::each_rejection_reason_leaves_x_reasons_3_6_7"],
         "starfall-gateway": ["ws::tests::recording_lag_boundary_rejects", "mine_resource_counted"]}
INTEG = {("starfall-gateway", "ws_integration"): ["mine_resource_counted"]}
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
for t in sel:
    print("test %s ... ok" % t)
print("test result: ok. %d passed; 0 failed; 0 ignored; 0 measured; %d filtered out" % (len(sel), len(tests) - len(sel)))
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
                "## 2. 다른 절\r\n| SC-99 | x | `cargo test -p starfall-sim not_section_1` | y |\r\n")

    def names(ext, kind="rust"):
        return sorted((f.sc, f.name) for f in ext["filters"] if f.kind == kind)

    full = extract(contract, idx, Fixes(), tmp)
    off1 = extract(contract, idx, Fixes(bare=False), tmp)
    off2 = extract(contract, idx, Fixes(value_flags=False), tmp)
    off3 = extract(contract, idx, Fixes(cell_split=False), tmp)
    res = run_filters(full["filters"], tmp / "out", cargo=cargo, env=dict(os.environ), root=tmp, progress=lambda *_: None)
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
        ("유령 1(정적) → 종료 코드 1", exit_code(sm) == 1),
        ("음성 대조: 입력이 0 이면(§1 없음) 행 0 — 0 = 0 을 통과로 읽지 않게 rows 를 찍는다",
         extract("## 2. x\n| SC-01 | a | `cargo test -p a b` | c |\n", idx, Fixes(), tmp)["rows"] == 0),
    ]
    # 실행 0 유령: 존재하지 않는 이름을 cargo 필터로 지명
    g = run_filters([Filter("SC-77", "cargo", "", "rust", "starfall-sim", "", [], "mining::no_such_test", [])],
                    tmp / "out_g", cargo=cargo, env=dict(os.environ), root=tmp, progress=lambda *_: None)
    cases.append(("실행 0 인 cargo 필터 = 유령(rc 0 이어도)", g[0]["ghost"] and g[0]["rc"] == 0))
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
    ext = extract(text, build_index())
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
    results = run_filters(ext["filters"], out, timeout=a.timeout)
    if a.unity_xml:
        unity = unity_bare_check(ext["filters"], [Path(x) for x in a.unity_xml])
    s = summarize(ext, results, unity)
    write_outputs(out, ext, results, s, unity)
    print_summary(s)
    return exit_code(s)


if __name__ == "__main__":
    sys.exit(main())
