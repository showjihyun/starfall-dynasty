# server의 ADR 검토 (p0-01-bootstrap)

- 검토자: server (rust-server-engineer), 서브 에이전트 모드 — 팀원 직접 메시지 불가, 리더가 architect에게 중계
- 날짜: 2026-09-18
- 읽은 것: `CLAUDE.md`, `00_request.md`, ADR-0001/0002/0003, `docs/specs/p0-01-bootstrap.md`, `01_architect_tasks.md`, `contracts/` 전체(스키마 7개, 레지스트리 2개, fixture 유효 4 + 무효 5), 스킬 `rust-authoritative-server`·`event-contracts`
- 결론: **ADR 3건 모두 accepted 전환에 동의한다.** 구조·계약 형식·환경 결정에 반대하는 항목은 없다. 다만 스펙 수용 기준 중 AC-10은 현재 문구로 **증명 불가능**하고, AC-1·AC-2는 현재 문구로 **실행 불가능**하다(작업 디렉토리 누락, PowerShell `curl` 별칭). 계약 파일 자체에 대한 변경 요청은 선택 1건뿐이다.

## architect가 물은 5개에 대한 답 (요약)

| # | 질문 | 답 |
|---|------|-----|
| 1 | `starfall-` 접두사 / "빈 크레이트를 만들지 않는다" | 둘 다 **동의**. 가드레일 한 줄만 추가 요청 (R-11) |
| 2 | `/readyz`가 부트스트랩에 과한가 | **과하지 않다. 유지하자.** ADR-0003의 위험한 결정(15432 포트, PG18 볼륨, `.env` 배선)을 서버 프로세스 관점에서 실제로 증명하는 유일한 경로다. 대신 구현 제약 4가지와 AC-3 보강 (R-3). 의존성 비용은 TLS feature를 빼서 줄이고, 실측치를 `03_server_impl.md`에 남긴다 |
| 3 | 계약 테스트 5종이 충분한가 / "재직렬화 = 원본" 실현 가능한가 | **실현 가능하다** — 단 Rust 타입 설계에 4가지 제약이 붙고, 그 제약이 지금 어디에도 적혀 있지 않다 (R-10). 5종은 **부족하다** — 9종으로 (R-5) |
| 4 | PG18 볼륨 경로·호스트 포트 15432 전제 | 포트 전제 **확인 완료**(5432는 PID 7612가 점유 중, 15432·16379·8080은 비어 있음). 이미지 태그 2개 존재 **확인 완료**. 볼륨 경로는 이미지 pull 없이는 확인 불가 → **미확인**. AC-4의 증거 방식은 교체 요청 (R-4) |
| 5 | Redis 클라이언트 크레이트 | `redis` (redis-rs) **1.7.0**, `default-features = false, features = ["tokio-comp"]`, 연결은 `redis::aio::ConnectionManager`. 이유는 아래 |

---

## 동의하는 결정

### ADR-0001 (레포·모듈 구조)

**A-1. 크레이트 접두사 `starfall-` — 동의.**
ADR이 든 이유(crates.io 동명 크레이트와의 혼동)에 더해, 구현자 입장에서 실질적인 이유가 둘 더 있다.

- `contracts`, `domain`은 crates.io에 실재하는 이름이다. 전이 의존성이 그중 하나를 끌어오면 `use contracts::...`가 어느 쪽인지 사람이 읽어서는 구분되지 않고, rust-analyzer의 go-to-definition이 엉뚱한 곳으로 간다. 접두사는 이 모호함을 컴파일러가 아니라 이름으로 제거한다.
- `cargo tree`, 빌드 로그, 패닉 백트레이스에서 우리 크레이트가 한눈에 구분된다. 나중에 프로파일링할 때(원칙 10) 이게 실제로 도움이 된다.

비용은 명령이 길어지는 것뿐이고, 그건 스크립트로 흡수된다. **반대 없음.**

작은 보완 제안(선택, 차단 아님): `[lib] name`을 따로 지정해 짧게 만들고 싶은 유혹이 생길 텐데, 그러면 패키지 이름과 crate 이름이 갈라져 위 장점이 절반 사라진다. lib 이름은 패키지 이름 그대로 두고, 호출부가 장황하면 `use starfall_sim as sim;`로 파일 단위에서 줄이자. 이 한 줄을 ADR-0001 §2에 넣어 두면 나중에 누가 다시 꺼내지 않는다.

**A-2. "부트스트랩에서는 실제로 쓰는 크레이트만 만든다" — 동의.**
ADR의 논거("빈 크레이트는 아무 것도 증명하지 않는다")에 더해 구현자 관점의 근거:

- 빈 크레이트 4개는 `cargo test --workspace`마다 테스트 하니스 4개를 **링크**한다. Windows MSVC에서 가장 느린 단계가 링크다. 아무 것도 검증하지 않는 링크 4회를 매 반복에 붙이는 셈이다.
- 더 중요한 건 심리적 효과다. 빈 `starfall-domain`이 있으면 "일단 여기 넣자"가 생기고, 그 시점에 의존 방향은 아직 결정되지 않았다. ADR이 말한 "실제 의존 방향은 첫 코드가 들어갈 때 결정된다"가 맞다.

**단, 이 결정의 부작용에 가드레일이 하나 필요하다 → R-11.**

**A-3. `rust-toolchain.toml`을 레포 루트에 — 동의.**
rustup은 현재 디렉토리부터 상위로 올라가며 찾으므로 `server/`와 `tools/bots/`가 같은 파일을 상속한다. 두 파일이 어긋나는 사고를 구조적으로 막는다.

확인한 사실: 이 PC에 설치된 툴체인은 `stable-x86_64-pc-windows-msvc` **하나뿐**이다. `channel = "1.98.1"`은 버전이 같아도 rustup에게는 **다른 툴체인 이름**이라, 레포 안에서 첫 `cargo` 실행 시 `1.98.1-x86_64-pc-windows-msvc`를 새로 내려받는다(수백 MB, 1회). 차단 요소는 아니지만 QA가 "왜 첫 명령이 오래 걸리나"로 놀라지 않도록 스펙 §8이나 README에 한 줄 있으면 좋겠다.

**A-4. `tools/bots/`를 별도 Cargo 워크스페이스로 — 동의.**
루트 `Cargo.toml`을 server와 qa가 함께 고치는 상황을 피한다는 이유에 동의한다. 이 결정의 논리적 귀결이 "레포 루트에는 워크스페이스가 없다"이고, 그게 R-1(실행 위치 누락)의 원인이기도 하다.

**A-5. 파일 소유권 표 — 동의.** `docker-compose.yml`, `.env.example`, `rust-toolchain.toml`이 server 소유인 것에 이견 없다. `server/crates/history/**`가 history 소유인 것도 동의한다(이번 슬라이스에는 만들지 않으므로 충돌 없음).

### ADR-0002 (계약 형식·코드 생성)

**A-6. Rust 타입은 손으로 쓴다 (typify·schemars 탈락) — 강하게 동의.**

