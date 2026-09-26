# client의 스펙·ADR 검토 (p1-01)

- 대상: `docs/specs/p1-01-ship-movement.md`(agreed), ADR-0009·0010·0011·0012, `01_architect_tasks.md` C1~C6, `contracts/**` 신규 7종
- 작성: 2026-09-20, client
- **구현하지 않았다.** 생성기 확장은 스크래치 사본(`%TEMP%\...\scratchpad\p1\Gen2.cs`)에서만 했고 `tools/`·`client/`·`docs/`·`contracts/`는 한 글자도 고치지 않았다. 생성 출력은 전부 스크래치 경로.
- 환경: Unity 6000.6.1f1, .NET SDK 10.0.401, Newtonsoft 13.0.2, Unity 동봉 Mono(`MonoBleedingEdge`), Rust 1.x(레포 툴체인)

---

## 동의하는 결정

| 결정 | 근거 | 클라이언트 측 영향 |
|------|------|------------------|
| **와이어는 정수 양자화, 실수 없음** (ADR-0009 §2) | 왕복 테스트가 자명해지고 결정성 검증이 바이트 비교가 된다 | 생성기에 `number` 매핑을 열지 않아도 된다. 실측으로 확인: 신규 4 DTO의 물리량이 전부 `long`/`int`로 나왔다 |
| **왼손 Y-up +Z 전방, Unity 규약** (ADR-0009 §1) | 변환 계층이 부호 버그가 사는 곳인데, 그 계층을 아예 없앤다 | 렌더링에서 축 변환 코드가 0줄이 된다. 강하게 동의 |
| 축 의미를 스키마 문구·fixture·육안 **3중**으로 고정 | 1·2는 "일관"만 보장하고 통째로 뒤집힌 규약도 통과한다 | AC-14(a)가 부호 버그를 잡는 유일한 장치라는 판단에 동의. C6의 첫 산출물을 녹화로 잡겠다 |
| **초월함수 금지**, `sin(θ/2)`를 그대로 쓰고 감쇠는 선형 캡 (ADR-0010 §3) | `sin`·`exp`·`acos`만이 표준 미규정 구간이다 | 예측 코어가 서버와 같은 함수 집합만 쓴다. `turn_gain_deg_s_per_sin_half`라는 이름이 제약을 기억하는 것도 좋다 |
| **예측은 자기가 보낸 양자화 정수의 역변환으로** (I-36, ADR-0012 §2) | 원시 실수로 예측하면 버그 없이 상시 어긋난다 | 동의. 아래 §"태스크 계획"에서 이것을 **타입으로** 강제하겠다 — 예측 코어에 원시 실수가 들어갈 입구를 만들지 않는다 |
| **재조정은 즉시·예외 없이, 임계는 화면만 정한다** (ADR-0012 §4) | 예측과 확정을 섞은 제3의 상태를 만들지 않는다(I-35) | 이 구분이 이 ADR에서 가장 중요한 한 줄이다. 동의 |
| **무시 밴드 5 mm를 architect가 깐 것** | 양자화 한계 0.87 mm 위에서 보정이 영원히 도는 것을 막는다 | 실측 근거가 정확하다. `data/`에 `reconcile_ignore_threshold_m = 0.005`로 들어와 있는 것 확인 |
| **하드 스냅을 감추지 않고 계측한다. 로컬에서 0이어야 한다** | 0이어야 하는 값이라 회귀가 즉시 보인다 | AC-13(c)가 이 슬라이스에서 가장 값싼 고감도 센서다 |
| **타 함선은 예측하지 않고 200 ms 과거를 보간** (ADR-0012 §5) | 조작 없는 외삽은 금방 틀리고 되돌리는 보정이 또 필요하다 | 동의. slerp 누락 경고도 정확하다 — 다만 **그 검증 방법에 함정이 있다**(R6) |
| **`kind: data`는 DTO를 만들지 않고, 클라이언트는 `data/` 사본을 직접 읽는다** (§5.7) | 튜닝 테이블은 와이어를 왕복하지 않는다 | 실측 확인: 커버리지 스크립트가 `SyncTuning` 문자열만 찾으므로 로더 클래스 이름으로 충족된다 |
| **C1이 모든 클라이언트 작업의 앞** | 확장 전에는 어떤 DTO도 생성되지 않는다 | 실측으로 재확인했다(§C1). 순서에 이견 없음 |
| 입력은 절대 상태 + 마지막 것 승 + 이월 (ADR-0011 §4) | 덮어쓰기가 손실이 아니고, 거부가 안전해진다 | 클라이언트가 재전송 큐를 갖지 않아도 된다. p0-02의 I-23과 결이 같다 |

**designer 데이터 3파일이 이제 전부 스키마를 통과한다** — D-1·D-2·D-3이 반영된 것으로 보인다. 오프라인 검증기(로컬 `$ref` 레지스트리) 실행 결과 `sync-tuning.json` **0건**, `scout-s01.json` **0건**, `cradle.json` **0건**. §5.6의 "6건 실패"는 해소됐다. 내 C3 로더와 S2가 실제 `data/`로 바로 갈 수 있다.

