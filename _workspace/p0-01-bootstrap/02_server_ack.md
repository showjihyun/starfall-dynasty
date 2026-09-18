# server 스프린트 계약 확인 (p0-01-bootstrap)

- 확인자: server (rust-server-engineer), 2026-09-18
- 대상: `02_sprint_contract.md` SC-01 ~ SC-18 + 부록(§0.4 층별 표, §2 비교표 대상, §4 미검증 기준)
- 결론: **차단급 이의 없음. 그대로 진행한다.** 이의 2건은 모두 *증거 수집 방법*의 문제이고, 판정 대상 항목 자체는 바꿀 필요가 없다. 두 건 다 이 PC의 실제 상태로 재현해 아래에 근거를 적었다.
- `01_architect_decisions.md` 확인: 내 리뷰 R-1 ~ R-11 이 **전부 수용**됐다. 거부·부분 수용 항목 없음. 반영 위치도 확인했다.

## 1. 영역별 확인

| 영역 | 항목 | 확인 | 비고 |
|------|------|------|------|
| A 빌드·품질 | SC-01 ~ SC-03 | ☑ 동의 | SC-03 보조 증거 명령에 이의 1 |
| B 기동·엔드포인트 | SC-04 ~ SC-08 | ☑ 동의 | 전부 실행으로 통과 확인 |
| C 로컬 인프라 | SC-09 ~ SC-11 | ☑ 동의 | SC-11 판정 방법에 이의 2 |
| D 계약 ↔ Rust | SC-12 ~ SC-18 | ☑ 동의 | QA 권장 테스트 이름을 그대로 채택 |
| 부록 §0.4 층별 표 | 반례 7건 × 3층 | ☑ 동의 | **Rust serde 열 7건 전부 "거부"가 실제 결과와 일치**했다 |
| 부록 §2 비교표 대상 | 필드 목록·정수 3행·널 가능 2행 | ☑ 동의 | Rust 쪽 구현이 표와 일치한다(아래 §3) |
| 부록 §4 미검증 기준 | — | ☑ 동의 | "환경이 없어서"와 "구현이 없어서"를 나누는 것이 맞다 |

### §0.4 층별 표 — 구현 결과 대조

`01_architect_decisions.md` §5가 "구현 결과가 표와 다르면 표를 고치지 말고 알리라"고 했다. **다른 칸이 없었다.** 표를 고칠 사유 없음.

| # | fixture | ② Rust serde 기대 | 실제 | 거부한 메커니즘 |
|---|---------|------------------|------|----------------|
| 1 | `actor-field-injected.json` | 거부 | 거부 | `unknown field 'player_id'` (`deny_unknown_fields`) |
| 2 | `command-id-not-v7.json` | 거부 | 거부 | `UuidV7 는 버전 7이어야 한다 (받은 값의 버전: 4)` |
| 3 | `probe-seq-negative.json` | 거부 | 거부 | `invalid value: integer '-1', expected u32` |
| 4 | `probe-seq-above-u32.json` | 거부 | 거부 | `invalid value: integer '4294967296', expected u32` |
| 5 | `missing-tick.json` | 거부 | 거부 | `missing field 'tick'` |
| 6 | `payload-unknown-field.json` | 거부 | 거부 | `unknown field 'client_sent_at'` (payload `deny_unknown_fields`) |
| 7 | `tick-above-safe-integer.json` | 거부 | 거부 | `Tick 는 0 ..= 9007199254740991 범위여야 한다` |

보조 증거(SC-15)도 만족한다: `serde(flatten)` / `serde(tag =` 는 `server/crates/contracts/src` 전체에서 **0건**이다. peek 구조체(`dispatch.rs`)도 둘 다 쓰지 않는다 — 평범한 구조체에 `deny_unknown_fields` 를 달지 않는 방식이라 grep 자체에 걸리지 않는다.

## 2. QA가 확인 요청한 쟁점 3건

### 쟁점 3 — SC-06/SC-07 의 프로세스 이름: **`starfall-game-server` 가 맞다**

실행으로 확인했다.

```
PS> Get-Process starfall-game-server | Select-Object Id,ProcessName
   Id ProcessName
   -- -----------
38816 starfall-game-server
```

