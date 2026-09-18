---
name: rust-authoritative-server
description: "STARFALL DYNASTY Rust 게임 서버 구현 규약. Cargo 워크스페이스 모듈형 모놀리스 구조, 고정 tick 시뮬레이션 루프(명령 큐 → 검증 → 상태 전이 → 도메인 이벤트), 서버 판정 8단계, 결정성(시드 난수·순서·정수 화폐), Axum REST/WebSocket, sqlx+PostgreSQL 트랜잭션·마이그레이션, Transactional Outbox, 멱등 명령 처리, 경제 복사 방지, Redis 사용 한계, tracing/OpenTelemetry, cargo 테스트·clippy 기준. server/ 폴더의 Rust 코드 작성·수정·리뷰, API·WebSocket·DB·경제·전투 로직, 서버 성능 작업 시 반드시 사용."
---

# Rust Authoritative Server — 세계의 사실을 결정하는 서버 규약

이 서버는 CRUD 백엔드가 아니라 **게임 세계의 사실을 결정하는 시뮬레이션**이다. 모든 규약은 세 가지를 지키기 위해 있다: 클라이언트를 믿지 않는다, 같은 입력이면 같은 결과가 나온다, 돈과 아이템은 절대 복사되지 않는다.

## 1. 워크스페이스 구조 (ADR-0001 기본안)

```
server/
├── Cargo.toml                 # [workspace], 공통 의존성 버전은 [workspace.dependencies]
├── crates/
│   ├── domain/                # 엔티티, 명령, 도메인 이벤트, 규칙. IO 없음
│   ├── sim/                   # tick 루프와 시스템(mining, trade, combat). IO 없음, 결정적
│   ├── history/               # 역사 엔진 (history-engine-engineer 소유)
│   ├── contracts/             # contracts/ 대응 serde 타입 + fixture 테스트
│   ├── persistence/           # sqlx 저장소, outbox, 트랜잭션
│   └── gateway/               # Axum REST + WebSocket, 인증, 세션
├── bins/game-server/          # 조립: 설정, tracing, 런타임, 태스크 기동
└── migrations/                # sqlx 마이그레이션 (타임스탬프 파일명)
```

`domain`·`sim`·`history`에 IO가 없어야 하는 이유: DB나 네트워크 없이 단위 테스트·재생 테스트를 돌릴 수 있고, 결정성이 깨지는 지점(시간, 소켓, 난수)이 IO 계층으로만 들어오게 된다. 크레이트 경계는 팀원 간 파일 소유권 경계이기도 하다.

## 2. Tick 루프 — 상태 변경은 여기서만

```
WebSocket/REST 핸들러 ── Command{command_id, player_id, payload} ──▶ bounded mpsc
                                                                     │
Tick 루프 (고정 간격, 전용 태스크) ◀───────────────────────────────────┘
  1. 명령 수집: 이번 tick에 받은 명령을 (수신 순번) 기준으로 확정 정렬
  2. 검증: 서버 판정 8단계 (아래)
  3. 상태 전이: WorldState 변경은 이 단계에서만
  4. 도메인 이벤트 발행: tick, sequence 부여
  5. 역사 판정: history 크레이트 호출 (순수·결정적)
  6. 영속화 요청: 영속화 태스크로 TickOutcome 전달
  7. 브로드캐스트: 세션별 가시 범위의 변경분 전송
```

- Tokio async 태스크와 시뮬레이션 tick은 다르다. 핸들러가 `Arc<Mutex<WorldState>>`를 직접 바꾸면 명령 적용 순서가 스케줄러에 따라 달라져 재현이 불가능해진다. 핸들러는 명령을 큐에 넣기만 한다.
- tick 주기(MVP 기본 10~20Hz)와 tick 초과 시 처리 방식은 ADR로 고정하고, tick 소요 시간을 메트릭으로 남긴다.
- **일관성 구분** (기획안 HSE §95): 이동·전투 같은 실시간 상태는 메모리 WorldState가 권위이고 주기적 스냅샷과 중요 이벤트만 영속한다. **돈·인벤토리·소유권·계약 정산은 DB 커밋이 끝나야 성공**이다. 경제 명령은 tick에서 검증 → 영속화 태스크가 트랜잭션 커밋 → 결과 이벤트(`TRADE_SETTLED` 등)가 다음 tick에 반영 → 그때 클라이언트에 `COMMAND_RESULT` 성공을 보낸다. 커밋 전에 성공을 알리면 서버 장애 시 "받았는데 사라진 아이템"이 생긴다.

## 3. 서버 판정 8단계 (기획안 GDD §33)

모든 명령 처리기는 이 순서를 따른다. 빠진 단계가 있으면 이유를 주석으로 남긴다.

1. 행위자 확인 — 세션의 player_id와 명령의 행위자가 같은가, 명령이 재전송(`command_id` 중복)이 아닌가
2. 위치·상태 확인 — 서버가 아는 위치·도킹·생존 상태로 가능한가 (클라이언트가 보낸 위치를 쓰지 않는다)
3. 대상 존재 확인 — 광물·아이템·상대 함선이 서버 상태에 존재하는가
4. 장비·자격 확인 — 모듈·스킬·권한
5. 규칙 검증 — 거리, 쿨다운, 용량, 잔액, 위험 지역 규칙
6. 경제·자원 상태 반영 — 매장량 감소, 가격 영향
7. 상태 변경 — 인벤토리·잔액 (경제는 트랜잭션)
8. 도메인 이벤트 생성 — 역사 판정의 입력

