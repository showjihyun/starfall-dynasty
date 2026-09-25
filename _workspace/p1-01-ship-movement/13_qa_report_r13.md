# QA 평가 리포트 r13 — 블록 7 판정 · 두 관측의 원인 규명 · 블록 10 실행

- 라운드: R17 (리더 지시 4건)
- 일시: 2026-09-25 KST
- 증거: `_workspace/p1-01-ship-movement/evidence/R17-block10/`
  (블록 7 원본은 `evidence/R16-block7/`, 나는 읽기만 했다)
- 소유 파일만 고쳤다: `tests/e2e/two_client_view.py` **주석 문자열 한 곳**(§3.4).
  C#·`contracts/`·`data/`·골든은 한 줄도 건드리지 않았다. 커밋하지 않았다.
- 서버: `stdin 'shutdown'` 으로 정상 종료했다(§5). 하드 킬 없음. `docker compose` 손대지 않음.

---

## §7a — 실행해 확인한 것과 읽어 추론한 것

| 사실 | 어떻게 |
|---|---|
| **리더의 네 verdict 는 옳다** — 내가 다시 돌린 `compare`·`s3` 의 JSON 이 리더의 것과 **키·값 전부 동일**(직렬화 바이트까지 1954 / 1219 로 같다) | **실행**: `evidence/R17-block10/qa-rerun/compare-qa.json` · `s3-qa.json`, 비교 결과 `identical=True` |
| **그 수가 도구와 독립으로도 나온다** — 도구를 쓰지 않고 CSV 를 직접 읽어 SC-61·62·64·65 의 다섯 수를 재계산했고 **소수 셋째 자리까지 일치** | **실행**: §1.2 의 독립 재계산 |
| **(iii) 방어가 살아 있다** — `--b-own-tolerance-m` 을 빼면 **exit 4** | **실행**: `qa-rerun/compare-no-tolerance.txt`, `exit=4` |
| **B 기록의 2초 구멍은 기록 단절이 아니다** — 같은 창의 **원격 함선 행은 간격이 정확히 2 tick, 결손 0** 이다 | **실행**: §3.1 |
| **원인은 Unity 메인 스레드 지연이다** — A·B 두 세션의 결손 tick 집합이 **완전히 동일**하고(5개 구멍, 같은 tick), 두 세션이 공유하는 것은 메인 스레드뿐이다. 서버는 배제된다(`tick_overrun_total=0`, `tick_body max=10.37 ms`, `messages_dropped_total=0`) | **실행**: §3.2 |
| **세션 종료 덤프(SC-56 (e))의 0 셋은 "오프셋을 세우지 않았다"가 아니다** — 밴드 4회는 **전부 자세(orientation) 밴드**였고, 카운터 셋은 **위치 오프셋만** 잰다. CSV 가 그것을 증명한다: **비-0 행 정확히 4개, 전부 `render_offset_deg` 만 비-0, `render_offset_mm` 은 0** | **실행**: §4.1 · **읽어서 확인**: `ObserverSession.cs:346-351, 479-483` |
| **블록 10 을 실행했다 — SC-63 PASS** | **실행**: 봇 2대 30초, `raw2` exit 0, `evidence/R17-block10/sc63.json` |
| **그 PASS 가 자명하지 않다** — 같은 실행 안의 1 mm 양성 대조가 **20건 전부 검출**, 실제 비교 쌍 **920건**, 최고 속도 **140.0 m/s**, `speed>0` tick **583개** | **실행**: §6 |
| 메인 스레드가 **왜** 2초 멈췄는지(GC · Editor 작업 · 그 밖)는 **특정하지 못했다** — Editor 로그에 그 구간의 stall 기록이 없다(Asset Refresh 는 12 ms·5 ms 둘뿐) | **실행**(로그 수색) — §3.3 에 미확인으로 남긴다 |

---

## 1. 작업 1 — 블록 7 판정 (SC-61 · SC-62 · SC-64 · SC-65)

### 1.1 판정표

| SC | 판정 | 핵심 수치 |
|----|------|----------|
| **SC-61** | **PASS** | 순변위 **5,125.307 m**, 경로 5,270.984 m, 역주행(>1 m) **0회**, 표본 **1,547**, span 3,092 tick, `max_possible@140 m/s` = 21,644 m |
| **SC-62** | **PASS** | 최대 이탈 **0.0 mm**(임계 50 mm), 표본 **1,486**, span **3,092 tick**, 같은 창 A 순변위 **5,125.307 m**, 넘긴 임계 **0.05**, `exceeds_one_lsb_1mm` **false** |
| **SC-64** | **PASS** | 두 화면 차이 최대 **0.175 m**(허용 2.0 m), 표본 **991**, `max_render_offset_mm` **0**, 귀속 **해당 없음(초과 없음)** |
| **SC-65** | **PASS** | 뒤쪽 평균 **27.156 m**(기대 28.0 ± 12.0), 부호 **뒤쪽**, 표본 **325** |
| ~~SC-63~~ | — | **이 블록에서 판정하지 않는다**(13차 개정). 블록 10 결과는 §6 |

