# p1-01-ship-movement — QA 평가 리포트 라운드 2

- 작성: qa, 2026-09-20
- 계약: `_workspace/p1-01-ship-movement/02_sprint_contract.md` (SC-01~86, M-1~M-17) — **4차 개정 반영**
- 라운드 1: `04_qa_report_r1.md` (블록 0·1·2 + SC-82·83·84·86 — **PASS 34 / FAIL 3 / 미검증 0**). **그 판정은 그대로 유효하다** — architect 의 문서 수정은 `contracts/` 를 건드리지 않았고, 라운드 1 실측 기반(스키마 18 / 유효 fixture 26 / 반례 34 / 레지스트리 타입 13)이 유지된다.
- 증거 원문: `evidence/R2-*.log` · `evidence/SC-87-*.log` · `evidence/B3-*` · `evidence/B4-*` · `evidence/B5-*`
- **범위**: FAIL 3건 재판정 + 신규 항목(SC-51·52·58·64·65 사전확인·SC-87) + **블록 3·4·5(실서버)**

---

## 0. 실행 전제

### 0.1 계약 §2 수정 (리더 지시로 qa 가 직접 반영)

`02_sprint_contract.md` §2 의 `WORLD_SNAPSHOT` 행 비고를 **`ShipState` 17필드 → 18필드**로 고쳤다(302행).
§9 변경 이력에 **4차** 항목으로 남겼다(507행). 같은 문서의 §0.5·SC-43·SC-48 은 이미 18 을 전제하고 있었고
**이 칸 하나만 2차 개정에서 빠져 있었다**. 라운드 1 의 SC-83 실측(`ShipState` 18행)과 client 생성물(`[JsonProperty]` 18개)이
근거다. **판정 기준은 바뀌지 않았다** — SC-43·SC-48 이 이미 18 을 요구했다.

### 0.2 G-a — 라운드 1 결정 유지

**`docker compose down` 을 어떤 형태로도 실행하지 않았다.** SC-80 은 **이번 실행의 tick 구간 한정**으로 판정하고,
구간 시작점은 서버 기동 로그의 `start_tick` 으로 잡는다(라운드 1 에서 정한 방식).

### 0.3 실행 순서 — 빌드를 전부 끝낸 뒤 실서버로 (게이트 G-c)

1. **Phase A (빌드)**: 재판정용 `cargo test` 3회 + `cargo fmt`/`clippy` + `unity test` + 정적 확인 + SC-87
2. **Phase B (실서버 관측)**: 블록 3 → 블록 4 → 블록 5. **이 구간에는 어떤 빌드도 돌리지 않는다.**

게이트 G-f 대로 블록 3(관측)을 **먼저** 끝내고 블록 4(치트·강제 close)로 넘어간다 —
치트의 강제 close 가 블록 3 의 "서버가 먼저 닫은 연결 0" 관측을 오염시키지 않게 한다.

### 0.4 §0.10 — 여전히 증명하지 못하는 것

> **손 방향 부호가 통째로 뒤집혀도 자동 검증은 전부 통과한다.** 검출기는 **SC-59 의 육안 관찰 하나뿐**이다.

이번 라운드에도 SC-59 는 실행되지 않았다 → **M-14: 부호 미검증** 유지.

---

## 1. Phase A — 재판정과 신규 항목 (빌드 단계)

### 1.1 집계

| 판정 | 항목 | 비고 |
|---|---|---|
| **PASS (FAIL → PASS 재판정)** | **SC-18 · SC-19 · SC-46** | 라운드 1 의 FAIL 3건이 전부 닫혔다 |
| **PASS (신규)** | **SC-51 · SC-52 · SC-87** | SC-51·52 는 architect 결정에 따른다(§1.6) |
| **PASS (판정 정정)** | **SC-55** | 라운드 2 초안에서 qa 가 **FAIL** 로 적었다가 **architect 판정(2026-09-21)에 따라 PASS 로 정정**했다. 근거가 둘로 나뉘고 ② 쪽은 CL-1 대기다(§1.7) |
| 사전 확인만 (판정은 블록 6·7) | SC-58 · SC-64 · SC-65 | 코드는 확인했다. 실서버 관측이 남았다 |

### 1.2 회귀 — `cargo test --workspace --locked` 3회 + 게이트

| 회차 | 결과 | 소요 |
|---|---|---|
| 1 | 바이너리 13 / **154 passed / 0 failed** / 4 ignored / exit 0 | 47 s |
| 2 | 동일 | 39 s |
| 3 | 동일 | 38 s |

라운드 1 의 **152 → 154** (SC-18·SC-19 신규 테스트 2건). `cargo fmt --all --check` exit 0,
`cargo clippy --workspace --all-targets -- -D warnings` exit 0.
**간헐 실패 0** — `sc20`·`sc22`·`sc30_sc31`·`i44_world_full`·`too_many_in_flight` 전부 3/3.
(`evidence/R2-SC-01-test-run{1,2,3}.log`, `R2-SC-01-fmt.log`, `R2-SC-01-clippy.log`)

`cargo test -p starfall-sim --locked -- --nocapture --test-threads=1` → **44 passed / 0 failed / exit 0**
(라운드 1 의 42 + 2). (`evidence/R2-SC-18-19-sim.log`)

### 1.3 SC-18 재판정 — **FAIL → PASS**

신규 테스트 `world::integrate::tests::soft_boundary_pulls_toward_origin_without_blocking_thrust`.
**두 단언이 계약이 요구한 두 절반과 정확히 대응한다:**

| 관찰 | 실측 | 단언 |
|---|---|---|
| (a) soft 밖에서 **원점 방향 가속이 더해진다** | `v = (-1.2500000000000002, 0, 0)` — `-boundary_pull_mps2 × DT` = `-25 × 0.05` | 크기 일치(`< 1e-9`) **+ 방향이 −x(원점 쪽)** |
| (b) **조작은 계속 먹는다** | 같은 위치에서 전방 추력을 **동시에** 주면 `v = (-1.2500000000000002, 0, 1.75)` — `1.75 = main_thrust_mps2(35) × DT` | `v.z` 일치(`< 1e-9`) **+ 경계 당김이 그대로 남아 있다** |

**(b) 가 이 항목의 핵심이었다.** 경계 당김과 추력이 **한 벡터 안에 동시에** 들어 있다 —
"경계가 조작을 빼앗지 않는다"가 수치로 보인다. hard 쪽 기존 테스트는 hard 전용으로 남았다.

### 1.4 SC-19 재판정 — **FAIL → PASS**

신규 테스트 `simulation::tests::carry_forward_expires_after_configured_ticks_then_decays_by_damping_only`.
**`step(state, &no_input())` 직접 호출이 아니라 `Simulation::new(world(), 0)` + `sim.step(vec![], …)` 로
진짜 이월 경로를 탄다** — 라운드 1 에서 FAIL 로 잡았던 바로 그 지점이다.

- **이월 플래그(tick별)**: `[1,1,1,1,1,1,1,1,1,1, 0,0,0,0,0]` — **정확히 10회**, `carry_forward_max_ticks = 10` 과 같다.
  단언: 실입력 tick 의 `input_carried_forward == 0`, 이후 증가 총합 `== carry_forward_max_ticks`.
- **속도 곡선(|v|)**: `1.750 → 3.500 → … → 17.360 → 19.073` (이월 구간, 가속 중)
  **→ 18.723 → 18.373 → 18.023 → 17.673 → 17.323** (만료 후)
  만료 지점부터 **tick 당 정확히 0.350** 씩만 줄어든다 = `assist_linear_decel_mps2(7.0) × dt(0.05)`.
  **가속은 0 이 되고 감쇠만 남으며 즉시 0 이 되지 않는다** — 계약이 요구한 "속도 곡선"이 그대로 출력된다.

→ **M-17 의 "이월"·"이월 만료" 두 줄이 이제 채워진다**(§4). 라운드 1 에서 "확인할 수단이 없다"고 적었던 자리다.

### 1.5 SC-46 재판정 — **FAIL → PASS**

`unity test client --mode EditMode --report-format nunit,junit …` → **exit 0** (12초).
qa 독립 실행 리포트(`_workspace/p1-01-ship-movement/unity-tests-qa-r2/EditMode.nunit.xml`):

> **`total=145 / passed=143 / failed=0 / inconclusive=0 / skipped=2`**

- **`inconclusive` 가 0 이 됐다** — 라운드 1 의 exit 8 원인이 사라졌다.
- `skipped` 2 는 `LiveServerTests.Live_{ServerInitiatedClose_ReconnectsAsANewSession, ThreePings_RoundTripInOrder}` 로
  `STARFALL_LIVE_TESTS` 게이트다(기존 자산, 이번 수정과 무관). **`Assert.Ignore` 는 이 2건에만 남아 있고**
  `ReconciliationTests.cs:239` 의 것은 "여기서는 절대 쓰지 말라"는 **주석**이다 → architect 금지 지시 위반 0건
  (`evidence/R2-inconclusive-sweep.log`).
