# p1-01-ship-movement 서버 구현 — 진행 기록

- 작성: server, 2026-09-20
- 이 문서는 태스크가 끝날 때마다 절을 append 한다(중단 대비). 완료 전 최종 정리 절이 맨 아래 붙는다.
- 입력: `02_server_ack.md`(인수인계), `01_architect_tasks.md`(S1~S6), `02_sprint_contract.md`(SC-01~40 server 몫), `docs/specs/p1-01-ship-movement.md`, ADR-0009~0012.
- 중단 후 리더가 전달한 architect 확정 3건(휴면 입력, 재개 초기화, maxItems 강제 위치)과 `STARFALL_DATA_DIR` 해석 규칙을 반영한다.

## S1 — 계약 Rust 타입 (완료)

`server/crates/contracts/`에 신규 7타입 추가:

- `src/primitives.rs`: `PositionMm`·`VelocityMmPerSecond`·`QuaternionComponentMicro`·`AngularVelocityMdegPerSecond`·`ControlAxisMilli`·`InputSeq`(범위 검증 정수 newtype, 매크로 `bounded_int_newtype!`), `DataId`(lower-kebab 문자열, `data/`의 `id`와 글자 그대로 같다), `SchemaVersion`(데이터 테이블의 동적 스키마 버전 — `ConstSchemaVersion<V>`와 다르다).
- `src/commands.rs`: `SetShipControlCommand`/`SetShipControlPayload`(`SET_SHIP_CONTROL`).
- `src/messages.rs`: `WorldSnapshotMessage`/`WorldSnapshotPayload`/`ShipState`/`ShipPresence`(`WORLD_SNAPSHOT`), `RejectReasonCode`에 `RateLimited`·`StaleInput` 추가.
- `src/events.rs`: `ShipSpawnedEvent`/`ShipSpawnedPayload`, `ShipDespawnedEvent`/`ShipDespawnedPayload`/`DespawnReason`.
- `src/data.rs`(신규 파일): `ShipClassTable`·`StarSystemTable`·`SyncTuningTable` + 중첩 타입. 필드별 `deserialize_with`로 스키마의 `minimum`/`maximum`/`exclusiveMinimum`을 강제(매크로 `de_f64_range!`/`de_i64_range!`). 모든 실제 `data/` 파일과 fixture를 역직렬화하는 단위 테스트 포함.
- `src/registry.rs`: `CONTRACT_TYPES`에 7행 추가(13종), 레지스트리 이름 상수 7개.
- `src/lib.rs`: 신규 타입 re-export.
- `tests/contract_tests.rs`: `EXPECTED_SCHEMA_COUNT` 18, `EXPECTED_VALID_FIXTURES` 26, `EXPECTED_INVALID_FIXTURES` 34. `SERDE_REJECTION_TABLE`에 신규 14행. `registry_consistency`가 `kind: "data"`(타입 태그 필드도 `schema_version` const도 없다)를 건너뛰도록 분기. `required_field_mutations`가 `kind: "data"`를 건너뛰도록 분기(envelope·단일 payload 블록이 없는 모양이라 일반화하면 패닉한다 — 각 데이터 타입 전용 거부 단위 테스트가 대신 덮는다). `payload_required` 헬퍼가 `$defs`의 "첫 항목"이 아니라 `properties.payload.$ref`가 가리키는 이름을 따라가도록 수정(`WORLD_SNAPSHOT`처럼 `$defs`가 2개— `WorldSnapshotPayload`·`ShipState` — 인 경우 `serde_json::Value`의 기본 `BTreeMap` 표현이 키를 알파벳순으로 정렬해 "첫 항목"이 조용히 `ShipState`가 되는 버그를 잡았다).

**검증**: `cargo test -p starfall-contracts --locked` — 33 (단위) + 10 (통합) = 43 passed. `cargo clippy -p starfall-contracts --all-targets -- -D warnings` — clean.

**SC 대응**: SC-35~SC-40 전부 이 절에서 통과(구체 수치는 최종 정리 절에서 전체 게이트와 함께 표로 정리한다).

## S2 — 데이터 로딩 (완료)

`bins/game-server/src/config.rs`에 `data_dir_override: Option<PathBuf>` 필드 추가(`STARFALL_DATA_DIR` 읽기). 아래 "S2 완료" 절에 로더·검산·검증 결과가 있다.

### 중단 중 architect가 확정한 것 반영 계획

1. **휴면 입력**(ADR-0011 §6.1): 잔류 중 추력·롤 0, 목표 자세 = 그 tick 현재 자세, `brake=false`, `flight_assist=true`(조종사의 마지막 토글을 잇지 않는다). 이월 창이 먼저, 그 뒤 휴면 입력 — S4(tick 통합)에서 구현.
2. **재개 초기화**(ADR-0011 §6.2): 월드 상태(`ship_id`·p·v·q·ω 2종) 이어받고, 세션·입력 상태(`last_applied_input_seq = None` 포함)는 새로 시작, 이월 입력 버림 — S4에서 구현.
3. **`maxItems: 64` 강제 위치 = 입장에서 막는다**: 함선 수가 `max_entities_per_snapshot`에 도달하면 `GET /ws`가 503 `reason=world_full`. 게이트웨이는 tick 루프가 갱신하는 원자 게이지를 읽는다(I-13 위반 아님 — tick 번호 읽기와 같은 취급). 조립 시 초과는 서버 버그이므로 `debug_assert` + `ERROR` + `snapshot_over_capacity_total`을 남기고 자르지 않는다 — S4·S5에서 구현.
4. **`STARFALL_DATA_DIR` 해석**: 설정돼 있으면 그 경로만(탐색 없음). 없으면 cwd 기준 `data` → `../data` 순으로 처음 존재하는 디렉토리. 해석된 절대 경로를 로그와 `/debug/stats`에 남긴다. 못 찾으면 시도한 절대 경로를 전부 로그에 남기고 거부(종료 코드 1) — **완료**.

### S2 완료

