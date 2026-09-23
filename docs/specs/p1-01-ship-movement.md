# p1-01-ship-movement: 함선 이동과 월드 상태 동기화

- 상태: **agreed — 최종** (사용자 결정 4건 + 리더 판단 4건 + server·client 검토 반영, 2026-09-20)
- 로드맵 Phase: p1 (코어 프로토타입) — **첫 슬라이스**
- 근거 기획안 절: GDD §4(Character ≠ Ship), GDD §6(Starship System·Scout S-01), GDD §33(서버 판정 8단계), GDD §36(MVP Scope), TECH §7(Reliable/Realtime 분리), TECH §12–13(Simulation Tick·결정성), HSE §95(일관성 모델), HSE §102(Correlation vs Causation)
- 관련 ADR: **0009**(좌표계·단위·양자화·경계), **0010**(결정적 적분), **0011**(상태 동기화·입력 규약·잔류), **0012**(예측·보정) — **전부 accepted**. 기반: 0002(계약 형식), 0005(전송·프레이밍), 0006(tick·명령 큐), 0007(도메인 이벤트 영속화, **§1 표 개정**)
- 선행 슬라이스: `docs/specs/p0-02-networking-spike.md`(implemented), `_workspace/p0-02-networking-spike/05_summary.md`
- 결정 기록: `_workspace/p1-01-ship-movement/01_architect_decisions.md` (충돌 12건 + 검토 라운드 결론)
- 검토: `_workspace/p1-01-ship-movement/01_server_spec_review.md`, `01_client_spec_review.md` (2026-09-20)
- **설계 수치의 정본은 `docs/design/p1-01-ship-movement-design.md`와 `data/`다. 이 스펙은 수치를 정하지 않고 모양·한계·불변식만 정한다.**

## 1. 목표

이 슬라이스가 끝나면 플레이어는 **함선을 조종해 우주를 날 수 있고, 같은 성계에 있는 다른 플레이어의 함선이 움직이는 것을 본다.**

p0-02가 증명한 것은 "명령이 tick을 돌고 사실이 기록된다"였다. 월드에는 아무것도 없었다. 이 슬라이스는 그 tick 루프 위에 **프로젝트 최초의 게임 상태**를 올린다. 그래서 여기서 정해지는 것들 — 좌표계, 단위, 상태를 주고받는 모양, 예측과 보정의 규약, 결정성의 범위 — 은 이 슬라이스의 기능이 아니라 **이후 모든 게임 상태가 올라탈 바닥**이다. 채굴도 전투도 여기서 정한 방식으로 위치를 읽고 상태를 동기화한다.

증명할 것은 넷이다(`00_request.md`).

1. 클라이언트 조작이 **서버 판정을 거쳐** 월드 상태를 바꾼다. 클라이언트가 보낸 위치를 서버가 믿지 않는다.
2. 여러 클라이언트가 **같은 월드를 본다.**
3. 예측·보정으로 **조작이 즉각 반응하면서도** 서버 값과 어긋나지 않는다.
4. p0-02의 성능 기준선이 **게임 로직이 들어간 뒤에도** 유지되는지 측정한다.

**조작 모델은 사용자가 골랐다: 비행 보조 6DoF**(designer 안 2). 마우스가 목표 자세를 가리키고 함선이 그쪽으로 돌며, 추력은 함선 로컬 축에 주고, 비행 보조가 어긋난 속도 성분을 깎는다. 버린 대안과 그 이유는 `docs/design/p1-01-ship-movement-design.md` §3.2에 있고 이 스펙에서 다시 논쟁하지 않는다.

## 2. 플레이 흐름

1. **개발자**가 인프라와 서버를 띄운다. 서버는 기동 시 `data/ships/*.json`·`data/world/systems/*.json`·`data/movement/sync-tuning.json`을 읽어 `contracts/data/`의 스키마로 검증한다. 어긋나면 **기동을 거부**한다(I-38).
2. **플레이어 A**가 접속한다. p0-02의 경로 그대로 `SESSION_READY`를 받아 자기 `actor_id`를 **통보받는다.**
3. **서버**가 같은 tick에 A의 함선을 스폰한다. 위치는 `actor_id`의 시드 해시로 고른 스폰 링 지점이고 간격이 막히면 앞으로 걸어간다 — **난수는 없다**(ADR-0010 §4). `SESSION_OPENED` 다음 `sequence`로 `SHIP_SPAWNED`를 발행하고 `causation_id`에 `SESSION_OPENED.event_id`를 넣는다.
4. **서버**가 그 tick부터 `snapshot_interval_ticks`마다 `WORLD_SNAPSHOT`을 보낸다. A는 여기서 **처음으로 자기 함선이 어디 있는지 알게 된다.** 클라이언트는 그 값을 주장하지 않고 받는다.
5. **플레이어 A**가 조작한다. 클라이언트는 매 tick 하나의 `SET_SHIP_CONTROL`을 보낸다 — **목표 자세·추력·롤·브레이크·보조 토글뿐이고, 위치나 현재 자세는 보낼 수 없다.** 동시에 자기 예측 시뮬레이션을 정확히 1 tick 전진시켜 화면을 즉시 갱신한다.
6. **서버**의 tick이 그 세션의 **이번 tick 도착분 중 마지막 하나**를 골라 ADR-0010 §2의 12단계를 적용한다. 0건이 도착했으면 직전 입력을 이월하고(최대 `carry_forward_max_ticks`), 만료되면 추력·롤을 0으로 두고 목표 자세를 현재 자세로 고정한다. **관성과 보조 감쇠는 계속 작용한다.**
7. **서버**가 `COMMAND_RESULT{ACCEPTED}`를 보낸다. **이것은 "접수"이지 "이만큼 움직였다"가 아니다.** 실제 결과는 다음 스냅샷에 있다.
8. **A의 클라이언트**가 스냅샷을 받아 `ack_input_seq`까지의 확정 상태로 **위치·속도·자세·각속도를 전부** 되돌린 뒤, 그 뒤에 보낸 입력을 다시 적용한다. 오차가 무시 밴드 이하면 화면에 아무 일도 일어나지 않고, 하드 스냅 임계를 넘으면 순간 이동하며 카운터가 오른다(ADR-0012 §4).
9. **플레이어 B**가 접속한다. B의 스냅샷에는 **A의 함선이 들어 있다.**
10. **B의 클라이언트**는 A의 함선을 예측하지 않는다. 스냅샷 2개 사이를 `remote_interp_delay_ms`만큼 과거 시점으로 보간한다 — **위치는 선형, 자세는 slerp.** A가 움직이면 B의 화면에서 A가 움직인다.
11. **플레이어 A**가 최대 추력으로 경계까지 간다. soft 경계를 넘으면 원점 방향 당김이 더해지고(조작은 계속 먹는다), hard 경계에서는 구면에 붙어 미끄러진다. A의 클라이언트도 같은 규칙을 예측하므로 보정이 튀지 않는다.
12. **A의 접속이 끊긴다.** 서버는 그 tick에 `SESSION_CLOSED`를 발행한다. **함선은 사라지지 않는다** — 이월이 만료되면 추력이 0이 되고, 관성으로 미끄러지다 보조 감쇠로 멈추며, `presence: LINGERING`으로 **스냅샷에 계속 포함된다.**
13. **B의 화면**에서 A의 함선은 조종사 없이 표류한다. 눈앞의 함선이 순간 증발하지 않는다.
14. **A가 `reconnect_resume_window_seconds` 안에 재접속하면** 서버는 그 `actor_id`의 함선을 찾아 **같은 함선을 그 자리에서** 돌려준다. 새 세션이고 새 `session_id`·새 `correlation_id`·새 `input_seq` 1번이지만, **`SHIP_SPAWNED`는 발행되지 않는다.**
15. **돌아오지 않으면** `linger_seconds` 만료 tick에 `SHIP_DESPAWNED{LINGER_EXPIRED}`가 발행된다. `causation_id`는 **그 잔류를 시작시킨 `SESSION_CLOSED`의 `event_id`**다 — 30초 전의 이벤트다. 함선이 월드에서 제거되고 다음 스냅샷에서 사라진다.
16. **QA**가 `psql`로 그 함선의 이벤트를 조회한다. `SHIP_SPAWNED` 1건과 `SHIP_DESPAWNED` 1건, 그리고 그동안의 세션 쌍들. **함선이 지나간 좌표는 하나도 기록되어 있지 않다.** 그것이 의도다(I-31).

## 3. 범위

**포함**

- ADR-0009·0010·0011·0012(신규), ADR-0007 §1 기록 범위 표 개정, 이 스펙, 결정 기록, 작업 분해
- 계약: `SET_SHIP_CONTROL`(명령), `WORLD_SNAPSHOT`(서버 메시지), `SHIP_SPAWNED`·`SHIP_DESPAWNED`(도메인 이벤트), `SHIP_CLASS`·`STAR_SYSTEM`·`SYNC_TUNING`(데이터 테이블) 신설. `COMMAND_RESULT.reason_code`에 `RATE_LIMITED`·`STALE_INPUT` 추가(호환 변경). 공통 primitives에 물리량 7종 추가. **유효 fixture 26, 반례 34**(§5.3)
- 서버: `starfall-sim` 안의 월드 상태 모델(함선 엔티티, `actor_id → ship_id` 표, 잔류 타이머), ADR-0010의 적분 12단계, 입력 확정·이월·거부, 스폰·잔류·재개·디스폰과 도메인 이벤트, 스냅샷 생성, 세션별 브로드캐스트, 송신 큐 용량 조정과 `send_queue_bytes` 메트릭
- 서버: `data/` 테이블 3종 로딩과 계약 스키마 검증, 기동 시 거부 경로, 유도값 검산(`snapshot_interval_ticks`, `point_count` ↔ `points_m`, 재개 창 ≤ 잔류 시간, 스폰 지점 ⊂ 경계)
- 클라이언트: `tools/codegen` 확장(배열, `maxItems`, `data`/`rest` kind 건너뛰기), 신규 DTO 생성, `double` 예측 코어, 재조정, 입력 생성, 타 함선 보간, 그레이박스 씬·카메라·기준 마커·진단 HUD
- QA: 봇 하네스에 이동·스냅샷 지원 추가, 2-클라이언트 가시성 검증, 치트 시나리오, 31 연결 부하와 **대역폭 실측**, p0-02 성능 기준선과의 회귀 비교

**제외 (다음 슬라이스로)**

- 채굴·인벤토리·거래·전투 (p1-02 이후)
- **Historical Event 판정 일체** — 이동은 역사가 아니다(원칙 4). 새 도메인 이벤트 2종도 Level 0이고 승격되지 않는다(§6)
- 워프·성계 간 이동·다수 성계
- **Floating Origin과 스케일 공간** — 경계를 20 km 이하로 제한하고 근거를 ADR-0009 §3에 수치로 남긴다. designer가 고른 값은 12 km다
- 함선 외형·모듈 비주얼·VFX·**충돌 판정**(함선끼리 통과한다. `hull_radius_m`은 스폰 간격과 지표에만 쓴다)
- 관심 영역 필터링, 델타 스냅샷 — 도입 트리거는 ADR-0011 §2에 **동시 함선 40척**으로 숫자화되어 있다
- 함선 상태 영속화 — 서버 재기동으로 모든 함선이 사라진다(ADR-0011 §7). 30초 잔류는 프로세스 안의 타이머이지 영속화가 아니다
- **Transactional Outbox와 `processed_commands`** — 이동에는 DB 상태 변경과 짝지어야 할 이벤트가 없다. 기한을 **p1-02(채굴·인벤토리)**로 정확히 다시 적는다. 근거는 ADR-0011 §7
- **CI + `sqlx-cli`/`.sqlx/`** — 사용자·리더 결정으로 **별도 슬라이스 `p1-01b`**로 뺀다. p1-01 완료 직후
- 클라이언트로의 데이터 테이블 배포 경로 — 이번엔 파일 복사(사용자 결정, ADR-0012 §7)

## 4. 규칙과 불변식

p0-01의 I-1~I-9, p0-02의 I-10~I-25는 계속 유효하다. 이 슬라이스가 새로 강제하는 것은 다음이다.

**클라이언트 권위 부정 (원칙 1)**

- **I-26 클라이언트는 위치·속도·현재 자세를 보낼 수 없다.** 무시하는 것이 아니라 **어휘에 없다.** `SET_SHIP_CONTROL`의 payload는 `additionalProperties: false`이고 Rust는 `deny_unknown_fields`이므로, 위치나 자세 필드를 주입한 명령은 `MALFORMED_COMMAND`로 거부된다.
  - **`aim_*`는 예외가 아니다.** 목표 자세는 **의도**(그쪽으로 돌고 싶다)이지 **현재 상태에 대한 주장**이 아니다. 서버는 그것을 채택하지 않고, 자기가 아는 자세에서 자기가 아는 선회율로 **그쪽을 향해 돌 뿐**이다. 클라이언트가 목표 자세를 아무리 극단적으로 보내도 한 tick의 회전량은 `turn_rate_max_deg_s`를 넘지 못한다.
- **I-27 함선 상태는 tick의 적분으로만 바뀐다.** ADR-0010 §2의 12단계가 유일한 변경 경로다. 스냅샷 생성·메트릭 수집·직렬화는 상태를 읽기만 한다. (I-13의 구체화)
- **I-28 한 tick에 한 세션의 입력은 정확히 1건 적용된다.** 여러 건이 도착하면 **마지막 것이 이기고** 나머지는 그 tick에서 사라진다. 입력은 누적 델타가 아니라 **절대 상태**이므로 덮어쓰기가 손실이 아니다. 더 빨리 보내도 더 멀리 가지 못한다 — 쌓아 둘 것이 없기 때문이다.
- **I-33 범위를 벗어난 조작 값은 클램프하지 않고 거부한다.** 조용히 정상화하면 망가진 클라이언트가 정상으로 보인다. **이월 규칙이 거부를 안전하게 만든다** — 거부된 입력 자리에는 직전 입력이 들어가므로 조작이 끊기지 않는다. 반면 **벡터 크기 클램프(대각선 추력)는 게임 규칙**이므로 정규화한다.
- **I-39 범위 위반(거부)과 퇴화 값(규칙 처리)을 섞지 않는다.** 각 성분이 범위 안이어도 목표 자세 쿼터니언의 노름이 0에 가까우면 정규화할 수 없다. 이것은 계약 위반이 아니므로 거부하지 않고 **그 tick의 목표 자세를 현재 자세로 대체**하며 `aim_degenerate_total`로 센다.
- **I-34 함선은 하드 경계 밖으로 나가지 않는다.** soft 경계에서는 원점 방향 당김을 **더하고**(조작은 계속 먹는다), hard 경계에서는 위치를 구면에 투영하고 **바깥으로 향하는 반경 속도 성분만** 제거한다. 접선 성분은 남아 벽을 따라 미끄러진다. 튕겨 내지 않는다.

**월드 상태와 엔티티**

