# p1-03-trading QA 리포트 — 라운드 1 (Phase 5)

- 일시: 2026-10-10 16:04 ~ 2026-10-11 00:09 (KST). 단계별 재실행이 여러 번 있었고 **정본 증거 디렉터리가 단계마다 다르다**(아래 §증거 색인).
- 기준: `02_sprint_contract.md` (133 항목, §8.2 규칙 1~7 승계). **판정 중 계약 문면은 고치지 않았다** — architect 의 `⊘` 칸 제안은 이 리포트 뒤 적용 예정(FREEZE).
- 트리: HEAD `b3bc8f28948dfaa67b3664dc139ad9aae3a09a96`, `git status --porcelain` **155 줄**(이 브랜치의 미커밋 변경 — 더러운 트리를 그대로 적는다).
- 편집 동결: `FREEZE`(2026-10-10 16:03, 리더). 동결 중 허용된 예외 **네 건**이 그 파일에 기록돼 있다 — ① `run_round.py`/round config 의 기대값 반전, ② `tools/bots/src/trade.rs` 의 `cycles` 분모 한 줄, ③ `server/crates/sim/tests/` 세 부모의 자식 관측 추가, ④ `tests/e2e/cargo_sc_map.py` 역할 분류 + `--show-output` + 미검증 분기(+ `run_round.py` 의 집계·metrics). **①②④ 는 판정 도구 쪽이고 ③ 은 제품에 대한 단언을 바꾸지 않는다**(로그가 무엇을 증명하는가만 바꾼다).
- 서버 바이너리: sha256 `da37b39f88f56232204ae574b4fd19b36ce39f1f9afe50f42af61c8439ff1ae4` (`f/bots` metrics).
- 표준 게이트 명령이 이 라운드에서 바뀌었다: `cargo test --workspace --locked -- --show-output` (CLAUDE.md §명령 표 · `.github/workflows/gates.yml`). 근거와 비용은 §게이트 명령 변경.

## 요약

**PASS 117 / FAIL 0 / 미검증 16 / 전체 133**

| 구분 | 항목 |
|---|---|
| FAIL | **없음** |
| 미검증 — 사람(최종 세션, UI 요소 ③) | SC-65 · 68 · 71 · 74 · 77 · 80 · 83 · 86 · 89 · 92 · 95 · 98 · 101 (13) |
| 미검증 — 사람(PR 체크 줄 미기입) | SC-131 (qa 몫 ⊘ 는 통과) |
| 미검증 — CI 첫 실행 전 | SC-130 |
| 미검증 — 건너뜀(사용자 결정) | SC-133 |

**단계별 결과(전부 PASS)**

| 단계 | 결과 | 정본 | 핵심 수 |
|---|---|---|---|
| freeze | PASS | 각 실행 | HEAD·porcelain·지문 전후 동일 |
| gates | PASS | `r1_20261010j` | fmt·clippy rc0 · workspace **386 passed / 0 failed / 7 ignored** · release sim rc0 |
| census | PASS | `r1_20261010` | 기대 16 == 실행 16 · 누락 0 · 건너뜀 0 (실행 줄 51) |
| repeat | PASS | `r1_20261010` | 10/10 회 **386/0/7** 동일 · 실패 이름 0 |
| unity | PASS | `r1_20261010` | EditMode 492/0(skip 2) · PlayMode 17/0 |
| filters | PASS | `r1_20261010s` | 아래 §filters |
| sources | PASS | `r1_20261010` | 133/133 도구 지명 · 위반 0 · Rust 항등식 ①② OK |
| offline | PASS | `r1_20261010` | 28 명령 rc0 · codegen 22 파일 run1==run2==커밋본 · golden 3 벌 baseline 일치 |
| bots | PASS | `r1_20261010f` | 37 명령 rc0 · 6 월드 전부 `SHUTDOWN exit=0` |
| judges | PASS | `r1_20261010f` | 20 명령 — 음성 대조 `sc32_negative_without_sales` 는 **rc 1 이 PASS**(기대값 반전) |
| db_stop | PASS | `r1_20261010c` | `sc39_accepted` rc0 · DB 지문 전/후 동일(`4805\|d1904d8f…\|4\|0`) |

## 증거 색인 — 단계마다 정본이 다르다

이 라운드는 r1 하나이고 재실행이 16 개 디렉터리로 남았다. **디렉터리를 섞어 읽으면 결론이 뒤집힌다.**

| 단계 | 정본 | 왜 본 실행이 아닌가 |
|---|---|---|
| gates | `r1_20261010j` | **본 실행(`r1_20261010`)의 `gates` 는 옛 명령이다** — `--show-output` 이 없어 `[CHILD]` 0 줄 |
| filters | `r1_20261010s` | 본 실행은 FAIL(유령 1). `s` 는 Rust 66 + **Unity 맨 이름 35** 를 같이 검사한 유일한 판 |
| bots | `r1_20261010f` | 본 실행 미검증(조건 미발생), `e` 는 FAIL. `f` 가 첫 전부 통과 |
| judges | `r1_20261010f` | 본 실행 FAIL(하네스 결함), `d` 입력 없음 |
| db_stop | `r1_20261010c` | 본 실행에 없다(사람 승인 단계) |
| census·repeat·unity·sources·offline | `r1_20261010` | 본 실행이 정본 |

**`m_noshow` 와 `n_noshow` 는 둘 다 인용한다 — 하나만 남기면 증명이 사라진다**(§미검증 분기).
`m_noshow` 의 `filters: PASS` 를 **PASS 증거로 읽지 마라.** 그것은 **결함이 있던 판**의 출력이다.

## filters — 역할 분류와 자식 입증 (`r1_20261010s`)

