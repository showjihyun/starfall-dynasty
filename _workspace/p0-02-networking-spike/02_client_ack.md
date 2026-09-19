# client 확인 (p0-02 스프린트 계약)

- 날짜: 2026-09-18
- 입력: `02_sprint_contract.md`(SC-38~51, SC-61, §0.5, §0.6, §4 게이트), 확정 스펙(AC-1~21), `01_architect_decisions.md`
- **차단급 이의 없음.** 아래 A-4 하나만 "동의하되 수치 근거를 정정" 건이고 비차단이다.

## 0. AC 번호 재배치 확인

내 리뷰의 옛 번호 → 새 번호로 읽었다. 내 작업 범위는 **AC-11(생성기) / AC-12(EditMode) / AC-13(실서버 왕복) / AC-14(재연결)**이고, 옛 AC-10~13이 여기에 대응한다. 옛 AC-17(봇+Unity)은 새 **AC-15**, 옛 AC-15(기록 무결성)는 새 **AC-16**으로 읽었다. 이 문서와 앞으로의 산출물은 전부 새 번호를 쓴다.

내 차단 2건 처리 확인:
- **R1 (fixture 12건)**: 수용 확인. 반례가 16건이 된 것(`SESSION_CLOSED/invalid/actor-id-null.json` 추가)도 확인. **기대 상수를 12/16으로 쓴다.**
- **R2 (U-2)**: (A) 채택 확인. T7에서 생성기를 고친다.
- 거부 3건(401 미재시도 / REST `/readyz` 찌르기 / A 단계 120초) **전부 수긍한다.** 특히 "클라이언트가 401을 알아낼 수 없으므로 구현할 수 없는 규칙을 스펙에 적지 않는다"는 내 U-5c 실측의 옳은 귀결이다. 접속 순서를 박는 쪽이 부하 창을 늘리는 것보다 싸다는 것도 동의한다.

---

## 1. SC-61 — `SESSION_READY` 수신 로그 문구 (QA 질문 6·8)

### 고정 문구

```
starfall.net: SESSION_READY session_id=<uuid> correlation_id=<uuid> actor_id=<uuid> world_id=<uuid> tick_hz=<int> server_version=<string> attempt=<int>
```

- **필드 순서 고정**: `session_id` → `correlation_id` → `actor_id` → `world_id` → `tick_hz` → `server_version` → `attempt`. 순서를 고정해야 QA의 정규식이 안정적이다.
- 모든 UUID는 계약에 실린 **소문자 하이픈 표기 그대로** 찍는다(`Guid.ToString("D")`). 재포맷하지 않는다.
- `attempt`는 그 연결이 몇 번째 시도였는지(0 = 첫 시도). **SC-51의 "카운터가 `SESSION_READY` 수신 시에만 리셋됨"을 같은 줄에서 확인할 수 있게** 넣었다.
- 한 줄이고 줄바꿈이 없다. `server_version`은 계약이 `^[0-9A-Za-z][0-9A-Za-z.+_-]{0,63}$`라 공백이 들어갈 수 없다.

### 위치

| 경로 | 내용 | 용도 |
|------|------|------|
| `client/Logs/Editor.log` | `UnityEngine.Debug.Log`로 남는다. **Unity가 그 다음 줄부터 스택 트레이스를 붙인다** — 줄 단위 grep이므로 문제없다 | 사람이 확인 / 증거 발췌 |
| `client/Logs/starfall-net.log` | **같은 줄만** 타임스탬프와 함께 append (스택 트레이스 없음). Unity와 무관한 일반 텍스트 파일 | QA 자동 수집 |

두 번째 파일을 덧붙이는 이유: Editor.log는 다른 서브시스템 로그와 스택 트레이스가 섞여 있고 Unity가 실행마다 회전시킨다. `client/Logs/`는 `.gitignore` 대상이라 저장소에 남지 않는다. **QA는 둘 중 편한 쪽을 쓰면 된다.**

### QA 추출 명령 (내가 실행해서 형식을 확인한 뒤 `03_client_impl.md`에 실제 출력과 함께 다시 싣는다)

```bash
# correlation_id만 뽑아 correlations.txt에 합치기 (§0.6)
grep -oE 'starfall\.net: SESSION_READY session_id=[0-9a-f-]{36} correlation_id=[0-9a-f-]{36}' \
  client/Logs/starfall-net.log \
| grep -oE 'correlation_id=[0-9a-f-]{36}' | cut -d= -f2 >> /path/correlations.txt

# 세션 한 줄 전체 (SC-49/50/51 증거)
grep -F 'starfall.net: SESSION_READY ' client/Logs/starfall-net.log
```