- Cargo 패키지 이름과 별개로 `bins/game-server/Cargo.toml` 에 `[[bin]] name = "starfall-game-server"` 를 **명시적으로 고정**했다. 이름이 우연히 맞는 상태로 두지 않았다.
- 실행 파일은 `server/target/debug/starfall-game-server.exe` 다. `Get-Process` 에는 확장자 없이 넣는 것이 맞고, QA 명령 그대로 동작한다.
- 주의 한 가지: `cargo run -p starfall-game-server` 로 띄우면 `cargo.exe` 가 부모 프로세스로 함께 남는다. `Get-Process starfall-game-server` 는 자식만 잡으므로 **PID 비교에 문제 없다**(위 PID 38816은 stop → start → down → up 전 구간에서 동일했다).

### 쟁점 4 — SC-10 의 psql 자격 증명: **`-U starfall -d starfall` 이 맞다**

`.env.example` 의 `POSTGRES_USER=starfall`, `POSTGRES_DB=starfall` 과 일치한다. 실행으로 확인했다.

```
$ docker compose exec -T postgres psql -U starfall -d starfall -c "create table if not exists _boot_marker(id int);"
CREATE TABLE
```

추가로 **`.env` 가 없어도 같은 값이 나오도록** `docker-compose.yml` 에 기본값을 넣었다(`${POSTGRES_USER:-starfall}` 형태). QA가 `.env` 복사를 건너뛰어도 C-2 명령이 그대로 동작한다.

**한 가지 덧붙임(명령 보강 제안, 이의 아님):** `docker compose up -d` 직후 바로 `psql` 을 부르면 아직 초기화 중이라 실패할 수 있다. 내가 실행할 때 겪었다. C-2 앞에 대기 한 줄을 넣으면 간헐 실패가 사라진다.

```bash
for i in $(seq 1 40); do docker compose exec -T postgres pg_isready -U starfall -d starfall >/dev/null 2>&1 && break; sleep 1; done
```

§0.3이 "간헐적으로 실패하는 항목도 FAIL"이라고 했으므로, 이 대기는 넣어 두는 편이 안전하다.

### 쟁점 5 — D 항목 테스트 이름: **QA 권장 이름을 그대로 쓴다**

권장 이름을 바꾸지 않았다. 대응표는 `03_server_impl.md` 에 표로 남겼다. SC-17 은 권장 이름이 둘(`registry_consistency`, `schema_ids_match_paths`)인데, 레지스트리 파일 자체 검증을 세 번째 테스트로 분리했다(아래).

| SC | 테스트 함수 | 비고 |
|----|-----------|------|
| SC-12 | `schemas_valid_offline` | 권장 이름 그대로 |
| SC-13 | `fixtures_roundtrip` | 권장 이름 그대로 |
| SC-14 | `invalid_rejected_by_schema` | 권장 이름 그대로 |
| SC-15 | `invalid_serde_matrix` | 권장 이름 그대로 |
| SC-16 | `registry_server_types_mapped` | 권장 이름 그대로 |
| SC-17 | `registry_consistency` + `schema_ids_match_paths` + **`registry_file_validates_against_schema`** | 세 번째는 추가 |
| SC-18 | `required_field_mutations` | 권장 이름 그대로 |
| — | `integer_bounds_rejected` | ADR-0002 §3 테스트 9. SC ID 가 따로 없어 SC-15 보강으로 둔다 |

세 번째를 나눈 이유: `registry/types.json` 을 `types.schema.json` 으로 검증하는 것과 `$id`↔경로 정합은 실패했을 때 원인이 전혀 다르다. 한 테스트에 묶으면 어느 쪽이 깨졌는지 이름만 보고 알 수 없다.

## 3. §2 경계면 비교표 — Rust 쪽 구현 값

QA가 SC-32/SC-33 에서 쓸 값을 미리 적는다. 표와 다른 칸은 없다.

**PING_SERVER** (`starfall_contracts::commands::PingServerCommand`)

| 계약 필드 | Rust 타입 | 필수 | 널 가능 |
|----------|----------|------|--------|
| `command_id` | `UuidV7` | 예 | 아니오 |
| `command_type` | `PingServerType` (단일 변형 열거형, `"PING_SERVER"`) | 예 | 아니오 |
| `schema_version` | `ConstSchemaVersion<1>` | 예 | 아니오 |
| `client_sent_at` | `Option<RealTime>` + `deserialize_with = "required_nullable"` | 예(**키**) | 예(값) |
| `payload.probe_seq` | `ProbeSeq(u32)` | 예 | 아니오 |