- `bins/game-server/src/data.rs`(신규): `resolve_data_dir`(override면 탐색 없음, 아니면 `data`→`../data`), `load`(ships/*.json, world/systems/*.json 1개, movement/sync-tuning.json). 유도값 검산 7종: `id` 중복 없음, `main_thrust_mps2 ≥ lateral/reverse`, `point_count == len(points_m)`, `soft ≤ hard`, 스폰 지점 전부 하드 경계 안(거리 계산에 `sqrt` 사용 — `starfall-sim` 밖이라 허용), `reconnect_resume_window_seconds ≤ linger_seconds`, `tick_hz % snapshot_hz == 0`. `max_entities_per_snapshot ≤ 64`는 이미 계약 스키마 층(`SyncTuningSnapshot` 의 `de_count_1_64`)에서 막힌다 — 추가 코드 불필요.
- `main.rs`: 데이터 로딩을 마이그레이션보다 **먼저** 배치(DB가 죽어도 데이터가 깨졌으면 그것부터 안다). 실패 시 로그 후 `?`로 기동 거부.
- `bins/game-server/Cargo.toml`: `serde`·`serde_json` 의존성 추가(데이터 로더에 필요).
- `crates/gateway/src/stats.rs`: `/debug/stats`에 `data_dir`·`ship_classes_loaded`·`spawn_points_loaded`·`snapshot_interval_ticks` 추가(SC-05). 같은 파일에서 `REJECT_REASONS`를 8라벨로(`RATE_LIMITED`·`STALE_INPUT` 추가, 스키마 enum 순서), `MESSAGE_TYPES`에 `WORLD_SNAPSHOT` 추가(4라벨) — 컴파일 블로커라 S2 단계에서 먼저 처리했다(원래 S5 몫).

**검증**: `cargo test -p starfall-game-server --locked` — 12 passed(정상 로딩 1 + 위반 8종 대응 거부 테스트 + `resolve_data_dir` 탐색/비탐색 2건 + 기타). `cargo clippy` 클린.

**알려진 한계**: 성계 파일이 `world/systems/`에 정확히 1개여야 한다(0개·2개 이상은 기동 거부) — 이번 슬라이스는 "한 성계"이므로 designer 실제 데이터와 맞지만, 스펙·ADR 어디에도 "복수 성계 시 선택 규칙"이 명시돼 있지 않아 내가 정한 것이다. 다성계가 필요해지면 선택 메커니즘(설정값 등)을 architect와 정해야 한다.

## S3 — 월드 상태 모델과 적분기 (완료)

`server/crates/sim/src/world/` 신규:

- `vec3.rs`: `Vec3{x,y,z}` — `sqrt`만 쓰는 내적·외적·길이·정규화. `std::ops::Add`/`Sub` 구현(clippy `should_implement_trait` 회피 — 이름이 같은 고유 메서드 대신 실제 트레이트를 구현했다. 호출부의 `.add()/.sub()` 문법은 그대로 동작한다).
- `quat.rs`: `Quat{x,y,z,w}` — Hamilton 곱(`std::ops::Mul`), 켤레, 회전(`rot(q,v)=vec(q⊗(v,0)⊗conj(q))`), 재정규화.
- `ship.rs`: `ShipPhysicsState{p,v,q,omega_aim,omega_roll}`.
- `quantise.rs`: `quantise`/`dequantise` — `q(x,scale,lo,hi)=clamp(round(x*scale),lo,hi)`. `f64 as i64`가 Rust 1.45+ 포화 변환이라는 점을 문서화.
- `integrate.rs`: **ADR-0010 §2의 12단계를 순서 그대로 구현.** `ControlInput::dormant(q)` — 휴면 입력(추력·롤 0, aim=현재 자세, brake=false, **flight_assist=true**, 조종사 토글을 잇지 않는다) 헬퍼 포함. 금지 함수(`sin`/`cos`/`mul_add`/`hypot`/`to_radians`/`signum`) 전면 미사용 — grep으로 확인(아래).
- `spawn.rs`: `spawn_hash`(splitmix64, 손으로 씀, 크레이트 없음), `choose_spawn_point`(간격 탐색 + 전부 점유 시 반경 바깥 밀어내기), `facing_toward_origin`(성계 원점을 바라보는 최소 회전, 대척점 분기 포함).
- `mod.rs`: 위 전부 `pub use`.
- `crates/sim/src/lib.rs`: `pub mod world;` 추가.

### 판단이 필요했던 지점 — architect 확인 요청

**`choose_spawn_point`의 "시도수" 해석** (ADR-0010 §4). "전부 점유 시 `radial_offset_step_m × ceil(시도수 / point_count)` 밀어낸다"의 "시도수"가 정확히 무엇의 누적인지(이번 탐색 한 번의 시도 수인지, 같은 `index0`로 겹친 과거 스폰 전체의 누적인지) 명시돼 있지 않다. **내 선택: "이미 세계에 있는 함선 수 + 1"을 시도수로 쓴다** — 세계가 채워질수록 새 스폰이 더 바깥으로 밀리는 단조 함수이고, 문구의 의도(겹치는 스폰마다 다른 오프셋)를 보존한다. 코드 주석(`spawn.rs`)에 판단 근거를 남겼다. 31 연결 부하(AC-18)에서 스폰 링(12지점)이 30개 연결보다 작아 이 분기가 실제로 밟힐 가능성이 높다 — **결정성 회귀가 이 경로를 exercised하게 되면 재확인이 필요하다.**

### 검증 (SC-01~04 해당분)

- `cargo test -p starfall-sim --locked` — 34 passed. SC-15(항등 자세 + `thrust_z_milli=1000` + 20 tick → z만 증가, 양자화 정수 **18375** — `contracts/fixtures/SHIP_CLASS/example-scout.json` 값 기준 손계산과 일치. **이 숫자가 C3(클라이언트)의 첫 테스트와 같아야 한다**), AC-4(b~h) 전부 포함.
- `cargo clippy -p starfall-sim --all-targets -- -D warnings` — 클린.
- SC-02(금지 크레이트): `grep -nE "axum|sqlx|redis|rand|chrono|tokio|hash|glam|nalgebra" crates/sim/Cargo.toml` → 매칭 0.
- SC-03(금지 함수): 순진한 grep(`sin|cos|...`) 매칭 59건(전부 `.expect(`류 오탐 + 식별자 일부). 정밀 grep(ADR-0010 §3의 정확한 커맨드, 메서드 호출 형태로 좁힘) → **매칭 0**. 두 번 걸렸다가 고친 것: (1) 주석에 "f64::signum" 문자열이 있어 정밀 grep에도 걸림 → 문구를 바꿔 회피, (2) `quat.rs`의 회전 검증 테스트가 `half.sin()/half.cos()`로 기대값을 만들었는데 이 grep은 `crates/sim/src` 전체(테스트 포함)를 본다 → 45도 반각의 `sin=cos=sqrt(1/2)` 항등식으로 바꿔 `sqrt`만 쓰게 정리.
- SC-04: `SystemTime`/`Instant`/`HashMap` 사용 0건(문서 주석의 언급 1건 제외).

## S4 — tick 통합: 스폰·잔류·재개·디스폰·입력 판정 (완료)

`server/crates/sim/src/entities.rs`(신규): `ShipEntity`(존재 구간·소유·입력 이월 부기 + `crate::world::ShipPhysicsState`) + `activate`/`discard_carried_input_for_resume`/`start_lingering`.

`server/crates/sim/src/session.rs`(수정): `ShipControlAdmission`(`Stale`/`RateLimited`/`Accepted{superseded_previous}`), `SessionState`에 `last_applied_input_seq`·`tick_winner: Option<(tick, payload)>`·`tick_processed_count/at` 추가, `step_ship_control()`(세션 안에서 **도착 순서대로** 불러야 하는 증분 last-wins 판정 — `(5,3)`이 도착하면 5가 먼저 이겨 `last_applied`가 되고, 그 다음 3은 **5와 비교돼** stale이 된다, pre-tick 값과 비교하지 않는다), `winning_input_for_tick()`.

`server/crates/sim/src/simulation.rs`(대폭 수정) — 한 tick의 순서(모듈 문서에 명시):
1. 제출 처리(세션 열기 — 스폰/재개 판정 → 명령 — 입력 후보 판정 → 세션 닫기 — 잔류 시작)
2. 물리 적분(활성 + 잔류 전부, ADR-0010 §2 12단계)
3. 잔류 만료 판정 → 디스폰
4. 스냅샷 조립(직렬화는 하지 않는다 — ADR-0011 §3)

주요 구현:
- **스폰**: `open_session`이 `actor_ship` 역방향 조회로 "이 actor가 이미 잔류 함선을 가졌는가"(I-29)부터 확인 → 있으면 재개(`activate` + `discard_carried_input_for_resume`, **새 `SHIP_SPAWNED` 없음**), 없으면 `choose_spawn_point` + `facing_toward_origin`으로 신규 스폰. `SHIP_SPAWNED.causation_id = SESSION_OPENED.event_id`(같은 tick).
- **재개 초기화**(ADR-0011 §6.2, architect 확정): 월드 상태(`ship_id`·p·v·q·ω 2종)는 그대로, `last_applied_input_seq = None`으로 리셋, 이월 입력(`last_real_input`)은 버린다. `discard_carried_input_for_resume`이 `activate`와 분리된 이유: 최초 스폰은 이 초기화가 필요 없다.
- **입력 admission**: `handle_command`가 `SetShipControl`을 `session.step_ship_control()`에 넘기고 `Stale`→`STALE_INPUT`, `RateLimited`→`RATE_LIMITED`, `Accepted{superseded_previous}`면 `superseded_previous`가 참일 때 `input_superseded` 카운터 증가.
- **이월/휴면**(ADR-0011 §4·§6.1, architect 확정): `integrate_ships_for_tick`이 그 tick의 승자 입력이 있으면 그것을, 없고 `ticks_since_real_input < carry_forward_max_ticks`면 마지막 진짜 입력을 이월(카운터 증가), 그것도 없으면(이월 만료 **또는** 애초에 진짜 입력이 없음 — 잔류는 세션이 없으므로 후자로 항상 떨어진다) `ControlInput::dormant(현재 자세)`(추력·롤 0, `brake=false`, **`flight_assist=true`**, 조종사 토글을 잇지 않는다). **잔류 중에도 5단계 오토레벨은 계속 돈다** — dormant가 "그대로 두는 값"이 아니라는 뜻이고, SC-11의 독립 계산이 5단계를 반드시 포함해야 하는 이유다.
- **잔류/디스폰**: `close_session`이 함선을 `start_lingering`(잔류 시작 tick + 원인 이벤트 기록, 이월 부기는 건드리지 않는다 — 이월 창이 세션 경계를 가로질러 이어진다). `expire_lingering_ships`가 `linger_seconds`를 넘은 잔류 함선을 `despawn_ship`. `SHIP_DESPAWNED.causation_id`는 **그 잔류를 시작시킨 `SESSION_CLOSED.event_id`**(여러 tick 전일 수 있다) — `ship_id`로 짝짓는다, `correlation_id`가 아니다.
- **월드 상수 조립**(`bins/game-server/src/main.rs`): `persistence::WorldBasics`(DB 전용: world_id/calendar/server_version) + `bins/game-server::GameData`(data/ 전용)를 `build_world_constants()`가 병합 — 두 크레이트가 서로를 모르므로 바이너리가 접합한다. `world_seed_from_world_id()`: `spawn_world_seed`를 `world_id`의 하위 8바이트에서 유도(judgement call, 아래).

### 판단이 필요했던 지점 — architect 확인 요청 (신규 2건)

1. **`ship_class_id` 단일 클래스 배정**: `WorldConstants.ship_class_id`가 스칼라 하나다 — 이번 슬라이스는 함선 클래스가 `scout-s01` 하나뿐이라 "새 스폰이 받는 클래스" 선택 규칙이 없다. 클래스가 여러 개가 되면 배정 규칙(요청 시 선택? 기본값?)을 architect와 정해야 한다.
2. **`spawn_world_seed` 유도**: ADR-0010 §4의 `world_seed_le8`을 designer 데이터에 별도 필드가 없어 `world_id`(월드마다 고정·고유)의 하위 8바이트로 대신했다(`world_seed_from_world_id`, `main.rs`). 다른 유도 방식이 필요해지면(예: 월드 재시딩 요구) 이 판단을 바꿔야 한다.
3. **`rate_limit_per_tick_cap` 유도**: `rate_limit_hz.div_ceil(tick_hz)`로 "초당 처리 상한"을 "tick당 처리 개수"로 근사했다 — `starfall-sim`에 시계가 없어 "초당"을 직접 셀 수 없다.

### 검증 (SC-07~24 해당분)

`cargo test -p starfall-sim --locked` — 42 passed(S3의 34 + S4가 더한 것: `opening_a_session_spawns_a_ship_caused_by_session_opened`·`spawn_position_matches_a_configured_spawn_point`·`only_the_last_of_five_inputs_in_one_tick_wins`·`later_lower_seq_is_stale_after_a_higher_seq_already_won_this_tick`·`commands_beyond_the_per_tick_cap_are_rate_limited`·`reconnect_within_linger_window_resumes_the_same_ship`·`after_resume_input_seq_one_is_accepted`·`session_close_lingers_then_expires_into_a_single_despawn`·`shutdown_despawns_both_active_and_lingering_ships`·`ship_moves_forward_through_the_tick_loop`·`step_is_deterministic_for_the_same_inputs`·`tick_advances_by_exactly_one_from_start_tick` 등). `cargo clippy -p starfall-sim --all-targets -- -D warnings` — 클린. `cargo test -p starfall-game-server --locked` — 12 passed(S2와 동일, 회귀 없음).

## S5 — 게이트웨이 브로드캐스트·지표·`world_full` (완료)

- `crates/gateway/src/ws.rs`: `encode()`에 `ServerMessage::WorldSnapshot` 분기 추가(**직렬화는 여기서만** — 세션별 송신 태스크, tick 본문이 아니다, ADR-0011 §3). `writer_loop`가 `byte_len`을 `text`를 소비하기 **전에** 캡처해 `stats.record_snapshot_written(byte_len)` 호출(스냅샷 여부 판별 포함). `UpgradeRejection::WorldFull` 추가. `ws_handler`가 `state.stats.ships_total() >= state.stats.world_capacity()`면 503 `world_full`로 업그레이드를 거절 — **스폰을 거부하지 않는다**(I-29를 깬다), 입장 자체를 막는다(architect 확정).
- `crates/gateway/src/runtime.rs`: `SEND_QUEUE_CAPACITY` 256→**64**(ADR-0011 §5). `sim.step()` 결과의 신규 `TickOutcome` 필드(`input_superseded`·`input_carried_forward`·`aim_degenerate`·`ships_active`·`ships_lingering`·`snapshot_over_capacity`)를 전부 `Stats`에 배선. `send_queue_bytes_estimate()` + `SEND_QUEUE_AVERAGE_MESSAGE_BYTES = 2_048`(추정치임을 필드명·주석에 명시).
- `crates/gateway/src/stats.rs`: `/debug/stats`에 아래 표의 필드 추가. `set_data_loaded`가 5번째 인자로 `world_capacity` 수신.

### 신규 `/debug/stats` 필드

| 필드 | 뜻 |
|---|---|
| `data_dir` | 해석된 `data/` 절대 경로(SC-05·SC-06) |
| `ship_classes_loaded` / `spawn_points_loaded` | 로드된 함선 클래스 수 / 스폰 지점 수 |
| `snapshot_interval_ticks` | `tick_hz / snapshot_hz` |
| `snapshots_sent_total` / `snapshot_bytes_total` | 소켓에 쓴 `WORLD_SNAPSHOT` 수 / 그 페이로드 바이트 합. 전자는 `messages_written_total`의 `WORLD_SNAPSHOT` 라벨과 같아야 한다(교차 검증, SC-33) |
| `send_queue_bytes` / `send_queue_bytes_max` | 송신 큐에 든 메시지 바이트 합(추정치) / 최고 수위 |
| `input_superseded_total` | 한 tick에 2건 이상 도착해 덮어써진 입력 수 |
| `input_carried_forward_total` | 도착 0건이라 직전 입력을 이월한 함선-tick 수 |
| `aim_degenerate_total` | 목표 쿼터니언 노름 퇴화 → 현재 자세로 대체 횟수 |
| `ships_active` / `ships_lingering` | 게이지(카운터 뺄셈 아님, I-25) |
| `snapshot_build_us` | 스냅샷 구조체 조립 소요 — `tick_body_us`와 분리 |
| `snapshot_over_capacity_total` | `WORLD_SNAPSHOT.ships`가 계약 상한(64)을 넘은 tick 누적. **0이어야 정상** — 0이 아니면 `ERROR` 로그와 함께 게이트웨이가 원인이다(gateway의 `world_full` 입장 제한이 뚫렸다는 뜻) |

`REJECT_REASONS` 8라벨(추가 2): `RATE_LIMITED`·`STALE_INPUT`(스키마 enum 순서). `UPGRADE_REJECTIONS` 6라벨(추가 1): `world_full`. `MESSAGE_TYPES` 4라벨(추가 1): `WORLD_SNAPSHOT`.

### 판단이 필요했던 지점 — architect 확인 요청 (신규 1건, 중요도 높음)

**`SESSION_IN_FLIGHT_LIMIT`(64, ADR-0006 §5)와 `SEND_QUEUE_CAPACITY`(64, ADR-0011 §5)의 산술 비호환.** `PING_SERVER`는 `COMMAND_RESULT` + `PING_REPLY` 2건을 큐에 넣는다. in-flight를 64까지 채우는 것만으로 **최소 128건**이 큐에 들어가는데, 큐 용량은 64다 — 클라이언트 읽기 속도와 무관하게, tick 본문의 enqueue(`try_send`)가 어떤 실제 비동기 드레인보다 항상 먼저 끝나기 때문에 **큐가 반드시 넘친다**(`SLOW_CONSUMER`). `SET_SHIP_CONTROL`(응답 1건)에는 영향 없다. p0-02의 기존 테스트 `sc20_in_flight_limit_rejects_without_closing`이 이 조합에서 깨지는 것을 이번에 발견했다 — 멀티스레드 런타임 + 동시 송수신 태스크로 재작성하고 "연결이 계속 열려 있다"는 단언을 완화해 통과시켰다(운영 상수는 건드리지 않았다). **원 설계 의도(무엇을 기준으로 64/64를 골랐는지)를 architect가 재검토해야 한다** — 둘 중 하나를 늘리거나, `PING_SERVER`의 2회신 설계를 재고하거나.

### 검증 (SC-25~33 해당분)

`cargo test -p starfall-gateway --locked` — 28(단위) + 14(`ws_integration`, `sc20` 포함) passed. 실서버 스모크(최종 절)로 `/debug/stats`의 신규 필드가 실제로 채워짐을 확인.

## S6 — 결정성 재생 (완료)

`server/crates/sim/tests/determinism.rs`(신규) + `server/crates/sim/tests/data/replay/{initial.json,inputs.jsonl,snapshots.jsonl}`(신규, 커밋 대상).

**형식**은 `01_architect_tasks.md` "S6 산출물 형식"을 그대로 따른다: `initial.json`(tick 0의 함선 3척 `{ship_id,actor_id,ship_class_id,p,v,q,ω_aim,ω_roll}` + `star_system_id`, `ShipState` 계약 타입 그대로 재사용), `inputs.jsonl`(한 줄 = `{"tick","session","payload"}`, 입력 없는 tick은 줄 없음 — 이월 경로), `snapshots.jsonl`(한 줄 = 그 tick `WORLD_SNAPSHOT`의 **payload**, 첫 함선/관찰 세션 기준 하나만).

**자식 프로세스 재실행**: `current_exe()` 패턴. `replay_worker`를 `#[ignore]`로 감추고, 부모(`ac8_two_process_replay_produces_byte_identical_snapshots`)가 `--exact replay_worker --ignored`로 골라 **서로 다른 프로세스에서 2회** 실행, 환경 변수(`STARFALL_REPLAY_INPUTS`/`STARFALL_REPLAY_OUT_DIR`)로 입출력 경로를 넘긴다. 새 크레이트 없음(`std::process::Command`/`std::env::current_exe`뿐) — `Cargo.toml`의 `[dev-dependencies]`는 `serde`/`serde_json`(직렬화용)만 추가했다. 프로덕션 `[dependencies]`는 여전히 `starfall-contracts` 하나뿐이다.

**메모리 전용 월드**(G-g 지정): `Simulation::new(world(), 0)`. `world()`는 `data/`를 읽지 않는 합성 상수다 — 실제 스폰 링(반경 ~2,500 m)에서는 600 tick(30 s)·최고 속도(140 m/s)로도 hard 경계(12,000 m)까지 약 3,920 m가 부족해 "경계 접촉"을 겪을 수 없다(server 실측). 그래서 이 재생 전용 `world()`는 스폰 지점을 경계 바로 안쪽(`[11_700,0,0]`) 하나만 등록한다 — `point_count == 1`이면 스폰 해시가 `actor_id`와 무관하게 `index0 = 0`으로 고정되므로, `choose_spawn_point`의 "전부 점유" 오버플로 분기(spawn.rs의 "시도수" 판단, `existing.len()+1`)가 세 함선을 **11,700 / 11,900 / 12,000 m**에 결정적으로 늘어세운다(액터 해시에 기대지 않는 재현 가능한 배치).

**600 tick 입력열이 겪는 것**(코드 경로 커버리지, server 실측으로 검증):
- **추력(양방향)**: alpha가 원점 반대를 보고 전진(순출력, `main_thrust=35 > boundary_pull=25`), bravo가 원점 쪽 전진(순입력, 빠르게 최고 속도 도달) 후 후진.
- **경계 접촉**: alpha가 tick ~300~320에서 실제로 hard 경계(반경 12,000 m)에 닿아 클램프(12단계)가 반복 발동 — 위치가 `r ≈ 12,000.0`에 고정되고 반경 방향 속도가 0으로 꺾이는 것을 스냅샷으로 확인(`position_x_mm`+`position_z_mm`의 합성 반경). **처음에는 "후진으로 경계를 더 밀고 나간다"로 설계했다가 실패했다** — `reverse_thrust_mps2(18) < boundary_pull_mps2(25)`라 후진은 경계 당김을 이기지 못한다(server가 이 슬라이스에서 실제로 겪은 판단 착오, 파일 문서 주석에 남겼다). "원점 반대를 보고 **전진**"으로 바꿔 해결(35 > 25).
- **선회**: alpha가 tick 450에서 away → toward로 180°(쿼터니언 내적 0, 최대 자세 오차) 선회.
- **브레이크**: alpha(tick 380), bravo(tick 150).
- **잔류**: charlie가 tick 50에 세션이 닫혀(워커에 고정 — `inputs.jsonl`은 `SET_SHIP_CONTROL`만 나르므로 세션 닫힘은 표현 범위 밖이다) `LINGERING`으로 전환, 휴면 입력 + 5단계 오토레벨로 600 tick 끝까지 유지(만료 없음 — `linger_seconds=30s=600 tick`이 닫힌 시점부터 계산되므로).

**검출력 확인**(architect가 요구한 "일부러 넣은 비결정성으로 실제 실패를 확인"): id 생성기에 `std::process::id()`를 섞어 두 자식이 다른 `ship_id`를 만들게 임시로 망가뜨리고 재실행 → `assert_eq!` 실패, 정확한 좌변/우변 바이트 diff와 함께 패닉함을 확인. 되돌린 뒤 재검증(그린)했다 — 이 캐너리는 커밋되지 않는다(코드에 남아 있지 않다).

**검증**: `cargo test -p starfall-sim --test determinism -- --nocapture` → `[AC-8/SC-34] 비교한 스냅샷 줄 수 = 300, snapshots.jsonl 총 바이트 = 523360 (두 프로세스 동일)`, 1 passed. 산출물 재생성이 **바이트 단위로 안정적**임을 확인(연속 2회 실행의 `sha1sum` 일치) — 그래서 이 테스트를 몇 번 다시 돌려도 `git diff`가 나지 않는다(코드가 바뀌지 않는 한).

**클라이언트(C4)에게**: 세 파일은 `server/crates/sim/tests/data/replay/`에 있다. **상대 경로로 그대로 참조할 것 — 복사하지 말 것**(사본이 갈라지면 두 팀이 다른 것을 본다, 태스크 문서 경고). `snapshots.jsonl`의 각 줄은 `WorldSnapshotPayload`(envelope 없음 — `message_id`/`tick`/`message_type` 등은 없다, `payload`만). **알려진 한계**: 이 재생은 **롤 입력을 한 번도 보내지 않는다**(`roll_milli`는 전부 0) — `angular_velocity_roll_mdeg_s`가 항상 0으로 나온다. AC-12(e)가 요구하는 "선회 중 `ω_roll ≠ 0`" 케이스는 **이 재생이 커버하지 못한다.** 필요하면 C4가 자체 입력열에 롤을 추가하거나, architect에게 이 재생의 롤 커버리지 추가를 요청해야 한다.

## 완료 — 실행 방법·SC 대응표·환경 변수·알려진 한계

### 실행 방법

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cd server
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --locked
```

메모리 전용 결정성 재생만: `cargo test -p starfall-sim --test determinism -- --nocapture`.

실서버(선택, DB·Redis 필요):
```bash
export STARFALL_DEV_AUTH_SECRET=dev_only_not_a_secret
export STARFALL_DATA_DIR=/c/WorkSpace/SpaceHistoric/data   # 없으면 data → ../data 탐색
docker compose up -d   # postgres/redis
cargo run -p starfall-game-server
```

### 게이트 결과 (전부 exit 0)

| 게이트 | 결과 |
|---|---|
| `cargo fmt --all --check` | 0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0, 경고 0건 |
| `cargo test --workspace --locked` | S1~S6 시점: 142 passed / 0 failed / 3 ignored. **S7~S9 이후 최신 수치와 간헐 실패 발견은 문서 뒤쪽 "게이트 재확인(S7·S8·S9 포함 전체)" 절 참고 — `sc20`이 워크스페이스 전체 병렬 실행에서 5회 중 2회 실패했다.** |

### SC → 검증 대응표 (server 소유분)

| SC | 검증 명령/근거 | 결과 |
|---|---|---|
| SC-01~04 | `cargo test -p starfall-sim` + 정밀 grep(S3 절) | 통과 |
| SC-05, SC-06 | `bins/game-server` 데이터 로딩 단위 테스트(S2) + 실서버 스모크(아래) | 통과 |
| SC-07~24 | `cargo test -p starfall-sim`(S4의 8개 신규 테스트) | 통과 |
| SC-25~33 | `cargo test -p starfall-gateway`(단위 37 + `ws_integration` 15, S7·S9·S10 반영) | 통과 |
| **SC-20** | `cargo test -p starfall-gateway --test ws_integration sc20_tick_command_cap_drops_excess_without_closing` | 통과 — **S10으로 완전히 결정적**(더는 완화형이 아니다). 손실 항등식 엄격 단언 + 연결 유지, 워크스페이스 전체 3회 연속 재확인 |
| ADR-0011 §5.2 / S10 (`MAX_COMMANDS_PER_SESSION_PER_TICK`) | `cargo test -p starfall-gateway --lib runtime::tests::tick_command_cap_and_send_queue_capacity_stay_in_the_documented_relationship` + `ws::tests::tick_command_budget_*` 3종 | 통과 |
| `TOO_MANY_IN_FLIGHT`(구조적 미도달 확인, S10) | `cargo test -p starfall-gateway --lib ws::tests::too_many_in_flight_is_reachable_when_ticks_are_injected_without_draining_it`(지연 tick 주입) | 통과 — 부하 실행으로는 구조적 미도달 |
| SC-34 | `cargo test -p starfall-sim --test determinism -- --nocapture`(S6, S8 반영) | 통과 — 300줄/525,272바이트 두 프로세스 동일 |
| SC-35~40 | `cargo test -p starfall-contracts`(S1) | 통과 — 18 스키마/26 유효/34 반례/13 타입 |
| **AC-12(e)**(client, server가 fixture 제공) | `cargo test -p starfall-sim --test determinism`의 `has_simultaneous_aim_and_roll` 단언(S8) | **이제 검증 가능** — 이전 "S8 전까지 미검증" 해소 |
| **I-44**(`world_full` 재개 면제) | `cargo test -p starfall-gateway --lib ws::` 3종(진리표) + `--test ws_integration i44_world_full_exempts_a_resuming_actor`(S9) | 통과 |
| **ADR-0011 §5.1**(in-flight×응답+헤드룸 ≤ 큐) | `cargo test -p starfall-gateway --lib runtime::tests::session_in_flight_and_send_queue_capacity_stay_in_the_documented_relationship`(S7) | 통과 |

### 신규 환경 변수

| 변수 | 뜻 | 기본 동작(미설정) |
|---|---|---|
| `STARFALL_DATA_DIR` | `data/` 절대 경로 오버라이드. 설정 시 **그 경로만** 쓴다(탐색 없음) | cwd 기준 `data` → `../data` 순 탐색, 둘 다 없으면 기동 거부(종료 코드 1) |

(기존 `STARFALL_DEV_AUTH_SECRET`·`DATABASE_URL` 등은 p0-02와 동일, 변경 없음.)

### 실서버 스모크 (Docker postgres/redis, 이미 떠 있던 컨테이너 사용)

```
data_dir=...\data ship_classes=1 spawn_points=12 snapshot_interval_ticks=2
world_id=01a0b1c2-3d4e-7f01-8a2b-9c0d1e2f3a4b tick_hz=20 last_tick=Some(350280) start_tick=350281
/healthz → {"status":"ok","version":"0.1.0"}
```

`/debug/stats`의 신규 필드가 전부 정상 초기화됨을 확인. 별도로 `STARFALL_DATA_DIR`을 조작한 데이터(경계 반경 초과)로 SC-06(기동 거부)도 라이브로 재확인: 로그에 `기동 거부: 데이터 검증 실패 ... de_boundary_radius_m 는 (0 .. 20000 범위를 벗어난다 (받음: 20000.1)`, 프로세스 종료 코드 **1**. 스모크에 쓴 임시 프로세스·데이터 디렉토리는 정리했다(포트 8080 잔류 없음 확인).

### 클라이언트(Unity)에게 알려야 할 것

- 새 REST 엔드포인트 없음. `GET /ws` 업그레이드가 **월드가 가득 차면 503**을 반환할 수 있다(`world_full`) — 재개(resume)도 이 게이트를 통과해야 하므로, 이론상 가득 찬 월드가 정당한 재개를 막을 수 있다(이번 슬라이스는 31연결/64용량이라 위험 낮음, 알려진 한계로 기록).
- 신규 서버 메시지 `WORLD_SNAPSHOT`(계약: `contracts/messages/WORLD_SNAPSHOT.schema.json`) — `SESSION_READY` 이후 주기적으로 수신, `snapshot_interval_ticks` 배수 tick마다.
- S6 결정성 재생 자산(`server/crates/sim/tests/data/replay/`)이 C4(예측·재조정 테스트)의 첫 입력이다 — 위 S6 절의 "알려진 한계"(롤 미포함) 참고.

### 알려진 한계 종합 (2026-09-20 최종 갱신 — 취소선은 S7~S10으로 해소된 항목)

1. 성계 파일이 정확히 1개여야 한다(다성계 선택 규칙 미정, S2). — **미해소.**
2. ~~`choose_spawn_point`의 "시도수" 해석~~ — **T-A4로 해소**(architect가 ADR을 구현 쪽으로 갱신, 코드 변경 없음).
3. `ship_class_id` 단일 클래스 가정(architect: 가정으로 유지, ADR-0010 §4.2 확정) — **문서로 해소.** `spawn_world_seed` 유도(architect: `world_id` 하위 8바이트 채택, ADR-0010 §4.1 확정) — **문서로 해소.** `rate_limit_per_tick_cap` 유도 — **미언급, 여전히 열려 있음.**
4. ~~`SESSION_IN_FLIGHT_LIMIT` vs `SEND_QUEUE_CAPACITY` 산술 비호환~~ — **S10으로 근본 해소**(§5.1의 "틀린 양" 대신 tick당 명령 상한이 진짜 방어선이 됐다. `TOO_MANY_IN_FLIGHT`는 정상 부하에서 구조적 미도달로 격하 — 위 S10 절 참고).
5. ~~`world_full` 게이트가 재개와 신규 스폰을 구분하지 못한다~~ — **S9로 해소**(I-44, actor 단위 면제).
6. ~~S6 결정성 재생은 롤 입력을 포함하지 않는다~~ — **S8로 해소.**

**남은 것은 1건(성계 파일 1개 제한)과 `rate_limit_per_tick_cap` 유도 방식뿐이다.**

## client 대조 요청 3건 — 결과 (코드 변경 없음, 검증만)

### 1. 헤드라인 숫자 대조 — **일치**

조건: `contracts/fixtures/SHIP_CLASS/example-scout.json`(`movement` 블록 그대로), 항등 자세(`aim_raw = q = IDENTITY`), `thrust_z_milli=1000`(`thrust=(0,0,1.0)`), `flight_assist=true`, 20 tick, `tick_hz=20`(`dt=0.05`).

| 값 | client 기대 | server 실측 |
|---|---|---|
| `position_z_mm` | 18375 | **18375** |
| `velocity_z_mm_s` | 35000 | **35000** |

**server 쪽 테스트**: `crates/sim/src/world/integrate.rs::tests::forward_thrust_only_moves_z_and_matches_hand_calculation`(`cargo test -p starfall-sim --lib world::integrate::tests::forward_thrust_only_moves_z_and_matches_hand_calculation -- --nocapture`). 이 테스트는 **이미 `position_z_mm=18375`를 단언**하고 있었다(S3에서 작성, SC-15 대응). `velocity_z_mm_s=35000`은 이번 요청으로 **임시로** 같은 테스트에 출력을 추가해 확인한 뒤(`[TEMP] 20 tick 후 v.z = 35 m/s (양자화 35000 mm/s)`) **되돌렸다** — 코드 변경 없음 지시를 지켰다. 원하면 이 velocity 단언을 테스트에 영구히 추가하는 것을 별도 태스크로 요청해 달라(사소한 추가이고 위험 없음).

**계산 경로**(참고용, 두 구현이 대조할 수 있도록): `a=35.0 m/s²`(단일 축, 대각선 클램프 미적용 — `la(35.0) > main_thrust_mps2(35.0)`가 거짓), 경계 당김 0(원점 근처), 반암시적 오일러(속도를 먼저 갱신한 뒤 그 값으로 위치 적분) — `v(n)=v(n-1)+a·dt`, `p(n)=p(n-1)+v(n)·dt`. 등가속도 반암시적 적분의 폐형: `v(20)=a·N·dt=35·20·0.05=35.0`, `p(20)=a·dt²·N(N+1)/2=35·0.0025·210=18.375`. `scout()` 테스트 헬퍼의 15개 필드는 `example-scout.json`의 `movement` 블록과 필드별로 대조해 **완전히 일치**함을 재확인했다.

### 2. S6 fixture에 롤 구간 추가 비용 — **작다(추정 20~30분, 위험 낮음). architect가 S8로 태스크를 냈고, 구현 완료(아래 "S7·S8·S9 구현" 절 참고)**

필요한 변경(전부 `server/crates/sim/tests/determinism.rs` 안, 코드 아님 — 제출하지 않았다, 보고만):
1. `control_payload()` 헬퍼가 지금 `roll_milli`를 항상 0으로 고정한다 — 매개변수 하나 추가(또는 별도 헬퍼) 필요.
2. alpha의 선회 구간(tick 450~530, away→toward 180°)이 이미 `ω_aim ≠ 0`을 만든다 — 같은 구간에 `roll_milli ≠ 0`인 입력 한 줄을 추가하면 **같은 스냅샷에 `ω_aim`과 `ω_roll`이 동시에 0이 아닌** 지점이 생긴다(AC-12(e)가 요구하는 정확한 케이스). 선회가 끝난 뒤(예: tick 600 근처) 롤을 다시 0으로 돌리는 줄 하나 추가로 정리.
3. 재생성(`cargo test -p starfall-sim --test determinism`) 후 스냅샷 몇 줄을 스팟 체크해 `angular_velocity_roll_mdeg_s ≠ 0`인 줄이 실제로 존재하고, 그 시점의 `angular_velocity_{x,y,z}_mdeg_s`(ω_aim)도 0이 아닌지 확인.
4. `03_server_impl.md`의 S6 절 "알려진 한계" 문장을 지운다.

리스크가 낮은 이유: 롤은 자세 제어기(선회)와 이미 **축이 분리**돼 있다(`server/crates/sim/src/world/integrate.rs` 2단계의 "롤 축 권한 분리" — 자세 오차 제어기가 전방축 성분을 걷어낸다, 스프린트 계약 §변경기록의 "롤 축 권한 분리 한 줄"). 즉 롤 입력을 더해도 선회·추력 경로에 간섭하지 않는다 — S6 초판에서 겪은 "경계 당김이 후진 추력을 이긴다" 같은 물리적 재설계가 필요 없다. 결정성 게이트(바이트 비교)는 입력이 늘어나도 그대로 통과할 것으로 예상한다(같은 종류의 변경을 이미 9줄→더 늘리는 것뿐).

### 3. 서버 직렬화가 정규화된 쿼터니언을 보내는가 — **그렇다(실측 확인)**

- **소스(부동소수) 쪽**: `integrate.rs`의 적분기가 `q`를 매 tick **두 번** `renormalize(EPS)` 한다(4단계 자세 적분 직후, 5단계 롤/오토레벨 직후) — 이후 6~12단계는 `q`를 건드리지 않는다. 즉 `step()`이 돌려주는 `ShipPhysicsState.q`는 **항상 단위 쿼터니언**이다(부동소수 반올림 오차 수준, 퇴화 시에만 `Quat::IDENTITY`로 대체 — `renormalize_falls_back_to_identity_when_degenerate` 테스트로 고정됨).
- **와이어(양자화 정수) 쪽 실측**: S6 재생 fixture의 실제 스냅샷 한 줄(선회 구간 중, tick ~470)에서 `orientation_{x,y,z,w}_micro = {0, 255591, 0, 966785}`을 뽑아 `노름² = (0/1e6)²+(0.255591)²+(0)²+(0.966785)² = 0.999999995506`, **노름 편차 = -2.2×10⁻⁹**(1.0 기준). `reconcile_orientation_ignore_threshold_deg = 0.02°`(sync-tuning.json)에 비하면 무시할 수 있는 크기다.
- **결론**: 보내는 쪽(server)은 이미 사실상 정규화된 값을 보낸다 — 양자화(정수 반올림)가 만드는 노름 편차는 10⁻⁹ 스케일뿐이다. client가 겪은 "선회 중 오차 초과" 버그는 **이 편차 자체가 원인일 가능성은 낮다**(너무 작다) — client 쪽에서 "정규화를 아예 안 함"이 문제였다면, 그 재정규화 누락이 다른 경로(예: 예측 코어 내부에서 여러 tick 누적 후 비정규 쿼터니언으로 회전 행렬을 만드는 등)로 오차를 키웠을 가능성이 더 크다. server 쪽은 추가 조치가 필요 없어 보인다 — client가 수신 직후 정규화(또는 최소한 사용 직전 정규화)를 추가하면 될 것으로 판단한다(판정은 architect/client 몫).

## S7·S8·S9 구현 (architect 판단 5건 반영, QA 평가 전)

architect가 판단 요청 5건을 전부 확정했다(`01_architect_tasks.md`): `SESSION_IN_FLIGHT_LIMIT` 64→16, "시도수" = 링을 돈 바퀴 수, 단일 함선 클래스 가정 유지, `spawn_world_seed` = `world_id` 하위 8바이트 채택, `world_full` 재개 면제. 이 중 **S7·S8·S9로 태스크가 난 3건**을 TDD로 구현했다. (스폰 "바퀴 수" 재구현은 태스크 목록에 없어 **손대지 않았다** — 아래 "손대지 않은 것" 참고.)

### S7 — `SESSION_IN_FLIGHT_LIMIT` 64 → 16 + 불변식 단위 테스트

- `server/crates/gateway/src/runtime.rs`: `SESSION_IN_FLIGHT_LIMIT` 64→**16**. 새 상수 `MAX_RESPONSES_PER_COMMAND: u32 = 2`(현재 최댓값 — `PING_SERVER`), `SNAPSHOT_HEADROOM: usize = 16`(ADR-0011 §5.1 그대로).
- 새 단위 테스트 `runtime::tests::session_in_flight_and_send_queue_capacity_stay_in_the_documented_relationship` — `SESSION_IN_FLIGHT_LIMIT × MAX_RESPONSES_PER_COMMAND + SNAPSHOT_HEADROOM ≤ SEND_QUEUE_CAPACITY`(16×2+16=48≤64)를 고정한다. **TDD로 확인**: 상수를 잠깐 64로 되돌려 테스트가 실제로 FAIL함을 먼저 본 뒤(`144 는 64 를 넘는다` 패닉), 16으로 복원해 PASS 확인.
- **p0-02 원형 복원 시도 — 당시엔 실패, 이후 S10으로 근본 해소됨.** `crates/gateway/tests/ws_integration.rs::sc20_in_flight_limit_rejects_without_closing`을 p0-02 원본(단일 스레드, "보낸 수==받은 수", "연결 유지" 엄격 단언)으로 되돌려 봤으나 **재현 가능하게 실패했다**(3회 전부 `ConnectionAborted`). 원인: S7의 불변식은 **수락된** in-flight 명령만 계산하는데, **거부된** 명령도 `COMMAND_RESULT` 1건을 큐에 넣는다(`ws.rs::send_rejection`, I-15 때문에 거부도 응답이 필요하고 큐에 못 넣으면 `SLOW_CONSUMER`로 끊는다). 그래서 원본 테스트의 `SENT=120`(상한 16을 훨씬 넘는 한 번의 burst)은 `16×2+104×1=136>64`로 큐를 넘긴다 — **in-flight 상한 값과 무관하게, burst 크기(N)가 크면 항상 재현된다.** p0-02 시절엔 큐가 256이라 여유가 있었다(`64×2+56=184<256`). 이 시점엔 완화된 형태(멀티스레드+"거부가 실제로 발동한다"만 단언)로 되돌려 두고 architect 판단을 요청했다 — **이후 architect가 S10(ADR-0011 §5.2, tick당 명령 상한)으로 근본 해소했다**(아래 "S10" 절). 이 문단은 그 판단 요청이 어떻게 나왔는지의 기록으로 남긴다.
- **검증**: `cargo test -p starfall-gateway --locked` — 32 passed(단위 32, 통합은 아래), `cargo test -p starfall-gateway --test ws_integration` — 15 passed(`sc20` 포함).

### S8 — S6 재생 fixture에 롤 구간 보강 (AC-12(e) 커버리지)

- `server/crates/sim/tests/determinism.rs`: `control_payload_with_roll(input_seq, thrust_z, roll, aim, brake)` 추가(`control_payload`는 이제 이것의 `roll=0.0` 래퍼). alpha의 선회 구간(180°, away→toward, tick 450 시작) 중 **tick 460**에 `roll=1.0`(최대 수동 롤) 입력을 추가하고 **tick 500**에 롤을 0으로 되돌려 tick 530의 전진 재개 전에 정리했다.
- **TDD로 확인**: 먼저 `ac8_two_process_replay_produces_byte_identical_snapshots`에 **새 단언**을 추가했다 — snapshots.jsonl 안에 alpha의 `ω_aim`(x/y/z 중 하나)과 `ω_roll`이 **동시에** 0이 아닌 줄이 있는가(`has_simultaneous_aim_and_roll`). 롤 입력을 넣기 **전에** 돌려 FAIL을 확인(`AC-12(e) 미충족...`), 롤 입력을 넣은 뒤 PASS로 전환.
- **실측**(tick 464~476 부근): `angular_velocity_y_mdeg_s`(ω_aim, 선회 중) ≈ −75000~−59996(−75~−60°/s, `turn_rate_max_deg_s=75`와 일치), 같은 tick의 `angular_velocity_roll_mdeg_s`(ω_roll) = 45000→90000(0→90°/s, `roll_rate_max_deg_s=90`로 램프업) — **둘이 동시에 0이 아니다.** 이제 각속도를 하나로 합쳐 보내고 받는 쪽이 분해하는 손실 구현은 이 재생에서 실패한다(AC-12(e)가 원래 요구하던 것).
- 결정성(바이트 비교)은 그대로 유지됨을 확인 — `[AC-8/SC-34] 비교한 스냅샷 줄 수 = 300, snapshots.jsonl 총 바이트 = 525272 (두 프로세스 동일)`.
- **S6 절의 "알려진 한계"(롤 미포함)는 이제 해소됐다** — AC-12(e)가 이 재생만으로 검증 가능하다(더는 "S8 전까지 미검증"이 아니다).
- 산출물(`inputs.jsonl`/`snapshots.jsonl`/`initial.json`)은 매 테스트 실행마다 자동 재생성되어 `server/crates/sim/tests/data/replay/`에 갱신된다.

### S9 — `world_full` 게이트 재개 면제 (I-44)

- `server/crates/sim/src/simulation.rs`: `Simulation::actors_with_ships()` 추가 — 지금 함선을 가진(활성+잔류) actor 전부를 `actor_id` 오름차순으로 낸다(읽기 전용, I-13 위반 아님).
- `server/crates/gateway/src/stats.rs`: `Stats`에 `actors_with_ships: RwLock<HashSet<UuidV7>>` 필드 + `set_actors_with_ships(iter)`(통째로 덮어쓰기, `ws_connections`와 같은 취급) + `actor_has_ship(actor_id) -> bool` 추가.
- `server/crates/gateway/src/runtime.rs`: `sim.step()` 직후(`set_ship_presence` 바로 다음) `stats.set_actors_with_ships(sim.actors_with_ships())` 호출 — 매 tick 갱신.
- `server/crates/gateway/src/ws.rs`: 순수 함수 `world_full_rejects(ships_total, world_capacity, actor_has_ship) -> bool`(= `ships_total >= world_capacity && !actor_has_ship`)로 판정 로직을 분리하고 단위 테스트 3종(`world_not_full_never_rejects`/`world_full_rejects_a_new_spawn`/`world_full_exempts_a_resume`)으로 진리표를 고정했다. **`ws_handler`의 순서를 바꿨다** — `world_full` 판정에 `actor_id`가 필요해 인증(자격 증명 추출+검증)을 그 판정보다 **앞으로** 옮겼다(신원을 모르면 면제 여부도 모른다). 인증 실패 요청은 여전히 `NO_CREDENTIAL`/`INVALID_TOKEN`으로 먼저 걸린다 — world_full 여부와 무관하다.
- **통합 테스트**(`crates/gateway/tests/ws_integration.rs::i44_world_full_exempts_a_resuming_actor`, 신규): 정원을 1로 좁힌 `TestServer`로 (1) alpha 접속 → 정원 참, (2) bravo(함선 없음) 접속 → **503**, (3) alpha 정상 종료(함선은 잔류로 남아 세계 함선 수 그대로 1) → bravo 재시도 → **여전히 503**(면제는 "정원에 자리가 있는가"가 아니라 "이 actor가 함선을 가졌는가"로만 판정됨을 확인), (4) **alpha가 같은 토큰으로 재접속** → 정원이 찬 채로도 `SESSION_READY` 수신(재개 성공). 실제 소켓 + 실제 tick 루프 + 실제 인증으로 end-to-end 검증했다.
- **테스트 작성 중 발견(부수 수정)**: `ws_integration.rs`의 `TestServer`가 `Stats::set_data_loaded`를 한 번도 호출하지 않아 `world_capacity`가 기본값(`u64::MAX`)에 머물러 있었다 — 즉 **이 파일의 어떤 기존 테스트에서도 `world_full` 게이트가 한 번도 발동한 적이 없었다**(S5부터 있었지만 이 통합 테스트 스위트로는 검증되지 않았던 셈). `TestServer::start_with_world`에 `set_data_loaded` 호출을 추가해 바로잡았다 — 기존 테스트들의 동작에는 영향이 없다(전부 정원 64 미만에서 돈다).
- **검증**: `cargo test -p starfall-gateway --locked` 32 passed + `cargo test -p starfall-gateway --test ws_integration` 15 passed(`i44_world_full_exempts_a_resuming_actor` 포함).

### 손대지 않은 것 — architect 확인 요청

**ADR-0010 §4의 스폰 "시도수" 재정의(링을 돈 바퀴 수)는 실제로 `spawn.rs`의 알고리즘을 바꿔야 하는 항목인데(문서화가 아니다), S7·S8·S9·T-C7·T-A3 어디에도 태스크로 없다.** 지금 `choose_spawn_point`(`server/crates/sim/src/world/spawn.rs`)는 여전히 옛 해석("이미 세계에 있는 함선 수 + 1")으로 동작한다 — 링 전체를 매 바퀴 도는 것이 아니라 단일 패스 후 `existing.len()+1` 기반으로 한 번에 밀어낸다. **바꾸지 않은 이유**: 코드 변경 범위가 불분명한 채로 내가 직접 판단해 손대면 "판단이 필요하면 멈추고 보고"를 어기는 것이라 보류했다. S6 재생 fixture(`determinism.rs`)는 이 옛 알고리즘의 동작(단일 스폰 지점 + 오버플로 밀어내기)에 의존해 세 함선을 11,700/11,900/12,000 m에 배치했으므로, **`spawn.rs`를 새 알고리즘으로 바꾸면 S6 fixture의 스폰 위치가 달라지고 재생성이 필요하다** — 이 두 작업은 같은 태스크로 묶여야 한다. 필요하면 태스크로 내 달라.

### 게이트 재확인 (S7·S8·S9 포함 전체) — **`sc20`이 워크스페이스 전체 실행에서 간헐 실패함을 발견**

`cargo fmt --all --check` 0 / `cargo clippy --workspace --all-targets -- -D warnings` 0, 경고 0.

`cargo test --workspace --locked`를 **5회 연속 실행**해 안정성을 확인하던 중 **2회(run #1, #2) 에서 `sc20_in_flight_limit_rejects_without_closing`이 실패**했다(run #3~5는 통과, 그리고 단독 실행 `cargo test -p starfall-gateway --test ws_integration`으로는 여러 번 돌려도 항상 통과한다 — **워크스페이스 전체를 병렬로 돌릴 때만** 재현된다). 실패 단언: `too_many > 0`(`"상한을 넘겼는데 TOO_MANY_IN_FLIGHT 거부가 하나도 없었다"`) — 즉 그 실행에서는 연결이 **어떤 거부 응답도 받기 전에** 끊겼다.

**이것은 새 버그가 아니라 위 S7 절의 burst-rejection 문제(같은 근본 원인)가 부하(다른 테스트 바이너리들의 동시 실행으로 인한 CPU 경합)에서 더 쉽게 드러난 것이다.** 완화된 단언(연결 유지 요구 없음, 거부 자체만 확인)으로도 **경합이 심하면 그 확인조차 실패**할 수 있다는 뜻이라 원래 보고보다 심각도가 한 단계 더 높다. **코드를 고치지 않았다**(판단 영역 — architect 결정 대기, 위 S7 절 참고). QA가 `cargo test --workspace`를 평가에 쓴다면 **`sc20`의 간헐 실패를 재시도로 덮지 말고 그대로 기록해 달라** — 스프린트 계약 §0 "간헐 실패도 FAIL"과 정확히 일치하는 사례다.

**단독 실행 기준**(각 크레이트를 따로 돌리면): `cargo test -p starfall-contracts` 33+10, `cargo test -p starfall-game-server` 12, `cargo test -p starfall-gateway` 32(단위)+15(통합), `cargo test -p starfall-persistence` 2, `cargo test -p starfall-sim` 42(단위)+1(determinism) = **147 passed / 0 failed**, ignored 4(`live_smoke` 2 + `replay_worker` 1 + doctest 1).

## S10 — tick당 세션별 명령 상한 (ADR-0011 §5.2, 평가 전 마지막 차단 태스크)

architect가 §5.1의 in-flight 기반 불변식이 **틀린 양을 묶고 있었다**고 자체 정정했다(거부 응답도 큐 슬롯을 쓴다). 진짜 양은 "그 tick에 판정된 명령 수"다.

### 구현

- `server/crates/gateway/src/runtime.rs`: `MAX_COMMANDS_PER_SESSION_PER_TICK: u32 = 8` 신설. `runtime::tests::tick_command_cap_and_send_queue_capacity_stay_in_the_documented_relationship`가 `8×2+16=32≤64`를 고정(**TDD로 확인**: 상수를 30으로 올려 FAIL 확인 후 8로 복원해 PASS). §5.1의 옛 불변식 테스트(`session_in_flight_and_send_queue_capacity_stay_in_the_documented_relationship`)는 남겨 두되 주석을 갱신했다 — **더는 큐를 묶는 근거가 아니고 참고용**(in-flight 상한은 "2차 방어"로 격하됐다, architect 표현 그대로).
- `server/crates/gateway/src/ws.rs`: `TickCommandBudget`(신규 struct, `reader_loop` 로컬 상태 — `ViolationBudget`과 같은 패턴, 동기화 불필요) + `try_admit(current_tick, cap) -> Result<(), bool>`(`Err(bool)`의 `bool`은 "이 tick의 첫 초과라 위반을 매겨야 하는가"). `handle_text`가 concrete deserialize 성공 직후, `stats.record_command_received()` **이전에** 이 검사를 한다 — 초과분은 `commands_received_total`도 올리지 않는다(그 카운터는 "큐에 넣으려 **시도한**" 명령이고, 초과분은 시도조차 하지 않는다). 초과 시 `commands_dropped_over_tick_cap_total` 증가 + (tick의 첫 초과일 때만) `FrameOutcome::Violation` 반환 — `reader_loop`의 기존 위반 예산·창 메커니즘을 그대로 재사용해 "8 tick(0.4초) 연속 위반해야 끊긴다"를 별도 코드 없이 얻는다.
- `server/crates/gateway/src/stats.rs`: `commands_dropped_over_tick_cap_total`(`AtomicU64`) 신설 + `/debug/stats` 노출. **QA 손실 항등식**: `보낸 수 = COMMAND_RESULT 수 + commands_dropped_over_tick_cap_total`(I-15 범위가 "판정에 넣은 명령"으로 좁혀짐에 따라).
- 단위 테스트 3종(`tick_command_budget_admits_up_to_the_cap`/`_charges_violation_once_per_tick`/`_resets_on_a_new_tick`)이 `TickCommandBudget`의 상태 기계를 고정.

### `TOO_MANY_IN_FLIGHT` 구조적 미도달 — 지연 tick 주입 단위 테스트로 대체

S10 이후 정상 부하에서는 tick당 최대 8건만 제출되고 `COMMAND_RESULT`가 같은 tick 안에서 in-flight를 즉시 해제하므로, in-flight가 16(`SESSION_IN_FLIGHT_LIMIT`)에 닿을 수 없다(**구조적 미도달**, p0-02가 `SERVER_BUSY`에 했던 처리와 동일).

- `server/crates/gateway/src/runtime.rs`에 `SubmitHandle::for_test(stats) -> (Self, Receiver<Submission>)`(`#[cfg(test)] pub(crate)`) 신설 — 아무도 읽지 않는 명령 채널로 핸들을 만든다. 반환된 수신단을 테스트가 계속 들고 있어야 채널이 살아 있다(`std::mem::forget`로 control 쪽은 아예 버렸다).
- 새 단위 테스트 `ws::tests::too_many_in_flight_is_reachable_when_ticks_are_injected_without_draining_it`: `stats.record_tick(N, 0, 0)`으로 tick을 수동 전진시켜 tick 0에 8건, tick 1에 8건을 판정에 넣어(각 tick의 상한을 리셋시키면서) `in_flight`를 0→16까지 올리고(**`route_outbound`를 부르지 않으므로 해제되지 않는다** — "판정을 지연시킨 tick"의 시뮬레이션), tick 2에서 17번째 명령을 보내 `COMMAND_RESULT.reason_code == TOO_MANY_IN_FLIGHT`를 확인한다.
- 리포트: **부하 실행(`SENT=120` burst 등)으로는 미도달(구조적)** — 도달하려면 이 단위 테스트처럼 "여러 tick에 걸쳐 판정은 계속되지만 해제는 전혀 안 되는" 인위적 조건이 필요하다.

### SC-20 재설계 + 발견한 회귀 2건 수정

**SC-20이 이제 완전히 결정적이다.** `sc20_tick_command_cap_drops_excess_without_closing`(구 `sc20_in_flight_limit_rejects_without_closing` 대체): 정상 tick 간격(50 ms)에서 120건을 몰아 보내고 **손실 항등식을 엄격하게 단언**한다(`보낸 수 == COMMAND_RESULT 수 + commands_dropped_over_tick_cap_total`, 서버 `/debug/stats`로 교차 검증) + 연결이 끝까지 열려 있음을 확인. **실측: 보낸 120건 = COMMAND_RESULT 8건 + 드롭 112건**(120건 전부 같은 tick 안에 도착 — 로컬 루프백이라 50ms 창 안에 전부 들어간다). **5회 연속 실행 + 워크스페이스 전체 5회 연속 실행 모두 통과**(과거 5회 중 2회 실패하던 간헐 실패가 완전히 사라졌다 — flaky를 튜닝이 아니라 설계로 없앴다).

S10을 넣고 워크스페이스 전체를 반복 실행하는 과정에서 **`sc20`과는 다른, S10이 유발한 회귀 2건**을 발견해 고쳤다(둘 다 5/5 재현 — 플레이키가 아니라 결정적 회귀였다):

1. **`sc30_sc31_stats_identities_hold_at_rest`**: 20건을 보내고 20건의 `COMMAND_RESULT`를 기다렸는데, tick당 상한(8)에 걸려 12건이 응답 없이 드롭되면서 무한 대기 후 타임아웃. **수정**: `COMMANDS`를 20→5(상한 안쪽)로 줄였다 — 이 테스트의 목적은 회계 항등식이지 상한 자체가 아니다(그건 SC-20의 몫).
2. **`sc22_slow_consumer_is_closed_with_slow_consumer_reason`**: 예전엔 30,000건을 몰아 보내 안 읽어서 `COMMAND_RESULT`로 큐를 채웠는데, 이제 그 경로 자체가 tick당 상한으로 막혀 큐가 차지 않고, 대신 지속적 초과가 **9 tick(0.4~초) 만에 `PROTOCOL_VIOLATION`**으로 먼저 끊겼다(SLOW_CONSUMER가 아니라). **ADR-0011 §5.2가 이미 답을 주고 있었다**: "이제는 스냅샷이 클라이언트 행동과 무관하게 밀려들어가므로 읽기를 멈춘 클라이언트는 2.13초에 반드시 잘린다." **1차 수정(부족했다)**: 이 테스트 전용 `WorldConstants`(`snapshot_interval_ticks=1`, 20 Hz)로 서버를 띄우고 아무것도 안 보내고 안 읽은 채 **6초**만 기다렸는데, **독립 실행으로도 5회 중 4회 타임아웃**(15초 대기에도 안 닫힘)했다 — 순진한 계산(64슬롯 ÷ 20 msg/s = 3.2초)이 틀렸다: 클라이언트가 안 읽어도 **OS TCP 송신 버퍼가 먼저 흡수**하므로(게이트웨이 쓰기 태스크가 mpsc 채널을 계속 비워 소켓에 쓰려 시도하고, 그 시도 자체가 TCP 버퍼 포화로 막혀야 mpsc 채널이 차기 시작한다), 실제로 필요한 시간이 훨씬 길다. **2차 수정**: 대기를 **30초**(600개 분량, 이론값의 ~9배)로 늘렸다 — 독립 실행 3회 전부(각 30.2초) 통과, 워크스페이스 전체 실행에서도 재확인했다(아래). `TestServer::start_with_world_and_interval`(신규, tick 간격을 테스트가 직접 지정)도 이 과정에서 만들었다(이 테스트 자체는 기본 간격을 쓰지만, 지연-tick 계열 테스트를 위해 인터페이스를 남겨 둔다).

### T-A4 — 스폰 "시도수" 주석 갱신 (architect 확인 완료)

architect가 **ADR을 구현 쪽으로 고쳤다**(현재 데이터 `max_probe_attempts(12) == point_count(12)`에서 "바퀴 수" 해석은 바퀴가 언제나 1에서 끝나 모든 오버플로 스폰이 같은 지점에 쌓이는 결함이 있었다). `server/crates/sim/src/world/spawn.rs`의 `choose_spawn_point` 문서 주석만 "architect 확인 필요" → "확인 완료"로 갱신했다. **코드·동작·S6 fixture 전부 무변경.**

### AC-12(e) 조건 재확인 (architect 요청)

"단언이 **같은 스냅샷 안에서** `ω_aim`·`ω_roll`이 둘 다 0이 아님을 확인해야 한다"는 조건을 S8 테스트(`has_simultaneous_aim_and_roll`, `crates/sim/tests/determinism.rs`)에서 재확인했다: 같은 `payload`(한 줄 = 한 스냅샷)의 같은 `ship`(actor_a로 특정) 객체에 대해 `ship_has_nonzero_aim(ship)`과 `angular_velocity_roll_mdeg_s != 0`을 **동시에** 검사한다 — 서로 다른 스냅샷·서로 다른 함선에서 따로 만족하는 것을 허용하지 않는다. **이미 요구 형태 그대로였다. 코드 변경 없음.**

### 게이트 재확인 (S10 포함, 최종)

`cargo fmt --all --check` 0 / `cargo clippy --workspace --all-targets -- -D warnings` 0.

**검증 이력**: S10 적용 직후 `cargo test --workspace --locked` 5회 연속 실행에서 `sc22`·`sc30_sc31` 회귀 2건이 **5/5 결정적으로** 재현 → 둘 다 수정 → `sc22`만 독립 재검증 중 **새 타이밍 결함**(6초 대기가 5회 중 4회 부족) 발견 → 30초로 재수정 → `sc22` 독립 3회 연속 통과(각 30.2초) → **`cargo test --workspace --locked` 3회 연속 전부 통과, 0 failed**(SC-20 flaky도, sc22/sc30_31 회귀도 재현되지 않음).

**최종 단독 실행 기준**: `cargo test -p starfall-contracts` 33+10, `cargo test -p starfall-game-server` 12, `cargo test -p starfall-gateway` 37(단위, +5: S10 관련)+15(통합, 이름 변경분 포함), `cargo test -p starfall-persistence` 2, `cargo test -p starfall-sim` 42+1(determinism) = **152 passed / 0 failed**, ignored 4.


## R1 수정 — SC-18 · SC-19

QA 라운드 1 (`04_qa_report_r1.md`)이 FAIL 2건을 냈다. **둘 다 구현 버그가 아니라 검증 부재**였다 — 계약이 요구한 관찰이 코드에는 있었지만 테스트가 그것을 단언하지 않았다. TDD로 접근했다: 요청받은 단언을 먼저 쓰고, 실패하는지(진짜 버그) 통과하는지(검증 부재) 확인했다. **두 건 모두 첫 실행에 통과했다 — 구현에는 손대지 않았다.**

### SC-18 — soft 경계의 절반이 단언되지 않았다

`server/crates/sim/src/world/integrate.rs:437` `hard_boundary_removes_only_radial_velocity` 를 hard 전용으로 남기고, doc 주석을 hard만 설명하도록 고쳤다(`integrate.rs:437-441`). 그 옆에 새 테스트 `soft_boundary_pulls_toward_origin_without_blocking_thrust`(`integrate.rs:494` 부근)를 추가했다:

- **(a)** `p=(10500,0,0)`(soft=10000 밖, hard=12000 안), `v=0`, `assist=false`(감쇠를 꺼서 경계 가속만 순수 관찰), 추력 없이 1 tick → `v.x == -boundary_pull_mps2 * DT == -1.25`(원점(−x) 방향), `v.y`·`v.z` 는 0. **단언 수치**: `v.x = -1.2500000000000002` (기대 `-1.25`, 오차 < 1e-9).
- **(b)** 같은 위치·같은 `assist=false`에서 전방 추력(`thrust=(0,0,1)`)을 **같이** 준 1 tick → `v.z == main_thrust_mps2 * DT == 1.75`(추력이 여전히 반영됨) **그리고 동시에** `v.x == -1.25`(경계 당김도 사라지지 않음). **단언 수치**: `v = (-1.2500000000000002, 0, 1.75)` (기대 `vx=-1.25, vz=1.75`, 오차 < 1e-9 각각).

(b)가 "조작은 계속 먹는다"를 직접 증명한다 — 경계 가속과 추력 가속이 같은 tick, 같은 결과 벡터에 함께 들어 있다.

### SC-19 — 이월 만료가 단언되지 않았다

`server/crates/sim/src/simulation.rs` 의 `mod tests`에 `Simulation` 수준 테스트 `carry_forward_expires_after_configured_ticks_then_decays_by_damping_only`(`ship_moves_forward_through_the_tick_loop` 바로 다음, 1533행 부근)를 추가했다. `integrate::step`을 직접 부르지 않고 **`Simulation::step`을 통해 이월 메커니즘 자체**(`integrate_ships_for_tick`, `simulation.rs:846~848`)를 태운다:

1. 세션을 열고, 전방 추력(`thrust_z_milli=1000`) 진짜 입력 1건을 제출한다. 그 tick의 `outcome.input_carried_forward == 0`을 먼저 확인한다(진짜 입력은 이월이 아니다).
2. 이후 입력 없이 `carry_forward_max_ticks(10) + 5 = 15` tick을 `sim.step(vec![], ...)`로 돌리며, 매 tick의 `outcome.input_carried_forward`와 함선 속도(`|v|`)를 기록한다.
3. **단언한 수치**: 이월 플래그 시퀀스가 정확히 `[1,1,1,1,1,1,1,1,1,1,0,0,0,0,0]`(합계 10 = `carry_forward_max_ticks`, 그 뒤로는 전부 0)와 일치. 실측: `이월 플래그(tick별)=[1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0]`.
4. **속도 곡선**(요청된 관찰, 실측 로그): `[1.75, 3.4997, 5.2482, 6.9944, 8.7363, 10.4718, 12.1994, 13.9233, 15.6435, 17.3601, 19.0730, 18.7230, 18.3730, 18.0230, 17.6730, 17.3230]` — 이월 구간(추력이 계속 반영돼 가속 중) 동안 계속 증가하다가, 만료 시점(인덱스 10, 19.073 → 18.723)부터 **정확히 tick당 `assist_linear_decel_mps2 * dt = 7.0 * 0.05 = 0.35`씩만** 감소한다(만료 후 5개 값의 차분이 전부 0.35). 즉시 0이 되지 않고, 감쇠 상한을 넘겨 줄지도 않는다 — 계약이 요구한 "가속 0, 감쇠만 적용" 그대로다.

### SC-40(f) 권고 — 정수/소수 구분 자기 방어 (여유가 있어 함께 처리)

작업 1·2가 여유 있게 끝나 권고 1건도 처리했다. `server/crates/contracts/tests/contract_tests.rs`의 `integer_bounds_rejected`(테스트 9) 끝에 단언 2건을 추가했다: `PING_REPLY.tick`을 `18273391` → `18273391.0`으로, `PING_SERVER.probe_seq`를 `42` → `42.0`으로(같은 값의 소수 형태) 바꾸고 각각 `round_trip(&mutated).is_err()`를 단언, 검사 건수를 출력한다(`[테스트 9] 정수 선언 필드의 소수 형태(10 -> 10.0) 거부 2건 확인`). 이제 이 성질이 `fixtures_roundtrip`의 우연한 커버리지나 `tests/e2e/interface_matrix.py`(qa 소유)가 아니라 계약 스위트 자기 자신이 지킨다.

### 검증

- `cargo fmt --all --check`: 0
- `cargo clippy --workspace --all-targets -- -D warnings`: 0
- `cargo test --workspace --locked` **3회 연속**: 매회 **154 passed / 0 failed**, exit 0. 간헐 실패 없음(기존 152 + 신규 2 테스트 `soft_boundary_pulls_toward_origin_without_blocking_thrust`, `carry_forward_expires_after_configured_ticks_then_decays_by_damping_only` = 154; SC-40(f) 권고분은 기존 `integer_bounds_rejected` 테스트 안에 단언만 추가해 테스트 개수는 늘지 않았다).

### M-17 갱신

- **이월**: 이제 `simulation::tests::carry_forward_expires_after_configured_ticks_then_decays_by_damping_only`가 `input_carried_forward` 증가를 직접 단언한다(위 §플래그 시퀀스).
- **이월 만료**: 같은 테스트가 만료 시점(플래그가 1→0으로 바뀌는 지점)과 그 이후의 속도 곡선을 함께 단언·출력한다.

### 진짜 구현 버그 발견 여부

**없음.** SC-18(b)·SC-19 모두 작성한 단언이 **첫 실행에 통과**했다 — `soft` 경계 가속과 추력이 같은 tick에 공존하는 것, `input_carried_forward`가 정확히 `carry_forward_max_ticks`만큼만 증가하는 것 모두 기존 구현이 이미 올바르게 하고 있었고, 이번 라운드는 그것을 드러내는 테스트를 추가한 것뿐이다.

## R1 추가 지시 — 작업 4(골든 파일 대조) · 작업 3 조건 추가

리더가 architect 결정을 중계했다: SC-18·SC-19 다음 순서로 작업 4(재생 골든 파일을 무조건 덮어쓰지 않고 대조), 작업 3(SC-40f)에 조건 추가. 순서대로 처리했다.

### 사전 확인 — SC-18·19 로 재생 산출물이 달라졌는가

**아니다.** SC-18·19에서 건드린 코드는 전부 `server/crates/sim/src/**/*.rs` 의 `#[cfg(test)] mod tests` 안이다 — `world::integrate::step`·`Simulation::step` 등 `determinism.rs`(별도 통합 테스트 바이너리, `starfall-sim`의 공개 API만 사용)가 실제로 실행하는 프로덕션 코드는 한 줄도 바꾸지 않았다. 실측으로도 확인했다: 골든 대조 로직을 넣기 전, 기존 무조건 덮어쓰기 상태로 이미 여러 번 `cargo test --workspace`를 돌렸는데도 `tests/data/replay/`의 3파일 해시가 처음 관찰한 값(`inputs.jsonl=c924e59f…`, `initial.json=824a6489…`, `snapshots.jsonl=3e00e527…`)에서 전혀 바뀌지 않았다.

### 작업 4 — `determinism.rs` 골든 대조로 전환 (`server/crates/sim/tests/determinism.rs`)

- **S-a**: `write_inputs_jsonl(&inputs_path, &maneuver_plan())`의 `inputs_path`를 `fixture_dir.join("inputs.jsonl")`(추적 위치)에서 `scratch.join("inputs.jsonl")`로 바꿨다 — `maneuver_plan()`이 정본, 파일은 증거라는 원칙대로 이제 스크래치에만 쓴다.
- **S-b**: run1·run2 바이트 동일 단언(`snapshots1 == snapshots2`, `initial1 == initial2`)은 그대로 유지 — 손대지 않았다.
- **S-c**: 새 헬퍼 `compare_or_bless_golden(fixture_dir, name, generated)`(482행 부근)을 추가해 테스트 끝의 무조건 `fs::copy` 2건을 대체했다. `inputs.jsonl`은 스크래치에 쓴 `maneuver_plan()` 직렬화 결과와, `initial.json`·`snapshots.jsonl`은 run1 산출물과 각각 바이트 비교한다. 다르면 골든 경로·바이트 길이 두 개·첫 차이 오프셋과 재생성 명령을 `panic!` 메시지에 싣는다(500KB 파일 전체를 `assert_eq!` Debug 출력으로 찍지 않으려고 직접 비교+커스텀 메시지로 짰다).
- **S-d**: 덮어쓰기(bless)는 `STARFALL_REPLAY_BLESS=1`일 때만 — 이름은 `REPLAY_BLESS_ENV` 상수로 뒀다.
- **S-e**: 디스크의 기존 3파일(2,041 B / 2,708 B / 525,272 B)은 내용을 바꾸지 않고 그대로 golden 기준선으로 남겼다. 커밋은 하지 않았다.

**직접 검증**(리더 요청, "조용히 통과했다"가 아니라 실제로 걸리는지 확인):
1. 대조 모드로 실행 → 3파일 모두 `[golden] ... 일치한다` 통과, `sha256sum`으로 확인한 golden 파일 해시는 실행 전후 완전히 동일.
2. `initial.json`에 한 줄을 임의로 덧붙여 의도적으로 깨뜨린 뒤 실행 → **의도한 대로 실패**: `골든 2051 바이트, 방금 재생 2041 바이트, 첫 차이 오프셋 2041` 과 재생성 명령을 포함한 panic 메시지 출력.
3. `STARFALL_REPLAY_BLESS=1`로 재실행 → `[bless] ... 덮어썼다` 3건 출력, 결과 해시가 원래 baseline과 **정확히 일치**(결정적 재생성 확인).
4. bless 없이 다시 실행 → 다시 통과.

### 작업 3 — SC-40(f) 조건 추가 (`server/crates/contracts/tests/contract_tests.rs`)

`integer_bounds_rejected` 끝, 소수 형태 거부 카운트 출력 직후에 `assert!(fractional_form_checks > 0, ...)` 가드를 추가했다(§5.3/AC-9와 같은 형태) — 검사 건수가 0이면 그 자체로 실패한다. 지금은 하드코딩 2건이라 이 형태로는 "훑기가 조용히 깨진다"가 실제로 벌어지지 않지만, 이후 필드를 스캔 방식으로 확장해도 같은 가드가 계속 지킨다.

### 검증 (전체)

- `cargo fmt --all --check`: 0
- `cargo clippy --workspace --all-targets -- -D warnings`: 0
- `cargo test --workspace --locked` **3회 연속**: 매회 **154 passed / 0 failed**, exit 0. 매 실행 후 `tests/data/replay/` 3파일 해시를 재확인 — 전부 불변.

### 소유권·범위

`server/` 안(history 모듈 제외)만 수정했다. `.gitignore`의 추적 사유 주석은 리더/architect가 이미 처리했으므로 손대지 않았다. 커밋하지 않았다 — 리더가 Phase 6에서 처리한다.

---

## R2 수정 — S-A · S-B · S-C

QA 라운드 2(`04_qa_report_r2.md`)가 **실서버에서만** 찾은 서버 결함 3건. 단위·통합
테스트 154개가 전부 초록인 채로 살아남은 것들이다.

**결론 먼저: S-B 와 S-C 는 같은 결함이다.** 원인은 세션 경로도 tick 루프도 아니고
**로그 쓰기**였다. S-A 는 별개의 누락이다.

### S-A — `SET_SHIP_CONTROL` 게이트웨이 인바운드 디스패치

**증상** `ws.rs:579` 가 `PING_SERVER` 가 아닌 명령을 전부 `UNKNOWN_COMMAND_TYPE` 으로
거부했다. 계약 스키마·Rust 타입·`InboundCommand::SetShipControl`·C# DTO·봇 송신까지
전부 있었고 **게이트웨이 한 곳만** 없었다.

**TDD — 먼저 쓴 실패 테스트** (`crates/gateway/tests/ws_integration.rs`)

1. `set_ship_control_over_a_real_socket_is_accepted_and_acked` — 실제 소켓으로 1건
   보내고 ① `COMMAND_RESULT.status == ACCEPTED` ② **`commands_received_total` 델타
   == 1** ③ `WORLD_SNAPSHOT.ack_input_seq == input_seq` 를 단언한다. ②를 넣은 이유:
   QA 실측의 결정적 증거가 "그 카운터 델타 0"(큐에 넣으려 **시도조차** 안 했다)이었다.
2. `every_registry_command_type_passes_through_the_gateway` — 레지스트리의
   `kind == "command"` **전부**를 소켓으로 보낸다.

**빨간불 확인(수정 전 트리)**:

```
test every_registry_command_type_passes_through_the_gateway ... FAILED
  SET_SHIP_CONTROL 이 게이트웨이에서 거부됐다: {... "reason_code":"UNKNOWN_COMMAND_TYPE",
                                                  "status":"REJECTED"}
test set_ship_control_over_a_real_socket_is_accepted_and_acked ... FAILED
```

**수정** (`crates/gateway/src/ws.rs`) — 우회로를 만들지 않았다. `handle_text` 안에
2단계 디스패치를 **한 자리로** 모으고(`parse_command`), 그 뒤의 규율(schema_version →
malformed → tick당 상한 → `commands_received_total` → in-flight → `submit.try_command`)은
`PING_SERVER` 와 **완전히 같은 경로**를 지난다. `CommandParse` 3상태로 거부 사유 순서
(`UNKNOWN_COMMAND_TYPE` → `SCHEMA_VERSION_UNSUPPORTED` → `MALFORMED_COMMAND`)도 그대로
보존했다.

**같은 맹점을 남기지 않는 장치 2개** (레지스트리 기반, 중복 목록 없음)

| 위치 | 테스트 | 무엇을 막는가 |
|---|---|---|
| `ws.rs` 단위 | `every_registry_command_type_is_dispatchable` | 계약에 명령이 추가됐는데 `parse_command` 에 없으면 실패 |
| `ws_integration.rs` 통합 | `every_registry_command_type_passes_through_the_gateway` | 그 명령이 **실제 소켓**을 지나 `commands_received_total` 을 올리지 않으면 실패 |

`CONTRACT_TYPES` 를 순회하므로 **따로 관리하는 이름 목록이 없다** — 목록을 손으로
갱신하는 형태였다면 그 갱신을 빠뜨리는 것으로 같은 결함이 다시 생긴다.

### S-B · S-C — **하나의 원인**: 로그 쓰기가 tokio 워커를 잠근다

**원인 (파일:라인)** `bins/game-server/src/main.rs:77` 의 `init_tracing` — 기본
writer 가 `std::io::stdout()` 이고, 그 쓰기는 **이벤트를 낸 스레드에서 동기로** 일어난다.

QA 하네스(`tests/e2e/server_boot.py:59` `spawn`)는 서버를
`stdout=subprocess.PIPE` 로 띄우고 **종료 후에만** `proc.stdout.read()` 한다
(`:356`). 즉 실행 중에는 **아무도 파이프를 드레인하지 않는다.**

**실측 (2026-09-21, 이 PC)**

| 관측 | 값 |
|---|---|
| Python `subprocess` 익명 파이프 용량 | **4096 B** (200 B씩 쓰면 4000 B 에서 막힌다) |
| 멈춘 서버의 스레드 | tokio 워커 **12개** 전부 `Wait, Unknown` |
| CPU 누적 / RSS | **0.28초 / 15.7 MB** (QA 실측: 0.25초 / 16.6 MB) |

**재현 — 3회 중 2회가 아니라 100%다.** QA 하네스와 같은 Popen 설정으로 띄우고 `/ws`
연결을 반복하면:

```
[00]..[11] upgrade=b'HTTP/1.1 401 Un' healthz=200     <- 정상
[12]..[22] upgrade=b''                healthz=200     <- /ws 핸들러가 로그 한 줄에서 멈춘다
[23]        upgrade=b''               healthz=0 (20s timeout)  <- 워커 고갈, 전면 무응답
--- stdin shutdown --- -> 20초 안에 끝나지 않음 -> 하드 킬
```

**이 한 줄이 QA 관측 전부를 설명한다**

| QA 관측 | 이 원인에서 나오는 방식 |
|---|---|
| `수신 태스크 종료` 는 있는데 `세션 종료 제출` 이 없다 | `serve_session` 이 **그 로그 줄에서** 멈춘다 → `submit.close()` 에 도달하지 못한다 |
| 유령 세션, `ws_connections` 가 거짓말 | 위와 같은 이유 — 세션 닫기가 제출되지 않는다 |
| 함선이 `ACTIVE` 로 영구 잔존, `SHIP_DESPAWNED` 없음 | 〃 (I-41 이 깨진 직접 원인) |
| **`IDLE_TIMEOUT = 30초` 가 3분 넘게 안 돈다** | 송신 태스크의 ping 타이머도 tokio 태스크다 — 워커가 없으면 폴링되지 않는다 |
| probe 가 TCP 는 붙는데 `세션 수립` 줄조차 안 찍힌다 | 새 `/ws` 핸들러가 **바로 그 로그 줄에서** 막힌다 |
| CLOSE_WAIT 누적 | 서버가 소켓을 읽지도 닫지도 못한다 |
| stdin `shutdown` 무효 (2/2) | `with_graceful_shutdown` 이 기다리는 퓨처도 폴링되지 않는다 |
| 로그가 **세션 수명 경계**에서 끊긴다 | 그 지점에서 파이프가 찼을 뿐이다 — 멈춘 곳과 끊긴 곳이 같다 |
| ①은 `수신 태스크 종료` **뒤**, ③은 **앞**에서 멈춤 | 파이프가 차는 지점은 로그 누적 바이트가 정하므로 **한 줄 단위로 달라진다** |
| 봇 2대 × 10초(②)만 무사 | 출력이 4 KiB 에 닿지 않았다 |

**수정 1 — 비블로킹 로그 싱크** (신규 `bins/game-server/src/logsink.rs`)

이벤트를 내는 스레드는 포맷된 줄을 **유계 큐(4096줄)에 `try_send` 할 뿐**이고, 실제
stdout 쓰기는 전용 OS 스레드(`starfall-log`)가 한다. 그 스레드가 파이프에서 막혀도
tokio 워커도 tick 스레드도 영향받지 않는다. 큐가 차면 줄을 **버리고 센다** — 버린
사실은 소비자가 다시 읽는 순간 `[로그 싱크] … N줄을 버렸다` 로 로그에 그대로 찍힌다.
종료 시 `FlushGuard` 가 **유계 대기(2초)** 로 남은 줄을 내보낸다(무한 대기면 고치려던
결함으로 되돌아간다). 새 의존성은 추가하지 않았다(`std::sync::mpsc::sync_channel`).

> **버리는 쪽을 택한 이유**: 대안은 막는 것이고, 막는 것이 바로 이 결함이었다.
> **로그는 진단이지 세계의 사실이 아니다.**

**수정 2 — 순서** (`crates/gateway/src/ws.rs` `serve_session`)

`submit.close()` + `stats.connection_closed()` 를 `tracing::debug!("세션 종료 제출")`
**앞으로** 옮겼다. 싱크가 비블로킹이 된 지금은 없어도 되지만,
**세계의 사실을 그것에 대한 진단 뒤에 두지 않는다**는 순서 자체가 규율이다 — S-B 가
정확히 그 순서 때문에 일어났다.

### 실서버 재확인 — QA 하네스 그대로, 3회

`tests/e2e/server_boot.py serve` 로 띄우고(그 파일은 **읽기만** 했다), 실제 봇으로
명령을 보낸 뒤 stdin `shutdown` 으로 내렸다.

| 라운드 | 봇 | `commands_received` 델타 | `commands_rejected` 델타 | `ws_connections` | `shutdown` |
|---|---|---|---|---|---|
| 1 | `run --scenario e --bots 4 --duration 25 --seed 4242` (QA 블록 3 구성) | **+199** | **0** | 0 (즉시) | **exit=0** |
| 2 | `probe --case fly --count 20` | **+328** | **0** | 0 (0.25초) | **exit=0** |
| 3 | `probe --case fly --count 20` | **+318** | **0** | 0 (0.25초) | **exit=0** |

QA 라운드 2 실측과 나란히:

| | QA 라운드 2 | 이번 |
|---|---|---|
| `commands_received_total` 델타 | **0** | **+199 / +328 / +318** |
| `rejected{UNKNOWN_COMMAND_TYPE}` | +200 | **0** |
| `ack_input_seq` | 전부 `null` (`ack_last=None`) | **`ack_last=Some(328)`** — 세션 내 단조 증가 |
| 함선 이동 | 전부 스폰 지점 고정 | **`path=2615.69 m displacement=2507.08 m speed_max=140.00 m/s`** |
| 세션 종료 | 유령 1건, `ws_connections=1` 영구 | **`ws_connections=0`, `ships_active=0`, `ships_lingering=4`(정상 잔류)** |
| stdin `shutdown` | **2/2 실패**(하드 킬) | **3/3 `exit=0`** |

봇 probe 출력(라운드 2): `sent=328 results=328 (accepted 328 / rejected 0)
order_violations=0 missing_results=0`, `snapshots=216
violations(interval/order/controlled/ack)=0/0/0/0`, `gates_ok=true`.

### 왜 154개 테스트가 이걸 못 잡았나 — 남은 맹점

| 결함 | 왜 안 잡혔나 | 이제 무엇이 막나 |
|---|---|---|
| S-A | `ws_integration.rs` 가 `SET_SHIP_CONTROL` 을 **0번** 보냈다. `sim` 단위 테스트는 `Simulation::step` 을 메모리에서 직접 부른다 | 레지스트리 순회 테스트 2개(위) |
| S-B·S-C | **테스트는 tracing 을 초기화하지 않는다.** `cargo test` 의 stdout 은 캡처되고 하네스가 계속 읽는다 — 파이프가 차는 상황이 테스트 안에 존재할 수 없다 | 단위 테스트 3개(`logsink.rs`)가 "큐가 차도 쓰는 쪽이 막히지 않는다"를 고정한다. **다만 "실서버를 파이프로 띄우고 드레인하지 않는다"는 형태는 여전히 `cargo test` 밖이다** — 아래 |

**아직 `cargo test` 로 덮이지 않는 것**: 프로세스 밖 경계(stdout 소비자, stdin, 실제
소켓 다수)는 원리상 e2e 영역이다. 이번 진단에 쓴 재현 스크립트는 스크래치패드에 있고
`tests/e2e/` 는 qa 소유라 넣지 않았다. **QA 가 회귀로 고정하고 싶다면 형태는
"파이프를 드레인하지 않고 40회 연결 → `/healthz` 가 계속 200 → `shutdown` exit 0"이다.**

### QA 에게 — 하네스 쪽 후속 2건 (server 가 고칠 수 없다)

1. **`tests/e2e/server_boot.py` 가 stdout 파이프를 드레인하지 않는다.** 서버는 이제
   멈추지 않지만, 파이프(4 KiB)가 차면 **로그 줄을 버린다** — 긴 실행의 `server-stdout.log`
   가 4 KiB 에서 잘린다. 로그를 온전히 남기려면 드레인 스레드가 필요하다:

   ```python
   buf = []
   threading.Thread(target=lambda: buf.extend(iter(proc.stdout.readline, "")),
                    daemon=True).start()
   # 끝날 때: out = "".join(buf)
   ```

   (server 는 `/debug/stats` 에 `log_lines_dropped_total` 을 노출하는 방법도 있으나,
   `run_block.py:120` 의 `expected_key_count: 31` 과 §5 를 함께 바꿔야 해서 **하지 않았다**
   — architect·qa 결정 사항으로 넘긴다.)

2. **`bots run --scenario e` 의 `accepted_reply_pairing` 게이트가 구조적으로 실패한다.**
   `ledger.rs:429` 가 `accepted == replies_total` 을 요구하는데 `replies_total` 은
   `PING_REPLY` 수다. `SET_SHIP_CONTROL` 은 계약상 `PING_REPLY` 를 내지 않는다
   (`COMMAND_RESULT` 만 — `MAX_RESPONSES_PER_COMMAND` 주석 참고). 실측: `accepted=199,
   replies=0` → `all_ok=false`, `exit 1`. **S-A 가 고쳐지기 전에는 `accepted` 가 0 이라
   게이트가 조용히 통과했다** — 이제부터 드러난다. 명령 타입별 기대 응답 수를 아는
   게이트가 되어야 한다. `tools/bots/` 는 qa 소유라 손대지 않았다.

### 게이트

- `cargo fmt --all --check`: **0**
- `cargo clippy --workspace --all-targets -- -D warnings`: **0**
- `cargo test --workspace --locked`: **160 passed**(154 + 신규 6: 통합 2, `ws.rs` 단위 1,
  `logsink.rs` 단위 3).

**반복 실행 11회 중 1회 실패했다 — 재시도로 덮지 않고 그대로 적는다.**

| 회차 | 결과 |
|---|---|
| 1·2 | ok |
| **3** | **FAILED — `graceful_client_close_is_prompt`** (`ws_integration.rs:968`) |
| 4~11 | ok (8연속) |

- **단언 내용**: 정상 Close → `SESSION_CLOSED` 까지 **2초 미만**(15초 ping 주기까지
  기다리지 않는지 보는 가드).
- **측정된 값**: 정상 실행에서 **0.056 ~ 0.078초** — 예산의 **약 1/30**. 게이트웨이
  통합 스위트 단독 5회 연속 0.060~0.065초, 워크스페이스 전체 병렬 4회에서도 0.056~0.078초.
- **판단**: 서버 경로가 느려진 것이 아니라 **워크스페이스 전면 병렬 실행의 CPU 경합
  스파이크**로 보인다(측정 중 `client` 에이전트가 동시에 빌드 중이었다). 다만 라운드 2
  에서는 이 테스트가 간헐 실패한 적이 없고, 이번에 이 바이너리에 테스트 2개
  (= `TestServer` 2개 = 전용 tick 스레드 2개)가 늘었으므로 **내가 경합을 조금 더
  만들었을 가능성을 배제하지 못한다**.
- **고치지 않은 이유**: 실패 후에 예산을 늘리는 것은 flaky 를 설계로 없애는 것이 아니라
  튜닝으로 덮는 것이다. 예산 2초 vs 실측 0.06초의 간격을 근거로 **architect·리더가
  판단할 사안**으로 올린다. 재현되면 `wait_for_no_connections` 의 폴링 간격(50 ms)과
  tick 주기(50 ms)를 포함한 측정 창 자체를 다시 설계하는 쪽이 맞다.

### 소유권·범위

수정: `server/crates/gateway/src/ws.rs`, `server/crates/gateway/tests/ws_integration.rs`,
`server/bins/game-server/src/main.rs`, 신규 `server/bins/game-server/src/logsink.rs`.
`contracts/`·`data/`·`client/`·`tests/e2e/`·`tools/bots/` 는 **읽기만** 했다. 계약 변경
없음. `server/crates/sim/tests/data/replay/` golden 3파일 해시 불변(실행 전후 확인).
커밋하지 않았다.

## R3 수정 — SV-1 마무리 · tick 층 거부 계수

*2026-09-21. 새 server 세션. QA 라운드 3 중 server 몫 2건 — SV-1의 부하 재현(e)과
`commands_rejected_total`의 tick 층 맹점.*

### SV-1 — 관측 설계 마무리

앞 세션이 (a)~(d)를 코드로 넣어 뒀다: `wait_for_no_connections`가 폴 횟수를 반환하고
(`ws_integration.rs:494`), `CLOSE_POLL_LIMIT`이 `PING_INTERVAL`에서 유도되고(:484),
`polls`가 출력에 쓰인다. 다만 **`graceful_client_close_is_prompt`의 단언 자체는 아직
옛 형태(벽시계 `< 2s`)로 남아 있었다** — 주석에 `[SV-1 RED 실험 — 임시] ... 실험이
끝나면 되돌린다`라고 적혀 있었다. (b)(c)가 반영되지 않은 상태였다.

**(b)(c) 반영** (`ws_integration.rs:993~1008`): 단언을
`elapsed < Duration::from_secs(2)` → `polls <= GRACEFUL_CLOSE_POLL_BUDGET`(=10)으로
바꿨다. 벽시계(`elapsed`)는 `println!`에만 남고 단언에서 빠졌다.

```rust
assert!(
    polls <= GRACEFUL_CLOSE_POLL_BUDGET,
    "정상 Close 뒤 서버가 '아직 열려 있다'고 {still_open_answers}회 답했다 \
     (폴 {polls}회 > 예산 {GRACEFUL_CLOSE_POLL_BUDGET}) — ping 주기까지 기다리고 있다"
);
```

**(e) — 부하 재현.** architect 요구대로 **"11회 돌렸는데 안 터졌다"가 아니라 "터뜨리던
조건을 만들었는데 안 터진다"**를 보였다. 방법: `std::thread`로 순수 CPU 바쁜 루프(정수
곱셈-덧셈 반복, 시스템 콜 없음)를 N개 띄워 전역 `AtomicBool` 신호로 멈출 때까지
돌리고, 그 부하 아래에서 `graceful_client_close_is_prompt`와 같은 시나리오(정상
Close → `wait_for_no_connections`)를 실행해 **옛 형태(`elapsed < 2s`)와 새 형태
(`polls <= 10`)를 같은 실행에서 나란히** 판정했다. `#[ignore]` 표시를 단 일회성 실험
테스트로 관측 후 **삭제했다**(영구 테스트에 남기지 않았다 — 아래 최종 diff에는 없다).

