r1 동결 — client/ server/ contracts/ data/ tools/ tests/e2e/ 편집·빌드 금지 (qa-r1 판정 중)
시작: 2026-10-10 16:03    
HEAD: b3bc8f28948dfaa67b3664dc139ad9aae3a09a96 (+ 미커밋 작업 트리 — 해시 스냅샷은 run_round 의 freeze 단계)
예외: art/ (techart4 의 p1-04 스테이징, Unity 밖), _workspace/ 기록
해제: 리더가 이 파일을 지운다

## 예외 추가 — 2026-10-10 18:3x (리더)

`tests/e2e/run_round.py` · `tests/e2e/round_configs/p1-03-trading.json` 의 **기대값 반전(expect_fail) 추가**를 허용한다.

근거: r1 의 `judges: FAIL` 은 제품 결함이 아니라 **하네스 결함**이다 — `sc32_negative_without_sales` 는
계약이 `(FAIL 이어야 한다)` 로 적은 **음성 대조**이고 실제로 올바르게 실패했는데(판매 있는 광물
ferrosite·glacine 둘만 `holds:false`, `denominators_ok:true`), 실행기가 `rc != 0` 을 단계 실패로 집계했다.

**지금 고치는 이유**: 안 고치면 다음 사람이 "음성 대조를 통과하게" 고쳐서 **대조 자체를 지울** 위험이 있다.
그 형태는 p1-01 에서 열두 번 나왔다.

**범위 제한**: 판정 임계·단언·계약은 건드리지 않는다. 바꾸는 것은 **기대값을 선언하는 자리**뿐이고,
`db_stop` 실행이 끝난 뒤에 손댄다(동시 편집 금지).

## 예외 추가 2 — 2026-10-10 19:0x (리더, 사용자 승인)

`tools/bots/src/trade.rs` 의 **`cycles` 분모 한 줄**(200 → 150)을 허용한다.

근거: r1 의 `bots: 미검증(조건 미발생)` 은 선단언 `min_ws >= 31` 이 깨진 것이고,
**지금 설계로는 만족될 수 없다.**

```
cycles = duration/200 = 6 · 한 사이클 ≈ 190~270s
6 × 190s = 1,140s < 측정 창 1,200s     ← 빠른 쪽 봇이 창 종료 전에 완주하고 연결을 닫는다
```

주석이 *"넉넉히 반복하고, 측정 창은 duration 으로 자른다"* 고 적었는데 **200 이 넉넉하지 않다.**
그리고 고갈(`inner-belt-1` 12,000 kg 에 봇 5대 × 2,000 kg × 6 사이클 = 60,000 kg 요구)이
`MineUntil` 을 120 s 타임아웃으로 밀어 사이클을 **짧은 쪽**으로 몰기 때문에 더 자주 걸린다.

150 으로 하면 8 사이클 → 8 × 190s = 1,520s > 1,200s 로 여지가 생긴다.

**범위 제한**: 판정 임계·단언·계약·데이터(광맥 보유량)는 건드리지 않는다. 바꾸는 것은
**봇이 측정 창을 덮는 반복 수**뿐이다. 보유량을 늘리는 쪽은 designer 소유이므로 손대지 않는다.

### 정정 — 위 "예외 추가 2" 의 근거를 실측으로 바꾼다 (원칙 5: 위 문단은 고쳐 쓰지 않는다)

위에서 나는 *"주석이 넉넉히 반복이라 적었는데 200 이 넉넉하지 않다"* 고 썼다. **그 읽기가 얕았다.**
같은 블록의 다른 주석이 **200 이 이미 고친 결과**임을 적고 있다:

> 사이클 수를 창 길이에 맞춘다(2026-10-07 첫 실행: 넉넉히 잡은 사이클 때문에 창이 끝난 뒤
> 20 분 넘게 돌아 바깥 timeout 에 걸려 JSON 을 못 썼다). 남은 연결은 창 + 5 분 뒤 끊고
> `connections_unfinished` 로 센다.

즉 200 은 **반대 방향 결함을 고치려고 고른 값**이고, 내가 그것을 되살릴 뻔했다.
되살아나지 않는 이유는 **유예 절단(창 + 300 s)이 그 뒤에 추가됐기 때문**이다 —
`case_timeout 2400` 안에서 끝나므로 2026-10-07 의 실패는 재발하지 않는다.