판정 명령(레포 루트, `--b-own-tolerance-m 0.05` 명시):

```
python tests/e2e/two_client_view.py compare --a evidence/R16-block7/observer-a.csv \
  --b evidence/R16-block7/observer-b.csv --ship 01a0d76c-a53d-76a2-8c24-b9305de133da \
  --b-own-tolerance-m 0.05 --evidence <…>/compare-qa.json      → exit 0
python tests/e2e/two_client_view.py s3   --a … --b … --ship …  --evidence <…>/s3-qa.json → exit 0
```

`judgment_inputs` 7개: `0.05` · `400` · `0.9` · `100.0` · `140.0` · `1.05` · `1000`.

### 1.2 리더의 판독을 믿지 않고 두 번 확인했다

1. **같은 도구 재실행** — 리더의 `compare.json`·`s3.json` 과 내 재실행 결과를 정렬 직렬화해 비교:
   `compare: identical=True (1954 bytes)` · `s3: identical=True (1219 bytes)`.
2. **도구를 쓰지 않은 독립 재계산** — CSV 를 직접 파싱해 계산한 값:

| 양 | 도구 | 독립 계산 |
|---|---|---|
| SC-61 경로 / 순변위 / 역주행 | 5270.984 / 5125.307 / 0 | **5270.984 / 5125.307 / 0** |
| SC-62 최대 이탈(첫 표본 대비) | 0.0 mm | **0.0000 mm** (끝점 거리도 0.0000 mm) |
| SC-64 표본 / 최대 차이 | 991 / 0.175 m | **991 / 0.175 m** |
| SC-65 표본 / 뒤쪽 평균 | 325 / 27.156 m | **325 / 27.156 m** |

**리더 판독에 틀린 곳은 없다.**

### 1.3 §7a — 겨냥한 조건이 실제로 발생했는가

- **SC-65**: `speed ≥ 100 m/s` 표본 **325행**. **전 표본의 `speed_mps` 최소 = 100.5, 최대 = 140.0**
  (`samples_head` 의 101.5 는 앞 5행일 뿐이라 내가 325행 전체에서 다시 쟀다). 문턱 위에서 실제로 순항했다.
  along-track 값은 **325행 전부 음수(= 뒤쪽)**, 범위 **19.775 ~ 28.001 m**.
  **평균 27.156 이 28.0 보다 낮은 이유는 표본이 가속 구간(100.5 → 140 m/s)을 포함하기 때문**이고,
  140 m/s 순항 구간의 값은 **28.001 m** 로 설계값 `200 ms × 140 m/s = 28 m` 를 그대로 재현한다.
- **SC-62 (ii) 대조**: A 순변위 **5,125 m** ≥ 100 m. A 가 안 날았으면 `미검증(대조 없음)` 이었다.
- **SC-62 (i) 표집**: 1,486행 / 스팬 3,092 tick / 커버리지 **1.0** (요구 400 tick · 0.9).
- **SC-62 (iii)**: 임계를 **인자로** 넘겼다. **빼고 돌리면 exit 4** 로 죽는 것을 같은 라운드에서 실행해 확인했다.
- **SC-64**: `speed < 1 m/s` 표본 991행. 0이면 `미검증(표본 없음)` 이었다.
- **SC-61**: `backward_steps_over_1m = 0` 이 자명하지 않은 이유 — 같은 도구가 되돌아가는 입력에서
  `SC-61_verdict = FAIL` 을 내는 것이 selftest 음성 대조로 이 라운드에도 확인됐다(`selftest.json`).

### 1.4 필수 기재표(절차서 §6)

| 항목 | 값 |
|---|---|
| `pairs_compared` | **2,972** (0 아님 → `미검증(표본 없음)` 아님) |
| `SC-64.samples` / `SC-65.samples` | **991 / 325** |
| SC-65 표본의 `speed_mps` **최소값** | **100.5 m/s** (전 325행 실측; `samples_head` 만 보면 101.5) |
| `max_render_offset_mm` / `render_offset_attribution` | **0** / `해당 없음 (초과 없음)` |
| **B 는 봇이 아니라 두 번째 Observer 세션이다** | Console: `starfall.two_session: booting A(subject=…0001) and B(subject=…0002)` · `starfall.observer.A: session ready, actor_id=…0001` · `starfall.observer.B: session ready, actor_id=…0002`. 서버 로그도 같은 시각 두 세션(actor …0001 / …0002). `SUPERSEDED`·close 4001 **없음**. **→ SC-65 의 부호 조건이 성립한다**(계약 §0.11) |
| `render_offset_mm` 이 실제로 비-0 인가 | **아니다 — 전 행 0.** 네 갈래 판정은 §4 |
| SC-62 의 다섯 값 | 최대 이탈 **0.0 mm** · 표본 **1,486** · 스팬 **3,092 tick** · A 순변위 **5,125.307 m** · 임계 **0.05** |
| **`recorded_row_gaps_ticks`** | `max_gap_between_a_ship_rows = 2` · **`max_gap_between_b_own_rows = 40`** · `expected_gap_ticks = 2`. **판정은 §3** |
| SC-62 방어 (i)(ii)(iii) | `defense_i_sampling.ok = true` · `defense_ii_contrast.ok = true` · `tolerance_m_passed_in = 0.05` |
| ~~SC-63 verdict~~ | 블록 7 리포트에 적지 않았다 |