**PING_REPLY** (`starfall_contracts::messages::PingReplyMessage`)

| 계약 필드 | Rust 타입 | 필수 | 널 가능 |
|----------|----------|------|--------|
| `message_id` | `UuidV7` | 예 | 아니오 |
| `message_type` | `PingReplyType` (`"PING_REPLY"`) | 예 | 아니오 |
| `schema_version` | `ConstSchemaVersion<1>` | 예 | 아니오 |
| `tick` | `Tick` (범위 검증 `u64`) | 예 | 아니오 |
| `correlation_id` | `Option<UuidV7>` + `required_nullable` | 예(**키**) | 예(값) |
| `payload.command_id` | `UuidV7` | 예 | 아니오 |
| `payload.probe_seq` | `ProbeSeq(u32)` | 예 | 아니오 |

**정수 3행(SC-33)** — 전부 스키마 범위를 손실 없이 담고, 범위 밖을 **serde 단계에서** 거부한다.

| 필드 | 스키마 범위 | Rust | 범위 밖 거부 |
|------|------------|------|-------------|
| `probe_seq` | 0 … 4294967295 | `ProbeSeq(u32)` | 예 (`u32` 가 정확히 그 범위) |
| `tick` | 0 … 9007199254740991 | `Tick(u64)` + 상한 검증 | 예 (`u64` 만으로는 부족해 newtype) |
| `schema_version` | 1 … 2147483647 | `ConstSchemaVersion<1>` (`u32` + const 검증) | 예 (1 이외 전부 거부) |

**널 가능 2행** — `skip_serializing_if` 를 **쓰지 않았다.** 두 필드 모두 키가 항상 직렬화되고 값이 `null` 로 나간다. 키를 통째로 빼면 역직렬화가 실패한다(SC-18 의 28건 변이가 이것을 증명한다).

## 4. 이의 제기 2건 (둘 다 증거 수집 방법 — 판정 항목은 그대로)

| # | SC ID | 무엇이 문제인가 | 제안 |
|---|-------|----------------|------|
| 1 | SC-03 | 보조 증거 `git status --porcelain server/Cargo.lock` 이 **빈 출력일 수 없다** | 아래 |
| 2 | SC-11 | `grep -E "^[0-9a-f]{64}$"` 가 **타 프로젝트의 기존 익명 볼륨 43개**를 잡는다 | 아래 |

### 이의 1 — SC-03: `git status --porcelain server/Cargo.lock` 은 항상 비어 있지 않다

**재현 (2026-09-18, 이 PC):**

```
$ git status --porcelain server/Cargo.lock
?? server/Cargo.lock
```

**왜.** 이 레포에는 커밋이 1건(`2d9cf08 Initial commit`)뿐이고 `server/` 는 그 커밋에 없다. 따라서 `server/Cargo.lock` 은 **untracked** 이고 `git status --porcelain` 은 언제나 `??` 를 낸다. 즉 "빈 출력 = lock 이 변하지 않음"이라는 판정식이 성립하지 않는다. 그리고 이 슬라이스는 커밋을 하지 않으므로(I-8) 이 상태는 QA 시점에도 그대로다.

**제안 (둘 중 하나).**

1. **보조 증거를 삭제한다.** `--locked` 는 lock 파일을 갱신해야 하는 상황이면 **빌드 자체를 실패**시킨다. 즉 `cargo test --workspace --locked` 가 종료 코드 0 인 것이 이미 "lock 이 변하지 않았다"의 증거다. 별도 확인이 필요 없다.
2. 굳이 별도 증거를 남기고 싶다면 해시 비교로 바꾼다.
   ```bash
   cd /c/WorkSpace/SpaceHistoric/server
   sha256sum Cargo.lock > /tmp/lock.before
   cargo test --workspace --locked; echo "exit=$?"
   sha256sum -c /tmp/lock.before
   ```

나는 1번을 권한다. 2번은 1번이 이미 보장하는 것을 다시 재는 것이다.

### 이의 2 — SC-11: 익명 볼륨 판정이 이 PC에서는 항상 "발견"으로 나온다

**재현:** 명령 C-3 의 마지막 줄을 그대로 실행하면

