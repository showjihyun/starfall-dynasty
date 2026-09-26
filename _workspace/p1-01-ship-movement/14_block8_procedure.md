# 블록 8 절차서 — SC-10 · SC-28 · SC-30 · SC-32 · SC-74

> **이 다섯은 p1-01 에서 한 번도 판정된 적이 없다.** 출처 게이트가 `미지명 5건` 으로 빨간불을
> 켜 온 그 다섯이고, 만기는 *"블록 8 절차서가 명령을 확정하는 시점"* 이다(계약 §570,
> architect R18). **이 문서가 그 시점이다.**
>
> **다섯 항목 전부 Unity 가 필요 없고 사람 손도 필요 없다.** 서버·docker·봇만 있으면 된다.
> (블록 8 의 다른 항목 — SC-69·70·71·72·73·75 — 은 31번째 연결로 Unity PlayMode 를 요구하지만
> **이 다섯은 아니다.** 그 항목들은 이 절차서의 범위가 아니다.)

- 판정 도구: **`tests/e2e/block8_invariants.py`** (신규, qa 소유). selftest **20 케이스 exit 0**
- 증거 디렉터리 권장: `_workspace/p1-01-ship-movement/evidence/R18-block8/`

---

## 0. 먼저 — 하지 말 것과 알아야 할 것

| 금지 | 왜 |
|---|---|
| **`docker compose down -v`** | `domain_events` 는 추가 전용 기록이고 슬라이스를 넘어 증거다. **SC-10 은 그 테이블을 읽는다** — 지우면 이 절차서가 못 돈다 |
| **`run_block.py`** | `EVIDENCE_ROOT` 가 p0-02 로 박혀 있다(qa 가 찾은 여덟 번째 사례). 아래 명령을 직접 쓴다 |
| **서버 하드 킬** | stdin 에 `shutdown` 한 줄. §E-4 |
| **D 단계에서 `docker compose stop postgres`** | 쓰지 마라 — **`pause`/`unpause`** 를 쓴다(§D). `stop` 은 컨테이너를 내리고 재기동 시 초기화 경로를 타 검증 창이 흐려진다 |

**두 실행을 동시에 돌리지 않는다.** `bots run` 은 언제나 `bot-000 … bot-(N-1)` 로 접속한다 —
라벨 오프셋 인자가 없다. 두 실행이 겹치면 **같은 actor 로 두 세션**이 되어 나중 것이 먼저 것을
`SUPERSEDED`/close 4001 로 밀어낸다(SC-88 의 설계 동작이지 버그가 아니다).

**서버·인프라**

```
docker compose up -d                       # PostgreSQL 15432 / Redis 16379
cd server && cargo run -p starfall-game-server     # 종료는 stdin 'shutdown'
curl -s http://127.0.0.1:8080/healthz       # {"status":"ok",...}
export STARFALL_DEV_AUTH_SECRET=<.env 의 값>
```

---

## A-0. SC-10 — 스폰 위치와 **만료 후 재스폰의 결정성**

계약이 재는 것 둘: (전반) `SHIP_SPAWNED` 위치가 `data/` 의 스폰 지점과 **정확히 일치**한다.
(후반) **잔류 만료 후**(> `linger_seconds`) 같은 `actor_id` 로 재접속하면 새 `SHIP_SPAWNED` 의
위치가 **첫 번째와 같다.**

### ⚠ 전반부는 **점유가 낮은 실행**에서 재야 한다 — 31봇 실행에서 재면 안 된다

`cradle.json` 의 `assignment_rule` 은 배정 지점이 점유돼 있으면 **반경 방향으로
`radial_offset_step_m`(150 m) 씩 민다.** 스폰 지점은 **12개**다. 31봇 단계 A 에서 재면
**밀린 스폰이 정상적으로 발생**하고, 그 위치는 `points_m` 와 일치하지 않는다 —
**정상 동작이 FAIL 로 인쇄된다.**