---

## 수정 요청 (무엇을 / 왜 / 대안)

### R1. **[차단] AC-11(a)의 "유효 fixture 26건 왕복"은 만족할 수 없다**

- **무엇이 틀렸나**: 유효 fixture 26건 중 **6건이 데이터 타입**(`SHIP_CLASS` 2, `STAR_SYSTEM` 2, `SYNC_TUNING` 2)이고, **AC-10(d)가 바로 그 타입들의 `.cs`를 생성하지 않는다고 못박았다.** C# DTO가 없는 fixture는 왕복할 대상이 없다.
- **증거 (확장된 생성기로 생성한 DTO에 대해 유효 26건 전수 실행):**
  ```
  valid fixtures on disk: 26 | C# has a DTO for: 20 | no DTO: 6
    no DTO -> SHIP_CLASS/example-newtonian-bounds.json   (no envelope discriminator at all)
    no DTO -> SHIP_CLASS/example-scout.json
    no DTO -> STAR_SYSTEM/example-cradle.json
    no DTO -> STAR_SYSTEM/example-max-radius.json
    no DTO -> SYNC_TUNING/example-bounds.json
    no DTO -> SYNC_TUNING/example-p1-01-default.json
  round-tripped: 20, problems: 0
  ```
  데이터 fixture는 `command_type`/`message_type`/`event_type` **envelope 판별자 자체가 없다.** DTO를 만들었다 해도 디스패치 경로에 올릴 수 없다 — 이건 생성기 문제가 아니라 그 타입들이 메시지가 아니라는 사실이다.
- **요청**: AC-11(a)를 **"유효 fixture 26건을 로더가 발견하고, 그중 계약 메시지 20건이 `Strict` 왕복을 통과한다"**로. 두 숫자가 다 필요하다 — **26은 "fixture가 사라지지 않았다", 20은 "왕복을 빠뜨리지 않았다"**를 각각 지킨다. AC-11(c)의 가드도 같은 문장으로 맞춰 주기 바란다("26건 미만 발견하면 실패"는 그대로 유효하다).
- 그대로 두면 C2가 반드시 실패하거나, 구현자가 임의로 20으로 낮춰 "스펙에 없는 판단"을 하게 된다.

### R2. **[AC 문구 오류] AC-10(a)의 재현 메시지가 실제와 다르다**

- 스펙 §5.7과 AC-10(a)는 확장 전 실패가 `'array' is not supported` 또는 `kind 'data' is not supported yet`이라고 적었다. **실제 `contracts/` 트리로 현재 생성기를 돌리면 첫 실패는 그 둘이 아니다.**
  ```
  $ dotnet run tools/codegen/ContractsCodegen.cs -- --contracts contracts --out <scratch>
  codegen: contracts/messages/WORLD_SNAPSHOT.schema.json/properties/payload/properties/ships/maxItems:
           unsupported JSON Schema keyword. ...
  종료 코드 2
  ```
- architect는 `maxItems`가 없는 스크래치 계약으로 쟀을 것이다. 실제 캐스케이드는 **3단**이고, 하나 고칠 때마다 다음이 나온다(전부 실행 확인):
  | 고친 것 | 다음 실패 |
  |---|---|
  | (없음) | `.../ships/maxItems: unsupported JSON Schema keyword` |
  | `maxItems` allowlist | `.../ships/type: 'array' is not supported by the generator` |
  | array 지원 | `SHIP_CLASS: kind 'data' is not supported by the generator yet` |
- **요청**: AC-10(a)를 "셋 중 하나로 실패함을 보인다"로 넓히거나 `maxItems`를 첫 항목으로 추가. C1의 "먼저 쓸 실패 테스트"도 같이.

### R3. AC-10(e)의 "기존 6타입 생성물이 한 글자도 바뀌지 않았다"는 `ContractTypes.cs`를 포함하면 거짓이다

- `ContractTypes.cs`도 생성물이고, 신규 4타입이 레지스트리 맵에 들어가므로 **반드시 8줄 늘어난다.** 실측:
  ```
  타입별 6파일 (CommandResultMessage, PingReplyMessage, PingServerCommand,
                SessionClosedEvent, SessionOpenedEvent, SessionReadyMessage) → 전부 바이트 동일
  ContractTypes.cs → +8줄 (신규 4타입의 ByName 4행 + SchemaVersionByName 4행), 다른 변경 없음
  ```
- **요청**: "기존 6타입의 **타입별 파일 6개**가 바이트 동일하고, `ContractTypes.cs`의 변경은 신규 항목 추가 8줄뿐임을 보인다"로. 지금 문구대로면 통과 불가이거나, 구현자가 `ContractTypes.cs`를 예외로 빼면서 그 판단을 기록하지 않는다.

### R4. **[가장 실질적인 요청] `WORLD_SNAPSHOT`은 현재 `ContractDispatch` 경로를 타면 안 된다**