- `schemars`는 계약의 진실을 Rust 코드로 옮긴다. `contracts/`가 단일 진실이라는 전제와 정면 충돌한다. 탈락이 맞다.
- `typify`는 정수 `minimum`/`maximum`을 타입으로 강제하지 못한다. 우리 계약은 `MoneyMinor`·`Tick`·`probe_seq`처럼 **범위가 곧 의미**인 정수가 핵심이다. 범위를 못 지키는 생성기는 우리 문제를 풀지 못한다.
- 손으로 쓰는 대신 "레지스트리 주도 테스트"로 드리프트를 잡는다는 교환이 옳다. 타입이 2개인 지금은 물론이고, 50개가 되어도 테스트가 fixture 디렉토리를 순회하는 한 새 타입이 자동 포함된다.

**A-7. `jsonschema`를 오프라인으로 쓴다 — 결정 동의, 이유 문구는 정정 제안.**
결정(모든 스키마를 미리 등록하고 네트워크 없이 검증)은 전적으로 옳다. Context7로 확인한 API도 정확히 이 형태를 지원한다.

```rust
let registry = jsonschema::Registry::new()
    .add("https://schemas.starfall.invalid/contracts/common/primitives.schema.json", json)?
    .add(/* ... 나머지 전부 ... */)?
    .prepare()?;
let validator = jsonschema::options().with_registry(&registry).build(&schema)?;
```

각 스키마를 자기 `$id`로 등록하면 `primitives.schema.json#/$defs/UuidV7`, `../common/command-envelope.schema.json` 같은 **상대 `$ref`가 `$id` 기준으로 해석**된다. AC-5(a)는 실행 가능하다.

정정 제안 2가지:

- ADR-0002 §3의 "기본 features에는 HTTP/파일 리졸버가 들어 있어"는 부정확해 보인다. 공식 문서는 `resolve-http`·`resolve-file`을 **opt-in feature로** 소개한다. `default-features = false`라는 **결정은 그대로 유효**하지만(어차피 필요 없는 feature를 끄는 게 맞다), 이유를 "기본에 들어 있어서"가 아니라 "리졸버 feature를 명시적으로 켜지 않아 미등록 `$ref`가 실패로 드러나게 한다"로 바꾸는 게 사실에 맞다.
- 더 강한 수단이 하나 더 있다: **`jsonschema::options().offline()`** (0.52.0에서 추가, MSRV 1.85 — 우리는 1.98.1이라 사용 가능). 리트리버 자체를 거부 리트리버로 바꿔 어떤 원격 fetch도 막는다. `with_registry()`와 함께 쓰면 "네트워크로 나가지 않는다"가 feature 플래그가 아니라 **검증기 자체로 보장**된다. T2에서 둘 다 쓰겠다.

**A-8. envelope 필드는 항상 존재하고 값이 없으면 `null` (I-5) — 동의.**
이 규칙이 없으면 "필드 없음"과 "null"이 Rust `Option`과 C# nullable에서 다르게 해석되고, 그 차이가 재직렬화 왕복 테스트를 불가능하게 만든다. 즉 I-5는 스타일이 아니라 **AC-5(b)를 성립시키는 전제**다. R-10에서 이 연결을 명문화해 달라고 요청한다.

**A-9. 명령 envelope에 `player_id`/`actor_id`를 두지 않는다 (I-6) — 동의.** CLAUDE.md 원칙 1의 가장 싼 구현이다. 다만 이 불변식을 **스키마만** 지키면 운영 경로에서는 뚫린다 → R-5의 T-6.

**A-10. `invalid/`를 커버리지 수에서 제외 — 동의.** "통과만 확인하는 테스트는 검증기가 꺼져 있어도 통과한다"는 판단이 정확하다. 반례가 커버리지 숫자를 부풀리면 안 된다.

**A-11. 모든 정수를 ±(2^53−1) 안으로 — 동의.** 로그·대시보드·분석 도구가 JS를 거치는 순간 조용히 값이 바뀌는 사고를 미리 막는다. 화폐를 정수 최소 단위로 두는 결정과 같은 계열이다.

**A-12. `occurred_at`(게임 시간)과 `recorded_at`(실제 시간) 분리, `command_id`의 내장 타임스탬프를 신뢰하지 않음 — 동의.** 결정성(원칙 9)과 서버 권위(원칙 1)를 계약 수준에서 지킨다.

**A-13. 계약 검토 결과: 현재 7개 스키마의 `$id`가 모두 `https://schemas.starfall.invalid/contracts/{레포 상대 경로}` 규칙과 일치함을 손으로 확인했다.** 상대 `$ref`도 전부 올바른 대상을 가리킨다. fixture 개수도 스펙과 일치한다(유효 4, 무효 5). `PING_REPLY/with-correlation.json`의 `tick: 9007199254740991`(정확히 2^53−1)과 `probe_seq: 4294967295`(u32::MAX)는 경계값으로 잘 고른 값이다.

### ADR-0003 (로컬 개발 환경)

**A-14. `channel = "1.98.1"` 패치 고정 — 동의.** "새 clippy 린트가 `-D warnings` 게이트를 기능과 무관하게 깨뜨린다"는 건 실제로 자주 일어난다. 툴체인 상승을 의도된 작업으로 만드는 게 맞다.

**A-15. docker compose에 인프라만, 게임 서버는 `cargo run` — 동의.** Windows에서 Rust 컨테이너 재빌드 반복은 개발 반복을 망친다. 디버거·프로파일러 접근도 네이티브가 낫다. 컨테이너화는 배포 슬라이스에서.

**A-16. 이미지 패치까지 고정 — 동의. 두 태그 모두 존재 확인.** `docker manifest inspect postgres:18.6-trixie`, `redis:8.10.1-trixie` 모두 성공(pull 없이 매니페스트만 조회). 태그 오타로 T3가 막히는 일은 없다.

**A-17. 호스트 포트 15432 / 16379, `127.0.0.1` 바인딩 — 동의. 전제 확인 완료.**

```
TCP  0.0.0.0:5432  LISTENING  7612      ← 네이티브 postgres, ADR의 전제가 맞다
```

15432·16379·8080은 LISTENING이 없다. 8080(`STARFALL_HTTP_ADDR` 기본값)도 비어 있어 AC-2가 그대로 성립한다.

추가로 확인한 것: 이 Docker에는 **다른 프로젝트 컨테이너 5개**가 이미 떠 있다(`livingfeed-*`: 4222, 4223, 8222, 9001, 9002, 9200, 6333-6334). 우리가 쓰려는 포트와 겹치지 않는다. 다만 QA에게 전달할 운영 주의가 하나 생긴다 → R-4 말미.

**A-18. `.env.example`에서 `localhost`가 아니라 `127.0.0.1` — 동의.** Windows에서 `localhost`가 `::1`로 먼저 풀려 IPv4에만 바인딩된 Docker 포트에 못 붙는 건 실제 함정이다. `STARFALL_HTTP_ADDR=127.0.0.1:8080`도 같은 이유로 맞다.

**A-19. Redis 영속화 끄기 (`--save "" --appendonly no`) — 동의.** CLAUDE.md 원칙 3을 문서가 아니라 환경으로 강제한다. I-7("Redis를 지워도 다른 검증이 깨지지 않는다")이 실제로 테스트 가능해진다.

**A-20. Compose `version:` 키 없음, 최상위 `name: starfall` — 동의.** `name`은 이 PC에서 특히 중요하다(다른 프로젝트 컨테이너와 섞이지 않게).