그래서 SC-10 은 **봇 1대짜리 짧은 실행 두 번**으로 잰다. 단계 A 와 섞지 않는다.

```
cd tools/bots
# 1차 스폰
cargo run --release -- run --scenario e --bots 1 --duration 5 \
  --out ../../_workspace/p1-01-ship-movement/evidence/R18-block8/spawn-1

# ⏳ 35초 이상 기다린다 — linger_seconds 30 이 지나야 잔류가 만료되고 **새 스폰**이 된다.
#    30초 안에 다시 붙으면 그것은 재개이고 SHIP_SPAWNED 가 새로 나지 않는다 (계약 게이트 G-j).
sleep 40

# 2차 스폰 — 같은 라벨(bot-000) = 같은 actor_id
cargo run --release -- run --scenario e --bots 1 --duration 5 \
  --out ../../_workspace/p1-01-ship-movement/evidence/R18-block8/spawn-2
```

판정:

```
python tests/e2e/block8_invariants.py spawn \
  --since-tick <서버 기동 tick> \
  --evidence _workspace/p1-01-ship-movement/evidence/R18-block8/sc10.json
```

**⚠ `--since-tick` 을 생략하지 마라 — R18 실행에서 실측으로 걸렸다.**
`domain_events` 는 추가 전용이라 범위를 안 주면 **동시 접속 수가 다른 실행이 전부 섞인다.**
무범위로 돌리면 스폰 106건 · 재스폰 짝 58건 · **불일치 4건**으로 **FAIL** 이 나오는데 그 4건은
결함이 아니라 **점유 탐침**이다. SQL 로 특정했다: actor `…0090` 의 2차 스폰 tick 398728 에
1차 지점은 actor `…0004` 의 함선(396971 스폰 → 399041 디스폰)이 **점유 중이었다.** 넷 다 같은
모양이고 **두 번째 위치가 전부 스폰 지점 위**다 — 난수면 지점 위에 떨어질 이유가 없다.
생략하면 도구 출력이 스스로 *"범위 없음 — 이 결과로 판정하지 말라"* 를 적는다.
R18 의 값은 `--since-tick 1576721` 이었다(서버 기동 tick).

**도구가 자명 통과를 막는 자리**: `linger_ticks`(30초 × 20 Hz = **600 tick**)를 넘는 같은
`actor_id` 의 스폰 짝이 **하나도 없으면 `미검증(표본 없음)` 이고 exit 4** 다. `sleep 40` 을
건너뛰면 여기서 걸린다 — 통과하지 않는다.

---

## A. SC-28 · SC-30 — 31 연결 60초 (단계 A)

```
cd tools/bots
cargo run --release -- run --scenario a --bots 31 --duration 60 --ramp 5 \
  --out ../../_workspace/p1-01-ship-movement/evidence/R18-block8/stage-a
```

판정:

```
python tests/e2e/block8_invariants.py snapshots \
  --bot-out-dir _workspace/p1-01-ship-movement/evidence/R18-block8/stage-a \
  --evidence   _workspace/p1-01-ship-movement/evidence/R18-block8/sc28-30-32.json
```

이 한 명령이 **SC-28 · SC-30 · SC-32 세 verdict 를 각각** 낸다.
**이 실행에서 SC-32 는 `미검증(표본 없음)` 이 나오는 것이 정상이다** — §B 를 보라.

**도구가 무엇을 하는가**

- **SC-28**: 관측자별로 스냅샷 tick 을 정렬해 **연속 차이를 재계산**한다. 기대값은
  `summary.json` 의 `snapshot_interval_ticks`(서버가 알려 준 값)에서 읽는다 — **도구는 숫자를
  지어내지 않는다.** 각 세션의 **첫 스냅샷 1건을 제외**하고 제외 수를 적는다(계약이 요구).