---

## 2. 리더가 틀린 곳

**네 verdict 에는 없다.** 다만 §3 과 §4 에서 리더가 미확인으로 남긴 두 관측은,
**리더가 인용한 도구의 설명문 자체가 틀렸기 때문에** 미확인으로 남았다(§3.4).

---

## 3. 작업 2(a) — B 기록의 2초 구멍: **기록 단절이 아니다. Unity 메인 스레드 지연이다**

### 3.1 결정적 관측 — 같은 창의 원격 행에는 구멍이 **하나도** 없다

`observer-b.csv` 를 함선별로 갈라 tick 간격을 전수 조사했다:

| 파일 | 행 계열 | 행 수 | 스팬 | 최대 간격 | 간격 분포 |
|---|---|---|---|---|---|
| `observer-b.csv` | **A 의 함선**(원격 = 보간 층) | **1,547** | 3,092 | **2** | `{2: 1546}` — **결손 0** |
| `observer-b.csv` | **B 자기 함선**(예측+오프셋 층) | 1,486 | 3,092 | **40** | `{2:1480, 18:1, 24:2, 26:1, 40:1}` |
| `observer-a.csv` | **B 의 함선**(원격) | **1,547** | 3,092 | **2** | `{2: 1546}` |
| `observer-a.csv` | **A 자기 함선** | 1,486 | 3,092 | **40** | 위와 **동일** |

**`1547 = 3092/2 + 1` 이다 — 원격 행은 스냅샷 tick 을 하나도 빠뜨리지 않았다.**
CSV 파일이 끊겼다면 원격 행도 같이 끊긴다. **끊기지 않았다. 따라서 기록 단절이 아니다.**

### 3.2 왜 자기 함선 행만 비는가 — **설계상 그렇다**

`ObserverSession` 은 두 계열을 **다른 경로로** 쓴다:

- **원격 행**: `OnWorldSnapshot()` 안에서 **스냅샷 메시지 1건당 1행**
  (`ObserverSession.cs:289`).
- **자기 함선 행**: F-2/H-9 배치 수정 이후 **`Update()` 당 최대 1행**. 한 프레임에 드레인된
  스냅샷 중 **가장 높은 tick 하나**만 `ApplyPendingRebase()` 가 재조정하고 기록한다
  (`ObserverSession.cs:262-271` 의 주석이 그 이유를 적고 있다 — 오래된 스냅샷이 새 것을
  되감아 CSV 에 새기는 결함을 막으려고 일부러 이렇게 만들었다).

⇒ **한 `Update()` 에 스냅샷이 N 개 들어오면 원격 행은 N 개, 자기 함선 행은 1 개다.**
결손 행 수도 정확히 맞는다: `(40/2−1)+(18/2−1)+(24/2−1)×2+(26/2−1) = 19+8+11+11+12 = 61`,
그리고 `1547 − 1486 = 61`.

### 3.3 그러면 왜 한 프레임에 스냅샷 20개가 들어왔는가

**후보 셋 중 둘이 배제된다.**

| 후보 | 판정 | 근거 |
|---|---|---|
| **서버가 2초 멈췄다가 몰아 보냈다** | **배제** | 서버 통계: `tick_overrun_total = 0`, `tick_body_us.max = 10,370 µs`(문턱 50,000), `snapshot_build_us.max = 9,743 µs`, `messages_dropped_total = 0`, `messages_enqueued_all == messages_written_all = 44,190` |
| **한쪽 소켓·전송 스레드 문제** | **배제** | **A 와 B 의 결손 tick 집합이 완전히 동일하다**: 둘 다 `1565732(40) · 1565812(18) · 1565846(24) · 1565898(26) · 1565930(24)`. 소켓·전송·CSV 라이터는 세션마다 독립이다. **두 세션이 공유하는 것은 Unity 메인 스레드(Update 루프) 하나뿐이다** |
| **Unity 메인 스레드가 멈췄다** | **원인** | 위 둘의 소거 + §3.2 의 기전. 다섯 번, 각 **2.0 · 1.3 · 1.2 · 1.2 · 0.9 초** |

**포커스 상실이 아니라는 리더의 말은 옳다**(`runInBackground: 1`). 그리고 포커스 상실이었다면
**원격 행도 같이 끊겼을 것**이므로, 관측 자체가 그 가설을 배제한다.

