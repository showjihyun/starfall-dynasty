"""실행 중에만 PC 를 깨워 둔다 — 사용자 결정(2026-10-09): 절전 설정은 바꾸지 않는다.

p1-03 Phase 4 에서 호스트 절전이 sell-load·sell-cas-market 실행을 무효로 만들었다(phase4_live7).
이 모듈은 Windows `SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED)` 로
**이 스레드가 살아 있는 동안만** 시스템 절전을 막고, 끝나면 `ES_CONTINUOUS` 로 해제한다.

- 화면 유지(`ES_DISPLAY_REQUIRED`)는 걸지 않는다. 화면은 꺼져도 된다.
- 전원 설정은 건드리지 않는다. 해제를 못 해도 프로세스가 끝나면 OS 가 요청을 거둔다.
- Windows 가 아니면 아무것도 하지 않는다.
- 요청은 스레드 단위다. 같은 스레드에서 들어가고 나와야 한다(context manager 가 보장).

    with keep_awake("run_round r1"):
        ...

    python tests/e2e/keep_awake.py selftest
    python tests/e2e/keep_awake.py hold --seconds 30     # 다른 창에서 확인할 때
    python tests/e2e/keep_awake.py state                 # 시스템 전체 실행 상태(관리자 권한 불필요)

확인: 실행 중 `state` 가 SYSTEM_REQUIRED=True, 끝난 뒤 False 다(다른 앱의 요청이 없을 때).
`powercfg /requests` 는 SYSTEM 항목에 `[PROCESS] ...\\python.exe` 를 보여 주지만 관리자 권한이 필요하다.
"""

from __future__ import annotations

import argparse
import contextlib
import sys
import time
from typing import Callable, Iterator

ES_CONTINUOUS = 0x80000000
ES_SYSTEM_REQUIRED = 0x00000001
ES_DISPLAY_REQUIRED = 0x00000002  # 쓰지 않는다 — selftest 가 꺼져 있음을 단언한다

HOLD_FLAGS = ES_CONTINUOUS | ES_SYSTEM_REQUIRED
RELEASE_FLAGS = ES_CONTINUOUS


def _win_setter() -> Callable[[int], int]:
    import ctypes

    fn = ctypes.windll.kernel32.SetThreadExecutionState  # type: ignore[attr-defined]
    fn.argtypes = [ctypes.c_uint32]
    fn.restype = ctypes.c_uint32
    return fn


@contextlib.contextmanager
def keep_awake(reason: str = "", *, platform: str | None = None,
               setter: Callable[[int], int] | None = None, log=sys.stderr) -> Iterator[bool]:
    """블록 동안 시스템 절전을 막는다. 값은 요청이 실제로 걸렸는지(비 Windows·실패면 False)."""
    plat = sys.platform if platform is None else platform
    if plat != "win32":
        yield False
        return
    fn = setter if setter is not None else _win_setter()
    held = fn(HOLD_FLAGS) != 0
    if held:
        print(f"[keep_awake] 절전 방지 시작{(' — ' + reason) if reason else ''}", file=log, flush=True)
    else:
        print("[keep_awake] SetThreadExecutionState 실패 — 절전 방지 없이 계속한다", file=log, flush=True)
    try:
        yield held
    finally:
        if held:
            fn(RELEASE_FLAGS)
            print("[keep_awake] 절전 방지 해제", file=log, flush=True)


def system_state() -> int | None:
    """CallNtPowerInformation(SystemExecutionState) — 모든 프로세스의 요청을 합친 값. 비 Windows 면 None."""
    if sys.platform != "win32":
        return None
    import ctypes

    v = ctypes.c_ulong(0)
    st = ctypes.windll.powrprof.CallNtPowerInformation(  # type: ignore[attr-defined]
        16, None, 0, ctypes.byref(v), ctypes.sizeof(v))
    return v.value if st == 0 else None


# ================================================================ selftest