- **141 → 145 의 내역** (qa 가 두 리포트를 이름 집합으로 대조): 신규 5건
  (`ObserverCsvTests` 4건 + `Reconcile_RealS6Replay_…`), 제거 1건(`Reconcile_MissingS6ReplayAsset_IsRecordedPending`).
  **라운드 1 에서 exit 8 을 만들던 그 테스트가 실물로 교체됐다.**

### 1.6 SC-51 · SC-52 — **PASS** (architect 결정에 따른다)

`ReconciliationTests.Reconcile_RealS6Replay_PositionAndOrientationErrorWithinIgnoreThreshold` **Passed**.
출력(qa 재실행본에서 그대로):

> `[SC-51/52] real S6 replay: compared points=299, position error p99=0.00065578842830249 m, max=0.000675860423635886 m, orientation error p99=8.65812355595793E-05 deg, max=9.28064127720879E-05 deg`

| 항목 | 임계 | 실측 p99 | 여유 |
|---|---|---|---|
| SC-51 위치 오차 | 0.005 m | **0.000656 m** | 약 7.6배 |
| SC-52 자세 오차 | 0.02 deg | **8.658e-5 deg** | 약 231배 |
| 비교 지점 | — | **299개** | — |

**합성 재생 대체가 끝났다.** 실제 S6 산출물(`server/crates/sim/tests/data/replay/`)을 읽고,
게이트 G-k 대로 **복사하지 않고 상대 경로로 참조**한다(테스트 안에서 `replayDir` 부재 시 `Assert.Fail`).

**리더가 넘긴 쟁점에 대한 architect 결론은 `01_architect_decisions.md` 끝에 있다: SC-51·52 PASS.**
근거 요약 — "계약이 재려던 것(§0.10·S-8 '두 언어가 같은 물리')을 더 엄격하게 쟀다. 문구만 실제 형태에 맞춘다."
`Reconciliation.Reconcile()` 을 타지 않는 이유는 테스트 머리말에 남아 있다: 이 fixture 의
`carry_forward_max_ticks = 10_000` 때문에 같은 `input_seq` 가 수백 tick 이어지는데, 그것은 20 Hz 실클라이언트가
만들지 않는 상태라 `Reconcile` 의 전제(tick 당 새 입력 1건, ADR-0012 §6)와 어긋난다.

> **qa 의 기록(판정 아님).** architect 가 이 형태를 "열린 고리"로 명시하고 수용했다.
> 그 결과 **`Reconciliation.Reconcile()` 은 여전히 진짜 서버 산출물을 만난 적이 없다** — 그것을 덮는
> SC-53·54·55 는 전부 합성 입력이다. **M-17 이 겨냥한 "만들고 한 번도 안 탄 경로"와 같은 계열**이므로
> M-17 표에 별도 행으로 남긴다(§4). 판정에는 쓰지 않는다.

### 1.7 SC-55 — **PASS (판정 정정)**, 근거는 둘로 나뉜다

> **qa 의 판정을 뒤집은 기록.** 라운드 2 초안에서 qa 는 SC-55 를 **FAIL** 로 적었다 — 당시 계약 §1 H절의
> SC-55 검증 방법이 **S6 fixture 의 롤 구간**을 지목하고 "같은 스냅샷 안에서 두 각속도가 둘 다 0이 아님을 봐야 한다"고
> 요구했는데, 실S6 대조 루프에 그 단언이 없었기 때문이다. **architect 판정(2026-09-21)이 이 요구를 신설 AC-12(f)/CL-1 로
> 분리했다** — 즉 그 관찰은 **구현 이후에 만들어진 요구**다. 이미 있던 구현을 나중에 생긴 요구로 FAIL 시키는 것은
> 공정하지 않으므로 **PASS 로 정정한다.** 계약 §1 H절과 §9(5차)에 이 분리를 반영했다.

**① wire→sim 의 두 필드 되감기 — 덮인다. PASS 의 근거는 이쪽이다.**
`ReconciliationTests.Reconcile_TurningSnapshot_RestoresBothAngularVelocitiesSeparately_NotFromASum` **Passed**
(`ReconciliationTests.cs:471`). qa 재실행본의 출력:

```
true       omega_aim=(-14.7895444803722, 61.3243705013794, 4.71218574503686) omega_roll=63
decomposed omega_aim=(-14.7897069790757, 61.3240461563904, 4.71231899304689) omega_roll=62.9995643989917
dot(fwd, true omega_aim) = 4.60993550452926E-05 (must be non-negligible for this test to mean anything)
```

- **같은 스냅샷 안에서 `ω_aim` 과 `ω_roll` 이 둘 다 0이 아니다**(`|ω_roll| > 1.0` 을 `482행` 에서 단언).
- 마지막 줄이 이 테스트가 항진명제가 아님을 스스로 증명한다 — 전방축 성분이 무시할 수 없어야 "합에서 분해"가 실제로 틀린다.
- **SC-55 본문이 말하는 케이스가 정확히 이것이다.**

**② 물리(선회 중 두 각속도가 자세에 반영되는가) — S6 재생 쪽은 아직 비어 있다.**
`Reconcile_RealS6Replay_…`(231~371행)의 단언은 **위치 오차 · 자세 오차 · `comparedPoints > 0` 뿐**이고,
**각속도를 비교하는 단언이 한 건도 없다.**

```
$ grep -n "AngularVelocity" client/Assets/_Project/Tests/EditMode/ReconciliationTests.cs
   95, 98    ← 양자화 헬퍼
  482, 490, 497, 501  ← 합성 테스트(①)
  231~371 구간: 0건
```

architect 가 찾은 구멍 그대로다: **그 경로는 열린 고리라 C# 이 자기 ω 를 스스로 적분하므로,
wire→sim 이 `angular_velocity_roll_mdeg_s` 를 떨어뜨려도 이 테스트는 통과한다.**
tick 0 의 ω 는 스폰 직후라 0 이어서 초기 상태 경로로도 걸리지 않는다.
**그것이 B-1 과 이 항목이 존재하는 이유다.**

**→ CL-1 이 남아 있다**(§5.2 C-A). **판정은 PASS 지만 ② 쪽 근거는 CL-1 이 붙어야 닫힌다.**

### 1.8 SC-87 (신규) — **PASS**, 검출력까지 qa 가 독립 확인

`cargo test -p starfall-sim --test determinism -- --nocapture` → **exit 0**, 골든 3파일 전부
`바이트 단위로 일치` 로그. 이어서 `git diff --exit-code -- server/crates/sim/tests/data/replay/` → **0**.
`git status --porcelain` 은 `A ` 3행(스테이징됨, 커밋 전) — 리더가 만든 상태 그대로다.
(`evidence/SC-87-golden.log`)

**조용히 통과하지 않는다는 것을 qa 가 직접 깨뜨려 확인했다**(I-25 — 출처가 독립일 때만 검증이다).
`evidence/SC-87-detection-probe.log`:

| 단계 | 결과 |
|---|---|
| `initial.json` 끝에 10바이트 추가(2041 → 2051) | — |
| `cargo test … determinism` | **FAILED / exit 101**. 메시지: `골든 2051 바이트, 방금 재생 2041 바이트, **첫 차이 오프셋 2041**` + `조용히 bless 하지 말고 원인을 먼저 확인하라` + 재생성 명령 안내 |
| 백업에서 복원(**`STARFALL_REPLAY_BLESS` 를 쓰지 않았다** — 원본 바이트로 돌아가는지가 확인 대상) | sha256 3개 복원 확인 |
| 재실행 | `1 passed`, 골든 3파일 일치 |
| `git diff --exit-code` | **0** |

**bless 로 덮지 않고 복원했다는 점이 중요하다** — bless 를 썼다면 "테스트가 자기 기대값을 다시 쓴 것"과
"원본이 돌아온 것"을 구분할 수 없다.

### 1.9 SC-58 — 코드 확인만 (판정은 블록 6)

**라운드 1 의 "구현 부재"는 해소됐다.** `client/Assets/_Project/Scripts/Greybox/GreyboxSession.cs:248`
에 `static readonly ProfilerMarker SnapshotHandleMarker = new ProfilerMarker("Starfall.Snapshot.Handle");`
가 있고, **선언만이 아니라 실제로 쓰인다**: 252행 `using (SnapshotHandleMarker.Auto()) { OnWorldSnapshotCore(message); }`
— **핸들러 전체를 감싼다**(역직렬화된 메시지 in → 예측·보간 상태 갱신 out), 즉 "스냅샷 1건"의 단위와 일치한다.
프로젝트 전체에서 `ProfilerMarker` 는 이 한 곳뿐이다. (`evidence/R2-SC-58-marker.log`)
**실측(할당량 수치)은 블록 6 이다 — Deep Profile 을 끄고 잰다.**

