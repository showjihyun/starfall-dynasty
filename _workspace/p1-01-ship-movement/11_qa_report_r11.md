# p1-01 QA 평가 리포트 R11 — Observer CSV 열 동기화 준비 · SC-63 판정 입력 확정

- 일시: 2026-09-24 KST
- 담당: qa (integration)
- 입력: 리더 R12 지시 3건, `01_architect_decisions.md` `## R10 후속 판정 (R22 이후)`, `02_sprint_contract.md` 12차 개정(SC-63·64·65) 및 §4 블록표, `client/Assets/_Project/Scripts/Greybox/ObserverCsv.cs`, `.../ObserverSession.cs`, `tests/e2e/two_client_view.py`, `tools/bots/src/snapshot.rs`
- 증거: `_workspace/p1-01-ship-movement/evidence/R11-csv-columns-and-sc63/`
- **고친 것은 `tests/e2e/two_client_view.py` 한 파일뿐이다.** C# 을 고치지 않았고, `COLUMNS` 의 값을 바꾸지 않았고, 커밋하지 않았고, `unity test` 를 돌리지 않았고, 서버·docker 를 건드리지 않았다. 골든 파일·`data/`·`contracts/` 무접촉.

---

## §7a — 실행해 확인한 것과 읽어 추론한 것

| 사실 | 어떻게 |
|---|---|
| `two_client_view.py` 의 헤더 불일치 검출기가 **살아 있다** — 개명·순서바꿈·열추가·열삭제 네 변형을 전부 `NotImplementedYet` 으로 거부하고, 정확히 일치하는 헤더만 수락한다 | **실행**(증거 01) |
| 그 검출기를 죽이면 selftest 가 **FAIL / exit 1** 로 돌아선다 — `header_guard` 는 장식이 아니라 하중을 받는다 | **실행**(증거 04) |
| **SC-63 의 판정 입력은 이 CSV 다** — 계약 §4 블록표 `\| 7 \| 2 클라이언트 가시성 (S-3) \| SC-61~65 \|` 이고, SC-61/62/63 을 산출하는 코드는 `two_client_view.py` 의 `cmd_compare` 하나뿐이며(`:94-102`) 그 입력은 `load()` 가 읽는 이 CSV 다. 레포 전체에서 SC-63 을 판정하는 다른 경로는 없다 | **실행**(레포 전체 grep) + **읽어서 확인**(계약 §4) |
| **SC-63 과 SC-65 는 같은 `(tick, ship_id)` 쌍 위에서 서로 배타적이다** — SC-65 가 기대하는 바로 그 층 관계(B 가 200 ms 뒤)를 먹이면 `s3` 는 SC-65 **PASS**(`behind_mean_m = 28.0`)를, `compare` 는 SC-63 **20/20 불일치 · verdict FAIL · exit 1** 을 낸다 | **실행**(증거 02·03, **같은 두 파일**) |
| A 의 자기 함선 행 = `_controller.CurrentState`(예측), 타 함선 행 = `GetDisplay(tick − 보간지연)`(보간). 원시 wire 값을 쓰는 행은 **한 줄도 없다** | **읽어서 확인**(`ObserverSession.cs:182`·`:206-215`·`:288`, `WriteRow` 는 `state` 인자만 양자화한다) |
| 같은 헤더의 **세 번째 생산자**가 있다 — `tools/bots/src/snapshot.rs:25-26` 의 `SNAPSHOT_CSV_HEADER` | **읽어서 확인** |
| 실서버 세션에서 두 CSV 의 `(tick, ship_id)` 교집합이 비어 있지 않다(= SC-63 이 0쌍 비교로 자명 통과하지 않는다) | **읽어서 추론했고 실행으로 확인하지 못했다.** 두 세션이 같은 서버 tick 의 스냅샷을 받으므로 겹친다고 본다. 자기 함선 행은 프레임당 1회(배치 리베이스)라 교집합이 부분집합일 수 있다. **블록 7 에서 `pairs_compared` 를 실측해 적는다** |

---

## 1. 작업 2 (확인) — **SC-63 의 판정 입력은 이 CSV 다. 그리고 SC-63 은 평활화 이전에 이미 깨져 있었다**

### 1.1 판정 입력 확정

- 계약 §4 블록표: **블록 7 = SC-61~65**.
- 레포 전체(`tests/`·`tools/`·`client/`·`server/`)에서 SC-63 을 산출하는 코드는 `tests/e2e/two_client_view.py:94-102`(`cmd_compare` 의 `mismatch` 루프) **하나뿐**이다. 다른 판정 경로는 없다.
- 그 루프의 입력은 `load(args.a)` / `load(args.b)` — **바로 이 Observer CSV** 다.