### 같이 고정하는 문구 4개 (QA가 파싱할 전부)

| 문구 | 언제 | 쓰이는 SC |
|------|------|----------|
| `starfall.net: SESSION_READY session_id=… correlation_id=… actor_id=… world_id=… tick_hz=… server_version=… attempt=…` | `SESSION_READY` 수신 | SC-49, SC-50, SC-51, SC-61 |
| `starfall.net: dropping N in-flight command(s) on disconnect (no resend, I-23)` | 연결이 끊겨 대기 명령을 버릴 때. **N=0이어도 남긴다**(0건이라는 사실도 증거다) | SC-51 |
| `starfall.net: reconnect attempt=<n> delay_ms=<ms> (counter resets only on SESSION_READY)` | 재연결 대기 직전 | SC-51 |
| `starfall.net: closing session_id=<uuid> reason=<CLIENT_CLOSED\|EDITOR_RELOAD\|PLAYMODE_EXIT\|APP_QUIT> code=1000` | 정상 Close를 보낼 때 | SC-50 (`close_reason=CLIENT_CLOSED` 기대의 반대편 증거) |

`reason=`의 값은 **클라이언트가 왜 닫았는지**이고 계약의 `close_reason`이 아니다(서버가 정한다). 네 값 모두 WebSocket close 1000(정상)으로 나가므로 서버는 전부 `CLIENT_CLOSED`로 기록해야 한다. **`reason=EDITOR_RELOAD`가 찍혔는데 DB가 `TRANSPORT_ERROR`면 내 Close가 늦은 것이고, 아무 줄도 없이 `TRANSPORT_ERROR`면 훅이 안 걸린 것이다** — QA가 이 둘을 구분할 수 있게 값을 나눴다.

---

## 2. Unity 개발용 토큰 주체 id (QA 질문 7)

### 고정 값

```
01a0b1c2-7e57-7c11-8e57-000000000001
```

- `contracts/common/primitives.schema.json`의 `UuidV7` 패턴을 만족한다(version nibble `7c11` → 7, variant `8e57` → 8). 검증은 T9의 기존 `UuidV7_MatchesSchemaPattern` 경로로 같이 돈다.
- 토큰은 ADR-0008 §1 그대로 `"<subject>." + hex(HMAC_SHA256(STARFALL_DEV_AUTH_SECRET, subject))`로 클라이언트가 계산한다. **비밀은 환경 변수에서만 읽고 파일에 쓰지 않는다.**
- **환경 변수 `STARFALL_DEV_ACTOR_SUBJECT`로 덮어쓸 수 있다.** 기본값이 위 리터럴이다. 봇 쪽 파생 규칙과 충돌이 나면 QA가 재빌드 없이 바꿀 수 있다.

### 겹치지 않음을 보장하는 방법

봇의 파생 함수(`bot-000`~`bot-029` → UUIDv7)를 내가 모르므로, **"다른 이름을 넣으면 다른 값이 나온다"에 기대지 않고 리터럴로 고정**했다. 대신 QA가 한 줄로 확인할 수 있다:

```bash
# 봇 신원 30개를 출력하는 명령(tools/bots가 제공)의 결과에 Unity 주체가 없어야 한다
tools/bots identities --count 30 | grep -F 01a0b1c2-7e57-7c11-8e57-000000000001 && echo "COLLISION" || echo "disjoint OK"
```

**qa에게**: 봇 신원을 출력하는 서브커맨드가 없다면 하나 만들어 주면 좋겠다(부하 실행 전 `disjoint OK` 한 줄이 SC-15의 "31개 연결"이 31개 서로 다른 행위자라는 것의 값싼 증거가 된다). 없어도 진행에는 지장 없다 — 그때는 내가 `SESSION_READY`에서 받은 `actor_id`를 봇 `sessions.json`의 `actor_id` 집합과 대조하는 것으로 대체한다.

**server에게**: 이 주체는 DB 어디에도 없는 UUID다(ADR-0008 §4). 서버가 주체에 대해 하는 일은 HMAC 검증과 `actor_id`로의 승격뿐이어야 한다.

---

