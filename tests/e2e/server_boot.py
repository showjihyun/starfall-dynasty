#!/usr/bin/env python
"""SC-05 / SC-06 — 서버 기동 경로를 밖에서 몬다.

두 가지 일만 한다.

* `stats` : 실제 `data/` 로 서버를 띄우고 `/debug/stats` 를 받아 적은 뒤 **stdin `shutdown`** 으로
  정상 종료시킨다(하드 킬 금지 — 계약 §0.1). 종료 코드와 왕복 로그를 그대로 남긴다.
* `reject`: `STARFALL_DATA_DIR` 로 **임시 사본**을 가리키고 한 파일만 어긴 뒤 **종료 코드 1** 과
  `기동 거부: 데이터 검증 실패 file=… field=… expected=… actual=… rule=…` 로그를 확인한다.
  **레포의 `data/` 원본은 절대 건드리지 않는다**(designer 소유).

왜 파이썬인가: 셸 인용 규칙 차이로 같은 명령이 다르게 깨지지 않게 하려는 것이고, 서버의 stdin 을
잡고 있어야 정상 종료를 시킬 수 있기 때문이다(`tests/e2e/README.md` 의 대기·셸 절과 같은 이유).

종료 코드: 0 = 기대대로, 1 = 기대와 다름, 2 = 서버 바이너리·DB 에 닿지 못함.
"""
from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import threading
import time
import urllib.error
import urllib.request
from pathlib import Path

for _stream in (sys.stdout, sys.stderr):
    try:
        _stream.reconfigure(encoding="utf-8")  # type: ignore[union-attr]
    except (AttributeError, ValueError):
        pass

REPO = Path(__file__).resolve().parents[2]
EXE = REPO / "server" / "target" / "debug" / "starfall-game-server.exe"


def load_dotenv() -> dict[str, str]:
    env = dict(os.environ)
    dotenv = REPO / ".env"
    if dotenv.is_file():
        for line in dotenv.read_text(encoding="utf-8").splitlines():
            line = line.strip()
            if not line or line.startswith("#") or "=" not in line:
                continue
            k, v = line.split("=", 1)
            env.setdefault(k.strip(), v.strip())
    env.setdefault("STARFALL_DEV_AUTH_SECRET", "dev_only_not_a_secret")
    return env


def http_json(url: str, timeout: float = 3.0):
    with urllib.request.urlopen(url, timeout=timeout) as r:
        return json.loads(r.read().decode("utf-8"))


def spawn(env: dict[str, str]) -> tuple[subprocess.Popen, list[str]]:
    """서버를 띄우고 **stdout 을 즉시 드레인하는 스레드**를 함께 건다.

    **왜 드레인 스레드가 필요한가 — 이 함수가 라운드 2 에서 서버를 세웠다.**
    `stdout=subprocess.PIPE` 로 띄워 놓고 프로세스가 끝난 뒤에야 `read()` 하면,
    실행 중에는 아무도 파이프를 비우지 않는다. Python `subprocess` 의 익명 파이프 용량은
    **4096 B** 이고, 서버의 `tracing` 기본 writer 는 **이벤트를 낸 스레드에서 동기로** 쓴다.
    그래서 로그가 4 KiB 를 넘는 순간 그 줄을 쓰려던 tokio 워커가 블록되고, 워커가 하나씩
    잠기다가 서버 전체가 무응답이 됐다(라운드 2 §3.1·§4.1 — 멈춘 두 인스턴스의 로그가
    각각 **4033 B / 4100 B** 였다).

    서버는 이제 비블로킹 싱크를 쓰므로 멈추지 않지만, **파이프가 차면 로그 줄을 버린다.**
    드레인하지 않으면 라운드 3 의 실서버 증거가 **4 KiB 에서 조용히 잘린다** — 그래서 여기서 비운다.

    반환하는 리스트는 **살아 있는 버퍼**다: 호출자는 프로세스가 끝난 뒤 `"".join(buf)` 로 전문을 얻는다.
    """
    proc = subprocess.Popen(
        [str(EXE)], cwd=str(REPO), env=env,
        stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
        text=True, encoding="utf-8", errors="replace", bufsize=1,
    )
    buf: list[str] = []

    def _drain() -> None:
        # readline 루프가 EOF 까지 돈다. 데몬이라 프로세스가 죽으면 같이 끝난다.
        try:
            for line in iter(proc.stdout.readline, ""):  # type: ignore[union-attr]
                buf.append(line)
        except (ValueError, OSError):
            pass  # 파이프가 닫히는 중이면 조용히 끝낸다

    threading.Thread(target=_drain, daemon=True, name="server-stdout-drain").start()
    return proc, buf


def _drain_settle(proc: subprocess.Popen, wait_s: float = 2.0) -> None:
    """드레인 스레드가 마지막 줄까지 읽을 시간을 유계로 준다.

    무한 대기하지 않는다 — 그러면 고치려던 결함(소비자가 생산자를 세운다)으로 되돌아간다.
    """
    deadline = time.monotonic() + wait_s
    while proc.poll() is None and time.monotonic() < deadline:
        time.sleep(0.05)
    time.sleep(0.2)


def graceful_shutdown(proc: subprocess.Popen, wait_s: float = 20.0) -> int:
    """stdin 에 `shutdown` 한 줄. 하드 킬은 최후의 수단이고 그 사실을 알린다."""
    try:
        assert proc.stdin is not None
        proc.stdin.write("shutdown\n")
        proc.stdin.flush()
    except (BrokenPipeError, OSError):
        pass
    try:
        proc.wait(timeout=wait_s)
    except subprocess.TimeoutExpired:
        print("!! 정상 종료가 시간 안에 끝나지 않아 terminate 했다 — 이 사실을 리포트에 적어야 한다",
              file=sys.stderr)
        proc.terminate()
        proc.wait(timeout=10)
    return proc.returncode