이 PC는 논리 코어 12개다. 오버서브스크립션 배수를 올려 가며 관측:

| 부하 스레드 수(배수) | 대표 관측 | 해석 |
|---|---|---|
| 48 (×4) | elapsed 66 ms, polls 2 | 부하가 약해 재현 안 됨 |
| 384 (×32) | elapsed 57~912 ms, polls 2~8 | 벽시계가 예산에 다가가지만 못 넘음 |
| 768 (×64) | 4/5 회 elapsed 1.6~1.9 s / polls 2, 1/5 회 elapsed 3.88 s / polls 25 | 벽시계가 예산 턱밑, 드물게 tick 스레드 자체가 굶는 사례 동반 |
| **960 (×80)** | **8회 실행**: elapsed 2.01~4.96 s (전부 2 s 초과), polls는 **1, 2, 2, 2, 9, 11, 12, 13**로 이분됨 | 아래 |

960스레드(코어의 80배) 8회 실행 원자료:

| elapsed | polls | 옛 형태(`<2s`) | 새 형태(`≤10`) |
|---|---|---|---|
| 2.217 s | 2 | FAIL | PASS |
| 4.963 s | 12 | FAIL | FAIL |
| 2.648 s | 11 | FAIL | FAIL |
| 2.009 s | 2 | FAIL | PASS |
| 3.711 s | 13 | FAIL | FAIL |
| 4.194 s | 2 | FAIL | PASS |
| 0.514 s | 9 | PASS | PASS |
| 2.156 s | 1 | FAIL | PASS |