## 3. A 단계 시작 전 PlayMode READY (QA 질문 8) — **가능하다. 단 실행 형태를 하나 고쳐야 한다**

**순서에는 동의한다**: Unity를 먼저 `SESSION_READY`까지 올린 뒤 봇 30개를 시작한다.

### 고쳐야 할 것: `unity test --mode PlayMode`는 이 용도로 쓸 수 없다

`unity test`는 Editor를 띄워 테스트를 돌리고 **끝나면 Editor를 종료한다.** 연결이 같이 죽는다. 그래서 두 가지를 분리해야 한다:

| 용도 | 실행 형태 | 해당 SC |
|------|----------|--------|
| **왕복 검증**(3건 보내고 정상 종료) | `unity test --mode PlayMode` 또는 수동 PlayMode 1회 | SC-49, SC-50 |
| **A 단계 동안 31번째 연결 유지** | **사람이 띄워 둔 Editor를 PlayMode로 두고 유지** | SC-61, SC-15 |

두 번째를 위해 내가 제공하는 것:

1. `STARFALL_NET_AUTOCONNECT=1` 환경 변수가 있으면 **PlayMode 진입 시 자동 접속**하고 명시적으로 끊을 때까지 유지하는 부트스트랩(`[RuntimeInitializeOnLoadMethod]` — **씬·프리팹을 만들지 않는다.** 빈 기본 씬에서 Play만 눌러도 된다).
2. Editor 메뉴 `Starfall/Net/Connect`·`Disconnect`·`Send PING_SERVER ×3` — 부하 중 사람이 손으로 제어할 수 있게.
3. 접속이 READY가 된 시점이 `starfall-net.log`에 찍히므로 **qa는 그 줄을 본 뒤 봇을 시작하면 된다.**

### 실행 절차 (내가 `03_client_impl.md`에 그대로 싣는다)

```
1. 서버·인프라 기동 확인
2. 셸에서: export STARFALL_DEV_AUTH_SECRET=... ; export STARFALL_NET_AUTOCONNECT=1
   같은 셸에서 Unity Hub/Editor로 client 프로젝트를 연다 (환경 변수 상속이 필요하다)
3. Play 버튼 → client/Logs/starfall-net.log 에 SESSION_READY 한 줄이 뜨는지 확인
4. 그 correlation_id를 correlations.txt에 넣는다  ← 31번째
5. qa가 봇 30개 시작 (A 단계)
6. A가 끝날 때까지 Editor를 건드리지 않는다
```

### 타이밍 근거와 위험

- 실측: 프로젝트 로드 **21.15초**, EditMode 웜 실행 13~20초. PlayMode 진입에 도메인 리로드가 한 번 더 붙는다. **3단계까지 콜드 기준 30~45초**로 보면 된다. A 시작 전에 끝내면 A의 60초를 온전히 쓴다.
- **가장 큰 위험은 A 도중의 도메인 리로드다.** 스크립트 파일이 하나라도 저장되거나 `unity test`가 같은 프로젝트에 붙으면 리로드가 일어나 31번째 연결이 끊긴다. 게이트 **G-h**("Unity 콜드 임포트·EditMode는 부하와 동시에 돌리지 않는다")가 이미 이것을 막고 있다 — **G-h를 지키면 이 순서는 성립한다.** 추가로 "A가 도는 동안 `client/` 아래 파일을 저장하지 않는다"를 실행 절차에 한 줄 넣어 주기 바란다.
- 리로드가 일어나도 훅이 정상 Close를 보내고 `reason=EDITOR_RELOAD` 줄이 남으므로, **QA는 SC-61이 30을 낸 이유를 사후에 구분할 수 있다**(31번째가 조용히 사라지지 않는다).

### 한 가지 확인 요청 (qa)

SC-61은 "A 시작 후 20~40초 구간에서 자동 실행"이다. **31번째 세션은 A 시작 *전에* 열리므로 `SESSION_OPENED`가 A 시작 시점에 이미 DB에 있다.** 그 행의 `recorded_at`이 A 시작보다 앞선다는 이유로 "A의 세션이 아니다"로 걸러지지 않는지만 확인 바란다. §0.6의 correlation 집합 방식이면 문제없다(시간이 아니라 집합으로 거른다).

---

## 4. AC-21의 46필드 규약 (QA 질문 4) — **동의한다. 다만 그 46은 내 계산 착오에서 나왔다**

### 규약은 그대로 써도 된다