- **SC-30**: 관측자마다 `controlled_ship_id` 가 **자기가 받은 모든 스냅샷의 `ships` 안에** 있는가.
- **봇의 자체 계수를 판정에 쓰지 않는다.** `summary.json` 은 이미 `interval_violations` ·
  `controlled_ship_missing` 을 들고 있지만 **CSV 에서 다시 계산**하고 봇의 수는 대조로만 싣는다.
  **둘이 어긋나면 FAIL** 이다 — 어느 쪽이 맞든 그 실행으로는 닫을 수 없다.
- **출처 게이트 P1·P2**: `summary.json` 이 동반돼야 하고(Unity 는 이 파일을 쓰지 않는다),
  CSV 의 `observer_actor_id` 집합이 `summary` 와 같아야 한다.

---

## B. SC-32 — 잔류 presence 와 **디스폰 후 소멸**

### ⚠ `bots run` 의 **어떤 단계도** 이 항목의 표본을 만들지 못한다

확인한 사실이다:

| 단계 | 왜 안 되는가 |
|---|---|
| **A**(정상 상태) | 아무도 중간에 나가지 않는다. 실행이 끝나면 **지켜볼 관측자가 남지 않는다** |
| **B**(세션 회전) | 다음 주기가 **같은 라벨**로 돌아온다 → 잔류 창(30초) 안의 **재개**다. 디스폰이 일어나지 않는다 |
| **C · D** | A 와 같은 접속 형태다 |
| 두 실행을 겹치기 | 둘 다 `bot-000` 부터 쓴다 → 같은 actor 충돌(§0) |

**`bots resume` 만이 "한쪽은 나가 있고 다른 쪽은 계속 보고 있다" 를 만든다.**

```
cd tools/bots
cargo run --release -- resume --label bot-007 --observer bot-008 \
  --fly 2.0 --settle 1.5 --gap 35 --listen 45 \
  --out ../../_workspace/p1-01-ship-movement/evidence/R18-block8/resume-despawn.json
```

- **`--gap 35`** — `linger_seconds` 30 을 **넘겨야 한다.** 30 이하면 재개이고 디스폰이 아니다(G-j).
  도구가 `--gap ≤ 30` 을 `미검증(표본 없음)` 으로 되돌린다.
- **`--listen 45`** — 관측자(bot-008)가 **함선이 사라진 뒤에도 계속 받고 있어야** 한다.
  짧으면 "사라졌다" 와 "관측자가 먼저 눈을 감았다" 를 구분할 수 없고, 도구가 그것도
  `미검증(표본 없음)` 으로 되돌린다.

판정:

```
python tests/e2e/block8_invariants.py resume-despawn \
  --resume-json _workspace/p1-01-ship-movement/evidence/R18-block8/resume-despawn.json \
  --evidence    _workspace/p1-01-ship-movement/evidence/R18-block8/sc32.json
```

**판정량은 "재등장 건수" 다.** *"마지막 등장 이후 안 보인다"* 는 정의상 참이라 그대로 재면
**항진명제**다. 재는 것은 그 반대 — **없어졌다가 다시 나타났는가.** 그리고 그 0 이 뜻을
가지려면 **이 실행에서 실제로 디스폰이 일어났어야** 하므로, 도구가 위의 세 단언
(`gap > linger` · `LINGERING` 관측 · `observer_last_tick > ship_last_tick`)을 먼저 건다.

---

## D. SC-74 — DB 중단 구간에도 스냅샷이 흐르는가

**순서가 판정이다.** stats 를 받는 시점이 틀리면 창 밖을 재게 된다.

