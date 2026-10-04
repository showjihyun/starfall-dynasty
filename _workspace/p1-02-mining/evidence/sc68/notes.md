# SC-68 사람 세션 — 1차 시도 (2026-10-04) — FAIL, 재시도 예정

- 월드: 01a104a3-b485-7078-9da9-400a01a42f01 (new_world.json, 발견 0 — 재시도에 그대로 쓴다)
- 서버 실행 1: 11:00~22:16, Editor 접속 뒤 PC 절전 → 실제 시간 기준 max-seconds 7200 도달, stdin shutdown 정상 종료(server_run1_timeout.log)
- 서버 실행 2: 같은 월드, 사람이 Play 했으나 광맥을 찾을 수 없어 중단, stdin shutdown 정상 종료(server_run2_aborted_fail.log)
- Editor 첫 Play 는 빈 씬 — 환경 변수(STARFALL_GREYBOX_AUTOBUILD·NET_AUTOCONNECT·DEV_AUTH_SECRET) 미주입. 리더가 프로세스 환경에만 주입해 재기동(unity_editor.log)

## 판정: FAIL (코드 확인, 스크린샷 없음)
요소 1: 광맥이 3D 로 그려지지 않는다 — HUD 텍스트 목록뿐, 위치·거리·방향 없음(GreyboxSession.BuildMarkers 는 reference_markers 만)
요소 2: 표면 거리·사거리 안·속도 표시 없음(GreyboxMiningSession.OnGUI)
요소 3: 쿨다운 표시 없음
기타: E 키가 채굴(GreyboxMiningSession.cs:142)과 롤(ShipInputSampler.cs:106)에 동시에 묶임
원인: 화면 요소는 테스트가 겨냥하지 않았고 03_client_impl.md "사람이 확인할 항목" 이 C3 를 사람 세션에 넘기지 않았다

사용자 결정(2026-10-04): 고치고 다시 세션 → 태스크 C4

---

# SC-68 사람 세션 — 2차 시도 (2026-10-04~05, C4 뒤)

- 월드: 01a104a3-b485-7078-9da9-400a01a42f01 (1차와 같음 — 1차에 발견 0)
- 서버: server.log (stdin shutdown 정상 종료, tick=277563). Editor: unity_editor.log (환경 변수 프로세스 주입)
- 클라이언트: C4 반영(03_client_impl.md `## C4`, evidence/c4/ EditMode 423/0/2, PlayMode 2/0)
- 봇: bot900.log — `bots mine --deposit far-reach --mines 1 --label bot-900`, 사람이 "봇 돌려" 신호 뒤 실행

## DB 관측 (판정 입력 — 월드의 MINERAL_DISCOVERED ≥ 1)
- historical_events: MINERAL_DISCOVERED cradle/glacine (사람, 2026-10-04 14:16:05Z) · cradle/starfall-glass (bot-900, 16:11:04Z), 둘 다 mineral-discovery@1
- domain_events: MINERAL_MINED ≥ 4 (사람 세션)

## 사람 진술 (원문)
- "잘 되는데?" — Play 뒤 광맥 표식·채굴 동작
- "끝났어. 하늘색 배너도 떴어" — 성계 전체 배너(요소 5 둘째 장면) 확인
- 스크린샷: **제출되지 않음**. 소감 두 문항(알림·배너 구분, 1 초 체감)에 대한 명시적 예/아니오 **없음**

## 판정
절차서의 판정 입력(스크린샷 6장 + 소감)이 갖춰지지 않았다. 사람 진술과 DB 기록은 요소 1~5 동작을 뒷받침하지만,
요소별 화면 증거가 없으므로 **PASS 가 아니라 "사람 확인(진술) — 스크린샷 없음"** 으로 기록하고, 수용 여부는 사용자 결정으로 넘긴다(starfall-dev Phase 5 완료 조건).

## 사용자 결정 (2026-10-05)
"진술로 수용하고 병합해줘" — SC-68 을 사람 진술로 **수용**. 스크린샷 없음은 그대로 기록에 남긴다.
