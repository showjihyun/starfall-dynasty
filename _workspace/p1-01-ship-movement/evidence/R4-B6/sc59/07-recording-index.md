# SC-59 녹화 — 증거 목록과 한계 (2026-09-23)

> ⚠ **정정 있음 — `08-correction.md`를 함께 읽어라.** (qa3 지적, 2026-09-23)


## 산출물
- **`SC-59-session.mp4`** — 연속 녹화. 3440×1440, 15 fps, **900.0초**, 160,055,377 바이트. 정상 마감(ffmpeg 자체 종료).
  녹화 시작 시각은 `recording-start.txt`(로컬 21:22). **이것이 정본 증거다.**
- `index-t{90,240,400,560,700,840}s.png` — 색인용 프레임 6장. HUD가 보이도록 좌상단 1146×720으로 잘랐다.
- `ffmpeg.log` — 녹화 로그.

## 한계 — 정직하게 적는다
계약(client §4)은 클립 a~d **각각의 mp4 + 대표 png**를 요구한다. 이번에는 **클립별로 나누지 않았다.**
사람이 조종하는 동안 각 동작의 시작·종료 시각을 정확히 기록하지 못했고, 짐작으로 자르면 **"그 구간이
그 동작이다"가 근거 없는 주장**이 되기 때문이다.

**qa2가 정확히 자를 수 있는 방법이 있다**: HUD에 `tick=`이 항상 찍혀 있고, 관찰 기록에 해당 tick이 남아 있다.
- 클립 a 연장(W 8초) — HUD `tick=455892`, `speed_mps=135.8` (`02-clip-a-extended.md`)
- 연결 확인 시점 — HUD `tick=449076` (`00-connection-check.md`)
- 세션 이벤트 tick — 448809 열림 / **457993 `PROTOCOL_VIOLATION`** / 458321 재접속 / 466602 정상 종료 / 467202 디스폰
tick은 20 Hz이므로 tick 차 = 초 × 20. 영상에서 해당 tick이 보이는 프레임을 찾으면 구간이 확정된다.

## 관찰 기록 파일
| 파일 | 내용 |
|---|---|
| `00-connection-check.md` | 실서버 연결의 두 출처 대조(HUD ↔ `/debug/stats` ↔ DB) |
| `01-clip-a.md` | 클립 a — 전방 추력이 뱃머리 방향 |
| `02-clip-a-extended.md` | 최고속 135.8 도달 + ⚠ 재조정 지표 이상치 |
| `03-clip-d.md` | 클립 d — 오토레벨 |
| `04-clip-c.md` | 클립 c — 경계 경고·미끄러짐·조작 유지 |
| `05-sc59-summary.md` | SC-59 종합 |
| `06-finding-protocol-violation.md` | 🔴 정상 플레이 중 `PROTOCOL_VIOLATION` 강제 종료 |
| `server-stdout.log` | 서버 로그 15줄(`dropped=0` — 유실 없음) |

## 환경 (측정 조건)
- 서버: `server_boot.py serve`, `start_tick = 443801`, 종료 `SHUTDOWN exit=0`(하드 킬 없음)
- Unity: Editor Play 모드, `STARFALL_NET_AUTOCONNECT=1` `STARFALL_GREYBOX_AUTOBUILD=1`, 사람이 Play/정지
- 인프라: `docker compose up -d`로 복구(**`down` 계열 미사용**), 기록 보존 확인(`domain_events` 4,527행,
  `QA_APPEND_ONLY_PROBE` 1, 자기 참조 7건 동결 장부 그대로)
- **동시 실행 없음**: 봇 없음, 다른 Unity 인스턴스 없음, 빌드 없음