```
# 1) 봇을 먼저 띄운다 (백그라운드). 90초면 30초 중단 창을 넉넉히 감싼다.
cd tools/bots
cargo run --release -- run --scenario d --bots 4 --duration 90 \
  --out ../../_workspace/p1-01-ship-movement/evidence/R18-block8/stage-d &

# 2) 봇이 다 붙을 때까지 기다린다
sleep 15

# 3) ⬅ 멈추기 **직전** 의 stats
curl -s http://127.0.0.1:8080/debug/stats \
  > ../../_workspace/p1-01-ship-movement/evidence/R18-block8/stats-before.json

# 4) DB 를 30초 멈춘다 — pause 다. stop 도 down 도 아니다
docker compose pause postgres
sleep 30
docker compose unpause postgres
# ⚠ 스크립트로 돌린다면 `trap 'docker compose unpause postgres' EXIT INT TERM` 을 먼저 건다.
#    중간에 죽어 postgres 가 멈춘 채 남으면 다음 사람이 그것을 결함으로 읽는다.

# 5) ⬅ 재개 **직후** 의 stats
curl -s http://127.0.0.1:8080/debug/stats \
  > ../../_workspace/p1-01-ship-movement/evidence/R18-block8/stats-after.json

wait    # 봇 실행이 끝나기를 기다린다
```

판정:

```
python tests/e2e/block8_invariants.py db-outage \
  --bot-out-dir  _workspace/p1-01-ship-movement/evidence/R18-block8/stage-d \
  --stats-before _workspace/p1-01-ship-movement/evidence/R18-block8/stats-before.json \
  --stats-after  _workspace/p1-01-ship-movement/evidence/R18-block8/stats-after.json \
  --evidence     _workspace/p1-01-ship-movement/evidence/R18-block8/sc74.json
```

**도구가 먼저 단언하는 것 — DB 가 실제로 멈췄는가.**
두 stats 의 `persist_backlog` 가 늘었거나 `domain_events_persist_failed_total` 이 늘어야 한다.
**둘 다 그대로면 `미검증(증거 요건)` 이고 exit 4** 다. postgres 가 멀쩡한 채로 잰
*"스냅샷이 흘렀다"* 는 아무것도 뜻하지 않는다 — **이 슬라이스가 네 번 데인 형태가 정확히 이것이다.**

그 단언이 선 다음에야 두 판정을 본다: 중단 창 tick 범위 안에서 **모든 봇이** 스냅샷을 1건 이상
받았는가, 그리고 끊긴 연결이 0 인가(`server_initiated_closes` · 봇 `errors`).

---

## E. 판정 명령 한눈에 · 종료 코드

| 항목 | 명령 | 입력 |
|---|---|---|
| **SC-10** | `block8_invariants.py spawn` | PostgreSQL `domain_events` + `data/world/systems/cradle.json` |
| **SC-28 · SC-30** | `block8_invariants.py snapshots --bot-out-dir <stage-a>` | 봇 `snapshots.csv` + `summary.json` |
| **SC-32** | `block8_invariants.py resume-despawn --resume-json <F>` | `bots resume` 의 관측자 행 |
| **SC-74** | `block8_invariants.py db-outage --bot-out-dir <stage-d> --stats-before … --stats-after …` | 봇 산출물(**`sessions.json` 포함**) + 중단 창 전후 stats |
| 도구 자체 | `block8_invariants.py selftest` | 합성 입력 (양성·음성 대조 20 케이스) |

종료 코드: `0` 전부 PASS / `1` FAIL / `2` 사용법·환경 / **`4` 미검증**(표본 없음·증거 요건).
**`4` 는 PASS 가 아니다.** FAIL 과 4 를 같은 코드로 내보내면 계약 §0.3 이 나눈 두 칸이 셸에서
다시 합쳐진다.

### E-4. 끝나면

```
# 서버: stdin 에 shutdown 한 줄. 하드 킬 금지
# docker: 그대로 둔다. down -v 절대 금지
```

---

## F. 리포트 필수 기재 — 자명 통과 방지