**멈춘 이유까지는 특정하지 못했다.** Editor 로그의 해당 구간에 stall 기록이 없다
(Asset Pipeline Refresh 는 12 ms·5 ms 둘뿐, 도메인 리로드 없음). GC·Editor 내부 작업이 남은
후보이고 **프레임 타이밍 계측이 없으면 로그만으로는 갈리지 않는다.** 미확인으로 남긴다.

### 3.4 판정에 미치는 영향 — **없다. 그리고 그것을 확인했다**

다섯 구멍은 **전부 tick 1565732~1565954 (세션 t+90.3 s ~ t+100.2 s)** 에 몰려 있고,
**그 구간의 A 속도는 전부 0.0 m/s** 다 — 비행이 끝난 **정지 유지 구간**이다.

- **SC-61 · SC-65 는 영향이 0** — 두 항목의 표본은 원격 행(결손 0)과 `speed ≥ 100 m/s` 구간에서만 나온다.
- **SC-62 · SC-64 는 표본이 61행 적어졌을 뿐**이고, (i) 의 커버리지는 여전히 1.0 이다.

**계약 (i) 이 이것을 통과시킨다는 전임자의 경고는 유효하다** — 다만 이번 건은
**(i) 이 놓친 진짜 단절이 아니라, (i) 이 통과시켜야 마땅한 정상 동작**이었다.

**도구의 설명문이 틀려 있었고 고쳤다**(`two_client_view.py:298-303`).
옛 문장은 *"이 값이 2 보다 크게 크면 창 안쪽에서 기록이 끊겼다 — `runInBackground: 0` 이라…"* 였다.
**두 문제가 있다**: (1) 두 값은 서로 다른 기록 층인데 같은 잣대로 읽으라고 시킨다.
(2) `runInBackground` 는 이미 `1` 로 바뀌었다. 새 문장은 **어느 값이 기록 연속성의 지표인지
(원격 행 간격)** 와 **자기 함선 행이 `Update()` 당 1행인 이유**를 적는다.
**숫자·verdict 는 한 곳도 바뀌지 않았다**(주석 제거 후 JSON 동일 확인). selftest exit 0.

---

## 4. 작업 2(b) — SC-56 (e) 덤프의 0 셋: **네 갈래 어디에도 없다. 다섯째 경우다**

### 4.1 무엇이 일어났는가

```
A: render_smooth_band_total=4  nonzero_frames=0  max=0.0000 (n=109960)  decay_frames=0
```

전임자의 표는 `band_total > 0 ∧ nonzero_frames == 0` 을 **"client 가 오프셋을 세우지 않았다 → FAIL"**
로 보낸다. **그 처분은 이번 건에 틀렸다.** CSV 가 반증한다:

```
observer-a.csv 의 비-0 행 — 정확히 4행, 전부 render_offset_deg 만 비-0
  tick 1565830  render_offset_mm=0  render_offset_deg=0.0001
  tick 1565870  render_offset_mm=0  render_offset_deg=1.4359
  tick 1565924  render_offset_mm=0  render_offset_deg=0.5749
  tick 1565954  render_offset_mm=0  render_offset_deg=0.9280
```

**4행 = `band_total` 4.** 오프셋은 **세워졌다. 자세 축에만.**

코드가 그대로 말한다:

- `ObserverSession.cs:346-349` — 밴드 계수는 **위치 OR 자세**:
  `if (IsSmoothed(positionBand) || IsSmoothed(orientationBand)) _smoothedReconcileTotal++;`
- `ObserverSession.cs:474-483` — 나머지 **세 카운터는 위치 오프셋만** 잰다:
  `offsetAfterDecayM = _renderPositionOffset.Length()` → `max` · `nonzero_frames` · `decay_frames`.
  **자세 오프셋에는 카운터가 하나도 없다.**

⇒ **이번 세션의 재조정 4건은 위치 오차가 (mm 양자화 아래로) 0 이고 자세 오차만 평활화 밴드에 들었다.**
`band_total=4, 나머지 0` 은 **정확히 그 상태의 정상 출력**이다. 결함이 아니다.

**같은 로그의 더 앞 세션에도 같은 형태가 이미 있다**(`Editor-session.log:779`:
`observer.B: band_total=1, nonzero=0, decay=0`) — 새 회귀가 아니다.
그리고 리더가 인용한 정상값(`:731` — band 43 / nonzero 1219 / max 0.6330 / decay 1203)이
**위치 경로가 동작한다**는 증거다.

### 4.2 SC-56 (e) 판정에 주는 영향

**블록 7 은 SC-56 (e) ②③④ 에 대해 아무 증거도 내지 못했다.**
이 세션은 **위치 평활화 경로를 한 번도 밟지 않았다** — 그러므로 `render_offset_mm` 열은
이 세션에서 **아무것도 증명하지 않는다**(절차서 §6.1 의 첫 행이 요구하는 문장이다).