```
$ docker volume ls --format "{{.Name}}" | grep -E "^[0-9a-f]{64}$" || echo "익명 볼륨 없음"
0b943a155f1f962206be8af7ddbc3b842a150bd21d63a82aa7dc6b5b8f7bef32
0e998c7204db8b709fbd03ff82668bd8a646bf5c4ae521ed82e1e97d93c8388c
... (총 43건)
```

**왜.** 이 PC에는 다른 프로젝트(`livingfeed-*` 등)가 만든 익명 볼륨이 **이미 43개** 있다. `grep` 은 절대 개수를 보므로 `|| echo "익명 볼륨 없음"` 분기는 **영원히 실행되지 않는다.** starfall 이 익명 볼륨을 하나도 만들지 않아도 이 줄만 보면 실패처럼 읽힌다. (§0.2가 전역 prune 을 금지했으므로 이 43개를 지워서 해결할 수도 없고, 지워서도 안 된다.)

**제안.** 판정을 절대 개수가 아니라 **diff** 로 한다. C-3 에 이미 before/after 스냅샷이 있으므로 도구는 갖춰져 있다.

```bash
# 익명 볼륨이 "새로" 생겼는지만 본다
comm -13 "$SCRATCH/vol-before.txt" "$SCRATCH/vol-after.txt" | grep -E "^[0-9a-f]{64}$" \
  && echo "FAIL: 새 익명 볼륨" || echo "OK: 새 익명 볼륨 없음"
```

**내 실행 결과 (증거):**

```
$ diff vol-before.txt vol-after.txt
94a95
> starfall_postgres-data          ← 추가된 볼륨은 이 명명 볼륨 하나뿐

$ diff ctr-before.txt ctr-after.txt
5a6,7
> starfall-postgres-1
> starfall-redis-1                ← livingfeed-* 5개는 그대로

$ docker inspect starfall-postgres-1 --format '{{range .Mounts}}{{.Type}} {{.Name}} -> {{.Destination}}{{end}}'
volume starfall_postgres-data -> /var/lib/postgresql     ← M-4 실측과 일치
```

## 5. 참고 — 이의는 아니지만 QA가 알면 좋은 것 2가지

1. **SC-07 보다 강한 증거가 이미 나왔다.** SC-07 은 `stop` → `start` 만 본다. 나는 SC-10 절차에서 `down` → `up -d` 로 **컨테이너를 통째로 재생성**한 뒤에도 서버 재시작 없이 `/readyz` 가 2초 만에 200 으로 돌아오는 것을 확인했다(PID 38816 동일). 풀이 죽은 연결을 걸러 내는 경로가 컨테이너 교체까지 견딘다는 뜻이다. SC-07 항목을 바꿀 필요는 없고, 리포트에 덧붙일 수 있는 사실로만 전한다.
2. **`/readyz` 의 최악 응답 시간은 약 2초다.** 두 점검을 `tokio::join!` 로 동시에 돌리고 각각 2초 타임아웃을 건다. 직렬이면 4초가 된다. 아무 것도 듣지 않는 포트를 향한 점검이 5초 안에 `unavailable` 로 끝나는지를 단위 테스트(`readyz_reports_unavailable_when_nothing_is_listening`)가 고정한다. QA가 SC-05/SC-06 에서 타임아웃을 잡는다면 5초 이상으로 잡으면 된다.

## 6. 확인란

| 영역 | 항목 | 담당 | 확인 | 서명/날짜 |
|------|------|------|------|----------|
| A 빌드·품질 | SC-01 ~ SC-03 | server | ☑ (이의 1 반영 요청) | server / 2026-09-18 |
| B 기동·엔드포인트 | SC-04 ~ SC-08 | server | ☑ | server / 2026-09-18 |
| C 로컬 인프라 | SC-09 ~ SC-11 | server | ☑ (이의 2 반영 요청) | server / 2026-09-18 |
| D 계약 ↔ Rust | SC-12 ~ SC-18 | server | ☑ | server / 2026-09-18 |
| 부록 | §0.4 층별 표, §2 비교표 대상, §4 미검증 기준 | server | ☑ | server / 2026-09-18 |

**차단급 이의 없음 → 2단계(구현)를 바로 이어서 수행했다.** 결과는 `03_server_impl.md`.