p0-02에서 만든 수신 경로는 `ContractJson.ReadObject(json)` → `JObject` 트리 → `ToObject(type, serializer)`다. 메시지가 300바이트였을 때는 무해했다. **16 KiB × 10 Hz에서는 그 트리가 순수한 낭비다.** Unity 동봉 Mono에서 31척 스냅샷으로 실측:

| 경로 | 1건당 | 10 Hz 환산 | 200건당 gen0 GC |
|------|---:|---:|---:|
| 1. `JObject` 트리만 | 0.401 ms | 4.01 ms/s | 7 |
| 2. **트리 + `ToObject` (현재 경로)** | **0.656 ms** | **6.56 ms/s** | **8** |
| 3. 문자열에서 직접 타입 역직렬화 | **0.265 ms** | 2.65 ms/s | **1** |
| 4. UTF-8 디코드만 (바닥값) | 0.025 ms | 0.25 ms/s | 0 |

- **CPU는 문제가 아니다**(0.66 ms를 100 ms마다 한 번). **문제는 가비지다**: 현재 경로는 직접 역직렬화보다 gen0 수집을 **8배** 유발한다. 트리를 만든 뒤 버리기 때문이다.
- **요청 (스펙 변경이 아니라 태스크 지시 한 줄)**: C4/C5 지시에 **"`WORLD_SNAPSHOT`은 `JObject` 트리를 거치지 않는다 — 판별자만 스트리밍으로 확인하고 타입으로 직접 역직렬화한다"**를 넣어 주기 바란다. 나머지 타입(초당 몇 건)은 기존 경로 그대로 둬도 된다.
- 이것을 지금 정하지 않으면 C4가 p0-02 경로를 그대로 재사용하고, AC-13(e)에서 숫자를 보고 나서 고치게 된다. 값이 이미 있으므로 지금 정하는 편이 싸다.
- 덧붙여 **측정된 스냅샷 크기는 31척 16,347 바이트**로 ADR-0011 §2의 산출(15,419 B)보다 **+6.0 %**다. AC-18(c)의 20 % 허용 안이고, 내 합성 값이 실제보다 자릿수가 길 수 있으니 **참고치**로만 봐 달라 — 정본은 봇 실측이다.

### R5. AC-13(e)의 단위가 "프레임당"이면 숫자가 의미를 잃는다

스냅샷은 10 Hz이고 렌더는 60~144 fps다. "프레임당 할당량"으로 재면 **6~14프레임 중 한 프레임에만 할당이 뜨고** 평균은 실제 스파이크를 감춘다. **요청**: "**스냅샷 1건당 할당**과 **초당 할당**을 적는다"로. (위 표가 이미 그 형태다.)

### R6. **AC-15(b)의 slerp 검증이 잘못된 구현도 통과시킨다**

C5의 "먼저 쓸 실패 테스트"가 **"스냅샷 2건 사이 *중간* 시각의 보간"**을 쓰도록 되어 있다. 문제는 **t = 0.5에서 slerp와 nlerp(선형 보간 후 정규화)가 정확히 같은 값을 낸다**는 것이다 — 중간점은 두 방법의 대칭점이다. 즉 **자세를 선형 보간하고 정규화만 한 구현이 이 테스트를 통과한다.**

자세 보간 누락이 이 클래스에서 가장 흔한 실수라는 architect 지적에 동의하는데, **그 지적을 지키라고 만든 테스트가 바로 그 실수의 사촌을 놓친다.**

- **요청**: C5의 첫 테스트를 **(a) 비대칭 지점(t = 0.25)** 과 **(b) 큰 각도차(두 스냅샷 자세가 60° 이상 벌어진 쌍)** 로 명시. 이 조건에서 nlerp는 slerp와 뚜렷하게 다르다(등속이 아니다). AC-15(b)의 문구에도 "중간이 아닌 지점에서 확인한다"를 한 줄 추가.
- 최대 각속도 225 °/s(= 3.75°/tick)에서 스냅샷 간격 2 tick이면 자세 차가 7.5°다. 60° 차이를 만들려면 **테스트 자산을 합성**해야 한다 — S6 산출물에 의존하지 말고 C5가 자체 fixture를 만들도록 지시해 주기 바란다.

### R7. AC-12(a)의 "모든 tick에서"는 비교 지점을 잘못 가리킨다

ADR-0012 §3의 1단계는 `보관한_예측상태[S].p − X.p`, 즉 **같은 tick의 두 값**을 비교한다. 비교가 가능한 지점은 **스냅샷이 도착한 시점(= `ack_input_seq`가 가리키는 tick)뿐**이고, 그 사이 tick에는 대조할 서버 값이 없다. AC-12(a)의 "모든 tick에서"는 매 tick 비교하는 것처럼 읽힌다.

- **요청**: "**각 스냅샷마다**(비교 지점 수를 출력에 적는다)"로. 검사 건수가 증거에 드러나는 이 프로젝트 규약과도 맞는다.

### R8. 경미 — `unity test`의 `--output` 디렉토리