### 1.10 SC-64 · SC-65 사전 확인 (판정은 블록 7)

**CSV 헤더가 세 구현에서 문자 단위로 같다** (`evidence/R2-SC-64-65-csv-header.log`):

| 출처 | 헤더 |
|---|---|
| qa `tests/e2e/two_client_view.py` `COLUMNS` | `tick,observer_actor_id,ship_id,presence,px_mm,py_mm,pz_mm,vx_mm_s,vy_mm_s,vz_mm_s` |
| client `Greybox/ObserverCsv.cs:52` `Header` | 동일 |
| **봇** `tools/bots/src/snapshot.rs:25` `SNAPSHOT_CSV_HEADER` | 동일 |

기계 대조 결과 `문자열 동일: True`, 열 수 10 / 10. **세 번째 출처(봇)까지 같다는 것이 중요하다** —
SC-64 를 봇으로 대체해야 할 때(§0.11 의 후퇴 경로) 같은 스크립트가 그대로 읽는다.
client 쪽에는 이를 지키는 테스트도 생겼다(`ObserverCsvTests.Header_MatchesQaScriptColumnsExactly` Passed).

**실서버 없이 확인된 것은 여기까지다.** client 가 스스로 적은 미확인 리스크 2건
(① 서버가 두 세션 모두에 브로드캐스트하는가 ② 서로 다른 subject 가 다른 `ship_id` 로 스폰되는가)은
**블록 3 에서 관측 가능하므로 그 절에 기록한다**(§2).

---

## 2. 블록 3 — 서버 단독 실서버 (SC-08·09, SC-28~33)

**실행**: 서버를 실제 `data/` 로 기동(`tests/e2e/server_boot.py serve`, stdin 을 잡고 있다가 `shutdown` 으로만 내린다).
봇 **4대 / `run --scenario e` / 25초 / 시드 4242**. 증거: `evidence/B3/`.

- 측정 구간: **`start_tick = 350421`** (기동 로그). **`docker compose down` 미실행**(G-a) → SC-80 은 이 구간 한정.
- 측정 중 빌드 0건(G-c). Unity Editor 대화형 인스턴스 없음.

### 2.1 🔴 **블록 3 에서 이번 슬라이스의 최대 결함을 찾았다**

> ### `SET_SHIP_CONTROL` 이 게이트웨이에서 통째로 거부된다. 함선을 움직이는 명령이 **소켓으로는 시뮬레이션에 도달한 적이 없다.**

**증거 (`evidence/B3/BLOCKER-set-ship-control.log`, `evidence/B3/bots-run.log`)**

봇 4대가 `SET_SHIP_CONTROL` **200건**을 보냈다. 결과:

| 관측 | 값 |
|---|---|
| 봇이 받은 `COMMAND_RESULT` | 200 (accepted **0** / rejected **200**) |
| `rejected by reason` | **`{"UNKNOWN_COMMAND_TYPE": 200}`** |
| `/debug/stats` `commands_rejected_total{UNKNOWN_COMMAND_TYPE}` 델타 | **+200** |
| **`commands_received_total` 델타** | **0** |
| `input_superseded_total` / `input_carried_forward_total` / `aim_degenerate_total` 델타 | **0 / 0 / 0** |
| 모든 스냅샷의 `ack_input_seq` | **전부 `null`** (`ack_last = None` ×4봇) |

**`commands_received_total` 델타가 0 이라는 것이 결정적이다** — 그 카운터는 "큐에 넣으려 **시도한** 명령" 인데,
200건 중 한 건도 시도조차 되지 않았다.

**원인 (파일:라인)** — `server/crates/gateway/src/ws.rs:579`, 인바운드 프레임 처리기 `handle_text`:

```rust
if peek.command_type.as_deref() != Some(registry::PING_SERVER) {
    return send_rejection(out_tx, stats, command_id, RejectReasonCode::UnknownCommandType);
}
```

**게이트웨이가 아는 명령은 `PING_SERVER` 하나뿐이다.** 이어지는 2단계 역직렬화도
`serde_json::from_str::<PingServerCommand>` 로 고정돼 있고, `submit.try_command(...)` 에 넘기는 것도
`InboundCommand::PingServer(command)` 하나다(`ws.rs:597`·`634`).

**아래 계층은 전부 준비돼 있다** — 그래서 더 위험하다:

| 계층 | `SET_SHIP_CONTROL` |
|---|---|
| 계약 스키마 | 있다 (`contracts/commands/SET_SHIP_CONTROL.schema.json`, `const = "SET_SHIP_CONTROL"`) |
| Rust 계약 타입 | 있다 (`contracts/src/commands.rs:76`·`112`) |
| **`sim`의 명령 열거형** | **있다** — `simulation.rs:200 InboundCommand::SetShipControl(SetShipControlCommand)` |
| C# DTO | 있다 (`SetShipControlCommand.cs`, SC-43·SC-83 통과) |
| 봇 송신 | 있다 (`tools/bots/src/wire.rs:168`) |
| **게이트웨이 인바운드 디스패치** | **없다 (`ws.rs:579` 가 막는다)** |

**왜 154개 테스트가 전부 초록인데 아무도 못 봤는가** — 이것이 M-17 의 존재 이유 그 자체다:

```
$ grep -c "PING_SERVER"      server/crates/gateway/tests/ws_integration.rs   → 1
$ grep -c "SET_SHIP_CONTROL" server/crates/gateway/tests/ws_integration.rs   → 0
```

**게이트웨이 통합 테스트 스위트는 `SET_SHIP_CONTROL` 을 단 한 번도 보내지 않는다.**
`sim` 쪽 단위 테스트(SC-15~22, SC-34)는 `Simulation::step` 을 **메모리에서 직접** 부르므로 게이트웨이를 지나지 않는다.
**"통과한 테스트 수는 경로가 실행됐다는 증거가 아니다"**(계약 §7) 의 두 번째 실례이고,
첫 번째(`world_full`)보다 훨씬 크다 — 이번 슬라이스의 제목이 "함선 이동"이다.

**재현 (2분)**
```bash
export STARFALL_DEV_AUTH_SECRET=dev_only_not_a_secret
python tests/e2e/server_boot.py serve --stop-file /tmp/STOP --ready-file /tmp/READY &
cd tools/bots && ./target/debug/bots.exe probe --case fly --count 10
#  기대: accepted 10 / ack_input_seq 가 증가
#  실제: rejected 10, rejected_by_reason={"UNKNOWN_COMMAND_TYPE": 10}, ack_last=None
```

**담당: server.** 요청은 §5.1.

### 2.2 이 결함이 막는 항목 — **판정 불가 7건 + 부분 관측 1건**

**이것은 "미검증(환경)"이 아니라 구현 부재다**(계약 §6). 그래서 아래는 전부 **FAIL(차단)** 이다.

| ID | 왜 막히는가 |
|---|---|
| **SC-31** | `ack_input_seq` 가 **영원히 `null`** 이다. "적용 전 `null`" 절반은 관측됐지만(1066건 전수 `null`), **"서버가 마지막으로 적용한 `input_seq`" 와 "세션 내 단조 비감소"는 적용이 0건이라 관측 자체가 불가능**하다 |
| **SC-23** | 위치·자세 주입 반례가 `MALFORMED_COMMAND` 가 아니라 `UNKNOWN_COMMAND_TYPE` 으로 거부된다 — **차단은 되지만 계약이 지정한 층에서 되지 않는다** |
| **SC-24(c)(d)** | 범위 초과 거부·이월 지속·회전량 상한 — 명령이 도달하지 않아 관측 불가 |
| **SC-25** | 속도 핵: 두 봇의 이동 거리 차이를 재려면 함선이 움직여야 한다. **모든 함선이 스폰 지점에 고정돼 있다** |
| **SC-26** | 손실 항등식 `보낸 수 = COMMAND_RESULT + 드롭`: 형식적으로는 200 = 200 + 0 으로 성립하지만 **200건 전부가 거부라 항등식이 측정하려던 것(정상 처리 경로의 손실)을 재지 못한다** |
| **SC-27** | `input_superseded_total` 델타 **0** — 같은 tick 에 경합하는 입력이 애초에 없다 |
| **SC-61·62·66·67** | 이동 관측·치트 판정이 전부 이 경로에 의존한다 (블록 4·7) |
| **M-7 · M-8** | designer 지표(브레이크 사용률·전추력 비율·최근접 거리·속도 p95) 전부 산출 불가 |

**세션 수명 경로는 영향받지 않는다** — 스폰·잔류·재개·디스폰·스냅샷 브로드캐스트는 명령과 무관하게 동작하고,
아래 항목들은 정상적으로 판정됐다.