**A-21. CI는 이 슬라이스에서 하지 않는다 — 동의.** 로컬에서 재현 가능한 명령을 확정하는 것이 선행이다. 다만 그 "명령"이 지금 작업 디렉토리 없이 적혀 있어 그대로 CI로 옮길 수 없다 → R-1.

---

## 수정 요청 (항목별: 무엇을 / 왜 / 대안)

### R-1 (차단) — cargo 명령의 작업 디렉토리가 어디에도 없다

**무엇을.** 스펙 §2-3·§2-4, AC-1, AC-2, ADR-0002 §6의 모든 cargo 명령에 실행 위치를 명시한다. 추가로 `cargo fmt --check` → `cargo fmt --all --check`, `cargo test --workspace` → `cargo test --workspace --locked`.

**왜.** ADR-0001 §4가 `tools/bots/`를 별도 워크스페이스로 두기로 했으므로 **레포 루트에 `Cargo.toml`이 없다**. 워크스페이스 루트는 `server/`다. 그런데 AC-1은 "Given 빈 체크아웃, When `cargo fmt --check` …"라고만 적혀 있다. 레포 루트에서 그대로 실행하면 `could not find 'Cargo.toml'`로 끝난다. QA(T10·T11)가 스펙 문구를 그대로 스크립트에 옮기면 첫 줄에서 실패한다. AC-2의 `cargo run -p starfall-game-server`도 같다.

`--all`: 가상 매니페스트 루트에서 `cargo fmt`의 대상 범위가 rustfmt 버전에 따라 달라진 전례가 있다. `--all`은 모호함을 없앤다.

`--locked`: ADR-0001 §2가 `Cargo.lock`을 추적하기로 했는데, `--locked` 없이는 lock이 조용히 갱신되어도 테스트가 통과한다. 즉 지금 상태로는 **`Cargo.lock` 추적 결정이 아무것에 의해서도 검증되지 않는다.**

**대안.** `cd server` 명시를 권한다. `cargo --manifest-path server/Cargo.toml ...`도 되지만 `-p`와 섞이면 사람이 헷갈린다. 어느 쪽이든 스펙 §2의 "개발자가 할 수 있어야 하는 일" 절차에 `cd server` 한 줄이 들어가는 게 가장 읽기 쉽다.

### R-2 (차단) — AC-10 "정수 타입이 3자 간 동일"은 현재 계약으로 성립할 수 없다

**무엇을.** AC-10의 문구를 바꾼다.

**왜.** 두 필드가 이미 반례다.

- `probe_seq`: 스키마는 `{"type":"integer","format":"int64","minimum":0,"maximum":4294967295}`. ADR-0002 §4의 매핑 규칙(`integer/format:int64` → `long`)을 따르면 C#은 `long`이 된다. 반면 스키마 description은 "(u32)"라 하고 T2 지시도 "`probe_seq`는 `u32`"라고 못박았다. 값의 범위는 같지만 **언어 타입은 다르다**. 즉 AC-10을 글자 그대로 검사하면 QA는 불일치 1건을 보고해야 하는데, 그건 설계대로 구현한 결과다.
- `tick`: 스키마는 `0..2^53−1`. Rust `u64`도 C# `long`도 그 상한을 넘는 값을 표현한다. "동일"이 무엇을 뜻하는지 정의되지 않는다.

**대안(권장).** AC-10을 이렇게 바꾼다.

> Then 필드 이름·필수 여부·널 가능이 3자 간 동일하고, 각 정수 필드의 언어 타입이 스키마의 `minimum`~`maximum`을 **모두 표현**하며, **범위를 벗어난 값을 거부**한다. 거부 가능 여부는 언어마다 다를 수 있으며, 어느 쪽이 무엇을 거부하는지는 §5의 `invalid/` 책임 표와 같은 형식으로 명시한다. 불일치 0건이 리포트에 기록된다.

이 문구는 R-5의 T-9(범위 거부 테스트)와 짝이 맞고, "Rust는 `u32`, C#은 `long`"이 버그가 아니라 기록된 설계 결정이 된다.

**대안(덜 권함).** 계약에서 `probe_seq`의 `"format": "int64"`를 빼고 `minimum`/`maximum`만 남긴다. 그러면 ADR-0002 §4에 "format 없는 integer" 매핑 규칙을 추가해야 해서 생성기와 ADR을 동시에 건드린다. 계약을 도구에 맞추는 방향이라 ADR-0002가 스스로 버린 사고방식에 가깝다.

### R-3 — `/readyz`는 유지하되, 구현 제약을 명시하고 AC-3를 한 줄 보강

**무엇을.** `/readyz`를 스펙에서 빼지 말 것. 대신 ADR-0003 §3 또는 스펙 §5에 구현 제약 4가지를 적고, AC-3에 복구 확인을 추가한다.

**왜 유지하는가.** architect가 "과하면 빼라"고 했는데, 구현자로서 **빼면 이 슬라이스의 목적이 절반 사라진다**고 본다.

- ADR-0003이 내린 위험한 결정은 세 가지다: 호스트 포트 15432, PG18 볼륨 경로, `.env`의 `127.0.0.1` 표기. `/readyz`가 없으면 이 셋을 검증하는 건 `docker compose ps`의 `healthy`뿐인데, 그건 **컨테이너 안에서** `pg_isready`가 도는 것만 증명한다. "Rust 프로세스가 `.env`의 자격 증명으로, 호스트 포트를 통해, IPv4로 붙을 수 있는가"는 전혀 증명되지 않는다. 그리고 Windows에서 실제로 깨지는 지점은 정확히 거기다.
- 부트스트랩 슬라이스의 존재 이유가 "다음 슬라이스가 기반 문제로 막히지 않게 하는 것"이다. 다음 슬라이스(마이그레이션·outbox)가 첫날 `DATABASE_URL`이 안 붙어서 멈추면 이번 슬라이스가 일을 안 한 것이다.
- 설정 배선(`DATABASE_URL`/`REDIS_URL` 읽기 → 풀 생성 → 에러 처리)은 어차피 다음 슬라이스에서 해야 한다. 지금 하면 **검증된 상태로** 넘어간다.

**의존성 비용은 통제 가능하다.** sqlx에 TLS feature를 켜지 않으면 rustls/ring 트리가 통째로 빠진다. 로컬 dev는 평문이다.

```toml
sqlx  = { version = "0.9", default-features = false, features = ["runtime-tokio", "postgres"] }
redis = { version = "1.7", default-features = false, features = ["tokio-comp"] }
```

`macros`·`migrate`·`uuid`/`chrono` 타입 feature는 이번에 켜지 않는다(빌드 시간). 원격 DB가 생기면 `tls-rustls-ring`을 추가하고 ADR을 갱신한다.

**그리고 비용을 의견이 아니라 숫자로 남기겠다**: T1 완료 시점(axum만)과 T3 완료 시점(sqlx·redis 추가)의 클린 빌드 시간을 각각 재서 `03_server_impl.md`에 적는다. 스펙 §8이 요구하는 "서버 클린 빌드 시간"이 마침 이 질문의 사후 증거가 된다. 차이가 감당 못 할 수준이면 그때 근거를 갖고 재논의하면 된다.

**구현 제약 4가지 (스펙/ADR에 적어 주기를 요청).**