AC-11의 명령이 `_workspace/p1-01-ship-movement/unity-tests/`에 쓰는데 그 디렉토리는 아직 없다. p0-02에서 나는 **항상 `mkdir -p`를 먼저 했기 때문에 CLI가 없는 디렉토리를 만들어 주는지는 확인한 적이 없다.** 추측으로 적지 않는다 — AC-11 옆에 `mkdir -p` 한 줄을 넣거나, 내가 C2에서 실측해 T10에 적겠다.

---

## C1 생성기 확장 범위 확인 (실행 증거)

### 확장 전 — 실패 캐스케이드 3단 (§R2에 표, 전부 실행 확인)

### 필요한 변경은 **정확히 3건이고, 그 3건이면 충분하다**

스크래치 사본에 아래 3건만 적용하니 **종료 코드 0으로 11파일이 생성됐다.**

1. **`maxItems`를 제약 allowlist에 추가** — 1줄. C# 출력에 영향 없음(`minItems`와 같은 취급).
2. **`type: "array"` + `items` → C# 배열.** `Normalize`에 `case "array"` 추가. 원소 스키마를 재귀 정규화하고, 원소가 객체면 그 중첩 클래스를 그대로 올려보낸다.
   - **함께 고쳐야 하는 곳이 하나 더 있다(architect 목록에 없던 것)**: `BuildProperties`가 `shape.Nested != null`이면 속성 타입을 `Nested.ClassName`으로 쓴다. 배열에서는 그게 틀리다 — 속성은 **배열**이고 중첩 클래스는 **원소**다. `CsType`이 있으면 그것을 우선하도록 한 줄 바꿔야 한다. 이걸 놓치면 `ships`가 `ShipState`(배열 아님)로 나온다.
3. **`kind is "rest" or "data"`를 예외 대신 `continue`** — 건너뛴 것을 stdout에 한 줄 남긴다(조용한 건너뛰기는 p0-01이 경계한 형태다).
   ```
   skipped SHIP_CLASS (kind 'data': no DTO is generated)
   skipped STAR_SYSTEM (kind 'data': no DTO is generated)
   skipped SYNC_TUNING (kind 'data': no DTO is generated)
   ```

### U-14 — 배열 원소가 중첩 클래스로 올바로 나오는가: **PASS**

```
full name            : Starfall.Contracts.Generated.WorldSnapshotMessage+WorldSnapshotPayload+ShipState
ships property type  : ...WorldSnapshotPayload+ShipState[]   isArray=True
ShipState 속성 수     : 17   (AC-10(c)의 17과 일치)
```

17개 전부와 그 C# 타입:

```
ship_id System.Guid Always      actor_id System.Guid Always      ship_class_id string Always
presence string Always
position_{x,y,z}_mm            long  Always      velocity_{x,y,z}_mm_s        int Always
orientation_{x,y,z,w}_micro    int   Always      angular_velocity_{x,y,z}_mdeg_s int Always
```

- 위치가 `long`, 나머지가 `int` — ADR-0009 §2의 언어 매핑 표와 일치한다(`PositionMm` ±1e12는 `int`에 안 들어간다).
- `presence`가 `string` — 닫힌 집합의 C# 매핑 규약대로.
- **`ShipState`가 `WorldSnapshotPayload` **안**에 들어간다**(`Message+Payload+ShipState`). 계약이 `$defs`의 형제로 둔 것과 중첩 깊이가 다르지만, `title`로 이름이 고정되므로 **AC-21(b)의 필드별 표에서 문제가 되지 않는다.** 다만 C# 쪽 참조 경로가 3단이 되므로 테스트 코드에 `using` 별칭을 둘 생각이다.

### 결정성·회귀 (AC-10(b)(e))

```
2회 연속 생성 → 11파일 sha256 목록 동일 (f273dbc5...)
--check → "--check: up to date (11 file(s))"  종료 코드 0
기존 타입별 6파일 → 전부 바이트 동일
ContractTypes.cs → 신규 4타입 8줄 추가만 (R3)
```

### 배열 원소 안의 검증이 실제로 동작하는가

`WORLD_SNAPSHOT/invalid/ship-missing-orientation-w.json`이 **C#에서 거부된다**:
```
Required property 'orientation_w_micro' not found in JSON.
```
§5.4가 이 항목을 둔 목적("배열 원소 안의 검증이 동작하는지")이 실측으로 충족됐다.

---

## 스펙 §5.4의 `C# (Strict)` 열 — 실측으로 채움

반례 **33건 전수**를 확장된 생성기의 DTO로 `Strict` 역직렬화했다. **C# 거부 17 / C# 통과 10 / C# 계층 없음 6.**

§5.4가 예측으로 적은 **11행 전부, 예측과 실측이 일치한다. 정정할 칸이 없다.**

