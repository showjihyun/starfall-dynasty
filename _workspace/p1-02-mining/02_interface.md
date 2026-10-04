# p1-02-mining — T0 경계 인터페이스 (server ↔ history)

작성: history (골격 초안, §1~§6) · 2026-09-27
확정: server(T0, #10) · 2026-09-27 — 아래 §1~§6은 실제 구현과 대조해 확정했다(불일치 0). §7 체크리스트 갱신, §8~§10을 server가 추가했다. 이 파일이 정본이다.

## 1. 크레이트·의존 방향

- `starfall-history`(위치 `server/crates/history`) — 순수 판정 코어. `starfall-contracts`만 의존. IO 없음(sqlx/tokio/axum/redis/rand/chrono 금지, `starfall-sim`과 같은 금지 목록을 Cargo.toml에 건다).
- DB 어댑터(러너)는 `starfall-persistence`의 history 모듈(`persistence/src/history.rs`·`history/**`) — 파일 소유는 history.
- 조립은 `bins/game-server`. `persistence → history(코어) + contracts`. `gateway → contracts`만(게임 규칙 없음).

## 2. 기동 순서 (ADR-0013 §7 확장, §10.2에서 확정)

경제 상태 적재 → `history::load`(한 스냅샷에서 판정 상태 + 초기 BACKFILL 목록 + 커서를 읽음) → `runtime::build(initial_backfill, history_rx)` → 영속화·러너 spawn → bind.

목록이 `build`의 인자이므로 "목록이 채워지기 전에 연결을 수락"하는 상태가 구조적으로 존재하지 않는다.

## 3. 커밋 알림 채널 (깨우기 전용)

- `watch::Sender<Option<u64>>` — 마지막 커밋 tick. 신호일 뿐이고 워터마크는 `worlds.last_tick`(DB)이다.
- 러너는 `changed()` 또는 1초 타이머로 깬다(ADR-0014 §2 — 알림 유실도 폴링이 따라온다).
- 영속화가 종료 시 Sender를 drop하면 러너는 마지막 커밋까지 한 번 더 따라잡은 뒤 종료한다(I-62).
- 러너 halt(§4)와 영속화 fatal은 **분리된 핸들**이다(§10.1 조정 3). 러너가 멈춰도 서버는 계속 돈다.

## 4. 역사 기록 채널 (LIVE)

- **채널이 나르는 것은 NOTICE가 아니라 역사 기록 레코드 자체**(계약 `MINERAL_DISCOVERED` 타입) — §10.1 조정 1. BACKFILL 목록도 같은 타입으로 보관한다.
- `mpsc<MineralDiscoveredEvent>` (bounded 256, delivery=LIVE). 러너가 커밋 뒤 만들어 보낸다. producer = history.
- `HISTORICAL_EVENT_NOTICE`(delivery/envelope tick/message_id 포함)는 게이트웨이가 조립한다 — producer = server(레지스트리 반영 완료, §10.2 후속).
- 드라이버가 먼저 끝나 send가 Closed로 실패하는 것은 오류가 아니다(§10.1 조정 2) — 기록은 이미 커밋돼 있고 다음 기동 BACKFILL로 전달된다.
- **T0 실제 타입(임시)**: S1이 아직 `MINERAL_DISCOVERED` 계약 타입을 만들지 않아서(T0 ∥ S1, 순서상 동시), 지금 채널 항목 타입은 `starfall_history::HistoricalEventRecord`(`server/crates/history/src/record.rs`) — `historical_event_id: UuidV7` 하나뿐인 자리표시자다. **H1이 S1의 계약 타입이 나오는 대로 이 구조체를 실제 타입으로 교체한다**(교체 지점 주석을 파일에 남겨 뒀다). 채널 용량은 합의대로 256, main.rs에 생성됐지만 T0 골격은 아무도 보내지 않는다(H2가 채운다).

## 5. 직렬화 문맥 (누락 틈 방지, ADR-0014 §6)

tick 드라이버 스레드(routes 소유)가 BACKFILL 목록을 갖는다. 한 tick 안 순서:
(a) route_outbound 중 `SESSION_READY` 받은 세션에 ready 표시 →
(b) 새로 ready된 세션에 목록 전체를 BACKFILL로 송신 →
(c) LIVE 수신 시 목록에 추가 + ready인 세션에 브로드캐스트.

BACKFILL → 조회 API 전환 트리거는 32건(ADR-0014 §6 개정, 세션 시작 메시지 3건 + 여유 고려).

## 6. 관측

`HistoryHandles` → `/debug/stats`(records, conflicts, halted, cursor_tick, catchup_rows).

## 7. server 확인 완료 항목 (2026-09-27)

- [x] `server/Cargo.toml`에 `crates/history`(`starfall-history`) 멤버 추가 완료. 빈 lib + smoke test 1개(`crate_links_into_the_workspace`) — `cargo test -p starfall-history` 1 passed. H1은 그 위에 실제 판정 테스트부터 쌓으면 된다.
- [x] 위 §1~§6이 T0 실제 구현과 어긋나지 않는지 확인 — 어긋남 0. §4에 임시 타입 메모 추가(위).
- [x] **SC-113 선행 조건 실측 완료 — RAN 줄이 `--nocapture` 없이도 보인다.** 아래 §9 참고. **설계를 바꿀 필요가 없다** (`std::io::stderr().write_all` 이 libtest 캡처를 실제로 우회한다).

## 8. `starfall-testdb` 크레이트 (Q-2, `02_server_ack.md` §3.2 그대로 구현)

- 위치 `server/crates/testdb`(`starfall-testdb`, `publish = false`, `[dependencies]`로만 — dev-dependency로 쓴다).
- `TestDb::create(test_name) -> Option<Self>`: `DATABASE_URL`의 dbname을 무시하고 관리 DB(`postgres`)에 붙어 `CREATE DATABASE starfall_test_<uuidv7 simple>` → 새 풀 → `sqlx::migrate!("../../migrations")`(0001~0003 전부, history의 0003도 같이 걸린다).
- **구조적 보호(SC-114)**: 풀을 여는 함수는 `open_pool` 하나뿐이고 `starfall_test_` 접두사가 아니면 `assert!`로 패닉한다. 단위 테스트 `refuses_non_test_database`(`#[should_panic(expected = "SC-114")]`)가 이름 그대로 확정됐다.
- 모드: `STARFALL_DB_TESTS=required`(CI) — 실패 시 패닉. 미설정(로컬) — `None` + `SKIPPED` 줄.
- 정리: `TestDb::drop()`(명시 호출) + `create()` 진입 시 1시간 넘은 `starfall_test_%` 를 UUIDv7 타임스탬프로 판별해 청소(패닉으로 남은 DB용).
- 실측(로컬 postgres:18.6-trixie, 15432): `cargo test -p starfall-testdb` 3 unit + 1 DB 통합(`tests/smoke.rs`) 전부 통과. DB 통합 테스트 전후 `starfall.domain_events` count **4805 → 4805**(불변, Q-2 §4 절차 확인). 테스트가 만든 `starfall_test_<uuid>` DB는 `drop()`으로 정상 삭제됨을 `pg_database` 조회로 확인.

## 9. SC-113 선행 조건 실측 (server, 2026-09-27) — qa 통보용

`cargo test -p starfall-testdb --test smoke`(`--nocapture` **없이**) 출력 발췌:

```
running 1 test
STARFALL_DB_TEST RAN testdb::smoke::creates_migrates_and_drops_an_isolated_database
test creates_migrates_and_drops_an_isolated_database ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.26s
```

RAN 줄이 **통과한** 테스트 위에 그대로 보인다 — `--nocapture` 여부와 무관(같은 명령을 `-- --nocapture`로도 실행해 동일 출력 확인). `DATABASE_URL`을 지운 실행에서는 `SKIPPED` 줄도 마찬가지로 보였다. **결론: `std::io::stderr().write_all`은 libtest의 print-매크로 캡처를 실제로 우회한다 — 방식을 바꿀 필요가 없다.** qa의 CI 게이트(§3.2-3, `RAN` 줄 수 대조)는 설계대로 진행하면 된다.

## 10. 게이트웨이 경계 — T0는 손대지 않았다 (S5로 넘김)

T0 합의(`server/Cargo.toml` 주석, 이 파일 §1)대로 `starfall-gateway`는 이번에도 `starfall-persistence`·`starfall-history`를 의존하지 않는다(`crates/gateway/Cargo.toml` 변경 없음). "목록을 넘기기 전에는 `/ws`가 연결을 받지 않는다"는 지금은 **런타임 플래그가 아니라 순서 그 자체**로 성립한다 — `bins/game-server/src/main.rs`에서 `starfall_persistence::history::load(...)`를 `await`한 뒤에야 `runtime::build(...)` → `persistence`/`history::run_runner` spawn → `TcpListener::bind` → `axum::serve`가 실행된다(실측: 새 월드로 기동 → `/healthz`·`/debug/stats` 응답 정상, tick 정상 진행 — 기동 로그 발췌를 `03_server_impl.md`에 남긴다). §4의 임시 타입이 실제 `MINERAL_DISCOVERED`로 바뀌고 `runtime::build`가 `initial_backfill: Vec<...>`을 **인자로 받게 되는 시점**(S5)에 이 성질이 컴파일 타임 보장으로 바뀐다 — 그 전까지는 "구조적으로 그렇게 짜여 있다"이지 "타입이 강제한다"는 아니다. S5가 이 차이를 알고 시작해야 한다.

## 11. H2 — `history::load`/`run_runner` 최종 시그니처 (2026-09-28, main.rs 배선 대기)

T0 골격(빈 `Ok`, 인자 2개/4개)에서 실제 구현으로 바뀌면서 시그니처가 늘었다. **호출부(`bins/game-server/src/main.rs:150,224`)는 아직 옛 시그니처를 쓴다 — server가 고친다(H2는 main.rs 를 건드리지 않는다).** 이 절이 그 배선에 필요한 정본이다.

```rust
// server/crates/persistence/src/history.rs
pub async fn load(
    pool: &PgPool,
    world_id: UuidV7,
    rule: starfall_contracts::data::SignificanceRuleTable, // S2 가 적재한 값을 그대로
) -> Result<HistoryBoot, HistoryError>;

pub async fn run_runner(
    pool: PgPool,
    world_id: UuidV7,
    boot: HistoryBoot,                              // load() 의 결과. run_runner 가 소비한다
    commit_notify: watch::Receiver<Option<u64>>,     // 기존과 동일
    live_tx: mpsc::Sender<MineralDiscoveredEvent>,   // 기존과 동일(타입은 이제 실제 계약 타입)
    handles: HistoryHandles,                         // 신규 — /debug/stats 연결용
) -> ();                                             // (반환 없음, 기존과 동일)
```

배선 순서(main.rs):

```rust
let history_boot = starfall_persistence::history::load(&pool, config.world_id, rule).await?;
let initial_backfill = history_boot.initial_backfill.clone(); // run_runner 에 boot 를 넘기기 전에 복사
// ... initial_backfill 을 runtime::build(...) 에 넘긴다 ...
let history_handles = starfall_persistence::history::HistoryHandles::new();
let history_runner = tokio::spawn(starfall_persistence::history::run_runner(
    pool, config.world_id, history_boot, commit_notify_rx, history_live_tx,
    history_handles.clone(),
));
```

`HistoryBoot.initial_backfill: Vec<starfall_contracts::historical::MineralDiscoveredEvent>` — clone 가능(derive Clone). `HistoryHandles` 필드: `records_total`·`conflicts_total`·`detector_halted_total`·`halted`·`cursor_tick`·`catchup_rows_total`(전부 `Arc<Atomic*>`, `PersistHandles` 와 같은 패턴).

**반성(팀장 지적)**: 이 경계 시그니처를 두 번 바꾸는 동안(1차: 타입만 바뀜, 2차: 인자 추가) 호출부 담당(server)에게 먼저 알리지 않아 워크스페이스 빌드가 두 차례 깨졌다. 앞으로 `load`/`run_runner`/`HistoryBoot`/`HistoryHandles` 의 공개 시그니처를 바꿀 때는 **커밋 전에** 이 절을 먼저 갱신하고 server 에게 diff 를 보낸다.
