# p0-02-networking-spike

## 요청 원문
p0-02 시작해줘 (네트워킹 스파이크)

## 분류
- 유형: SLICE (기술 스파이크)
- 로드맵 Phase: p0 (사전 제작) — **이 슬라이스가 Phase 0 종료 기준을 증명한다**
- 종료 기준(GDD §39): **"30명 동시 접속 + 실시간 이벤트 기록" 검증**

## 범위
포함:
- 서버 WebSocket 게이트웨이: 연결 수립·인증(개발용 토큰 수준)·수명 주기·정상 종료
- 명령 수신 → 처리 → 응답의 왕복 경로 (계약 기반. 기존 `PING_SERVER`/`PING_REPLY` 재사용 또는 확장은 architect 판단)
- **도메인 이벤트 기록**: 발생한 이벤트를 PostgreSQL에 영속화(첫 sqlx 마이그레이션), 조회로 확인 가능
- 고정 tick 루프의 최소 형태: 명령은 큐에 넣고 tick에서 처리 (아키텍처 전제를 지금 검증한다 — 나중에 바꾸면 전면 수정이 된다)
- Unity 클라이언트 전송 계층: `IRealtimeTransport` 추상화 + PC용 WebSocket 구현, 연결·송신·수신·재연결 기본
- 부하 검증: 봇 클라이언트 30개 동시 접속, 이벤트 기록률·지연 측정 (qa 소유 `tools/bots/`)
- 관측: tick 소요 시간, 연결 수, 이벤트 기록 수 메트릭·로그

제외(다음 Phase로):
- 게임플레이(이동·채굴·전투·인벤토리), 월드 상태 모델
- Historical Event 판정·증거·연대기 (p2)
- Outbox 워커·NATS, Redis 세션 저장 (필요성이 측정되면 도입)
- WebGL 전송 구현 (추상화만 두고 PC 구현만)
- 실제 인증·계정 (개발용 토큰으로 대체, ADR에 한계 명시)
- 함선·씬·비주얼 (techart 미투입)

## 실행 모드
- 서브 에이전트 대체 모드 (이 세션에 Agent `name` 파라미터·Task 도구 없음)
- 팀원 간 메시지는 `_workspace/p0-02-networking-spike/` 파일과 리더 중계로 대체

## 환경
- Bash 명령 앞에: `export PATH="$HOME/.cargo/bin:$HOME/.dotnet/tools:/c/Users/CHOISOOYEON/AppData/Local/Unity/bin:$PATH"`
- p0-01에서 확립: Rust 워크스페이스(`server/`), Unity 6000.6.1f1 프로젝트(`client/`), 계약(`contracts/`), docker compose(PostgreSQL 15432 / Redis 16379), DTO 생성기(`tools/codegen/`)
- 인프라는 healthy 상태로 떠 있다. 전역 Docker 정리 금지(타 프로젝트 컨테이너 5개 상주)
- 커밋 `eb64f3e`까지 로컬에만 있음. 푸시 안 함

## 사용자 결정 (2026-09-18)
1. **성능 잠정 게이트는 Phase 0 종료를 막지 않는다** (Q2 → b). tick 초과율·단일 tick 상한·왕복 p99는 측정 전 추정치이므로 미달이어도 실패로 보지 않는다. 대신 **첫 측정치를 p1 회귀 기준선으로 고정**해 기록한다. 정확성 게이트(손실 0, sequence 빈틈 0, tick skip 0, 이벤트 기록 가시성)는 그대로 하드 게이트다.
2. **"30명 동시 접속" = 봇 30 + Unity 클라이언트 1** (Q3 → AC-17). 실제 클라이언트가 부하 중에도 정상 동작하는지까지 본다.
3. **`calendar_scale = 60`** (Q1). 현실 1초 = 게임 1분. ADR-0006의 게임 시간 계산에 반영하고, 월드에 묶여 불변인 상수로 둔다.
4. **CI는 다음 슬라이스로 미룬다** (Q5). `sqlx-cli`/`.sqlx/` 오프라인 빌드 설정과 함께 별도 슬라이스로 묶는다.
5. Q4(`.env.example`에 `STARFALL_DEV_AUTH_SECRET=dev_only_not_a_secret` 공개): 값 자체가 비밀이 아니고 저장소가 공개임을 전제로 그대로 둔다. 실제 비밀은 `.env`(무시 대상)에만.

## 투입
- 팀원: architect, server, client, qa
- 제외: designer(게임 규칙 없음), history(역사 판정은 p2), techart(비주얼 없음)

## 주의 (p0-01에서 얻은 것)
- 문서에 적은 명령은 실행해서 확인한 것만 적는다 (p0-01에서 Unity 테스트 명령·로그 경로가 실제와 달랐다)
- 검증은 "0건 통과"가 아니라 **몇 건을 검사했는지**가 증거에 드러나야 한다
- 구현자는 자기 영역의 판정 방법이 이 PC에서 무효가 되는 조건을 먼저 찾는다 (p0-01에서 2건 발견)