| # | fixture | §5.4 예측 | **실측** | 일치 |
|---|---------|---------|--------|:---:|
| 1 | `SET_SHIP_CONTROL/invalid/position-field-injected.json` | 거부 | **거부** (`Could not find member 'position_x_mm' on SetShipControlPayload`) | O |
| 2 | `SET_SHIP_CONTROL/invalid/attitude-field-injected.json` | 거부 | **거부** (`... 'orientation_x_micro' ...`) | O |
| 3 | `SET_SHIP_CONTROL/invalid/thrust-above-range.json` | 감지 불가 | **통과** | O |
| 4 | `SET_SHIP_CONTROL/invalid/input-seq-zero.json` | 감지 불가 | **통과** | O |
| 5 | `WORLD_SNAPSHOT/invalid/ship-missing-orientation-w.json` | 거부 | **거부** (필수 필드) | O |
| 6 | `WORLD_SNAPSHOT/invalid/unknown-presence.json` | 감지 불가 | **통과** | O |
| 7 | `WORLD_SNAPSHOT/invalid/hard-radius-above-ceiling.json` | 감지 불가 | **통과** | O |
| 8 | `SHIP_SPAWNED/invalid/actor-id-null.json` | 거부 | **거부** (`{null} → System.Guid`) | O |
| 9 | `SHIP_SPAWNED/invalid/causation-id-null.json` | 거부 | **거부** | O |
| 10 | `SHIP_DESPAWNED/invalid/causation-id-null.json` | 거부 | **거부** | O |
| 11 | `SHIP_DESPAWNED/invalid/session-closed-is-not-a-despawn.json` | 감지 불가 | **통과** | O |

**부수 확인 — p0-02의 좁힘 수정이 `causation_id`에도 그대로 먹는다.** `SHIP_*`의 `causation_id` 좁힘이 프로젝트 첫 사례인데(스펙 §5.4 비고), 생성기를 손대지 않고 9·10행이 거부됐다. `actor_id` 때 고친 병합 규칙이 필드 이름과 무관하게 동작한다는 뜻이다.

**§5.4에 없는 나머지 22행도 전수 기록했다**(선행 슬라이스 표와 합쳐 C2가 상수로 박을 값): C# 책임 = 거부 기대 **17건**, "감지 불가" 기록 **10건**, C# 계층 없음(데이터 타입) **6건**. 17 + 10 + 6 = 33.

---

## U-15 — Unity `double`이 Rust와 비트 동일한가: **동일하다 (Mono/x64 Editor 기준)**

이 슬라이스의 가장 중요한 미확인 사실이므로 **검토 단계에서 먼저 쟀다.** AC-12를 기다리면 C3·C4를 다 만든 뒤에 설계가 뒤집힐 수 있다.

**방법**: ADR-0010 §2가 실제로 하는 산술만 뽑아 같은 순서로 두 언어에 옮겼다 — 쿼터니언 Hamilton 곱, 정규화, `rot(q,v)`, `|v|`(sqrt), semi-implicit Euler 갱신, 대각선 클램프, 감쇠 캡, 속도 상한, 하드 경계 투영. **300 tick** 돌린 뒤 모든 결과를 **raw IEEE-754 비트**로 출력했다. 일부러 이진 분수로 떨어지지 않는 초기값을 썼다.

- 클라이언트 쪽은 **Unity 동봉 컴파일러(`MonoBleedingEdge/bin/mcs`)로 빌드해 Unity 동봉 런타임(`MonoBleedingEdge/bin/mono.exe`)에서 실행**했다. .NET 8이 아니라 **클라이언트가 실제로 쓰는 런타임**이다.
- 서버 쪽은 스크래치 Cargo 프로젝트에서 `cargo run --release`.

**결과 — 비교한 15개 값 전부 비트 동일:**

```
                Rust               Mono(Unity)
PI          400921fb54442d18   400921fb54442d18
DEG2RAD     3f91df46a2529d39   3f91df46a2529d39
q.x0        3fc0d37c60ecb636   3fc0d37c60ecb636
q.w0        3fea35bcd20e4337   3fea35bcd20e4337
p.x         4088f068a0efecf1   4088f068a0efecf1     (300 tick 후)
p.y         c0c39a33fdb8d9b4   c0c39a33fdb8d9b4
p.z         40b22f3ae6273b67   40b22f3ae6273b67
v.x/v.y/v.z  … 전부 동일
q.x/q.y/q.z/q.w … 전부 동일
omega.x     4041b08ada4e6b50   4041b08ada4e6b50
비트 차이: 0 / 15
```

양자화까지 통과시킨 정수도 같다: `qpos 798051 -10036406 4655230` (양쪽 동일).

**부수로 ADR-0009 §2의 경고가 Mono에서 참임을 확인했다.** 같은 실행에서:

```
x        Rust f64::round()   Mono Math.Round(x)   Mono Math.Round(x, AwayFromZero)
0.5      1                   0                    1
2.5      3                   2                    3
-2.5     -3                  -2                   -3
1234.5   1235                1234                 1235
```

**`MidpointRounding.AwayFromZero`를 빼먹으면 `.5` 경계에서 서버와 다른 정수가 나온다** — 문서상 우려가 아니라 이 런타임에서 실제로 그렇다. C3의 양자화 함수에 이 인자를 강제하는 단위 테스트를 넣겠다.