고정된 규약(메시지 타입은 `payload` 자체를 1행으로 세고, 이벤트 타입은 `payload`를 세지 않는다)으로 계산하면:

| 타입 | envelope | payload 멤버 | 합 |
|------|---:|---:|---:|
| `COMMAND_RESULT` | 6 (`payload` 포함) | 3 | 9 |
| `SESSION_READY` | 6 (`payload` 포함) | 5 | 11 |
| `SESSION_OPENED` | 11 (`payload` 제외, 실제 top-level은 12) | 2 | 13 |
| `SESSION_CLOSED` | 11 (`payload` 제외, 실제 top-level은 12) | 2 | 13 |
| | | | **46** |

산수는 맞다. **46으로 진행하는 데 이의 없다.**

### 다만 사실관계를 정정한다

이 비대칭은 **설계가 아니라 내 검토 문서의 계산 착오다.** 메시지 envelope는 6필드를 `payload` 포함으로 세고, 이벤트 envelope는 12필드를 `payload` 빼고 11로 셌다. 같은 기준을 쓰면:

- `payload`를 어디서나 1행으로 세면 → 6+3, 6+5, **12**+2, **12**+2 = **48**
- `payload`를 어디서도 세지 않으면 → 5+3, 5+5, 11+2, 11+2 = **44**

**권장은 48이다.** `payload`도 3층(스키마·Rust·C#)이 이름·필수 여부·널 가능성을 합의해야 하는 필드이고(C#에서 `Required.Always`, Rust에서 비-`Option`), 한쪽에서만 비교 대상에서 빠질 이유가 없다.

### 그래서 제안

**총계를 바꾸지 말고**(스펙·SC-73·리포트가 이미 46으로 고정돼 있다), 내가 `03_client_impl.md`에 **필드별 표(타입 × 필드 × C# 타입 × Required × 스키마 필수/널가능)를 전부 싣겠다.** 그러면 총계는 주장이 아니라 표에서 **유도**되고, QA는 46이든 48이든 같은 표로 SC-73을 채점할 수 있다. 다음 슬라이스에서 계수 규약을 손볼 때 이 표가 근거가 된다.

`payload`를 세는 행에는 `counted: envelope-row` 표시를 달아 어느 규약으로 센 것인지가 표 자체에 드러나게 한다.

---

## 5. 그 밖에 확인한 것 (이의 없음)

| 항목 | 확인 |
|------|------|
| §0.5 표의 C# 열: 거부 기대 **11건**(#1,3,4,5,6,9,11,13,16 + 좁힘 후 #12,14), "감지 불가" **5건**(#2,7,8,10,15) | 동의. 내 15건 실측(거부 9 / 통과 6)에서 #12가 좁힘 수정으로 거부로 옮겨가고 #14가 새로 추가되어 11/5가 된다. 산수 일치 |
| SC-44의 "표와 다르면 FAIL이 아니라 architect 통지 사유" | 동의. 내가 표와 다른 결과를 얻으면 최종 답과 `03_client_impl.md`에 통지로 적는다 |
| SC-46 보조 요구("`Could not find member` 접두사 의존을 고정하는 테스트") | 이미 그렇게 할 계획이었다. Newtonsoft 업그레이드가 런타임이 아니라 테스트를 깨게 한다 |
| SC-47의 7개 n 값과 delay를 출력에 남긴다 | 동의. `TestContext.WriteLine`으로 남겨 리포트 `<system-out>`에 들어가게 한다(p0-01 방식) |
| G-2가 **실물 `Generated/`에 쓰고** 해시를 비교한다 | 동의. 생성물은 저장소에 추적되는 것이 맞다(ADR-0001 `.gitignore` 주석). 커밋은 하지 않는다 |
| E3/E4 미검증 처리 기준 | 동의. 서버가 없으면 SC-49~51은 **미검증(환경)**으로 적고 통과로 적지 않는다 |
| G-h + "A 단계에는 Editor가 PlayMode로 붙어 있어야 한다" | 동의. §3의 실행 절차가 이것을 전제한다 |

## 6. 진행

차단급 이의가 없으므로 **바로 T7 → T8 → T9로 들어간다.** SC-49~51(실서버)은 서버 기동 후에만 가능하므로 그 전까지 SC-38~48을 끝낸다.

계약 변경 요청 **없음.** (§2의 "봇 신원 출력 서브커맨드"는 `tools/bots` 편의 요청이고 계약 변경이 아니다.)