def cmd_stats(args) -> int:
    env = load_dotenv()
    addr = env.get("STARFALL_HTTP_ADDR", "127.0.0.1:8080")
    base = f"http://{addr}"
    if not EXE.is_file():
        print(f"미검증(환경): {EXE} 가 없다 — cargo build -p starfall-game-server", file=sys.stderr)
        return 2

    proc, log_buf = spawn(env)
    stats, boot_log = None, []
    deadline = time.monotonic() + args.timeout
    try:
        while time.monotonic() < deadline:
            if proc.poll() is not None:
                break
            try:
                stats = http_json(f"{base}/debug/stats")
                break
            except (urllib.error.URLError, OSError, TimeoutError):
                time.sleep(0.25)
        if stats is None:
            print("FAIL: /debug/stats 에 닿지 못했다", file=sys.stderr)
            return 1
        rc = graceful_shutdown(proc)
    finally:
        if proc.poll() is None:
            proc.kill()
    # 드레인 스레드가 이미 읽었다 — 여기서 read() 하면 빈 문자열이다.
    _drain_settle(proc)
    out = "".join(log_buf)
    boot_log = out.splitlines()

    print("### /debug/stats (서버 기동 직후, 접속 0건)")
    print(json.dumps(stats, indent=2, ensure_ascii=False))
    print(f"\n### stdin `shutdown` 후 종료 코드 = {rc}")
    print("\n### 서버 로그 (전문)")
    for line in boot_log:
        print(line)
    if args.evidence:
        Path(args.evidence).write_text(
            json.dumps({"stats": stats, "shutdown_exit_code": rc, "log": boot_log},
                       indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    return 0 if rc == 0 else 1


def mutate(doc, key: str, value):
    """중첩 무관하게 첫 번째 `key` 를 `value` 로 바꾼다. 못 찾으면 False."""
    if isinstance(doc, dict):
        for k in list(doc):
            if k == key:
                doc[k] = value
                return True
            if mutate(doc[k], key, value):
                return True
    elif isinstance(doc, list):
        for item in doc:
            if mutate(item, key, value):
                return True
    return False


def find_parent_with(doc, key: str):
    if isinstance(doc, dict):
        if key in doc:
            return doc
        for v in doc.values():
            found = find_parent_with(v, key)
            if found is not None:
                return found
    elif isinstance(doc, list):
        for v in doc:
            found = find_parent_with(v, key)
            if found is not None:
                return found
    return None


# (이름, 대상 파일 glob, 변형 함수, 기대 rule)
def _case_hard_radius(docs):
    return mutate(docs["system"], "hard_boundary_radius_m", 20000.1)


def _case_empty_points(docs):
    p = find_parent_with(docs["system"], "points_m")
    if p is None:
        return False
    p["points_m"] = []
    return True


def _case_point_count(docs):
    p = find_parent_with(docs["system"], "point_count")
    if p is None:
        return False
    p["point_count"] = p["point_count"] + 1
    return True


def _case_non_integer_ratio(docs):
    return mutate(docs["tuning"], "snapshot_hz", 7)


def _case_resume_above_linger(docs):
    p = find_parent_with(docs["system"], "reconnect_resume_window_seconds")
    if p is None:
        p = find_parent_with(docs["tuning"], "reconnect_resume_window_seconds")
    if p is None:
        return False
    p["reconnect_resume_window_seconds"] = p.get("linger_seconds", 30) + 5
    return True


def _case_spawn_outside(docs):
    p = find_parent_with(docs["system"], "points_m")
    if p is None or not p["points_m"]:
        return False
    first = p["points_m"][0]
    if isinstance(first, dict):
        for axis in ("x", "x_m"):
            if axis in first:
                first[axis] = 999999.0
                return True
        k = sorted(first)[0]
        first[k] = 999999.0
        return True
    if isinstance(first, list):
        first[0] = 999999.0
        return True
    return False


def _case_thrust_order(docs):
    p = find_parent_with(docs["ship"], "main_thrust_mps2")
    if p is None:
        return False
    p["main_thrust_mps2"] = 0.1
    return True


def _case_duplicate_class_id(docs):
    # 같은 id 를 가진 두 번째 함선 클래스 파일을 만든다 (호출부에서 처리).
    return "duplicate"


# (이름, 변형 함수, 기대 rule, 기대 field(derived 만), 거부 로그에 **반드시** 들어 있어야 하는 문자열들)
#
# **왜 `must_contain` 이 필요한가 (§7a).** "기동 거부 8건" 은 "서버가 늘 실패한다" 와 구별되지
# 않는다. 각 건이 **자기가 심은 위반 때문에** 거부됐는지를 단언해야 뜻이 생긴다. 그래서 각 건은
# 자기 사례에만 나타나는 값(심은 값·대상 파일)을 로그에서 찾는다. 라운드 19 이전의 이 함수는
# `exit==1 and 거부라인 존재` 만 봤고, `has_file`/`has_field`/`rule_matches` 를 **계산해 놓고
# verdict 에 쓰지 않았다** — 다른 이유로 여덟 번 죽어도 8/8 PASS 가 인쇄됐을 것이다.
CASES = [
    ("1-hard-radius-above-ceiling", _case_hard_radius, "schema", "play_area.hard_boundary_radius_m",
     ["cradle.json", "20000.1", "20000"]),
    ("2-spawn-points-empty", _case_empty_points, "schema", "spawn.points_m",
     ["cradle.json", "받음: 0"]),
    ("3-point-count-mismatch", _case_point_count, "derived", "spawn.point_count",
     ["cradle.json", "actual=13"]),
    ("4-tick-snapshot-ratio-not-integer", _case_non_integer_ratio, "derived", "snapshot.snapshot_hz",
     ["sync-tuning.json", "actual=7"]),
    ("5-resume-window-above-linger", _case_resume_above_linger, "derived",
     "presence.reconnect_resume_window_seconds", ["cradle.json", "actual=35"]),
    ("6-spawn-point-outside-hard-boundary", _case_spawn_outside, "derived", "spawn.points_m[0]",
     ["cradle.json", "999999"]),
    ("7-main-thrust-below-lateral-or-reverse", _case_thrust_order, "derived",
     "movement.main_thrust_mps2", ["scout-s01.json", "actual=0.1"]),
    ("8-duplicate-ship-class-id", _case_duplicate_class_id, "derived", "id",
     ["zz-duplicate.json", "actual=scout-s01"]),
]

CONTRACTS_DATA_RS = REPO / "server" / "crates" / "contracts" / "src" / "data.rs"


def validator_field_map(path=None):
    """`deserialize_with = "de_xxx"` 마다 그 아래 필드 이름을 모은다.

    **왜 이 표가 SC-06 의 판정에 들어가는가 (architect R25).**
    `rule=schema` 로그는 필드명이 아니라 `de_f64_range!` 의 `stringify!($fn_name)` —
    **검증자 함수 이름**을 찍는다. 검증자가 필드와 **1:1 이면** 그 이름이 필드를 특정하지만,
    **여럿이 공유하면 로그로 어느 필드인지 알 수 없다.** 계약 SC-06 은 *"로그에 파일·필드·
    기대값이 나온다"* 를 요구하므로 **다중도 > 1 이면 그 요구가 만족되지 않는다.**

    **로그 여덟 줄을 사람이 읽어 판정하면 우연히 1:1 인 사례(②)가 전체를 대표한다** —
    architect R24 가 정확히 그렇게 틀렸다. 그래서 눈 대신 이 표를 건다.
    **다중도가 전부 1 로 떨어지는 것이 구제의 종료 조건이다.**
    """
    path = path or CONTRACTS_DATA_RS
    lines = path.read_text(encoding="utf-8").splitlines()
    mapping: dict[str, list[str]] = {}
    for i, line in enumerate(lines):
        hit = re.search(r'deserialize_with\s*=\s*"(de_[a-z0-9_]+)"', line)
        if not hit:
            continue
        field = f"?:{i + 1}"
        for j in range(i + 1, min(i + 4, len(lines))):
            named = re.search(r"pub\s+([a-z_][a-z0-9_]*)\s*:", lines[j])
            if named:
                field = f"{named.group(1)} ({path.name}:{j + 1})"
                break
        mapping.setdefault(hit.group(1), []).append(field)
    return mapping


REJECT_MARK = "기동 거부: 데이터 검증 실패"


def judge_case(name, expected_rule, expected_field, must_contain, exit_code, log,
               validator_map=None):
    """한 건의 거부가 **그 이유로** 일어났는지 판정한다. 순수 함수 — `selftest` 가 합성 입력으로 건다."""
    reject_lines = [ln for ln in log.splitlines() if REJECT_MARK in ln]
    has_file = any("file=" in ln for ln in reject_lines)
    rule_matches = any("rule=" + expected_rule in ln for ln in reject_lines)
    # `field=` 토큰 검사는 **derived 층에만** 건다. schema 층은 토큰이 없고
    # 필드 지칭을 §A-1b 의 결과 검사로 본다(아래).
    field_matches = (
        expected_rule != "derived"
        or expected_field is None
        or any("field=" + expected_field + " " in ln
               or ln.rstrip().endswith("field=" + expected_field)
               for ln in reject_lines)
    )
    # derived 층은 계약이 요구하는 네 토큰을 전부 갖춰야 한다. schema 층은 serde 메시지가
    # `error=` 로 들어오고 `field=`/`expected=`/`actual=` 토큰이 **없다** — 계약이 모순되는
    # 자리라 여기서 임의로 정하지 않고 형식 관측만 따로 싫는다(리포트 §계약 침묵).
    tokens_required = ["file=", "field=", "expected=", "actual="] if expected_rule == "derived" else ["file="]
    tokens_present = {t: any(t in ln for ln in reject_lines)
                      for t in ["file=", "field=", "expected=", "actual=", "error="]}
    tokens_ok = all(tokens_present[t] for t in tokens_required)
    missing = [needle for needle in must_contain if not any(needle in ln for ln in reject_lines)]

    # `rule=schema` 층 — **계약이 요구하는 것은 결과다: 로그가 위반한 필드를 지칭하는가.**
    #
    # **왜 원인이 아니라 결과를 재는가 (리더 지적, S-R25 이후).** 처음에는 *"공유 검증자를
    # 쓰는가"* 를 판정량으로 삼았다. 공유는 **필드가 특정되지 않는 원인**이었지 그 자체가
    # 위반이 아니다. server 가 구제 ⓐ(`serde_path_to_error`)를 고르면 **공유는 그대로인데
    # JSON 경로가 앞에 붙어 필드가 특정된다** — 원인을 재는 검사는 그 로그를 **잘못된 이유로
    # FAIL 로 돌린다.** 그래서 층을 나눈다:
    #   1. 로그가 **사례가 심은 필드**를 지칭하는가       → 판정
    #   2. 지칭하지 않으면 **왜 못 하는지**(다중도)        → 설명
    #
    # **자명 통과 방지 둘.**
    # (가) *"필드명이 로그에 들어 있다"* 만 보면 **검증자 이름 자체가 필드명을 품은 경우**
    #      (`de_points_m` ⊃ `points_m`) 아무 일도 안 하고 통과한다. 그래서 `de_*` 토큰을
    #      **먼저 지우고** 필드명을 찾는다.
    # (나) *"어떤 필드명이든 있으면 통과"* 로 두면 **엉뚱한 필드가 찍혀도 통과한다.**
    #      `soft` 를 위반시켰는데 `hard` 가 찍히는 것은 `cradle.json:16,17` 한 줄 차이라
    #      **실제로 일어날 수 있고**, architect 가 "없는 것보다 나쁜 정보" 라고 한 그 상황이다.
    #      그래서 **형제 필드가 대신 찍혔는지**를 따로 단언한다.
    field_identifiable, validators_in_log, basis = None, [], None
    names_expected, names_sibling = None, []
    if expected_rule == "schema":
        vmap = validator_map if validator_map is not None else validator_field_map()
        stripped = []
        for line in reject_lines:
            for candidate in re.findall(r"\bde_[a-z0-9_]+\b", line):
                if candidate in vmap and candidate not in [v["validator"] for v in validators_in_log]:
                    validators_in_log.append({
                        "validator": candidate,
                        "field_count": len(vmap[candidate]),
                        "shared_by": vmap[candidate] if len(vmap[candidate]) > 1 else None,
                    })
            stripped.append(re.sub(r"\bde_[a-z0-9_]+\b", " ", line))   # (가)
        haystack = "\n".join(stripped)

        leaf = (expected_field or "").rsplit(".", 1)[-1]
        names_expected = bool(leaf) and leaf in haystack

        # (나) 같은 검증자를 쓰는 **다른** 필드가 대신 찍혔는가.
        for v in validators_in_log:
            for entry in vmap.get(v["validator"], []):
                other = entry.split(" ")[0]
                if other and other != leaf and other in haystack:
                    names_sibling.append(other)

        field_identifiable = names_expected and not names_sibling
        if names_sibling:
            basis = ("엉뚱한 필드를 지칭한다 — 심은 것은 " + (expected_field or "?")
                     + " 인데 로그는 " + ", ".join(sorted(set(names_sibling)))
                     + " 를 가리킨다. 없는 것보다 나쁜 정보다")
        elif names_expected:
            basis = "로그가 심은 필드를 지칭한다: " + (expected_field or "?")
        else:
            shared = [v["validator"] + "×" + str(v["field_count"])
                      for v in validators_in_log if v["field_count"] > 1]
            basis = ("로그가 필드를 지칭하지 않는다(심은 것은 " + (expected_field or "?") + ")"
                     + ("  — 원인: 공유 검증자 " + ", ".join(shared) if shared else ""))

    ok = (exit_code == 1 and bool(reject_lines) and rule_matches
          and field_matches and tokens_ok and not missing
          and field_identifiable is not False)
    return {
        "case": name,
        "expected_rule": expected_rule,
        "expected_field": expected_field,
        "exit_code": exit_code,
        "reject_log_lines": reject_lines[:1],
        "has_file": has_file,
        "rule_matches": rule_matches,
        "field_matches": field_matches,
        "log_format_tokens": tokens_present,
        "discriminators_missing": missing,
        "schema_field_identifiable": field_identifiable,
        "schema_field_identifiable_basis": basis if expected_rule == "schema" else None,
        "schema_names_injected_field": names_expected,
        "schema_names_sibling_field_instead": sorted(set(names_sibling)) or None,
        "validators_named_in_log": validators_in_log,
        "verdict": "PASS" if ok else "FAIL",
    }


def judge_negative_control(log, rc, timed_out):
    """음성 대조 로그를 판정한다. **순수 함수** — `selftest` 가 합성 로그로 건다."""
    lines = log.splitlines()
    data_idx = next((i for i, ln in enumerate(lines) if "게임 데이터 로딩 완료" in ln), None)
    later_idx = next((i for i, ln in enumerate(lines) if "오류로 종료됐다" in ln), None)
    rejects = [ln for ln in lines if REJECT_MARK in ln]
    # **이 대조가 얹혀 있는 전제를 같은 실행에서 확인한다** (architect R24 · F-4).
    # 수법 전체가 `main.rs` 의 **data → DB** 순서에 의존한다. 그 순서가 뒤집히면
    # (DB 를 먼저 열면) 데이터 로딩은 아예 시도되지 않은 채 죽고, 그런데도
    # "거부 0건" 은 참이라 **대조가 아무것도 증명하지 않으면서 계속 통과한다.**
    # 그래서 `게임 데이터 로딩 완료` 가 `오류로 종료됐다` 보다 **먼저** 나왔는지를 단언한다.
    order_ok = data_idx is not None and later_idx is not None and data_idx < later_idx
    return {
        "what": "멀쩡한 data/ 사본 — 데이터 검증을 통과하는가 (DATABASE_URL 을 죽은 포트로 돌려 포트를 잡지 않는다)",
        "data_load_succeeded": data_idx is not None,
        "data_load_line": lines[data_idx] if data_idx is not None else None,
        "data_validation_rejects": len(rejects),
        "exit_code": rc,
        "timed_out": timed_out,
        "later_stage_error": lines[later_idx] if later_idx is not None else None,
        "premise_data_before_db": {
            "asserted": order_ok,
            "why": "main.rs 의 data → DB 순서. 순서가 바뀌면 이 대조는 아무것도 증명하지 않으면서 통과한다",
            "data_load_line_index": data_idx,
            "later_stage_error_line_index": later_idx,
        },
        "verdict": "PASS" if (order_ok and not rejects) else "FAIL",
    }


def source_provenance():
    """**이 관측이 어느 소스에서 나왔는가** — 계약 §7b 규칙 9 의 이행 조항.

    `git rev-parse HEAD` 와 `git status --porcelain` 의 **공백 여부**를 함께 남긴다.
    조항이 요구하는 것은 *"공백이다"* 가 아니라 **"공백 여부를 함께 남긴다"** 이므로,
    더러운 트리는 **그대로 적는다** — 숨기면 반증 가능성이 죽는다.

    **이것이 증명은 아니다**(규칙 9 (나)): 오래된 타깃 캐시나 다른 머신에서 나온 바이너리일
    수 있다. 기록이 주는 것은 **반증 가능성**이다. 더 강한 형태가 필요하면 빌드 산출물 해시를
    함께 남긴다 — 그래서 `server_binary_sha256` 도 같이 싣는다.
    """
    import hashlib

    def git(*args):
        try:
            out = subprocess.run(["git", *args], cwd=str(REPO), capture_output=True,
                                 text=True, encoding="utf-8", errors="replace", timeout=30)
            return out.stdout.rstrip()
        except (OSError, subprocess.SubprocessError) as exc:
            return f"(git 실패: {exc})"

    porcelain = git("status", "--porcelain")
    digest = None
    if EXE.is_file():
        h = hashlib.sha256()
        h.update(EXE.read_bytes())
        digest = h.hexdigest()[:16]
    return {
        "rule": "계약 §7b 규칙 9 이행 — 바이너리로 판정하는 항목은 HEAD 와 트리 청결 여부를 함께 남긴다",
        "head": git("rev-parse", "HEAD"),
        "head_subject": git("log", "-1", "--format=%s"),
        "worktree_clean": porcelain == "",
        "status_porcelain": porcelain.splitlines(),
        "server_binary_sha256_16": digest,
        "caveat": "기록은 증명이 아니라 반증 가능성이다 — 이 바이너리가 이 트리에서 나왔다는 증명은 아니다(규칙 9 (나))",
    }


def negative_control(env0, data_copy, timeout):
    """**멀쩡한 `data/` 사본으로는 데이터 검증을 통과하는가.**

    통과 여부를 **포트를 잡지 않고** 보기 위해 `DATABASE_URL` 을 죽은 포트로 돌린다.
    서버는 데이터 로딩을 끝내고 **그 다음 단계**인 DB 에서 죽는다(`main.rs` 의 순서: data → DB).
    tick 도 전진하지 않는다 — world 를 열기 전에 죽기 때문이다.
    """
    env = dict(env0)
    env["STARFALL_DATA_DIR"] = str(data_copy)
    env["DATABASE_URL"] = "postgres://starfall:starfall_dev_only@127.0.0.1:15999/starfall"
    timed_out = False
    try:
        proc = subprocess.run([str(EXE)], cwd=str(REPO), env=env, input="",
                              capture_output=True, text=True, encoding="utf-8",
                              errors="replace", timeout=timeout)
        log = (proc.stdout or "") + (proc.stderr or "")
        rc = proc.returncode
    except subprocess.TimeoutExpired as exc:
        out = exc.stdout if isinstance(exc.stdout, str) else ""
        err = exc.stderr if isinstance(exc.stderr, str) else ""
        log, rc, timed_out = out + err, None, True
    return judge_negative_control(log, rc, timed_out)


def cmd_reject(args) -> int:
    env0 = load_dotenv()
    if not EXE.is_file():
        print("미검증(환경): " + str(EXE) + " 가 없다", file=sys.stderr)
        return 2
    work = Path(args.workdir).resolve()
    results = []
    vmap = validator_field_map()
    shared_validators = {k: v for k, v in vmap.items() if len(v) > 1}

    for name, fn, expected_rule, expected_field, must_contain in CASES:
        case_dir = work / name
        if case_dir.exists():
            shutil.rmtree(case_dir)
        shutil.copytree(REPO / "data", case_dir)

        ship_files = sorted((case_dir / "ships").glob("*.json"))
        system_files = sorted((case_dir / "world" / "systems").glob("*.json"))
        tuning_file = case_dir / "movement" / "sync-tuning.json"
        docs = {
            "ship": json.loads(ship_files[0].read_text(encoding="utf-8")),
            "system": json.loads(system_files[0].read_text(encoding="utf-8")),
            "tuning": json.loads(tuning_file.read_text(encoding="utf-8")),
        }
        outcome = fn(docs)
        if outcome == "duplicate":
            dup = json.loads(ship_files[0].read_text(encoding="utf-8"))
            (case_dir / "ships" / "zz-duplicate.json").write_text(
                json.dumps(dup, indent=2), encoding="utf-8")
        elif not outcome:
            results.append({"case": name, "verdict": "FAIL(변형 지점을 못 찾음)"})
            continue
        else:
            ship_files[0].write_text(json.dumps(docs["ship"], indent=2), encoding="utf-8")
            system_files[0].write_text(json.dumps(docs["system"], indent=2), encoding="utf-8")
            tuning_file.write_text(json.dumps(docs["tuning"], indent=2), encoding="utf-8")

        env = dict(env0)
        env["STARFALL_DATA_DIR"] = str(case_dir)
        proc = subprocess.run([str(EXE)], cwd=str(REPO), env=env, input="",
                              capture_output=True, text=True, encoding="utf-8",
                              errors="replace", timeout=args.timeout)
        log = (proc.stdout or "") + (proc.stderr or "")
        results.append(judge_case(name, expected_rule, expected_field, must_contain,
                                  proc.returncode, log, validator_map=vmap))

    clean_copy = work / "0-negative-control-unmodified"
    if clean_copy.exists():
        shutil.rmtree(clean_copy)
    shutil.copytree(REPO / "data", clean_copy)
    neg = negative_control(env0, clean_copy, args.timeout)

    total = len(results)
    passed = sum(1 for r in results if r["verdict"] == "PASS")
    all_cases_pass = passed == total == len(CASES)
    if neg["verdict"] != "PASS":
        verdict = "미검증(증거 요건: 음성 대조 실패 — 8건 실패가 '서버가 늘 실패한다' 와 구별되지 않는다)"
        code = 4
    elif all_cases_pass:
        verdict, code = "PASS", 0
    else:
        verdict, code = "FAIL", 1

    summary = {
        "item": "SC-06 / AC-2(b~g) — 기동 거부 8종",
        "server_binary": str(EXE),
        "server_binary_mtime": time.strftime("%Y-%m-%dT%H:%M:%S", time.localtime(EXE.stat().st_mtime)),
        "source_provenance": source_provenance(),
        "cases_run": total,
        "passed": passed,
        "validator_field_multiplicity": {
            "source": str(CONTRACTS_DATA_RS.relative_to(REPO)).replace("\\", "/"),
            "why": "진단용이다 — **종료 조건이 아니다.** 구제 ⓑ(공유 분리)면 이 수가 0 이 되지만, 구제 ⓐ(serde_path_to_error)면 공유는 남고 JSON 경로가 필드를 특정한다. 종료 조건은 schema 사례 전부가 `schema_names_injected_field=true` 가 되는 것이다",
            "validators_total": len(vmap),
            "shared_count": len(shared_validators),
            "shared": {k: v for k, v in sorted(shared_validators.items(), key=lambda kv: -len(kv[1]))},
        },
        "negative_control": neg,
        "verdict": verdict,
        "results": results,
    }
    text = json.dumps(summary, indent=2, ensure_ascii=False)
    print(text)
    if args.evidence:
        Path(args.evidence).write_text(text + "\n", encoding="utf-8")
    return code


def cmd_rejudge(args) -> int:
    """이미 남긴 거부 로그를 **새 판정 기준으로 다시 판정한다** — 서버를 다시 돌리지 않는다.

    **왜 필요한가.** architect R25 가 `rule=schema` 층의 기준을 고쳤다(검증자가 필드와 1:1 이어야
    한다). 관측은 그대로이고 **기준만 바뀌었으므로**, 같은 로그를 새 기준으로 다시 재는 것이
    맞는 처분이다 — 바이너리를 다시 돌리면 다른 관측이 섞인다.

    **주의**: 입력 증거의 `reject_log_lines` 는 잘린 목록이다(`[:1]`). 판정에 쓰이는 첫 줄은
    보존돼 있으므로 이 재판정은 유효하지만, **원본 실행의 바이너리 시점 한계는 그대로 이어받는다.**
    """
    prior = json.loads(Path(args.evidence_in).read_text(encoding="utf-8"))
    by_name = {r.get("case"): r for r in prior.get("results", [])}
    vmap = validator_field_map()
    results, changed = [], []
    for name, _fn, expected_rule, expected_field, must_contain in CASES:
        row = by_name.get(name)
        if row is None:
            results.append({"case": name, "verdict": "미검증(증거 없음)"})
            continue
        log = "\n".join(row.get("reject_log_lines") or [])
        fresh = judge_case(name, expected_rule, expected_field, must_contain,
                           row.get("exit_code"), log, validator_map=vmap)
        fresh["verdict_before"] = row.get("verdict")
        if fresh["verdict"] != row.get("verdict"):
            changed.append({"case": name, "was": row.get("verdict"), "now": fresh["verdict"]})
        results.append(fresh)

    passed = sum(1 for r in results if r["verdict"] == "PASS")
    summary = {
        "item": "SC-06 재판정 — architect R25 의 기준(검증자 1:1)으로 같은 로그를 다시 잰다",
        "source_evidence": args.evidence_in,
        "source_binary_mtime": prior.get("server_binary_mtime"),
        "limitation": "관측은 원본 실행의 것이다 — 바이너리 시점 한계(§A-4 근거 ②)를 그대로 이어받는다",
        "cases_run": len(results),
        "passed": passed,
        "verdict_changes": changed,
        "results": results,
    }
    text = json.dumps(summary, indent=2, ensure_ascii=False)
    print(text)
    if args.evidence:
        Path(args.evidence).write_text(text + "\n", encoding="utf-8")
    return 0 if passed == len(CASES) else 1


def cmd_selftest(args) -> int:
    """`judge_case` 를 **합성 로그**로 건다 — 양성 1 · 음성 6."""
    good = ("2026-01-01T00:00:00Z ERROR starfall_game_server: 게임 데이터 로딩 실패 "
            "error=기동 거부: 데이터 검증 실패 file=C:/x/world/systems/cradle.json "
            "field=spawn.point_count expected=points_m.len() (12) 과 같다 actual=13 rule=derived")
    spec = ("3-point-count-mismatch", "derived", "spawn.point_count", ["cradle.json", "actual=13"])
    cases = [
        ("양성: 기대한 그대로", spec, 1, good, "PASS"),
        ("음성: 종료 코드가 0", spec, 0, good, "FAIL"),
        ("음성: 거부 로그가 없다(다른 이유로 죽음)", spec, 1,
         "ERROR 서버가 오류로 종료됐다 error=마이그레이션 적용 실패", "FAIL"),
        ("음성: 다른 필드로 거부됐다", spec, 1,
         good.replace("spawn.point_count", "spawn.clearance_m"), "FAIL"),
        ("음성: rule 이 다르다", spec, 1, good.replace("rule=derived", "rule=schema"), "FAIL"),
        ("음성: 심은 값이 로그에 없다", spec, 1, good.replace("actual=13", "actual=99"), "FAIL"),
        # 리더가 이름 댄 대조 — 계약 SC-06 은 "로그에 **파일**·필드·기대값이 나온다" 를 요구한다.
        ("음성: file= 토큰이 없다(파일명을 안 찍는다)", spec, 1,
         good.replace("file=C:/x/world/systems/cradle.json ", ""), "FAIL"),
        ("음성: file= 는 있는데 다른 파일이다", spec, 1,
         good.replace("cradle.json", "sync-tuning.json"), "FAIL"),
        ("음성: derived 인데 expected= 토큰이 없다", spec, 1,
         good.replace("expected=points_m.len() (12) 과 같다 ", ""), "FAIL"),
    ]
    failures = 0
    for label, (name, rule, field, needles), rc, log, want in cases:
        got = judge_case(name, rule, field, needles, rc, log)["verdict"]
        ok = got == want
        failures += 0 if ok else 1
        print(("ok   " if ok else "FAIL ") + label + ": 기대 " + want + " / 실제 " + got)

    # --- rule=schema: 로그가 **심은 필드를 지칭하는가** (architect R25 · 리더 S-R25 지적) ---
    # **합성 다중도 표**로 건다 — 실제 소스가 고쳐지면 기대값이 따라 흔들리지 않게.
    # rust 가 지금 그 소스를 고치고 있으므로 이 선택이 곧 값을 한다.
    vmap = {"de_points_m": ["points_m (data.rs:1)"],
            "de_boundary_radius_m": ["soft_boundary_radius_m (data.rs:446)",
                                     "hard_boundary_radius_m (data.rs:449)",
                                     "visual_radius_m (data.rs:566)"]}
    head = "ERROR error=기동 거부: 데이터 검증 실패 file=C:/x/world/systems/cradle.json error="
    tail = " rule=schema"
    HARD = "play_area.hard_boundary_radius_m"
    schema_cases = [
        # 구제 **전** 로그 — 검증자 이름만 찍는다. 필드가 없다.
        ("음성: 검증자 이름만 찍는다 — 필드 지칭 없음 (구제 전, 사례 ①)",
         HARD, head + "de_boundary_radius_m 는 (0 .. 20000 범위를 벗어난다 (받음: 20000.1)" + tail,
         "FAIL"),
        # 구제 **후**(ⓐ serde_path_to_error) — 공유는 그대로인데 JSON 경로가 필드를 특정한다.
        # **원인(공유)을 재던 옛 검사는 이것을 잘못된 이유로 FAIL 로 돌렸다.**
        ("양성: JSON 경로가 앞에 붙어 필드가 특정된다 (구제 ⓐ 후) — 공유는 그대로다",
         HARD, head + HARD + ": de_boundary_radius_m 는 (0 .. 20000 …) at line 17 column 37" + tail,
         "PASS"),
        # **엉뚱한 필드** — soft 가 찍혔는데 심은 것은 hard. 한 줄 차이라 실제로 일어난다.
        ("음성: 형제 필드가 대신 찍혔다 (soft ↔ hard, cradle.json:16,17 한 줄 차이)",
         HARD, head + "play_area.soft_boundary_radius_m: de_boundary_radius_m 는 (0 .. 20000 …)" + tail,
         "FAIL"),
        # 검증자 이름이 필드명을 **품은** 경우 — 지우지 않으면 아무 일도 안 하고 통과한다.
        ("음성: 검증자 이름이 필드명을 품었을 뿐이다 (de_points_m ⊃ points_m)",
         "spawn.points_m", head + "de_points_m 이 거부했다" + tail, "FAIL"),
        # 메시지가 필드명을 직접 적는 경우 — 사례 ② 의 구제 전 로그.
        ("양성: 메시지가 필드명을 직접 적는다 (사례 ② 의 구제 전 로그)",
         "spawn.points_m", head + "points_m 은 1 ..= 64 개여야 한다 (받음: 0)" + tail, "PASS"),
    ]
    for label, field, log, want in schema_cases:
        got = judge_case("x-schema", "schema", field, ["cradle.json"], 1, log,
                         validator_map=vmap)["verdict"]
        ok = got == want
        failures += 0 if ok else 1
        print(("ok   " if ok else "FAIL ") + label + ": 기대 " + want + " / 실제 " + got)

    # --- 음성 대조 자실을 건다 (architect R24 · F-4) ------------------------
    # 이 대조는 `main.rs` 의 **data → DB** 순서에 엹혀 있다. 순서가 바뀜면
    # 아무것도 증명하지 않으면서 **계속 통과한다** — 그것을 잡는지 보인다.
    dl = "INFO starfall_game_server: 게임 데이터 로딩 완료 ship_classes=1 spawn_points=12"
    db = "ERROR starfall_game_server: 서버가 오류로 종료됐다 error=마이그레이션 적용 실패"
    rej = ("ERROR error=기동 거부: 데이터 검증 실패 "
           "file=C:/x/movement/sync-tuning.json field=a expected=b actual=c rule=derived")
    neg_cases = [
        ("양성: data 로딩이 DB 오류보다 먼저다", dl + "\n" + db, "PASS"),
        ("음성: 순서가 뒤집혔다(DB 가 먼저 — 전제 붕괴)", db + "\n" + dl, "FAIL"),
        ("음성: data 로딩 완료 자체가 없다", db, "FAIL"),
        ("음성: DB 단계에 닿지도 못했다(로그가 한 줄)", dl, "FAIL"),
        ("음성: 순서는 맞지만 데이터 거부가 섞여 있다", dl + "\n" + rej + "\n" + db, "FAIL"),
    ]
    for label, log, want in neg_cases:
        got = judge_negative_control(log, 1, False)["verdict"]
        ok = got == want
        failures += 0 if ok else 1
        print(("ok   " if ok else "FAIL ") + label + ": 기대 " + want + " / 실제 " + got)

    total = len(cases) + len(schema_cases) + len(neg_cases)
    print("\n" + str(total - failures) + "/" + str(total) + " 케이스 통과"
          + " (judge_case " + str(len(cases)) + " · schema 필드 특정 "
          + str(len(schema_cases)) + " · judge_negative_control "
          + str(len(neg_cases)) + ")")
    return 0 if failures == 0 else 1


def cmd_serve(args) -> int:
    """서버를 띄우고 **중지 파일이 생길 때까지** 붙잡고 있는다 (블록 3~5 관측용).

    왜 필요한가: 관측 중에는 서버가 살아 있어야 하고, 끝낼 때는 **stdin `shutdown`** 이어야 한다
    (하드 킬 금지 — 계약 §0.1). 배경 프로세스로 띄워도 stdin 을 잡고 있는 주체가 있어야 그게 된다.
    """
    env = load_dotenv()
    addr = env.get("STARFALL_HTTP_ADDR", "127.0.0.1:8080")
    if not EXE.is_file():
        print(f"미검증(환경): {EXE} 가 없다", file=sys.stderr)
        return 2
    stop = Path(args.stop_file)
    if stop.exists():
        stop.unlink()
    log_path = Path(args.log) if args.log else None

    proc, log_buf = spawn(env)
    deadline = time.monotonic() + args.ready_timeout
    ready = False
    while time.monotonic() < deadline:
        if proc.poll() is not None:
            break
        try:
            http_json(f"http://{addr}/debug/stats")
            ready = True
            break
        except (urllib.error.URLError, OSError, TimeoutError):
            time.sleep(0.25)
    if not ready:
        if proc.poll() is None:
            proc.kill()
        print("FAIL: 서버가 준비되지 않았다", file=sys.stderr)
        return 1

    Path(args.ready_file).write_text(addr + "\n", encoding="utf-8")
    print(f"READY {addr} pid={proc.pid}", flush=True)

    limit = time.monotonic() + args.max_seconds
    while not stop.exists() and time.monotonic() < limit:
        if proc.poll() is not None:
            print("!! 서버가 스스로 종료됐다", file=sys.stderr)
            break
        time.sleep(0.25)

    rc = graceful_shutdown(proc) if proc.poll() is None else proc.returncode
    _drain_settle(proc)
    out = "".join(log_buf)
    if log_path:
        log_path.write_text(out, encoding="utf-8")
    print(f"SHUTDOWN exit={rc}")
    try:
        Path(args.ready_file).unlink()
    except OSError:
        pass
    return 0 if rc == 0 else 1


def main() -> int:
    ap = argparse.ArgumentParser(description="SC-05/SC-06 서버 기동 경로")
    sub = ap.add_subparsers(dest="cmd", required=True)
    s = sub.add_parser("stats", help="SC-05: 실제 data/ 로 띄우고 /debug/stats 를 받는다")
    s.add_argument("--timeout", type=float, default=30.0)
    s.add_argument("--evidence")
    s.set_defaults(func=cmd_stats)
    r = sub.add_parser("reject", help="SC-06: 기동 거부 8종")
    r.add_argument("--workdir", required=True, help="임시 data 사본을 둘 디렉토리")
    r.add_argument("--timeout", type=float, default=60.0)
    r.add_argument("--evidence")
    r.set_defaults(func=cmd_reject)
    rj = sub.add_parser("rejudge", help="이미 남긴 거부 로그를 새 기준으로 다시 판정한다")
    rj.add_argument("--evidence-in", required=True)
    rj.add_argument("--evidence")
    rj.set_defaults(func=cmd_rejudge)
    st = sub.add_parser("selftest", help="judge_case 를 합성 로그로 건다 (양성·음성 대조)")
    st.set_defaults(func=cmd_selftest)
    v = sub.add_parser("serve", help="서버를 띄우고 중지 파일이 생길 때까지 붙잡는다 (블록 3~5)")
    v.add_argument("--stop-file", required=True)
    v.add_argument("--ready-file", required=True)
    v.add_argument("--log")
    v.add_argument("--ready-timeout", type=float, default=40.0)
    v.add_argument("--max-seconds", type=float, default=900.0)
    v.set_defaults(func=cmd_serve)
    args = ap.parse_args()
    return args.func(args)


if __name__ == "__main__":
    sys.exit(main())