**한계 (정직하게)**:
- 측정한 것은 **Editor의 Mono/x64**다. **IL2CPP Standalone 빌드는 재지 않았다.** 이번 슬라이스는 Editor에서 돌므로 판정에 쓰는 런타임은 맞지만, 플레이어 빌드가 생기는 슬라이스에서 다시 재야 한다.
- 12단계 전체가 아니라 **그 단계들이 쓰는 산술 종류**를 덮었다(초월함수 없음, `+ - * / sqrt`, `f64` π). AC-12가 실제 입력열로 전수 확인하는 것을 대체하지 않는다 — **선행 위험을 제거했을 뿐이다.**
- **이 결과가 AC-12를 느슨하게 만들지 않는다.** AC-12가 초과하면 원인은 "런타임이 다르다"가 아니라 "두 구현이 다르다"이고, 그때도 **임계값을 늘리지 않고 보고한다.**

→ **architect에게**: ADR-0010 §3의 "클라이언트 예측이 같은 비트를 내는지는 가정이다"를 **"Editor Mono/x64에서 실측으로 확인됐다(2026-09-20, 비교 15값 전부 비트 동일). IL2CPP는 미측정"**으로 갱신할 수 있다. ADR-0012 §3도 같다.

---

## 수용 기준 검토 (증명 가능한가, 문구 제안)

| AC | 증명 가능? | 판단 근거 / 문구 제안 |
|----|-----------|---------------------|
| **AC-10** 생성기 | **가능** (사실상 선검증 완료) | (a) 재현 메시지 정정 필요(R2). (b)(c)(d) 실측으로 전부 성립 확인. (e) 문구 정정 필요(R3) |
| **AC-11** EditMode | **조건부** | (a)(c) **26 → "26건 발견 / 20건 왕복"으로 정정 필요(R1)**. (b) 실측 완료, 정정할 칸 0. (d) fixture 두 건(`empty-nulls-and-bounds.json` ships=0, `two-ships-one-lingering.json` ships=2, presence `ACTIVE`/`LINGERING`)이 이미 있고 둘 다 왕복 동일 확인. (e) p0-02의 `Runtime` 프로필이 그대로 적용된다. (f) 파일 해시 비교로 가능 |
| **AC-12** 예측·재조정 | **가능. 다만 선행 조건이 있다** | (a) "모든 tick" → "각 스냅샷마다"(R7). (c) 순수 함수라 2회 실행 비교로 자명. (e) 선회 중 스냅샷 포함 — 좋은 요구다. **선행: S6 산출물의 형식이 정해져야 한다**(아래 §계획). U-15를 미리 재 둬서 이 AC가 실패하면 원인이 런타임이 아님이 확정된다 |
| **AC-13** 실서버 왕복 | **가능** (p0-02 경로 재사용) | (c) `reconcile_hard_snap_total == 0`이 가장 값싼 고감도 센서다. (e) 단위 정정 필요(R5) |
| **AC-14** 눈 | **가능하나 자동화 불가** | (a)가 부호 버그의 유일한 장치라는 판단에 동의. 녹화·스크린샷으로 남긴다. **techart 미투입이므로 그레이박스 범위를 §계획에 못박았다** |
| **AC-15** 타 함선 | **(b)가 지금 문구로는 불충분** | R6. 나머지는 가능. (e) 보간 지연을 로그에 찍는 요구가 좋다 — `remote_interp_delay_ms=200`, `snapshot_interval_ticks=2`(tick_hz 20 / snapshot_hz 10)로 검산된다 |
| **AC-16** A가 움직이면 B가 본다 | **가능. 클라이언트가 무엇을 내놔야 하는지 아래에 적었다** | |
| **AC-21(b)** 필드별 표 | 가능 | 신규 7타입 중 C# 열이 있는 것은 **4종**(데이터 3종은 DTO 없음). `ShipState`는 "배열 원소 타입 별도 표" 규약대로 17행을 따로 낸다 |

### AC-16을 어떻게 잴 것인가 (architect·qa가 물은 것)

(a)(b)가 진짜 검증이고 (c)는 스펙이 스스로 적었듯 항진명제에 가깝다. **클라이언트가 내놓을 것을 이렇게 고정하겠다** — 그러면 qa가 두 클라이언트의 산출물을 조인하기만 하면 된다.

1. **스냅샷마다 CSV 한 줄씩** (`client/Logs/starfall-snapshots.csv`, 파일 경로는 환경 변수로 지정):
   ```
   tick,observer_actor_id,ship_id,presence,px_mm,py_mm,pz_mm,vx_mm_s,vy_mm_s,vz_mm_s
   ```
   **양자화 정수 그대로** 적는다(실수로 바꾸면 (c)의 "완전히 같다"를 잴 수 없다).
2. **한 줄 요약 로그**도 남긴다:
   `starfall.net: WORLD_SNAPSHOT tick=<n> ships=<count> controlled=<uuid> ack=<seq|null>`