**그리고 그 사실이 SC-64 의 읽기를 바꾼다**: SC-64 의 `max_render_offset_mm = 0` 은
"평활화가 작다"가 아니라 **"이 세션에 위치 평활화가 없었다"** 는 뜻이다.
차이 0.175 m 는 전부 예측 vs 보간의 차이이고 **평활화 오프셋이 섞이지 않은 값**이다 —
SC-64 의 PASS 는 그대로 유효하되, **평활화가 SC-64 예산을 얼마나 먹는지는 이 블록이 답하지 않는다.**
그 답은 `:731` 같은 세션(위치 오프셋 최대 0.6330 m = 2 m 예산의 **32 %**)에서 와야 한다.

**SC-56 (e) verdict 는 이 리포트에서 내지 않는다** — 블록 7 의 항목이 아니고, 이 세션은 입력을 못 만들었다.

### 4.3 client 에게 — 수정 요청 1건 (계측, 판정 아님)

**파일**: `client/Assets/_Project/Scripts/Greybox/ObserverSession.cs:346-349, 474-483`
**문제**: `_smoothedReconcileTotal` 은 위치 **또는** 자세 밴드를 세고, `_renderOffsetNonZeroFrameTotal` ·
`_renderOffsetMaxM` · `_renderOffsetDecayFrameTotal` 은 **위치만** 잰다. 두 지표의 축이 다르다.
**결과**: 자세만 평활화된 세션이 절차서 §6.1 표에서 **"client 가 오프셋을 세우지 않았다 (FAIL)"**
로 읽힌다. 실제로 이번 라운드에 리더가 그 표 앞에서 멈췄고, 앞선 세션(`:779`)에서도 같은 형태가 나왔다.
**기대 동작**: 자세 오프셋에도 같은 셋을 붙이거나(`render_orientation_offset_nonzero_frames_total` ·
`..._max_deg` · `..._decay_frames_total`), 최소한 밴드 계수를 **축별로 나눈다**
(`band_position_total` / `band_orientation_total`).
**재현**: `evidence/R16-block7/Editor-session.log:1786` + `observer-a.csv` 의 비-0 4행(§4.1).
**판정에 걸리는가**: 아니다. SC-56 (e) 의 **판정 불능**을 만들 뿐이다 — 그래서 고쳐야 한다.

### 4.4 그 수정의 후속 — **컴파일 확인. 테스트는 아직 미검증(환경, E10)**

client 가 같은 날 (a) 방식으로 고쳤다(`ObserverSession.cs` + **같은 결함이 있던 `GreyboxSession.cs`**,
자세 축 트리플렛 신설 · `OnSessionReady` 리셋 · 세션 종료 로그 · Greybox 는 HUD 까지).
`PeriodicStatusLog` 에 남은 같은 결함은 범위 밖으로 두고 `03_client_impl.md` R24 절에 기록했다 —
**내 리포트가 이 건을 "판정에 안 걸리는 계측 결함"으로 분류했으므로 그 판단에 동의한다.**

**Unity Editor 가 PID 19348 으로 점유돼 `unity test` 를 돌릴 수 없다**(내 점유가 아니다 — 나는 Editor 를
연 적이 없다). 그래서 **컴파일만** Editor 없이 확인했다. Unity 가 생성한 csproj 를 그대로 썼다:

```
cd client
dotnet build Starfall.Greybox.csproj        -t:Rebuild  → 경고 0 / 오류 0  (exit 0)
dotnet build Starfall.Tests.EditMode.csproj -t:Rebuild  → 경고 0 / 오류 0  (exit 0)
```

`netstandard2.1` · `LangVersion 9.0` · HintPath 293개(= Unity 가 쓰는 참조 DLL 그대로).
`-t:Rebuild` 이므로 증분 no-op 이 아니다. `client/*.csproj` 는 이미 `.gitignore` 대상이고,
스크래치 파일은 전부 지웠다(`git status` 에 client 변경 셋 외 없음).

**§7a — 이 초록불이 실제로 무언가를 쟀는가 (단언 둘)**

1. **산출물에 새 심볼이 들어 있다.** DLL 을 지우고 다시 빌드해 `client/Temp/bin/Debug/Starfall.Greybox.dll`
   이 **내 빌드로 재생성된 것**을 mtime 으로 확인한 뒤(16:46:22), 바이트를 뒤져
   `render_offset_orientation_nonzero_frames_total` · `render_offset_max_deg` ·
   `render_offset_orientation_decay_frames_total` **셋 다 존재**. 옛 DLL 을 보고 통과를 선언한 것이 아니다.
2. **이 검사는 실패할 수 있다(음성 대조).** 같은 csproj 를 복사해 깨진 파일 하나를 `Compile Include` 에
   더해 돌리니 **`error CS1061` · exit 1**. 없으면 "무엇을 먹여도 0 errors 를 내는 빌드"와 구분되지 않는다.

**이것이 답하지 않는 것 — `unity test` 를 대신하지 않는다**