- **I-29 한 `actor_id`는 그 월드에 최대 1척의 함선을 가진다. 그리고 열린 세션과 `ACTIVE` 함선은 1:1이다** — 열린 세션은 정확히 1척을 조종하고, `ACTIVE` 함선은 정확히 1개의 **열린** 세션이 조종하며, `LINGERING` 함선은 조종 세션이 없다. 세션이 열리면 서버가 먼저 그 actor의 함선 상태를 본다. **경우는 셋이고 셋 다 정해져 있어야 한다**: 함선 없음 → 스폰 / `LINGERING`(재개 창 안) → **그 함선을 되돌려준다**(새 `SHIP_SPAWNED` 없음) / **`ACTIVE`(다른 세션 S1이 조종 중) → 넘겨받는다**(사용자 결정 5, 2026-09-22). 새 세션 S2의 `SESSION_OPENED` 바로 다음에 S1을 `SESSION_CLOSED{SUPERSEDED}`로 닫는다. 이 이벤트의 `causation_id`는 `SESSION_OPENED(S2).event_id`다. 같은 tick에 조종을 S2로 옮기고(규칙은 재개와 같다 — I-43), 잔류를 거치지 않으며, 함선 이벤트는 없다. 옛 연결은 close 4001로 닫힌다(ADR-0005 §2, ADR-0011 §6.3). **어느 경우에도 두 번째 함선은 만들지 않는다.** 판정은 sim이 제출 순번으로 한다 — 게이트웨이가 아는 "함선을 가졌는가"는 tick 스냅샷이라 같은 tick 창의 두 연결을 가르지 못한다. **함선의 소유는 세션이 아니라 actor에 묶인다.** *(2026-09-22 개정. 셋째 경우가 비어 있어서 서버가 두 번째 함선을 스폰했다. 그러자 첫 함선이 조종 세션 없는 `ACTIVE`로 남았고(I-40 위반), 종료 때 자기 자신을 원인으로 디스폰됐다(I-30 위반) — QA R3 §6.7. 소리 없이 끊긴 연결(FIN 없음)의 재접속이 바로 이 경로라서 드문 경우가 아니다.)*
- **I-40 함선은 세션보다 오래 산다.** `SESSION_CLOSED`는 디스폰이 아니라 **잔류의 시작**이다. 단 **`SUPERSEDED`는 예외**다: 같은 tick에 조종이 새 세션으로 넘어가므로 잔류가 없다(I-29, 2026-09-22). 디스폰은 `linger_seconds` 만료 또는 서버 정상 종료에만 일어난다. **조종 세션 없는 `ACTIVE` 함선은 어느 tick에도 존재하지 않는다.**
- **I-41 `SHIP_SPAWNED`와 `SHIP_DESPAWNED`는 함선당 정확히 1건씩이고, 짝짓기는 `ship_id`로 한다.** `correlation_id`로 짝지으면 맞지 않는다 — 한 함선이 여러 세션을 거칠 수 있고, 두 이벤트의 `correlation_id`는 각각 **그 이벤트를 일으킨 세션**을 가리킨다. 정상 종료 시 잔류 함선도 `SERVER_SHUTDOWN`으로 디스폰해 이 불변식을 프로세스 경계에서 지킨다.
- **I-30 두 함선 이벤트의 `causation_id`는 비-null이다.** `SHIP_SPAWNED`는 같은 tick의 `SESSION_OPENED.event_id`, `SHIP_DESPAWNED`는 **그 잔류를 시작시킨 `SESSION_CLOSED.event_id`**(여러 tick 전일 수 있다). **프로젝트에서 `causation_id`가 실제 값을 갖는 첫 사례**이고, 디스폰 쪽은 상관과 인과가 같은 행에서 서로 다른 시각을 가리키는 첫 데이터다(HSE §102).
  - **원인은 언제나 이미 발행된 다른 이벤트다.** 타입이 정해져 있고(`SHIP_SPAWNED` ← `SESSION_OPENED`, `SHIP_DESPAWNED` ← `SESSION_CLOSED`, *2026-09-22부터* **`SESSION_CLOSED{SUPERSEDED}` ← 새 세션의 `SESSION_OPENED`** — 다른 사유의 `SESSION_CLOSED`는 원인이 null이다) `(tick, sequence)`가 결과보다 작다. 그러므로 **`causation_id = event_id`(자기 참조)는 어떤 경우에도 결함이다.**
  - **원인을 지어내지 않는다.** 원인이 없는 상태에 도달했더라도 그 자리를 이벤트 자신의 id나 "그럴듯한" 다른 이벤트로 채우지 않는다. **빠진 기록은 짝 검사(I-41)가 찾아내지만 거짓 원인은 진짜처럼 보인다**(원칙 9). 원인 없는 상태는 **구조적으로 도달할 수 없어야** 하고(I-29의 1:1), 테스트는 매 tick 끝에 그 1:1을 검사한다. *(2026-09-22 추가. 코드는 이 경우를 "있을 수 없다"는 주석과 로그만 달고 자기 id로 채웠는데, 실제로 그 경로를 탔다. DB에 이미 7행이 있다 — §11-8)*
- **I-31 이동은 도메인 이벤트를 만들지 않는다.** 함선의 위치는 메모리 `WorldState`가 권위이고 `domain_events`에 들어가지 않는다. 기록되는 것은 **존재의 시작과 끝뿐**이다. 관측 경로는 `WORLD_SNAPSHOT`이다.
- **I-38 데이터 테이블이 계약을 어기면 서버는 기동하지 않는다.** 스키마 검증에 더해 유도값도 검산한다: `tick_hz / snapshot_hz`가 정수, `point_count == len(points_m)`, 모든 스폰 지점이 하드 경계 안, `reconnect_resume_window_seconds ≤ linger_seconds`, `soft ≤ hard`, `main_thrust_mps2 ≥ lateral`·`≥ reverse`, **`max_entities_per_snapshot ≤ 64`**(계약 `maxItems`), 함선 클래스 `id` 중복 없음. **런타임에 발견되는 것은 실패다.**
  - **데이터 디렉토리 해석 규칙(확정).** `STARFALL_DATA_DIR`이 설정돼 있으면 **그 경로만** 쓴다(탐색 없음 — QA의 기동 거부 주입이 정확해야 한다). 설정돼 있지 않으면 cwd 기준 **`data` → `../data` 순으로 처음 존재하는 디렉토리**를 쓴다. `cargo run`의 cwd가 `server/`이고 서버 바이너리를 레포 루트에서 직접 실행할 수도 있어, 둘 중 하나를 기본값으로 박으면 **다른 하나가 항상 틀린다.** 어느 쪽이든 **해석된 절대 경로를 기동 로그와 `/debug/stats`에 찍고**, 찾지 못하면 **시도한 절대 경로를 전부 찍고 기동을 거부한다.** 탐색은 기본값에만 있고 2단계로 끝나며 결과가 기록되므로, "어느 데이터를 읽었는지 모른다"는 상태가 생기지 않는다.
- **I-42 잔류 함선도 매 tick 12단계를 돈다.** 조종사가 없을 뿐 시뮬레이션에서 빠지지 않는다. 이월 창이 지나면 **휴면 입력**(추력·롤 0, 목표 자세 = 현재 자세, `brake=false`, **`flight_assist=true`**)이 들어가고, 그 결과 함선은 보조 감쇠로 감속하면서 **오토레벨로 수평을 되찾는다**. 값과 근거는 ADR-0011 §6.1. `flight_assist`를 조종사의 마지막 토글에서 이어받지 않는 이유는 그러면 같은 잔류가 클라이언트 설정에 따라 다르게 굴러 재현이 불가능해지기 때문이다.
- **I-43 재개는 월드 상태를 이어받고 세션·입력 상태를 버린다.** 특히 **`last_applied_input_seq`를 `None`으로 초기화**한다 — 옛 값을 남기면 새 세션의 `input_seq = 1`이 전부 `STALE_INPUT`으로 거부되어 "재접속하면 조작이 안 먹는다"가 된다. 이월 중이던 입력도 버린다(되살리면 재접속 순간 함선이 가속한다). 표는 ADR-0011 §6.2.
- **I-44 월드에는 정원이 있고, 정원은 입장에서 막는다. 단 재개는 면제한다.** 함선 수가 `max_entities_per_snapshot`에 도달하고 **그 actor가 함선을 갖고 있지 않을 때만** `GET /ws`가 503 `reason=world_full`이다. 잔류 함선을 되찾는 접속은 함선 수를 늘리지 않으므로 막지 않는다 — 막으면 **잠깐 끊긴 플레이어가 자기 함선으로 돌아오지 못하고 그 함선이 주인 없이 사라진다.** 스폰을 거부하지 않는 이유는 **함선 없는 세션**이 I-29를 깨기 때문이다. 조립 시 길이가 상한을 넘으면 그것은 서버 버그이므로 로그·카운터를 남기고 **그대로 보낸다** — 잘라 내면 틀린 세계를 보여 주게 된다(ADR-0011 §1).

**표현과 결정성**

- **I-47 한 세션이 한 tick에 큐에 넣는 메시지 수는 서버가 정한다.** tick당 판정 명령을 `MAX_COMMANDS_PER_SESSION_PER_TICK`(8)으로 묶고, 초과 프레임은 **제출하지 않으며 `COMMAND_RESULT`도 만들지 않는다** — 카운터 + 그 tick에 대한 프로토콜 위반 1회다. 따라서 **I-15(명령 1건 ↔ 결과 1건)는 "서버가 판정에 넣은 명령"에 대한 진술**이고, QA의 손실 항등식은 `보낸 수 = COMMAND_RESULT 수 + commands_dropped_over_tick_cap_total`이다(두 항의 출처가 달라 이전보다 강한 검사다 — I-25). 근거와 산수는 ADR-0011 §5.2.
- **I-45 재연결 백오프 상한(10초) < `linger_seconds`(30초).** 정원이 차서 밀려난 플레이어가 **잔류 창 안에 여러 번 재시도**할 수 있어야 자기 함선을 되찾는다(I-44의 면제 + ADR-0012 §8). `linger_seconds`를 백오프 상한 아래로 줄이면 "정원이 차면 자기 함선을 잃는다"가 되살아난다 — designer가 그 값을 만질 때 알아야 할 제약이다. *(2026-09-22 보충)* 정확히 쓰면 부등식은 **백오프 상한 < `reconnect_resume_window_seconds`(≤ `linger_seconds`)**이다. 지금은 두 값이 모두 30이고 서버가 잔류 구간 전체를 재개 가능으로 취급하므로 차이가 없다(Q15). 이 부등식은 옛 세션이 정리되는 시점과 무관하게 성립한다. 되찾기 창이 옛 세션의 `SESSION_CLOSED`에서 시작하므로, 그 뒤 첫 재시도가 상한 안에 창 안으로 떨어지기 때문이다.
- **I-46 계약이 소비자에게 요구하는 처리는 설명이 아니라 구현 의무다.** 지금 셋이다: **양자화된 쿼터니언을 쓰기 전에 정규화한다**(ADR-0009 §2), **역양자화는 선언된 배율로만 한다**, **닫힌 값 집합의 모르는 값을 받아도 죽지 않는다**(ADR-0005 §4). 스키마 `description`에 적혀 있다는 것이 구현되어 있다는 뜻이 아니다 — client가 **자세를 정규화하지 않던 버그를 실행으로 잡았고**(2026-09-20), 계약은 그 처리를 명시하고 있었다. **fixture가 강제하지 않는 요구는 문서일 뿐이다**(후속 T-C7).
- **I-32 와이어의 모든 물리량은 정수 양자화 값이다.** 계약에 실수는 없다. **데이터 테이블은 명시적 예외**로 SI 실수를 쓴다(ADR-0009 §2). C#의 반올림은 `MidpointRounding.AwayFromZero`를 명시해야 Rust와 같다.
- **I-35 예측은 서버 상태를 덮어쓰지 않는다.** 확정 기준점은 언제나 가장 최근 스냅샷의 자기 함선 항목이고, 예측은 그 위에 미확인 입력을 다시 적용한 결과다. 두 값을 섞은 제3의 상태를 만들지 않는다.
- **I-36 클라이언트 예측은 `double`로, 서버와 같은 연산 순서로, 자기가 보낸 양자화 정수를 역변환한 값으로 한다.** 원시 실수로 예측하면 어디에도 버그가 없는데 상시 어긋난다.
- **I-37 같은 초기 상태 + 같은 입력열 = 같은 스냅샷 바이트.** 시뮬레이션 안에서 초월함수(`sin`·`cos`·`acos`·`atan2`·`exp`·`pow`)를 쓰지 않고, 난수가 없으며, 함선 순회는 `ship_id` 순이다. 검증은 근사 비교가 아니라 **바이트 비교**다.

## 5. 데이터 계약

### 5.1 타입 (`contracts/registry/types.json`, `registry_version: 3`, 타입 6 → 13)

| 타입 | kind | 생산자 | 소비자 | 요지 |
|------|------|--------|--------|------|
| `SET_SHIP_CONTROL` | command | client, bots | server | **신규.** `{input_seq, thrust_x/y/z_milli, roll_milli, aim_x/y/z/w_micro, brake, flight_assist}`. 위치·현재 자세 필드가 **없다** |
| `WORLD_SNAPSHOT` | server_message | server | client, bots | **신규.** 월드부 `{star_system_id, soft/hard_boundary_radius_mm, snapshot_interval_ticks, ships[]}` + 세션부 `{controlled_ship_id, ack_input_seq}`. `ShipState`는 각속도를 **둘**로 싣는다(`ω_aim` 3성분 + `ω_roll` 스칼라 — ADR-0010 §1.1) |
| `SHIP_SPAWNED` | domain_event | server | server | **신규.** `actor_id`·`causation_id` 비-null로 좁힘 |
| `SHIP_DESPAWNED` | domain_event | server | server | **신규.** `{ship_id, last_session_id, despawn_reason, 위치 3}`. 같은 좁힘 |
| `SHIP_CLASS` | data | (없음) | server | **신규.** `data/ships/*.json`의 모양 |
| `STAR_SYSTEM` | data | (없음) | server | **신규.** `data/world/systems/*.json`. 경계 상한이 float32 예산 |
| `SYNC_TUNING` | data | (없음) | server, client | **신규.** `data/movement/sync-tuning.json`. 클라이언트도 보정 임계를 여기서 읽는다 |
| `COMMAND_RESULT` | 변경 | server | client, bots | `reason_code`에 `RATE_LIMITED`·`STALE_INPUT` 추가 — 같은 `schema_version`의 호환 변경 |

`SHIP_*`의 소비자에 `history`를 넣지 않는다 — `server/crates/history`가 아직 없고, 없는 소비자는 커버리지 스크립트가 오류로 막는다. p2에서 추가한다.
데이터 3종의 `producers`는 **빈 배열**이다. 테이블은 사람이 쓰는 것이지 어떤 컴포넌트가 발행하는 것이 아니다.

닫힌 값 집합: `ShipState.presence` = `ACTIVE` | `LINGERING`, `SHIP_DESPAWNED.despawn_reason` = `LINGER_EXPIRED` | `SERVER_SHUTDOWN`. 기존 규약대로 Rust는 닫힌 열거형, C#은 `string`으로 매핑한다.

### 5.1a 명령 → 기대 응답 (규범. 도구가 참조하는 정본)

*2026-09-21 신설(architect R3). 이 관계는 지금까지 어느 표에도 없었고 `tools/bots`가 암묵적으로 가정하다가 **"명령이 하나도 수락되지 않을 때만 통과하는 게이트"**를 만들었다(라운드 3). 레지스트리 필드로 올리는 것은 **명령 타입이 세 번째로 늘 때 다음 `registry_version` 인상과 함께** 한다 — 두 타입으로 필드 모양을 정할 수 없다(원칙 8·10). 그때까지 **정본은 이 표**이고, 봇·QA 도구는 이 절을 참조한다는 주석을 코드에 남긴다.*

**공통 규칙: 판정에 들어간 모든 명령은 수락이든 거부든 정확히 하나의 `COMMAND_RESULT`를 낳는다**(I-15, AC-9(c)의 1:1). **타입별 응답은 그 위에 더해지는 것**이고 아래에 타입마다 명시한다.

| 명령 | `COMMAND_RESULT` | 타입별 응답 | 비고 |
|------|------------------|-------------|------|
| `PING_SERVER` | 1건 (항상) | **`PING_REPLY` 1건 — 수락일 때만**, 그 명령의 `COMMAND_RESULT` **뒤에** 온다 | 레지스트리 `PING_REPLY`의 `description`이 같은 것을 산문으로 적고 있다 |
| `SET_SHIP_CONTROL` | 1건 (항상) | **없다** | 상태 변화는 `WORLD_SNAPSHOT`으로 나타나지 명령에 대한 응답이 아니다. **`PING_REPLY`를 기대하면 안 된다** |

**게이트를 쓸 때의 의무 (라운드 3에서 실제로 깨진 자리):** 이 표에서 유도한 **카운터 항등식은 입력이 전부 0일 때 반드시 실패해야 한다.** 항등식마다 **"그 경로가 실제로 탔다"는 단언을 짝지어라**(예: `accepted > 0`). 짝이 없으면 그 게이트는 **"아무 일도 일어나지 않았다"를 통과로 읽는다** — M-17을 게이트 자신에게 적용한 것이다.

### 5.2 공통 primitives 추가 (`contracts/common/primitives.schema.json`)

`DataId`, `PositionMm`, `VelocityMmPerSecond`, `QuaternionComponentMicro`, `AngularVelocityMdegPerSecond`, `ControlAxisMilli`, `InputSeq` 7종. **`DataId`는 lower-kebab-case이고 `data/`의 `id` 필드와 글자 그대로 같다** — 대소문자·구분자 변환을 두지 않는다(ADR-0009 §2). 서버는 `data/`를 **파일 이름이 아니라 `id` 필드로** 색인한다(client U-C2 질의에 대한 답). 배율·범위·언어 매핑은 ADR-0009 §2가 정본이다. **각 정의의 `description`에 축 의미가 명시 문구로 들어 있다**(리더 요구) — 계약을 읽는 사람이 ADR을 찾아가지 않아도 어느 축이 위/앞/오른쪽인지 안다.