1. **지연 연결.** `PgPoolOptions::new().max_connections(2).acquire_timeout(2s).connect_lazy_with(opts)`. DB가 꺼져 있어도 프로세스는 기동해야 한다 — AC-3 후반("서버는 죽지 않는다")이 이미 이걸 요구한다. 기동 시 연결을 강제하면 AC-3를 만족할 수 없다.
2. **점검마다 바운드 타임아웃, 두 점검은 동시에.** `tokio::time::timeout(2s, ...)` × 2를 `tokio::join!`로. `/readyz` 자체가 매달리면 readiness 신호가 아니라 장애가 된다. Redis 쪽은 `AsyncConnectionConfig::new().set_connection_timeout(..).set_response_timeout(..)`로 클라이언트 레벨 타임아웃도 같이 건다(API 확인함).
3. **응답 본문에 드라이버 에러 문자열·DSN·비밀번호를 넣지 않는다.** AC-3의 현재 문구는 `checks.postgres != "ok"`라서 어떤 문자열이든 통과한다. 개발 편의로 에러를 그대로 실으면 그 습관이 배포까지 간다. `checks.*`의 값을 **닫힌 집합 `"ok" | "unavailable"`** 로 고정하고, 원인은 `tracing` 로그에만 남긴다.
4. **`/healthz`와 `/readyz`의 용도를 섞지 않는다.** `/healthz`는 의존성을 보지 않는 liveness, `/readyz`는 트래픽 수용 가능 여부. 배포 슬라이스에서 `/readyz`를 재시작 트리거로 쓰면 DB 장애가 서버 재시작 루프가 된다. 이 구분을 지금 문서에 남겨 두자.

**AC-3 보강 제안(한 줄 추가).**

> When `docker compose start postgres` 후 서버를 **재시작하지 않고** `/readyz`를 다시 호출, Then 200으로 돌아온다.

이게 없으면 "한 번 실패한 풀이 영구히 고장 난 상태로 남는지"를 아무도 보지 않는다. 지연 연결 + `test_before_acquire`(sqlx 기본 true)가 실제로 동작하는지 증명하는 한 줄이고, 비용은 명령 한 번이다.

### R-4 — AC-4의 증거를 로그 문자열이 아니라 데이터 잔존으로 바꾸자

**무엇을.** AC-4의 Then을 "PostgreSQL 로그에 `initdb`가 다시 돌지 않는다"에서 데이터 잔존 + 익명 볼륨 확인으로 교체한다.

**왜.** 두 가지 문제가 있다.

- **로그 문자열 매칭은 이미지 버전에 종속된다.** 다음 패치에서 문구가 바뀌면 AC가 조용히 무의미해진다.
- **더 중요한 것**: 볼륨을 `/var/lib/postgresql/data`에 잘못 마운트해도 "매 기동마다 `initdb`"가 **항상 재현되지는 않는다**. 이미지가 `/var/lib/postgresql`에 `VOLUME`을 선언하고 있으면 익명 볼륨이 생기고, Compose는 컨테이너를 재생성할 때 이전 컨테이너의 마운트를 이어받는 경우가 있다. 즉 잘못된 설정이 `down` → `up -d` 한 사이클에서는 통과해 버릴 수 있다. **AC-4가 잡으려는 실패를 AC-4가 항상 잡는다는 보장이 없다.**

**대안(권장).**

> When `docker compose exec -T postgres psql -U starfall -d starfall -c "create table _boot_marker(id int)"` → `docker compose down` (**`-v` 없이**) → `docker compose up -d` → 같은 테이블 조회, Then 테이블이 남아 있다. 그리고 `docker volume ls`에 이 프로젝트의 **익명 볼륨(64자 해시 이름)이 새로 생기지 않는다**.

데이터 잔존은 마운트 경로가 맞다는 직접 증거이고, 익명 볼륨의 유무는 "선언된 `VOLUME`이 명명 볼륨에 덮이지 않았다"의 직접 신호다. 둘 다 이미지 문구에 의존하지 않는다.

추가로 T3에서 `docker image inspect postgres:18.6-trixie`로 `PGDATA`와 `Config.Volumes`를 직접 읽어 `03_server_impl.md`에 기록하겠다(지금은 pull하지 않아 미확인 — "미확인 사실" 참조).

**QA에게 전달 요망(운영 주의).** 이 PC의 Docker에는 다른 프로젝트 컨테이너 5개(`livingfeed-nats`, `minio`, `qdrant`, `opensearch`, `nats-test`)가 돌고 있다. T10 스모크 스크립트와 AC-4 검증에서 **`docker system prune`, `docker volume prune`, 프로젝트 밖 `down -v`를 절대 쓰지 말 것.** ADR-0003의 `name: starfall` 덕분에 `docker compose -p starfall ...` 범위의 명령은 안전하다. 스펙 AC-4의 "깨끗한 Docker 상태"라는 표현이 전역 정리로 읽힐 수 있어 문구를 "starfall 프로젝트 볼륨이 없는 상태"로 좁혀 주면 좋겠다.

### R-5 — 계약 테스트 5종은 부족하다. 9종으로

5종의 골격은 옳다(레지스트리 주도, fixture 순회, 반례 포함). 그런데 **운영 경로를 아무도 보지 않는다**는 큰 구멍이 하나 있다. 추가 4종은 전부 계약 변경 없이 넣을 수 있다.

**T-6 (가장 중요) — `invalid/` fixture에 대한 serde 거부표.**
각 `invalid/` fixture를 해당 **Rust 타입으로 역직렬화 시도**하고, 기대 결과표와 대조한다.

*왜*: 운영 중 들어오는 명령을 막는 건 스키마 검증기가 아니라 **serde**다. 모든 WebSocket 메시지에 2020-12 검증을 돌릴 수는 없다. 그런데 현재 5종 중 `invalid/`를 보는 건 테스트 3뿐이고 그건 **스키마만** 본다. 스키마는 거부하는데 serde는 통과시키는 조합이 생기면, I-6("클라이언트는 행위자를 주장하지 않는다")은 문서상 불변식일 뿐 실제 서버는 뚫려 있다. 스펙 §5가 C# 책임 표를 만든 것과 **정확히 같은 이유로 Rust 열이 필요하다.**

*목표*: 현재 5건 전부 serde도 거부. 달성 조건은 R-10의 타입 설계(`deny_unknown_fields`, `UuidV7` newtype, `u32`).

*스펙 §5 표에 "Rust serde(운영 경로)" 열을 추가해 달라.* 스키마 열과 serde 열이 다른 칸이 생기면 그건 그 자체로 기록할 가치가 있는 사실이다.

**T-7 — required 필드 변이 테스트.**
유효 fixture에서 스키마의 `required` 목록을 **하나씩 제거**해 serde 역직렬화가 전부 실패하는지 본다.

*왜*: 테스트 2(왕복)는 fixture에 있는 필드만 본다. 스키마가 required인데 Rust가 `Option<T>`로 선언한 드리프트는 **fixture가 항상 그 필드를 갖고 있으므로 영원히 드러나지 않는다.** 타입이 2개인 지금은 눈으로 봐도 되지만, 20개가 되면 못 본다. 레지스트리 주도라 새 타입에 자동 적용된다.