- **테스트를 한 줄도 실행하지 않았다.** `PeriodicStatusLogTests.cs:86-88` 의 통과 여부는 **모른다.**
  (읽어서 추론한 것: client 가 `PeriodicStatusLog` 를 건드리지 않았고 단언이 `Does.Contain` 이므로
  종료 로그가 길어진 것은 영향이 없어야 한다. **추론이지 실행이 아니다.**)
- asmdef 수준 검증 · 도메인 리로드 · 런타임 카운터 거동은 보지 않았다.

⇒ **컴파일 위험은 해소됐다**(§4.3 을 낼 때의 "낮다"는 추정이 실측으로 바뀌었다).
**계측 수정 자체는 `미검증(환경, E10)` 이고 PASS 가 아니다.** Editor 가 풀려 client 가 `unity test` 를
돌리면 이 절을 갱신한다.

**나머지 두 문장 수정도 확인했다**(내가 §4.3 이후에 제기한 것):
`ObserverSession.cs:209-211` 의 로그 줄 주석이 *"Compare this triplet"* → *"Read this triplet's
**PRESENCE**(nonzero vs zero) … do **NOT** compare their magnitudes"* 로 바뀌어 필드 선언부의 금지
문장과 방향이 맞다. denormal 꼬리 단서(감쇠 꼬리는 후속 재조정이 없을 때의 것이고, 실세션에서는
비평활 밴드가 offset 을 Zero/Identity 로 덮어써 도달하지 않는다 — 실측 `1219/26683 = 4.6 %`)가
**두 파일 필드 선언부 각각에** 들어갔다.

---

## 5. 작업 4 — 서버 정상 종료

```
echo "shutdown" > evidence/R17-block10/ctl.fifo
```

```
07:31:41 INFO starfall_game_server: stdin 'shutdown' 수신 — graceful shutdown 시작
07:31:41 INFO starfall_gateway::runtime: tick 루프 종료 — SERVER_SHUTDOWN 스윕 후 영속화 flush
                                        tick=1576736 sessions_closed=0 pending_batches=0
07:31:41 INFO starfall_persistence: 영속화 태스크 종료 — 남은 배치 없음
07:31:41 INFO starfall_game_server: starfall game-server 정상 종료 — 마지막 tick 까지 커밋 완료 tick=1576735
```

종료 후 `curl /healthz` 무응답(= 내려갔다). FIFO 를 지웠다. **하드 킬 없음.**
증거: `evidence/R17-block10/server-stdout.log` · `server-stats-before.json` · `server-stats-after.json`.

---

## 6. 작업 3 — 블록 10 실행 (SC-63)

| SC | 판정 | 근거 |
|----|------|------|
| **SC-63** | **PASS** | `mismatching_pairs = 0` / **920 쌍** / 4척 / 양성 대조 20 |

### 6.1 실행

**`run_block.py` 는 쓰지 않았다**(`EVIDENCE_ROOT` 가 p0-02 로 박혀 있다). 직접 돌렸다:

```
cd tools/bots
STARFALL_DEV_AUTH_SECRET=<.env> cargo run --release -- run \
  --scenario e --bots 2 --duration 30 --send-hz 20 \
  --out ../../_workspace/p1-01-ship-movement/evidence/R17-block10/bots
→ exit 0.  GATES one_to_one=true results_partition=true accepted_exercised=true (accepted 1001) all_ok=true

python tests/e2e/two_client_view.py raw2 \
  --bot-out-dir _workspace/p1-01-ship-movement/evidence/R17-block10/bots \
  --evidence  _workspace/p1-01-ship-movement/evidence/R17-block10/sc63.json
→ exit 0
```

**`--send-hz` 는 명시적으로 `20` 을 넘겼다.** 이번 라운드에 기본값이 2 Hz → 20 Hz 로 바뀌었으므로
블록 10 의 수가 기본값 변경에 묶이지 않게 인자로 고정했다. 봇 출력 첫 줄이
`send_hz=[20.0]` 으로 에코하고 `summary.json` 의 `send_hz: [20.0]` 이 증거에 남는다.
**블록 10 판정 자체는 이 값에 민감하지 않다** — SC-63 (b) 가 요구하는 것은 `speed > 0` 인 tick 이
1개 이상이고, 두 봇 모두 **최고 140.0 m/s** 까지 올라갔다.

### 6.2 방어 6절 — 전부 만족