**근거를 실측으로 다시 세웠다** — r1 load 의 서버 로그에서 세션 지속 시간(n=62):

```
min 1,066s · p25 1,157s · 중앙 1,192s · p75 1,239s · max 1,510s(유예 절단)
측정 창 1,200s · 평균 사이클 = 1,192 / 6 ≈ 199 s
```

**분모 200 이 실측 사이클 199 s 와 거의 같다.** 그래서 봇이 창 경계에 몰려 끝나고 약 절반이
먼저 닫는다 — `min_ws 13` 이 그것이다. 150 으로 하면 8 사이클이고,
**가장 빠른 봇(178 s/사이클)도 8 × 178 = 1,424 s > 1,210 s 로 창을 덮는다.**

**예상되는 부작용**: `connections_unfinished` 가 크게 는다(유예 1,500 s 에서 절단).
그 카운터가 바로 그 용도라고 주석이 적고 있다. **판정식에는 들어가지 않는다**(`trade.rs:1238`).

## 예외 추가 3 — 2026-10-10 22:4x (리더)

`server/crates/sim/tests/` 의 **세 부모 테스트에 자식 관측 추가**를 허용한다 —
`trading_replay.rs` · `mining_replay.rs` · `determinism.rs`.

근거(architect D-1 후속, 리더 검산): 부모가 자식 stdout 을 `assert!` **실패 메시지에만** 쓴다.
**성공 경로에 자식의 흔적이 없다.** `gates/test.log` 에 글자까지 같은 짝 셋이 있다:

```
:558/:559  replay_worker         ... ignored  /  ac8_two_process_replay…   ... ok
:566/:567  mining_replay_worker  ... ignored  /  sc33_two_process_replay…  ... ok
:583/:584  trading_replay_worker ... ignored  /  sc40_two_process_replay…  ... ok
filters/logs/040_SC-40_…sc40_two_process….log  →  자식 흔적 0   ← 게이트가 읽을 자리
```

**`spawn_replay_child` 를 in-process 호출로 바꿔도 위 두 줄이 글자 하나 안 변한다.**
SC-40·SC-33·AC-8 이 요구한 *"서로 다른 프로세스에서 2 회"* 중 **절반에 관측이 없다**
— 형태 (나) 검출기 사망형.

**왜 지금 허용하는가**: r1 의 모든 단계가 돌았고 증거가 완성됐다
(`bots` PASS · `judges` PASS · `db_stop` PASS, `evidence/r1_20261010{,c,f}`).
**리포트는 증거를 읽으므로 소스 수정이 판정을 바꾸지 않는다.**

**왜 과거 PASS 를 무르지 않는가**: 이 수정은 **제품에 대한 단언을 바꾸지 않는다.** 바뀌는 것은
**로그가 무엇을 증명하는가**뿐이다. 과거 PASS 는 **관측되지 않은 가정에 기대 있었고**, 이 수정은
그 가정을 앞으로 보이지 않게 두는 것을 멈춘다. **소급 재판정이 아니라 증거 형태의 보강이다.**

**범위 제한**: 판정 임계·단언의 **내용**을 바꾸지 않는다. 테스트가 제품에 대해 주장하는 것은
그대로다. 더하는 것은 **자식이 실제로 돌았다는 관측**뿐이다. 골든 재생 파일 금지.
**계약 문면은 여전히 동결**이다 — architect 의 `⊘` 칸 제안은 리포트 뒤에 적용한다.

## 예외 추가 4 — 2026-10-10 2x:xx (qa, 리더 배차)

`tests/e2e/cargo_sc_map.py` 의 **역할 분류 추가**를 허용한다 — architect D-1 §9.1~6 의 구현.

무엇: Rust 필터마다 `role` ∈ {RUNNABLE, CHILD, MANUAL}(+UNCLASSIFIED) 를 **Rust 소스 속성**
(`#[ignore]` + 부모의 spawn 리터럴)에서 읽고, CHILD 는 단독 실행하지 않고 `--list --ignored` 존재 +
부모 필터 로그의 자식 실행 수로, MANUAL 은 `--ignored` 로 만족시킨다. 항등식 ①②③ 과 산문 RUNNABLE
분모를 매 실행 출력에 찍고, 분류가 깨지면 `exit 4`(판정 보류)를 낸다.

