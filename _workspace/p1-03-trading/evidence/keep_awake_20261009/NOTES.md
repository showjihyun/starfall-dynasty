# keep_awake 실측 (2026-10-09, qa3)

사용자 결정: 절전 설정은 바꾸지 않고 실행기가 도는 동안만 깨어 있게 한다.

- `powercfg /requests` 는 이 세션에서 **관리자 권한이 필요하다**고 거부했다("이 명령에는 관리자 권한이 필요하며 ..."). 권한 상승은 시도하지 않았다.
- 대신 `keep_awake.py state`(`CallNtPowerInformation(SystemExecutionState)`, 관리자 불필요)로 시스템 전체 요청을 쟀다. `state_probe.txt`:
  - before 0x00000000 → serve READY 뒤 0x00000001(SYSTEM_REQUIRED) → +3 s 0x00000001 → shutdown 뒤 0x00000000
  - DISPLAY_REQUIRED 는 내내 False
  - 기준선이 0 이라 다른 앱의 요청이 섞이지 않았다(대조 조건 성립)
- serve: 새 월드 `01a11e44-7200-71a8-bbef-46ce1c5bb7c8`(`new_world.txt`), stop 파일 → stdin shutdown, `SHUTDOWN exit=0`. stderr 에 `[keep_awake] 절전 방지 시작/해제` 각 1줄.
- `run_round.py --dry-run` 에서도 시작/해제 줄이 나왔다.
- selftest 13/13(`selftest.txt`). 비 Windows no-op, 플래그 값(0x80000001, 디스플레이 비트 없음, 해제 0x80000000), 예외 시 finally 해제와 finally 없는 구현 음성 대조, powercfg·subprocess 미사용, 적용 위치 두 곳, 실제 API 반환값.
- 사람이 관리자 프롬프트에서 확인하려면: `python tests/e2e/keep_awake.py hold --seconds 60` 을 띄운 채 `powercfg /requests` 의 SYSTEM 항목.
