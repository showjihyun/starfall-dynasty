# 블록 1 절차서 — SC-06 · SC-07 · SC-57

> **셋 다 Unity 도 사람 손도 필요 없다.** 그리고 **게임 서버를 띄우지 않는다** —
> SC-06 은 서버가 *데이터 검증에서 죽는* 경로만 타고(포트도 DB 도 건드리지 않는다),
> SC-07 은 서버 밖의 오프라인 검증기이며, SC-57 은 **이미 쌓인 `domain_events` 를 읽는 SQL** 이다.
>
> - 판정 도구: `tests/e2e/server_boot.py`(SC-06 — **강화. 실행은 §A-4 ② 때문에 재실행 대기**) · `tests/e2e/validate_data_files.py`(SC-07, **강화**) · **순수 SQL**(SC-57)
> - 증거 디렉터리: `_workspace/p1-01-ship-movement/evidence/R19-block1/`
> - 실행일 **2026-09-25**, 서버 바이너리 `server/target/debug/starfall-game-server.exe` **2026-09-23T22:01:51** 빌드
>   (다른 에이전트가 `sim/src/simulation.rs` 에 임시 편집 중이라 `cargo` 를 쓰지 않았다 — **재빌드하지 않은 바이너리로 판정했다**)

---

## 0. 먼저 — 하지 말 것

| 금지 | 왜 |
|---|---|
| **`docker compose down -v` · `stop` · `pause` · `prune`** | `domain_events` 는 추가 전용 증거 장부다. **SC-57 이 그 테이블을 읽는다.** 이 절차서는 **SELECT 만** 한다 |
| **`data/` 원본 수정** | designer 소유. SC-06 은 **임시 사본**(스크래치패드)에 `STARFALL_DATA_DIR` 를 걸어 주입한다. 끝난 뒤 `git status --porcelain data/` 가 **비어 있어야 한다** |
| **게임 서버 정상 기동** | 리더가 금지했다. 음성 대조가 그래서 §A-2 의 형태를 쓴다 — **포트를 잡지 않는다** |
| **`cargo` 사용** | `simulation.rs` 에 다른 에이전트의 임시 편집이 걸려 있다. **기존 debug 바이너리를 쓰고 빌드 시각을 증거에 박는다** |
| **계약 수정** | architect 소유. 계약이 침묵·모순하는 자리는 **§F 에 적고 넘긴다** |

---

## A. SC-06 — 기동 거부 8종

계약 §1 SC-06 이 열거하는 8건을 `STARFALL_DATA_DIR` 로 **한 파일만 어긴 임시 사본**에 주입하고,
**종료 코드 1** 과 `기동 거부: 데이터 검증 실패 file=… rule=<schema|derived>` 로그를 확인한다.

### A-1. 판정이 실제로 무엇을 재는가 — **R19 에서 고친 것**

**고치기 전의 `cmd_reject` 는 `exit==1 and "기동 거부" 라인 존재` 만 보고 PASS 를 인쇄했다.**
`has_file` · `has_field` · `rule_matches` 를 **계산해 놓고 verdict 에 쓰지 않았다**
(`tests/e2e/server_boot.py` 구버전 `cmd_reject`). 그 상태에서는 **여덟 번 전부 다른 이유로 죽어도
8/8 PASS** 다 — 이 슬라이스가 네 번 데인 모양 그대로다.

지금 한 건이 PASS 가 되려면 **전부** 서야 한다:

| 단언 | 왜 |
|---|---|
| `exit_code == 1` | 계약이 지정한 종료 코드 |
| `기동 거부: 데이터 검증 실패` 라인 존재 | 데이터 검증 단계에서 죽었는가(마이그레이션·포트 실패와 구별) |
| `rule=<기대값>` 일치 | ①② 는 `schema`, ③~⑧ 은 `derived` — 계약이 나눠 놓은 층 |
| `field=<기대 필드>` 일치 (derived) | **다른 필드로 거부된 것을 통과시키지 않는다** |
| `file=`·`field=`·`expected=`·`actual=` 토큰 (derived) | 계약이 요구하는 로그 형식 |
| **`must_contain` — 사례 고유 문자열** | 심은 값(`actual=13`, `20000.1`, `999999`, …)과 대상 파일명. **"그 이유로 거부됐는가" 를 재는 자리다** |
| **`schema_field_identifiable`** (schema) | 로그가 찍은 **검증자 이름이 필드와 1:1 인가** — §A-1b |

### A-1b. `rule=schema` 층의 기계 검사 — **사람 눈으로 여덟 줄을 읽으면 틀린다**

architect R24 는 `rule=schema` 층을 *"serde 문장이 필드·기대값을 담는다"* 로 통과시켰다가
**R25 에서 철회했다.** 오판의 구조가 중요하다:

| | 로그 | 필드 특정 | 왜 |
|---|---|---|---|
| **②** | `error=points_m 은 1 ..= 64 개여야 한다 (받음: 0)` | **된다** | `de_points_m` 은 **`points_m` 하나만** 쓰고 메시지가 필드명을 적었다 |
| **①** | `error=de_boundary_radius_m 는 (0 .. 20000 …` | **안 된다** | **세 필드가 공유**한다 |

