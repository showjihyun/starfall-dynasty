---
name: event-contracts
description: "STARFALL DYNASTY의 Unity(C#)↔Rust 공유 데이터 계약 규약. contracts/ 폴더 구조, 타입 레지스트리, 이벤트 envelope(event_id·tick·correlation_id·causation_id·schema_version), 명령·도메인 이벤트·역사 이벤트·서버 메시지 이름 규칙, JSON Schema와 fixture, 스키마 버전 관리·업캐스팅, Rust serde/C# DTO 대응 방식을 정의한다. 이벤트·메시지·API·게임 데이터의 shape을 만들거나 바꿀 때, 서버와 클라이언트가 주고받는 형식을 다룰 때, 직렬화 불일치를 조사할 때 반드시 사용."
---

# Event Contracts — Unity와 Rust가 어긋나지 않게 하는 단일 진실

서버(Rust)와 클라이언트(C#)에 같은 데이터 모양이 따로 정의되면 반드시 어긋난다. 그래서 모양은 `contracts/`에 한 번만 정의하고, 양쪽은 **같은 fixture를 역직렬화하는 테스트**로 계약을 지키는지 증명한다.

소유자는 game-architect다. 다른 에이전트는 계약을 읽고 변경을 요청한다.

## 폴더 구조

```
contracts/
├── registry/types.json            # 모든 계약 타입 등록부 (아래 형식)
├── common/                        # envelope, id, game_time, money 등 공통 정의
│   ├── event-envelope.schema.json
│   └── command-envelope.schema.json
├── commands/{NAME}.schema.json    # 클라이언트 → 서버 의도
├── events/domain/{NAME}.schema.json
├── events/historical/{NAME}.schema.json
├── history/                       # evidence, claim, interpretation 레코드
├── messages/{NAME}.schema.json    # 서버 → 클라이언트 실시간 메시지
├── api/                           # REST 요청·응답 스키마 (경로별)
├── data/                          # 게임 데이터 테이블 스키마 (data/ 폴더 검증용)
└── fixtures/{NAME}/*.json         # 타입마다 최소 1개의 실제 예시
```

형식은 JSON Schema(2020-12)로 시작한다. Protobuf 같은 바이너리 형식 전환은 실시간 전투 규모 테스트에서 필요가 측정된 뒤 ADR로 결정한다.

## 타입 레지스트리 — `contracts/registry/types.json`

```json
{
  "registry_version": 1,
  "types": [
    {
      "name": "MINERAL_MINED",
      "kind": "domain_event",
      "schema": "events/domain/MINERAL_MINED.schema.json",
      "schema_version": 1,
      "producers": ["server"],
      "consumers": ["history", "client"],
      "status": "active"
    }
  ]
}
```

- `kind`: `command` | `domain_event` | `historical_event` | `server_message` | `rest` | `data`
- `producers`/`consumers` 태그: `server`, `history`, `client`, `bots`. QA의 계약 커버리지 스크립트가 이 태그로 코드 쪽 구현 여부를 검사한다.
- `status`: `active` | `deprecated`(새 생산 금지, 소비는 유지) — 과거 이벤트가 DB에 남아 있으므로 타입을 지우지 않는다.

## 이름 규칙

이름은 서사가 아니라 기계적 사실로 짓는다. "THE_GREAT_TRAGEDY_OF_VESTA" 같은 이름은 Narrative 계층이 만든다.

| 종류 | 형식 | 예 |
|------|------|----|
| 명령 (의도) | 명령형 `VERB_NOUN` | `MINE_RESOURCE`, `SELL_ITEM`, `FIRE_WEAPON` |
| 도메인 이벤트 (원자적 사실) | 과거형 `NOUN_VERBED` | `MINERAL_MINED`, `SHIP_DAMAGED`, `ITEM_SOLD` |
| 역사 이벤트 | 과거형, MVP 10종 우선 | `MINERAL_DISCOVERED`, `SHIP_DESTROYED`, `PLAYER_DIED`, `CONTRACT_BROKEN` |
| 서버 메시지 | 명사형 | `WORLD_SNAPSHOT`, `COMMAND_RESULT`, `CHRONICLE_ENTRY_ADDED` |

- 와이어 JSON의 필드 이름은 `snake_case`, 타입 이름 문자열은 `SCREAMING_SNAKE_CASE`.
- Rust: 변형 이름은 `PascalCase` + `#[serde(rename_all = "SCREAMING_SNAKE_CASE")]`, 필드는 기본 snake_case.
- C#: 클래스·속성은 `PascalCase`, 직렬화 이름은 `[JsonProperty("snake_case")]`로 명시한다. 이름 변환 규칙에만 의존하면 설정 하나로 조용히 깨진다.

## Envelope

모든 도메인·역사 이벤트는 공통 envelope을 가진다 (기획안 HSE §101).

| 필드 | 타입 | 의미 |
|------|------|------|
| `event_id` | string (UUIDv7) | 이벤트 고유 ID. 멱등 처리 키 |
| `event_type` | string | 레지스트리 이름 |
| `schema_version` | integer | payload 스키마 버전 |
| `world_id` | string | 월드(샤드) ID |
| `tick` | integer | 발생 시뮬레이션 tick |
| `sequence` | integer | 같은 tick 안의 결정적 순서 |
| `occurred_at` | string | **게임 시간** (예: `3827-04-13T18:32:11Z`) |
| `recorded_at` | string | 실제 시간 (RFC 3339) — 감사용, 판정에 쓰지 않음 |
| `correlation_id` | string | 하나의 게임플레이 트랜잭션(예: 거래 1건)을 묶는 ID |
| `causation_id` | string \| null | 이 이벤트를 **직접 유발한** 이벤트/명령 ID |
| `actor_id` | string \| null | 행위자 |
| `payload` | object | 타입별 스키마 |

역사 이벤트는 추가로 `rule_version`(판정 규칙 버전), `source_event_ids`(근거 도메인 이벤트), `importance_level`(0~5)을 가진다.

`correlation_id`와 `causation_id`를 섞지 않는다. 거래 한 건에서 나온 "아이템 제거·송금·계약 갱신"은 같은 correlation, "전쟁 선포 → 무역 금지"는 causation 관계다.

명령 envelope: `command_id`(클라이언트가 생성한 UUID, 서버의 재전송 방지·멱등 키), `command_type`, `schema_version`, `client_sent_at`(참고용, 신뢰하지 않음), `payload`.

## 값 규칙

- 돈·가격: 정수 최소 단위(`i64`/`long`), 실수 금지. 실수 오차는 화폐 복사와 결정성 붕괴의 원인이다.
- 위치: 서버는 성계 로컬 좌표를 `f64`로 가진다. 클라이언트 표시용 변환(Floating Origin)은 클라이언트 책임이며 계약에 float32를 강제하지 않는다.
- 선택 필드와 null을 구분한다. 스키마의 `required`에 없는 필드는 소비자가 없을 수 있다고 처리해야 한다.
- 열거값을 추가할 수 있는 필드는 소비자가 모르는 값을 받아도 죽지 않게 처리한다 (클라이언트는 "알 수 없음" 표시, 서버는 거부 + 로그).

## 스키마 변경 규칙

| 변경 | 처리 |
|------|------|
| 선택 필드 추가 | 같은 `schema_version`, fixture 추가 |
| 필드 제거·이름 변경·타입 변경·의미 변경 | `schema_version` 증가, 서버에 이전 버전 → 새 버전 업캐스터, 기존 저장 이벤트는 다시 쓰지 않음 |
| 타입 폐기 | `status: deprecated`, 스키마·fixture는 유지 |

변경 절차:
1. architect가 스키마·레지스트리·fixture를 함께 수정한다 (셋 중 하나만 바꾸면 안 된다).
2. 레지스트리의 모든 `producers`·`consumers` 담당자에게 SendMessage로 알린다: 변경 파일, 호환성(호환/비호환), 필요한 조치.
3. qa가 `integration-qa` 스킬의 계약 커버리지 스크립트로 확인한다.

## 양쪽 구현이 계약을 지키는 방법

구체적인 코드 생성 도구는 ADR-0002에서 정한다. 어떤 도구를 쓰든 다음은 필수다.

- **Rust:** 모든 fixture를 해당 타입으로 역직렬화 → 다시 직렬화 → 스키마 검증하는 테스트. fixture 폴더를 순회하는 테스트 하나로 새 타입이 자동 포함되게 만든다.
- **C#:** 모든 fixture를 DTO로 역직렬화하는 EditMode 테스트. 생성된 DTO는 `client/Assets/_Project/Scripts/Contracts/Generated/`에 두고 손으로 고치지 않는다.
- 타입을 추가했는데 fixture가 없으면 이 테스트들이 그 타입을 검증하지 못한다. 그래서 fixture 없는 타입은 레지스트리에 `active`로 올리지 않는다.

## 흔한 경계면 버그 (이 프로젝트에서 먼저 의심할 것)

- 서버는 `{ "items": [...] }`로 감싸 보내는데 클라이언트는 배열을 기대함
- 서버 `snake_case` ↔ 클라이언트 `[JsonProperty]` 누락으로 필드가 조용히 기본값이 됨
- 열거값 추가 후 클라이언트 switch에 default가 없어 메시지 무시
- `schema_version` 올렸는데 업캐스터 없이 과거 이벤트 조회 실패
- 명령 응답(즉시 `COMMAND_RESULT: accepted`)과 실제 결과(다음 tick의 이벤트)를 클라이언트가 같은 것으로 취급