| 적을 것 | 어디서 | 왜 |
|---|---|---|
| `SC-28.checked_pairs` · `excluded_first_snapshots` | `snapshots` | **0 이면 `미검증(표본 없음)` 이지 PASS 가 아니다.** 제외 수는 계약이 명시적으로 요구한다(세션당 1건) |
| `SC-28.expected_interval_ticks` **와 그 출처** | 〃 | 기대값이 도구에 박혀 있으면 계약이 바뀌었을 때 옛 수로 조용히 초록이 난다 |
| `SC-28.bot_self_count_cross_check` | 〃 | 봇의 자체 계수와 재계산이 **일치하는가.** 어긋나면 FAIL 이고 그 차이가 곧 발견이다 |
| `SC-30.ticks_checked` | 〃 | 0 이면 잰 것이 없다 |
| `SC-32.despawn_events_observed` · `reappearances_after_absence` | `resume-despawn` | **디스폰이 0 건이면 "재등장 0" 은 항진명제다** |
| `SC-32.observer_ticks_after_ship_vanished` | 〃 | 관측자가 **함선보다 오래 봤는가.** 0 이하면 눈을 먼저 감은 것이다 |
| `SC-10.respawn_pairs_beyond_linger` · `gap_seconds` | `spawn` | 0 이면 후반부가 판정되지 않았다. `gap_seconds > 30` 인지 눈으로 확인한다 |
| `SC-10.spawns_not_on_a_point` **와 그 실행의 동시 접속 수** | 〃 | 0 이 아니면 FAIL 로 닫기 전에 **점유로 밀린 것인지** 본다(§A-0) |
| `SC-74.outage_actually_happened.asserted` | `db-outage` | **`false` 면 DB 가 안 멈춘 것이고 PASS 가 아니다** |
| `SC-74.snapshots_in_window_per_observer` | 〃 | 봇별 수. 하나라도 0 이면 FAIL |
| `SC-74.server_initiated_closes` **와 그 출처** | 〃 | **`sessions.json` 의 세션별 `close_initiator` 에서 읽는다.** `summary.json` 에는 그 수가 **없다**(봇이 stdout 에만 찍는다) — R18 에서 도구가 늘 `None` 을 읽어 **"끊긴 연결 0" 절반이 평가되지 않은 채 PASS 가 인쇄됐다.** 지금은 못 읽으면 `미검증(증거 요건)` 이다 |
| `SC-10.judgment_scope.since_tick` | `spawn` | **범위 없이 낸 판정은 무효다**(§A-0 의 경고) |

---

## G. 이 절차서가 닫지 **못하는** 것

- **블록 8 의 나머지 항목**(SC-69 · 70 · 71 · 72 · 73 · 75, 그리고 M-1~M-6) — 31번째 연결로
  **Unity PlayMode** 가 필요하고 `bandwidth.py` · `perf_compare.py` · `stats_delta.py` 가 판정한다.
  **이 문서의 범위가 아니다.**
- **SC-71 이 블록 8 을 막는다**(architect R16, 계약 SC-71): 봇 실측 대역폭과 ADR-0011 §2 산출의
  차이가 20 % 를 넘으면 **ADR 의 표를 갱신하기 전까지 비-통과**다. ADR 은 architect 소유이고
  qa 가 직접 고치지 않는다. **SC-10·28·30·32·74 는 이 의존과 무관하게 먼저 돌 수 있다.**
- **SC-10 의 전반부를 31봇 실행에서** — §A-0 의 이유로 구조적으로 불가능하다.

### G-1. `snapshot_hz` 가 바뀌면 SC-28·30·32 를 다시 돌린다 (architect 가 넘긴 순서 조건)

`data/movement/sync-tuning.json:9-10` 에 `egress_budget_kib_s_per_session: 192` 와
`fallback_snapshot_hz_if_over_budget: 5` 가 있다. 세션당 실측이 **192 KiB/s 를 넘으면** designer 가
`snapshot_hz` 를 **10 → 5** 로 내리고 그러면 **`snapshot_interval_ticks` 가 2 → 4 가 된다.**
**이 셋의 판정량이 그 수에 직접 걸린다**(SC-28 은 그 간격 자체를, SC-30·32 는 같은 CSV 를 읽는다).