| 절 | 관측 | 만족 |
|---|---|---|
| **(a)** 표집 | `pairs_compared = 920`, **4척 전부 쌍 > 0** (`…81ac: 294`, `…701e: 294`, `…f490: 166`, `…514d: 166`), `ships_without_pairs = []`. `required_ships` = `summary.json` 의 `controlled_ship_id` 둘 | ✔ |
| **(b)** 운동 | `max_speed_mps = **140.0**`, `ticks_with_speed_gt_0 = **583**`. 정지 세션이 아니다 | ✔ |
| **(c)** 출처(값) | `nonzero_offset_rows = 0` | ✔ |
| **(c)** 출처(표기) | `render_offset_deg_literals = ["0"]`, `decimal_formatted_literals = []` — client 의 `F4`(`0.0000`) 지문이 **없다**. 봇 `snapshot.rs` 의 상수 `,0,0` 뿐 | ✔ |
| **(d)** 생산자 | `producer` 블록에 `stage=e-fly` · `url=ws://127.0.0.1:8080/ws` · `bots=2` · `seed=42` · `duration_secs=30` · CSV·summary 경로 · `sessions_json_present=true` | ✔ |
| **(e)** 양성 대조 | 같은 실행 안에서 `positive_control_one_mm_offset_mismatches = **20**` | ✔ |
| **(f)** 분리 | 독립 하위 명령 `raw2` 가 SC-63 verdict 를 단독으로 낸다. `compare` 에는 SC-63 계산이 없다 | ✔ |
| **P1** | `summary.json` 동반 — Unity 는 이 파일을 쓰지 않는다 | ✔ |
| **P2** | CSV 의 `observer_actor_id` 집합 `{…b010-…0000, …b010-…0001}` == `summary.snapshots_per_bot` 의 집합 | ✔ |

### 6.3 §7a — 이 PASS 가 무엇을 재고 무엇을 못 쟀는가

- **겨냥한 조건이 발생했다**: 두 봇이 **같은 성계에 동시 접속**해 서로를 봤다
  (`max_ships_seen = 4`, 각 봇 `snapshots_received` 318 / 320, `controlled_ship_missing = 0`),
  **둘 다 날았다**(`own_displacement_m` 3,899 m / 4,062 m, `own_speed_max_mps` 140.0).
  두 봇이 **정지해 있었다면** (b) 가 `미검증(표본 없음)` 으로 닫혔을 것이다.
- **검출기가 살아 있다**: 같은 실행의 1 mm 양성 대조가 **20/20** 을 잡았다. 이 수가 0 이면
  `mismatching_pairs = 0` 은 아무것도 뜻하지 않는다.
- **못 잰 것(정직하게)**: 실제 봇 데이터에 **진짜 불일치를 주입할 수단이 없다.**
  음성 대조는 selftest 의 합성 입력(`fail_on_1mm_mismatch = FAIL`)으로만 있다.
  **양성 대조가 계약 (e) 가 요구한 형태이고 그것은 충족했다.**
- **비교된 4척 중 2척은 봇이 아니다** — tick 1574596 이전에 이 서버에 붙었던 Unity 관측자 세션
  둘의 **LINGERING 함선**이다(`ships_lingering=2`, 서버 로그 07:29:39 의 actor …0001/…0002).
  두 봇이 **그 두 척에 대해서도 원시 정수 6개가 완전히 일치**했다(166 × 2 쌍, 불일치 0).
  이 항목이 잡으려는 것이 *"세션별 직렬화가 `ships` 배열을 다르게 만드는 서버 버그"* 이므로
  **표본이 늘어난 것이지 오염이 아니다.**

---

## 7. 게이트 재실행

| 방향 | 명령 | 결과 |
|---|---|---|
| 역방향 + 순방향 | `python tests/e2e/check_contract_items.py --contract 02_sprint_contract.md --report 13_qa_report_r13.md` | §7.1 |
| 출처 | `python tests/e2e/check_item_sources.py --contract 02_sprint_contract.md --tools-dir tests/e2e` | §7.1 |
| 도구 selftest | `python tests/e2e/two_client_view.py selftest` | **exit 0** (`evidence/R17-block10/selftest.json`) |

### 7.1 결과

| 게이트 | exit | 출력 |
|---|---|---|
| `check_contract_items.py` (순방향+역방향) | **0** | 계약 항목 **90** · **리포트가 판정표 행으로 다룬 항목 5** · 안 나온 항목 85(위반 아님) · **위반 없음** |
| `check_item_sources.py` (출처) | **3** | 계약 항목 90 · 도구가 지명된 항목 **85 / 90** · 검사한 도구 27 · **출처 위반 0** · 미지명 5건(`SC-10 · 28 · 30 · 32 · 74`) |
| `two_client_view.py selftest` | **0** | `verdict_separation_ok=true` · `block10_defenses_ok=true` · 음성 대조 `fail_on_1mm_mismatch=FAIL` |

**판정 행 파싱 수**: 정규식 `^\|\s*\*{0,2}SC-\d+` 에 걸린 **행 12개**, **고유 항목 5개**
(`SC-61 · 62 · 63 · 64 · 65`). 12행 중 5행만 verdict 행이고 나머지 7행은 §1.2·§1.4 의
관측·재계산 표 행이다 — **게이트는 고유 항목 집합으로 판정하므로 이 중복은 위반을 만들지 않는다.**