### 5.3 파일과 건수

| 파일 | 상태 |
|------|------|
| `contracts/common/primitives.schema.json` | `$defs` 7종 추가 + 축 의미 문구 |
| `contracts/commands/SET_SHIP_CONTROL.schema.json` | 신규 |
| `contracts/messages/WORLD_SNAPSHOT.schema.json` | 신규 (`items`를 쓰는 **첫 계약**) |
| `contracts/events/domain/SHIP_SPAWNED.schema.json` | 신규 |
| `contracts/events/domain/SHIP_DESPAWNED.schema.json` | 신규 |
| `contracts/data/ship-class.schema.json` | 신규 (`contracts/data/`의 첫 파일) |
| `contracts/data/star-system.schema.json` | 신규 |
| `contracts/data/sync-tuning.schema.json` | 신규 |
| `contracts/messages/COMMAND_RESULT.schema.json` | `reason_code` 값 2개 추가 |
| `contracts/registry/types.json` | `registry_version` 2 → 3, 타입 6 → 13. *(2026-09-22: `SHIP_DESPAWNED` description 정정 — "같은 tick의 `SESSION_CLOSED`"는 잔류 도입 전 문구였다. `registry_version`은 그대로 3)* |
| `contracts/events/domain/SESSION_CLOSED.schema.json` | *(2026-09-22, 사용자 결정 5)* `close_reason` 값 **`SUPERSEDED`** 추가. **`schema_version` 1 유지** — 추가형이고, 필드 설명이 이미 값 추가를 약속했다(ADR-0005 §4-2: C#은 `string`, Rust는 닫힌 열거형) |
| `contracts/fixtures/SESSION_CLOSED/superseded.json` | *(2026-09-22)* 신규 유효 fixture. `causation_id` 비-null(= 새 세션의 `SESSION_OPENED.event_id`). **이전 스키마에서는 거부됨을 확인했다** — 새 값을 실제로 검사하는 fixture다 |

**건수 (architect 실측, 2026-09-19 — Python `jsonschema` 4.23.0 오프라인 검증기 전수 실행):**

| | 수 | 실행 결과 |
|---|---:|------|
| 스키마 파일 (`registry/types.schema.json` 포함) | **18** | 메타스키마 검증 통과 18/18, `$id`↔경로 불일치 0 |
| 유효 fixture | **27** *(2026-09-22: 26 → 27)* | **27 전부 통과** (타입 13 × 2 + `SESSION_CLOSED/superseded.json`). 그중 **C# DTO가 있는 것은 21건**(예측, client 확인 대기) — 데이터 6건은 envelope 판별자조차 없다(client 실측). *2026-09-19 실측은 26/20이었다.* **2026-09-22 재실측(architect, 같은 검증기)**: 스키마 18 / 유효 27 통과 / 반례 34 전부 거부 / 레지스트리 13, `check_contract_coverage.py --strict` PASS |
| 반례 fixture | **34** | **34 전부 거부.** B-1로 `WORLD_SNAPSHOT/invalid/angular-velocity-roll-missing.json` 1건 추가 |

반례에는 공식이 없다 — **반례 수가 바뀌면 architect가 이 표와 AC를 함께 갱신한다.**

### 5.4 반례 fixture의 층별 거부 책임

p0-01 §5, p0-02 §5.4 표를 이어간다. 운영 중 명령을 막는 것은 스키마 검증기가 아니라 serde이므로 **Rust 열이 핵심**이다.

> **`C# (Strict)` 열은 client가 반례 34건을 전수 역직렬화해 채웠다 (2026-09-20).** 초안이 예측으로 적은 11행이 **전부 실측과 일치했고 정정할 칸이 0이다.** 전체 집계: **C# 거부 18 / C# 통과(감지 불가) 10 / C# 계층 없음(데이터 타입) 6 = 34.** *B-1로 추가된 34번째 반례 `WORLD_SNAPSHOT/invalid/angular-velocity-roll-missing.json`은 데이터 타입이 아니라 **메시지 타입이라 C#에 계층이 있고 거부된다** — 그래서 거부 17 → 18, 계층 없음 7 → 6이다. 계층 없음은 **데이터 3종 × 2건 = 6건**이지 7건이 아니다(client 실측, QA R1 §6.2).* 부수 확인: p0-02의 생성기 좁힘 수정이 **`causation_id`에도 생성기를 손대지 않고 그대로 먹는다** — 병합 규칙이 필드 이름과 무관하게 동작한다는 뜻이다.

| fixture | 스키마 | Rust serde | C# (Strict) | 비고 |
|---------|--------|-----------|-------------|------|
| `SET_SHIP_CONTROL/invalid/position-field-injected.json` | 거부 | 거부 (`deny_unknown_fields`) | **거부 (실측)** | **이 슬라이스의 대표 반례.** 원칙 1의 계약 수준 방어 |
| `SET_SHIP_CONTROL/invalid/attitude-field-injected.json` | 거부 | 거부 | **거부 (실측)** | "목표 자세는 되는데 현재 자세는 안 된다"의 경계를 못박는다 |
| `SET_SHIP_CONTROL/invalid/thrust-above-range.json` | 거부 (`maximum`) | 거부 (범위 newtype) | **감지 불가 (실측)** | ±1000 → C# `int`. 명령은 클라이언트가 **생산**하므로 C# 역직렬화는 테스트 경로뿐이다 |
| `SET_SHIP_CONTROL/invalid/input-seq-zero.json` | 거부 (`minimum: 1`) | 거부 | **감지 불가 (실측)** | `input_seq`는 1부터다 |
| `WORLD_SNAPSHOT/invalid/ship-missing-orientation-w.json` | 거부 | 거부 (필수 필드) | **거부 (실측)** | **배열 원소 안의 검증이 동작하는지**를 보는 항목 — 동작한다 |
| `WORLD_SNAPSHOT/invalid/angular-velocity-roll-missing.json` | 거부 (필수) | 거부 | **거부 (실측)** | **B-1로 추가.** 각속도를 합 하나로 되돌리려는 구현을 계약 단계에서 막는다 |
| `WORLD_SNAPSHOT/invalid/unknown-presence.json` | 거부 | 거부 (닫힌 열거형) | **감지 불가 (실측)** | `enum` → C# `string` 매핑의 의도된 대가 |
| `WORLD_SNAPSHOT/invalid/hard-radius-above-ceiling.json` | 거부 (`maximum`) | 거부 | **감지 불가 (실측)** | float32 예산이 계약으로 강제되는 지점 |
| `SHIP_SPAWNED/invalid/actor-id-null.json` | 거부 (좁힘) | 거부 (비-`Option`) | **거부 (실측)** | p0-02의 생성기 좁힘 수정이 **새 타입에도** 적용된다 |
| `SHIP_SPAWNED/invalid/causation-id-null.json` | 거부 (좁힘) | 거부 | **거부 (실측)** | **`causation_id` 좁힘의 첫 사례** — 생성기 수정 없이 통과 |
| `SHIP_DESPAWNED/invalid/causation-id-null.json` | 거부 (좁힘) | 거부 | **거부 (실측)** | 대칭 확인 |
| `SHIP_DESPAWNED/invalid/session-closed-is-not-a-despawn.json` | 거부 | 거부 (닫힌 열거형) | **감지 불가 (실측)** | **설계를 반례로 고정한다**: 세션 종료는 디스폰 사유가 아니다(I-40) |
| `SHIP_CLASS/invalid/turn-gain-zero.json` | 거부 (`exclusiveMinimum`) | 거부 | (계층 없음) | 게인 0이면 함선이 절대 돌지 않는다 |
| `SHIP_CLASS/invalid/movement-unknown-field.json` | 거부 | 거부 | (계층 없음) | 오타·엉뚱한 블록 혼입 |
| `STAR_SYSTEM/invalid/hard-radius-above-ceiling.json` | 거부 | 거부 | (계층 없음) | **I-38의 증거.** designer가 20 km를 넘기면 기동이 멈춘다 |
| `STAR_SYSTEM/invalid/no-spawn-points.json` | 거부 (`minItems: 1`) | 거부 | (계층 없음) | 스폰 지점 0개면 접속이 불가능해진다 |
| `SYNC_TUNING/invalid/snapshot-hz-zero.json` | 거부 | 거부 | (계층 없음) | |
| `SYNC_TUNING/invalid/integrator-is-not-a-tunable.json` | 거부 | 거부 | (계층 없음) | **적분기 정체성은 데이터가 아니라 법이다**(ADR-0010) |

### 5.5 fixture는 설계 값이 아니다

`contracts/fixtures/SHIP_CLASS/*`·`STAR_SYSTEM/*`·`SYNC_TUNING/*`의 숫자는 **스키마를 검증하기 위한 예시**이며 `data/`에 들어갈 값이 아니다. 정본은 designer의 `docs/design/p1-01-ship-movement-design.md`와 `data/`다. 두 값이 달라도 모순이 아니다 — fixture는 "이 모양이 유효한가"를, `data/`는 "무엇이 재미있는가"를 답한다. **실제 `data/` 파일의 검증은 서버 기동 경로가 한다**(AC-2).

### 5.6 designer의 실제 데이터 파일 검증 결과 (architect 실측, 2026-09-19)

| 파일 | 결과 |
|------|------|
| `data/ships/scout-s01.json` | **통과 (오류 0)** |
| `data/world/systems/cradle.json` | **통과 (오류 0)** |
| `data/movement/sync-tuning.json` | **통과 (오류 0)** — D-1·D-2가 반영됐다 |

**세 파일 모두 통과한다** (architect·server·client가 독립으로 재확인, 2026-09-20). server가 I-38의 유도값 5종도 실제 `data/`로 검산해 전부 통과했다: `20/10 = 2` 정수, `point_count 12 == len(points_m) 12`, `soft 10000 ≤ hard 12000`, 스폰 최대 반경 **2512.469 m** < 12000, `resume 30 ≤ linger 30`. **S2가 실제 `data/`로 바로 갈 수 있다.**

### 5.7 생성기 확장이 필요하다 (client, 실행으로 확인)

`tools/codegen/ContractsCodegen.cs`는 현재 상태로 이 계약을 처리하지 못한다. architect가 스크래치 계약으로 실제 실행해 확인했다(2026-09-19).

**실제 `contracts/` 트리로는 실패가 3단 캐스케이드다** (client 실측 — architect의 초기 측정은 `maxItems`가 없는 스크래치 계약이었다). 하나 고칠 때마다 다음이 나오고, 전부 종료 코드 2다.

| 고친 것 | 다음 실패 |
|---|---|
| (없음) | `codegen: .../ships/maxItems: unsupported JSON Schema keyword` |
| `maxItems` allowlist | `codegen: .../ships/type: 'array' is not supported by the generator` |
| array 지원 | `codegen: SHIP_CLASS: kind 'data' is not supported by the generator yet` |

**마지막 것은 건너뛰기가 아니라 예외다** — 데이터 타입을 등록하는 순간 생성기 전체가 멈춘다. 그래서 **C1이 모든 클라이언트 작업의 맨 앞에 있다.** 필요한 변경 넷(client가 스크래치 사본으로 **전부 검증했고, 이 넷이면 충분하다**):

1. `type: "array"` + `items` → C# 배열. 원소가 객체면 중첩 클래스를 만든다(`title` 필수는 기존 규칙 그대로).
2. `maxItems`를 "제약(C# 출력에 영향 없음)" allowlist에 추가한다.
3. **`BuildProperties`의 한 줄** (초안 목록에 없던 것, client 발견): `shape.Nested != null`이면 속성 타입을 `Nested.ClassName`으로 쓰는데 **배열에서는 그게 틀리다** — 속성은 **배열**이고 중첩 클래스는 **원소**다. `CsType`이 있으면 그것을 우선하도록 고친다. **놓치면 `ships`가 배열이 아닌 `ShipState`로 나온다.**
4. `kind is "rest" or "data"`에서 **예외 대신 건너뛰고, 건너뛴 것을 stdout에 한 줄 남긴다**(조용한 건너뛰기는 p0-01이 경계한 형태다).
   - `SYNC_TUNING`의 소비자에 `client`가 있지만 **DTO를 생성하지 않는다.** 클라이언트는 그 파일을 `data/` 사본에서 직접 읽는다(ADR-0012 §7). 커버리지 스크립트는 코드에 `SYNC_TUNING` 또는 `SyncTuning` 문자열이 있으면 통과하므로 로더 클래스 이름이 그 역할을 한다.

## 6. 역사 연결

**이 슬라이스는 Historical Event를 하나도 만들지 않는다.** 판정 규칙·`rule_version`·중요도·가시성은 p2 범위다. designer도 같은 결론을 독립적으로 냈다(디자인 §7): `starfall-spec` 체크리스트는 새 Historical Event마다 "이 사건이 어떤 새 플레이를 만드는가"에 답할 것을 요구하는데, **"함선이 움직였다"에는 답이 없다.**

| 도메인 이벤트 | 중요도 | 지금 기록하는 이유 | 이 사건이 어떤 플레이로 이어지는가 |
|--------------|:---:|------|------------------|
| `SHIP_SPAWNED` | Level 0 (역사 아님) | 함선 엔티티가 **어디에서** 존재를 시작했다는 사실. `SESSION_OPENED`(행위자가 접속했다)와 다른 사실이다 — 캐릭터 ≠ 함선(GDD §4) | 함선의 생애 기록(GDD §6.4 Launch Date, Previous Owners)의 첫 행. 나중에 `SHIP_DESTROYED`나 `MINERAL_MINED`가 `ship_id`를 참조할 때, **그 id가 언제 어디서 생겼는지가 없으면 참조가 허공을 가리킨다.** "그 시각 그 함선이 이 성계에 있었는가"에 답할 수 있게 하는 1차 재료이고, 그 답이 **주장(Claim)의 반증 가능성**을 만든다 |
| `SHIP_DESPAWNED` | Level 0 (역사 아님) | 존재 구간의 끝과 **마지막 위치**. 메모리 월드가 함선을 버리는 순간, 그 함선이 어디 있었는지는 이 행 말고 어디에도 없다 | 사건 재구성의 첫 질문("그때 그 함선은 어디 있었나")에 답한다. `despawn_reason`은 지금 값이 둘뿐이지만, 전투가 들어오면 **"격추당했다"와 "조종사가 돌아오지 않았다"를 구분하는 원자적 사실**이 된다 — 현상금·수배(GDD §12)가 정확히 그 구분 위에 선다 |

**두 타입은 Historical Event로 승격되지 않는다.** 접속해서 함선을 받는 것은 사건이 아니다. 여기 적는 이유는 반대 방향의 실수를 막기 위해서다 — 중요도 판정기를 만들 때 "모든 도메인 이벤트에 레벨을 매긴다"고 접근하면 스폰 로그가 연대기에 올라간다. **판정기의 기본값은 판정하지 않음이어야 한다.**

**tick별 위치 이벤트를 기록하지 않는다.** designer가 독립적으로 같은 결론을 냈고 숫자를 냈다: 30척 × 20 Hz = **초당 600행, 시간당 216만 행**. 그리고 "3시간 전 좌표 (1234, 0, 5678)"은 **사실이지만 아무도 소비하지 않는다.** GDD §42가 경고한 "재미없는 데이터베이스"로 가는 가장 빠른 길이 이 지점이며, 막는 방법은 판단이 아니라 **표**다 — I-21에 따라 ADR-0007 §1의 표에 없는 것은 기록되지 않는다. 위치 이력이 필요해지면 그때 **표본화된 항해 일지**로 설계할 일이고 별도 슬라이스다.

**이번에 새로 확보하는 것 둘.**

1. **`causation_id`에 실제 값이 처음 들어간다**(I-30). p0-02는 전부 `null`이었고, 그래서 "원인 사슬"은 계약에만 있고 데이터에는 없는 필드였다. `SESSION_OPENED → SHIP_SPAWNED`는 인과의 가장 단순한 사례이고, **`SESSION_CLOSED →(30초 후) SHIP_DESPAWNED`는 더 값진 사례**다: 상관(같은 세션)과 인과(이것이 저것을 일으켰다)가 같은 행에서 **서로 다른 시각을 가리킨다.** 역사 그래프(HSE §23)가 나중에 읽을 간선의 첫 실물이고, 둘의 차이가 데이터로 보인다(HSE §102).
2. **"기록하지 않는다"는 결정이 표로 남는다**(ADR-0007 §1 개정).

**QA가 알아야 할 결과**: `SHIP_*`는 **세션당이 아니라 함선당 1쌍**이다. 재접속으로 이어 탄 함선은 세션이 2개여도 스폰이 1건이다. **짝짓기는 `ship_id`로 한다** — p0-02의 correlation 기반 대조 SQL을 그대로 쓰면 맞지 않는다.

## 7. 수용 기준

모든 Then은 실행 증거다. 실행하지 못한 항목은 PASS가 아니라 **"미검증(환경)"**이다. 증거에는 **몇 건을 검사했는지**가 드러나야 한다.

**실행 위치와 도구**: cargo 명령은 `server/`에서, 나머지는 레포 루트에서. HTTP 확인은 `curl.exe`. SQL 확인은 `docker compose exec -T postgres psql -U starfall -d starfall -At -c "..."`.

### 서버

- **AC-1 (server) 게이트와 결정성 위생.** When `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --locked`, Then 세 명령 모두 종료 코드 0. 그리고 `starfall-sim`의 `Cargo.toml`에 `axum`·`sqlx`·`redis`·`rand`·`chrono`·`tokio`가 **여전히 없고**, 해시 크레이트도 들어오지 않았다(ADR-0010 §4). 추가로 **`starfall-sim`의 소스에 ADR-0010 §3의 금지 함수 호출이 하나도 없다** — 특히 **`mul_add`(FMA)**·`hypot`·`to_radians`/`to_degrees`·`signum`이 목록에 포함된다. **검사는 두 줄을 나란히 보인다**: 순진한 grep이 오탐 13건(`.expect(`가 `exp`에 걸린다)을 내고, 메서드 호출 형태로 좁히면 0건임을(server 실측) 구현 요약에 적는다. **오탐 13 → 0을 함께 보이는 것이 "검사가 실제로 무언가를 검사한다"의 증거다.**
- **AC-2 (server) 데이터 테이블이 계약과 유도값을 강제한다.** (a) 정상 `data/` 3파일로 기동하면 `/debug/stats`에 로드된 함선 클래스 수·스폰 지점 수·유도된 `snapshot_interval_ticks`가 나오고 파일과 일치한다. (b)~(g) 다음이 **전부 기동 실패**이고 로그에 파일·필드·기대값이 나온다: `hard_boundary_radius_m = 20000.1`(스키마 상한), 스폰 `points_m` 빈 배열, `point_count ≠ len(points_m)`, `tick_hz / snapshot_hz`가 정수가 아님, `reconnect_resume_window_seconds > linger_seconds`, 스폰 지점이 하드 경계 밖, **`main_thrust_mps2 < lateral_thrust_mps2` 또는 `< reverse_thrust_mps2`**(대각선 클램프 상수가 최대값이 아니게 된다), `max_entities_per_snapshot > 64`, 함선 클래스 `id` 중복.
  (h) **데이터 디렉토리 해석**: `STARFALL_DATA_DIR`을 존재하지 않는 경로로 주면 **시도한 절대 경로를 찍고 기동을 거부**한다. 설정하지 않고 `server/`에서 `cargo run`, 레포 루트에서 바이너리 실행 **둘 다 성공**하며, 두 경우 모두 **해석된 절대 경로가 기동 로그와 `/debug/stats`에 같은 값으로 나온다**(I-38). **기동에 성공한 뒤 런타임에 발견되는 것은 실패다.**
  (i) **로그 소비자가 느려도 이 성질이 유지된다** *(2026-09-21 추가, architect R3. 실제로 터진 버그다: 기본 writer가 `stdout()`에 **이벤트를 낸 스레드에서 동기로** 쓰는데, 하네스가 파이프를 드레인하지 않아 4096 B가 차는 순간 tokio 워커가 전부 막히고 `shutdown`이 무효가 됐다. **2026-09-22 번호 정정**: 처음에 (h)로 적어 위의 기존 (h)와 겹쳤다. QA R3 리포트의 "AC-2(h-log)"가 이 항목이고 "AC-2(h-dir)"가 위의 (h)다.)* stdout을 **드레인하지 않는 파이프**로 서버를 띄우고, 로그가 파이프 용량을 넘기도록 부하를 준다. 그 뒤에도 `/healthz`가 계속 200이고 `shutdown` 종료 코드가 0이다. **판정 조건 셋을 함께 지킨다**: ① **파이프가 실제로 찼음을 단언한다**(로그 싱크가 버린 줄 수 > 0, 또는 미드레인 stdout 누적 바이트 ≥ 파이프 용량). 연결 횟수는 파이프를 채우는 수단이지 판정 조건이 아니다. ② **RED를 먼저 보인다**(수정 전 바이너리나 싱크를 끈 상태에서 이 항목이 실패함을 한 번 보인다). ③ **stderr는 드레인하거나 파일로 돌린다.** 서버가 stderr에 쓰는 것(패닉 메시지 등)이 있으면 교착이 그쪽으로 옮겨 갈 뿐이다. *(2026-09-22 정정: 처음 문구의 "버린 줄 통지가 그 경로로 나간다"는 사실이 아니었다. 통지는 **stdout에 in-band로** 나가고, 그쪽이 맞다. 줄이 빠진 바로 그 자리에 표시가 남고, stdout을 비우는 유일한 스레드가 두 번째 파이프에 막히는 경로가 생기지 않는다. 조건 ③ 자체는 유지된다.)* **이 성질은 `cargo test` 안에 존재할 수 없다.** 테스트는 tracing을 초기화하지 않고, `cargo test`는 stdout을 계속 읽는다. 그래서 프로세스 밖 경계(e2e)가 유일한 자리다(server 지적).
- **AC-3 (server) 스폰·잔류·재개·디스폰과 인과.** (a) 세션 1개를 열면 그 `correlation_id`에 `SESSION_OPENED`·`SHIP_SPAWNED` 각 1건이고 **`SHIP_SPAWNED.causation_id = SESSION_OPENED.event_id`**를 SQL 조인으로 확인한다. (b) 원인의 `(tick, sequence)`가 결과보다 작다. (c) `SHIP_SPAWNED`의 위치가 `data/`의 어느 스폰 지점과 **정확히 일치**하고, **잔류가 만료된 뒤**(> `linger_seconds`) 같은 `actor_id`로 다시 접속하면 새 `SHIP_SPAWNED`가 발행되며 **그 위치가 첫 번째와 같다**(난수 아님). *30초 안의 재접속은 스폰이 아니라 재개이고 (d)가 다룬다 — 그것을 (c)로 재면 재개된 함선의 현재 위치를 스폰 지점과 비교해 실패하거나 "같은 함선이니 같은 위치"라는 항진명제가 된다(server 지적, I-25).* (d) 연결을 끊고 잔류 창 안에 같은 `actor_id`로 재접속하면 **`ship_id`가 같고**, `SHIP_SPAWNED`가 **추가로 발행되지 않으며**, 재개된 함선의 상태는 끊기기 직전 상태에서 [이월 창 동안 마지막 실제 입력 → 그 뒤 휴면 입력]으로 이어진 것이다. 휴면 입력은 ADR-0011 §6.1의 표다. **`flight_assist=true`라 감쇠가 작용하고 오토레벨이 `q`·`ω_roll`을 바꾼다** — "그대로 두면 되는 값"으로 계산하면 틀린다. **판정은 두 층이고 층마다 기준이 다르다.** *(2026-09-22 정정. 옛 문구 "재개 직후 스냅샷이 끊기기 직전 상태에서 N tick 적분한 값과 양자화 정수로 같다, 허용 오차 없음"은 출발점이 스냅샷일 때 성립할 수 없다. 서버 상태는 `f64`이고 양자화는 메시지를 만들 때 한 번만 일어난다(ADR-0009 §5). 그래서 같은 스냅샷 정수로 반올림되는 서로 다른 `f64` 상태들이 N tick 뒤 서로 다른 정수를 낸다. QA 실측(R3 블록 5)으로, 같은 T0와 양립하는 상태들이 **122 tick 뒤 위치에서 최대 8 mm** 갈렸다. 서버 T1은 14필드 중 10필드가 정수까지 일치했고, 나머지 4필드도 봉투 안이었다. 정직한 서버도 만족시킬 수 없는 기준이었다 — ADR-0010 §3 "보장의 입력은 `f64` 상태다".)*
  - (d1) **`f64` 층 — 비트 일치, 허용 오차 없음 (server, in-process).** 같은 초기 상태와 같은 입력열로 두 실행을 돌린다. 하나는 끊고 재접속하고, 대조 실행은 끊기만 하고 재접속하지 않는다(재접속 간격은 이월 창보다 길게). 재개 tick부터 이후 매 tick, 함선의 `f64` 상태 `(p, v, q, ω_aim, ω_roll)`이 두 실행에서 **비트 단위로 같다** — 즉 재개는 물리에 아무것도 하지 않는다. 비교한 tick 수를 출력에 찍는다. 휴면 모델 자체의 독립 검증은 잔류 구간을 포함한 재생 데이터에서 C#과 비트가 일치하는 것으로 한다(AC-8·AC-12).
  - (d2) **와이어 층 — 양자화 봉투 (qa, 독립 계산).** 끊기기 직전 스냅샷 T0의 각 정수를 ±0.5 양자 범위에서 흔든 시작점 집합을 만든다. 거기서 같은 입력 규칙으로 N tick을 독립 적분한 결과의 [min, max] 안에, 재개 직후 스냅샷 T1의 14필드와 **잔류 중 관측자 행 전부**가 들어야 한다(N = 두 스냅샷의 envelope `tick` 차이). **음성 대조가 필수다**: 휴면 모델을 일부러 틀리게 한 계산이 봉투를 **벗어나지 않으면** 이 대조는 아무것도 검사하지 않은 것이다. 봉투는 표본으로 만든 **과소 추정**이다. 그래서 PASS는 그대로 유효하고, 봉투 밖 1 양자 이내의 FAIL은 표본을 늘려 재판정한 뒤에야 서버 FAIL로 적는다. **허용 오차를 고르지 않는다. 봉투는 양자화가 버린 정보에서 유도된다.** (e2) 재개 후 **첫 `SET_SHIP_CONTROL`(`input_seq = 1`)이 `ACCEPTED`이고** `ack_input_seq`가 1이 된다(I-43 — 거부되면 `last_applied_input_seq`를 초기화하지 않은 것이다). (e) 재접속하지 않으면 `linger_seconds` 뒤에 `SHIP_DESPAWNED{LINGER_EXPIRED}` 1건이 나오고 **`causation_id`가 그 잔류를 시작시킨 `SESSION_CLOSED.event_id`**다(조인 확인). (f) 잔류 중 정상 종료하면 `SHIP_DESPAWNED{SERVER_SHUTDOWN}`이 나온다. (g) 전 구간에서 **`ship_id`별 `SHIP_SPAWNED` 1건 / `SHIP_DESPAWNED` 1건**이다(I-41). (h) **같은 actor의 동시 세션** *(2026-09-22 추가, QA R3 §6.7)*. 같은 토큰으로 세션 두 개를 겹쳐 연다. 그러면 **어느 시점에도 그 actor의 함선은 1척**이고(함선 수명 구간의 겹침 0), 검사 구간 전체에서 **`causation_id = event_id`인 행이 0건**이다. 모든 세션을 닫고 잔류 창이 지나면 `ships_active = 0`이다(조종 세션 없는 `ACTIVE`가 남지 않는다). 그 상태로 종료하면 `SHIP_DESPAWNED{SERVER_SHUTDOWN}` 전부의 원인이 **실제 `SESSION_CLOSED`**다. (g)의 짝 검사만으로는 이 결함이 보이지 않는다 — 두 함선 모두 짝은 맞기 때문이다. **in-process 재현(server)을 먼저 둔다**: 같은 actor의 `OpenSession` 2건 → 닫기 → 만료 → 종료를 돌리며 매 tick I-29의 1:1을 검사하고, **수정 전 코드에서 이 테스트가 실패함을 먼저 보인다**(RED). **넘겨받기 단언(사용자 결정 5)**: 둘째 세션이 열린 tick에 ① `SESSION_OPENED(S2)` 다음 sequence에 `SESSION_CLOSED(S1, SUPERSEDED)` 1건이 있다. ② 그 행의 `causation_id = SESSION_OPENED(S2).event_id`이고 `correlation_id`는 S1의 것이다(SQL 조인). ③ `SHIP_SPAWNED`가 추가로 나오지 않고, S2의 `SESSION_READY` 뒤 첫 스냅샷에서 `controlled_ship_id`가 **S1이 조종하던 `ship_id`와 같다**. ④ 첫 연결이 **close code 4001**을 받는다. ⑤ `SUPERSEDED`가 아닌 `SESSION_CLOSED`의 `causation_id`는 여전히 null이다. **순서의 반대 경우도 본다**: S1의 닫기가 S2의 열기보다 먼저 제출되면 넘겨받기가 아니라 잔류 → 재개이고 `SUPERSEDED`가 없다(in-process, 제출 순번을 통제해서). 클라이언트 쪽 단언은 AC-13 계열에 붙는다 — 4001을 받은 클라이언트는 **재연결을 시도하지 않고**, grep할 수 있는 로그 한 줄을 남긴다(client). **이 단언의 S1은 Unity 클라이언트여야 한다**(QA 지적, 2026-09-22). 봇은 원래 재연결하지 않으므로, 봇을 S1로 두면 "재연결하지 않았다"는 항상 참이 되어 아무것도 검사하지 않는다. 실행 검증은 Unity를 S1로 한 e2e(QA 블록 6)이고, 단위 수준은 client EditMode 테스트다(4001이면 재연결 예약 없음 / 다른 코드면 기존 백오프). 두 증거를 함께 쓴다.
- **AC-4 (server) 적분과 서버 판정.** 수동 step 모드의 통합 테스트에서(tokio·소켓 없이), (a) **`thrust_z_milli = 1000`을 항등 자세에서 적용하면 위치의 `z`만 증가한다**(ADR-0009 §1의 축 고정), 그리고 T tick 후 값이 ADR-0010 §2로 손계산한 값과 **양자화 후 정수로 일치**한다. (b) 최대 추력을 오래 넣어도 `|v|`가 `max_speed_mps`를 **넘지 않는다**. (c) 세 축 전부 1000인 입력의 로컬 가속 크기가 `main_thrust_mps2`를 넘지 않는다(대각선 클램프). (d) soft 경계를 넘으면 원점 방향 가속이 더해지고 **조작은 계속 먹으며**, hard 경계에서 반경 속도 성분이 0이 되고 **접선 성분은 남는다**. (e) 조작을 멈추고 이월이 만료되면 가속이 0이 되지만 **속도는 감쇠만 적용되어 즉시 0이 되지 않는다**. (f) 브레이크 중에는 추력이 무시되고 감쇠가 **하나만** 적용된다. (g) 목표 자세를 정반대로 주면 **오버슈트 없이 안착**한다 — 정의: (1) 매 tick `|ω_aim| ≤ turn_rate_max_deg_s`, (2) 자세 오차 `s = |vec(q_err)|`가 `turn_deadzone_sin_half` 아래로 내려간 뒤 **40 tick 동안 다시 올라가지 않는다**, (3) **안착까지의 tick 수를 출력에 찍는다**(U-21이 이 수치로 확인된다). 그리고 (g2) **오토레벨이 켜진 상태에서 목표 자세를 유지할 때 자세가 진동하지 않는다** — ADR-0010 §2의 롤 축 권한 분리가 동작하는지를 보는 항목이다. (h) 퇴화 쿼터니언(전 성분 0)은 거부되지 않고 **현재 자세 유지 + `aim_degenerate_total` 증가**다(I-39).
- **AC-5 (server) 클라이언트가 보낸 위치를 서버가 쓰지 않는다.** (a) `position-field-injected.json`과 `attitude-field-injected.json`을 그대로 소켓으로 보내면 `COMMAND_RESULT{REJECTED, MALFORMED_COMMAND}`가 오고 **그 세션 함선의 상태가 그 tick에 바뀌지 않는다**(스냅샷으로 확인). (b) 서버 코드 어디에도 명령 payload에서 위치·속도·현재 자세를 읽는 경로가 없음을 타입으로 보인다 — `SetShipControlPayload`의 필드 목록이 곧 증거다. (c) 조작 값이 범위를 벗어난 명령은 클램프되지 않고 **거부**되며, **그 tick에는 직전 입력이 이월되어** 함선이 계속 정상 이동한다(I-33의 안전성 근거). (d) `aim_*`를 극단값으로 계속 보내도 한 tick의 회전량이 `turn_rate_max_deg_s × dt`를 넘지 않는다 — **목표 자세는 채택되지 않고 목표로만 쓰인다**(I-26).
- **AC-6 (server) 빨리 보내도 더 가지 못한다 (속도 핵).** Given 같은 조작을 두 봇이 보내되 하나는 `client_send_hz`, 하나는 **10배 속도**로, When 30초 유지, Then (a) 두 함선의 이동 거리 차이가 **한 tick 이동분 이내**다, (b) 빠른 쪽은 `RATE_LIMITED` 거부를 받고 **연결은 유지**되며, **보낸 명령 수 == 받은 `COMMAND_RESULT` 수 + `commands_dropped_over_tick_cap_total`**이다(I-47 — tick당 상한을 넘은 프레임은 응답이 없고 카운터로만 드러난다), (c) `input_superseded_total`이 증가한다, (d) **`close_reason`이 `SLOW_CONSUMER`가 아니다** — 몰아 보내는 클라이언트는 읽기를 멈춘 적이 없으므로 그 사유로 끊기면 진단이 거짓이다. 규약을 계속 어겨 끊긴다면 사유는 `PROTOCOL_VIOLATION`이어야 한다. **판정 근거는 "코드에 방어가 있다"가 아니라 두 거리의 차이다.** 그리고 **봇이 실제로 보낸 명령 수**를 세어, 빠른 봇이 자기 CPU나 소켓에 먼저 막힌 것이 아님을 보인다.
- **AC-7 (server) 스냅샷.** (a) 열려 있는 모든 세션이 `snapshot_interval_ticks`마다 정확히 1건을 받고, 연속 두 스냅샷의 envelope `tick` 차이가 그 값과 같다. (b) `ships`에 **월드의 모든 함선**(잔류 포함)이 `ship_id` 오름차순으로 있다. (c) 각 수신자의 `controlled_ship_id`가 자기 함선이고 `ships` 안에 있다. (d) `ack_input_seq`가 그 세션에 대해 **서버가 마지막으로 적용한 `input_seq`**이고, 적용 전에는 `null`이며 **되돌아가지 않는다.** 한 tick에 `(5, 3)` 순서로 도착하면 후보는 5뿐이므로 5가 적용되고 3은 `STALE_INPUT`이다(ADR-0011 §4). (e) 잔류 함선의 `presence`가 `LINGERING`이고, 디스폰된 다음 스냅샷부터 `ships`에서 사라진다. (f) `/debug/stats`에 `snapshots_sent_total`·`snapshot_bytes_total`·`send_queue_bytes`(현재/최대)·`input_superseded_total`·`input_carried_forward_total`·`aim_degenerate_total`이 있고 동작한다.
- **AC-8 (server) 결정성.** Given 고정된 초기 상태와 **파일에 기록된 입력열**(함선 3척, 600 tick, 추력·선회·브레이크·경계 접촉·잔류를 포함), When 같은 실행을 **서로 다른 프로세스에서 2회** 돌려 매 tick의 스냅샷 payload를 직렬화해 저장, Then 두 산출물이 **바이트 단위로 동일**하다(I-37). 비교한 tick 수와 총 바이트를 출력에 찍는다. **근사 비교(허용 오차)를 쓰지 않는다** — 허용 오차는 이 테스트가 잡아야 할 차이를 정확히 덮는다.
- **AC-9 (server) 계약 테스트.** When `cargo test -p starfall-contracts --locked`, Then ADR-0002 §3의 테스트 9종이 **타입 13종 전부**를 덮고 통과한다. 특히 (a) 유효 fixture **27건**이 왕복 후 동일 *(2026-09-22: 26 → 27, `SESSION_CLOSED/superseded.json`. 계약 적용 직후 실측으로 이 항목은 **예상대로 RED**다: `fixtures_roundtrip`이 `unknown variant SUPERSEDED`로, `registry_consistency`가 하드코딩된 26으로 실패한다. 둘 다 server가 S-6에서 고친다)*, (b) 반례 **34건이 스키마 검증에서 전부 거부**, (c) 반례 34건의 Rust 역직렬화 결과가 §5.4 + 선행 스펙 표의 "Rust serde" 열과 일치, (d) `required` 제거 변이가 전부 실패, (e) `server`가 태그된 타입 13종이 이름→Rust 타입 대응표에 있다. **테스트가 검사한 건수(27/34/...)를 출력에 찍는다.**

**(a)의 비교 방식을 `kind`로 나눈다 (U-20 해소, server B-2).** server 실측: 긴 소수(`2165.0635095`)는 **완벽히 왕복한다.** 깨지는 것은 `"type": "number"` 필드에 **정수 리터럴**이 적힌 경우다 — `serde_json::Number`가 `PosInt`와 `Float`를 다른 값으로 보므로 `192`가 `192.0`으로 되돌아와 `Value` 비교가 실패한다(해당 필드 실재: `egress_budget_kib_s_per_session: 192`, `remote_interp_delay_ms: 200`, `linger_seconds: 30`, `cargo_capacity_t: 20`).

| kind | 비교 |
|------|------|
| `command`·`server_message`·`domain_event` | **지금 그대로 `Value` 엄격 비교.** 와이어 드리프트를 잡는 것이 이 테스트의 존재 이유다 |
| `data` | 비교 직전 양쪽 `Value`를 재귀적으로 정규화(모든 `Number`를 `as_f64()`로)한 뒤 **정확히(`==`) 비교.** 근사 비교가 아니다 — `192` vs `192.5`는 여전히 실패한다 |

데이터 테이블은 와이어를 왕복하지 않으므로 요구사항은 **바이트 동일이 아니라 값 동일**이다. `192`와 `192.0`이 다른 값이라는 것은 JSON의 사실이지 우리 계약의 사실이 아니다. **fixture에 `192.0`을 쓰게 하는 대안은 버렸다** — designer가 `30`이라고 타이핑할 때마다 테스트가 깨지고 실패 메시지가 이유를 설명하지 못한다. 규율을 사람이 아니라 비교기에 지운다.

(f) **이 정규화가 놓치는 것을 별도 단언으로 막는다**: `"type": "integer"` 필드의 `10` ↔ `10.0` 드리프트도 함께 통과하므로, **integer로 선언된 필드를 Rust가 정수 타입(`u32`/`i64`)으로 받는다**는 것을 단언한다(정수 타입이면 `10.0`은 역직렬화에서 이미 거부된다).

### 클라이언트

- **AC-10 (client) 생성기 확장.** (a) 확장 **전에** 현재 `contracts/`로 생성기를 돌려 실패함을 먼저 보인다 — §5.7의 **3단 캐스케이드**(`maxItems` → `array` → `kind 'data'`)를 전부 기록한다. 첫 실패는 `maxItems`이지 `array`가 아니다. (b) 확장 후 생성 명령을 **두 번** 실행하면 두 번째 후 해시가 변하지 않고 `--check`가 종료 코드 0. (c) `WorldSnapshotMessage`의 `ships`가 배열이고 `ShipState`가 중첩 클래스로 생성되며 **18개** 속성이 전부 있다(B-1의 `angular_velocity_roll_mdeg_s` 포함). (d) `SHIP_CLASS`·`STAR_SYSTEM`·`SYNC_TUNING`에 대한 `.cs`가 **생성되지 않는다**(건너뛰기이지 실패가 아니다). (e) 확장 전후 생성물 diff를 떠서 **기존 6타입의 타입별 파일 6개가 바이트 동일**하고, **`ContractTypes.cs`의 변경이 신규 항목 추가 8줄뿐**임을 보인다. *`ContractTypes.cs`도 생성물이고 신규 4타입이 레지스트리 맵에 들어가므로 반드시 늘어난다 — "한 글자도 바뀌지 않았다"는 통과 불가능한 문구였다(client R3).*
- **AC-11 (client) EditMode.** When `mkdir -p _workspace/p1-01-ship-movement/unity-tests` 후 `unity test client --mode EditMode --report-format nunit,junit --output _workspace/p1-01-ship-movement/unity-tests/EditMode.nunit.xml --junit-output _workspace/p1-01-ship-movement/unity-tests/EditMode.xml`, (`unity test`가 없는 출력 디렉토리를 만들어 주는지는 확인된 적이 없다 — client가 C2에서 실측해 T10에 적는다.) Then 종료 코드 0, 실패 0, 두 리포트 파일 존재, **`tests` 수가 0이 아니고 그 수를 구현 요약에 적는다.** 포함: (a) 유효 fixture **27건을 로더가 발견하고, 그중 계약 메시지 21건이 `Strict` 왕복을 통과한다** *(2026-09-22: 26/20 → 27/21. `SESSION_CLOSED`는 DTO가 있어 왕복 대상에 들어간다고 예측했다 — client가 실측으로 확인한다)*. *두 숫자가 각각 "fixture가 사라지지 않았다"(27)와 "왕복을 빠뜨리지 않았다"(21)를 지킨다. 데이터 6건은 AC-10(d)가 `.cs`를 만들지 않기로 했고 envelope 판별자조차 없으므로 왕복할 대상이 없다 — client R1.* (b) §5.4의 C# 열이 **실측으로 채워졌고**(2026-09-20, 정정 0건) 그 결과를 상수로 박는다(거부 **18** / 통과 10 / 계층 없음 **6**), (c) fixture 로더가 유효 fixture를 **26건 미만** 발견하면 실패, (d) `ships`가 빈 배열인 스냅샷과 2척(하나는 LINGERING)인 스냅샷이 모두 왕복한다, (e) `Runtime` 프로필이 `ShipState` 안의 모르는 필드를 무시하고 경고를 남기며 같은 입력이 `Strict`에서는 예외다, (f) **클라이언트가 가진 `data/` 사본이 레포의 `data/` 원본과 내용이 같다**(파일 복사 방식의 유일한 방어 — ADR-0012 §7).
- **AC-12 (client) 예측·재조정이 서버와 같은 결과를 낸다.** AC-8이 만든 입력열과 tick별 스냅샷을 **테스트 자산으로 가져와서**, When 클라이언트 예측 코어에 같은 입력열을 넣고 각 스냅샷 시점에 **서버 상태와 대조**(S6 재생에서는 재조정을 거치지 않고 **열린 고리로 적분해** 대조한다 — 아래 주석), Then (a) 대조 시점의 위치 오차가 **각 스냅샷마다** `reconcile_ignore_threshold_m` 이하이고 p99·최대값과 **비교 지점 수**를 출력에 찍는다 *(비교가 가능한 지점은 스냅샷이 도착한 tick뿐이다 — 그 사이 tick에는 대조할 서버 값이 없다. client R7)*, (b) 자세 오차가 `reconcile_orientation_ignore_threshold_deg` 이하, (c) 재조정이 **순수 함수**임을 같은 입력 2회 실행의 동일 결과로 보인다, (d) `input_seq`를 하나 건너뛴 입력열에서도 재조정 후 상태가 서버 상태와 같다, (e) **각속도까지 되돌리지 않으면 실패하는 케이스**(선회 중 스냅샷)가 포함되어 있다 — `ω_aim`과 `ω_roll`을 **스냅샷의 두 필드에서 각각** 받아야 통과하고, 합에서 분해하면 실패한다(ADR-0010 §1.1), (f) **같은 대조 지점에서 각속도 두 필드도 서버 값과 대조한다** — 롤 구간의 적어도 한 지점에서 **서버가 보낸 `ω_aim`과 `ω_roll`이 둘 다 0이 아님**을 단언한다. *이것이 wire→sim 변환이 `angular_velocity_roll_mdeg_s`를 떨어뜨리는 경우를 잡는 유일한 관찰이다 — 위치·자세만 대조하면 확인 상태의 각속도를 읽고도 쓰지 않으므로 그 버그가 빠져나간다(architect R2).* 각속도 오차 분포는 출력에 찍되 **임계값은 이번 슬라이스에서 정하지 않는다**(해당 튜너블이 없다 — 관측부터 한다).

> **해소됐다 (S8 완료, 2026-09-20).** 초안 fixture에는 롤 입력이 없어 모든 tick에서 `ω_roll = 0`이었고, 그러면 잘못된 분해도 통과했다. S8이 `server/crates/sim/tests/data/replay/`에 **롤 구간(tick 460~500)**을 넣어 `ω_aim`과 `ω_roll`이 **동시에 0이 아닌 스냅샷**을 만들었고, 그것을 단언하는 테스트가 **RED → GREEN으로 확인**됐다. **AC-12(e)의 "미검증" 고정을 해제한다 — 정상 판정 대상이다.** 판정 조건 하나를 명시한다: 그 단언이 **같은 스냅샷 안에서** 두 값이 모두 0이 아님을 확인해야 한다(따로따로면 잘못된 분해가 다시 빠져나간다). **이 AC가 ADR-0010 §3의 "같은 비트" 가정을 실제로 검증하는 유일한 지점이다** — 초과하면 그것이 결과이고, **임계값을 늘려 통과시키지 말고 architect에게 알린다.**

> **검증 경로 정정 (2026-09-20, architect R2). (a)(b)가 재는 것은 "재조정 직전 오차"가 아니라 "열린 고리 적분 오차"다.**
>
> S6 fixture는 600 tick에 조작 명령이 **6건뿐**이고 나머지는 서버 이월로 채워진다(`carry_forward_max_ticks = 10_000`으로 만료를 일부러 막았다 — AC-8이 이월 경로를 타게 하려는 의도적 설정이다). 그런데 **운영 분포는 정반대다**: `client_send_hz = 20 = tick_hz`이고 designer 노트가 "**one input per tick**"이라고 못박았으며, `carry_forward_max_ticks = 10`(500 ms)은 **딸꾹질 흡수용이지 정상 경로가 아니다.** `Reconciliation.Reconcile`의 히스토리는 그 운영 분포대로 **입력 1건 = tick 1개**를 기록한다.
>
> 그래서 이 fixture에 `Reconcile()`을 그대로 걸면 **재는 값이 물리가 아니라 장부 불일치가 된다**: 첫 스냅샷에서 `history[S].StateAfter`는 1 tick분인데 확인 상태는 N tick분이라 오차가 미터 단위로 뜨고, 그 다음부터는 4단계가 `seq ≤ S`를 버려 매칭이 사라져 **나머지 ~293개 지점이 `HasError = false`(잰 것 없음)로 떨어진다.** 임계를 넘든 안 넘든 **두 적분기가 같은가**에 대한 답이 아니다.
>
> **그래서 client가 택한 경로(확인 상태로 되감지 않고 `ShipIntegrator.Step`만으로 600 tick을 끝까지 적분해 매 스냅샷과 대조)를 채택한다.** 이 경로가 재는 것은 **누적 표류**이고, 재조정이 매 스냅샷 되감는 경로보다 **엄격하다** — 재조정을 끼우면 오차가 `snapshot_interval_ticks`(2 tick) 창으로 리셋되기 때문이다. `reconcile_ignore_threshold_m`(0.005 m)·`reconcile_orientation_ignore_threshold_deg`(0.02°)를 그대로 쓰는 것은 **보수적인 재사용**이다(실측 여유 7.4배·215배). 임계를 넘으면 규칙은 그대로다 — **늘리지 말고 architect에게 알린다.**
>
> **이 정정이 남기는 구멍과 그 처리:** `Reconcile()` 자체는 이제 **실서버 산출물을 만나지 않는다**(SC-53·54·55는 합성 입력이다). 그 구멍은 fixture를 고쳐 메우지 않는다 — fixture를 조밀하게 만들면 (a)(b)가 2 tick 창만 재게 되어 **더 약해진다.** 대신 **AC-13(실서버 60초)이 메운다**: 거기서는 입력 분포가 정의상 운영과 같다. AC-13(b)에 관찰 3건을 추가한다 — ① `Reconcile()` 호출 중 **`HasError == true`가 1회 이상**(0이면 그 경로가 실서버 데이터로 한 번도 타지 않은 것이다), ② **3단계에서 재생한 입력 수가 0이 아닌 호출이 1회 이상**(되감기만 하고 재생은 안 한 것과 구분된다), ③ **`ω_aim`·`ω_roll`이 둘 다 0이 아닌 스냅샷에서 재조정한 횟수 ≥ 1**(SC-55가 실서버 데이터에서도 성립하는지를 보는 유일한 관찰).

- **AC-13 (client) 실서버 왕복과 오차 분포.** Given 서버와 인프라가 떠 있음, When Unity 클라이언트가 접속해 60초간 조작(직진·선회·브레이크·정지·경계 접근 포함), Then (a) `SESSION_READY` → 첫 `WORLD_SNAPSHOT`이 오고 그 안에 자기 함선이 있다, (b) 재조정 직전 위치 오차의 p50/p99/최대를 HUD와 로그에 남긴다 — **그리고 `Reconcile()`이 실제로 탔음을 같은 로그에 남긴다**: ① `HasError == true`인 호출 수(**0이면 FAIL** — 재조정 경로가 실서버 데이터로 한 번도 타지 않은 것이다), ② 3단계에서 재생한 입력 수가 0이 아닌 호출 수(되감기만 한 것과 구분된다), ③ `ω_aim`·`ω_roll`이 **둘 다 0이 아닌** 스냅샷에서 재조정한 횟수(**≥ 1**). *AC-12의 S6 재생이 `Reconcile()`을 거치지 않게 됐으므로(위 주석), 실서버가 그 경로의 유일한 실데이터 관찰 지점이다 — architect R2,* (c) **`reconcile_hard_snap_total`이 0이다**(로컬 링크에서 0이 아니면 버그 신호다 — 디자인 §6.4), (d) 정상 종료 후 잔류 창이 지나면 DB에 그 함선의 `SHIP_DESPAWNED{LINGER_EXPIRED}`가 있다, (e) **스냅샷 1건당 할당량과 초당 할당량**을 Unity Profiler로 재서 숫자를 남긴다(U-16, p0-02 U-10의 만기). *스냅샷은 10 Hz이고 렌더는 60~144 fps이므로 "프레임당"으로 재면 6~14프레임 중 한 프레임에만 할당이 뜨고 평균이 실제 스파이크를 감춘다 — client R5.* 실행할 수 없으면 **미검증(환경)**.

> **판정 전에 `close_reason`을 먼저 본다.** 세션 송신 큐 64는 **2.13초 분량**이다(ADR-0011 §5). Unity Editor의 도메인 리로드·GC 히치가 2초를 넘으면 서버가 그 연결을 **정당하게** `SLOW_CONSUMER`로 끊는다. 그것은 서버 버그가 아니라 측정 환경의 사실이고, 그 경우 재측정한다.
- **AC-14 (client) 눈으로 확인한다.** Given 그레이박스 씬, When 사람이 조작, Then (a) 전방 추력이 함선을 **뱃머리 방향으로** 보내고, 마우스를 오른쪽으로 움직이면 함선이 **오른쪽을 향해 돌며**, 위 방향 추력이 **위로** 민다 — ADR-0009 §1의 부호 규약이 화면에서 참임을 스크린샷·짧은 녹화로 보인다. (b) 기준 마커 4개가 보이고 그것 대비 이동이 읽힌다. (c) 경계 경고가 soft 경계에서 뜨고, hard 경계에 닿으면 미끄러진다. (d) 오토레벨이 손을 떼면 수평을 되찾고, 수동 롤 중에는 동작하지 않는다. **(a)가 이 슬라이스에서 부호 버그를 잡는 유일한 장치다** — 문서와 테스트는 부호가 통째로 뒤집혀도 일관되면 통과한다.
- **AC-15 (client) 타 함선.** (a) 두 번째 접속의 함선이 화면에 나타나고 움직인다. (b) 그 함선은 **예측되지 않고 보간된다** — **자세도 slerp로 보간된다**. 검증은 **`t = 0.25`(비대칭 지점)에서 자세 차가 60° 이상인 쌍**으로 한다. *`t = 0.5`는 slerp와 nlerp가 정확히 같은 값을 내는 대칭점이라, 자세를 선형 보간하고 정규화만 한 구현이 통과한다 — 검증하려던 실수의 사촌을 놓친다(client R6). 60° 차이는 실제 스냅샷 쌍(최대 7.5°)에서 나오지 않으므로 C5가 자산을 합성한다.* (c) 스냅샷이 끊기면 `remote_extrapolate_max_ms`까지만 외삽하고 그 뒤 **정지**한다(날아가지 않는다). (d) 그 세션이 끊기면 함선이 `LINGERING`으로 계속 보이고, 디스폰되면 다음 스냅샷에서 제거된다. (e) 보간 지연이 `snapshot_interval_ticks`와 `sync-tuning`에서 계산되며 그 값을 로그에 찍는다.

### QA

- **AC-16 (qa) A가 움직이면 B가 본다.** Given 클라이언트 2개가 접속, When A만 30초간 전방 추력을 넣고 B는 아무 입력도 보내지 않는다, Then (a) **B가 받은 스냅샷 안의 A의 위치**가 단조적으로 변하고 총 이동 거리가 `max_speed_mps`와 시간으로 설명되는 범위 안이다, (b) B의 함선은 스폰 위치 근처에 머문다, (c) 같은 tick의 스냅샷에서 **A가 본 자기 위치와 B가 본 A의 위치가 양자화 정수로 완전히 같다**, (d) 검사한 스냅샷 쌍의 수를 리포트에 적는다. **이것이 "여러 클라이언트가 같은 월드를 본다"의 실증이다.**
  > **(c)는 항진명제에 가깝다 — 리포트에 그 사실을 적는다(I-25).** 두 값은 같은 `ships` 배열에서 나오므로 정상 구현에서는 다를 수 없다. (c)가 잡는 것은 **세션별 직렬화가 배열을 다르게 만드는 버그** 하나뿐이고, 진짜 검증은 (a)와 (b)다.
- **AC-17 (qa) 치트 시나리오 전수.** 각 항목마다 "봇이 시도한 횟수"와 "서버가 막은 횟수"를 적는다. (a) 위치·현재 자세 필드 주입 → 전부 `MALFORMED_COMMAND`, 상태 변화 0. (b) 조작 값 범위 초과 → 전부 거부, 클램프 흔적 없음, **그 tick에 이월이 동작해 조작이 끊기지 않음**. (c) 10배 폭주 → AC-6. (d) `input_seq` 역행·반복 → `STALE_INPUT` 거부, `ack_input_seq`가 되돌아가지 않음. (e) 다른 `ship_id`를 조작하려는 시도 → **불가능함을 보인다**: 명령에 `ship_id` 필드가 없으므로 시도할 방법 자체가 없다. 리포트에 "어휘에 없어 시도 불가"로 적고, 그것이 코드 검사보다 강한 보장임을 명시한다. (f) 다른 actor의 잔류 함선을 가로채려는 재접속 → 자기 `actor_id`의 함선만 돌아온다. (g) `SESSION_READY` 이전에 보낸 `SET_SHIP_CONTROL`이 함선을 만들거나 움직이지 않는다.
- **AC-18 (qa) 31 연결 부하와 대역폭 실측.** p0-02의 A~D 단계를 갱신한다(A: 31 연결 60초, 각 봇이 `client_send_hz`로 조작 전송. B: 세션 회전 5회 — **잔류·재개 경로를 포함**. C: 1개 폭주. D: DB 30초 중단). Then (a) 명령 손실 0, `COMMAND_RESULT` 1:1(측정 창 델타 기준), (b) 스냅샷 손실 0(봇이 센 수신 수 == `snapshots_sent_total` 델타), (c) **실제 송신 바이트를 봇 쪽에서 독립으로 측정**해 ADR-0011 §2의 산출(31 연결 세션당 **161.8 KiB/s**, 합계 약 **42.5 Mbit/s**)과 대조하고 **차이가 20 %를 넘으면 ADR의 표를 고친다.** **독립 출처가 성립하려면 `snapshot_bytes_total`이 소켓에 실제로 쓴 바이트여야 한다** — 큐에 넣은 시점에 세면 드롭·잘림이 양쪽에서 같이 사라져 독립이 아니게 된다(server 지적, p0-02의 `messages_written_total`과 같은 자리에서 센다), (d) 세션당 송신이 `egress_budget_kib_s_per_session` 아래임을 확인한다, (e) `send_queue_bytes` 최대와 용량 대비 비율, 그리고 **`close_reason = SLOW_CONSUMER` 발생 수**(정상 부하에서 0이어야 한다. 0이 아니면 용량이 아니라 소비자가 문제다)를 기록한다, (f) DB 중단 구간에도 **스냅샷은 계속 흐른다**(이동은 DB에 의존하지 않는다 — I-31의 부수 증거).
- **AC-19 (qa) 성능 회귀.** p0-02 기준선과 같은 표로 나란히 적는다: tick 초과 비율(`run_tick` 본문 > 50 ms), 단일 tick 본문 최대·p50·p99, 왕복 p50·p99, 큐 최대, 서버 RSS. **기준선: 초과 0.000 % / 본문 최대 21.65 ms / 왕복 p99 50.4 ms / RSS 16.6 MB.** 게임 로직과 브로드캐스트가 들어갔으므로 **tick 본문 소요가 늘어나는 것은 회귀가 아니라 예상**이다. 판정하는 것은 하나다: **tick 초과 비율 ≤ 0.5 %**. 나머지는 기록이고, 눈에 띄게 나빠진 항목에는 원인 가설을 적는다. **`tick_body_us`를 그대로 p0-02와 비교하지 않는다** — 스냅샷이 2 tick마다라 본문 소요가 이봉분포가 되어 p50은 낮은 쪽만, p99는 높은 쪽만 본다. **`snapshot_build_us`를 분리해 "스냅샷 조립 X µs, 나머지 Y µs"로 적는다**(server B-14). Windows 타이머 바닥값(+0.7 %/분)을 `tick_lag_seconds` 옆에 나란히 적는다. 측정 환경(Unity Editor 실행 여부, 동시 컨테이너 수)을 반드시 적는다.
- **AC-20 (qa) 기록 무결성.** (a) 수집한 `ship_id` 집합에 대해 `SHIP_SPAWNED` 수 == `SHIP_DESPAWNED` 수 == 집합 크기, 짝 없는 이벤트 0, 중복 0(**`ship_id` 기준 — correlation 아님**, I-41). (b) `SESSION_OPENED`/`SESSION_CLOSED`는 p0-02 그대로 correlation 기준으로 검사한다. (c) 재개가 일어난 함선에 대해 **세션 쌍은 2개인데 스폰은 1건**임을 보인다. (d) `(world_id, tick)`별 `sequence`가 0..n−1로 빈틈없음(p0-02 AC-16(c)의 SQL 그대로). (e) **`domain_events`에 위치 시계열이 없다**: **이번 실행의 tick 구간으로 한정한** `event_type` distinct가 이 슬라이스의 4종(`SESSION_OPENED`·`SESSION_CLOSED`·`SHIP_SPAWNED`·`SHIP_DESPAWNED`)뿐이고 **위치·상태 시계열을 뜻하는 타입이 하나도 없음**을 보인다(I-31·I-21). *초안의 "6종"은 어떤 셈으로도 맞지 않았고, 이 PC의 DB에는 p0-02가 정당하게 남긴 `QA_APPEND_ONLY_PROBE` 행이 1건 있어 전수 distinct는 5종을 반환한다(server 실측). 전수로 재려면 측정 전에 `docker compose down -v`를 게이트로 둔다.* (f) 모든 `SHIP_*`의 `causation_id`가 비-null이고 실제 이벤트를 가리킨다(조인 0행 누락).
- **AC-21 (qa) 계약 커버리지와 경계면 교차 검증.** (a) When `python .claude/skills/integration-qa/scripts/check_contract_coverage.py --strict`, Then 종료 코드 0(errors 0, warnings 0). **구현 전 기준선(architect 실행, 2026-09-19): errors 14, warnings 0** — 신규 7타입의 코드 참조 없음 14건. 이 14가 0이 되는 것이 통과 조건이다. (b) 신규 7타입의 스키마·Rust 타입·C# DTO를 **필드별 표**로 나란히 비교한다(총계가 아니라 표가 근거다). 열거 규약: envelope 필드 전부(`payload` 컨테이너 포함) + payload 자체 필드, 배열 원소 타입은 별도 표로. 데이터 3종은 C# 열이 없다(생성 안 함). 불일치 건수(0이어야 한다)와 대조한 행 수를 적는다. (c) fixture의 `world_id`↔`tick_hz` 조합이 I-19와 모순되지 않는지 확인한다. (d) **`data/` 실제 3파일이 스키마를 통과하는지 재확인한다** — architect 실측은 `sync-tuning.json`이 6건 실패였다(§5.6).

## 8. 비기능 요구

- **동시 연결 31**(봇 30 + Unity 1)이 이번 슬라이스의 상한이다(원칙 8). 관심 영역 필터링·델타의 도입 트리거는 **동시 함선 37척**으로 ADR-0011 §2에 숫자화되어 있다(B-1의 각속도 1필드 추가로 40 → 37이 됐다. 트리거를 정하는 것은 함선 수가 아니라 **예산**이므로 계약이 커지면 함께 당겨진다).
- **대역폭 (ADR-0011 §2 산출, fixture 실측 바이트 기반)**: `ShipState` **523 B**, 31척 스냅샷 **16,566 B**. 세션당 10 Hz = **161.8 KiB/s**(designer 예산 192 KiB/s의 84 %), 서버 송신 합계 약 **42.5 Mbit/s**(5.3 MB/s), 수신 약 1.8 Mbit/s. **N²로 자란다** — 100 연결이면 422 Mbit/s다. AC-18(c)가 독립 출처로 대조한다.
- **tick 본문 예산**: 스냅샷 **직렬화는 tick 본문 밖**에서 한다(ADR-0011 §3). tick이 만드는 것은 타입 구조체이고 문자열로 바꾸는 것은 게이트웨이다. 이 경계를 지키지 않으면 브로드캐스트 비용이 게임 시간 지연으로 바뀐다.
- **메모리**: 세션 송신 큐를 256 → **64**로 줄인다(ADR-0011 §5). 16.2 KiB 메시지에서 256은 세션당 4.24 MB, 31 세션에 131 MB다(p0-02 실측 RSS는 16.6 MB). `send_queue_bytes` 메트릭을 추가해 다음 결정을 측정 위에서 한다.
- **송신 큐 64 = 2.13초. 정상 클라이언트도 2.1초 이상 읽기를 멈추면 `SLOW_CONSUMER`로 끊긴다.** 생산율이 세션당 30 msg/s(스냅샷 10 + `COMMAND_RESULT` 20)이기 때문이다. **Unity Editor의 도메인 리로드·GC 히치가 그 시간을 넘을 수 있고, 그때 끊기는 것은 서버 버그가 아니다**(AC-13·AC-15 판정 시 `close_reason`을 먼저 본다).
- **`snapshot_build_us`를 `tick_body_us`에서 분리한다.** 스냅샷이 2 tick마다라 본문 소요가 이봉분포가 되어 p0-02 기준선과의 비교가 의미를 잃는다(AC-19).
- **결정성**: ADR-0010 §3의 범위. 시뮬레이션 안에 초월함수·난수·`HashMap` 순회 의존·외부 해시 크레이트가 없다.
- **클라이언트 핫 경로**: p0-02가 p1로 미룬 항목(U-10)이 **여기서 만기다.** 규약은 유지하고 **실제 할당량을 Unity Profiler로 한 번 재서 숫자를 남긴다**(스냅샷 1건당 + 초당). 목표치를 정하지 않는다 — 첫 측정이 다음 슬라이스의 기준선이다.
  - **`WORLD_SNAPSHOT`은 p0-02의 `ContractDispatch` 트리 경로를 타지 않는다.** client가 Unity 동봉 Mono에서 31척 스냅샷으로 실측한 결과: 현재 경로(`JObject` 트리 → `ToObject`) **0.656 ms/건, 200건당 gen0 8회** vs 문자열에서 직접 타입 역직렬화 **0.265 ms/건, gen0 1회**. **CPU가 아니라 가비지가 8배 차이**다(트리를 만든 뒤 버리기 때문). 판별자만 확인하고 타입으로 직접 역직렬화한다. 초당 몇 건인 나머지 타입은 기존 경로 그대로 둔다.
- **WebGL**: 이번에도 `IRealtimeTransport` 뒤 PC 전용이다. WebGL 빌드를 시도하지 않는다.
- **보안**: 서버는 `127.0.0.1`에만 바인딩한다. 이 슬라이스가 추가하는 공격면은 `SET_SHIP_CONTROL` 하나이고, 방어는 §4의 I-26·I-28·I-33·I-34·I-39이며 AC-17이 전수 확인한다.
- **재현성**: 모든 검증 명령은 인자 없이 같은 결과를 낸다. 봇 하네스는 시드를 인자로 받고 로그에 남긴다. AC-8의 입력열은 **파일로 저장**해 재실행이 같은 입력을 쓰게 한다.

## 9. 열린 질문과 통보

### 9.1 사용자·리더 결정으로 확정된 것 (2026-09-19)

| # | 항목 | 결정 |
|---|------|------|
| Q1 | 플레이 영역 20 km 기술 상한 | **받아들인다.** designer가 고른 12 km가 안쪽. 스키마 `maximum`이 강제 |
| Q2 | 스냅샷 10 vs 20 Hz | **10 Hz.** designer의 세션당 예산 192 KiB/s가 31 연결에서 20 Hz(301 KiB/s)를 배제한다. 유도 과정은 ADR-0011 §2 |
| Q3 | 클라이언트의 `data/` 취득 | **파일 복사, 이번만.** 방어는 AC-11(f) |
| Q4 | CI + `sqlx-cli` | **별도 슬라이스 `p1-01b`**. p1-01 완료 직후 |
| Q5 | 보정 임계값 | designer 수치를 받되 **architect의 무시 밴드를 아래에 깐다**(양자화 잡음 0.87 mm 위에서 보정이 영원히 도는 것을 막는다 — ADR-0012 §4) |
| Q6 | 30초 잔류·재개 | **승인.** `despawn_reason`에 `LINGER_EXPIRED` 추가, I-29·I-30·I-40·I-41 재작성 |
| 충돌 2 | 좌표계 손 방향 | **Unity 왼손 규약으로 통일.** 축 의미를 스키마 문구·fixture·육안 3중으로 고정 |
| 충돌 4 | 비행 모델 | **designer의 비행 보조 6DoF를 받는다.** 범위는 designer 문서에 있는 것까지 |
| 충돌 11 | `tick_hz` 중복 | **받지 않는다.** `worlds` 행의 월드 상수다(I-19) |

### 9.2 designer에게 보내는 통보 (리더 중계) — **D-1·D-2·D-3 완료 확인 (2026-09-20)**

**`data/` 3파일이 전부 스키마를 통과한다.** architect·server·client가 독립으로 재확인했고 I-38의 유도값 5종도 실제 데이터로 검산됐다(§5.6). **구현을 막는 항목은 남아 있지 않다.**

| # | 항목 | 조치 |
|---|------|------|
| **D-1 ✅ 완료** | `data/movement/sync-tuning.json`이 스키마 6건 실패했다 | `tick_hz`·`tick_hz_note` 제거(월드 상수). `snapshot.{mode, position_round_m, velocity_round_mps, orientation_round}` 제거(ADR-0009가 정본). `input.{max_inputs_applied_per_tick_per_session, superseded_policy, thrust_axis_range, roll_axis_range, quaternion_component_scale}` 제거(ADR-0009·0011이 정본). `prediction.{fixed_step_ms, integrator, orientation_integrator, transcendental_functions_allowed}` 제거(ADR-0010이 법이지 튜너블이 아니다). **`prediction.reconcile_ignore_threshold_m`와 `reconcile_orientation_ignore_threshold_deg` 추가**(각각 0.005, 0.02 권장) |
| **D-2 ✅ 완료** | `snapshot_hz: 20` → **10** | designer 자신의 예산 규칙이 산출한 값(ADR-0011 §2). 함께 `remote_interp_delay_ms` 100 → **200** |
| **D-3 ✅ 완료** | `cradle.json`의 `coordinate_space.note` "Right-handed" | **"Left-handed, Y up, +Z forward (Unity 규약, ADR-0009 §1)"로 정정.** 좌표값은 대칭이라 고칠 것이 없다 |
| D-4 | 입력 양자화 배율 | **±100/32767을 받지 않는다.** ±1000/1e6으로 간다(ADR-0009 §2에 근거). designer 수치 중 배율에 의존하는 것은 없다 |
| D-5 | 명령 이름 `SHIP_CONTROL_INPUT` | **`SET_SHIP_CONTROL`로 확정.** 명령은 명령형 `VERB_NOUN`(event-contracts 이름 규칙) |
| **D-11** (신규) | 스냅샷의 `ship_class_id`·`star_system_id` 표기 | **`data/`의 `id` 필드와 글자 그대로 같다**(lower-kebab). 변환을 두지 않으므로 `id`를 바꾸면 그대로 와이어와 도메인 이벤트에 나간다 — **`id`는 튜닝하는 값이 아니라 역사에 박히는 값이다**(원칙 5) |
| **D-12** (신규) | 추력 수치의 새 제약과 측면 실효 최대 | (a) **`main_thrust_mps2 ≥ lateral`·`≥ reverse`가 기동 검산 조건이 됐다** — 대각선 클램프 상수가 항상 최대값이어야 한다(현재 데이터는 통과). (b) 순수 측면 대각선의 실효 최대는 `lateral × √2 = 25.46 m/s²`로 클램프에 걸리지 않는다 — 의도한 설계라면 디자인 문서에 한 줄 있는 편이 좋다 |
| D-6 | 범위 밖 값 클램프 제안 | **거부를 유지한다.** 이월 규칙이 "조작이 끊긴다"를 이미 해결한다(ADR-0011 §4) |
| D-7 | `input_seq` 역행을 조용히 버리기 | **`STALE_INPUT`으로 거부한다.** I-15가 손실 계측 기준이라 이동만 예외로 두면 계측기가 눈이 먼다 |
| D-8 | 오토레벨 법칙이 비어 있었다 | architect가 `auto_level_rate_deg_s × sin_err`(사인 비례, 상한 = 그 속도)로 채웠다(ADR-0010 §2 5단계). 상수를 새로 만들지 않았다 |
| D-9 | 단위 접미사 규약을 스키마로 강제 | **명시 선언으로 강제한다.** 모든 필드를 이름으로 선언하고 `additionalProperties: false`이므로 `propertyNames` 패턴은 불필요하고 위험만 더한다 |
| D-10 | `xxhash64` 스폰 해시 | 크레이트를 결정적 코어에 들이지 않는다. **레포 안에 손으로 쓴 결정적 해시 + 고정 입력 테스트**로 간다(ADR-0010 §4). 규칙("같은 actor는 같은 자리")은 그대로 지켜진다 |

### 9.3 남은 열린 질문 (차단 없음)

| # | 질문 | 상태 |
|---|------|------|
| Q7 | `data/movement/sync-tuning.json`을 서버 설정으로 옮길 것인가 | designer가 "코드 리터럴만 아니면 된다"고 열어 두었다. **이번엔 `data/`에 둔다** — 클라이언트도 읽어야 하고(보정 임계), 서버 설정은 클라이언트가 못 읽는다. 옮기는 것은 클라이언트 데이터 배포 경로가 생길 때 |
| Q8 | 성계 콘텐츠 id `cradle`과 마커 이름 | designer가 전부 `가정:`으로 표시했다. **그대로 간다.** 다른 이름이 필요하면 데이터만 고치면 된다 |
| Q9~Q13 | LTS 이전, 라이선스 검토, 공개 불리 데이터, Defender 예외, `unityyamlmerge`, 영속화 백로그 상한 | p0-02에서 이월, 변동 없음 |
| Q15 | `reconnect_resume_window_seconds < linger_seconds`일 때의 의미 *(2026-09-22)* | **정의돼 있지 않다.** 서버는 이 값을 기동 검산(≤)에만 쓰고, 재개 판정에서는 잔류 구간 전체를 재개 가능으로 취급한다. 두 창 사이에 들어온 재접속을 스폰으로 처리하면 **한 actor에 함선 2척**(I-29 위반)이 되고, 디스폰으로 처리하려면 새 `despawn_reason`이 필요하다. 지금 데이터는 두 값이 같아 차단은 없다. **designer가 두 값을 다르게 두고 싶어지면 architect가 먼저 의미를 정한다** |

### 9.4 사용자 결정 *(2026-09-22)*

| # | 질문 | 결정 | 반영 |
|---|------|------|------|
| **Q14** | **같은 actor의 세션이 이미 열려 있을 때(함선이 `ACTIVE`) 새 세션이 들어오면?** (A) 옛 세션이 끝날 때까지 새 연결을 업그레이드 전에 거부한다 / (B) 새 세션이 넘겨받는다 — 옛 세션을 `SESSION_CLOSED{SUPERSEDED}`로 닫고 같은 tick에 함선 조종을 옮긴다("마지막 로그인 우선") | **✅ 결정됨 — (B) 나중 접속이 이어받는다** (사용자 결정 5, `00_request.md`, architect 추천안 채택). 결정 4번("30초 안에 재접속하면 같은 함선·같은 자리에서 재개")을 **소리 없는 끊김**에서도 지키는 유일한 안이다. (A)였다면 서버가 옛 연결의 죽음을 알아챌 때까지(코드상 최대 약 45초) 재접속이 거부됐다. 근거는 `01_architect_decisions.md` "R3 추가 판정" 사안 1 | I-29(세 경우), I-30(원인 타입), I-40(`SUPERSEDED` 예외), AC-3(h)(넘겨받기 단언 ①~⑤ + 순서 반대 경우), AC-9(a)·AC-11(a)(27/21). ADR-0005 §2·§5(close 4001, 재연결 안 함), ADR-0011 §6.3. **계약**: `SESSION_CLOSED.close_reason += SUPERSEDED`(schema_version 1 유지), 유효 fixture `superseded.json`(26 → 27), 레지스트리 `SHIP_DESPAWNED` description 정정 |

## 10. 확인하지 못한 사실

### 10.1 이번 검토 라운드에서 해소된 것

| # | 사실 | 결과 |
|---|------|------|
| U-11 | 생성기가 배열·`data` kind를 처리하지 못한다 | **해소.** 실제 트리에서는 **3단 캐스케이드**이고 첫 실패는 `maxItems`다(client 실측) |
| U-12 | 신규 스키마·fixture의 오프라인 검증 | **해소.** 18/18, 유효 26 전부 통과, 반례 34 전부 거부 |
| U-13 | 계약 커버리지 구현 전 기준선 | **해소.** errors 14 / warnings 0 (3자 독립 확인) |
| U-14 | 확장된 생성기가 배열 원소를 중첩 클래스로 내는가 | **해소 — PASS.** `ships`가 `ShipState[]`, 속성 17개(**측정 시점 기준. B-1이 `angular_velocity_roll_mdeg_s`를 더해 지금은 18개** — AC-10(c)), 위치 `long`·나머지 `int`, `presence` `string`. 필요한 변경은 **3건 + `BuildProperties` 1줄**이고 그것이면 충분하다(client 실측) |
| **U-15** | **Unity `double`이 Rust와 비트 동일한가** | **해소 — 동일하다.** Unity 동봉 `mcs`/`mono.exe`에서 300 tick 적분 후 **비교 15값 전부 비트 동일**, 양자화 정수도 동일(client 실측). **IL2CPP는 미측정**(U-15b) |
| **U-20** | 데이터 fixture의 실수 왕복 | **해소 — 깨지지만 원인이 달랐다.** 긴 소수는 완벽히 왕복하고, 깨지는 것은 `"type":"number"` 필드의 **정수 리터럴**이다. 해법은 AC-9(a)의 kind별 비교(server 실측) |
| U-19 | designer 데이터가 계약 한계 안에 드는가 | **해소.** 3파일 전부 통과 + 유도값 5종 검산 통과 |
| — | AC-8 "서로 다른 프로세스 2회"가 이 PC에서 도는가 | **해소.** `current_exe()` 재실행 패턴이 동작하고, 일부러 넣은 `HashMap` 순회에서 **실제로 실패해 검출력까지 확인**했다. 새 의존성 0 (server 실측) |
| — | 스냅샷 31벌의 tick 본문 비용 | **해소.** 세션마다 복제하면 961 `ShipState`(150 KiB)·할당 31회, **공유 배열 + 빌린 뷰**면 31개(4.8 KiB)·할당 1회. 스펙이 정한 경계("직렬화는 tick 밖") 안의 구현 선택이라 ADR 변경 없음 (server) |

### 10.2 남은 미확인

| # | 사실 | 누가·언제 | 안 되면 |
|---|------|----------|--------|
| **U-15b** | **IL2CPP 빌드의 `double`이 같은가** | 플레이어 빌드가 생기는 슬라이스 | 이번 슬라이스는 Editor(Mono)라 판정에 영향 없음. ADR-0010 §3에 기록됨 |
| U-16 | 실제 수신 경로의 **프로파일러 실측** | client, AC-13(e) | client의 합성 측정은 절대값이 다를 수 있다. 경로 비교(트리 vs 직접)는 이미 유효 |
| U-16c | 실제 스냅샷 바이트 | qa, AC-18(c) | architect·server·client의 계산이 서로 44 B 안에서 일치한다. **봇 실측이 정본** |
| U-17 | 세션별 직렬화 CPU | server, S5 후 | 예상 4.7 MB/s(한 코어의 수 %). `snapshot_build_us`로 분리 측정 |
| U-18 | 송신 큐 64의 정상 부하 충분성 | qa, AC-18(e) | 산술로는 2.13초 여유. `SLOW_CONSUMER` 발생 수를 함께 본다 |
| U-21 | 1차 쿼터니언 적분이 오버슈트 없이 안착하는가 | server, AC-4(g) | 안착 tick 수를 출력에 찍어 "약 1 % 늦어질 뿐"을 수치로 확인 |
| U-C1 | `unity test`가 없는 `--output` 디렉토리를 만드는가 | client, C2 | AC-11에 `mkdir -p`를 넣어 회피했다. 실측해 T10에 적는다 |
| U-22 | 스폰 해시 `H`의 선택 | server, S3 | 레포 안에 명세 + 고정 입력 테스트. **크레이트 요청 없음** |

## 11. QA가 알아야 할 환경 사실 — 판정이 무효가 되는 조건

server가 이 PC에서 확인해 정리한 것이다. **앞의 셋은 지금 이미 참이다.** 스프린트 계약은 이것들을 전제로 써야 한다.

| # | 조건 | 어떻게 판정을 무효로 만드는가 | 방어 |
|---|------|------------------------------|------|
| 1 | **`domain_events`에 `QA_APPEND_ONLY_PROBE` 행 1건이 남아 있다** (p0-02가 정당하게 넣은 탐침) | AC-20(e)의 전수 `distinct event_type`이 **이번 실행과 무관한 이유로** 기대와 달라진다 | 측정 세션 첫 단계로 `docker compose down -v`(프로젝트 범위) 또는 **쿼리를 이번 실행 tick 구간으로 한정**(AC-20(e)가 이미 그렇게 고쳐졌다) |
| 2 | **`data/`는 designer 소유이고 살아 있다** | 서버 테스트가 `data/`의 수치를 기대값으로 쓰면, designer가 조작감을 튜닝하는 순간 테스트가 빨간불이 된다. **"재미를 고치면 테스트가 깨진다"는 최악의 결합이다** | **규칙으로 올린다: 단위·통합 테스트의 기대 숫자는 `contracts/fixtures/`에서만 온다. `data/`는 기동 경로 검증(AC-2)에만 쓴다** |
| 3 | **`worlds.last_tick = 350280`** | 실서버 tick이 35만대에서 시작한다. tick 0 기준으로 손계산한 기대값이 맞지 않고 `occurred_at`도 게임 시간 12일째다 | 결정성·적분 테스트는 `Simulation::new(world, 0)`으로 **메모리에서** 돌린다(DB 무관). 실서버 검증만 실제 tick을 쓴다 |
| 4 | **송신 큐 64 = 2.13초 vs Unity Editor 히치** | Editor가 2초 넘게 멈추면 서버가 정상 클라이언트를 `SLOW_CONSUMER`로 끊고 **QA가 서버 버그로 오독한다** | §8에 적혀 있다. 리포트는 `close_reason`을 먼저 본다 |
| 5 | **측정 중 빌드** | p0-02에서 실제로 겪었다(같은 측정이 14.9초 → 0.079초). 이번에는 AC-8이 자식 프로세스를 띄워 CPU 경합이 더 크다 | AC-19 측정 중 `cargo`·Unity 임포트 금지 |
| 6 | **Windows 타이머 바닥값 +0.7 %/분** | `tick_lag_seconds`가 0이 아닌 것을 회귀로 읽으면 안 된다 | AC-19 표에 바닥값을 나란히 적는다(p0-02와 같은 형식) |
| 7 | **손 방향 부호가 통째로 뒤집혀도 모든 자동 검증이 통과한다** | 서버·클라이언트가 같은 공식을 쓰므로 예측 오차 0, fixture 왕복 통과, 결정성 통과. **특히 오토레벨의 `sin_err` 부호 한 줄이 그 위험을 진다**(ADR-0010 §2.1) | **AC-14(a)(d)가 유일한 검출기다. 사람이 실제로 볼 때까지 이 슬라이스는 부호에 대해 아무것도 증명하지 못한다** — QA 리포트에 그렇게 적는다. **두 번째 관측자를 봇으로 대체하면 이 검출기가 사라진다**(봇은 화면이 없다): 부호 검증이 필요한 항목은 Unity 클라이언트 2개 또는 클라이언트 1 + 사람의 눈이어야 한다(client 지적, QA 계약 §0.11) |
| 8 | **`domain_events`에 자기 참조 인과 행 7건이 남아 있다** *(2026-09-22)* — 전부 `SHIP_DESPAWNED{SERVER_SHUTDOWN}`이고 tick 395902에 6건(QA R3 RED 인스턴스), 430030에 1건(QA R3 §6.7 재현)이다. 식별 쿼리는 `causation_id = event_id` | 전 구간 인과 검사(AC-3(b)·(h), SC-81)가 **이번 실행과 무관한 이유로** 빨간불이 된다 | **동결된 결함 장부로 판정한다** *(2026-09-22 실측: 이 7건을 빼면 표 전체에서 인과 결함은 **0건**이다. 타입·≠self·`(tick, sequence)` 순서로 검사)*. 장부의 `event_id`는 다음 7개다: `01a0c46d-412c-75c4-a72e-5eb9b6eda64b`, `…-5ebab4dbe3a7`, `…-5ebbfc8128c6`, `…-5ebc0f2f7da9`, `…-5ebd1f652604`, `…-5ebe4e8056ac`(이상 tick 395902, seq 8~13, 접두 `01a0c46d-412c-75c4-a72e-`), `01a0c48c-6427-744b-9d9a-25dcd98df09f`(tick 430030). **판정 규칙**: 표 전체의 결함 집합이 이 장부와 **정확히 같아야** 한다. 하나라도 더 있으면 새 결함이고, 하나라도 빠지면 추가 전용 위반(I-20)이거나 도구 결함이다. 그리고 **그 라운드의 인스턴스 구간에서는 결함 0건**이어야 한다. 구간만 한정하면 구간 밖의 새 결함이 보이지 않는다. 장부가 있으면 구간을 좁히지 않아도 된다. **행은 지우지도 고치지도 않는다**(원칙 5). 이 행들은 스스로 결함임을 드러낸다 — 올바른 행은 `causation_id = event_id`일 수 없다. 그래서 **인과 그래프를 읽는 미래의 소비자(역사 엔진)는 자기 루프를 순환이 아니라 "원인 미상(결함 기록)"으로 읽어야 한다.** 정정 레코드는 역사 엔진의 정정 메커니즘이 생기는 슬라이스에서, 이 월드가 그때도 쓰이고 있다면 덧붙인다 |

## 변경 기록

| 날짜 | 변경 | 이유 |
|------|------|------|
| 2026-09-19 | 최초 작성 (draft). 계약 6타입 신설, ADR-0009~0012 신설, ADR-0007 §1 개정, 불변식 I-26~I-38, 수용 기준 21개 | p1-01-ship-movement 스펙·계약 초안 |
| 2026-09-20 | **평가 전 판단 2건.** ADR-0011 **§5.2 신설 — 송신 큐를 묶는 양은 in-flight가 아니라 `MAX_COMMANDS_PER_SESSION_PER_TICK`(8)**이다(거부 응답도 슬롯을 쓰므로 §5.1의 부등식은 틀린 양을 묶고 있었다). 초과 프레임은 응답 없이 카운터 + tick당 프로토콜 위반 1회 → **I-47 신설**, I-15 범위 명시, `TOO_MANY_IN_FLIGHT`는 구조적 미도달로 강등. ADR-0010 §4 **스폰 밀어내기 규칙을 "월드 함선 수" 기준으로 뒤집었다** — 내가 쓴 "바퀴 수" 규칙은 현재 데이터에서 **모든 오버플로 스폰을 한 점에 모으는 결함**이 있었다. AC-6(b)(d) 갱신, **AC-12(e) 미검증 해제**(S8 완료) | S7 재확인 + SC-20 간헐 실패, ADR↔코드 불일치 보고 |
| 2026-09-20 | **구현자 판단 5건 + 기록 2건 반영.** ADR-0011 §5.1 **`SESSION_IN_FLIGHT_LIMIT` 64 → 16**과 큐 관계 불변식(`in_flight × 응답배수 + 스냅샷여유 ≤ 큐`), §1 **`world_full` 재개 면제**, ADR-0010 §4 **"시도수" = 링을 돈 바퀴 수**·§4.1 **`spawn_world_seed` = `world_id` 하위 8바이트**·§4.2 **단일 함선 클래스 가정**, ADR-0012 §8 **재연결 정책 유지**(단일 백오프, cap 10 s < linger 30 s). 스펙에 I-44 보강·I-45·I-46 신설, **AC-12(e)를 S7 전까지 미검증으로** 고정 | server 4건 + client 1건의 판단 요청, 구현 중 발견 |
| 2026-09-20 | **QA 스프린트 계약 공백 3건 반영.** ADR-0011 §6.1 **휴면 입력 표**(`flight_assist=true`가 잔류 궤적 전체를 정한다) + **잔류 중 오토레벨이 계속 돈다** 명문화, §6.2 **재개 시 이어받는 것/버리는 것**(`last_applied_input_seq` = `None`), §1 **`maxItems: 64`를 입장에서 강제**(503 `world_full`, 스폰 거부는 I-29를 깬다). 스펙에 I-42·I-43·I-44 신설, I-38에 **데이터 디렉토리 해석 규칙**(`STARFALL_DATA_DIR` 있으면 그것만, 없으면 `data` → `../data` 탐색 후 절대 경로 로깅), AC-2(h)·AC-3(d)(e2) 추가, §11-7에 **봇으로 관측자를 대체하면 부호 검출기가 사라진다** | QA 계약 86항목 확정 중 발견된 공백 |
| 2026-09-20 | **최종 확정.** server·client 검토 반영: **계약 변경 1건**(`ShipState`에 `angular_velocity_roll_mdeg_s` 추가 — 각속도 합에서 둘을 복원하는 것은 손실 분해이고 선회 중 AC-12가 반드시 실패한다), 적분 2단계에 **롤 축 권한 분리** 한 줄 추가, 금지 함수에 `mul_add`·`hypot`·`to_radians`·`signum` 추가, 스폰 대척점 규칙을 수식으로, `DataId`를 **lower-kebab으로 통일**(변환 제거), AC-9(a)를 **kind별 비교**로(U-20 해소), AC-11(a)를 **26 발견/20 왕복**으로, AC-3(c)(d)·AC-4(g)·AC-7(d)·AC-10(a)(e)·AC-12(a)·AC-13(e)·AC-15(b)·AC-18(c)·AC-20(e) 문구 정정, 송신 큐 시간 표 3배 오차 정정(6.4 → 2.13초)과 Editor 히치 주의, `snapshot_build_us` 분리, §11 QA 환경 7건 신설. **U-15·U-20·U-14 해소** | server 검토(B-1~B-14) + client 검토(R1~R8, P-1~P-14) |
| 2026-09-19 | **확정 (agreed).** 사용자 결정 4건(비행 보조 6DoF / 140 m·12 km / 기준 마커 / 30초 잔류·재개)과 리더 판단 4건(좌표 왼손 통일 / designer 비행 모델 수용 / 잔류 수용 / `tick_hz` 중복 불가)을 반영. `SET_SHIP_CONTROL`을 목표 자세·브레이크·보조 토글 payload로 재설계, 입력 큐를 **마지막 것이 이긴다 + 이월**로 교체(`INPUT_QUEUE_FULL` → `RATE_LIMITED`·`STALE_INPUT`), `WORLD_SNAPSHOT`에 `presence`·`ack_input_seq`·soft/hard 경계 추가, 데이터 스키마 3종을 designer 파일 레이아웃에 맞춰 재작성, 불변식 I-39~I-41 신설, 수용 기준 전면 갱신 | 충돌 12건 조정 완료. 상세는 `_workspace/p1-01-ship-movement/01_architect_decisions.md` |
| 2026-09-20 | **QA 라운드 1 통지 반영 — 문서 옛값 정정 4칸 + 1칸 주석.** B-1이 `ShipState`에 `angular_velocity_roll_mdeg_s`를 더하면서 생긴 옛값을 정본에 맞췄다: §1 요약 **반례 33 → 34**, §5.4 머리말 **거부 17/통과 10/계층 없음 7 → 18/10/6**(34번째 반례는 데이터가 아니라 **메시지 타입이라 C#이 거부한다** — 계층 없음은 데이터 3종 × 2건 = 6건이다), 같은 표의 `angular-velocity-roll-missing.json` 행 **거부(예측) → 거부(실측)**, AC-10(c) **속성 17 → 18개**, AC-11(b) 상수 **17/10/7 → 18/10/6**. §8 U-14의 "속성 17개"는 측정 시점 기록이므로 고치지 않고 **현재 18개임을 괄호로 붙였다**(원칙 5의 정신 — 과거 측정을 덮어쓰지 않는다). **구현·계약·SC-43·SC-48은 이미 새 값이었고 이 슬라이스에서 바뀐 것은 스펙 문서뿐이다.** ADR-0010 **§3.1 신설**(재생 산출물 추적 + 대조 규칙 R-1~R-5), `.gitignore`에 추적 사유 주석 | QA 라운드 1 §6.2 통지(FAIL 아님) + 리더 발견(S6 산출물 미추적) |
| 2026-09-20 | **SC-51/52 검증 경로 정정(architect R2).** AC-12 (a)(b)가 재는 양을 "재조정 직전 오차"에서 **"열린 고리 적분 오차"**로 바로잡고 근거를 주석으로 남겼다 — S6 fixture는 600 tick에 명령 6건(이월 만료 차단)이라 **운영 분포(`client_send_hz = tick_hz = 20`, "one input per tick")와 정반대**이고, 그 위에서 `Reconcile()`을 돌리면 첫 지점은 장부 불일치로 미터급 오차가 뜨고 나머지 ~293지점은 `HasError = false`로 떨어져 **잰 것이 없어진다.** client가 택한 경로는 누적 표류를 재므로 **더 엄격하다**(실측 여유 7.4배·215배). **AC-12(f) 신설** — 각속도 두 필드 대조 (위치·자세만 보면 wire→sim이 `ω_roll`을 떨어뜨려도 빠져나간다). **AC-13(b)에 관찰 3건 추가** — `Reconcile()`이 실서버 데이터로 실제로 탔는지(`HasError` 1회 이상 / 재생 입력 0 아님 / 선회 중 재조정 ≥ 1). **fixture는 바꾸지 않는다** — 조밀하게 만들면 (a)(b)가 2 tick 창만 재어 더 약해지고, golden 재생성 비용이 이득보다 크다 | client의 SC-51/52 구현 보고 + 리더 판정 요청 |
| 2026-09-21 | **라운드 3 판정 3건.** **§5.1a 신설** — 명령→기대 응답 규범 표(`PING_SERVER` → `COMMAND_RESULT` + `PING_REPLY`(수락 시, 결과 뒤) / `SET_SHIP_CONTROL` → `COMMAND_RESULT`만)와 **"카운터 항등식은 입력이 전부 0이면 반드시 실패해야 한다"** 규율. 이 관계가 어느 표에도 없어 봇 게이트가 `accepted == PING_REPLY 수`를 가정했고, `accepted = 0`이라 **아무 일도 없을 때만 통과하는 검사**였다. **레지스트리 `responses` 필드는 미룬다** — 명령 타입 2종으로 필드 모양을 정할 수 없고(원칙 8·10), `registry_version` 인상은 Rust·C# 양쪽 생성물을 건드려 라운드 1 실측 기반을 무효화한다. 발동 조건: **세 번째 명령 타입이 들어오는 슬라이스에서 다음 `registry_version` 인상과 함께**. **AC-2(h) 신설** — 드레인하지 않는 stdout 파이프에서도 `/healthz` 200 + `shutdown` 0, 조건 셋(파이프가 실제로 찼음 단언 / RED 선행 / stderr 처리). 이 성질은 `cargo test` 안에 존재할 수 없다. `graceful_client_close_is_prompt` flaky는 **관측 설계 FAIL**로 판정 — 벽시계가 서버 지연이 아니라 **자기 폴링 비용**(50 ms × HTTP 왕복)을 재고 있었고 2초는 진짜 경계(`PING_INTERVAL` 15초)의 대리값이다. 예산을 늘리지 않고 **폴 횟수 단언**으로 바꾼다 | server S-A·S-B·S-C 수정 중 올린 권한 밖 사안 2건 + 리더 부수 질문 |
| 2026-09-22 | **R3 추가 판정 6건.** **I-29 개정**: 세션 ↔ `ACTIVE` 함선 1:1을 명문화했고, 세션 열기의 세 경우(없음/`LINGERING`/`ACTIVE`)를 전부 정의하도록 바꿨다. `ACTIVE` 경우의 정책은 **Q14(§9.4, 사용자 확인 대기, 추천 B = 넘겨받기)**. **I-30 보강**: 자기 참조 인과는 언제나 결함이고, **원인을 지어내지 않는다**. **AC-3(h) 신설**: 동시 세션에서 함선 1척·자기 참조 0·유령 `ACTIVE` 0, in-process RED 선행. **AC-3(d)를 두 층으로 정정**: (d1) `f64` 비트 일치(재접속 유무 차분, in-process) / (d2) 와이어 양자화 봉투 + 음성 대조. 옛 "양자화 정수로 같다, 허용 오차 없음"은 스냅샷에서 출발하면 성립할 수 없었다. **AC-2 번호 정정**: 두 번째 (h)(로그 소비자) → **(i)**. 2026-09-21 행의 "AC-2(h) 신설"이 지금의 (i)다. **AC-2(i) 조건 ③의 사유 정정**: 버린 줄 통지는 stderr가 아니라 stdout에 in-band로 나가며, 코드가 맞다. **I-45 보충**(정확한 부등식은 백오프 상한 < 재개 창), **Q15**(재개 창 < 잔류 창의 의미 미정), **§11-8**(DB의 자기 참조 7행 — 지우지 않는다. 인과 검사는 **동결된 결함 장부 7건과의 등식** + 인스턴스 구간 0건으로 판정. 이 7건을 빼면 표 전체의 인과 결함은 0건이다). ADR-0010 §3에 **"보장의 입력은 `f64` 상태다"** 추가 | QA R3 §6.7(🔴 I-29·I-30 위반, 실서버 재현), §6.5(SC-11(3) 불성립), §6.4(SV-1 전제 반증), §5.5(문서·관측 3건). 상세: `_workspace/p1-01-ship-movement/01_architect_decisions.md` "R3 추가 판정" |
| 2026-09-22 | **사용자 결정 5 반영 — 동시 세션은 "나중 접속이 이어받는다"(Q14 결정됨).** I-29의 `ACTIVE` 경우를 넘겨받기로 확정했다: `SESSION_CLOSED{SUPERSEDED}`(원인 = 새 `SESSION_OPENED`), 같은 tick에 조종 이전, 잔류 없음, close 4001, 판정은 sim이 제출 순번으로 한다. I-30의 원인 타입 목록에 `SESSION_CLOSED{SUPERSEDED}` ← `SESSION_OPENED`를 추가했고, I-40에 `SUPERSEDED` 예외와 "조종 세션 없는 `ACTIVE`는 없다"를 넣었다. AC-3(h)에 넘겨받기 단언 ①~⑤와 순서 반대 경우를 붙였다. **계약 변경 적용**: `SESSION_CLOSED.close_reason`에 `SUPERSEDED` 추가(`schema_version` 1 유지), 유효 fixture `SESSION_CLOSED/superseded.json` 신설, 레지스트리 `SHIP_DESPAWNED` description 정정(`registry_version` 3 유지). **실측**: 스키마 18 / 유효 26 → **27** 전부 통과 / 반례 34 전부 거부(`unknown-close-reason.json`의 `KICKED_BY_GM`은 여전히 거부) / 레지스트리 13, `check_contract_coverage.py --strict` PASS. 새 fixture는 **이전 스키마에서 거부됨**을 확인했다. Rust 계약 테스트는 **예상대로 RED 2건**(`fixtures_roundtrip`: unknown variant, `registry_consistency`: 하드코딩 26)이고 server S-6이 고친다. AC-9(a)·AC-11(a) 건수를 27/21로 갱신했다(C# 21은 client 확인 대기). ADR-0005 §2·§5(4001 + 재연결 안 함 — 클라이언트가 행동을 바꾸는 첫 close code), ADR-0011 §6.3 신설 | `00_request.md` 사용자 결정 5(2026-09-22). 상세: `01_architect_decisions.md` "사용자 결정 반영 — 계약 변경 적용" |