거부된 명령도 `COMMAND_RESULT{status: rejected, reason_code}`로 응답한다. 반복 거부는 부정행위 신호로 메트릭에 남긴다.

## 4. 결정성 규칙

- 시뮬레이션·역사 판정 안에서 `SystemTime::now()`/`Instant::now()`를 쓰지 않는다. 시간은 tick과 주입된 `GameClock`으로만 얻는다.
- 난수는 시드 기반 `rand_chacha::ChaCha8Rng`를 `(world_seed, tick, entity_id)`로 시드해 쓴다. `thread_rng()` 금지.
- 순서가 결과에 영향을 주는 곳에서 `HashMap` 순회 순서에 의존하지 않는다. `BTreeMap`을 쓰거나 ID로 정렬한다.
- 화폐·가격·수량은 정수(`i64`). 물리 계산의 `f64`는 허용하되 판정 결과(피해량 등)는 정해진 규칙으로 정수화한다.
- 시뮬레이션 규칙이 바뀌면 `sim_version`을, 역사 규칙이 바뀌면 `rule_version`을 올린다.

## 5. 영속화 (PostgreSQL + sqlx)

세부 SQL과 패턴은 `references/persistence-patterns.md`를 읽는다 (outbox, 멱등 명령, 원장, 잠금).

- 마이그레이션은 `server/migrations/`에 추가만 한다. 적용된 마이그레이션을 수정하지 않는다.
- `sqlx::query!`/`query_as!`로 컴파일 타임 검사를 받고, `cargo sqlx prepare`로 `.sqlx/` 오프라인 메타데이터를 커밋해 DB 없이도 빌드되게 한다.
- 읽고-계산하고-쓰기는 반드시 트랜잭션 안에서 행 잠금(`FOR UPDATE`) 또는 버전 컬럼으로 한다. 트랜잭션 밖의 read-modify-write가 화폐 복사의 가장 흔한 원인이다.
- 상태 변경, `domain_events` 기록, `outbox_events` 기록은 **같은 트랜잭션**이다.
- Redis는 세션·접속 상태·레이트 리밋·캐시만. Redis를 비워도 PostgreSQL로 전부 복구되어야 한다.

## 6. Axum 게이트웨이

- 라이브러리 API는 버전마다 바뀐다(예: axum 경로 파라미터 문법). 코드를 쓰기 전에 Context7로 `Cargo.toml`에 고정된 버전의 문서를 확인한다.
- 모듈별 `Router`를 조립하고, 공유 상태는 `AppState`(내부는 `Arc`)로 전달한다.
- 도메인 에러는 `thiserror`로 정의하고 `IntoResponse`에서 계약의 에러 코드로 변환한다. `anyhow`는 바이너리 조립부에서만.
- WebSocket: 업그레이드 전에 인증한다. 연결마다 수신/송신 태스크를 나누고, 송신은 bounded 채널을 쓴다. 느린 클라이언트 때문에 tick 루프가 막히지 않도록 가득 차면 스냅샷 병합 또는 연결 종료로 처리한다.
- 명령 역직렬화는 `#[serde(deny_unknown_fields)]`로 엄격하게 한다. 클라이언트 버그를 조용히 삼키지 않기 위해서다.
- 세션·명령 타입별 레이트 리밋, payload 범위 검증, `command_id` 재전송 거부.

## 7. 관측성

- `tracing` span에 `correlation_id`, `command_id`, `player_id`, `tick`을 붙인다. 하나의 채굴 요청이 게이트웨이 → 시뮬레이션 → 트랜잭션 → 역사 이벤트까지 한 trace로 이어져야 한다 (기획안 TECH §32).
- `tracing-opentelemetry` + OTLP로 내보낸다. 로컬에서는 콘솔 JSON 로그만으로도 된다.
- 필수 메트릭: tick 소요 시간(p50/p99), 명령 큐 길이, outbox 미발행 건수·지연, 거부 명령 수(사유별), WebSocket 연결 수.

## 8. 테스트와 완료 기준

| 종류 | 대상 | 도구 |
|------|------|------|
| 단위 | domain, sim 규칙 | `cargo test` |
| 속성 | 멱등성(같은 명령 2회 = 1회 효과), 화폐 보존(명시적 발행·소각 외 총량 불변), 결정성(같은 시드·명령열 = 같은 상태 해시) | `proptest` |
| 계약 | `contracts/fixtures/**` 역직렬화·재직렬화·스키마 검증 | `serde_json` + `jsonschema` |
| DB 통합 | 트랜잭션, outbox, 잠금 | `#[sqlx::test]` (docker compose PostgreSQL) |
| 시나리오 | 봇 클라이언트 흐름 | qa 소유 `tools/bots/` |

완료로 보고하기 전: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --workspace` 통과. 요청 처리 경로에서 `unwrap()`/`expect()`를 쓰지 않는다 (테스트와 시작 시 설정 로드는 예외).

## 9. 로컬 개발 환경

- 레포 루트 `docker-compose.yml`에 PostgreSQL과 Redis를 **버전 고정**하고 healthcheck를 둔다.
- `.env.example`에 `DATABASE_URL`, `REDIS_URL`을 두고 실제 `.env`는 커밋하지 않는다.
- 툴체인은 `server/rust-toolchain.toml`로 고정한다.
