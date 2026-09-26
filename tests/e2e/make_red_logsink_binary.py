#!/usr/bin/env python
"""AC-2(i) ② RED 선행용 — **싱크를 끈 서버 바이너리**를 레포 밖에서 만든다.

왜 필요한가
-----------
AC-2(i) 는 "이 관측이 그 조건을 감지할 수 있는가"를 먼저 보여야 한다(architect R3 조건 ②).
수정 전 바이너리가 필요하지만, p1-01 작업은 전부 커밋되지 않은 작업 트리에 있어 되돌릴 커밋이 없다.
그래서 **현재 소스를 레포 밖으로 복사하고 한 줄만 바꿔** 빌드한다:

    let Some((writer, guard)) = logsink::non_blocking_stdout() else {
 →  let Some((writer, guard)) = None::<(logsink::NonBlockingStdout, logsink::FlushGuard)> else {

바뀐 쪽은 서버 자신의 **동기 stdout 폴백 경로**(`init_tracing` 의 `else` 분기)다 — 수정 전 동작과 같다.
**레포의 `server/` 는 건드리지 않는다**(qa 소유가 아니다). 복사본·빌드 산출물은 전부 `--out` 아래에 있다.

RED 와 GREEN 이 **이 한 줄 말고는 같은 소스**임을 보이기 위해, 복사 시점의 레포 `server/` 트리 해시와
복사본(패치 전) 트리 해시를 함께 찍는다. 두 값이 같아야 하고, GREEN 바이너리는 그 뒤에 소스가
바뀌지 않은 상태에서 빌드된 것이어야 한다(측정 직전에 `--hash-only` 로 다시 확인한다).

종료 코드: 0 = 준비 완료, 1 = 패치 지점이 정확히 1곳이 아님(서버 코드가 바뀌었다 — 이 스크립트를 고친다),
          2 = 빌드 실패·도구 없음.
"""
from __future__ import annotations

import argparse
import hashlib
import os
import shutil
import subprocess
import sys
from pathlib import Path

for _stream in (sys.stdout, sys.stderr):
    try:
        _stream.reconfigure(encoding="utf-8")  # type: ignore[union-attr]
    except (AttributeError, ValueError):
        pass

REPO = Path(__file__).resolve().parents[2]
MAIN_RS = Path("server/bins/game-server/src/main.rs")
NEEDLE = "let Some((writer, guard)) = logsink::non_blocking_stdout() else {"
REPLACEMENT = (
    "let Some((writer, guard)) = None::<(logsink::NonBlockingStdout, logsink::FlushGuard)> else {"
    " // QA-RED: 싱크를 끈 상태 (AC-2(i) ②)"
)
IGNORE = shutil.ignore_patterns("target", ".git", "__pycache__")


def tree_hash(root: Path) -> tuple[str, int]:
    """`.rs`·`Cargo.toml`·`Cargo.lock`·`.sql` 의 경로+내용 해시. 빌드 산출물은 보지 않는다."""
    h = hashlib.sha256()
    n = 0
    for p in sorted(root.rglob("*")):
        if "target" in p.relative_to(root).parts or not p.is_file():
            continue
        if p.suffix in {".rs", ".sql"} or p.name in {"Cargo.toml", "Cargo.lock"}:
            h.update(p.relative_to(root).as_posix().encode())
            h.update(p.read_bytes())
            n += 1
    return h.hexdigest()[:16], n


def main() -> int:
    ap = argparse.ArgumentParser(description="AC-2(i) RED: 싱크를 끈 서버 바이너리를 레포 밖에서 빌드")
    ap.add_argument("--out", required=True, help="복사본·타깃 디렉토리 (scratchpad 권장)")
    ap.add_argument("--no-build", action="store_true", help="복사·패치만")
    ap.add_argument("--hash-only", action="store_true", help="레포 server/ 트리 해시만 찍는다")
    args = ap.parse_args()

    repo_hash, repo_n = tree_hash(REPO / "server")
    print(f"repo server/ tree hash = {repo_hash} ({repo_n} files)")
    if args.hash_only:
        return 0

    out = Path(args.out).resolve()
    src = out / "src"
    if src.exists():
        shutil.rmtree(src)
    src.mkdir(parents=True)
    for d in ("server", "contracts", "data"):
        shutil.copytree(REPO / d, src / d, ignore=IGNORE)
    shutil.copy2(REPO / "rust-toolchain.toml", src / "rust-toolchain.toml")
    copy_hash, copy_n = tree_hash(src / "server")
    print(f"copy server/ tree hash = {copy_hash} ({copy_n} files) — 패치 전")
    if copy_hash != repo_hash:
        print("!! 복사 중에 레포 소스가 바뀌었다 — 다시 돌린다", file=sys.stderr)
        return 1

    main_rs = src / MAIN_RS
    text = main_rs.read_text(encoding="utf-8")
    hits = text.count(NEEDLE)
    if hits != 1:
        print(f"!! 패치 지점이 {hits}곳이다(1곳이어야 한다): {NEEDLE!r}", file=sys.stderr)
        return 1
    main_rs.write_text(text.replace(NEEDLE, REPLACEMENT), encoding="utf-8")
    print(f"patched {MAIN_RS}: 1곳")
    print(f"  - {NEEDLE}")
    print(f"  + {REPLACEMENT}")
    if args.no_build:
        return 0

    env = dict(os.environ)
    env["CARGO_TARGET_DIR"] = str(out / "target")
    cargo = shutil.which("cargo", path=env.get("PATH")) or str(Path.home() / ".cargo" / "bin" / "cargo.exe")
    r = subprocess.run([cargo, "build", "-p", "starfall-game-server"], cwd=str(src / "server"), env=env)
    if r.returncode != 0:
        print(f"!! 빌드 실패 rc={r.returncode}", file=sys.stderr)
        return 2
    exe = out / "target" / "debug" / "starfall-game-server.exe"
    digest = hashlib.sha256(exe.read_bytes()).hexdigest()[:16]
    print(f"RED exe = {exe}")
    print(f"RED exe sha256[:16] = {digest}")
    print(f"source tree hash (patch 전) = {copy_hash}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