**로그 여덟 줄을 사람이 읽어 판정하면 우연히 1:1 인 사례(②)가 전체를 대표한다** —
R24 가 정확히 그렇게 틀렸고, 근거로 쓴 것은 **구현 doc 주석**(*"필드·기대값·실제값이 문장에
들어 있다"*)이었다. **그 주석이 거짓이다. doc 주석은 주장이지 증거가 아니다.**

그래서 눈 대신 **표를 건다**(`server_boot.py` 의 `validator_field_map`):

> `deserialize_with` 값별 필드 수가 **1 을 넘는 검증자가 그 경로의 로그를 낸다면 필드가
> 특정되지 않는다.** **전부 1 로 떨어지는 것이 구제의 종료 조건이다.**

실측 (증거 `evidence/R19-block1/sc06-validator-multiplicity.txt`) — **검증자 29개 중 10개가 공유**:

```
7 de_accel_mps2            main_thrust_mps2 · reverse_thrust_mps2 · lateral_thrust_mps2 ·
                           brake_decel_mps2 · assist_linear_decel_mps2 ·
                           assist_lateral_decel_mps2 · boundary_pull_mps2
3 de_boundary_radius_m     soft_boundary_radius_m(:446) · hard_boundary_radius_m(:449) · visual_radius_m(:566)
3 de_positive_upto_100000  3 de_accel_deg_s2
2 de_spawn_radius_m · de_rate_deg_s · de_presence_seconds_i · de_orientation_deg_0_180 · de_hz_1_60 · de_deadzone_sin
1 de_points_m              ← ② 가 만족하는 이유
```

**`at line/column` 은 대신이 못 된다**: 위치는 필드 지칭이 아니고, `soft`/`hard` 가
`cradle.json:16,17` 로 **한 줄 차이**라 한 줄 옆을 고치기 쉽다. 그리고
**`de_boundary_radius_m` 은 필드처럼 보이는데 필드가 아니라서 엉뚱한 필드를 고치도록 유도한다**
— `de_accel_mps2`(7 필드)는 더 나쁘다. **없는 것보다 나쁜 정보다.**

### A-1c. ⚠ 판정량을 **원인에서 결과로** 옮겼다 — 안 고쳤으면 구제된 로그를 FAIL 로 돌렸다

§A-1b 의 첫 구현은 **"공유 검증자를 쓰는가"** 를 판정량으로 삼았다. 그런데 **공유는 필드가
특정되지 않는 *원인*이었지 그 자체가 위반이 아니다.** 계약이 요구하는 것은
*"로그에 파일·필드·기대값이 나온다"* 이지 *"공유 검증자를 안 쓴다"* 가 아니다.

**server 가 구제 ⓐ 를 고르면서 그 차이가 드러났다**(리더 지적): `serde_path_to_error` 는
**공유를 그대로 두고 JSON 경로를 앞에 붙인다.** 필드는 특정되는데 `de_boundary_radius_m`
토큰은 여전히 로그에 있다 — **원인을 재던 검사는 고쳐진 로그를 잘못된 이유로 FAIL 로 돌린다.**
`shared_count == 0` 을 종료 조건으로 둔 것도 **ⓑ 갈래만 구현한 것**이었다.

**두 층으로 갈랐다:**

| 층 | 무엇 |
|---|---|
| **1. 판정** | 로그가 **사례가 심은 필드**를 지칭하는가 (`schema_names_injected_field`) |
| **2. 설명** | 지칭하지 못하면 **왜 못 하는지** — 다중도를 basis 문자열에 싣는다 |

**종료 조건도 바뀐다**: `shared_count == 0` 이 아니라 **schema 사례 전부가
`schema_names_injected_field = true`** 가 되는 것. 다중도 표는 **진단용**으로 남는다.

**이 판정에 숨을 자명 통과 둘을 함께 막았다:**

- **(가) 검증자 이름이 필드명을 품은 경우.** `de_points_m` ⊃ `points_m` 이므로 *"필드명이 로그에
  있다"* 를 그냥 찾으면 **사례 ②는 검증자 이름만 보고 통과한다 — 아무것도 재지 않고.**
  그래서 **`de_*` 토큰을 먼저 지우고** 필드명을 찾는다.
- **(나) 엉뚱한 필드.** *"어떤 필드명이든 있으면 통과"* 로 두면 `soft` 를 위반시켰는데 `hard` 가
  찍혀도 통과한다 — `cradle.json:16,17` **한 줄 차이**라 실제로 일어나고, architect 가
  *"없는 것보다 나쁜 정보"* 라고 한 그 상황이다. **같은 검증자를 쓰는 형제 필드가 대신 찍혔는지**를
  따로 단언한다(형제 목록은 다중도 표에서 자동으로 나온다 — 하드코딩하지 않는다).
  server 도 같은 형태로 상시 테스트를 남겼다: **양성(`soft` 가 나온다)과 음성(`hard` 가 안 나온다)을
  둘 다 단언한다**(`bins/game-server/src/data.rs:496-517`).

**selftest 가 구제 전후를 둘 다 건다** — 실제 소스가 고쳐지면 기대값이 흔들리지 않게
**합성 다중도 표**로 건다:

```
음성: 검증자 이름만 찍는다 — 필드 지칭 없음 (구제 전, 사례 ①)
양성: JSON 경로가 앞에 붙어 필드가 특정된다 (구제 ⓐ 후) — 공유는 그대로다
음성: 형제 필드가 대신 찍혔다 (soft ↔ hard)
음성: 검증자 이름이 필드명을 품었을 뿐이다 (de_points_m ⊃ points_m)
양성: 메시지가 필드명을 직접 적는다 (사례 ② 의 구제 전 로그)
```

**두 번째 케이스가 이 수정이 무언가를 고쳤다는 증거다** — 고치기 전 도구는 그 로그를 FAIL 로 돌렸다.

### A-2. 음성 대조 — **멀쩡한 `data/` 로는 데이터 검증을 통과한다**

거부 8건만으로는 *"서버가 늘 실패한다"* 와 구별되지 않는다. 그런데 리더가 서버 기동을 금지했다.
**포트를 잡지 않고 데이터 검증 통과를 보이는 방법**을 쓴다:

`main.rs:114-127` 의 순서가 **data → DB** 다(주석: *"마이그레이션·DB 접속보다 먼저다"*).
그래서 **멀쩡한 사본 + 죽은 `DATABASE_URL`(포트 15999)** 로 띄우면 서버는
`게임 데이터 로딩 완료 ship_classes=1 spawn_points=12 snapshot_interval_ticks=2` 를 찍고
**그 다음 단계인 DB 에서** 죽는다. 포트 8080 도, 실제 postgres 도 건드리지 않고 **5초 안에 스스로 끝난다.**

**대조가 실패하면 전체가 `미검증(증거 요건)` 이고 exit 4 다.** PASS 가 아니다.

**⚠ 이 수법은 `main.rs` 의 순서에 얹혀 있다 — 그 전제를 같은 실행에서 단언한다**
(architect R24 §F-4). 순서가 뒤집혀 DB 를 먼저 열면 **데이터 로딩은 시도되지도 않은 채** 죽는데
"거부 0건" 은 여전히 참이라 **대조가 아무것도 증명하지 않으면서 계속 통과한다.**
그래서 도구는 `게임 데이터 로딩 완료` 줄이 `오류로 종료됐다` 줄보다 **먼저** 나왔는지를 확인하고
`premise_data_before_db.asserted` 로 싣는다. `false` 면 음성 대조가 **FAIL** 이다.
`selftest` 의 `judge_negative_control` 5케이스가 그 방어를 건다 — **순서를 뒤집은 합성 로그에서
실제로 FAIL 이 난다.**

### A-3. 명령

```bash
SCRATCH=<스크래치패드>/sc06-work
python tests/e2e/server_boot.py selftest          # 합성 로그 양성 1 · 음성 6
python tests/e2e/server_boot.py reject \
  --workdir "$SCRATCH" \
  --evidence _workspace/p1-01-ship-movement/evidence/R19-block1/sc06.json
git status --porcelain data/                      # 반드시 비어 있어야 한다
```

종료 코드: `0` 8/8 PASS + 음성 대조 통과 / `1` FAIL / `2` 바이너리 없음 / **`4` 미검증(음성 대조 실패)**.

### A-4. SC-06 = **8 / 8 PASS** (2026-09-25T22:08:29 빌드, S-R25 적용 후) — 그러나 여기까지 오는 데 근거 둘이 갈렸다

> **다음 사람이 헤매지 않게 갈라 적는다.** architect 가 R24 에서 *"8/8 PASS 유효"* 라고 했다가
> **R25 에서 철회하고 7/8 로 재판정**했고(① FAIL), 그것과 **별개로** 리더의 근거 ②가 열려 있다.

| 근거 | 무엇을 의심했나 | 상태 |
|---|---|---|
| **① 로그가 필드를 특정하는가** (§F-1 · §A-1b) | `rule=schema` 층의 로그로 어느 필드가 위반했는지 알 수 있는가 | **해소 — 사례 ① 은 FAIL 이다.** architect R25: `de_boundary_radius_m` 은 **세 필드가 공유**하므로 필드가 특정되지 않는다. **요구를 낮추지 않는다**(구제가 싸고 표준적이다). ② 는 `de_points_m` 이 1:1 이라 만족하고, ③~⑧ 은 derived 로 다섯 토큰을 이미 찍는다 |
| **② 판정 바이너리가 09-23 빌드** | 그 바이너리가 HEAD 의 `server/` 소스와 같은지 **알 수 없다** | **해소 — 실행으로 닫혔다**(§A-4c). 새 빌드로 다시 돌렸고 **거부 로그 8건이 전부 같다** |

**두 근거가 사례별로 다르게 걸렸다:**

| 사례 | verdict | 근거 |
|---|---|---|
| **①** | 구제 전 **FAIL** → 구제 후 **PASS**(§A-4d) | 근거 ②에 **애초에 걸리지 않았다** — 판정의 두 다리(로그가 함수 이름을 찍는다 · 그 함수를 세 필드가 쓴다)가 **모두 HEAD 소스에서 확인됐다.** 그래서 **소스만으로 FAIL 을 확정할 수 있었다** |
| **②~⑧** | **PASS** | 근거 ②는 §A-4c 로, 형식 변경은 §A-4d 로 닫혔다 |

**이 비대칭이 계약 §7b 규칙 9 가 됐다** (architect R26→R27, qa 삽입):
**소스는 부정을 증명할 수 있지만 긍정은 증명하지 못한다.**
① 의 FAIL 은 소스만으로 확정된다(*"HEAD 소스로 빌드된 어떤 바이너리도 그 사례에서 필드를
특정할 수 없다"*). ②~⑧ 의 PASS 는 그럴 수 없다 — *소스가 `points_m` 을 형식 문자열에 갖고 있다*
는 것은 *그 소스로 빌드된 바이너리가 그 로그를 실제로 냈다* 는 증명이 아니다.

**②가 왜 논증으로 닫히지 않는가.** 리더가 `git log --since='2026-09-23 22:01:51' -- server/` → 0건
으로 닫으려 했다가 **스스로 철회했다**: 그 명령은 **커밋된 변경만** 본다. 09-23 22:01 시점의
워킹트리에 커밋 안 된 편집이 있었는지는 **사후에 알 수 없다.** QA 가 "데이터 검증 경로 세 파일
(`bins/game-server/src/data.rs` · `main.rs` · `crates/contracts/src/data.rs`)에 **지금** 미커밋 변경이
없다" 를 확인했지만, 그것도 **지금**의 상태이고 빌드 시점의 상태가 아니다.
**추론을 실행된 사실로 옮기는 것** — 이 슬라이스가 반복해서 데인 그 형태다.

`cargo` 를 쓰지 않은 이유는 따로 있다: 다른 에이전트가
`server/crates/sim/src/simulation.rs:728` 에 `false &&` 를 걸어 둔 상태였다(릴리스 음성 대조용,
`git diff` 로 확인: `+        if false && let Some(&ship_id) = self.actor_ship.get(&actor_id) {`).
**지금 빌드하면 그 방해가 들어간 서버가 나온다.**

**② 를 어떻게 닫았는가 → §A-4c.** (논증으로 좁히지 않고 **1분짜리 실행**으로 닫았다.)

**재개 조건 (① — server 소유, S-R25)**

`de_f64_range!` 의 메시지가 **필드를 특정하게** 되면 다시 돌린다. 길은 둘이고 architect 가
server 에 넘겼다: **ⓐ `serde_path_to_error`** 로 JSON Pointer 를 채우거나 **ⓑ 공유 검증자를 분리**한다.
`DataError::Schema` 의 **거짓 doc 주석**(*"필드·기대값·실제값이 문장에 들어 있다"*)도 함께 고친다 —
**그 주석이 R24 오판의 근거였다.** **종료 조건은 §A-1b 의 다중도가 전부 1 이 되는 것**이고,
그 검사는 `reject` 의 `validator_field_multiplicity.shared_count == 0` 으로 기계가 본다.

**논증으로 해소하지 않고 실행으로 해소한다.** §A-2 의 방법이면 서버를 안 띄우므로 1분이면 된다.

### A-4d. S-R25 적용 후 최종 실행 (2026-09-25T22:08:29 빌드) — **8 / 8 PASS**

server 가 구제 **ⓐ `serde_path_to_error`** 를 골랐다. `read_and_parse`(`bins/game-server/src/data.rs`)가
세 로더가 지나는 **단일 관문**이라 거기 한 곳만 고쳤고, **공유 검증자 10개는 그대로 남아 있다 —
의도된 것이다.** 리더가 게이트 셋(`fmt` · `clippy` · `test --workspace`)을 exit 0 으로 확인하고 신호를 줬다.

**⚠ 이 실행 전에 판정 도구를 먼저 고쳐야 했다 — §A-1c.**

**규칙 9 이행 — 출처 기록** (도구가 자동으로 싣는다: `source_provenance()`):

```
HEAD                    443ab3c129aaa395691c32029fc981f8eaa53107
worktree_clean          false  ← 공백이 아니다. 12건을 전문 그대로 증거에 실었다
                               (S-R25 가 커밋 대기 중이었다 — 리더가 승인을 받는 중)
server_binary_mtime     2026-09-25T22:08:29
server_binary_sha256_16 339baec449d3177d
```

**더러운 트리를 그대로 적는 것이 규칙을 어기는 것이 아니다** — 조항이 요구하는 것은 *"공백이다"* 가
아니라 **"공백 여부를 함께 남긴다"** 이고, 그래야 **반증 가능성**이 산다(규칙 9 (나)).

**결과**

```
verdict PASS · 8/8 · exit 0
negative_control PASS · premise_data_before_db.asserted true
validator_field_multiplicity.shared_count 10  ← 줄지 않았다. ⓐ 에서는 그게 정상이다
① play_area.hard_boundary_radius_m 를 지칭한다 · 형제 오지칭 없음
② spawn.points_m 를 지칭한다
```

**S-R25 전후 대조** (증거 `evidence/R19-block1/sc06-sr25-before-after.txt`).
**비교한 것은 `error=` 뒤의 형식**이다 — rust 가 검증용으로 넣었다 뺀 임시 `S-R25 CAPTURE` 출력과
문자 비교하지 않는다(리더 주의):

```
형식이 바뀐 사례 2건 (둘 다 rule=schema)
  ① FAIL → PASS
     전: de_boundary_radius_m 는 (0 .. 20000 범위를 벗어난다 (받음: 20000.1) at line 24 column 37
     후: play_area.hard_boundary_radius_m: de_boundary_radius_m 는 (0 .. 20000 …) at line 24 column 37
  ② PASS → PASS
     전: points_m 은 1 ..= 64 개여야 한다 (받음: 0)
     후: spawn.points_m: points_m 은 1 ..= 64 개여야 한다 (받음: 0)

형식이 그대로인 사례 6건 (전부 rule=derived) — 전부 PASS → PASS
```

**`derived` 여섯이 안 바뀌었다는 것도 관측이다**(리더). S-R25 가 안 건드린 경로임을 **재서** 안 것이지
추론한 것이 아니다.

### A-4c. 근거 ②를 닫은 재실행 (2026-09-25T21:50:52 새 빌드)

리더가 `git diff --stat server/crates/sim/src/simulation.rs` **빈 출력**을 확인하고 신호를 줬다.
QA 가 독립으로 재확인한 뒤 빌드했다:

```bash
git status --porcelain server/          # → ws_integration.rs 만 (테스트 파일, 바이너리에 안 들어간다)
git diff --stat server/crates/sim/src/simulation.rs   # → 빈 출력
cd server && cargo build -p starfall-game-server      # 4.47s, sim·gateway·persistence·game-server 재컴파일
```

그리고 §A-3 을 그대로 다시 돌렸다(**서버를 띄우지 않는다** — §A-2 방법):

```
verdict FAIL · 7/8 · exit 1 · server_binary_mtime 2026-09-25T21:50:52
negative_control PASS · premise_data_before_db.asserted true · 데이터 거부 0건
validator_field_multiplicity.shared_count 10 / 29
① FAIL · ②~⑧ PASS   ← 재판정본(§A-4b)과 사례별로 동일
```

**두 실행의 대조** (증거 `evidence/R19-block1/sc06-two-run-diff.txt`).
**정규화한 것은 타임스탬프와 스크래치 경로뿐**이고 **로그 본문(필드명·expected·actual·rule)은
그대로 비교했다**:

```
동일   1-hard-radius-above-ceiling            exit 1→1
동일   2-spawn-points-empty                   exit 1→1
동일   3-point-count-mismatch                 exit 1→1
동일   4-tick-snapshot-ratio-not-integer      exit 1→1
동일   5-resume-window-above-linger           exit 1→1
동일   6-spawn-point-outside-hard-boundary    exit 1→1
동일   7-main-thrust-below-lateral-or-reverse exit 1→1
동일   8-duplicate-ship-class-id              exit 1→1

거부 로그 본문이 다른 사례: 0 / 8
```

**→ 09-23 빌드와 09-25 새 빌드가 SC-06 이 재는 경로에서 같은 동작을 한다. 근거 ②가 닫혔다.**
**다르면 그 차이가 "09-23 빌드에 커밋 안 된 편집이 있었다" 는 증거였을 것이다** — 0 건이므로
그 경우는 없었다. **논증을 쌓아 좁히는 것보다 실행이 싸다는 것이 이 절의 요점이다.**

### A-4b. 첫 관측과 재판정 (2026-09-23T22:01:51 빌드)

**같은 로그를 새 기준으로 다시 재는 방법 — 서버를 다시 돌리지 않는다.**
R25 는 **기준만** 바꿨으므로 관측을 다시 만들 이유가 없다(다시 돌리면 다른 관측이 섞인다):

```bash
python tests/e2e/server_boot.py rejudge \
  --evidence-in _workspace/p1-01-ship-movement/evidence/R19-block1/sc06.json \
  --evidence    _workspace/p1-01-ship-movement/evidence/R19-block1/sc06-rejudged-r25.json
```

```
passed 7 / 8 · exit 1
verdict_changes: [{"case": "1-hard-radius-above-ceiling", "was": "PASS", "now": "FAIL"}]

① 1-hard-radius-above-ceiling  FAIL  공유 검증자 때문에 필드가 특정되지 않는다: de_boundary_radius_m×3
                                     shared_by: soft_boundary_radius_m(:446) · hard_boundary_radius_m(:449) · visual_radius_m(:566)
② 2-spawn-points-empty         PASS  검증자 이름이 로그에 없다 — 메시지가 필드명을 직접 적는다(must_contain 이 확인)
③~⑧                            PASS  derived — 다섯 토큰 전부
```

**강화 전 도구는 ① 을 PASS 로 인쇄했다**(`sc06-raw.json`). 그 차이가 이 라운드의 산출물이다.

#### 원본 실행 출력 (verdict 는 위 재판정이 정본)

```
selftest                 → 19/19 케이스 통과, exit 0 (합성 로그 — 바이너리와 무관하다)
reject                   → verdict PASS, 8/8, exit 0
negative_control.verdict → PASS
  data_load_succeeded      = true
  data_validation_rejects  = 0
  later_stage_error        = "마이그레이션 적용 실패" (DB 단계 — 데이터 단계가 아니다)
git status --porcelain data/ → (빈 출력)
```

| # | 사례 | exit | rule | field 일치 | 심은 값이 로그에 | verdict |
|---|---|---|---|---|---|---|
| ① | `hard_boundary_radius_m = 20000.1` | 1 | schema ✓ | — | `20000.1` ✓ | PASS |
| ② | 스폰 `points_m` 빈 배열 | 1 | schema ✓ | — | `points_m` · `받음: 0` ✓ | PASS |
| ③ | `point_count ≠ len(points_m)` | 1 | derived ✓ | `spawn.point_count` | `actual=13` ✓ | PASS |
| ④ | `tick_hz / snapshot_hz` 비정수 | 1 | derived ✓ | `snapshot.snapshot_hz` | `actual=7` ✓ | PASS |
| ⑤ | `reconnect_resume_window > linger` | 1 | derived ✓ | `presence.reconnect_resume_window_seconds` | `actual=35` ✓ | PASS |
| ⑥ | 스폰 지점이 하드 경계 밖 | 1 | derived ✓ | `spawn.points_m[0]` | `999999` ✓ | PASS |
| ⑦ | `main_thrust < lateral` | 1 | derived ✓ | `movement.main_thrust_mps2` | `actual=0.1` ✓ | PASS |
| ⑧ | 함선 클래스 `id` 중복 | 1 | derived ✓ | `id` | `actual=scout-s01` ✓ | PASS |

로그 한 줄의 실제 모양(③):

```
ERROR starfall_game_server: 게임 데이터 로딩 실패 error=기동 거부: 데이터 검증 실패
file=…\3-point-count-mismatch\world\systems\cradle.json field=spawn.point_count
expected=points_m.len() (12) 과 같다 actual=13 rule=derived
```

**⚠ ①② 는 형식이 다르다 — §F-1 을 보라.**

---

## B. SC-07 — `data/` 3파일 스키마 검증 (회귀 확인)

### B-1. 자명 통과를 막는 자리

- **`0 파일 검증 → 오류 0` 은 통과가 아니다.** 검증기는 glob 이 하나도 안 맞으면
  그 자체를 오류 1건으로 센다(`validate_data_files.py:92-96`). §B-3 의 음성 대조 B 가 그것을 건다.
- **R19 에서 `fields_checked` 를 추가했다.** `files_checked: 3 · errors_total: 0` 만으로는
  검증기가 **파일을 열고 안을 걸었는지**를 볼 수 없다. 재귀 키 수를 싣는다.

### B-2. 명령

```bash
python tests/e2e/validate_data_files.py \
  --evidence _workspace/p1-01-ship-movement/evidence/R19-block1/sc07-positive.json
```

### B-3. 음성 대조 두 가지 — `--root` 로 **사본**을 가리킨다 (원본은 안 건드린다)

| 대조 | 만드는 것 | 기대 |
|---|---|---|
| **A: 일부러 깨뜨린 사본** | `contracts/` + `data/` 를 스크래치에 복사 후 ① ship `main_thrust_mps2` → 문자열 ② system `hard_boundary_radius_m` → 20000.1 ③ tuning `prediction` 제거 + `bogus_field_qa` 추가 | **3파일 전부에서 검출 · 오류 4 · exit 1** |
| **B: 빈 `data/` 하위 디렉터리** | `data/ships`·`data/world/systems`·`data/movement` 를 비워 둔 루트 | **오류 3 · exit 1** (0 파일이 0 오류로 새지 않는다) |

### B-4. 실행 결과 (2026-09-25)

```
양성: PASS · files_checked=3 · fields_checked=128 · errors_total=0 · exit 0
   data/ships/scout-s01.json          fields=44  errors=0
   data/world/systems/cradle.json     fields=58  errors=0
   data/movement/sync-tuning.json     fields=26  errors=0

음성 A: FAIL · errors_total=4 · files_checked=3 · exit 1
   /movement/main_thrust_mps2: 'fast' is not of type 'number'
   /play_area/hard_boundary_radius_m: 20000.1 is greater than the maximum of 20000
   /: 'prediction' is a required property
   /: Additional properties are not allowed ('bogus_field_qa' was unexpected)

음성 B: FAIL · errors_total=3 · files_checked=0 · exit 1
```

**회귀 여부**: architect 실측(2026-09-19)의 `sync-tuning.json` **6건 실패는 돌아오지 않았다** —
그 파일 26필드 전부 오류 0.

---

## C. SC-57 — 잔류 만료 디스폰

계약의 방법 칸이 **SQL(ship_id 기준)** 이므로 **새 판정 도구를 만들지 않는다**
(만들면 출처 게이트가 빨개진다 — 계약 §7b 규칙 7).

### C-1. ⚠ 범위를 반드시 준다 · ⚠ `SERVER_SHUTDOWN` 과 섞지 않는다

`domain_events` 는 추가 전용이라 **범위 없이 세면 다른 실행이 전부 섞인다.**
오늘 실행 구간 **tick 1,576,721 ~ 1,595,272** 로 자른다. 무범위 대조(참고용):

```
무범위: LINGER_EXPIRED 86 (tick 352051~1592035) · SERVER_SHUTDOWN 54 (364788~1045930)
범위 내: LINGER_EXPIRED 40 · SERVER_SHUTDOWN 0
```

**`SERVER_SHUTDOWN` 은 SC-13 의 사유다.** 범위 내에 0건이라 섞일 여지가 없다.

### C-2. 페이로드 키 — **`reason` 이 아니라 `despawn_reason` 이다**

처음 `payload->>'reason'` 으로 세면 **전부 NULL 인데 count 는 40** 이 나온다.
그 상태의 `count(*) > 0` 은 "LINGER_EXPIRED 가 40건" 을 **전혀 뜻하지 않는다.**
`jsonb_pretty(payload)` 로 키를 먼저 확인하고 쓴다.

### C-3. 판정 SQL

```sql
with win as (select * from domain_events where tick between 1576721 and 1595272),
s as (select payload->>'ship_id' ship, payload->>'session_id' sess, tick stick
      from win where event_type='SHIP_SPAWNED'),
c as (select payload->>'session_id' sess, payload->>'close_reason' creason, tick ctick
      from win where event_type='SESSION_CLOSED'),
d as (select payload->>'ship_id' ship, payload->>'last_session_id' sess,
             payload->>'despawn_reason' dr, tick dtick
      from win where event_type='SHIP_DESPAWNED')
select
  (select count(*) from c)                                    as sessions_closed,
  (select count(*) from c where creason='CLIENT_CLOSED')      as closed_normally,
  (select count(*) from s)                                    as ships_spawned,
  (select count(*) from d where dr='LINGER_EXPIRED')          as linger_expired,
  (select count(*) from d where dr='SERVER_SHUTDOWN')         as server_shutdown,
  (select count(*) from c join d on d.sess=c.sess and d.dr='LINGER_EXPIRED'
     where c.creason='CLIENT_CLOSED')                         as normal_close_with_linger_despawn,
  (select count(*) from c where creason='CLIENT_CLOSED'
     and not exists (select 1 from d where d.sess=c.sess and d.dr='LINGER_EXPIRED'))
                                                              as normal_close_without_despawn,
  (select count(*) from d join s on s.ship=d.ship and s.sess=d.sess where d.dr='LINGER_EXPIRED')
                                                              as despawn_matches_spawn_of_same_session,
  (select count(distinct d.ship) from d where dr='LINGER_EXPIRED') as distinct_ships;
```

그리고 **잔류 창이 실제로 지났는가**(항진명제 방지):

```sql
-- d ⨝ c 로 세션 종료 tick 과 디스폰 tick 의 간격을 잰다
select count(*) pairs, min(d.dtick-c.ctick) min_gap, max(d.dtick-c.ctick) max_gap,
       count(*) filter (where d.dtick-c.ctick =  600) gap_equals_linger,
       count(*) filter (where d.dtick-c.ctick <> 600) violations,
       string_agg(distinct c.creason, ',') close_reasons
from d join c on c.sess=d.sess;
```

**임계는 `== 600` 이다 — `>=` 도 `>` 도 아니다** (architect R24, ADR-0011 §6 표에 반영됨).

- `linger_seconds 30 × tick_hz 20 = 600 tick` 이고, `sim/src/simulation.rs:1041,1049` 가
  `tick_number - linger_started_tick >= linger_ticks` 로 판단한다. **스윕이 매 tick 돌므로
  조건이 참이 되는 첫 tick 이 정확히 `linger_ticks`** 다 — 그래서 관측값은 결정적으로 600 이다.
- 처음 **`> 600`** 으로 썼다 → **40건 전부가 "창을 안 넘겼다"** 로 인쇄됐다. 판정이 뒤집히는 게
  아니라 **반대로 인쇄되는** 종류의 오차다.
- 그다음 **`>= 600`** 으로 고쳤다 → verdict 는 맞지만 **늦게 지는 경우를 통과시킨다.**
  잔류가 700 tick 뒤에 지는 회귀가 조용히 통과한다. **등호만 양쪽을 잡는다.**

**음성 대조 — 등호가 실제로 무언가를 고쳤는가** (증거: `evidence/R19-block1/sc57-boundary-equality.txt`).
간격 601(늦게 짐)을 심은 합성 입력을 `VALUES` CTE 로 만든다(**DB 에 쓰지 않는다**):

```sql
with c(sess,ctick) as (values ('s1',1000),('s2',2000)),
     d(sess,dtick) as (values ('s1',1600),('s2',2601))   -- s1=600(정상) · s2=601(늦게 짐)
select count(*) pairs,
       count(*) filter (where d.dtick-c.ctick <> 600) violations_by_equality,
       count(*) filter (where d.dtick-c.ctick <  600) violations_by_old_ge_check
from d join c on c.sess=d.sess;
```

```
pairs | violations_by_equality | violations_by_old_ge_check
    2 |                      1 |                         0
```

**등호 검사는 601 을 위반 1건으로 잡고, 옛 `>= 600` 검사는 0건으로 통과시킨다.**
실데이터는 40/40 이 정확히 600 이므로 **verdict 는 재판정하지 않았다 — 도구만 조였다.**

### C-4. 실행 결과 (2026-09-25, 범위 tick 1,576,721~1,595,272 · `recorded_at` 10:58:04Z ~ 11:09:20Z)

| 수량 | 값 |
|---|---|
| 범위 내 이벤트 총수 | 160 (`SESSION_OPENED` 40 · `SESSION_CLOSED` 40 · `SHIP_SPAWNED` 40 · `SHIP_DESPAWNED` 40) |
| 정상 종료한 세션(`CLIENT_CLOSED`) | **40** |
| `SHIP_DESPAWNED{LINGER_EXPIRED}` | **40** (distinct ship_id **40**) |
| `SHIP_DESPAWNED{SERVER_SHUTDOWN}` | **0** |
| 정상 종료 ↔ 잔류 만료 디스폰 짝 | **40 / 40** |
| 정상 종료했는데 디스폰이 없는 세션 | **0** |
| 디스폰 함선이 같은 세션의 `SHIP_SPAWNED` 함선과 일치 | **40 / 40** |
| 세션 종료 → 디스폰 간격 | **min 600 · max 600 tick** (= 정확히 30초) |
| **`gap == 600` 인 짝** | **40 / 40** — 등호 위반 **0건** (architect R24 의 단언) |

표본 행(앞 5건):

```
ship                                   close_tick  despawn_tick  gap
01a0d837-18af-7176-b619-ae2097259ffb   1578662     1579262       600
01a0d837-d0dd-717c-a115-a4d9c1bbc838   1579604     1580204       600
01a0d83b-197d-752e-85fb-c3431bedfebc   1585009     1585609       600
01a0d83b-1adc-7192-9e26-d559d0f73743   1585015     1585615       600
01a0d83b-1a45-7428-95ec-27b65f21ad2a   1585015     1585615       600
```

**음성 대조(카운터 항등식이 0 입력에서 무너지는가)**: 이벤트가 없는 범위
(`tick between 1595273 and 1699999`) 로 같은 SQL 을 돌리면
`sessions_closed=0 · linger_expired=0 · any_events=0` 이다. **그 경우는 PASS 가 아니라
`미검증(표본 없음)`** 이다 — 오늘 구간은 표본 40 이라 그 자리에 걸리지 않았다.

---

## D. 판정 명령 한눈에 · 종료 코드

| 항목 | 명령 | 입력 | 오늘 결과 |
|---|---|---|---|
| **SC-06** | `server_boot.py reject --workdir <scratch>` | `data/` 임시 사본 8벌 + 음성 대조 1벌 | **PASS 8/8 · exit 0** (S-R25 후) |
| SC-06 재판정 | `server_boot.py rejudge --evidence-in <sc06.json>` | 이미 남긴 거부 로그(서버 재실행 없음) | **7 / 8** · exit 1 |
| 도구 자체 | `server_boot.py selftest` | 합성 로그 (`judge_case` 9 · schema 필드 지칭 5 · `judge_negative_control` 5) | exit 0 · **19/19** |
| **SC-07** | `validate_data_files.py [--root <사본>]` | `data/` 3파일 + `contracts/data/*.schema.json` | exit 0 (양성) · exit 1 (음성 A·B) |
| **SC-57** | 위 SQL (psql) | `domain_events` **읽기만** | 표본 40 · 짝 40/40 |

SC-06 종료 코드: `0` PASS / `1` FAIL / `2` 환경 / **`4` 미검증(음성 대조 실패)**. **`4` 는 PASS 가 아니다.**

---

## E. 출처 게이트 — **빨개지지 않았다**

```
python tests/e2e/check_item_sources.py --selftest                              → PASS, 케이스 24, exit 0
python tests/e2e/check_item_sources.py --contract _workspace/p1-01-ship-movement/02_sprint_contract.md
  계약 항목: 90 / 도구가 지명된 항목: 90 / 90 (도구 26개) / 검사한 도구 28
  위반 없음 → exit 0
```

**새 판정 도구를 만들지 않았기 때문**이다. `server_boot.py`(SC-06) 와 `validate_data_files.py`(SC-07)
는 계약 §3.2 도구 표 **368·369행**이 이미 지명하고 있고, SC-57 은 계약이 지정한 **SQL** 로 잰다.
`server_boot.py` 에 붙인 `selftest` 하위 명령은 **verdict 를 내지 않으므로** 출처 주장이 아니다.

---

## F. 계약이 침묵·모순하는 자리 — **architect 에게 (qa 가 고치지 않았다)**

> **판정 완료 (architect R24, 2026-09-25).** F-1·F-3·F-4 는 **계약 결함**으로 판정돼
> 계약 §1 SC-06·SC-57 방법 칸과 `docs/adr/0011` §6 이 고쳐졌다. 전문은
> `01_architect_decisions.md` 의 `## R24 판정`. **아래는 QA 가 올린 원문이고, 각 항목 끝에
> 판정 결과를 붙였다.** `contracts/` 무변경 · 판정 기준 칸 무변경 · 새 임계값 0건 · 항목 수 90 불변.

### F-1. SC-06 의 로그 형식이 ①② 에 대해 **자기모순이다** ⚠

계약 §1 SC-06 판정 칸은 **여덟 건 모두**에 대해 이 형식을 요구한다:

```
ERROR 기동 거부: 데이터 검증 실패 file=<절대경로> field=<JSON Pointer> expected=<제약> actual=<값> rule=<schema|derived>
```

그러면서 같은 칸이 **①②는 `rule=schema`** 라고 못 박는다. 그런데 구현의 `rule=schema` 변형
(`server/bins/game-server/src/data.rs:57-65`, `DataError::Schema`)은 **`field=`·`expected=`·`actual=`
토큰을 아예 갖고 있지 않다** — serde 메시지를 `error=` 하나에 담는다:

```
… file=…\cradle.json error=points_m 은 1 ..= 64 개여야 한다 (받음: 0) at line 46 column 18 rule=schema
```

**둘 다 만족시킬 수 없다.** 판정을 임의로 정하지 않았다. 도구는 **derived 에만 네 토큰을 강제**하고
schema 층은 **형식 관측만 `log_format_tokens` 로 싣는다**(증거에 그대로 있다).
**→ 판정 (architect R24 → **R25 에서 철회**). 최종: SC-06 은 **7/8**, 사례 ① **FAIL**.**

R24 는 아래와 같이 판정했다가 **R25 에서 스스로 철회했다** — *"필드·기대값·실제값이 문장에
들어 있다"* 는 **구현 doc 주석을 증거로 쓰고 실제 로그를 보지 않았다.** 그 주석은 거짓이다
(§A-1b). **doc 주석은 주장이지 증거가 아니다.**

**R25 최종 판정**: 갈리는 기준은 **"검증자가 필드와 1:1 인가"** 다. ② 는 우연이 아니라 1:1 이라서
만족하고, ① 은 세 필드 공유라서 **FAIL** 이다. **요구를 낮추지 않는다** — 구제(ⓐ `serde_path_to_error`
· ⓑ 공유 검증자 분리)가 싸고 표준적이며, **싸게 고칠 수 있는 요구를 낮추는 것은 §7 의 임계 인상과
같은 종류**다. 그리고 낮추면 SC-06 이 보증하는 것이 바뀐다 — *"거부됐다"* 와 *"그 필드 때문에
거부됐다"* 는 다르고 **후자가 이 항목의 값**이다. R24 의 *"(b) 미룸"* 도 철회됐다:
**요구를 만족하지 않으면 미루는 게 아니라 FAIL 이다.**

<details><summary>철회된 R24 판정 (원칙 5 — 지우지 않고 남긴다)</summary>

**(a) 다. 틀린 것은 방법 칸이고 구현도 QA 의 판정 기준도 옳다.**
판정 기준 칸이 요구한 것은 **토큰이 아니라** *"로그에 파일·필드·기대값이 **나온다**"* 이고,
serde 문장이 그것을 **문장으로** 담는다(`points_m 은 1..=64 개여야 한다 (받음: 0)`). 구현의 doc
주석도 *"필드·기대값·실제값이 문장에 들어 있다"* 라고 적고 있었다 — **구현자는 알고 있었고
계약만 한 가지 모양을 가정했다.** 계약이 두 층을 갈라 적도록 고쳐졌다:

| 층 | 단언 |
|---|---|
| `rule=derived` | 다섯 토큰 전부 |
| `rule=schema` | `file=` · `rule=schema` **＋ 심은 값과 대상 파일명이 메시지에 있다** |

**QA 가 택한 형태가 토큰보다 약한 것이 아니다.** 토큰 존재는 *"형식이 맞다"* 만 말하는데,
`20000.1` · `받음: 0` · `cradle.json` 을 확인하는 것은 **"거부 이유가 내가 심은 그것인가"** 를
묶는다 — **§7b(1) 의 짝 단언**이다. (b)(`serde_path_to_error` 로 토큰을 채우는 길)는 **측정된
필요가 없어 미뤘다**(원칙 8·10). 발동 조건: 부팅 거부를 **기계가 일괄 파싱해야 하는 소비자**가 생길 때.

</details>

**QA 가 택한 형태가 재판정을 가능하게 했다** (architect R25): derived 에만 토큰을 강제하고
schema 층은 **형식 관측만 실어 둔** 덕분에, R24 가 틀린 뒤에도 **증거가 남아 있어서** 같은 로그로
다시 잴 수 있었다(§A-4b 의 `rejudge`).

**⚠ 그리고 ①의 FAIL 이 ②~⑧ 을 PASS 로 만들지 않는다** — §A-4 의 **근거 ②**(바이너리 시점)가
②~⑧ 에 그대로 열려 있다.

### F-2. `rule=schema` 로그가 **필드명 대신 Rust 역직렬화 함수 이름**을 찍는다 — **계약 외 발견이 아니라 SC-06 ① 의 FAIL 근거가 됐다** (server 소유, S-R25)

사례 ① 의 실제 로그:

```
error=de_boundary_radius_m 는 (0 .. 20000 범위를 벗어난다 (받음: 20000.1) at line 24 column 37
```

- `de_boundary_radius_m` 은 **JSON 필드명이 아니다.** 필드는 `play_area.hard_boundary_radius_m` 이고,
  저것은 `server/crates/contracts/src/data.rs:36-57` 의 `de_f64_range!` 매크로가 찍는
  **`stringify!($fn_name)` — 검증 함수 이름**이다. 같은 함수를 여러 필드가 공유하면
  로그만 보고 **어느 필드인지 특정할 수 없다**(`at line/column` 이 간접 단서일 뿐이다).
- 구간 표기도 닫히지 않는다: `(0 .. 20000` — `data.rs:49-54` 의 포맷 문자열이 여는 괄호만 낸다.

**이것은 SC-06 의 판정량에 직접 닿고, 공유는 미래의 위험이 아니라 이미 벌어져 있다.**
계약이 요구하는 것은 *"로그에 파일·필드·기대값이 나온다"* 인데, `de_f64_range!` 로 만든 검증
함수는 **이미 여러 필드가 공유한다**(실측):

```
$ grep -o 'deserialize_with = "de_[a-z0-9_]*"' server/crates/contracts/src/data.rs | sort | uniq -c | sort -rn
      7 de_accel_mps2
      3 de_positive_upto_100000
      3 de_boundary_radius_m        ← 사례 ① 이 찍는 이름
      3 de_accel_deg_s2
      2 de_spawn_radius_m · de_rate_deg_s · de_presence_seconds_i · …
```

**사례 ① 의 `de_boundary_radius_m` 은 세 필드가 쓴다** — `PlayArea.soft_boundary_radius_m`
(`data.rs:445-446`) · `PlayArea.hard_boundary_radius_m`(`:448-449`) · `visual_radius_m`(`:565-566`).
그러므로 `error=de_boundary_radius_m 는 …` 한 줄로는 **세 필드 중 어느 것이 위반했는지 알 수 없다.**
`at line 24 column 37` 이 유일한 간접 단서이고, 그것은 JSON Pointer 가 아니다.
`de_accel_mps2` 는 **일곱 필드**가 공유한다.

**즉 `rule=schema` 층에서 계약의 "필드" 요구는 지금 이미 만족되지 않는다.** SC-06 을 FAIL 로
돌리지 않은 이유는 F-1 의 계약 모순 때문이고(①② 에 `field=` 를 요구하는지가 불명확하다),
**그 모순이 풀리는 방향에 따라 이 발견이 곧 SC-06 의 FAIL 근거가 된다.**

사례 ②(`points_m 은 1 ..= 64 개여야 한다`)는 **필드명을 제대로 찍는다** — 다른 경로라서다.
**처음에는 SC-06 을 FAIL 로 돌리지 않고 발견으로만 올렸다** — 계약이 ①② 에 `field=` 를
요구하는지가 F-1 대로 불명확했기 때문이다. **architect R25 가 그 불명확을 풀었고(요구한다),
그래서 이 발견이 곧 사례 ① 의 FAIL 근거가 됐다.** 기계 검사로 바꿨다 — §A-1b.
**server 작업(S-R25)**: ⓐ `serde_path_to_error` 또는 ⓑ 공유 검증자 분리 ·
`DataError::Schema` 의 **거짓 doc 주석 수정** · ① 재실행.

### F-3. SC-57 의 "정상 종료" 가 **누구의 종료인지** 계약이 적지 않는다 — **판정: 둘 다 계약 결함**

- 클라이언트 세션의 정상 종료(`close_reason=CLIENT_CLOSED`) 로 읽었다. 그래야 SC-13 의
  `SERVER_SHUTDOWN` 과 갈린다.
- 또 계약이 **잔류 창 경계의 방향**을 적지 않는다. 실측 gap 이 **정확히 600 tick** 이라
  이 차이가 곧 verdict 를 뒤집는다(§C-3).

**→ 판정 (architect R24): 둘 다 계약 결함이다.** 정상 종료 = `close_reason = CLIENT_CLOSED`
(QA 의 읽기와 근거가 맞다). 경계는 **등호** — `SHIP_DESPAWNED.tick − SESSION_CLOSED.tick
== linger_ticks`. **`>=` 는 늦게 지는 경우를 통과시키므로** 등호만 양쪽을 잡는다. 새 임계값이
아니라 **이미 결정론적인 값을 그대로 단언하는 것**이다. **`docs/adr/0011` §6 표**(잔류 창의
정본)에 경계 방향이 **없었고**, 그 침묵이 이 검사를 한 글자에 걸리게 했다 —
QA 가 `simulation.rs:1041,1049` 를 읽어서 맞춘 것이지 문서가 알려 준 것이 아니다.
**도구는 `==` 로 조였고 601 음성 대조를 넣었다(§C-3). verdict 는 재판정하지 않았다.**

### F-4. SC-06 의 **음성 대조 방법**을 계약이 정하지 않는다

계약은 거부 8건만 적는다. "멀쩡한 `data/` 로는 기동이 성공한다" 는 §7a 가 요구하는 것이지
SC-06 행이 적은 것이 아니다. **포트를 잡지 않는 §A-2 의 방법**을 썼다 — 리더가 서버 기동을
금지했기 때문이다. **계약에 남길지는 architect 판단이다.**

**→ 판정 (architect R24): 계약에 남겼다 — 레시피만이 아니라 전제와 함께.**
이 기법은 `main.rs` 의 **data → DB 순서에 얹혀 있고, 그 순서가 바뀌면 아무것도 증명하지 않으면서
계속 통과한다.** 그래서 방법 칸이 **전제를 함께 확인하라**고 적는다. 도구에 그 단언을 넣었다
(`premise_data_before_db.asserted` — §A-2). **8099 실기동은 철회됐다**(리더): tick 이 전진하지
않으므로 기동 전후 tick 을 적을 것도 없고, 그게 이 방법의 요점이다.

---

### F-5. SC-07 이 **함선 클래스의 삭제를 못 본다** — 그리고 더 큰 것이 옆에 있다 (만기 있음)

**공백**: `validate_data_files.py` 는 **기대 파일 수가 없다.** `data/ships/*.json` 이 3개에서 2개로
줄어도 **glob 이 0 매치가 아니면 통과한다**(0 매치만 오류 1건으로 센다 — §B-3 음성 대조 B).

**고정 수를 박는 것은 틀렸다.** 그 도구는 *"designer 가 클래스를 한 벌 더 추가하면 자동 포함"* 을
설계로 적고 있고, 계약 SC-07 의 "3파일" 은 **glob 3종**을 말한다. `== 3` 을 박으면 **designer 의
정상적인 추가가 FAIL 로 인쇄된다** — qa11 이 지운 `RUST_KNOWN_MISMATCHES = 8` 과 **같은 병을
반대편에서** 만드는 것이다. *손으로 적은 수는 늘어날 때 틀리고, 수를 안 적으면 줄어들 때 못 잡는다.*

**세 번째 길(리더)**: *"몇 개인가" 를 세지 말고 **"참조된 것이 다 있는가"** 를 봐라.* 그래서 확인했다:

```
grep -rn "scout-s01\|ship_class" data/   →  data/ships/scout-s01.json:3 의 자기 id 한 줄뿐
```

**`data/` 안에서 함선 클래스를 가리키는 곳이 하나도 없다.** 그러므로 **참조 무결성 검사를 걸 대상이
지금은 없고, 이 공백은 p1-01 에서 위험이 아니다** — `ship_classes=1` 이라 *"한 벌이 사라졌다"* 는
*"전부 사라졌다"* 이고 그건 glob 0 매치로 이미 잡힌다.

**⚠ 그런데 찾다가 더 큰 것을 봤다 — 서버가 클래스를 `BTreeMap` 순서로 고른다.**

```rust
// server/bins/game-server/src/main.rs:346-350
let (_, ship_class_table) = game_data.ship_classes.iter().next()
    .ok_or("data/ships/ 에 함선 클래스가 없다 — S2 가 이미 막았어야 한다")?;
```

`ship_classes` 는 `BTreeMap<String, _>` 이므로 `.iter().next()` 는 **id 사전순 첫 번째**다.
**선언된 선택이 아니라 이름 순서다.** 클래스가 둘 이상이 되는 순간:

- **`scout-s01` 보다 앞서는 id**(예: `frigate-f01`)를 designer 가 추가하면 **월드가 쓰는 함선이
  조용히 바뀐다.** 추가는 정상 작업인데 결과가 전면적이다.
- `sim/src/simulation.rs:1375`(테스트의 `world()`)는 `"scout-s01"` 을 **박아 두고** 있어
  **테스트와 런타임이 서로 다른 클래스를 보게 된다.**

**지금은 클래스가 하나라 두 경우 다 일어나지 않는다.** 그래서 **고치지 않았고 부르지도 않았다.**

**만기(계약 §7b 규칙 8 형식) — 날짜가 아니라 게이트다. 분모가 있다: `data/ships/*.json` 의 개수.**

> `미뤄둠(만기: **함선 클래스가 둘 이상이 되는 시점**) — ① SC-07 이 클래스 삭제를 못 본다(참조
> 무결성 검사를 걸 대상이 지금 없다) · ② 서버가 클래스를 `BTreeMap` 사전순으로 고른다
> (`main.rs:346`) · 만기가 지나면 **리포트 요약에 비-통과로 올라온다**`

**계약 반영은 architect 소유다** — `data/` 소유(designer)와 계약이 함께 걸리는 문제라 qa 가 §7b 에
임의로 더 넣지 않았다. **리더 지시로 지금 부르지 않고 Unity 둘 뒤에 묶어서 본다.**

---

## G. 이 절차서가 닫지 **못하는** 것

### G-0. ⚠ S-R25 가 **이 판정 직후에 착수됐다** — ②~⑧ 도 다시 돌아야 한다

§A-4c 의 확정(2026-09-25T21:52 기재) **직후**에 server 가 S-R25 를 시작했다. 실측:

| 파일 | mtime | 무엇 |
|---|---|---|
| `server/Cargo.toml` · `bins/game-server/Cargo.toml` | 21:51:39 · 21:51:42 | `serde_path_to_error = "0.1.20"` 추가 (구제 ⓐ) |
| `server/target/debug/starfall-game-server.exe` | **21:52:06** (11,363,328 B) | **server 의 빌드** — 내 빌드(21:50:52, 10,976,256 B)가 아니다 |
| `server/bins/game-server/src/data.rs` | 21:54:48 | `serde_path_to_error::deserialize` 로 감싸고 `DataError::Schema` 에 JSON 경로를 담는다 |

**내 판정은 내 빌드로 났다** — 증거의 `server_binary_mtime` 이 **21:50:52** 이고 실행은 21:52:06
이전에 끝났다. **`server_binary_mtime` 을 증거에 박아 둔 것이 이 확인을 가능하게 했다**(같은 장치가
리더의 `git log` 논증을 검산하게 했던 것과 같다).

**그러나 S-R25 는 `DataError::Schema` 자체를 고친다 — 그러면 `rule=schema` 로그의 모양이 바뀌고,
②(`points_m` 빈 배열)도 그 층이다.** 즉 **① 만이 아니라 ② 도 다시 돌아야 한다.**
`rule=derived` 인 ③~⑧ 은 다른 변형이라 영향이 없어 보이지만, 같은 파일이므로 **한 번에 8건을
다시 도는 것이 싸다.**

**§A-4c 의 ②~⑧ PASS 는 "S-R25 이전 빌드에 대한 판정" 으로 읽어야 한다.** 그것이 무효는 아니다 —
리더의 근거 ②(09-23 빌드 문제)를 닫은 것은 유효하다. **새로 생긴 것은 다른 재실행 조건이다.**

**→ 해소됐다(§A-4d).** 리더가 게이트 셋 exit 0 을 확인하고 신호를 줬고, 8건 전부를 다시 돌려
**8/8 PASS** 를 얻었다. **`derived` 여섯의 형식이 안 바뀌었다는 것도 재서 확인했다.**

---

- **`data.rs` 가 또 바뀐 뒤의 SC-06.** 2026-09-25T22:08:29 빌드(S-R25 적용)로 확정했다.
  `server/bins/game-server/src/data.rs` 나 `server/crates/contracts/src/data.rs` 가 바뀌면
  **로그 형식이 곧 판정량이므로 다시 돌린다.**
- **서버가 정상 기동해 포트를 잡는 경로**(SC-05). §A-2 는 DB 단계 직전까지만 보인다.
  SC-05 는 `server_boot.py stats` 가 판정하고 **이 문서의 범위가 아니다.**
- **오늘 구간 밖의 SC-57.** tick 1,576,721~1,595,272 에 대한 판정이다. 다른 실행을 닫으려면
  그 실행의 tick 범위로 다시 돌린다.