왜: r1 `filters: FAIL` 의 유령 지명 1 건(`SC-40 trading_replay_worker`)은 **계약이 아니라 도구의
결함**이다. 계약은 부모를 올바르게 지명하고 있고, 자식은 설계상 `#[ignore]` 다.

범위 제한: **계약 문면을 고치지 않는다**(architect 의 `⊘` 칸 제안은 리더가 적용 시점을 정한다).
`server/` · `tools/bots/` · `contracts/` · `data/` · `client/` 를 고치지 않는다. **분모 66 을 줄이지
않는다** — CHILD 행은 분모에 남고 다른 조건으로 만족된다. **자식 실행 기대값(2)을 도구에 박지 않는다** —
세서 찍기만 한다. 음성 대조 `Fixes.ignore_role=False` 와 돌연변이 6 종으로 검출기 생존을 확인했다.

### 예외 4 의 범위 안 — `tests/e2e/run_round.py` 두 줄 (qa)

1. `stage_filters` 의 `exit_map` 에 `4: FAIL` 을 **명시**한다. exit 4(판정 보류)는 지금 매핑이 없어
   fallthrough 로 FAIL 이 되는데, 선언되지 않은 통로는 다음에 조용히 바뀐다.
2. `stage_filters` 의 metrics 통과 목록에 `roles · ignore_census · prose_runnable · child_rows ·
   child_unobserved · child_parent_missing · ran_ge_1_self · child_proven` 을 더한다.
   D-1 만기의 수들이 `cargo_sc_map.out` 에만 있으면 **리포트가 보는 자리(`summary.json`)에는 없다.**

판정 임계·단언·계약은 건드리지 않는다. 바꾸는 것은 **수가 어디에 찍히는가**뿐이다.

### 예외 4 추가분 — `--nocapture` 와 미검증 분기 (qa, 리더 지시)

`tests/e2e/cargo_sc_map.py` 에 넷을 더한다. **계약·`server/` 불변, 분모 66 유지.**

1. **CHILD 행의 부모 필터만** `--nocapture` 로 돌린다(실측 1/66 필터). 부모가 자식 관측을
   `println!` 로 찍고 libtest 가 성공 경로에서 그것을 캡처하므로, 플래그 없이는 로그에 줄이 없다.
   전부에 붙이지 않은 이유: 필요한 로그가 그 하나뿐이고 나머지는 로그만 커진다.
2. **`--nocapture` 없이 돌린 CHILD 는 `exit 4`(미검증)** — PASS 도 FAIL 도 아니다.
   `child_measurable` 로 측정 경로를 따로 찍는다. **"흔적이 없다 → PASS" 와 "자식이 안 돌았다 →
   FAIL" 을 둘 다 막는다.**
