"""psql 호출 한 곳. 스프린트 계약 §0.1 이 고정한 형태를 코드로 옮긴 것이다.

    docker compose exec -T postgres psql -U starfall -d starfall -At -c "..."

왜 파이썬인가: Bash(Git Bash)와 PowerShell 두 셸의 인용 규칙 차이 때문에 같은 SQL 문자열이
셸마다 다르게 깨진다. 검증 스크립트가 셸에 따라 다르게 동작하면 "재현 가능한 명령"이 아니다.

종료 코드 규약 (계약 §0.3 / §5):
  0  정상
  1  검증 실패 (FAIL — 구현이 기대와 다르다)
  2  환경 문제 (미검증(환경) — docker/psql 자체가 안 된다)
  3  구현 없음 (FAIL — 테이블이 아직 없다. 환경 문제가 아니다)
"""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

EXIT_OK = 0
EXIT_FAIL = 1
EXIT_ENV = 2
EXIT_NOT_IMPLEMENTED = 3

REPO_ROOT = Path(__file__).resolve().parents[2]

# Windows 콘솔 기본 코드 페이지(cp949)에서 한글 메시지가 깨져 증거가 읽히지 않는다.
# 증거 파일은 UTF-8 로 쓰므로 표준 출력도 맞춘다.
for _stream in (sys.stdout, sys.stderr):
    try:
        _stream.reconfigure(encoding="utf-8")  # type: ignore[union-attr]
    except (AttributeError, ValueError):  # 파이프·리다이렉트 등
        pass


class EnvironmentProblem(RuntimeError):
    """docker/psql 이 동작하지 않는다 → 미검증(환경)."""


class NotImplementedYet(RuntimeError):
    """테이블·엔드포인트가 아직 없다 → FAIL(구현 없음). 환경 문제와 섞지 않는다."""


def psql(sql: str, *, timeout: int = 60) -> str:
    """한 줄 SQL 을 실행하고 stdout 을 돌려준다(-At: 헤더·정렬 없음)."""
    cmd = [
        "docker", "compose", "exec", "-T", "postgres",
        "psql", "-U", "starfall", "-d", "starfall", "-At", "-c", sql,
    ]
    try:
        proc = subprocess.run(
            cmd, cwd=REPO_ROOT, capture_output=True, text=True,
            timeout=timeout, encoding="utf-8", errors="replace",
        )
    except FileNotFoundError as exc:
        raise EnvironmentProblem(f"docker 를 찾을 수 없다: {exc}") from exc
    except subprocess.TimeoutExpired as exc:
        raise EnvironmentProblem(f"psql 이 {timeout}s 안에 끝나지 않았다") from exc

    if proc.returncode != 0:
        err = (proc.stderr or "").strip()
        low = err.lower()
        if "does not exist" in low and "relation" in low:
            raise NotImplementedYet(err)
        if "no such service" in low or "is not running" in low or "cannot connect" in low:
            raise EnvironmentProblem(err)
        # psql 이 의도적으로 낸 오류(append-only 트리거 등)는 호출자가 본문으로 판단한다.
        return err
    return (proc.stdout or "").strip()


def psql_rows(sql: str, *, sep: str = "|", timeout: int = 60) -> list[list[str]]:
    out = psql(sql, timeout=timeout)
    if not out:
        return []
    return [line.split(sep) for line in out.splitlines()]


def scalar_int(sql: str) -> int:
    out = psql(sql)
    try:
        return int(out.strip())
    except ValueError as exc:
        raise NotImplementedYet(f"정수를 기대했으나 받은 것: {out!r}") from exc


def table_exists(name: str) -> bool:
    out = psql(f"select to_regclass('public.{name}') is not null;")
    return out.strip() == "t"


def require_tables(*names: str) -> None:
    """없으면 FAIL(구현 없음)으로 끝낸다 — '환경이 없어서'와 섞지 않는다(계약 §5)."""
    missing = [n for n in names if not table_exists(n)]
    if missing:
        raise NotImplementedYet(
            f"테이블이 없다: {', '.join(missing)}. server T2(마이그레이션 0001)가 끝나지 않았다."
        )


def read_correlations(path: str | Path) -> list[str]:
    """봇이 수집한 correlation 집합 (계약 §0.6). Unity 의 것도 이 파일에 덧붙인다."""
    p = Path(path)
    if not p.is_file():
        raise EnvironmentProblem(f"correlation 파일이 없다: {p}")
    seen: list[str] = []
    dedup: set[str] = set()
    for line in p.read_text(encoding="utf-8").splitlines():
        v = line.strip()
        if not v or v.startswith("#"):
            continue
        if v not in dedup:
            dedup.add(v)
            seen.append(v)
    if not seen:
        raise EnvironmentProblem(f"correlation 집합이 비어 있다: {p} (0건 대조는 검증이 아니다)")
    return seen


def uuid_array_literal(values: list[str]) -> str:
    """`= any('{...}'::uuid[])` 에 넣을 리터럴. 값 검사를 먼저 해 SQL 주입 여지를 없앤다."""
    for v in values:
        if len(v) != 36 or any(c not in "0123456789abcdef-" for c in v.lower()):
            raise EnvironmentProblem(f"correlation 값이 UUID 모양이 아니다: {v!r}")
    return "{" + ",".join(values) + "}"


def emit(result: dict, evidence_path: str | Path | None = None) -> None:
    """사람이 읽을 요약 + 기계가 읽을 JSON. 증거는 파일로도 남긴다."""
    text = json.dumps(result, indent=2, ensure_ascii=False)
    print(text)
    if evidence_path:
        p = Path(evidence_path)
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(text + "\n", encoding="utf-8")


def main_guard(fn) -> int:
    """공통 예외 → 종료 코드 매핑."""
    try:
        return fn()
    except NotImplementedYet as exc:
        print(f"FAIL(구현 없음): {exc}", file=sys.stderr)
        return EXIT_NOT_IMPLEMENTED
    except EnvironmentProblem as exc:
        print(f"미검증(환경): {exc}", file=sys.stderr)
        return EXIT_ENV