*비용*: 스키마에서 `required` 배열을 읽어 필드를 지우는 루프 하나.

**T-8 — 레지스트리 자체 검증 + 경로·`$id` 정합.**
(a) `registry/types.json`을 `registry/types.schema.json`으로 검증한다. (b) 각 엔트리의 `schema` 경로에 파일이 실제로 존재한다. (c) 모든 스키마의 `$id`가 `https://schemas.starfall.invalid/contracts/{레포 상대 경로}`와 일치한다.

*왜*: 현재 5종 중 (a)를 하는 것이 없다. 테스트 1은 "`*.schema.json`이 메타스키마로 유효"하므로 `types.schema.json`은 검사하지만, **데이터 파일인 `types.json`은 아무도 검사하지 않는다.** architect가 만든 레지스트리 스키마가 실제로는 한 번도 쓰이지 않는 셈이다. (c)는 오프라인 등록의 전제다 — `$id`와 경로가 어긋나면 등록은 성공하고 `$ref`는 **조용히 엉뚱한 문서**를 가리킨다. 오늘 기준 7개 전부 일치함을 손으로 확인했지만, 그 상태를 테스트가 지켜야 한다.

**T-9 — 정수 상한 거부.**
`tick = 9007199254740992`(2^53), `probe_seq = 4294967296`이 Rust 역직렬화에서 **실패**하는지.