### 2.3 SC-08 · SC-09 — **PASS**

`python tests/e2e/ship_events.py causation --from-tick 350421` → **exit 0 / verdict PASS**
(`evidence/B3/ship-events.log`)

| 관측 | 값 |
|---|---|
| 검사한 `SHIP_*` 이벤트 | **7건** (스폰 4 + 디스폰 3) |
| `causation_id` 가 null 인 행 | **0** |
| `causation_id` 가 실제 이벤트를 못 가리키는 행(dangling) | **0** |
| **스폰 조인 수** | **4** — 전부 `SHIP_SPAWNED.causation_id = SESSION_OPENED.event_id` |
| 원인의 타입이 틀린 건수 | **0** |
| **SC-09** 원인의 `(tick, sequence)` 가 결과보다 크거나 같은 건수 | **0** |

DB 원문(`evidence/B3/db-ship-spawned.txt`): 스폰 4건이 **서로 다른 `actor_id` 4개**
(`…000000000000` ~ `…000000000003`)에 각각 1건, tick 350919 / 350944 / 350969 / 350994, `sequence` 전부 1.

### 2.4 SC-28 — **PASS**

봇 계측과 qa 의 CSV 독립 집계가 같다(`evidence/B3/snapshot-gates.log`).

| 관측자 | 스냅샷 tick 수 | **검사한 쌍** | 간격 ≠ 2 인 쌍 |
|---|---|---|---|
| actor …0000 | 266 | 265 | **0** |
| actor …0001 | 267 | 266 | **0** |
| actor …0002 | 265 | 264 | **0** |
| actor …0003 | 268 | 267 | **0** |
| **합계** | **1066** | **1062** | **0** |

**제외한 첫 스냅샷 = 4건**(세션당 1건, 계약이 지정한 제외 규칙). 간격 분포는 네 관측자 모두 `{2: N}` 단일값이다.
선언된 `snapshot_interval_ticks` 도 4봇 전부 **2** 로 일치한다.

### 2.5 SC-29 — **PASS**

- 봇 계측 `ship_order_violations = 0` (4봇 전부).
- **qa 독립 재계산**: CSV 를 `(관측자, tick)` 로 묶어 `ship_id` 문자열 오름차순인지 검사 →
  **검사한 그룹 1066개, 정렬 위반 0개**.
- **함선 수 교차검증**: 봇 `max_ships_seen = 4` ↔ DB `SHIP_SPAWNED` 4건 ↔
  `/debug/stats` 의 `ships_active + ships_lingering` 최대 4. 셋이 일치한다.

### 2.6 SC-30 — **PASS**

`controlled_ship_missing = 0` — **스냅샷 1066건 전수**에서 각 수신자의 `controlled_ship_id` 가
자기 함선이고 `ships` 배열 안에 있었다. qa 교차: 관측자 4명이 각각 **ship 4종**을 봤다(자기 것 포함).

### 2.7 SC-31 — **FAIL (§2.1 이 차단)**

관측된 것: 스냅샷 **1066건 전수에서 `ack_input_seq = null`**, 봇 4대 전부 `ack_last = None`,
`ack_regressions = 0`.
**"적용 전에는 null" 은 만족하지만, 적용이 0건이라 나머지 절반(마지막 적용 `input_seq` · 세션 내 단조 비감소)은
관측 자체가 불가능하다.** `(5,3)` 치트 케이스도 같은 이유로 의미가 없다. §2.1 이 고쳐진 뒤 재판정한다.

### 2.8 SC-32 — **부분 PASS → FAIL(관측 미완)**

**관측된 절반**: `presence` 가 실제로 두 값을 갖는다 — CSV 4140행 중 **`ACTIVE` 4012 / `LINGERING` 128**.
**`LINGERING → ACTIVE` 역전 0건**(전이가 단방향이다).

**관측되지 않은 절반**: "디스폰된 다음 스냅샷부터 `ships` 에서 사라진다".
**이번 실행에서는 관측할 수 없었다** — 디스폰이 tick 352051·352078·352100 에 일어났는데
**마지막 관측자가 tick 351528 에 끊겨** 그 시점에 아무도 보고 있지 않았다.
→ **블록 5 에서 "한 봇은 붙어 있고 다른 봇의 잔류가 만료되는" 시나리오로 다시 잰다.**

### 2.9 SC-33 — **부분 PASS → FAIL(델타 미관측 3키)**

**키는 전부 있고**(라운드 1 §3.2 에서 확인), **부하 전후로 값이 변한 것**:

| 키 | before → after | 델타 |
|---|---|---|
| `snapshots_sent_total` | 0 → 1066 | **+1066** |
| `snapshot_bytes_total` | 0 → 2,514,466 | **+2,514,466** |
| `ships_active` / `ships_lingering` | 0 / 0 → 1 / 0 | 게이지 동작 확인 |
| `commands_rejected_total{UNKNOWN_COMMAND_TYPE}` | 0 → 200 | +200 (8라벨 유지) |
| `messages_written_total{WORLD_SNAPSHOT}` | 0 → 1066 | +1066 |

**교차 검증 성립**: `snapshots_sent_total (1066) == messages_written_total{WORLD_SNAPSHOT} (1066)`.
**봇이 독립으로 센 수신 스냅샷도 1066** — 세 출처가 일치한다(손실 0).
바이트도 서버 `snapshot_bytes_total` **2,514,466** == 봇 `snapshot_bytes_received` **2,514,466** 으로 같다.

**변하지 않은 3키**: `input_superseded_total` · `input_carried_forward_total` · `aim_degenerate_total` 전부 **0**.
셋 다 **§2.1 이 막고 있다**(명령이 도달하지 않으면 경합도 이월도 퇴화도 생기지 않는다).
`send_queue_bytes` / `send_queue_bytes_max` 도 이 부하에서는 0 이다(4연결, 여유 충분).
**계약이 "존재만으로 PASS 주지 않는다"고 못박은 항목이므로, 7키 중 3키의 델타가 없는 현재는 FAIL 이다.**

### 2.10 SC-80 — **PASS** (tick 구간 한정)

`ship_events.py types --from-tick 350421` → **exit 0 / PASS**.
구간 내 `event_type` distinct = **정확히 4종**: `SESSION_OPENED` 4 · `SESSION_CLOSED` 3 · `SHIP_SPAWNED` 4 · `SHIP_DESPAWNED` 3.
**위치·상태 시계열 타입 0건.** p0-02 의 `QA_APPEND_ONLY_PROBE` 는 구간 밖이라 잡히지 않는다 —
**`docker compose down -v` 를 쓰지 않았고 tick 구간 한정을 썼다**(계약 §0.2 가 요구한 기록).

### 2.11 SC-12 — **PASS** (블록 5 항목이지만 이번 실행에서 완전히 관측됐다)

`SHIP_DESPAWNED{LINGER_EXPIRED}` **3건**, 전부 `causation_id = 그 잔류를 시작시킨 SESSION_CLOSED.event_id`
(`despawn_cause_wrong_type = 0`). **원인이 결과보다 앞선 tick 수**: `min 600 / max 600` — 즉 **정확히 600 tick = 30초**,
`linger_seconds = 30` 과 일치한다.

| SESSION_CLOSED tick | SHIP_DESPAWNED tick | 차이 |
|---|---|---|
| 351451 | 352051 | 600 |
| 351478 | 352078 | 600 |
| 351500 | 352100 | 600 |

**상관과 인과가 서로 다른 시각을 가리키는 첫 데이터**다(계약이 이 항목에 적어 두라고 한 것).

### 2.12 🟠 **블록 3 발견 2 — 유령 세션** (계약 외 발견이 아니라 SC-14 를 깨뜨린다)

`ship_events.py pairs --from-tick 350421` → **exit 1 / verdict FAIL**:
`ships_checked 4 / spawned_total 4 / despawned_total 3`, 짝 없는 함선 1척
(`01a0bf0f-9150-…015e`, spawned 1 / despawned 0).

**원인은 짝짓기 로직이 아니라 서버가 세션 하나를 닫지 않은 것이다** (`evidence/B3/FINDING-ghost-session.log`):

| 관측 | 값 |
|---|---|
| 봇 프로세스 | **실행 중인 `bots.exe` 없음** (`tasklist` 확인) |
| OS 소켓 | **ESTABLISHED 연결 0개** — `netstat` 에 `LISTENING` 과 `TIME_WAIT` 만 |
| 서버 `/debug/stats` | **`ws_connections = 1`, `ships_active = 1`** (봇 종료 후 **3분 이상** 경과) |
| DB | `SESSION_OPENED 4` / `SESSION_CLOSED 3` |
| 설정 | `ws.rs:60 IDLE_TIMEOUT = 30초` — **발동하지 않았다** |