**따라서 architect 가 유보를 건 질문의 답은 "이 CSV 다".**

### 1.2 그러므로 SC-63 은 항진명제가 아니라 **모순**이다

계약 SC-63 의 근거는 *"두 값은 같은 `ships` 배열에서 나오므로 정상 구현에서는 다를 수 없다"* 이다. **이 주장은 이 데이터 원천에 대해 거짓이다.** 어떤 행도 wire 값을 그대로 쓰지 않는다:

| 함선 | A 의 CSV 행 | B 의 CSV 행 |
|---|---|---|
| A 의 함선 | **예측**(`_controller.CurrentState`) | **보간**(`tick − 200 ms`) |
| B 의 함선 | **보간**(`tick − 200 ms`) | **예측** |

**모든 함선에 대해 정확히 한쪽이 예측이고 다른 쪽이 보간이다.** 그리고 그 두 층의 차이가 바로 **SC-65 가 28 ± 12 m 로 요구하는 양**이다. 같은 쌍에 두 계약 항목이 서로 반대를 요구한다.

**실행 증거 (증거 02·03 — 입력 파일이 같다):**

```
compare  → "SC-63": { "mismatching_pairs": 20 }   verdict FAIL   exit 1
s3       → "SC-65_verdict": "PASS"   behind_mean_m = 28.0
```

이 두 줄은 **같은 두 CSV** 에서 나왔다. A 는 예측 위상, B 는 A 를 200 ms 뒤에서 본 보간 위상 — **SC-65 가 정상이라고 정의한 바로 그 상태**다.

### 1.3 판정

- **SC-63 = FAIL 로 예측된다 (블록 7 실행 전 정적 판정).** 함선이 움직이는 모든 tick 에서 불일치가 난다.
- **이것은 R22 평활화가 만든 것이 아니다.** 자기 함선 행이 `CurrentState`(예측)인 것은 R22 이전부터이고, 타 함선 행이 보간인 것도 원래부터다. **R22 는 이 항목을 조금도 악화시키지 않았다** — 오프셋은 mm~0.3 m 규모이고 층 불일치는 이미 28 m 규모다.
- **SC-63 이 지금까지 초록이었다면**, 그것은 (i) 블록 7 을 정지 상태에서만 돌렸거나 (ii) 교집합이 비어 `pairs_compared = 0` 이었기 때문이다. **둘 다 "0 == 0 통과"의 변종**이다(architect R9 §3 (나)). 블록 7 리포트에 `pairs_compared` 와 표본의 `speed_mps` 분포를 **반드시** 싣는다.
- **계약을 고치지 않았다.** SC-63 의 처분(문구 수정 / 판정 입력 변경 / 폐지)은 architect 몫이다. 선택지만 적는다:
  1. **판정 입력을 바꾼다** — 원시 wire 값을 쓰는 별도 산출물(예: 봇의 `snapshot.csv`, `tools/bots/src/snapshot.rs`)끼리 비교한다. 그러면 "같은 `ships` 배열" 논거가 처음으로 참이 된다.
  2. **조건을 좁힌다** — `speed < still_speed_mps` 인 쌍에만 정수 일치를 요구한다(정지 시 두 층이 수렴하므로). 다만 그때는 **정말로** 항진명제에 가까워진다.
  3. **폐지한다** — 계약 자신이 *"진짜 검증은 SC-61·62다(I-25)"* 라고 적고 있다.

### 1.4 `ObserverSession.cs:179` 주석이 어디까지 맞는가

> *"SC-64/65 measure the two observers' SCREENS, not the two observers' raw snapshots — the latter is SC-63's job and is intentionally tautological"*

- **맞는 절**: SC-64/65 가 화면을 잰다는 것(12차 확정), 그리고 SC-63 이 *의도상* 원시 비교라는 것(계약 문구가 실제로 그렇게 적혀 있다).
- **틀린 절**: *"the latter is SC-63's job"* — **SC-63 은 원시 스냅샷을 받지 못한다.** 이 세션이 쓰는 파일에는 원시 행이 없고, SC-63 의 유일한 판정 경로가 그 파일이다. 주석은 **존재하지 않는 대조를 SC-63 의 몫으로 가리킨다.**
- **`intentionally tautological`** 도 틀렸다 — 실측으로 20/20 불일치가 난다.
- 이 주석은 C# 이고 client 소유다. **client 에게 수정 요청을 보냈다**(§4).

---

## 2. 작업 1 (동기화) — **준비 완료, 이름·순서 대기 중. 완료가 아니다.**