> **단계 A 산출물로 낸 SC-28·30·32 는 그 실행 시점의 `snapshot_interval_ticks` 에 대한 판정이다.**
> `snapshot_hz` 가 바뀌면 **무효이고 다시 돌린다.**

**⚠ 다만 그 폴백은 런타임 자동 전환이 아니다 — qa 가 실행으로 확인했다.**
`fallback_snapshot_hz_if_over_budget` 과 `egress_budget_kib_s_per_session` 은
`server/crates/contracts/src/data.rs:617,621` 의 **구조체 필드 선언이 전부이고 서버 코드가 읽는
곳이 없다**(`grep -rn fallback_snapshot_hz server/ tools/ --include=*.rs` → 선언 1건뿐).
그러므로 **폴백은 데이터 파일을 고치고 서버를 재기동해야 발동하는 designer 결정**이고,
**단계 A 실행 중이나 판정 사이에 값이 저절로 바뀔 수는 없다.** 위험은 하나뿐이다 —
**나중에 누가 설정을 고치면 이 판정이 낡는다.**

**그래서 비용이 드는 재실행을 피하는 방법은 둘이다:**

1. **판정에 쓴 간격을 증거에 박는다.** 도구가 이미 그렇게 한다 — `SC-28.expected_interval_ticks`
   와 그 출처(`summary.json`, 즉 서버가 `SESSION_READY` 로 알려 준 값)를 출력에 싣는다.
   **도구에 기대값을 박아 두지 않았으므로 조용한 초록은 나지 않는다.**
2. **단계 A 직후에 세션당 대역폭을 먼저 본다.** `192 KiB/s` 를 넘었으면 designer 가 폴백을
   당길 가능성이 있으므로, **그 산출물로 SC-28·30·32 를 닫기 전에 리더·designer 에게 확인한다.**
   관측은 봇 산출물만으로 계산된다(`summary.snapshots_per_bot[].snapshot_bytes_received`
   ÷ `duration_secs` ÷ 1024). **이것은 SC-72 의 verdict 가 아니다** — SC-72 는 `bandwidth.py`
   가 판정하고 이 문서의 범위가 아니다. 여기서는 **재실행 위험을 재는 관측**으로만 쓴다.

---

## H. architect 에게 — 게이트가 지금 **더 빨개졌다** (의도된 중간 상태)

출처 게이트(`check_item_sources.py`)가 **exit 3 → exit 1** 로 바뀐다.

| 전 | 후 |
|---|---|
| `미지명 5건`(SC-10·28·30·32·74) · **출처 위반 0** · exit **3** | **출처 위반 5건** — `block8_invariants.py → SC-10·28·30·32·74` · exit **1** |

**도구가 생겼는데 계약이 그 도구를 지명하지 않았기 때문이고, 이것이 옳은 동작이다** —
게이트 자신이 *"번호의 존재만 보는 역방향 게이트는 이것을 통과시킨다 — 그래서 이 검사가 있다"*
고 적고 있다. 숨기지 않고 적는다.

**닫는 방법은 계약 두 곳에 줄을 넣는 것이고 그것은 architect 소유다:**

1. **§3.2 도구 표**에 한 행:
   `| **block8_invariants.py** (신규 — 하위 명령 snapshots·spawn·resume-despawn·db-outage·selftest) | SC-10 · SC-28 · SC-30 · SC-32 · SC-74 | 블록 8. 입력이 셋(봇 산출물 · domain_events · 중단 창 stats)이라 하위 명령으로 갈랐다 |`
2. **§1 의 다섯 항목 행**의 검증 방법 칸에 `block8_invariants.py` 와 하위 명령 이름.

그 두 줄이 들어가면 게이트는 **`90 / 90` · 위반 0 · exit 0** 이 된다 — 이 슬라이스에서
출처 게이트가 처음으로 초록이 되는 지점이다.

**qa 는 계약을 고치지 않는다.**