*왜*: T2 지시가 "범위를 타입으로 좁히고 넘는 값은 역직렬화에서 실패해야 한다"고 적었는데, 5종 중 이를 보는 테스트가 없다. 현재 `invalid/` 5건은 구조(누락·미지 필드·패턴·음수)만 보고 **상한 위반이 하나도 없다.** 상한을 넘긴 정수가 조용히 통과하는 것은 나중에 `MoneyMinor`에서 그대로 화폐 버그가 된다. 계약 변경 없이 Rust 단위 테스트로 넣을 수 있다(C#까지 덮으려면 R-9).

### R-6 — ADR-0002 §3.5의 "`server`/`history` 태그"를 p0-01에서는 `server`만으로

**무엇을.** ADR-0002 §3 테스트 5의 "레지스트리에 `server`/`history` 태그가 있는 타입은…"을 "해당 크레이트가 존재하는 태그에 대해서만 검사한다. p0-01에서는 `server`만"으로.

**왜.** ADR-0001 §2 변경 2에 따라 `starfall-history` 크레이트는 **이번에 만들지 않는다**. `history` 태그를 검사 대상에 두면 규칙이 자기 모순이다. 현재 레지스트리에 `history` 태그 타입이 없어 당장 실패하지는 않지만, 역사 슬라이스에서 이 문장을 근거로 엉뚱한 크레이트에 대응표를 만들게 될 수 있다. 참고로 T2 지시와 AC-5(e)는 이미 `server`만 적고 있어, **ADR 쪽만 넓다** — 셋을 맞추자.

### R-7 — AC-5(e)의 "`server` 태그" 정의가 모호하다

**무엇을.** "`producers` 또는 `consumers`에 `server`가 포함된 타입"으로 명시.

**왜.** 현재 레지스트리에서 `PING_SERVER`는 `consumers: ["server"]`, `PING_REPLY`는 `producers: ["server"]`다. "`server` 태그가 있는 타입"을 `producers`만으로 읽으면 `PING_SERVER`가 검사에서 빠지고, `consumers`만으로 읽으면 `PING_REPLY`가 빠진다. 서버는 **둘 다** 다루므로 합집합이 맞다. 한 글자 차이로 검사 범위가 절반이 된다.

### R-8 — AC-2·AC-3의 `curl`은 PowerShell에서 동작하지 않는다

**무엇을.** 명령을 `curl.exe`로 표기하거나, 해당 AC에 실행 셸을 명시한다.

**왜.** 이 PC에서 확인했다.

```
PS> (Get-Command curl).CommandType  →  Alias        # Invoke-WebRequest의 별칭
$ where curl → C:\Windows\System32\curl.exe, C:\Program Files\Git\mingw64\bin\curl.exe (8.11.0)
```

AC-2의 `curl -s -o - -w "%{http_code}"`는 Git Bash에서는 동작하지만 Windows PowerShell에서는 `Invoke-WebRequest`로 해석되어 `-w` 같은 인자가 없어 실패한다. 그런데 **T10 지시는 "Windows PowerShell에서 도는지 확인한다"**고 되어 있다. 지금 상태로는 QA가 스펙 명령을 그대로 옮기면 실패하고, 그 실패는 서버 문제로 오인되기 쉽다.

**대안.** `curl.exe`로 표기(두 경로 모두 존재하므로 PATH 문제 없음). 또는 PowerShell 쪽은 `Invoke-WebRequest -SkipHttpErrorCheck`를 쓰도록 스펙에 적는다. AC-3는 503을 기대하므로 **`-SkipHttpErrorCheck` 없이는 PowerShell에서 예외가 난다** — 이 부분도 T10에 미리 알려주는 게 좋겠다.

### R-9 (계약 변경 요청 — architect 결정 필요, 선택) — 범위 위반 `invalid/` fixture 2건

**무엇을.** 다음 2개 추가를 요청한다(내가 직접 만들지 않았다 — 계약은 architect 소유).

- `contracts/fixtures/PING_REPLY/invalid/tick-above-safe-integer.json` — `tick: 9007199254740992`
- `contracts/fixtures/PING_SERVER/invalid/probe-seq-above-u32.json` — `probe_seq: 4294967296`

**왜.** 현재 무효 fixture 5건은 구조 위반(필드 누락, 미지 필드, 패턴 불일치, 음수)만 본다. **상한 위반을 보는 반례가 하나도 없다.** 그런데 이 프로젝트에서 정수 상한은 장식이 아니다 — `MoneyMinor`, `Tick`, `SafeInteger`가 전부 "범위가 곧 의미"인 타입이고, 화폐 복사 버그의 전형적 입구다. 지금 2개 파일로 그 계열의 첫 회귀 테스트를 만들 수 있고, 타입이 2개뿐인 지금이 가장 싸다.

**영향(architect가 함께 고쳐야 할 곳).** 스펙 §5의 "총 5개" → 7개, AC-5(c) "`invalid/` 5건" → 7건, §5 책임 표에 2행 추가. 두 건 모두 C# 열은 "감지 불가 → 제외"다(Newtonsoft가 `long`에 4294967296을 문제없이 넣는다).

**받아들이지 않아도 차단은 아니다.** T-9(Rust 단위 테스트, 인라인 리터럴)로 서버 쪽은 막는다. 다만 그 경우 **C# 쪽은 같은 보장을 못 받고**, 그 차이가 AC-10 리포트에 남아야 한다.

### R-10 — "재직렬화 = 원본"의 달성 조건 4가지를 문서에 명문화해 달라

**무엇을.** ADR-0002 §3 테스트 2 또는 스펙 AC-5(b)에 다음 4줄을 추가한다.

**왜.** architect의 질문 3에 대한 답: **실현 가능하다. 단 조건부다.** 그리고 그 조건이 지금 어디에도 없어서, 구현자가 자연스럽게 선택하는 흔한 방식 두 가지가 이 테스트를 **반드시** 깨뜨린다. 문서에 없으면 구현 → 실패 → 디버깅 → 뒤늦게 발견의 경로를 밟는다.

1. **비교는 문자열이 아니라 `serde_json::Value`로 한다.** → 필드 순서 문제가 소멸한다. (serde_json의 Map은 기본 `BTreeMap`이라 정렬되고, `preserve_order`를 켜도 `IndexMap`의 동등성은 순서 무관이다. 어느 쪽이든 안전하다.) 스펙이 이미 "의미적으로 동일"이라고 적은 건 옳다 — 다만 "의미적"의 구현이 `Value` 비교임을 못박아야 누가 문자열 비교로 구현하지 않는다.
2. **널 가능 envelope 필드에 `skip_serializing_if`를 쓰지 않는다.** `Option<T>` + `#[serde(skip_serializing_if = "Option::is_none")]`은 Rust에서 거의 반사적으로 붙이는 속성인데, 이게 붙으면 `client_sent_at: null`이 재직렬화에서 **사라진다**. 그 순간 I-5(envelope 필드는 항상 존재)가 깨지고 왕복이 실패하며, 더 나쁘게는 클라이언트가 "필드 없음"을 받게 된다. **이 한 줄이 이 테스트의 실제 함정이다.**
3. **`RealTime`·`GameTime`을 시간 타입으로 매핑하지 않는다.** `chrono::DateTime<Utc>`는 재직렬화 시 소수 자리를 0/3/6/9로 정규화한다(`.1` → `.100`). 계약의 `RealTime` 패턴은 1~9자리를 허용하므로 왕복이 깨진다. **검증하는 문자열 newtype**으로 간다. (C#도 ADR-0002 §4 매핑상 `string`이 되므로 양쪽이 일치한다. C#의 `DateParseHandling.None` 요구와 같은 문제의 Rust판이다.)
4. **정수는 폭이 정확한 타입 + 범위 검증 newtype으로.** `f64`가 끼면 왕복이 깨진다. 계약에 실수가 없으므로 오늘은 문제가 없고, **"계약에 float 금지"가 이 테스트의 전제**임을 적어 두면 나중에 누가 좌표를 float로 넣으려 할 때 근거가 된다.

**선택 필드에 대해.** 현재 계약에는 선택 필드가 **하나도 없다**(모든 payload 필드가 `required`). 그래서 p0-01에서는 2번만 지키면 된다. 다만 첫 선택 필드가 들어오는 순간 "필드 없음 vs null"이 문제가 되고, 그때는 ADR-0002 §1의 "payload 필드만 선택을 쓴다"와 짝이 되는 Rust 규칙(선택 필드에만 `skip_serializing_if` 허용)이 필요하다. 지금 한 줄로 적어 두는 게 가장 싸다.

### R-11 — ADR-0001 §2에 gateway 오염 방지 한 줄

**무엇을.** ADR-0001 §2 "결과"에 추가: "`starfall-gateway`에는 게임 규칙과 월드 상태를 두지 않는다. 첫 게임 상태 코드가 `starfall-domain`을 만든다."

**왜.** "빈 크레이트를 만들지 않는다"에 동의하지만, 그 결과 p0-01에 존재하는 유일한 로직 크레이트가 `starfall-gateway`가 된다. 다음 슬라이스에서 게임 규칙을 넣을 가장 쉬운 자리가 이미 존재하는 gateway이고, 그게 가장 쉬운 길이다. 그러면 ADR이 컴파일러로 강제하겠다던 `domain ← sim ← gateway` 방향이 **문서에만 남는다**(ADR이 "단일 크레이트 서버"를 버린 이유가 정확히 이것이다). 한 줄이면 다음 슬라이스의 첫 리뷰에서 근거가 된다.

---

## 수용 기준 검토 (증명 가능한가, 문구 제안)

| AC | 증명 가능? | 판단 |
|----|-----------|------|
| AC-1 | 조건부 | 작업 디렉토리 없으면 **실행 불가**(R-1). `--all`·`--locked` 추가 시 증명력 상승 |
| AC-2 | 조건부 | 셸에 따라 `curl`이 실패(R-8). 그 외에는 명확. 8080 비어 있음 확인 |
| AC-3 | 조건부 | 증명 가능. `checks` 값 닫기 + 복구 확인 한 줄 추가 권장(R-3) |
| AC-4 | **부분적** | 잡으려는 실패를 항상 잡는다는 보장이 없다 → 증거 방식 교체(R-4) |
| AC-5 | 예 | (a)~(e) 모두 실행 가능 확인. 단 (b)는 4가지 제약 전제(R-10), (e) 문구 모호(R-7), 5종→9종 권장(R-5) |
| AC-6 | client | — |
| AC-7 | client | (a)의 왕복 조건은 Rust와 같은 함정을 공유 — client에게 R-10 (2)(3) 전달 요망 |
| AC-8 | client | — |
| AC-9 | 예 | server 쪽 전제 충족 예정 (아래) |
| AC-10 | **아니오** | 현재 문구로는 성립 불가 → R-2 |
| AC-11 | qa/architect | server 쪽 전제 1건 (아래) |

**AC-5 보충.** (a)는 `jsonschema::Registry::new().add(...).prepare()` + `options().with_registry(&r).offline()`로 실행 가능함을 문서로 확인했다. (b)의 "의미적으로 동일"은 달성 가능하지만 R-10의 4조건이 전제다. (c)는 현재 5건에 대해 architect가 이미 실행으로 확인했다고 기록되어 있다 — T2 착수 첫 단계에서 내가 재현하고 결과를 남기겠다. 특히 `actor-field-injected`의 거부는 `unevaluatedProperties`가 `allOf` 애너테이션을 제대로 수집해야만 성립하므로, **거기부터 확인**한다(그게 안 되면 계약 작성 스타일 전체가 흔들린다).

**AC-9 (커버리지 `--strict`) — server 쪽 전제.** 스크립트가 코드에서 타입 이름 문자열을 찾는 방식이므로, 손으로 쓴 serde 타입에 `"PING_SERVER"` / `"PING_REPLY"` 리터럴이 **계산되지 않은 형태로** 남아야 한다. 타입 태그를 `concat!`이나 매크로 조합으로 만들면 문자열 검색에 안 걸릴 수 있다. T2에서 리터럴 상수로 박겠다. (이건 요청이 아니라 내가 지킬 사항이다.)

**AC-11 — architect에게 전달할 `.gitignore` 관련 2건.**

- 제외해야 할 것: `/server/target/`, `/.env`, `client/Library/`, `client/Temp/`, `client/Logs/`, `client/obj/`.
- **제외하면 안 되는 것: `server/.sqlx/`.** sqlx 오프라인 메타데이터는 **커밋 대상**이다(DB 없이 빌드되게 하는 근거, `rust-authoritative-server` §5). 이번 슬라이스에는 쿼리가 없어 생성되지 않지만, `.gitignore`에 `.sqlx`류 패턴이 들어가면 다음 슬라이스에서 조용히 빠진다. 지금 한 번 확인해 두면 된다.

**스펙 §8 (비기능) 관련.** "서버 클린 빌드 시간"을 **두 번** 재겠다 — T1 직후(axum만)와 T3 직후(sqlx·redis 추가). 이 두 숫자가 architect의 질문 2("`/readyz`가 과한가")에 대한 사후 증거가 된다.

**스펙 §2 흐름 관련.** 2번(`docker compose up -d`) 앞에 "첫 실행 시 이미지 pull에 수 분이 걸린다"는 주석이 있으면 QA가 타임아웃으로 오판하지 않는다. 마찬가지로 1번 앞에 "레포 안에서 첫 cargo 실행 시 툴체인 1.98.1을 내려받는다"(A-3 참조).

---

## 내 태스크 실행 계획 요약 (순서와 예상 위험)

**순서.** T1 → T2 → T3 → T4. T2를 T3보다 먼저 두는 이유: 계약 크레이트는 외부 의존(Docker·DB)이 없어 환경 문제와 무관하게 끝낼 수 있고, AC-5가 이 슬라이스의 핵심 증명이기 때문이다. T3는 이미지 pull·볼륨 등 환경 변수가 가장 많아 마지막에 둔다.

### T1 — 워크스페이스 + 툴체인 + `/healthz`

1. 루트 `rust-toolchain.toml` (`1.98.1`, `rustfmt`·`clippy`, `minimal`).
2. `server/Cargo.toml`: `[workspace]` + `[workspace.dependencies]`(버전 한 곳) + `[workspace.lints]`.
3. `starfall-gateway`(Router 조립, `/healthz`), `starfall-game-server`(env 설정, `tracing` 초기화, `axum::serve`, graceful shutdown).
4. 착수 직전 Context7로 axum 0.8.9 문서 확인(Router·serve·State 시그니처). 현재 최신은 axum **0.8.9**, tokio **1.53.1**, tower-http **0.7.1**, tracing-subscriber **0.3.23**, thiserror **2.0.20**.

*위험 1 (확실).* 레포 안 첫 cargo 실행이 툴체인 `1.98.1-x86_64-pc-windows-msvc`를 새로 내려받는다(설치된 것은 `stable`뿐). 오프라인이면 여기서 막힌다.

*위험 2.* `[workspace.lints]`는 **각 멤버가 `[lints] workspace = true`로 옵트인**해야 적용된다. 빠뜨리면 린트가 조용히 꺼진 채로 clippy가 통과한다. 크레이트를 추가할 때마다 확인하고, `03_server_impl.md`에 체크 항목으로 남긴다.

*위험 3.* T1 지시의 "요청 처리 경로에 `unwrap()`/`expect()` 금지"를 clippy로 강제하면(`clippy::unwrap_used`) 테스트에서도 걸린다. `clippy.toml`/`#[allow]`로 테스트만 예외 처리하고, **조정 사실을 `03_server_impl.md`에 명시**한다(T1 지시가 요구한 대로).

### T2 — `starfall-contracts`

1. primitives newtype부터: `UuidV7`(v7 아니면 역직렬화 거부), `RealTime`/`GameTime`(검증하는 문자열 newtype — chrono 금지, R-10 (3)), `Tick`/`SafeInteger`/`NonNegativeSafeInteger`(상한 검증), `ProbeSeq = u32`.
2. 타입별 구조체. **envelope을 `#[serde(flatten)]`으로 공유하지 않는다.**
   *이유(중요)*: serde의 `deny_unknown_fields`는 `flatten`과 함께 **지원되지 않는다**. 내부 태그 열거형(`#[serde(tag = "command_type")]`)도 같은 버퍼링 경로라 마찬가지다. 둘 중 하나라도 쓰면 `actor-field-injected`를 **스키마는 거부하는데 서버는 받아들인다** — I-6이 운영 경로에서 무너진다. 그래서 타입마다 envelope 5개 필드를 펼쳐 쓰고 `#[serde(deny_unknown_fields)]`를 건다. 타입이 늘면 선언 매크로로 반복을 줄이되 `deny_unknown_fields`는 유지한다. `command_type` 디스패치는 `deny`를 걸지 않은 별도의 작은 peek 구조체로 한다(p0-02에서 필요).
3. 스키마 로더: `env!("CARGO_MANIFEST_DIR")` 기준 상대 경로로 `contracts/`를 읽고 `$id`로 `Registry`에 등록 → `options().with_registry(&r).offline()`.
4. 테스트 1~5 + R-5의 T-6~T-9.

*위험 1.* `unevaluatedProperties`가 기대대로 동작하는지. architect가 확인했다고 기록되어 있으나 내가 재현하지 않았다. **T2의 첫 작업으로 이것만 먼저 검증**하고, 안 되면 즉시 architect에게 알린다(계약 작성 스타일 전체가 여기에 걸려 있다).

*위험 2.* `contracts/` 경로가 `CARGO_MANIFEST_DIR`에서 `../../../contracts`다. 디렉토리가 한 단계라도 옮겨지면 깨진다. 경로 해석 실패 시 "contracts를 찾을 수 없다"는 명확한 메시지로 죽게 한다(조용히 0개 순회하고 통과하면 최악이다). **fixture 개수가 0이면 테스트 실패**로 못박겠다.

*위험 3.* `uuid` 1.26.1의 역직렬화는 대문자·중괄호 표기도 받는다. 재직렬화가 소문자 하이픈인지 확인하는 테스트를 넣는다(T2 지시대로).

### T3 — docker compose + `.env.example` + `/readyz`

1. `docker-compose.yml`(`name: starfall`, `version:` 없음, 명명 볼륨을 `/var/lib/postgresql`에, `127.0.0.1:${...}` 바인딩, healthcheck 2종, Redis 영속화 off).
2. `.env.example`(ADR-0003 §4 그대로).
3. `/readyz`: sqlx + redis, R-3의 제약 4가지 적용.
4. 착수 시 `docker image inspect postgres:18.6-trixie`로 `PGDATA`·`Config.Volumes` 실측 → `03_server_impl.md`에 기록.

*위험 1.* 첫 `docker compose up -d`가 이미지를 pull한다(postgres trixie는 수백 MB). 시간이 걸릴 뿐 차단은 아니다.

*위험 2.* **sqlx 0.9.0은 갓 나온 메이저 변경이다.** 0.8 계열이 훨씬 검증되어 있다. 릴리스 노트를 읽고 파괴적 변경이 크면 **0.8.x로 핀**한다 — 부트스트랩 슬라이스가 라이브러리 마이그레이션을 떠안을 이유가 없다. 어느 쪽이든 `[workspace.dependencies]`에 정확한 버전으로 고정하고 선택 이유를 `03_server_impl.md`에 남긴다.

*위험 3.* `/readyz`가 처음부터 503을 주는 경우(포트·자격 증명·IPv6). 이게 바로 이 엔드포인트를 만드는 이유이므로 "위험"이라기보다 기대 산출이다. 원인은 `tracing` 로그로만 노출하고 응답 본문에는 넣지 않는다(R-3 제약 3).

*위험 4.* 다른 프로젝트 컨테이너 5개와 공존. 전역 prune 금지(R-4 말미).

### T4 — `03_server_impl.md`

구현 요약, 실행 방법(작업 디렉토리 포함), 계약 대응표(레지스트리 이름 → Rust 타입), 빌드 시간 2회 측정치, clippy 조정 사실, 미검증 항목.

### Redis 크레이트 선택 (architect 질문 5의 답)

`redis` (redis-rs) **1.7.0**, `default-features = false, features = ["tokio-comp"]`, 연결은 `redis::aio::ConnectionManager`.

- **왜 `redis`인가.** 1.0에 도달해 API 안정성을 약속했다. async 지원이 feature 하나(`tokio-comp`)로 끝난다. `get_multiplexed_async_connection_with_config`로 연결·응답 타임아웃을 지정할 수 있고(문서 확인), `ConnectionManager`는 clone이 싸고 자동 재연결을 제공해 `/readyz`와 이후 세션·레이트 리밋 용도에 **그대로 재사용**된다. 우리에게 필요한 건 딱 이만큼이다.
- **왜 `fred` 10.1.0이 아닌가.** 클러스터·센티넬·RESP3·내장 백오프 정책 등 능력은 더 크지만, ADR과 CLAUDE.md 원칙 3에서 Redis는 **"지워도 되는" 계층**이다(I-7). 없어도 되는 데이터에 큰 의존성과 큰 설정 표면을 사는 건 원칙 7("모듈형 모놀리스부터")과도 어긋난다. 클러스터·센티넬이 실제로 필요해지면 그때 ADR로 재검토한다.
- **왜 `deadpool-redis` 0.23 / `bb8-redis` 0.26이 아닌가.** 멀티플렉스 연결이 이미 동시 명령을 한 연결에 파이프라인한다. 풀을 얹으면 크레이트와 실패 모드만 늘고 얻는 게 없다. 단, 나중에 `BLPOP` 같은 블로킹 명령이나 pub/sub이 필요해지면 그건 **전용 연결**을 따로 만드는 문제이지 풀 문제가 아니다(멀티플렉스 연결에서 블로킹 명령을 돌리면 다른 명령이 막힌다 — 그때 이 경계를 명시적으로 나눈다).
- **TLS 없음.** 로컬 `127.0.0.1` 평문. 원격 Redis가 생기면 feature 추가 + ADR 갱신.
- **주의로 기록해 둘 것**(문서에서 확인): 멀티플렉스 연결의 clone은 채널을 공유하므로 **동시 트랜잭션(`WATCH`/`MULTI`/`EXEC`)이 서로 끼어들 수 있다.** 우리는 Redis에서 트랜잭션을 쓸 계획이 없지만(트랜잭션은 PostgreSQL의 일), 누군가 레이트 리밋을 `WATCH`로 구현하려 할 때를 대비해 `03_server_impl.md`에 남기겠다.

---

## 미확인 사실

정직하게 분리한다. 아래는 **추측하지 않고 확인하지 못한 것**이다.

1. **PG18 이미지의 `PGDATA`·`VOLUME` 경로.** ADR-0003 §3의 "`PGDATA = /var/lib/postgresql/18/docker`, `VOLUME = /var/lib/postgresql`"를 직접 확인하지 못했다. 이미지를 pull하지 않았기 때문이다(매니페스트로 **태그 존재만** 확인). T3 착수 시 `docker image inspect postgres:18.6-trixie`로 읽어 기록한다. **볼륨을 `/var/lib/postgresql`에 마운트한다는 결정 자체에는 동의한다** — 틀렸을 경우의 비용이 비대칭적으로 크기 때문이다.
2. **잘못 마운트했을 때의 실패 양상.** ADR은 "에러 없이 매 기동마다 `initdb`가 돈다"고 단정하지만, Compose가 재생성 시 이전 컨테이너의 익명 볼륨을 이어받는 경로가 있어 **항상 그렇게 나타나지는 않을 수 있다**. 이것이 R-4(증거 방식 교체)의 근거다. 실제 양상은 T3에서 실측해 기록한다.
3. **sqlx 0.9.0의 0.8 대비 파괴적 변경.** 릴리스 노트를 읽지 않았다. T3 착수 시 확인하고 0.8.x 핀 여부를 결정한다.
4. **axum 0.8.9의 정확한 API.** 아직 Context7로 확인하지 않았다(T1 착수 직전에 한다). 0.8에서 경로 파라미터 문법이 바뀐 것으로 알려져 있으나 이번 슬라이스에는 파라미터 라우트가 없다.
5. **`jsonschema` 0.56의 `unevaluatedProperties` 지원.** ADR-0002 "검증한 사실 1"에 architect가 `invalid/` 5건 전부 거부를 확인했다고 적혀 있다. 나는 직접 실행하지 않았다. `actor-field-injected`의 거부는 `unevaluatedProperties`가 `allOf` 애너테이션을 수집해야만 성립하므로, **T2의 첫 작업으로 재현**한다.
6. **`jsonschema`의 기본 feature 구성.** 공식 문서상 `resolve-http`·`resolve-file`은 opt-in으로 보이나, 0.56의 `[features]` 섹션을 직접 읽지 않았다. **결정(`default-features = false`)에는 영향 없고**, ADR의 *이유 문구*만 정정 대상이다(A-7).
7. **Unity·codegen·C# 관련 전부.** 내 범위가 아니다. 단 R-10의 (2)(3)은 C#에서도 같은 형태로 재현되는 함정이므로(`NullValueHandling.Ignore`, `DateParseHandling`) client에게 전달되기를 요청한다.
8. **`_workspace/STATUS.md`의 현재 내용.** 읽지 않았다(이번 검토 범위 밖).

### 이번에 실제로 실행해 확인한 것

| 확인 | 결과 |
|------|------|
| `cargo` / `rustc` / clippy / rustfmt | 1.98.1 / 1.98.1 / 0.1.98 / 1.9.0-stable — ADR-0003 표와 일치 |
| `rustup toolchain list` | `stable-x86_64-pc-windows-msvc` **하나뿐** → `1.98.1` 핀은 새 다운로드를 유발 |
| Docker / Compose | 27.3.1 / v2.29.7-desktop.1 — 실행 중 |
| 포트 5432 | `0.0.0.0:5432 LISTENING PID 7612` — ADR-0003의 전제 확인 |
| 포트 15432 / 16379 / 8080 | 전부 비어 있음 |
| 타 프로젝트 컨테이너 | `livingfeed-*` 5개 가동 중 (4222·4223·8222·9001·9002·9200·6333-6334) — 우리 포트와 무충돌, 전역 prune 금지 |
| `postgres:18.6-trixie` / `redis:8.10.1-trixie` | 매니페스트 조회 성공 = 태그 존재 |
| PowerShell `curl` | **Alias** (→ `Invoke-WebRequest`). `curl.exe`는 System32·Git 양쪽에 존재(8.11.0) |
| git | 2.47.1.windows.1, `core.autocrlf=true`, **커밋 0건**(`main`에 커밋 없음) — I-8 현재 만족 |
| crates.io 최신 버전 | axum 0.8.9 / tokio 1.53.1 / sqlx 0.9.0 / redis 1.7.0 / jsonschema 0.56.0 / uuid 1.26.1 / serde_json 1.0.151 / tower-http 0.7.1 / tracing-subscriber 0.3.23 / thiserror 2.0.20 / fred 10.1.0 / deadpool-redis 0.23.1 / bb8-redis 0.26.0 |
| `contracts/` 손 검토 | 스키마 7개의 `$id`가 전부 경로 규칙과 일치, 상대 `$ref` 전부 올바른 대상, 유효 fixture 4 / 무효 5 (스펙과 일치) |