### 2.1 지금 한 것

`tests/e2e/two_client_view.py` 의 `cmd_selftest` 에 **헤더 불일치 검출기의 양성 대조**를 영구히 넣었다. `COLUMNS` 에서 파생시키므로 **열이 추가·개명돼도 대조가 따라 움직인다** — 확정된 이름을 추측해 넣지 않았다.

검사하는 다섯 가지:

| 변형 | 기대 | 실측 |
|---|---|---|
| `renamed` (이름만 다름) | 거부 | `rejected(NotImplementedYet)` |
| `reordered` (순서만 다름) | 거부 | `rejected(NotImplementedYet)` |
| `appended` (**client 만 고친 상태**) | 거부 | `rejected(NotImplementedYet)` |
| `truncated` (**qa 만 고친 상태의 거울**) | 거부 | `rejected(NotImplementedYet)` |
| `exact_match` (**음성 대조**) | 수락 | `accepted` |

`header_guard_ok = true` 이고, 이 값이 selftest 의 `verdict` 에 **AND 로 들어간다**.

**검출기 사망 시험(증거 04):** `load()` 의 `fieldnames` 검사를 건너뛰는 버전으로 바꿔치기하면 selftest 가 **`verdict: FAIL` · `header_guard_ok: false` · exit 1** 로 돌아선다. 통과가 무언가를 재고 있다는 뜻이다.

```
python tests/e2e/two_client_view.py selftest
→ verdict PASS · columns_count 10 · header_guard 5/5 기대대로
```

### 2.2 지금 **하지 않은** 것 — 대기 상태

- **`COLUMNS` 의 값을 바꾸지 않았다.** client 가 확정할 최종 이름·순서를 리더에게서 받기 전까지 추측하지 않는다.
- 모듈 docstring `:4` 의 하드코딩된 헤더 문자열, `:12-14` 의 SC-63 항진명제 설명 — **둘 다 갱신 대기**. 후자는 §1 의 architect 판정이 나온 뒤에 고친다(계약 문구를 내가 앞질러 쓰지 않는다).

### 2.3 이름이 오면 한 번에 들어갈 변경 (qa 쪽 전부)

1. `two_client_view.py:31-34` `COLUMNS` — 끝에 열 추가.
2. `two_client_view.py:1-19` docstring 의 헤더 문자열.
3. `cmd_selftest` 의 `write()` — 행 생성기가 지금 10열 고정이다(`f"{tick},{actor},{ship},ACTIVE,0,0,{z},0,0,140000"`). 새 열 수에 맞춘다.
4. **`tools/bots/src/snapshot.rs:25-26` `SNAPSHOT_CSV_HEADER` — 세 번째 생산자. §3 참조.**
5. (선택) `render_offset_mm` 을 `load()` 가 실어 오고, `cmd_s3` 의 SC-64 출력에 `max_render_offset_mm` 을 함께 싣는다 — architect R10 후속 §4 가 요구한 **귀속 가능성**의 실체. 이것이 없으면 열을 만들어 놓고 읽지 않는 것이 된다.

**⚠ 5번이 빠지면 새 열은 "존재하지만 아무도 보지 않는 열"이 된다.** 리더 지시는 열 추가까지였으나, architect 가 그 열을 요구한 이유(1.5 m 초과 시 귀속)는 판정 산출물에 실려야 달성된다. **계약 외 제안으로 표시하고, 리더 승인 없이는 넣지 않는다.**

---

## 3. 계약 외 발견 — **같은 헤더의 생산자가 셋이다. 지시는 둘만 말했다**

```
client/Assets/_Project/Scripts/Greybox/ObserverCsv.cs:52-53   ObserverCsvRow.Header   (client)
tests/e2e/two_client_view.py:31-34                            COLUMNS                 (qa)
tools/bots/src/snapshot.rs:25-26                              SNAPSHOT_CSV_HEADER     (qa — tools/bots/ 는 qa 소유)
```

`tools/bots/tests/snapshot_faults.rs:337` 이 이 헤더를 **동결하는 테스트**를 들고 있다(*"실수로 바꾸면 SC-63 을 잴 수 없고 1 mm 차이가 조용히 사라진다"*).

- 계약 §0.11 과 SC-64 가 **봇을 B 로 대체하는 경로**를 허용한다(SC-64 한정). 그 경로를 쓰면 봇 CSV 를 `two_client_view.py` 가 읽는데, **열을 둘만 고치면 봇 CSV 가 `NotImplementedYet` 으로 거부된다.**
- 봇에는 렌더 평활화가 없으므로 `render_offset_mm`·`render_offset_deg` 를 **상수 0** 으로 쓰면 된다(타 함선 행과 같은 규칙).
- **이것도 이름 확정 대기다.** 이름이 오면 4번(위 §2.3)을 같은 변경에 포함한다. `cargo test -p starfall-bots` 로 `snapshot_faults.rs` 동결 테스트를 함께 갱신·실행한다.