초안에는 §7a 표에 `| **SC-56 (e) …` 로 시작하는 줄이 있어 **고유 항목이 6개로 잡혔다**.
이 리포트는 **SC-56 의 verdict 를 내지 않으므로**(§4.2) 그 줄의 머리를
`세션 종료 덤프(SC-56 (e))의 …` 로 바꿔 판정 행에서 뺐다. 게이트는 SC-56 이 계약에 있으므로
**위반을 내지 않았지만**, 판정하지 않은 항목이 판정 수에 섞이는 것은 그 자체가 §7b 가 막으려는 형태다.

**출처 게이트의 exit 3 은 이 라운드가 만든 것이 아니다** — `미지명만`(출처 위반 0)이고,
다섯 항목(`SC-10 · 28 · 30 · 32 · 74`)은 계약 §7b 규칙 8 의 기존 만기 항목이다.
`raw2`·`compare`·`s3` 가 내는 `SC-61~65` 는 전부 지명 안에 있다.

---

## 8. 계약 외 발견

### 발견 1 — `recorded_row_gaps_ticks` 의 설명문이 두 기록 층을 같은 잣대로 읽으라고 시켰다

§3.4. **내 파일이므로 내가 고쳤다.** 이 문장 때문에 리더가 정상 동작을 "미확인 이상"으로 남겼다.
**값 자체는 옳았고 verdict 도 옳았다** — 틀린 것은 읽는 법이었다.

### 발견 2 — SC-56 (e) 계측의 축이 갈라져 있다 (§4.3)

client 수정 요청. 판정에는 걸리지 않지만 **SC-56 (e) 를 판정 불능으로 만든다.**

### 발견 3 — 절차서 §6.1 의 네 갈래 표에 다섯째 칸이 필요하다

현행 표는 `band > 0 ∧ nonzero == 0` 을 **FAIL** 로 보낸다. **이번 세션이 그 칸에 들어가는데 결함이 아니었다.**
표에 한 줄이 필요하다:

| 관측 | 원인 | 처분 |
|---|---|---|
| `band_total > 0` ∧ `nonzero_frames == 0` ∧ **CSV 의 `render_offset_deg` 가 비-0 인 행 수 == `band_total`** | **자세 축만 평활화됐다.** 카운터 셋은 위치만 잰다 | **결함이 아니다.** "이 세션은 **위치** 평활화 경로를 밟지 않았다"를 적는다. 그 위에서만 FAIL 로 보낸다 |

이 줄이 없으면 다음 사람이 같은 자리에서 client 에게 잘못된 FAIL 을 보낸다.
**절차서는 내 파일이 아니므로 고치지 않고 architect·리더에게 요청한다.**

---

## §4.5 — SC-56 (e) 계측 수정: **미검증 해제** (리더, 2026-09-25)

`§4.3`·`§4.4`가 `미검증(환경, E10 — Unity 점유)`로 둔 상태를 해제한다.
Editor가 닫혔고 **테스트가 실제로 돌았다.**

### 실행

```
unity test client --mode EditMode
→ total=323  passed=321  failed=0  skipped=2   exit 0
```

**client 실행과 리더 재실행이 일치한다**(리더가 별도로 한 번 더 돌렸다).
결함 표식 잔존 **0건**.

### §4.4가 "모른다"고 적었던 것 — 이제 안다

| | §4.4 시점 | 지금 |
|---|---|---|
| 컴파일 | 실측(0 errors, 음성 대조 포함) | 동일 |
| **테스트 실행** | **한 줄도 안 돌았다** | **돌았다** |
| `PeriodicStatusLogTests.cs:86-88` | **통과 여부 모름** — "안 건드렸으니 통과해야 한다"는 **추론** | **PASS 확인** (`--filter PeriodicStatusLogTests` → 12/12, `Format_ContainsKeyValueSubstringsForEveryField` = `Passed`) |

그 세 줄의 통과가 뜻하는 것(qa가 미리 지정한 대로): **세션 종료 로그 줄이 길어진 것이
기존 문자열 단언을 깨지 않았다.** 그냥 초록이 아니라 **무엇을 증명하는 초록인지**가
실행 전에 정해져 있었다.

### 총 개수 불변도 단언에 포함된다

`323` → `323`. 이번 수정은 테스트를 추가하지 않았으므로 **개수가 그대로여야 했고 그랬다.**
늘거나 줄었다면 그 자체가 확인할 거리였다(리더가 실행 전에 그렇게 지정했다).

### 남는 것

- `PeriodicStatusLog`에 **같은 축 분리 결함이 남아 있다** — client가 범위 밖으로 두되
  `03_client_impl.md` R24에 명시했다. 없는 것처럼 두지 않았다.
- 이 수정은 **SC-56 (e)를 판정 가능하게 만든 것**이지 판정한 것이 아니다.
  ②③④에 증거를 내려면 **위치 평활화가 실제로 발동한 세션**이 필요하고,
  블록 7 세션은 자세 밴드만 밟았다.