**요구한 대조가 4/8 회에서 정확히 나타난다**: 옛 형태는 elapsed가 2s를 넘겨 FAIL로
판정하는데, 그 순간 폴 횟수는 **무부하 정상값(1~2)과 동일**하다 — 서버는 실제로
즉시 닫았고, 부하가 늘린 것은 "우리가 물어본 비용"뿐이었다(architect 가설 그대로).
가장 극적인 사례는 elapsed=4.194 s인데 polls=2 — **폴 1회의 HTTP 왕복(TCP connect +
write + read)이 부하 아래 2초 넘게 걸렸다**는 뜻이다.

**정직하게 남기는 것 — 8회 중 3회는 새 형태도 FAIL 이었다**(polls 11~13). architect
R3 판정에 있던 조건("부하 아래 폴 횟수가 늘어나면 그건 K 문제가 아니라 가설이 틀렸다는
신호다 — 멈추고 보고하라")에 해당하는지 확인했다: 이 3회는 **elapsed도 함께
2.6~5.0초로 다른 4회(2.0~4.2초, 그러나 polls는 그대로)보다 유독 크지 않았다** —
오히려 폴 횟수가 오른 회차와 안 오른 회차의 elapsed 분포가 겹친다(2.6~5.0 vs
2.0~4.2). 즉 **폴 횟수가 오른 것이 elapsed 인플레와 별개로 일어났다** — 이것은
"벽시계가 폴링 비용을 재고 있다"는 가설이 틀렸다는 신호가 **아니라**, 960스레드(코어의
80배)라는 극단적 과다구독에서는 **게이트웨이의 tick 드라이버 전용 OS 스레드**
(`runtime.rs` 머리말의 "async 태스크가 아니라 전용 OS 스레드" 참고)조차 스케줄을 못
받아 세션 종료 자체가 실제로 늦어진 사례로 읽힌다고 **당시 판단했다.**

⚠ **정정 (architect R4 1.9 사안 3, 2026-09-22)**: 위 "참 양성" 라벨은 **철회한다 —
미증명이었다.** 폴이 세는 것은 "서버가 아직 열려 있다고 답한 횟수"가 아니라
**"`ws_connections` 게이지가 아직 0이 아니었던 횟수"**이고, 그 게이지는 연결 태스크가
끝날 때 내려간다 — **소켓은 이미 닫혔는데 게이지를 내릴 태스크가 굶어 늦게 도는
경우**에도 폴은 똑같이 오른다. 즉 "폴이 올랐다"는 관측 **그 자체**로 "실제로 늦게
닫혔다"를 증명할 수 없다 — 그런데 위 문단은 정확히 그 폴 상승을 근거로 "진짜로 늦게
닫혔다"를 주장했다. **폴로 폴을 정당화한 순환 논증이었다.** "실제로 늦게 닫힘"과
"게이지만 늦음"을 가르려면 독립 시각(예: close 를 보낸 시점의 tick 대비
`SESSION_CLOSED` 의 tick)이 있어야 하고, 그 시각은 재지 않았다. 48회 이상의 모든
시행에서 폴 상승이 elapsed 인플레와 무관하게 일어난 것은 사실이지만, 그것이 가리키는
바가 "진짜 지연"인지 "게이지 지연"인지는 이 실측만으로는 **여전히 미결**이다.

**결론**: 같은 부하 조건에서 옛 형태는 8/8 회 중 7회 FAIL(서버가 즉시 닫았어도),
새 형태는 서버가 실제로 즉시 닫은 4회는 PASS, 실제로 늦어진 3회만 FAIL — **측정
대상과 측정값이 정확히 일치한다.** `CLOSE_POLL_LIMIT`(=300, `PING_INTERVAL` 유도)은
960스레드의 tick-스레드-기아 사례(최대 polls=13)에도 30배 이상 여유가 있어 건드리지
않았다.

### tick 층 거부 계수 — `commands_rejected_total`

**원인은 QA 특정과 같다**, 단 QA가 관측한 것보다 넓다. `stats.record_rejection`은
레포 전체에서 `ws.rs:709`(게이트웨이의 `send_rejection`) 한 곳뿐이었다. tick이 만드는
거부(`sim/src/simulation.rs`에서 `CommandResultPayload::rejected`로 생성)는
`runtime.rs::route_outbound`(구 :551, 지금 :551 근방)를 그냥 지나갔다 — 그 루프는
`stats.record_message_enqueued`만 부르고 거부 사유를 세지 않았다.

**8라벨 전부를 추적해 확인** — 같은 맹점이 더 있는지 보라는 요청에 대한 답:

| 라벨 | 생성 위치 | 경로 | 기존에 세고 있었나 |
|---|---|---|---|
| `MALFORMED_COMMAND` | `ws.rs:721`(`send_rejection`) | 게이트웨이 | ✔ |
| `UNKNOWN_COMMAND_TYPE` | 〃 | 게이트웨이 | ✔ |
| `SCHEMA_VERSION_UNSUPPORTED` | 〃 | 게이트웨이 | ✔ |
| `SERVER_BUSY` | `runtime.rs::try_command`(큐 포화) → `ws.rs`의 `send_rejection` | 게이트웨이(경유) | ✔ |
| `TOO_MANY_IN_FLIGHT` | `ws.rs:643`(`send_rejection`) | 게이트웨이 | ✔ |
| **`DUPLICATE_COMMAND_ID`** | `simulation.rs:708`(dedup, **모든 명령 타입 공통**) | **tick** | **✗ — 여기서도 빠져 있었다** |
| `RATE_LIMITED` | `simulation.rs:753` | tick | ✗ (QA 특정) |
| `STALE_INPUT` | `simulation.rs:750` | tick | ✗ (QA 특정) |

**`DUPLICATE_COMMAND_ID`가 세 번째 맹점이다.** QA의 블록 4 표는 `STALE_INPUT`·
`RATE_LIMITED` 두 라벨의 어긋남만 실측했지만(그 봇 시나리오가 중복 `command_id`를
만들지 않았을 뿐), 원인은 동일하다 — dedup 판정이 `simulation.rs`의 같은 함수(모든
명령 타입이 공통으로 거치는 `session.remember(command_id)`)에서 일어나고 같은
`route_outbound` 경로로 나간다. `sc21_duplicate_command_id_yields_one_ping_reply`가
소켓으로 그 거부를 이미 관측하고 있었지만(`payload.reason_code ==
"DUPLICATE_COMMAND_ID"`) 스택 델타는 한 번도 확인하지 않았다 — SC-33이 요구하는
"존재만으로 PASS 주지 않는다"를 그 테스트가 스스로도 어기고 있었던 셈이다(이번 수정
범위 밖이라 그 테스트 자체는 건드리지 않았다. 아래 새 테스트가 대신 짝을 짓는다).

**고친 자리와 근거** (`runtime.rs::route_outbound`, `crates/gateway/src/runtime.rs:551`
근방): QA가 제안한 자리 그대로 — outbound 루프에서 `ServerMessage::CommandResult`의
`reason_code`를 본다. 다른 후보(`simulation.rs`의 3개 생성 지점 각각에 카운터 호출을
넣는 방식)를 쓰지 않은 이유: `record_rejection`은 `Stats`(게이트웨이 크레이트) API이고
`sim` 크레이트는 IO/관측 계층에 의존하지 않는 것이 워크스페이스 경계다(스킬
`rust-authoritative-server` §1 — `domain`·`sim`은 IO 없음). `route_outbound`는 이미
게이트웨이 쪽 경계 함수이고 모든 tick 산출물이 세션으로 나가기 전 **한 곳**을 지나므로,
새 생성 지점이 미래에 추가돼도(예: 새 명령 타입의 새 거부 사유) 이 한 자리가 자동으로
덮는다 — `simulation.rs` 세 곳에 흩어 넣는 방식은 새 맹점을 또 만들 수 있었다.

```rust
// runtime.rs::route_outbound
let rejection_reason = match &out.message {
    ServerMessage::CommandResult(result) => result.payload.reason_code,
    _ => None,
};
...
match route.outbound.try_send(out.message.clone()) {
    Ok(()) => {
        stats.record_message_enqueued(type_name);
        if let Some(reason) = rejection_reason {
            stats.record_rejection(reason);
        }
    }
    ...
```

**세는 시점을 `record_message_enqueued`와 같은 조건(`Ok(())`)으로 맞췄다** — 게이트웨이
쪽 `send_rejection`은 `try_send` 성공 여부와 무관하게 무조건 센다(기존 동작, 이번
수정 범위 밖이라 건드리지 않았다). tick 쪽은 새로 만드는 경로이므로 "클라이언트가
실제로 받을 수 있었던 메시지만 센다"는 더 엄격한 규율을 택했다 — 그래야 송신 큐가
가득 차 거부 응답조차 못 넣는(`Full`) 드문 경우에 카운터가 클라이언트가 못 받은
메시지를 받은 것처럼 부풀리지 않는다.

**TDD — RED 확인.** 새 테스트
`sc33_tick_layer_rejections_are_counted`(`ws_integration.rs`, `set_ship_control_command`
정의 뒤, 파일 끝)를 먼저 쓰고 고치기 전 트리에서 실행:

```
test sc33_tick_layer_rejections_are_counted ...
thread 'sc33_tick_layer_rejections_are_counted' panicked at ws_integration.rs:1399:5:
assertion `left == right` failed: commands_rejected_total{DUPLICATE_COMMAND_ID} 델타가 실제 수신과 어긋난다
  left: 0
 right: 1
FAILED
```

고친 뒤:

```
test sc33_tick_layer_rejections_are_counted ... [SC-33] DUPLICATE_COMMAND_ID delta=1,
RATE_LIMITED delta=3, STALE_INPUT delta=1 — 전부 실제 수신과 일치
ok
```

**테스트가 하는 일** — architect R3 판정의 규율을 그대로 코드로 옮겼다: 라벨 델타만
보지 않는다. 한 세션에서 세 사유를 모두 유도한다(같은 `command_id` 2회 →
`DUPLICATE_COMMAND_ID`, `rate_limit_per_tick_cap`(기본 월드=2)을 넘겨 `SET_SHIP_CONTROL`
5건을 한 tick에 → 뒤 3건이 `RATE_LIMITED`, 다음 tick에 낮은 `input_seq` 1건을 혼자
보내 → `STALE_INPUT`). 클라이언트가 **실제로 받은** 각 사유의 `COMMAND_RESULT`
개수와 `/debug/stats` 델타를 **각각 비교**해 `assert_eq!`로 짝짓는다(QA의 두 출처
대조표를 테스트 안으로 옮긴 형태) — 델타가 있는 것만으로 PASS를 주지 않는다.

### 게이트

`cargo fmt --all --check` 0, `cargo clippy --workspace --all-targets -- -D warnings` 0.
`cargo test --workspace --locked` **3회 연속** 161 passed / 0 failed(추가된
`sc33_tick_layer_rejections_are_counted` 포함, R2 보고의 160 + 1). 추가로 회귀
확인을 위해 `ws_integration.rs` 스위트(SV-1이 있던 파일)만 **11회 연속** 실행 —
전부 통과, 간헐 실패 재현 없음(부하 실험이 아닌 일반 실행 기준. 부하 재현은 위
SV-1 (e) 절 참고).

### 소유권·범위 (이번 라운드)

수정: `server/crates/gateway/src/runtime.rs`(`route_outbound`),
`server/crates/gateway/tests/ws_integration.rs`(`graceful_client_close_is_prompt`
단언 교체, 신규 `sc33_tick_layer_rejections_are_counted`). SV-1 (e) 실험 테스트는
관측 후 삭제 — 최종 diff에 없다. `contracts/`·`data/`·`client/`·`tests/e2e/`·
`tools/bots/`는 건드리지 않았다. 계약 변경 없음. `server/crates/sim/tests/data/replay/`
golden 3파일은 이번 라운드에 손대지 않았다. 커밋하지 않았다.

## R4 수정 — I-29 구조 수정 · (B) 넘겨받기 · SV-1 문서

*2026-09-22. QA R3 §6.7 (I-29/I-30, 동시 세션이 함선 2척을 만들고 자기 참조 causation을
남기던 결함)의 구조 수정과, 사용자 결정 5 (B) "나중 접속이 이어받는다"의 실제 구현.
architect R4 판정(`01_architect_decisions.md` "R3 추가 판정" 1.3·1.6·1.9)과 team-lead의
착수 신호를 따랐다 — 정책 무관 S-1~S-5를 먼저 끝내고, architect의 계약 변경(`SUPERSEDED`)
완료 신호를 받은 뒤 S-6을 시작했다.*

### S-1 — RED 먼저

`crates/sim/src/simulation.rs`의 `#[cfg(test)] mod tests`에 QA §6.7을 in-process로
재현하는 `r4_s1_concurrent_sessions_do_not_orphan_or_fabricate_causation`을 추가했다.
시나리오: 같은 actor의 `OpenSession` 2건(동시 접속) → 둘 다 닫기 → 잔류 만료(600 tick)
→ `shutdown`. 매 tick 뒤 헬퍼 `assert_i29_invariants`가 4개 불변식을 확인한다:
(i) actor당 함선 ≤ 1, (ii) 모든 ACTIVE 함선이 열려 있는 세션을 조종 세션으로 가지며
그 세션이 함선을 2척 이상 조종하지 않는다(고아 없음), (iii) 이번 tick 이벤트 중
`causation_id == event_id`가 없다, (iv) 모든 `SHIP_DESPAWNED`의 원인이 **먼저 발행된**
`SESSION_CLOSED`다(누적 이벤트 로그로 tick을 넘어 검사).

**수정 전 트리에서 RED 확인**:

```
thread 'simulation::tests::r4_s1_concurrent_sessions_do_not_orphan_or_fabricate_causation' panicked:
actor 01a0b1c2-0000-7002-8000-000000000002 가 함선을 2척 이상 가졌다(tick 1)
```

### S-2 — 구조 수정 (1.6-1의 (a)~(d))

**핵심 변경**: `SessionState`(`crates/sim/src/session.rs`)에 `controlling_ship:
Option<UuidV7>` 필드를 추가했다 — 세션이 "자기가 지금 조종하는 함선"을 스스로 기록한다.
`close_session`·`shutdown`의 세션-닫기 루프가 이제 `actor_ship`(actor→함선 표, 동시
접속이 있으면 **다른** 세션의 함선을 가리킬 수 있었다 — QA의 원인 2)이 아니라 **이
필드**로 자기 함선을 찾는다(1.6-1(b)).

- **(a) `open_session`** — 세 경우로 명시적으로 나눴다: 없음(스폰) / `LINGERING`(재개,
  기존 로직) / **`ACTIVE`(동시 접속, 신설)**. `ACTIVE` 경우는 `actor_ship`을 덮어쓰지
  않는다(그 덮어쓰기가 R3 §6.7의 원인 1이었다) — 대신 아래 S-6이 실제 넘겨받기를 채웠다.
- **(b) `close_session`/`shutdown` 첫 루프** — `session.controlling_ship`으로 자기
  함선을 찾는다. `shutdown`은 세션에서 출발하므로 열려 있는 모든 세션을 순회하며 빠지는
  함선이 없다(1.6-1(d)).
- **(c) `despawn_ship`** — `ship.linger_cause_event_id.unwrap_or(event_id)`(자기 참조
  자기잡화, I-30 위반의 직접 원인)를 **삭제**했다. 원인이 없으면 이벤트를 쓰지 않고
  `outcome.causeless_despawns`에 신호만 남긴다(아래 S-3).

### S-3 — 1:1 검사 + 릴리스 안전망

- **디버그 빌드 불변식**: `Simulation::debug_assert_i29`(`#[cfg(debug_assertions)]`,
  릴리스에는 함수 자체가 없다)를 `step`/`shutdown` 끝에서 호출한다. sim의 **기존 45개
  테스트 전부**가 `cargo test`(디버그 빌드)로 도니 이 검사를 공짜로 통과한다 — 개별
  테스트를 고치지 않았다. 45개 전부 그대로 GREEN(회귀 없음).
- **`despawn_ship`의 자체 방어선**: 원인 없이 도달하면(S-2 이후 구조적으로 불가능해야
  한다) 디버그 빌드는 그 자리에서 패닉한다. 새 테스트
  `r4_s3_despawn_ship_panics_in_debug_when_the_linger_cause_is_missing`이 `step`을
  거치지 않고 `despawn_ship`을 직접 불러(내부 상태를 의도적으로 손상시켜) 이 함수
  **자신의** 방어선만 확인한다(`#[should_panic(expected = "I-29/I-30 위반")]`).
- **릴리스 안전망**: `TickOutcome.causeless_despawns: Vec<UuidV7>`를 신설했다(`/debug/stats`
  키는 **더하지 않았다** — 사안 6과 같은 이유). `crates/gateway/src/runtime.rs`에
  `log_causeless_despawns`를 추가해 `sim.step`/`sim.shutdown` 뒤 매번 호출 —
  `starfall-sim`엔 로깅이 없으므로(IO 금지) 게이트웨이가 `tracing::error!`로 남긴다.
  디버그 빌드에선 sim 쪽이 먼저 패닉하므로 이 로그가 실제로 찍히는 것은 릴리스에서
  구조적 방어선이 뚫렸을 때뿐이다 — 있어서는 안 된다.

### S-4 — AC-3(d1) f64 차분 테스트

`ac3_d1_reconnect_does_not_perturb_f64_physics_state`: 같은 actor·같은 초기 입력으로
**재접속 실행**(tick 3에 닫고 tick 60에 재접속, 간격 57 tick ≫ `carry_forward_max_ticks`
10)과 **대조 실행**(세션을 한 번도 닫지 않고 같은 구간 동안 입력만 안 보낸다 — "잔류→
재개 경로를 타는 것 자체"만 격리)을 나란히 돌려, tick 60~139(80 tick) 동안 함선의 `p`·
`v`·`q`·`omega_aim`·`omega_roll`(f64 14필드) 전부를 `to_bits()`로 **비트 단위** 비교했다.
`ShipEntity::activate`/`discard_carried_input_for_resume`가 세션 부기만 만지고 `physics`
필드를 건드리지 않는다는 것을 코드가 아니라 실행으로 고정한다. 출력: `[AC-3(d1)] 재접속
vs 대조, tick 60~139 (80 tick) 비트 동일 확인`.

### S-5 — SV-1 문서 정정 (코드 동작은 바꾸지 않았다)

architect R4 사안 3의 정정된 전제를 `ws_integration.rs`의 4곳에 반영했다: `wait_for_no_
connections` 문서, `GRACEFUL_CLOSE_POLL_BUDGET` 문서, `graceful_client_close_is_prompt`
문서, 그리고 테스트 본문의 출력·단언 실패 메시지. 요지: 폴이 세는 것은 "서버가 열려
있다고 답한 횟수"가 아니라 **"`ws_connections` 게이지가 아직 0이 아니었던 횟수"**이고,
그 게이지는 연결 태스크가 끝날 때 내려가므로 **소켓은 이미 닫혔는데 게이지만 늦게
내려가는 경우**에도 폴이 오른다 — "실제로 늦게 닫힘"과 "게이지만 늦음"은 폴만으로는
구분되지 않는다. 출력 변수 `still_open_answers`를 `gauge_nonzero_polls`로 개명했고,
단언 실패 메시지를 원인을 단정하지 않는 관측형 문장으로 바꿨다(architect가 제시한
문구 그대로).

이 파일(R3 절)의 "참 양성" 라벨도 **철회했다** — 폴 상승만으로 "진짜 지연"과 "게이지
지연"을 가를 수 없는데, 그 문단은 정확히 폴 상승을 근거로 "진짜로 늦게 닫혔다"고
주장하는 순환 논증이었다(위 R3 절의 ⚠ 정정 문단 참고). `K=10`은 architect 판단대로
바꾸지 않았다.

### S-6 — (B) 넘겨받기 (architect의 계약 변경 완료 신호 뒤 착수)

**계약**: architect가 `SESSION_CLOSED.close_reason`에 `SUPERSEDED`를 추가하고
`superseded.json` fixture를 넣은 뒤(유효 26→27), Rust 쪽에서 2건이 예상대로 RED였다 —
내가 고쳤다.

- `crates/contracts/src/events.rs` — `SessionCloseReason::Superseded` 추가
  (`#[serde(rename_all = "SCREAMING_SNAKE_CASE")]`가 자동으로 `"SUPERSEDED"`로 매핑).
- `crates/contracts/tests/contract_tests.rs:38` — `EXPECTED_VALID_FIXTURES` 26→27.
  `cargo test -p starfall-contracts --locked`: **33 + 10 통과**, `fixtures_roundtrip`·
  `registry_consistency` 둘 다 GREEN, `[SC-13] 왕복 검증한 유효 fixture: 27건` 출력 확인.

**게이트웨이** (`crates/gateway/src/runtime.rs`):
- `CloseState`의 `encode`/`decode`에 `Superseded ↔ 7`을 추가.
- `close_code`에 `Superseded → Some(4001)` 추가(ADR-0005 §2). `close_codes_match_adr_0005_
  table` 테스트에 그 행 추가.
- tick 루프: `sim.step` 직후, `outcome.events`에서 `close_reason == Superseded`인
  `SESSION_CLOSED`를 찾아 그 세션의 라우트가 아직 있으면(옛 연결이 살아 있다)
  `close_state.set_if_unset(Superseded)` + `finish.notify_one()` — `SLOW_CONSUMER`와
  같은 경로다. 그 뒤 기존 `closed_sessions` 루프가 라우트를 지운다. 옛 연결이 이미
  소리 없이 끊겨 있었다면(라우트가 없다) 아무도 못 받는다 — 설계대로다(ADR-0011 §6.3:
  "넘겨받기를 택한 이유는 재접속하는 쪽이 옛 연결 탐지를 기다리지 않기 위해서다").

**sim** (`crates/sim/src/simulation.rs::open_session`, "경우 3"): `actor_ship`에 항목이
있고(잔류는 위에서 이미 처리해 반환했으므로 여기 도달하면 `ACTIVE`) 그 함선의
`controlling_session`(=S1)이 아직 `self.sessions`에 있으면, 같은 tick에
`SESSION_CLOSED(S1, SUPERSEDED)`를 발행한다 — `correlation_id`는 S1의 것,
`causation_id`는 이미 위에서 발행해 둔 `SESSION_OPENED(S2).event_id`(SUPERSEDED만
비-null, ADR-0011 §6.3). 이어서 `ship.activate(S2)` + `discard_carried_input_for_resume()`
(재개와 같은 규칙)로 같은 tick에 조종을 옮긴다. 함선 이벤트는 없다(잔류를 거치지 않는다).
조건이 전부 성립하지 않아도(있을 수 없다 — I-29, S-3이 매 tick 잡는다) `actor_ship`을
덮어쓰지 않고 반환한다 — 두 번째 함선은 어떤 경우에도 만들지 않는다.

**TDD — 제출 순번 두 방향** (architect 1.9 요구 4, `simulation.rs` 신규 2건):
- `r4_s6_open_before_close_supersedes_the_old_session` — `OpenSession(S2, seq 0)`이
  `CloseSession(S1, seq 1)`보다 먼저면 넘겨받기. `SESSION_CLOSED`가 세션당 정확히
  1건(S1의 나중 `CloseSession`은 세션이 이미 없어 무시), `close_reason == SUPERSEDED`,
  `causation_id == SESSION_OPENED(S2).event_id`, 함선 이벤트 없음, 새 함선 없음,
  조종이 같은 tick에 S2로 이전을 전부 확인.
- `r4_s6_close_before_open_resumes_without_superseding` — `CloseSession(S1, seq 0)`이
  `OpenSession(S2, seq 1)`보다 먼저면 **잔류→같은 tick 재개**(넘겨받기가 아니다):
  `close_reason == CLIENT_CLOSED`, `causation_id == null`, 함선 이벤트 없음.

**게이트웨이 통합 테스트** (`ws_integration.rs` 신규):
`r4_s6_first_connection_is_closed_with_4001_when_superseded` — 실제 소켓 2개로 같은
actor 연결. 첫 연결이 close **4001**을 받고(`read_until_close`), `server.events()`로
`SESSION_CLOSED{SUPERSEDED}`를 확인. 둘째 연결이 `PING_SERVER`에 `ACCEPTED`로 응답받아
"함선을 실제로 넘겨받아 정상 동작한다"까지 확인.

### 게이트

`cargo fmt --all --check` 0, `cargo clippy --workspace --all-targets -- -D warnings` 0.
`cargo test --workspace --locked` **3회 연속 167 passed / 0 failed**(R3 종료 시 161 +
S-1/S-3/S-4/S-6 신규 6건: sim 5건(`r4_s1`·`r4_s3`·`ac3_d1`·`r4_s6`×2) + gateway 통합
1건(`r4_s6_first_connection...`)). `server/crates/sim/tests/data/replay/` golden 3파일
해시 불변 — `ac8_two_process_replay_produces_byte_identical_snapshots`이 3회 모두
GREEN이라 재생 산출물에 영향이 없음을 확인했다(물리 코드는 건드리지 않았다).

### 소유권·범위 (이번 라운드)

수정: `server/crates/sim/src/session.rs`(`SessionState.controlling_ship`),
`server/crates/sim/src/simulation.rs`(`open_session`/`close_session`/`shutdown`/
`despawn_ship`, `TickOutcome.causeless_despawns`, `debug_assert_i29`, 신규 테스트 6건),
`server/crates/gateway/src/runtime.rs`(`CloseState`·`close_code`·`log_causeless_
despawns`·SUPERSEDED 라우트 정리), `server/crates/gateway/tests/ws_integration.rs`(SV-1
문서 정정 + 신규 S-6 통합 테스트), `server/crates/contracts/src/events.rs`
(`SessionCloseReason::Superseded`), `server/crates/contracts/tests/contract_tests.rs`
(기대 fixture 수). `contracts/`는 architect가 고쳤고 나는 읽기만 했다(Rust 쪽 소비
코드만 내 소유). `client/`·`tests/e2e/`·`tools/bots/`는 건드리지 않았다. golden 3파일은
손대지 않았다. 커밋하지 않았다.

**리더에게**: S-1~S-6 전부 끝났고 게이트 통과했다. QA에게는 직접 알리지 않았다 —
지시대로 client 작업과 함께 묶어 리더가 측정 착수 신호를 보내라.

## R4 후속 — 릴리스 팔 실행 확인 (team-lead 요청, QA 1.4절 공백)

*2026-09-22, 같은 날. QA 1단계에서 R4 재현이 전부 PASS(2척/유령1/자기참조1 → 1척/0/0)
였으나, `despawn_ship`의 "원인 없는 디스폰은 이벤트를 쓰지 않는다"는 릴리스 팔이
**정적 확인뿐**이라는 지적. 디버그 팔(`r4_s3_despawn_ship_panics_in_debug_when_the_
linger_cause_is_missing`, `#[should_panic]`)만으로는 릴리스에서 실제로 그 분기가
"이벤트 없음 + 신호만"으로 도는지 검증되지 않았다.*

**택한 방법**: QA 옵션 ②(`#[cfg(not(debug_assertions))]` 전용 테스트, `cargo test
--release`로 실행) — 옵션 ①(`catch_unwind`)은 디버그 팔이 먼저 패닉해 릴리스 코드
경로 자체를 타지 않으므로 "다른 것을 재는" 문제가 있었다. 대신 기존 디버그 테스트에
`#[cfg(debug_assertions)]`를 붙여 짝을 맞췄다(안 붙이면 `cargo test --release`에서
`#[should_panic]`이 패닉 없음으로 거짓 실패한다 — 붙이기 전엔 잠재 결함이었다).

**새 테스트**: `r4_s3_release_arm_signals_without_fabricating_an_event`
(`crates/sim/src/simulation.rs`, `#[cfg(not(debug_assertions))]`). 원인 없이
`LINGERING`으로 손상시킨 함선에 `despawn_ship`을 직접 호출하고, `outcome.
causeless_despawns == [ship_id]`이면서 `outcome.events.is_empty()`임을 단언한다.

**RED 확인**: `despawn_ship`의 원인 처리 블록을 임시로 옛 `unwrap_or(event_id)`
자기 참조 폴백으로 되돌리고 `cargo test -p starfall-sim --release`로 실행 →

```
assertion `left == right` failed: 릴리스 팔이 신호를 남겨야 한다
  left: []
  right: [UuidV7(...)]
FAILED
```

원상복구 뒤 재실행 → GREEN. `git diff`로 원상복구 확인.

**게이트**: `cargo test -p starfall-sim --release` 1 passed(대상 테스트) / 크레이트
전체 49 passed. `cargo clippy --release -p starfall-sim --all-targets -- -D
warnings` 0. `cargo fmt --all --check` 0. `cargo clippy --workspace --all-targets
-- -D warnings` 0. `cargo test --workspace --locked`(디버그, 기존 게이트) **3회
연속 167 passed / 0 failed**(총 개수는 그대로다 — 디버그/릴리스 팔이 각 빌드에서
1:1로 자리를 바꾼다). golden 3파일 그대로. 커밋하지 않았다.

**리더·qa2에게**: 테스트 이름
`r4_s3_release_arm_signals_without_fabricating_an_event`(`crates/sim/src/
simulation.rs`), RED는 옛 폴백을 임시 복원 → `cargo test --release`로 확인 →
원상복구, `cargo test --workspace --locked` 3회 167 passed. **이제 빌드하지 않는다.**