def selftest() -> int:
    import io
    from pathlib import Path

    cases: list[tuple[str, bool]] = []
    quiet = io.StringIO()

    calls: list[int] = []
    with keep_awake(platform="linux", setter=lambda f: calls.append(f) or 1, log=quiet) as held:
        pass
    cases.append(("비 Windows 는 no-op (setter 호출 0, held False)", calls == [] and held is False))

    cases.append(("HOLD = ES_CONTINUOUS|ES_SYSTEM_REQUIRED = 0x80000001", HOLD_FLAGS == 0x80000001))
    cases.append(("HOLD 에 ES_DISPLAY_REQUIRED 없음", HOLD_FLAGS & ES_DISPLAY_REQUIRED == 0))
    cases.append(("RELEASE = ES_CONTINUOUS = 0x80000000", RELEASE_FLAGS == 0x80000000))

    calls = []
    with keep_awake(platform="win32", setter=lambda f: calls.append(f) or 1, log=quiet) as held:
        inside = list(calls)
    cases.append(("Windows: 진입 때 HOLD 1회, 블록 안에서는 해제 전",
                  inside == [HOLD_FLAGS] and held is True))
    cases.append(("Windows: 나올 때 RELEASE 1회", calls == [HOLD_FLAGS, RELEASE_FLAGS]))

    calls = []
    raised = False
    try:
        with keep_awake(platform="win32", setter=lambda f: calls.append(f) or 1, log=quiet):
            raise RuntimeError("본문 예외")
    except RuntimeError:
        raised = True
    cases.append(("본문 예외에도 finally 에서 RELEASE (예외는 그대로 전파)",
                  raised and calls == [HOLD_FLAGS, RELEASE_FLAGS]))

    calls = []
    with keep_awake(platform="win32", setter=lambda f: calls.append(f) or 0, log=quiet) as held:
        pass
    cases.append(("setter 실패(0) → held False, 해제 호출 없음, 예외 없음",
                  held is False and calls == [HOLD_FLAGS]))

    # 음성 대조: 해제가 finally 밖이면 위 예외 케이스가 잡는가 — 같은 형태의 결함 구현에 건다
    @contextlib.contextmanager
    def broken(setter):
        setter(HOLD_FLAGS)
        yield True
        setter(RELEASE_FLAGS)  # finally 아님
    calls = []
    try:
        with broken(lambda f: calls.append(f) or 1):
            raise RuntimeError
    except RuntimeError:
        pass
    cases.append(("음성 대조: finally 없는 구현은 예외 시 RELEASE 누락으로 검출됨", calls == [HOLD_FLAGS]))

    src = Path(__file__).read_text(encoding="utf-8")
    code = src.split("# ================================================================ selftest")[0]
    code = code.split('"""', 2)[2]  # 모듈 docstring 제외
    cases.append(("전원 설정 변경 코드 없음(powercfg·subprocess 미사용)",
                  "powercfg" not in code and "subprocess" not in code))

    here = Path(__file__).parent
    for name, marker in (("run_round.py", "with keep_awake(f\"run_round"),
                         ("server_boot.py", "with keep_awake(\"server_boot serve\")")):
        cases.append((f"적용 위치: {name} 에 `{marker}` 가 있다", marker in (here / name).read_text(encoding="utf-8")))

    if sys.platform == "win32":
        real: list[int] = []
        fn = _win_setter()

        def spy(f: int) -> int:
            r = fn(f)
            real.append(r)
            return r
        with keep_awake(setter=spy, log=quiet) as held:
            pass
        cases.append(("실제 Windows API: HOLD·RELEASE 둘 다 0 이 아닌 반환", held and len(real) == 2 and all(real)))

    bad = 0
    for label, ok in cases:
        bad += 0 if ok else 1
        print(("ok   " if ok else "FAIL ") + label)
    print(f"selftest: {'PASS' if not bad else f'FAIL ({bad})'}  케이스={len(cases)}")
    return 0 if not bad else 1


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    sub.add_parser("selftest")
    h = sub.add_parser("hold", help="주어진 초 동안 절전 방지를 건다(state 로 확인할 때)")
    h.add_argument("--seconds", type=float, default=30.0)
    sub.add_parser("state", help="시스템 실행 상태를 한 줄로 찍는다")
    a = ap.parse_args()
    if a.cmd == "selftest":
        return selftest()
    if a.cmd == "state":
        v = system_state()
        if v is None:
            print("state=unavailable")
            return 2
        print(f"SystemExecutionState=0x{v:08x} SYSTEM_REQUIRED={bool(v & ES_SYSTEM_REQUIRED)} "
              f"DISPLAY_REQUIRED={bool(v & ES_DISPLAY_REQUIRED)}")
        return 0
    with keep_awake("hold") as held:
        print(f"held={held}", flush=True)
        time.sleep(a.seconds)
    return 0


if __name__ == "__main__":
    sys.exit(main())