3. **측정 경로가 열렸는데 실행 증거 0 이면 `exit 1`** (만기 #5 종료값 "각 ≥ 1"). "정확히 2 번" 은
   여전히 계약 `⊘` 칸의 일이고 **게이트에 2 를 박지 않았다**(부모 로그가 없으면 0 이 나오는 것을
   selftest 가 단언한다).
4. **`[CHILD] … child_pid` 비교**: 같은 pid 두 줄이면 한 프로세스를 두 번 적은 것 → `exit 1`.
   자식 pid = 부모 pid 도 `exit 1`. 그리고 **`0 passed` 는 실행으로 세지 않는다** — rust 의 음성
   대조에서 determinism 자식이 `0 passed; 2 filtered out` 으로 `exit 0` 을 냈다.

selftest 35 케이스 · 돌연변이 12 종 전부 사망(`…/scratchpad/mut/m{1..12}*.py`).

### 예외 4 정정분 — `--nocapture` 를 걷고 `-- --show-output` 으로 (qa, 리더 판정 + architect D-2)

앞의 "예외 추가 4 … `--nocapture`" 를 **되돌리고** 아래로 바꾼다(원칙 5: 위 문단은 고쳐 쓰지 않는다).

1. **`tests/e2e/run_round.py` `stage_gates`**: 표준 게이트 두 명령에 `-- --show-output` 을 붙인다
   (`cargo test --workspace --locked --no-fail-fast` · `release_*`). **증거가 `gates/test.log` 에 있어야
   한다** — 게이트 전용 실행에만 두면 CI 는 눈먼 채 남는다.
2. **`tests/e2e/cargo_sc_map.py`**: `--nocapture` 를 걷고 `--show-output` 으로. 자식 입증의 **주 우물은
   `--test-log`(표준 게이트 로그)** 이고, 세는 자리는 `---- <부모> stdout ----` **블록 안**이다
   (부모 귀속이 로그 구조에서 나온다). `--nocapture`·raw write 금지 — libtest 가 `test <이름> ... ` 과
   `ok\n` 을 나눠 쓰는 사이에 끼어 **결과 줄을 splice 한다**(architect D-2 실측).
3. **기대 회차를 소스에서 센다** — 부모 본문의 **spawn 헬퍼 호출 수**. 실측 세 부모 전부 2.
   도구에 2 를 박지 않았다(소스가 3 을 요구하는 입력에서 3 이 나오는 것을 selftest 가 단언).
4. **`child_unobserved` 가 비어 있지 않으면 PASS 가 아니라 `미검증(관측 없음)`**(리더 판정).
   새 종료 코드 **5** 와 새 상태 `NOOBS = "미검증(관측 없음)"`. exit 4 는 분류 깨짐 전용으로 좁혔다.
5. **`-q` 금지의 기계 판**: 결과 줄은 통과를 말하는데 `test … ok` 줄이 0 이면 **유령이 아니라 미검증**.
   유령 66 으로 인쇄하면 다음 사람이 집계를 느슨하게 고친다(architect D-2 부수 발견).
6. **`aggregate()` 의 선언되지 않은 통로를 막았다** — 목록에 없는 `미검증(…)` 이 **조용히 PASS 로
   떨어지고 있었다.** NOOBS 를 처음 붙여 돌렸을 때 `cargo_sc_map` 이 exit 5 를 냈는데도 단계가
   PASS 로 올라와 **실행으로 잡았다.** 기존 상태(ENV·NOCOND·FAIL)의 동작은 바뀌지 않는다
   (HUMAN·NOINPUT 은 `stage.status` 에 직접 쓰여 `aggregate` 에 오지 않는다 — 확인했다).
7. `identities` 셋을 `summary.json` metrics 에 **값째로** 올린다.

검증: `cargo_sc_map` selftest **43 케이스** · 돌연변이 **19 종 전부 사망** ·
`run_round` selftest **31 케이스**(aggregate 통로 3 케이스 추가).
**계약 문면·`server/`·`contracts/`·`data/`·`client/`·`tools/bots/` 불변. 분모 66 유지.**

### 예외 4 추가분 2 — 부모 센서스(분모 3) · parent_census 를 metrics 로 (qa, 리더 배차)

`tests/e2e/cargo_sc_map.py` · `tests/e2e/run_round.py`.

리더가 실측으로 두 걱정을 닫았다(`--workspace` 전달 O · 로그 2.7배 98 KB). **그 걱정을 단언으로
남긴다**: `parent_census` 가 레포의 `#[ignore]` 자식을 spawn 하는 **부모 전체(분모 3)** 를 표준 게이트
로그에 대고 세고, 세 부모 각각 자식 실행 수가 **소스가 요구하는 회차와 정확히 같은지** 본다.
일부만 찍히면 **유령이 아니라 미검증(exit 5)** 이다 — "일부만 찍힌 로그로는 판정할 수 없다".
pid 이상(같은 pid·부모와 같음)은 exit 1.

**계약이 지명하지 않은 부모 둘(mining·determinism)은 판정 대상이 아니라 로그 건전성의 증거다** —
분모 3 을 줄이지 않는다(architect 만기 #5).

범위 제한: 판정 임계·계약·`server/` 불변. 더하는 것은 **수와 그 수가 어디에 찍히는가**뿐이다.
검증: selftest **47 케이스** · 센서스 돌연변이 5 종 포함 **24 종 전부 사망**.