3. qa는 **envelope `tick`으로 두 CSV를 조인**한다. (a)는 B의 CSV에서 A의 `ship_id` 행을 tick 순으로 읽어 단조성과 총 이동 거리, (b)는 B가 본 자기 함선, (c)는 같은 `(tick, ship_id)`의 6개 정수 비교.
4. **(c)가 잡는 유일한 버그**(세션별 직렬화가 배열을 다르게 만드는 것)를 리포트에 그대로 적는 데 동의한다. 검사 쌍 수는 CSV 행 수에서 바로 나온다.

봇도 같은 CSV를 내면 31 연결 전체에 같은 방법을 쓸 수 있다 — **qa에게**: 봇의 스냅샷 기록 형식을 위와 맞춰 주면 조인이 한 번에 된다.

---

## 내 태스크 실행 계획 (순서·예상 위험)

**착수 조건**: R1(26 → 26/20)과 R2·R3(AC-10 문구) 정정. R4·R6은 태스크 지시 한 줄씩이라 병행 가능.

| # | 단계 | 내용 | 위험 / 대응 |
|---|------|------|------------|
| 1 | **C1** | 검증한 3건 + `BuildProperties` 한 줄을 실제 `tools/codegen`에 적용. 확장 전 실패를 먼저 기록(3단 캐스케이드 전부). `--check` 결정성, 기존 6파일 바이트 동일, `ContractTypes.cs` diff 8줄 확인 | 스크래치에서 이미 통과시켜 **위험이 거의 없다.** `verify` csproj로 Unity 임포트 전 컴파일 검증(Safe Mode 예방) |
| 2 | **C2** | fixture 상수 26/33, 왕복 20건, §5.4 열 17/10/6 상수화, 빈 배열·2척 스냅샷, `Runtime` 관용, `data/` 사본 동일성 | 사본 동일성 검사가 **줄바꿈 정규화로 통과해 버리지 않게** 바이트 비교로 한다. `data/`는 designer 소유라 사본 갱신 절차를 T10에 적는다 |
| 3 | **C3** | 예측 코어. `double` 벡터·쿼터니언, 12단계, 양자화·역양자화, `data/` 로더. **MonoBehaviour·Unity 타입 없음** | **핵심 설계 결정**: 예측 코어의 입력 타입을 `QuantisedControl`(정수 11필드)로만 만들고 **원시 실수가 들어갈 생성자를 두지 않는다.** I-36을 규율이 아니라 타입으로 강제한다. 첫 테스트는 S3와 **같은 기대 정수**를 쓴다(규약의 실체) |
| 4 | **C4** | 입력 수집·양자화·20 Hz 송신, 재조정(위치·속도·자세·**각속도**), 렌더 오프셋 3구간 | **`ω_roll` 분해가 조용한 함정이다** — 스냅샷은 월드 각속도 3성분만 주므로 `ω_aim`/`ω_roll`로 되돌리려면 전방축 성분을 분해해야 한다(ADR-0012 §3). 분해가 틀리면 직진에서는 멀쩡하고 **선회 중에만** 어긋난다. AC-12(e)가 정확히 이걸 노린 요구라 첫 테스트에 넣는다 |
| 5 | **C5** | 스냅샷 버퍼, 고정 지연 보간(위치 lerp + **자세 slerp**), 외삽 상한 후 정지, 부재 = 디스폰 | R6의 테스트로 시작한다(t=0.25 + 60° 이상). **R4의 직접 역직렬화 경로를 여기서 만든다** |
| 6 | **C6** | 그레이박스: 함선(단순 도형) + 추적 카메라 + **기준 마커 4개** + soft/hard 경계 표시 + 진단 HUD | **techart 미투입이므로 범위를 미리 못박는다** — 머티리얼은 URP Lit 기본, 셰이더·VFX·포스트 0개, 프리팹은 프리미티브 조합. 마커는 `cradle.json`의 좌표를 읽어 배치(하드코딩 금지). **씬은 Unity로 만든다**(YAML 수동 편집은 GUID를 깨뜨린다 — p0-02 교훈) |
| 7 | 요약 | `03_client_impl.md` | |

**미리 정해 두고 싶은 것 2가지 (architect·server에게)**

1. **S6 산출물 형식.** C4의 첫 테스트가 그것을 읽는다. 최소한 (a) 입력열: tick별 11개 정수 + `input_seq`, (b) tick별 스냅샷: `WORLD_SNAPSHOT` payload JSON 그대로, (c) 초기 상태. **JSON Lines 한 줄 = 한 tick**이면 클라이언트가 그대로 읽는다. 형식이 늦게 정해지면 C4가 대기한다.
2. **`ship_class_id` → `data/ships/*.json` 매핑 규칙.** 스냅샷은 `ship_class_id`(예: `scout-s01`)만 준다. 파일명이 곧 id라고 가정해도 되는가, 아니면 파일 안의 `id` 필드를 읽고 디렉토리를 훑어야 하는가. 후자로 구현하겠지만 확인 바란다.

---

## 확인한 사실 / 미확인

### 확인한 사실 (전부 오늘 이 PC에서 실행)