본 실행에서 `filters: FAIL`(유령 지명 1: SC-40 `trading_replay_worker`)이었다. **계약은 틀리지 않았다** — SC-40 행은 부모를 올바르게 지명하고 자식은 설계상 `#[ignore]` 다. 도구가 설명용 백틱 식별자를 필터 지명으로 읽었다. architect D-1 의 처방대로 **역할을 Rust 소스 속성에서** 읽게 했다(`#[ignore]` + 부모의 spawn 리터럴 — 둘 다 오타가 조용히 면제로 바뀌지 않는다).

```
executed 66 · ran_ge_1 66 · 유령 0 · 실패 테스트 0 · rc≠0 0 · 다른 SC 도 잡는 테스트 0
Unity 맨 이름 35 · xml 에 없음 0
역할(소스에서): RUNNABLE 65 · CHILD 1 · MANUAL 0 · UNCLASSIFIED 0
항등식 ① Rust 필터 = RUNNABLE+CHILD+MANUAL+UNCLASSIFIED   66 = 65 + 1 + 0 + 0   성립
항등식 ② 자기 실행 + 자식 입증 + 로그 못 읽음 = Rust 필터   65 + 1 + 0 = 66      성립
항등식 ③ 레포 #[ignore] = CHILD + MANUAL + 미분류          5 = 3 + 2 + 0        성립
```

**분모 66 을 66 으로 유지했다.** 느슨하게 가는 쪽(백틱 맨 이름 무시)은 고르지 않았다 — architect 실측으로 그러면 필터 66 → 61 이고 **§1 SC 32 개가 분모에서 조용히 사라진다.**

**자식 입증(CHILD SC-40)**

| 수 | 값 |
|---|---|
| `--list --ignored` 존재 | 1 (단독 실행하지 않는다 — `--ignored` 로 돌리면 환경 변수 없이 패닉해 **옳은 구현이 FAIL** 이 된다) |
| 증거 우물 | **표준 게이트 로그** `r1_20261010j/gates/test.log` (게이트 전용 실행이 아니다) |
| 블록 | `---- sc40_two_process_replay_with_trading_is_byte_identical stdout ----` |
| 자식 실행 수 | **2** · 이름 언급 3 (`... ignored` 줄은 건너뜀 기록이라 세지 않는다) |
| 소스가 요구하는 회차 | **2** = 부모 본문의 `spawn_replay_child(` 호출 수 — **도구에 2 를 박지 않았다** |
| 자식 pid | 59852 / 33900 — **서로 다르고** 부모 38644 와도 다르다 |

**부모 센서스 — 분모 3**(계약이 지명하든 않든, 레포의 `#[ignore]` 자식을 spawn 하는 부모 전체)

| 자식 | 부모 | 자식 실행 / 소스 요구 | pid |
|---|---|---|---|
| `mining_replay_worker` | `sc33_two_process_replay_with_mining_is_byte_identical` | **2 / 2** | 67732 · 28356 |
| `replay_worker` | `ac8_two_process_replay_produces_byte_identical_snapshots` | **2 / 2** | 68716 · 17196 |
| `trading_replay_worker` | `sc40_two_process_replay_with_trading_is_byte_identical` | **2 / 2** | 59852 · 33900 |