**그 함선은 `LINGERING` 도 되지 못한다** — 세션이 닫혀야 잔류가 시작되므로 `ACTIVE` 로 영원히 남는다.
ADR-0005 §2 가 유휴 타임아웃을 둔 이유("TCP 만으로는 죽은 연결이 몇 분간 살아 있는 것처럼 보이고,
그러면 **동시 접속 수가 거짓말이 된다**" — `ws.rs:326` 주석)가 **정확히 그대로 일어났다.**

**qa 도구 쪽 가능성은 배제했다**: 봇 프로세스도 소켓도 남아 있지 않다.
**담당: server.** 요청은 §5.1. **SC-14 · SC-76 은 이 결함이 고쳐진 뒤 재판정한다.**

---

## 3. 블록 4 — 치트·프레이밍: **실행하지 못했다. 서버가 멈췄다**

블록 3 관측을 끝낸 뒤(G-f 순서 준수) 치트 probe 7종을 돌리려 했다. **한 건도 판정하지 못했다.**

### 3.1 🔴 **발견 3 — 서버가 무응답 상태가 된다 (프로세스는 살아 있다)**

**증거**: `evidence/B4/FINDING-server-unresponsive.log`, `evidence/B4/server-stdout.log`, `evidence/B4/probe-*.log`

| 시각(KST) | 사건 |
|---|---|
| 22:43:54 | 서버 기동. `start_tick = 350421` |
| 22:44:19~23 | 봇 4대 세션 수립 (로그 `세션 수립` **4건**) |
| 22:44:46~50 | 봇 4대 종료. 로그 `수신 태스크 종료` **4건**, `세션 종료 제출` **3건** ← **1건이 없다** |
| 22:48 | `/debug/stats` 정상. `ws_connections = 1`, ESTABLISHED 소켓 0개 (발견 2) |
| ~22:50 | `/debug/stats` 마지막 정상 응답: `tick = 358135`, `tick_total = 7715`, `tick_overrun_total = 0` |
| ~22:50 | probe 가 TCP 연결에는 성공(`connect_error=None`) → **`SESSION_READY` 를 받지 못함**. 서버 로그에 그 연결의 `세션 수립` 줄이 **찍히지 않았다** |
| 이후 | `/debug/stats`·`/healthz` **완전 무응답** (20초 타임아웃에도 `http_code=000`) |
| 22:57 | stdin `shutdown` **무효** — 20초 안에 끝나지 않아 qa 드라이버가 `terminate` 했다 |

**서버 로그가 22:44:50.411 이후 한 줄도 없다.** 그 마지막 줄이 바로 짝이 없는
`수신 태스크 종료 session_id=01a0bf0f-9141-7538-a816-b4cdf4e8de93 elapsed_ms=26793` 이다.

```
$ grep -c "세션 수립"        server-stdout.log   → 4
$ grep -c "수신 태스크 종료"  server-stdout.log   → 4
$ grep -c "세션 종료 제출"    server-stdout.log   → 3      ← 1건 누락
누락된 session_id: 01a0bf0f-9141-7538-a816-b4cdf4e8de93   (= 발견 2 의 유령 세션)
```

**두 발견은 같은 결함이다.** `수신 태스크 종료` 와 `세션 종료 제출` 사이에서 멈췄고,
그래서 (a) 세션이 닫히지 않고(유령), (b) 소켓이 해제되지 않으며(CLOSE_WAIT 누적),
(c) **나중에 들어온 새 연결의 세션 수립도 같은 자리에서 막힌다** — probe 의 `세션 수립` 줄이 없는 이유다.

**소켓 증거** (`netstat -ano | grep :8080`): 서버 쪽 **CLOSE_WAIT 4개**(상대는 닫았는데 서버가 자기 쪽을 안 닫음),
상대 쪽은 전부 `FIN_WAIT_2`. 프로세스는 **CPU 누적 0.25초 / RSS 16.6 MB** — 바쁜 게 아니라 멈춰 있다.

**추정 위치(qa 의 가설, 단정하지 않는다)**: `세션 수립`(연결 수립)과 `세션 종료 제출`(종료 제출)이
공유하는 것은 시뮬레이션으로 들어가는 **`SubmitHandle`** 하나뿐이다
(`crates/gateway/src/ws.rs` 의 `submit.*`). 그 경로가 막히면 두 로그 줄이 정확히 이 조합으로 사라지고,
tick 루프와 Axum 라우터가 같은 런타임에 있으므로 HTTP 도 함께 멈춘다.
**단정은 server 가 해야 한다** — qa 가 가진 것은 로그의 빈자리와 소켓 상태까지다.

**qa 가 하드 킬을 했다는 사실을 기록한다**(계약 §0.1 은 하드 킬 금지다).
**정상 종료 경로를 먼저 시도했고 그것이 실패해서** 드라이버가 `terminate` 했다.
`evidence/B4/serve.out`: `!! 정상 종료가 시간 안에 끝나지 않아 terminate 했다`.
**그 실패 자체가 관측 결과다** — 멈춘 서버는 stdin `shutdown` 으로 내려오지 않는다.

**담당: server.** 요청은 §5.1.

### 3.2 블록 4 항목 판정

| ID | 판정 | 사유 |
|---|---|---|
| SC-23 | **FAIL(차단)** | probe `cheat-position`·`cheat-attitude` 가 `SESSION_READY` 타임아웃. 설령 붙었어도 §2.1 때문에 `MALFORMED_COMMAND` 가 아니라 `UNKNOWN_COMMAND_TYPE` 이 된다 |
| SC-24 | **FAIL(차단)** | (b) 필드 목록은 §3.3 에서 정적으로 확인됨. (c)(d) 는 probe `cheat-range` 타임아웃 |
| SC-66 | **FAIL(차단)** | 같음. **시도 0 / 차단 0** — 세션이 서지 않아 시도조차 못 했다 |
| SC-67 | **FAIL(차단)** | probe `cheat-seq` 타임아웃. `ack_input_seq` 는 §2.7 대로 구조적으로 `null` |
| SC-68 | **부분** | (e) 는 §3.3 에서 **PASS**(계약이 증거다). (f) 잔류 가로채기·(g) `SESSION_READY` 이전 송신은 **미실행** |

**probe 별 실측** (`evidence/B4/probe-*.log`):

| probe | 결과 |
|---|---|
| `cheat-position` | `SESSION_READY` 타임아웃, sent=0 |
| `cheat-attitude` | 〃 |
| `cheat-range` | 〃 |
| `cheat-seq` | 〃 |
| `cheat-flood` | `SESSION_READY` 타임아웃. **sent=572 / results=0 / missing_results=572**, 이어서 `os error 10054`(강제 종료) — 서버가 응답하지 않는 채로 572건을 받아만 갔다 |
| `pre-ready` | `os error 10061` 연결 거부 (이 시점엔 서버가 이미 종료됨) |
| `fly` | 〃 |

### 3.3 SC-68(e) — **PASS** (계약 필드 목록이 증거다)

계약 §1 I절이 "**어휘에 없어 시도 불가**"로 적으라고 한 항목이다. 세 출처를 나란히 확인했다:

| 출처 | `SetShipControl` payload 필드 | `ship_id` |
|---|---|---|
| 스키마 `SET_SHIP_CONTROL.schema.json` | `input_seq`, `thrust_x/y/z_milli`, `roll_milli`, `aim_x/y/z/w_micro`, `brake`, `flight_assist` — **11개** | **없다** |
| Rust `commands.rs:86 SetShipControlPayload` | 같은 11개 | **없다** |
| C# `SetShipControlCommand.cs` | 같은 11개 | **없다** |

라운드 1 SC-83 의 경계면 표(행 16개, 불일치 0)가 이 목록을 기계적으로 대조했다.
**"다른 함선을 조작한다"는 명령을 만들 수 있는 필드가 계약에 없다** — 코드 검사보다 강한 보장이다
(코드는 바뀔 수 있고 검사는 빠질 수 있지만, 필드가 없으면 그 값을 실어 보낼 자리가 없다).
SC-24(b) 의 "위치·속도·현재 자세가 필드 목록에 없다"도 같은 표로 확인된다.

---

## 4. 블록 5 — 잔류·재개·디스폰

첫 서버가 멈춰 하드 종료된 뒤, **두 번째 서버 인스턴스**를 띄워 진행했다(`start_tick = 358261`).
증거: `evidence/B5/`.

### 4.1 발견 3 이 **재현됐다** — 그리고 간헐적이다

세 번의 실서버 세션을 돌렸고 **두 번 멈췄다**:

| 실행 | 구성 | 결과 |
|---|---|---|
| ① 블록 3 | 봇 4대 × 25초 | **멈춤.** `세션 종료 제출` 1건 누락 → 유령 세션 → 이후 무응답 |
| ② 재현 시도 | 봇 2대 × 10초 | **정상.** 2 opened / 2 closed / `ships_lingering = 2` / CLOSE_WAIT 0 |
| ③ 블록 5 | 관측자 1대(60초) + `probe fly` 1대 동시 | **멈춤.** 아래 |

③ 의 서버 로그(`evidence/B5/server-stdout.log`):

```
세션 수립       5건
수신 태스크 종료  3건      ← 2건 누락
세션 종료 제출    3건
마지막 로그: 2026-09-20T14:02:30.644 세션 수립 … actor_id=…0007
```

`세션 수립` 은 됐는데 `수신 태스크 종료` 조차 없는 session_id **2개**:
`01a0bf20-16c5-74a7-9a64-704e89d7e9c2`(관측자) · `01a0bf20-2774-7200-91a2-d2b3f154d5ab`(bot-007).
**로그가 그 줄에서 끊긴다.** 이후 `/debug/stats`·`/healthz` 무응답, `probe` 재접속은 `os error 10054`.

> **①과 ③은 멈추는 지점이 한 칸 다르다.** ①은 `수신 태스크 종료` **뒤**, `세션 종료 제출` **앞**.
> ③은 `수신 태스크 종료` **앞**. 둘 다 **다른 세션이 열려 있는 상태에서 한 세션이 끝나는 순간**이다.
> ②(둘이 동시에 끝남)만 무사했다. **qa 는 여기까지가 관측이고, 원인 특정은 server 의 몫이다.**

**stdin `shutdown` 은 2/2 로 실패했다.** 두 인스턴스 모두 qa 드라이버가 20초 대기 후 `terminate` 했다
(`evidence/B4/serve.out`, `evidence/B5/serve.out`). **하드 킬을 한 사실과 그 이유를 기록한다**(계약 §0.1).

**간헐 실패도 FAIL 이다**(계약 §0.3). 3회 중 2회 멈춘 것은 "가끔"이 아니다.

### 4.2 SC-10 — **PASS** (이번 라운드에서 가장 깨끗한 관측)

같은 `actor_id` 가 **잔류 만료 후** 재접속해 다시 스폰됐고, 두 스폰 위치가 **정수까지 같다**.
(`evidence/B5/db-spawn-positions.txt`)

| tick | actor_id | 스폰 위치 (mm) | ship_id |
|---|---|---|---|
| 358615 | `…000000000000` | `(0, -250000, 2500000)` | `01a0bf1d-e571-…266d` |
| **359569** | `…000000000000` | **`(0, -250000, 2500000)`** | `01a0bf1e-9fc5-…006a` (**새 함선**) |
| 358665 | `…000000000001` | `(2500000, 250000, 0)` | `01a0bf1d-ef35-…ff00` |

- **게이트 G-j 준수**: 첫 세션은 tick ~358855 에 닫혔고 재스폰은 **359569** — 차이 **714 tick = 35.7초 > `linger_seconds` 30초**.
  잔류 창 **밖**이므로 재개가 아니라 스폰이다(계약이 경고한 항진명제를 피했다).
- **`data/` 의 스폰 지점과 정확히 일치**: `(0, -250000, 2500000)` = `points_m[3] = [0.0, -250.0, 2500.0] m × 1000`.
  다른 actor 는 `points_m[0]` 을 받았다 — **actor 별 시드 해시가 동작하고 난수가 아니다**(ADR-0010 §4).
- 그 시점 `data/` 값을 증거에 함께 적었다(§0.7): 스폰 지점 12개, `point_count = 12`.

### 4.3 SC-12 — **PASS** (블록 3 §2.11 에서 완전히 관측됨)

`SHIP_DESPAWNED{LINGER_EXPIRED}` 3건, `causation_id` 전부 그 `SESSION_CLOSED.event_id`,
원인→결과 tick 차이 **min 600 / max 600** = 정확히 30초.

### 4.4 SC-11 — **FAIL(차단)**

**잔류 창 *안*의 재접속을 한 번도 성사시키지 못했다.**
계획: `bot-007` 이 접속→종료 후 **8초 뒤**(30초 창 안) 재접속. 실제: 첫 접속은 성공했으나
(`b007-first`: sent=153 / results=153) **그 직후 서버가 멈춰** 재접속이 `os error 10054` 로 실패했다
(`evidence/B5/b007-resume.log`).

따라서 (1) `ship_id` 동일 (2) `SHIP_SPAWNED` 추가 발행 없음 (3) **적분 상태 연속성**(qa 독립 계산) **셋 다 미관측**이다.
§4.1 이 고쳐진 뒤 재판정한다.

### 4.5 SC-13 — **FAIL(차단)**

**정상 종료 자체가 성립하지 않았다.** 두 인스턴스 모두 stdin `shutdown` 이 무효였고 하드 종료됐으므로
`SHIP_DESPAWNED{SERVER_SHUTDOWN}` 이벤트가 **한 건도 발행되지 않았다**
(두 구간의 `SHIP_DESPAWNED` 는 전부 `LINGER_EXPIRED` 다).
(a) 잔류 함선 · (b) 활성 함선 **둘 다 미관측**.

### 4.6 SC-14 · SC-76 — **FAIL** (서버 결함의 직접 결과)

| 구간 | `ships_checked` | spawned | despawned | 짝 없는 함선 |
|---|---|---|---|---|
| 첫 서버 (tick ≥ 350421) | 4 | 4 | 3 | **1** (`01a0bf0f-9150-…015e`) |
| 둘째 서버 (tick ≥ 358261) | 5 | 5 | 3 | **2** (`01a0bf20-16f7-…c504`, `01a0bf20-2791-…e801`) |

**짝이 없는 3척은 전부 §4.1 의 "닫히지 않은 세션"의 함선이다.** 짝짓기 로직(`ship_id` 기준, §0.6)에는
문제가 없다 — 정상 종료한 세션의 함선은 **전부** 짝이 맞았다.
I-41(함선은 반드시 스폰 1 / 디스폰 1)이 **프로세스 경계에서 깨졌다.**

### 4.7 SC-32 — **FAIL(관측 미완)** (블록 3 §2.8 과 같은 이유)

두 번째 시도도 실패했다. 관측자를 60초 붙여 두고 다른 봇의 잔류 만료를 보게 하려 했으나
**서버가 그 전에 멈췄다**. `presence` 의 두 값과 단방향 전이는 관측됐지만(§2.8),
**"디스폰된 다음 스냅샷부터 `ships` 에서 사라진다"는 여전히 미관측**이다.

### 4.8 SC-79 · SC-80 — **PASS**

- **SC-79**: `python tests/e2e/check_sequence_gaps.py` → **PASS**, **전 테이블 577행**, `(world_id, tick)` 별 `sequence` 가 0..n−1 로 빈틈없다.
- **SC-80**: 두 번째 구간(tick ≥ 358261)도 **PASS** — `event_type` distinct 4종뿐, 위치 시계열 0건.
  **두 구간 모두 tick 구간 한정으로 판정했고 `docker compose down` 은 실행하지 않았다**(G-a).

---

## 5. FAIL 상세 — 담당자별 수정 요청

### 5.1 server — **3건, 전부 실서버에서만 드러난다**

| # | 제목 | 심각도 |
|---|---|---|
| **S-A** | `SET_SHIP_CONTROL` 이 게이트웨이에서 거부된다 | **차단** — 슬라이스의 주제를 막는다 |
| **S-B** | 세션이 닫히지 않는다(유령 세션·CLOSE_WAIT 누적) | **차단** — I-41 이 깨진다 |
| **S-C** | 서버가 무응답이 되고 `shutdown` 이 안 먹는다 | **차단** — 3회 중 2회 |

**S-B 와 S-C 는 같은 뿌리일 가능성이 높다**(둘 다 세션 수명 경계에서 로그가 끊긴다).

#### S-A — `SET_SHIP_CONTROL` 인바운드 디스패치 부재
- **파일:라인** `server/crates/gateway/src/ws.rs:579` (그리고 `:597` 의 2단계 역직렬화, `:634` 의 `InboundCommand::PingServer`)
- **기대** `command_type == "SET_SHIP_CONTROL"` 이면 `SetShipControlCommand` 로 역직렬화해
  `InboundCommand::SetShipControl` 로 제출한다. 그 열거형 변형은 **이미 있다**(`crates/sim/src/simulation.rs:200`).
- **실제** `PING_SERVER` 가 아니면 전부 `UNKNOWN_COMMAND_TYPE` 으로 거부. `commands_received_total` 델타 **0**.
- **재현**
  ```bash
  export STARFALL_DEV_AUTH_SECRET=dev_only_not_a_secret
  python tests/e2e/server_boot.py serve --stop-file /tmp/STOP --ready-file /tmp/READY &
  cd tools/bots && ./target/debug/bots.exe probe --case fly --count 10
  # 실제: rejected 10, {"UNKNOWN_COMMAND_TYPE": 10}, ack_last=None
  ```
- **함께 고쳐야 할 것**: `crates/gateway/tests/ws_integration.rs` 는 `SET_SHIP_CONTROL` 을 **0번** 보낸다
  (`grep -c` 실측). **이 테스트가 없었기 때문에 154개가 초록인 채로 이 결함이 살아남았다.**
  수정과 함께 **소켓을 통과하는** `SET_SHIP_CONTROL` 통합 테스트를 넣어야 한다 —
  최소한 `accepted` 1건 + `ack_input_seq` 가 그 `input_seq` 로 올라오는 것까지.

#### S-B — 세션이 닫히지 않는다
- **관측** `수신 태스크 종료` 는 찍혔는데 `세션 종료 제출` 이 없는 session_id 1건(첫 인스턴스),
  `세션 수립` 뒤 `수신 태스크 종료` 조차 없는 session_id 2건(둘째 인스턴스).
- **결과** `ws_connections` 게이지가 거짓말을 한다(OS ESTABLISHED 0개인데 1), 함선이 `ACTIVE` 로 영구 잔존,
  소켓이 **CLOSE_WAIT** 로 쌓인다, `SHIP_DESPAWNED` 가 발행되지 않아 **I-41 이 깨진다**(SC-14 FAIL).
- **`ws.rs:60 IDLE_TIMEOUT = 30초` 가 발동하지 않는다** — 3분 이상 지나도 살아 있었다.
  `ws.rs:326` 주석이 이 타임아웃의 목적을 "동시 접속 수가 거짓말이 되지 않게"라고 적고 있는데,
  정확히 그 실패가 일어났다.
- **재현**: `bots run --scenario e --bots 4 --duration 25` 뒤 `netstat -ano | grep :8080` 에서 CLOSE_WAIT 확인,
  `/debug/stats` 의 `ws_connections` 와 대조. **3회 중 2회 재현**.

#### S-C — 무응답 + `shutdown` 무효
- **관측** 서버 로그가 세션 수명 경계에서 끊기고, 이후 `/debug/stats`·`/healthz` 가 20초 타임아웃에도 `http_code=000`.
  프로세스는 살아 있고 **CPU 누적 0.25초** — 스핀이 아니라 정지다. stdin `shutdown` 무효(2/2).
- **qa 가 좁혀 둔 것**: 멈춤 직전 마지막 정상 응답은 `tick_total = 7715`, `tick_overrun_total = 0` 이었다
  (성능 문제로 느려진 게 아니다). 두 멈춤 모두 **다른 세션이 열린 상태에서 한 세션이 끝나는 순간**에 시작됐다.
  `세션 수립` 과 `세션 종료 제출` 이 공유하는 것은 시뮬레이션으로 가는 `SubmitHandle` 뿐이다.
- **영향** 이 상태에서는 어떤 실서버 항목도 잴 수 없다. **블록 4·5·7·8 전체가 이 결함에 걸려 있다.**

### 5.2 client — **1건 (FAIL 아님, architect 가 넘긴 남은 작업)**

#### C-A / CL-1 — SC-55 의 S6 재생 쪽 근거(AC-12 f)가 없다
- **파일** `client/Assets/_Project/Tests/EditMode/ReconciliationTests.cs`,
  `Reconcile_RealS6Replay_PositionAndOrientationErrorWithinIgnoreThreshold` (231~371행)
- **기대**(architect CL-1) 실S6 대조 루프에 **각속도 대조**를 넣는다 — 롤 구간의 한 지점 이상에서
  `confirmed` 의 `ω_aim`·`ω_roll` 이 **둘 다 0이 아님**을 단언. 오차 분포는 출력만. **임계값을 새로 만들지 않는다.**
- **실제** 그 구간의 단언은 위치 오차·자세 오차·`comparedPoints > 0` 뿐.
  `grep -n "AngularVelocity" ReconciliationTests.cs` → 95·98(양자화 헬퍼), 482·490·497·501(**합성** 테스트)
  — **231~371 구간에 0건**.
- architect 의 말 그대로: **"S6 재생만으로 SC-55 를 판정하지 말 것 — 그 경로는 각속도를 대조하지 않으면 `ω_roll` 누락을 통과시킨다."**

### 5.3 server (권고, 판정 아님)

- SC-40(f): `contract_tests.rs` 에 `10.0` 거부 단언 1건 (라운드 1 §6.1 그대로 유효).
- architect 가 넘긴 낮은 우선순위 1건: `determinism.rs:181` 주석에 출하 튜너블(10)과 1000배 차이 한 줄.

---

## 6. M-17 — 신규 경로를 실제로 탔는가 (라운드 2 갱신)

라운드 1 의 표를 실서버 관측으로 갱신했다. **통과한 테스트 수는 경로 실행의 증거가 아니다**(계약 §7).

| 신규 경로 | 라운드 1 | **라운드 2** | 근거 |
|---|---|---|---|
| 스폰 | 실행됨(메모리) | **실행됨(실서버)** | DB `SHIP_SPAWNED` **9건**(두 구간 4 + 5), 서로 다른 `actor_id`, `causation_id` 전부 `SESSION_OPENED` |
| **이월** | 미확인 | **실행됨(메모리)** | `input_carried_forward` 플래그 `[1×10, 0×5]`, 증가 총합 == `carry_forward_max_ticks` 단언 |
| **이월 만료** | 미확인 | **실행됨(메모리)** | 같은 테스트의 속도 곡선이 만료 지점부터 tick 당 정확히 0.350 감소 |
| 잔류 만료 디스폰 | 실행됨(메모리) | **실행됨(실서버)** | `SHIP_DESPAWNED{LINGER_EXPIRED}` 6건, 원인→결과 tick 차이 **정확히 600** |
| **만료 후 재스폰** | — | **실행됨(실서버)** | 같은 actor 가 714 tick(35.7초) 뒤 **같은 위치**에 재스폰(SC-10) |
| 경계 soft | 실행됨·단언 없음 | **실행 + 단언됨** | `soft_boundary_pulls_toward_origin_without_blocking_thrust` (a)(b) |
| 경계 hard | 실행 + 단언됨 | 동일 | — |
| 브레이크 | 실행 + 단언됨 | 동일 | — |
| 퇴화 쿼터니언 | 단위만 / 카운터 미관측 | **여전히 카운터 미관측** | `aim_degenerate_total` 델타 **0** — S-A 가 막는다 |
| `world_full` 거부 | 실행됨 | 동일 | `i44_world_full_exempts_a_resuming_actor` 3/3 |
| tick 상한 초과 | 실행됨(테스트) | **실서버 미실행** | `commands_dropped_over_tick_cap_total` 델타 **0** — S-A 가 막는다 |
| **`SET_SHIP_CONTROL` 소켓 경로** | — | **🔴 한 번도 실행된 적이 없다** | §2.1. `ws_integration.rs` 에 `SET_SHIP_CONTROL` **0회** |
| **재개(잔류 창 안)** | 실행됨(메모리) | **실서버 미실행** | S-C 가 막았다(SC-11) |
| **종료 디스폰** | 실행됨(메모리) | **실서버 미실행** | 정상 종료가 2/2 실패(S-C) → `SERVER_SHUTDOWN` 이벤트 0건 |
| **`Reconciliation.Reconcile()` × 실서버 데이터** | — | **실행된 적 없음** | architect 가 "열린 고리"로 수용. SC-53·54·55 는 전부 합성 입력(§1.6) |

**라운드 1 에서 "통과한 테스트 수는 경로 실행의 증거가 아니다"를 적었을 때 근거는 `world_full` 하나였다.
라운드 2 는 그보다 훨씬 큰 사례를 찾았다** — 이 슬라이스의 주제인 함선 조작 명령이 소켓으로는 한 번도
시뮬레이션에 닿은 적이 없고, 게이트웨이 통합 테스트가 그 명령을 한 번도 보내지 않아 154개 테스트가 전부 초록이었다.

---

## 7. 라운드 2 집계

### 7.1 판정

| 판정 | 수 | ID |
|---|---|---|
| **PASS (재판정: FAIL → PASS)** | 3 | SC-18 · SC-19 · SC-46 |
| **PASS (신규)** | 14 | SC-08 · SC-09 · SC-10 · SC-12 · SC-28 · SC-29 · SC-30 · SC-51 · SC-52 · SC-55 · SC-68(e) · SC-79 · SC-80 · SC-87 |
| **FAIL** | 12 | SC-11 · SC-13 · SC-14 · SC-23 · SC-24 · SC-31 · SC-32 · SC-33 · SC-66 · SC-67 · SC-68(f)(g) · SC-76 |
| 사전 확인만 (판정 보류) | 3 | SC-58(블록 6) · SC-64 · SC-65(블록 7) |

**FAIL 12건 중 11건은 server 결함 3개(S-A·S-B·S-C)가 직접 막은 것이다.** 나머지 1건은
SC-32 로, 관측 기회를 **두 번 다 서버가 가져갔다**(§2.8·§4.7).

### 7.2 담당자별

| 담당 | 항목 | 요약 |
|---|---|---|
| **server** | **S-A** | `SET_SHIP_CONTROL` 게이트웨이 디스패치 + **소켓을 통과하는 통합 테스트** |
| **server** | **S-B** | 세션이 닫히지 않는다(유령 세션·CLOSE_WAIT·`IDLE_TIMEOUT` 미발동) |
| **server** | **S-C** | 무응답 + `shutdown` 무효 (3회 중 2회) |
| **client** | **C-A / CL-1** | SC-55 의 실S6 각속도 대조(AC-12 f). **FAIL 이 아니라 남은 작업** — 판정은 PASS |
| server(권고) | — | SC-40(f) `10.0` 거부 단언, `determinism.rs:181` 주석 |

### 7.3 회귀 — 간헐 실패

- `cargo test --workspace --locked` **3/3 통과**(154 passed), fmt·clippy 0 → **빌드 쪽 간헐 실패 0**.
- **실서버 쪽은 간헐 실패가 있다**: 세션 3회 중 **2회 서버 멈춤**. 계약 §0.3 대로 **재시도로 덮지 않고 FAIL 로 기록**했다.

---

## 8. 성능 기록 (M-1~M-6) — **이번에도 없음**

S-C 때문에 부하를 걸 수 없었다. 멈춤 직전의 무부하 값만 남긴다(기준선 아님):
`tick_total = 7715`, **`tick_overrun_total = 0`**, `tick_body_us.max = 2037 µs`, RSS 16.6 MB.
**SC-75(tick 초과 비율 ≤ 0.5 %)는 블록 8 에서 판정한다.**

designer 지표 M-7·M-8 은 **S-A 때문에 산출 자체가 불가능하다** — 함선이 움직이지 않으므로
`brake_uses_per_minute`·`full_thrust_time_ratio`·`ship_speed_mps` 가 전부 0 이거나 정의되지 않는다.

---

## 9. 계약 외 발견

1. **`bots run --out` 이 상대 경로면 `tools/bots/` 기준으로 풀린다.** qa 가 산출물을 옮겼다. 다음 턴은 절대 경로를 쓴다.
2. **`ws_connections` 게이지를 신뢰할 수 없다**(S-B). 블록 8 의 "31 연결" 확인에 이 값을 단독 근거로 쓰면 안 된다 — OS `netstat` 과 대조해야 한다.
3. **라운드 1 의 유령 함선이 DB 에 영구히 남는다.** 첫 구간의 짝 없는 `SHIP_SPAWNED` 1건은 서버를 하드 종료했으므로
   디스폰이 발행되지 않았고, **과거 기록은 수정하지 않는다**(절대 원칙 5). 다음 라운드의 SC-14 는
   **새 tick 구간**으로 판정해야 하며, 이 3척은 "그때 서버가 그랬다"는 기록으로 남는다.

---

## 10. 다음 턴 — 진입 전제

### 10.1 블록 4·5·7·8 은 **S-A·S-B·S-C 가 고쳐지기 전까지 진입 불가**

- **S-C 가 최우선이다.** 서버가 멈추면 어떤 실서버 항목도 잴 수 없다.
- S-A 없이는 블록 4(치트)·7(2 클라이언트)·8(부하)의 **거의 전부**가 의미를 갖지 못한다.
- S-B 없이는 SC-13·14·76 과 블록 8 의 연결 수 계측이 성립하지 않는다.

### 10.2 블록 6 (Unity) — client 가 할 일 (라운드 1 목록 갱신)

| # | 항목 | 상태 |
|---|---|---|
| 1 | `Reconcile_MissingS6ReplayAsset_IsRecordedPending` 를 실S6 산출물로 교체 | ✅ **완료** — SC-46·51·52 닫힘 |
| 2 | `ProfilerMarker "Starfall.Snapshot.Handle"` | ✅ **코드 완료**(`GreyboxSession.cs:248·252`). **실측은 블록 6**, Deep Profile 끄고 |
| 3 | SC-59 육안 관찰(mp4 4 + 스크린샷 4) | ⬜ 미실행 — **§0.4 의 유일한 검출기** |
| 4 | **CL-1: SC-55 의 실S6 각속도 대조**(AC-12 f) | ⬜ 남은 작업 (SC-55 판정은 PASS) |
| 5 | **CL-2: AC-13(b) 관찰 ①②③ 을 실서버 세션 로그에**(architect) | ⬜ 블록 6 전에 필요. **①이 0이면 FAIL** |

**블록 6 은 S-C 가 고쳐진 뒤에만 의미가 있다** — Unity 가 실서버에 붙어야 한다.

### 10.3 블록 7 (2 클라이언트) — 사전 확인 완료

CSV 헤더가 **qa 스크립트 · client `ObserverCsv` · 봇** 세 곳에서 문자 단위로 같다(§1.10).
client 의 미확인 리스크 2건은 **블록 3 에서 답이 나왔다**:

| client 의 질문 | 블록 3 관측 |
|---|---|
| ① 서버가 두 세션 모두에 브로드캐스트하는가 | **한다.** 관측자 4명이 각각 265~268건의 스냅샷을 받았다 |
| ② 서로 다른 subject 가 다른 `ship_id` 로 스폰되는가 | **된다.** `actor_id` 4개 → `ship_id` 4개, 각 관측자가 4척 전부를 본다 |

---

## 10.4 리더에게 — **블록 3·4·5 는 이미 끝났다. "방해 없이 진행하라" 의 전제가 성립하지 않는다**

리더의 2026-09-21 지시 §6 은 "client·server 를 지금 깨우지 않는다 / 블록 3·4·5 를 계속하라" 였다.
**그 메시지가 오기 전에 세 블록을 이미 실행했고, 그 과정에서 서버가 두 번 멈췄다**(§3.1·§4.1).

- **더 잴 것이 없다.** 남은 실서버 항목(SC-11·13·14·23·24·31·32·33·66·67·76)은 전부
  **S-A·S-B·S-C 가 고쳐져야** 측정 가능하다. qa 가 지금 더 돌려도 같은 자리에서 멈춘다(3회 중 2회).
- **따라서 G-c 의 "측정 중 빌드 금지" 가 더 이상 server·client 투입을 막지 않는다.**
  qa 의 측정은 끝났고, 다음에 필요한 것은 측정이 아니라 **수정**이다.
- **"블록 3·4 까지만 하고 보고할까" 라는 물음도 이미 지나갔다** — 블록 5 까지 갔고, 그 안에서
  SC-10 과 SC-12 는 **완전히 관측해 PASS** 로 닫았다. 블록 5 에서 못 닫은 것은 SC-11·13·32 뿐이고
  셋 다 서버 결함이 원인이다.

**권고 순서**: **S-C(무응답) → S-B(세션 미종료) → S-A(`SET_SHIP_CONTROL`)** 를 server 에 함께 투입하고,
같은 시간에 client 에 **CL-1 + CL-2** 를 투입한다(둘은 파일이 겹치지 않는다).
그 뒤 라운드 3 에서 블록 4·5 를 다시 돌리고 블록 6·7 로 넘어간다.

---

## 11. 요약 — 여전히 증명하지 못한 것

> **손 방향 부호가 통째로 뒤집혀도 자동 검증은 전부 통과한다.** 서버와 클라이언트가 같은 공식을 쓰므로
> 예측 오차 0, fixture 왕복 통과, 결정성 바이트 동일까지 전부 초록이다. 특히 오토레벨의 `sin_err` 부호
> 한 줄이 그 위험을 진다(ADR-0010 §2.1). **검출기는 SC-59(AC-14 a·d)의 육안 관찰 하나뿐이고,
> 그것을 사람이 실제로 볼 때까지 이 슬라이스는 부호에 대해 아무것도 증명하지 못한다.**

**M-14: 부호 미검증.** 라운드 2 에서도 SC-59 는 실행되지 않았다.

그리고 라운드 2 는 그 위에 한 문장을 더한다:

> **이 슬라이스는 "함선 이동"인데, 함선을 움직이는 명령이 소켓을 통해 시뮬레이션에 도달한 적이 한 번도 없다.**
> 라운드 1 의 PASS 34건과 라운드 2 의 PASS 16건 — Rust 154개 테스트, Unity 145개 테스트,
> 경계면 201행, 두 프로세스 525,272바이트 동일까지 — **전부 그 사실과 양립한다.**
> 그것이 실서버 관측을 계약에 넣어 둔 이유다.