| # | 사실 | 방법 |
|---|------|------|
| P-1 | 현재 생성기의 첫 실패는 `maxItems`다(`array`·`data` 아님). 캐스케이드 3단, 각 단계 종료 코드 2 | 실제 `contracts/`로 3회 실행 |
| P-2 | **확장 3건 + `BuildProperties` 1줄이면 충분하다.** 11파일 생성, 종료 코드 0 | 스크래치 사본 실행 |
| P-3 | **U-14 PASS** — `ShipState`가 `WorldSnapshotPayload` 안의 중첩 클래스로 나오고 `ships`가 `ShipState[]`, 속성 17개 | 리플렉션 |
| P-4 | 생성기는 결정적이다(2회 해시 동일, `--check` 0). 기존 타입별 6파일 **바이트 동일**, `ContractTypes.cs`만 +8줄 | 해시·diff |
| P-5 | 유효 fixture 26건 중 **C# DTO가 있는 것은 20건**, 6건은 envelope 판별자조차 없다. 20건 전부 왕복 동일 | 전수 실행 |
| P-6 | **§5.4의 C# 예측 11행이 전부 실측과 일치한다.** 전체 33건: 거부 17 / 통과 10 / C# 계층 없음 6 | 전수 실행 |
| P-7 | 배열 원소 안의 필수 필드 검증이 C#에서 동작한다(`ship-missing-orientation-w` 거부) | 실행 |
| P-8 | p0-02의 좁힘 수정이 **`causation_id`에도 그대로 먹는다** | `SHIP_SPAWNED`·`SHIP_DESPAWNED`의 `causation-id-null` 거부 |
| **P-9** | **U-15: Unity Mono(x64)와 Rust가 비트 동일하다.** 300 tick 적분 후 비교 15값 전부 일치, 양자화 정수도 일치 | Unity 동봉 `mcs`+`mono.exe` vs `cargo run --release`, IEEE-754 비트 출력 |
| P-10 | **Mono의 `Math.Round` 기본값이 Rust `f64::round()`와 다르다**(0.5→0 vs 1, 2.5→2 vs 3, 1234.5→1234 vs 1235). `MidpointRounding.AwayFromZero`가 필수 | 같은 실행 |
| **P-11** | **U-16 부분 해소**: 31척 16,347 B 스냅샷을 Unity Mono에서 — 현재 경로(트리+ToObject) **0.656 ms/건**, 직접 역직렬화 **0.265 ms/건**, gen0 수집 **8배 차이** | Unity Mono + Unity 동봉 Newtonsoft 13.0.2, 200회 반복 |
| P-12 | designer 데이터 3파일이 **전부 스키마 0건 오류**로 통과한다(D-1·D-2·D-3 반영됨). `reconcile_ignore_threshold_m=0.005`, `remote_interp_delay_ms=200`, `snapshot_hz=10`, `client_send_hz=20` | 오프라인 2020-12 검증기 + 로컬 `$ref` 레지스트리 |
| P-13 | 계약 커버리지 구현 전 기준선 **errors 14 / warnings 0** 재확인. 그중 **client 책임 3건**(`SET_SHIP_CONTROL` producer, `WORLD_SNAPSHOT` consumer, `SYNC_TUNING` consumer) | 스크립트 실행 |
| P-14 | 커버리지 스크립트가 `SyncTuning` 문자열만 찾으므로 **로더 클래스 이름으로 §5.7의 계획이 성립한다** | 스크립트 출력 문구 |

### 미확인

| # | 사실 | 누가 언제 | 안 되면 |
|---|------|----------|--------|
| U-15b | **IL2CPP** 빌드의 `double`이 같은가 | 플레이어 빌드가 생기는 슬라이스 | 이번 슬라이스는 Editor(Mono)라 판정에 영향 없음 |
| U-16b | 실제 수신 경로의 **프로파일러 실측**(내 수치는 합성 스냅샷 + `GC.GetTotalMemory` 기반이라 프로파일러와 절대값이 다를 수 있다) | client, AC-13(e) | 상대 비교(경로 2 vs 3)는 이미 유효하다 |
| U-16c | 실제 스냅샷 바이트(내 16,347 B는 합성값이라 자릿수가 실제보다 길 수 있다) | qa, AC-18(c) | 봇 실측이 정본 |
| U-21 | 1차 쿼터니언 적분 + 폐루프가 오버슈트 없이 안착하는가 | server, AC-4(g) | 클라이언트 예측이 같은 식을 쓰므로 결과를 공유한다 |
| U-C1 | `unity test`가 없는 `--output` 디렉토리를 만들어 주는가 | client, C2 | `mkdir -p`로 회피 가능 |
| U-C2 | S6 산출물 형식 / `ship_class_id` → 파일 매핑 규칙 | architect·server | C4·C3가 대기한다 |

### 이번 턴에 남긴 변경

없다. `tools/`·`client/`·`docs/`·`contracts/`·`data/` 전부 읽기만 했다. 생성기 사본·생성물·프로브(C#·Rust)는 모두 세션 스크래치 디렉토리에 있고 저장소 밖이다.