일부만 찍힌 부모 0 · pid 이상 0. **분모 3 중 이 계약이 판정하는 것은 1 개뿐이다** — `mining_replay`·`determinism` 은 p1-03 계약에 CHILD 필터가 없어 **게이트가 판정하지 않는다.** 위 둘은 로그 건전성의 증거이고, 그 사실을 분모에 남긴다(만기 #5).

**산문 RUNNABLE 분모 — Rust 4 · Unity 맨 이름 35.** 이것이 남는 가장 큰 구멍이다: 계약 산문에 적힌 **돌 수 있는** 테스트 이름은 돌고 초록이 되어 분자를 부풀리고 아무 신호가 없다. **이번 라운드에서 닫지 않았다 — 수만 센다.** 세지 않으면 "구멍이 있다" 와 "구멍이 없다" 가 같은 출력이다. (혼입을 재는 유일한 수는 `cross_sc` = **0** 이고, 그것은 다른 SC 가 같은 테스트를 지명할 때만 걸린다.)

## 미검증 분기 — "재지 못한 실행" 과 "재서 통과한 실행"을 가른다

세 부모는 자식 관측을 `println!` 로 찍는데 **libtest 가 성공한 테스트의 출력을 캡처한다.** 플래그가 없으면 그 줄이 로그에 없다.

| 실행 | `--test-log` | 결과 |
|---|---|---|
| `r1_20261010m_noshow` | `r1_20261010/gates/test.log`(플래그 없음) | `filters: **PASS**` ← **결함 판.** 관측 0 을 통과로 읽었다 |
| `r1_20261010n_noshow` | 같은 로그 | `filters: **미검증(관측 없음)**` rc 5 ← **수정 판** |
| `r1_20261010s` | `r1_20261010j/gates/test.log`(플래그 있음) | `filters: PASS` rc 0 · 자식 실행 2/2 |

**같은 입력에 코드만 바뀌어 PASS 와 미검증이 갈린다.** 두 디렉터리 중 하나만 남기면 이 증명이 사라진다.

종료 코드와 상태를 셋으로 갈랐다. **두 미검증 사유를 합치지 마라** — 처방이 정반대다:

| 코드 | 사유 | 상태 | 처방 |
|---|---|---|---|
| 1 | 유령 · 실패 · 자식 회차 불일치 · pid 충돌 | `FAIL` | 구현을 고친다 |
| 4 | **분류가 깨졌다**(`#[ignore]` 미분류 · 역할 UNCLASSIFIED · 부모 미지명 · 항등식 불성립) | `FAIL` | 도구·계약을 고친다. **미검증이 아니다** |
| 5 | **관측이 없다**(`--show-output` 블록 없음 · 증거 0 · `-q` 로 로그 못 읽음) | `미검증(관측 없음)` | **측정 경로를 연다** |
| — | 세상에서 조건이 일어나지 않았다 | `미검증(조건 미발생)` | **부하·데이터를 고친다** |

마지막 두 줄이 다른 사실이다. r1 의 첫 `bots` 가 `미검증(조건 미발생)`(`min_ws` 가 31 에 한 번도 닿지 않았다)이고, `n_noshow` 가 `미검증(관측 없음)`(자식은 **실제로** 두 프로세스로 돌았고 — pid 59852/33900 — 우리가 볼 수 없었다)이다.

## 게이트 명령 변경 — 비용과 전달 확인

`-- --show-output` 을 고른 이유(architect D-2 실측): `--nocapture` 와 raw write 는 캡처를 우회하지만 **libtest 가 `test <이름> ... ` 과 `ok\n` 을 나눠 쓰는 사이에 끼어 결과 줄을 splice 한다**(단일 스레드에서 5/5 회). 그러면 `TEST_LINE` 이 그 줄을 못 읽어 **그 부모가 거짓 유령이 된다** — 관측 장치가 관측 대상을 망가뜨린다. `--show-output` 은 캡처를 유지하고 실행 뒤 블록으로 찍는다(오염 0/13).

```
gates/test.log   37,683 → 100,490 bytes (×2.67 · +61 KB)
테스트 합계       전/후 동일: passed 386 · failed 0 · ignored 7 · 테스트 줄 391   ← 동작 변화 없음
--workspace 전달  전달된다. 블록이 contracts·gateway·sim·persistence 여러 크레이트에서 나오고
                 세 부모 전부 [CHILD] 2 줄. "일부만 찍힘" 은 없다 — 그래도 부모 센서스가 매 실행 단언한다
```

**`-q`·`--quiet` 를 넣지 마라.** 실측: `cargo test -q` 는 `test X ... ok` 줄을 **아예 안 찍는다**(점 진행). 그러면 모든 필터가 유령이 되거나, 누가 거기 맞춰 집계를 느슨하게 고치면 전부 초록이 된다. 도구는 그 형태를 **유령이 아니라 미검증**으로 읽는다(결과 줄은 통과를 말하는데 테스트 줄이 0).

## SC-117 — PASS 이지만 **덮는 범위가 얇아졌다**

`cycles` 분모 200 → 150(FREEZE 예외 ②)이 ⊘ 전제를 충족시켰다. 그런데 같은 변경이 판매량을 줄였다.

| 실행 | ⊘ `min_ws` = 31 | 수락 | 광물별 판매량 | 판정 |
|---|---|---|---|---|
| `r1_20261010` (분모 200) | **13** ✗ | 127 | cobaltine 5,040 · ferrosite 55,600 · glacine 31,100 (**3 종**) | 미검증(조건 미발생) |
| `r1_20261010e` (분모 150) | 31 ✓ | 8 | glacine 6,450 (**1 종**) | PASS |
| `r1_20261010f` (분모 150) | 31 ✓ | **22** | glacine **14,200** (**1 종**) | **PASS(정본)** |

**PASS 인 실행이 판매를 가장 적게 본 실행이다.** 전제(31 연결 유지)와 내용(판매량·광물 다양성)이 **서로 당긴다**: 사이클을 늘려 봇이 측정 창을 덮게 하면 `inner-belt-1` 고갈이 빨라져 수락이 줄고 광물이 한 종으로 수렴한다(`RESOURCE_DEPLETED` 766). 계약 ⊘ 는 "판매 0 이면 FAIL" 만 요구하므로 **22 는 문자 그대로 통과**다.

**판정은 PASS 로 둔다**(계약에 없는 기준을 평가 중에 만들지 않는다). **다만 이 PASS 를 "부하에서 거래 경제가 다양하게 돈다" 로 읽으면 안 된다.** 분모를 바꿀 것인지, ⊘ 에 광물 종수·수락 하한을 넣을 것인지는 designer·architect·리더 몫이다.

## 수정 요청

**없다.** FAIL 0.

## 결정 필요 (리더)

1. **SC-117 ⊘ 의 전제와 내용이 서로 당긴다**(위 절). 광맥 보유량(designer) 또는 ⊘ 문구(architect) 중 어디서 풀지.
2. **SC-02 방법 칸이 문자 그대로는 불성립.** "grep → 0 매칭" 인데 2 적중이고 둘 다 주석이다. `sim/Cargo.toml` 은 *"주석에 적으면 grep 이 주석에 걸려 검사가 무의미해진다"* 고 적고 이름을 피하는데 `history/Cargo.toml:20-21` 은 그 이름들을 적는다. **두 파일의 규율이 반대다.** 판정은 실질(의존성 목록)로 PASS 했다.
3. **SC-130 은 CI 첫 실행 전**이라 로컬 rc 0 으로 대체할 수 없다(계약이 PR job 로그를 요구한다).
4. **미검증 라벨 둘을 합치지 말 것** — 위 §미검증 분기 표를 계약 §8.2 나 하네스 문서에 한 줄로 박아 두는 쪽을 권한다. 다음 사람이 "라벨이 둘이네" 로 합칠 자리다.

## 계약 외 발견 (판정에 넣지 않음)

1. **`HistoryHandles.detector_halted_total` 죽은 카운터** (이전 인계 승계).
2. **호스트 일시정지 뒤 tick 따라잡기 폭주** — `evidence/presession/server.log`: `pending_batches` 4,148, stdin `shutdown` 이 시간 초과해 도구가 terminate. 절전/일시정지를 낀 측정은 믿을 수 없다.
3. **`run_round.py` `aggregate()` 의 선언되지 않은 통로** — 목록에 없는 `미검증(…)` 이 **조용히 PASS 로 떨어지고 있었다.** `미검증(관측 없음)` 을 붙여 처음 돌렸을 때 도구가 exit 5 를 내고 상태가 기록됐는데도 단계가 PASS 로 올라와 **실행으로 잡았다.** 고쳤고(`미검증` 접두를 전부 잡는다) selftest 3 케이스를 더했다. 기존 상태(ENV·NOCOND·FAIL)의 동작은 바뀌지 않는다 — `HUMAN`·`NOINPUT` 은 `stage.status` 에 직접 쓰여 `aggregate` 에 오지 않는 것을 확인했다.
4. **`trading_replay.rs` 주석 정정**(판정 영향 없음, 이전 인계 승계).

## 경계면 점검

- **계약 스키마 ↔ Rust serde ↔ C# DTO**: `offline/sc60_codegen` — 생성 22 파일, run1 == run2 == 커밋본(결정적 코드 생성). `sc128_interface_matrix` rc0.
- **도메인 이벤트 ↔ 역사 판정 ↔ 프로젝션**: `judges/sc55_history_replay_cycle` rc0(읽기 전용 재생), `sc123_causation_*` 3 월드 전부 rc0, `sc124_gaps_*` 3 월드 전부 rc0.
- **골든 재생**: `offline/sc41_golden_{move,mining,history}` — 3·4·2 파일 전부 baseline 일치(물리·판정 불변).
- **Unity 맨 이름 35** 전부 결과 xml 적중(유령 0) — `r1_20261010s` 가 이것을 Rust 66 과 **같은 실행에서** 검사한 유일한 판이다.

## 게임 특화 위험

| 위험 | 증거 |
|---|---|
| 서버 판정 우회 | `cases.cheat-sell-{inject,range,fast,overqty,cooldown,unknown}` 6 건 전부 rc0 (SC-106~111) |
| 화폐·아이템 복사 | `sell-dup` · `sell-dup-reconnect` · `sell-dup-cross-kind` · `restart_sell_dup` rc0 (SC-112) · `judges/sc32_conservation_cycle` rc0 + **음성 대조 `sc32_negative_without_sales` 가 올바르게 실패(rc 1 = PASS)** |
| 멱등성 | `restart_sell_{cas,cas_market,recover}` rc0 · `db_stop/sc39_accepted` rc0(수락 ⊆ `processed_commands`) |
| 결정성 | 세 부모 바이트 동일 재생 **서로 다른 프로세스에서 2 회** — 이제 pid 로 기계 확인 · `repeat` 10/10 동일 |
| 기록 불변 | DB 지문 `db_stop` 전/후 동일 · `judges/sc124_gaps_*` 3 월드 |
| 가시성 누출 | `leak.leak-scan-market` rc0 · `trace-sell-b` 판정 입력 = B 가 받은 메시지뿐 (SC-115) |

## CI 가 보지 않는 것

- **SC-130**: PR 의 `judgment gates (python)` job 로그가 이 항목의 증거인데 **그 실행이 아직 없다.** 로컬 `check_item_sources` rc 0 은 대체물이 아니다.
- **`gates.yml` 의 새 명령(`-- --show-output`)은 아직 CI 에서 한 번도 돌지 않았다.** 로컬에서 전달·비용·동작 불변을 실측했지만 **CI 러너에서의 첫 실행은 미관측**이다.
- `gates.yml:118`(tools/bots)에는 플래그를 붙이지 않았다 — 세 부모는 `server/crates/sim/tests/` 에 있어 거기선 로그만 커진다(리더 판단).

## 이 라운드의 방법론 메모 — 다음 사람에게

1. **in-process 치환이 자식 관측 단언을 깨뜨리는지는 아무도 실행하지 않았다.** rust 는 워크스페이스 lint 가 `unsafe`(`set_var`)를 막아 그 경로를 못 썼고, 차선으로 자식 이름 오타를 썼다. 그 대조에서 **determinism 자식이 `0 passed; 2 filtered out` 을 내고도 `exit 0`** 이었고 **종료 상태만 보는 옛 검사는 그 입력에서 초록이었다** — 그건 실측이다. **"이제 CI 가 잡는다" 로 읽지 마라.** 정확한 진술: 단언이 그 회귀의 비용을 *조용한 표류* 에서 *눈에 보이는 삭제* 로 올렸다.
2. **`run_round.py` `stage_filters` 의 `exit_map` 을 `5: PASS` 로 바꾸는 돌연변이는 selftest 로 죽지 않는다.** 죽이는 것은 end-to-end 실행(`n_noshow`)이다. **selftest 만 보면 그 자리는 비어 있다.**
3. **돌연변이 자체도 틀릴 수 있다.** 이 라운드에서 내가 쓴 돌연변이 하나가 가짜였다(`partial = [] or [...]` 는 `[...]` 와 같다). 통과를 보고 "검출기 없음" 으로 읽을 뻔했고, 제대로 고쳐 쓰니 죽었다. **"돌연변이가 살아남았다" 는 "검출기가 없다" 와 "돌연변이가 가짜다" 를 구별하지 못한다.**
4. 검증 규모: `cargo_sc_map` selftest **47 케이스** · 돌연변이 **24 종 전부 사망**. `run_round` selftest **31 케이스**.

## 항목별 결과

표기 — `s/filters` = `r1_20261010s`, `j/gates` = `r1_20261010j`, `f/bots`·`f/judges` = `r1_20261010f`, `c/db_stop` = `r1_20261010c`, `기본/*` = `r1_20261010`. `rc0` 은 그 명령의 종료 코드이고, 단계 상태는 §요약 표에 있다.

| ID | 결과 | 증거 |
|----|------|------|
| SC-01 | PASS | j/gates:fmt rc0 · j/gates:clippy rc0 · j/gates:test rc0 · j/gates:release_starfall-sim rc0 |
| SC-02 | PASS | sim `[dependencies]` = `starfall-contracts` 하나 · history = contracts·serde·serde_json·uuid(v5)·thiserror. **grep 2 적중은 history/Cargo.toml:20-21 주석** — 계약 방법 칸의 "0 매칭" 은 문자 그대로는 불성립(§계약 외 발견) |
| SC-03 | PASS | `git diff --stat b3bc8f2 -- server/crates/history server/crates/persistence/src/history.rs` 빈 출력 · `git status --porcelain -- server/crates/history` 빈 출력 |
| SC-04 | PASS | f/bots:sc04_new_world rc0 · f/bots:sc04_boot rc0 |
| SC-05 | PASS | filters(s) 필터 3/3 실행 ≥1, 잡힌 테스트 17, 실패 0 · f/bots:sc05_reject rc0 |
| SC-06 | PASS | filters(s) 필터 1/1 실행 ≥1, 잡힌 테스트 1, 실패 0 |
| SC-07 | PASS | filters(s) 필터 3/3 실행 ≥1, 잡힌 테스트 15, 실패 0 |
| SC-08 | PASS | filters(s) 필터 1/1 실행 ≥1, 잡힌 테스트 1, 실패 0 |
| SC-09 | PASS | filters(s) 필터 1/1 실행 ≥1, 잡힌 테스트 1, 실패 0 |
| SC-10 | PASS | filters(s) 필터 1/1 실행 ≥1, 잡힌 테스트 1, 실패 0 |
| SC-11 | PASS | filters(s) 필터 1/1 실행 ≥1, 잡힌 테스트 2, 실패 0 |
| SC-12 | PASS | filters(s) 필터 1/1 실행 ≥1, 잡힌 테스트 1, 실패 0 |
| SC-13 | PASS | filters(s) 필터 3/3 실행 ≥1, 잡힌 테스트 4, 실패 0 |
| SC-14 | PASS | filters(s) 필터 1/1 실행 ≥1, 잡힌 테스트 10, 실패 0 |
| SC-15 | PASS | filters(s) 필터 1/1 실행 ≥1, 잡힌 테스트 1, 실패 0 |
| SC-16 | PASS | filters(s) 필터 1/1 실행 ≥1, 잡힌 테스트 2, 실패 0 |
| SC-17 | PASS | filters(s) 필터 1/1 실행 ≥1, 잡힌 테스트 2, 실패 0 |
| SC-18 | PASS | filters(s) 필터 1/1 실행 ≥1, 잡힌 테스트 1, 실패 0 |
| SC-19 | PASS | filters(s) 필터 1/1 실행 ≥1, 잡힌 테스트 1, 실패 0 |
| SC-20 | PASS | filters(s) 필터 1/1 실행 ≥1, 잡힌 테스트 3, 실패 0 |
| SC-21 | PASS | filters(s) 필터 3/3 실행 ≥1, 잡힌 테스트 5, 실패 0 |
| SC-22 | PASS | filters(s) 필터 1/1 실행 ≥1, 잡힌 테스트 1, 실패 0 |
| SC-23 | PASS | filters(s) 필터 1/1 실행 ≥1, 잡힌 테스트 1, 실패 0 |
| SC-24 | PASS | filters(s) 필터 1/1 실행 ≥1, 잡힌 테스트 1, 실패 0 |
| SC-25 | PASS | filters(s) 필터 1/1 실행 ≥1, 잡힌 테스트 1, 실패 0 |
| SC-26 | PASS | filters(s) 필터 2/2 실행 ≥1, 잡힌 테스트 2, 실패 0 |
| SC-27 | PASS | filters(s) 필터 1/1 실행 ≥1, 잡힌 테스트 1, 실패 0 |
| SC-28 | PASS | filters(s) 필터 1/1 실행 ≥1, 잡힌 테스트 1, 실패 0 |
| SC-29 | PASS | filters(s) 필터 1/1 실행 ≥1, 잡힌 테스트 1, 실패 0 |
| SC-30 | PASS | f/judges:sc30_ledger_cycle rc0 |
| SC-31 | PASS | f/judges:sc31_chain_cycle rc0 |
| SC-32 | PASS | f/judges:sc32_conservation_cycle rc0 · f/judges:sc32_negative_without_sales rc1 |
| SC-33 | PASS | f/judges:sc33_saturation_cycle rc0 |
| SC-34 | PASS | filters(s) 필터 1/1 실행 ≥1, 잡힌 테스트 1, 실패 0 |
| SC-35 | PASS | filters(s) 필터 1/1 실행 ≥1, 잡힌 테스트 1, 실패 0 |
| SC-36 | PASS | filters(s) 필터 1/1 실행 ≥1, 잡힌 테스트 1, 실패 0 |
| SC-37 | PASS | filters(s) 필터 1/1 실행 ≥1, 잡힌 테스트 1, 실패 0 · f/judges:sc37_zero_rows_cycle rc0 |
| SC-38 | PASS | filters(s) 필터 1/1 실행 ≥1, 잡힌 테스트 1, 실패 0 |
| SC-39 | PASS | c/db_stop:sc39_accepted rc0 |
| SC-40 | PASS | filters(s) 필터 2/2 실행 ≥1, 잡힌 테스트 1, 실패 0 · **자식 입증: 표준 게이트 로그의 `---- sc40_… stdout ----` 블록에 `[CHILD] worker=trading_replay_worker run=1|2` 2 줄, child_pid 59852 / 33900(서로 다르고 부모 38644 와도 다르다), 소스가 요구하는 회차 2 와 일치** |
| SC-41 | PASS | filters(s) 필터 3/3 실행 ≥1, 잡힌 테스트 15, 실패 0 · 기본/offline:sc41_golden_move rc0 · 기본/offline:sc41_golden_mining rc0 · 기본/offline:sc41_golden_history rc0 |
| SC-42 | PASS | filters(s) 필터 4/4 실행 ≥1, 잡힌 테스트 4, 실패 0 |
| SC-43 | PASS | filters(s) 필터 1/1 실행 ≥1, 잡힌 테스트 1, 실패 0 |
| SC-44 | PASS | filters(s) 필터 3/3 실행 ≥1, 잡힌 테스트 3, 실패 0 |
| SC-45 | PASS | filters(s) 필터 1/1 실행 ≥1, 잡힌 테스트 1, 실패 0 |
| SC-46 | PASS | filters(s) 필터 1/1 실행 ≥1, 잡힌 테스트 1, 실패 0 |
| SC-47 | PASS | filters(s) 필터 1/1 실행 ≥1, 잡힌 테스트 1, 실패 0 |
| SC-48 | PASS | filters(s) 필터 1/1 실행 ≥1, 잡힌 테스트 1, 실패 0 |
| SC-49 | PASS | filters(s) 필터 1/1 실행 ≥1, 잡힌 테스트 1, 실패 0 |
| SC-50 | PASS | filters(s) 필터 1/1 실행 ≥1, 잡힌 테스트 1, 실패 0 |
| SC-51 | PASS | filters(s) 필터 3/3 실행 ≥1, 잡힌 테스트 3, 실패 0 |
| SC-52 | PASS | filters(s) 필터 1/1 실행 ≥1, 잡힌 테스트 1, 실패 0 |
| SC-53 | PASS | filters(s) 필터 1/1 실행 ≥1, 잡힌 테스트 1, 실패 0 |
| SC-54 | PASS | filters(s) 필터 2/2 실행 ≥1, 잡힌 테스트 2, 실패 0 |
| SC-55 | PASS | filters(s) 필터 1/1 실행 ≥1, 잡힌 테스트 1, 실패 0 · f/judges:sc55_history_replay_cycle rc0 · f/judges:sc55_sold_rows_cycle_cycle rc0 |
| SC-56 | PASS | unity 이름 2 전부 xml 적중() |
| SC-57 | PASS | 기본/offline:sc57_copy_scope rc0 |
| SC-58 | PASS | 기본/offline:sc58_glob_census rc0 |
| SC-59 | PASS | 기본/offline:sc59_validate_data rc0 |
| SC-60 | PASS | 기본/offline:sc60_codegen_run1 rc0 · 기본/offline:sc60_codegen_run2 rc0 · 기본/offline:sc60_codegen_compare rc0 |
| SC-61 | PASS | unity 이름 3 전부 xml 적중() |
| SC-62 | PASS | unity 이름 1 전부 xml 적중() |
| SC-63 | PASS | unity 이름 1 전부 xml 적중() |
| SC-64 | PASS | unity 이름 1 전부 xml 적중() |
| SC-65 | **미검증(사람 — 최종 세션)** | 도구 없음(사람 관찰). ⊘ = `SESSION_OPENED` ≥ 1 · `live_connections` 1 |
| SC-66 | PASS | unity 이름 1 전부 xml 적중() |
| SC-67 | PASS | unity 이름 1 전부 xml 적중() |
| SC-68 | **미검증(사람 — 최종 세션)** | 도구 없음(사람 관찰). ⊘ = `SESSION_OPENED` ≥ 1 · `live_connections` 1 |
| SC-69 | PASS | unity 이름 1 전부 xml 적중() |
| SC-70 | PASS | unity 이름 1 전부 xml 적중() |
| SC-71 | **미검증(사람 — 최종 세션)** | 도구 없음(사람 관찰). ⊘ = `SESSION_OPENED` ≥ 1 · `live_connections` 1 |
| SC-72 | PASS | unity 이름 1 전부 xml 적중() |
| SC-73 | PASS | unity 이름 1 전부 xml 적중() |
| SC-74 | **미검증(사람 — 최종 세션)** | 도구 없음(사람 관찰). ⊘ = `SESSION_OPENED` ≥ 1 · `live_connections` 1 |
| SC-75 | PASS | unity 이름 1 전부 xml 적중() |
| SC-76 | PASS | unity 이름 1 전부 xml 적중() |
| SC-77 | **미검증(사람 — 최종 세션)** | 도구 없음(사람 관찰). ⊘ = `SESSION_OPENED` ≥ 1 · `live_connections` 1 |
| SC-78 | PASS | unity 이름 1 전부 xml 적중() |
| SC-79 | PASS | unity 이름 1 전부 xml 적중() |
| SC-80 | **미검증(사람 — 최종 세션)** | 도구 없음(사람 관찰). ⊘ = `SESSION_OPENED` ≥ 1 · `live_connections` 1 |
| SC-81 | PASS | unity 이름 1 전부 xml 적중() |
| SC-82 | PASS | unity 이름 1 전부 xml 적중() |
| SC-83 | **미검증(사람 — 최종 세션)** | 도구 없음(사람 관찰). ⊘ = `SESSION_OPENED` ≥ 1 · `live_connections` 1 |
| SC-84 | PASS | unity 이름 1 전부 xml 적중() |
| SC-85 | PASS | unity 이름 1 전부 xml 적중() |
| SC-86 | **미검증(사람 — 최종 세션)** | 도구 없음(사람 관찰). ⊘ = `SESSION_OPENED` ≥ 1 · `live_connections` 1 |
| SC-87 | PASS | unity 이름 1 전부 xml 적중() |
| SC-88 | PASS | unity 이름 1 전부 xml 적중() |
| SC-89 | **미검증(사람 — 최종 세션)** | 도구 없음(사람 관찰). ⊘ = `SESSION_OPENED` ≥ 1 · `live_connections` 1 |
| SC-90 | PASS | unity 이름 1 전부 xml 적중() |
| SC-91 | PASS | unity 이름 1 전부 xml 적중() |
| SC-92 | **미검증(사람 — 최종 세션)** | 도구 없음(사람 관찰). ⊘ = `SESSION_OPENED` ≥ 1 · `live_connections` 1 |
| SC-93 | PASS | unity 이름 1 전부 xml 적중() |
| SC-94 | PASS | unity 이름 1 전부 xml 적중() |
| SC-95 | **미검증(사람 — 최종 세션)** | 도구 없음(사람 관찰). ⊘ = `SESSION_OPENED` ≥ 1 · `live_connections` 1 |
| SC-96 | PASS | unity 이름 1 전부 xml 적중() |
| SC-97 | PASS | unity 이름 1 전부 xml 적중() |
| SC-98 | **미검증(사람 — 최종 세션)** | 도구 없음(사람 관찰). ⊘ = `SESSION_OPENED` ≥ 1 · `live_connections` 1 |
| SC-99 | PASS | unity 이름 1 전부 xml 적중() |
| SC-100 | PASS | unity 이름 1 전부 xml 적중() |
| SC-101 | **미검증(사람 — 최종 세션)** | 도구 없음(사람 관찰). ⊘ = `SESSION_OPENED` ≥ 1 · `live_connections` 1 |
| SC-102 | PASS | unity 이름 1 전부 xml 적중() |
| SC-103 | PASS | `grep -rnE "(BalanceCr|balance_cr|QuantityKg|quantity_kg)\s*[+\-]=" client/Assets/_Project/Scripts` → 0 매칭(rc 1) |
| SC-104 | PASS | unity 이름 1 전부 xml 적중() |
| SC-105 | PASS | unity 이름 1 전부 xml 적중() |
| SC-106 | PASS | f/bots:cases.cheat-sell-inject rc0 |
| SC-107 | PASS | f/bots:cases.cheat-sell-range rc0 |
| SC-108 | PASS | f/bots:cases.cheat-sell-fast rc0 |
| SC-109 | PASS | f/bots:cases.cheat-sell-overqty rc0 |
| SC-110 | PASS | f/bots:cases.cheat-sell-cooldown rc0 |
| SC-111 | PASS | f/bots:cases.cheat-sell-unknown rc0 |
| SC-112 | PASS | f/bots:cases.sell-dup rc0 · f/bots:cases.sell-dup-reconnect rc0 · f/bots:cases.sell-dup-cross-kind rc0 · f/bots:restart_sell_dup rc0 |
| SC-113 | PASS | f/bots:trace.trace-sell-a rc0 |
| SC-114 | PASS | f/bots:trace.trace-sell-b rc0 |
| SC-115 | PASS | f/bots:trace.trace-sell-b rc0 |
| SC-116 | PASS | f/bots:race.race-sell-same-tick rc0 |
| SC-117 | PASS (⚠ 얇다) | `f/bots/load/sell-load.json`: `min_ws_connections` **31** = 31(⊘ 충족) · 수락 **22** · `sold_kg_by_mineral` = **glacine 14,200 만**. rc0. **아래 §SC-117 참조 — 같은 항목의 첫 실행은 수락 127 · 광물 3 종이었고 그때는 ⊘ 미충족(`min_ws` 13)이었다** |
| SC-118 | PASS | f/judges:sc118_load rc0 |
| SC-119 | PASS | `sell-load.json` `results_by_reason` = ACCEPTED 117 · INSUFFICIENT_QUANTITY 2 · RESOURCE_DEPLETED 766 · `max_recording_lag` **0** · `recording_backlog_rejects` 0. ⊘ 검출기 생존 = SC-38·39 같은 빌드 PASS(`c/db_stop:sc39_accepted` rc0) |
| SC-120 | PASS | f/judges:sc120_market_events_load rc0 |
| SC-121 | PASS | f/bots:cycle.trade-cycle rc0 |
| SC-122 | PASS | f/judges:sc122_cycle rc0 · f/judges:sc122_cases rc0 · f/judges:sc122_load rc0 |
| SC-123 | PASS | f/judges:sc123_causation_cycle rc0 · f/judges:sc123_causation_cases rc0 · f/judges:sc123_causation_load rc0 |
| SC-124 | PASS | f/judges:sc124_gaps_cycle rc0 · f/judges:sc124_gaps_cases rc0 · f/judges:sc124_gaps_load rc0 |
| SC-125 | PASS | 기본/offline:sc125_self_causation rc0 |
| SC-126 | PASS | 기본/offline:sc126_constraint_census rc0 |
| SC-127 | PASS | 기본/offline:sc127_coverage_strict rc0 |
| SC-128 | PASS | 기본/offline:sc128_interface_matrix rc0 |
| SC-129 | PASS | f/bots:sc129_pairs rc0 |
| SC-130 | **미검증(CI 첫 실행 전)** | 로컬 `check_item_sources` rc 0(`기본/sources`). 계약이 요구한 증거는 **PR 의 `judgment gates (python)` job 로그**이고 그 실행이 아직 없다 |
| SC-131 | **미검증(사람)** | qa 몫 ⊘ 통과: xml mtime 2026-10-10 16:16 > `client/` 최종 변경 2026-10-09 23:02 · 테스트 509(EditMode 492 + PlayMode 17) ≥ Unity 이름 35 · 파일 `evidence/r1_20261010/unity/*.xml`. **PR 템플릿 §3.3 p1-03 줄이 미기입(`- [ ]`)** |
| SC-132 | PASS | 세 실행 모두 수락 ≥ 1(⊘ `0 == 0` 방지): cycle · race · load. `sell-load.json sub_verdicts.sc132 = PASS` · 순서 위반 0 |
| SC-133 | **미검증 — 건너뜀** | 사용자 결정(2026-10-10)으로 사전 세션을 건너뛰었다. 리더 소유 |


---

## 추가 (2026-10-11, 리포트 뒤) — CI 를 빨갛게 만들던 라벨 귀속 결함

**r1 판정은 바뀌지 않는다.** p1-03 출처 게이트는 수정 전후 모두 exit 0 이다. 바뀐 것은 **p1-01 게이트**다.

### 증상과 기제

```
출처 게이트 p1-01 → exit 1
  !! 위반 — 계약이 지명하지 않은 번호로 verdict 를 내는 도구:  trade_ledger.py -> SC-32
```

`check_item_sources.py` 의 `label_slice` 가 **문두 고정** `SLICE_TAG.match` 였다. 실측:

| 라벨 | 귀속(수정 전) |
|---|---|
| `p1-03 SC-32 (AC-6a) 질량 보존 I-84 — 판매 항 포함` | `p1-03` ✓ |
| `참고 — p1-03 SC-32 음성 대조: 판매 항 없는 p1-02 식(FAIL 이어야 한다)` | **`p1-01`** ✗ |

**`참고 —` 관측 표식이 슬라이스 표지를 0 번 위치에서 밀어낸다.** 그 라벨이 p1-01 것으로 귀속돼 p1-01 계약의 SC-32(다른 항목)를 위반했다. 그 라벨은 **이 라운드에서 `exit: negative` 로 표시한 바로 그 음성 대조**다 — 하네스 결함 ①을 고친 자리가 다른 게이트를 빨갛게 만들었다.

**왜 r1 중에 안 보였나**: `trade_ledger.py`(697 줄)가 리더 커밋 전까지 **미추적**이었고 CI 는 체크아웃된 레포를 보므로 그 파일을 보지 못했다. 옛 판 게이트(`b3bc8f2`)로 돌려도 exit 1 이므로 역할 분류 수정 탓이 아니다.

### 처방 — 표식 접두를 걷은 뒤 **문두 고정 그대로** 읽는다

`.search` 로 느슨하게 만들지 않았다. **그 라벨 자체가 본문에 `p1-02` 를 담고 있다**(*"판매 항 없는 p1-02 식"*). `.search` 는 먼저 나오는 표지를 집으므로 *"p1-02 식과 다른 p1-03 …"* 같은 라벨이 오면 **조용히 p1-02 로 귀속된다** — 느슨하게 만들면 반대 방향 구멍이 생긴다. 걷어내는 것은 **선언된 표식**(`OBSERVATION_MARKS = ("관측용","참고","참조")`)뿐이고, 선언에 없는 접두는 걷히지 않아 *표지 없는 라벨*로 세어진다.

| 슬라이스 | 판정 py 라벨 전 → 후 | exit 전 → 후 |
|---|---|---|
| p1-03 | 22 → **23** | 0 → 0 |
| p1-02 | 16 → 16 | 0 → 0 |
| p1-01 | 27 → **26** | **1 → 0** |

라벨 하나가 p1-01 에서 p1-03 으로 옮겨졌다. **p1-03 분모가 1 늘었다**(줄지 않았다).

### 선언되지 않은 통로 — 세어서 찍는다

`label_slice` 는 **표지가 없으면 p1-01** 로 돌려준다(docstring 이 적은 의도된 하위 호환). 닫지 않았지만 **그 수를 매 실행 찍게 했다**:

```
표지 없는 py 라벨 26 건(p1-01 로 귀속) · 도구 11:
  append_only_probe.py bandwidth.py block8_invariants.py check_sequence_gaps.py check_sessions.py
  fixture_world_scan.py interface_matrix.py perf_compare.py server_boot.py ship_events.py two_client_view.py
```

**0 이 아니다 — 통로가 살아 있고 p1-01 시절 도구 11 개가 쓰고 있다.** 수정 전후 26 으로 같다(이 수정이 바꾼 값이 아니다). 새 도구가 표지를 빠뜨리면 이 수가 늘고 그 자리에서 보인다. `aggregate()` 가 모르는 `미검증` 을 PASS 로 떨어뜨린 것과 같은 가족이다.

### 검증

- CI `judgment gates (python)` job 을 **명령 그대로** 돌렸다: 출처 게이트 3 개 · selftest 24 개 · `validate_data_files` 2 개 → **전부 exit 0**.
- `check_item_sources.py --selftest` **52 케이스**(슬라이스 대조 ⑦~⑦'''''' 6 건 추가).
- 돌연변이 **5 종 전부 사망**: 문두 고정 되돌리기 · **`.search` 로 느슨하게**(리더가 금지한 처방) · 아무 접두나 걷기 · 표지 없는 라벨 세지 않기 · `label_unmarked` 를 항상 False.

**측정 방법에서 내가 한 실수 하나**: 수정 전 수를 재려고 옛 판 사본을 `tests/e2e/` 에 뒀더니 **그 사본이 도구로 스캔돼** 양쪽 수가 오염됐다(표지 없는 라벨 26 → 35, 알 수 없는 표지 0 → 1, exit 0 → 4). 제자리에서 되돌려 재고 즉시 복원하는 방식으로 다시 쟀다. **도구 디렉터리를 스캔하는 게이트는 그 디렉터리에 둔 사본이 입력이 된다.**