**리더에게**: 열 변경 지시를 **client·qa 2인이 아니라 3개 파일**로 잡아 달라. 한 파일만 남으면 봇 대체 경로가 조용히 죽는다 — 이 프로젝트에서 반복된 경계면 결함의 형태 그대로다.

---

## 4. 작업 3 — 블록 7 선후 조건 (절차서에 박는다)

> ### 블록 7 실행 전 게이트 — **셋이 전부 참일 때만 돈다**
>
> 1. **`ObserverCsv.cs` 의 `Header` 에 새 열이 들어갔다** (client R23 완료 보고).
> 2. **`two_client_view.py` 의 `COLUMNS` 에 같은 이름·순서로 들어갔다** (qa).
> 3. **`tools/bots/src/snapshot.rs` 의 `SNAPSHOT_CSV_HEADER` 에도 들어갔다** (qa) — 봇을 B 로 쓸 경우에만 필수지만, 어긋난 채 두면 다음 사람이 밟는다.
>
> **게이트 확인 명령 (실행하고 출력을 증거에 남긴다):**
>
> ```
> python tests/e2e/two_client_view.py selftest
> ```
>
> - `header_guard` 가 **5/5 기대대로**이고 `columns_count` 가 **새 열 수**여야 한다.
> - `columns_count` 가 옛 수 그대로면 **qa 쪽이 안 들어간 것**이다. 돌리지 마라.
> - C# 쪽이 안 들어갔으면 실제 CSV 를 읽는 순간 `NotImplementedYet` 으로 **즉시 죽는다** — 조용히 통과하지 않는다(증거 01 이 그 보장이다).
>
> **먼저 돌면 무엇이 잘못되는가:** 한쪽만 고친 상태면 블록 7 은 한 줄도 못 읽는다(최선의 경우). **두 쪽 다 안 고친 상태면 더 나쁘다 — 돌아가고, 숫자가 나오고, 그 숫자는 R22 이전 층(시뮬 vs 화면)의 값이다.** SC-64 의 2 m 예산을 그 층으로 재면 이름 없는 양을 재는 것이고, 통과해도 무의미하고 실패해도 귀속할 수 없다.
>
> **블록 7 실행 시 리포트 필수 기재 (자명 통과 방지):**
>
> | 적을 것 | 왜 |
> |---|---|
> | `pairs_compared` | 0 이면 SC-61/62/63 은 전부 `미검증(표본 없음)` 이지 PASS 가 아니다 |
> | `s3` 의 `SC-64.samples` · `SC-65.samples` | 둘 다 `미검증(표본 없음)` 을 낼 수 있고, 그것은 조종을 안 한 것이다 |
> | SC-65 표본의 `speed_mps` 최소값 | `--moving-speed-mps 100` 문턱 위에서 **실제로** 순항했는가 |
> | `max_render_offset_mm` (§2.3-5 를 넣는 경우) | SC-64 가 1.5 m 를 넘었을 때 귀속의 유일한 근거 |
> | B 가 **봇이 아니라 두 번째 Unity 세션**임을 명시 | 봇이면 SC-65 는 부호가 뒤집혀 `미검증(환경, E8)` 이다(계약 §0.11) |
> | **SC-63 은 §1 판정에 따라 FAIL 로 예측된다** | 나오는 FAIL 을 "새 회귀"로 오독하지 않기 위해 |

---

## 5. 요약

| 작업 | 상태 | 증거 |
|---|---|---|
| 작업 1 — `COLUMNS` 동기화 | **대기 (준비 완료)** — 이름·순서 미수령. 양성 대조는 넣고 실행했다 | 증거 01·04 |
| 작업 2 — SC-63 판정 입력 확인 | **확정: 이 CSV 다.** SC-63 은 평활화 이전에 이미 깨져 있었고, 계약의 "항진명제" 근거가 거짓이다 | 증거 02·03 |
| 작업 3 — 블록 7 선후 기록 | **완료** (§4) | — |
| 계약 외 발견 | 같은 헤더의 생산자가 **3개** (`tools/bots/src/snapshot.rs`) | §3 |

**FAIL 로 예측되는 항목: SC-63** (블록 7 실행 전 정적 판정, §1.3).
**미검증(실행 대기): SC-61·62·64·65** — 블록 7 게이트(§4) 미충족.
