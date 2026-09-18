# p0-01-bootstrap 스프린트 계약

- 작성: qa, 2026-09-18 (구현 착수 전 / Phase 3)
- 스펙: `docs/specs/p0-01-bootstrap.md` (상태 agreed, AC 전면 개정판)
- ADR: `docs/adr/0001~0004` (전부 accepted) · 태스크: `_workspace/p0-01-bootstrap/01_architect_tasks.md`
- 계약 데이터: `contracts/**` (유효 fixture 4, 반례 fixture 7, 스키마 7)
- 합의: server ☐ · client ☐ · qa ☑ · architect(참조) ☐ — §7 확인란에 표시한다
- 항목 수: **38** (server 18 / client 12 / qa 8). 이 중 **SC-25는 "기록" 항목**이라 PASS/FAIL 판정에서 제외된다 → 판정 대상 37.

**이 문서의 구속력.** Phase 5 평가(`04_qa_report_r{N}.md`)는 이 표의 항목으로만 한다. 스펙 §7의 수용 기준을 실행 가능한 관찰로 옮긴 것이고, **스펙에 없는 새 요구는 넣지 않았다.** 평가 중 발견한 스펙 밖 문제는 리포트의 "계약 외 발견"에 따로 적고 판정에 쓰지 않는다. 항목을 바꾸려면 §8 변경 이력에 기록하고 server·client의 재확인을 받는다.

---

## 0. 공통 실행 전제

### 0.1 환경 준비

```bash
# Bash에서 새 도구를 쓰기 전에 반드시 한 번
export PATH="$HOME/.cargo/bin:$HOME/.dotnet/tools:/c/Users/CHOISOOYEON/AppData/Local/Unity/bin:$PATH"
```

- **cargo 명령은 `C:\WorkSpace\SpaceHistoric\server`에서** 실행한다(레포 루트에 `Cargo.toml`이 없다 — ADR-0001 §4).
- **그 외(codegen, unity, docker, python, git)는 레포 루트 `C:\WorkSpace\SpaceHistoric`에서** 실행한다.
- **HTTP 점검은 `curl.exe`로 적는다.** PowerShell의 `curl`은 `Invoke-WebRequest` 별칭이라 `-s -o -w`가 통하지 않는다. PowerShell 네이티브(`Invoke-WebRequest`)로 쓸 경우 **503을 기대하는 점검(SC-06)에는 `-SkipHttpErrorCheck`가 필요**하다(없으면 예외로 죽어 증거가 남지 않는다).
- `-w "\n%{http_code}\n"` 형태는 curl이 `\n`을 해석하므로 Bash·PowerShell 양쪽에서 같게 동작한다.
- 이 세션에서 확인한 도구(2026-09-18): Python 3.12.0, cargo 1.98.1, dotnet 10.0.401, unity CLI 1.0.0-beta.8, Docker 27.3.1 / Compose v2.29.7, `curl.exe` 8.x, git 2.47.1.

### 0.2 Docker 금지 사항 (위반 시 항목 전체가 무효)

이 PC에는 **다른 프로젝트 컨테이너 5개(`livingfeed-*`)가 돌고 있다.** 따라서:

- `docker system prune`, `docker volume prune`, `docker builder prune` **금지**
- 프로젝트 밖 `docker compose down -v` **금지**, starfall에 대해서도 `-v`는 쓰지 않는다(SC-10의 증거가 사라진다)
- 정리는 항상 `docker compose`(레포 루트, `name: starfall`) 또는 `docker compose -p starfall …` 범위로만
- **AC-4의 "깨끗한 Docker 상태" = starfall 프로젝트 볼륨이 없는 상태**를 뜻한다. 전역 초기화가 아니다. starfall 볼륨만 지울 때는 이름을 직접 지정한다(`docker volume rm starfall_<name>`).

### 0.3 증거 기준 (PASS의 조건)

| 판정 | 조건 |
|------|------|
| **PASS** | 명령과 출력 요약(종료 코드 포함), 또는 테스트 이름, 또는 파일:라인이 리포트에 있다. 실행하지 않은 정적 읽기만으로는 PASS가 아니다 |
| **FAIL** | 기대 관찰이 나오지 않음. **간헐적으로 실패하는 항목도 FAIL**(비결정성 이슈로 기록, 재시도로 덮지 않는다) |
| **미검증(환경)** | §4의 게이트 조건에 걸려 실행 자체가 불가능했다. PASS로 올리지 않는다 |
| **기록** | 판정하지 않고 사실만 리포트에 남긴다 (SC-25, §5 기록 항목) |

**빈 순회 방지(모든 순회 테스트 공통).** fixture·반례·변이를 순회하는 항목(SC-13, 14, 15, 18, 23, 24)은 **실제로 순회한 개수가 증거에 드러나야 한다**(유효 4 / 반례 7 / 변이 N). 개수가 없으면 "검증기가 꺼진 채 0건 통과"와 구분할 수 없으므로 PASS로 인정하지 않는다. 테스트 이름과 SC ID의 대응은 server는 `03_server_impl.md`, client는 `03_client_impl.md`에 표로 남긴다.

### 0.4 반례 7건의 층별 거부 책임 (SC-14 / SC-15 / SC-24 / SC-25의 판정 기준)

스펙 §5의 표가 **정본**이다. 구현 결과가 이 표와 다르면 표를 고치지 말고 architect에게 알린다.

| # | fixture | ① 스키마 검증 (SC-14) | ② Rust serde = 운영 경로 (SC-15) | ③ C# Strict (SC-24/25) |
|---|---------|---------------------|-------------------------------|----------------------|
| 1 | `PING_SERVER/invalid/actor-field-injected.json` | 거부 | 거부 (`deny_unknown_fields`) | 거부 |
| 2 | `PING_SERVER/invalid/command-id-not-v7.json` | 거부 | 거부 (`UuidV7` newtype) | **감지 불가** (SC-25) |
| 3 | `PING_SERVER/invalid/probe-seq-negative.json` | 거부 | 거부 (`u32`) | 거부 (`uint`) |
| 4 | `PING_SERVER/invalid/probe-seq-above-u32.json` | 거부 | 거부 (`u32`) | 거부 (`uint`) |
| 5 | `PING_REPLY/invalid/missing-tick.json` | 거부 | 거부 (필수 필드) | 거부 (`Required.Always`) |
| 6 | `PING_REPLY/invalid/payload-unknown-field.json` | 거부 | 거부 (`deny_unknown_fields`) | 거부 (알 수 없는 멤버) |
| 7 | `PING_REPLY/invalid/tick-above-safe-integer.json` | 거부 | 거부 (`Tick` 범위) | **감지 불가** (SC-25) |

**왜 층을 나누는가.** 커버리지 스크립트(SC-31)는 `fixtures/{TYPE}/*.json`만 세고 `invalid/`를 보지 않는다 — **의도된 동작**이다. 그래서 반례 거부는 커버리지로 증명되지 않고, 여기 별도 항목으로 있어야 한다. 그리고 **운영 중 명령을 막는 것은 스키마 검증기가 아니라 serde**다. `#[serde(flatten)]`이나 내부 태그 열거형을 쓰면 `deny_unknown_fields`가 무력화되어 **스키마는 거부하는데 서버는 통과시키는** 구멍(#1이 대표)이 생긴다. 그래서 ①과 ②는 반드시 **다른 항목**이다.

---

## 1. 검증 항목

### A. 서버 빌드·품질 게이트 (AC-1)

| ID | 검증 항목 (관찰 가능한 결과) | 검증 방법 (재현 명령·관찰) | 담당 | 근거 AC |
|----|---------------------------|--------------------------|------|--------|
| SC-01 | `cargo fmt --all --check`가 종료 코드 0, 출력 없음 | `cd /c/WorkSpace/SpaceHistoric/server && cargo fmt --all --check; echo "exit=$?"` | server | AC-1 |
| SC-02 | `cargo clippy --workspace --all-targets -- -D warnings`가 종료 코드 0, warning 0건 | `cd /c/WorkSpace/SpaceHistoric/server && cargo clippy --workspace --all-targets -- -D warnings; echo "exit=$?"` | server | AC-1 |
| SC-03 | `cargo test --workspace --locked`가 종료 코드 0, 실패 0건. `--locked`가 통과했다는 것이 곧 `Cargo.lock`이 최신이고 실행 중 변경되지 않았다는 증거다 | `cd /c/WorkSpace/SpaceHistoric/server && cargo test --workspace --locked; echo "exit=$?"` | server | AC-1 |

- **SC-03 보조 증거 폐기(2026-09-18 정정).** 초안은 `git status --porcelain server/Cargo.lock`이 빈 출력일 것을 보조 증거로 요구했으나, 이 레포는 커밋이 1건뿐이라 `server/`가 한 번도 추적된 적이 없고 그 명령은 **항상 `?? server/Cargo.lock`**을 낸다. "빈 출력 = lock 불변"이 성립하지 않으므로 뺀다. `--locked`의 종료 코드 0이 이미 필요한 증거 전부다.
- SC-02 보조 증거: `server/Cargo.toml`의 `[workspace.lints]`에 대해 **각 멤버 크레이트가 `[lints] workspace = true`로 옵트인**했는지(T1 지시). 옵트인이 빠지면 린트가 꺼진 채 clippy가 통과한다. `03_server_impl.md`의 체크 항목으로 남긴다. 부재 시 SC-02는 PASS로 두되 리포트에 "린트 옵트인 미확인"을 함께 적는다.
- SC-02 보조: 테스트에서 `clippy::unwrap_used`를 예외 처리했다면 그 조정 사실이 `03_server_impl.md`에 있어야 한다(T1 지시).

### B. 서버 기동과 운영 엔드포인트 (AC-2, AC-3)

| ID | 검증 항목 | 검증 방법 | 담당 | 근거 AC |
|----|----------|----------|------|--------|
| SC-04 | `/healthz`가 200이고 본문이 `{"status":"ok","version":…}`. 기동 로그에 바인딩 주소(`127.0.0.1:8080`)가 남는다 | 명령 B-1 | server | AC-2 |
| SC-05 | 인프라가 떠 있을 때 `/readyz`가 200이고 `checks.postgres == "ok"`, `checks.redis == "ok"`이며 **두 값이 `{"ok","unavailable"}` 안**에 있다 | 명령 B-2 | server | AC-3 |
| SC-06 | `docker compose stop postgres` 후 `/readyz`가 **503**, `checks.postgres == "unavailable"`, `checks.redis` 값도 닫힌 집합 안. 그 순간 **서버 프로세스가 같은 PID로 살아 있고** `/healthz`는 200 | 명령 B-3 | server | AC-3 |
| SC-07 | `docker compose start postgres` 후 **서버를 재시작하지 않고** `/readyz`가 200으로 복귀(프로세스 PID 동일). 한 번 실패한 풀이 영구 고장으로 남지 않는다 | 명령 B-4 | server | AC-3 |
| SC-08 | `/readyz` 응답 본문(200·503 모두)에 드라이버 에러 문자열·DSN·비밀번호가 없다. `checks.*` 값이 `"ok"`/`"unavailable"` 외의 값을 갖지 않는다 | 명령 B-5 | server | AC-3, 스펙 §5 |

```powershell
# B-1 (서버는 별도 창에서 계속 띄워 둔다)
cd C:\WorkSpace\SpaceHistoric\server; cargo run -p starfall-game-server     # 로그에 바인딩 주소 확인
curl.exe -s -w "\nHTTP %{http_code}\n" http://127.0.0.1:8080/healthz

# B-2
curl.exe -s -w "\nHTTP %{http_code}\n" http://127.0.0.1:8080/readyz

# B-3  (PID는 정지 전/후 같아야 한다)
Get-Process starfall-game-server | Select-Object Id,StartTime
docker compose stop postgres
curl.exe -s -w "\nHTTP %{http_code}\n" http://127.0.0.1:8080/readyz        # 503 + postgres unavailable
curl.exe -s -o NUL -w "healthz=%{http_code}\n" http://127.0.0.1:8080/healthz   # 200 (프로세스 생존)
Get-Process starfall-game-server | Select-Object Id,StartTime

# B-4
docker compose start postgres
curl.exe -s -w "\nHTTP %{http_code}\n" http://127.0.0.1:8080/readyz        # 서버 재시작 없이 200
Get-Process starfall-game-server | Select-Object Id,StartTime              # PID 동일
```

```bash
# B-5  닫힌 집합과 비밀 누출을 한 번에 본다 (200·503 두 상태에서 각각)
curl.exe -s http://127.0.0.1:8080/readyz | tee /dev/stderr | python -c "import json,sys; d=json.load(sys.stdin); v=set(d['checks'].values()); assert v <= {'ok','unavailable'}, v; print('closed-set OK', d['checks'])"
curl.exe -s http://127.0.0.1:8080/readyz | grep -Ei "postgres://|redis://|starfall_dev_only|password|sqlx|io error|connection refused" && echo "LEAK" || echo "no leak"
```

### C. 로컬 인프라 (AC-4)

| ID | 검증 항목 | 검증 방법 | 담당 | 근거 AC |
|----|----------|----------|------|--------|
| SC-09 | `docker compose up -d` 후 `docker compose ps`에서 postgres·redis가 **둘 다 `healthy`** | 명령 C-1 | server | AC-4 |
| SC-10 | 마커 테이블이 `down`(**`-v` 없이**) → `up -d` 후에도 남아 있다 (`to_regclass('public._boot_marker')`가 null이 아님) | 명령 C-2 | server | AC-4 |
| SC-11 | 이 절차 동안 **새로 생긴 볼륨이 starfall 명명 볼륨뿐**이고 **64자 해시 익명 볼륨이 하나도 추가되지 않는다**(before/after diff로 판정). 타 프로젝트 컨테이너 5개와 그 볼륨이 그대로다 | 명령 C-3 | server | AC-4 |

```bash
SCRATCH=/c/Users/CHOISO~1/AppData/Local/Temp/claude/C--WorkSpace-SpaceHistoric/scratch-ac4
mkdir -p "$SCRATCH"; cd /c/WorkSpace/SpaceHistoric

# C-3 (전)
docker volume ls --format "{{.Name}}" | sort > "$SCRATCH/vol-before.txt"
docker ps -a --format "{{.Names}}"   | sort > "$SCRATCH/ctr-before.txt"

# C-1
docker compose up -d
docker compose ps --format "{{.Service}}\t{{.Status}}"          # 둘 다 (healthy)

# C-2  (.env.example 기준 user/db = starfall/starfall)
# up 직후 postgres는 아직 초기화 중이라 psql이 실패한다 → 반드시 준비 대기를 먼저 건다
for i in $(seq 1 60); do docker compose exec -T postgres pg_isready -U starfall -d starfall >/dev/null 2>&1 && break; sleep 2; done
docker compose exec -T postgres pg_isready -U starfall -d starfall
docker compose exec -T postgres psql -U starfall -d starfall -c "create table if not exists _boot_marker(id int);"
docker compose down                                             # -v 절대 금지
docker compose up -d
for i in $(seq 1 60); do docker compose exec -T postgres pg_isready -U starfall -d starfall >/dev/null 2>&1 && break; sleep 2; done
docker compose exec -T postgres psql -U starfall -d starfall -c "select to_regclass('public._boot_marker');"

# C-3 (후) — 판정은 before/after diff로만 한다
docker volume ls --format "{{.Name}}" | sort > "$SCRATCH/vol-after.txt"
docker ps -a --format "{{.Names}}"   | sort > "$SCRATCH/ctr-after.txt"
diff "$SCRATCH/vol-before.txt" "$SCRATCH/vol-after.txt"          # 추가는 starfall_* 명명 볼륨만, 64자 해시 0건
diff "$SCRATCH/vol-before.txt" "$SCRATCH/vol-after.txt" | grep '^>' | sed 's/^> //' | grep -cE "^[0-9a-f]{64}$"   # 0 기대
diff "$SCRATCH/ctr-before.txt" "$SCRATCH/ctr-after.txt"          # livingfeed-* 5개 불변
```

- **로그 문자열 매칭(`initdb` 등)은 증거로 쓰지 않는다** — 이미지 버전에 종속된다(AC-4 명시).
- **SC-11 판정 방식 정정(2026-09-18).** 초안은 `docker volume ls | grep -E "^[0-9a-f]{64}$" || echo "익명 볼륨 없음"`을 증거로 삼았으나, 이 PC에는 **타 프로젝트의 익명 볼륨이 이미 43개** 있어 "없음" 분기가 절대 실행되지 않는다. 판정은 **before/after diff에서 새로 추가된 64자 해시 볼륨이 0건**인지로만 한다.
- **SC-10 대기 루프 추가(2026-09-18).** `up -d` 직후 `psql`을 바로 부르면 초기화 중이라 실패한다(server 실측). `pg_isready` 대기 없이 실행한 실패는 §0.3의 비결정성 FAIL이 아니라 **절차 오류**다.
- PG18 볼륨 마운트 경로(`/var/lib/postgresql`) 실측 결과는 §5 기록 항목이다.

### D. 계약 ↔ Rust (AC-5)

공통 명령: `cd /c/WorkSpace/SpaceHistoric/server && cargo test -p starfall-contracts --locked -- --nocapture`
(테스트 이름은 server가 정한다. **SC ID → 테스트 이름 대응표를 `03_server_impl.md`에 남긴다.** 괄호 안은 권장 이름.)

| ID | 검증 항목 | 검증 방법 | 담당 | 근거 AC |
|----|----------|----------|------|--------|
| SC-12 | 스키마 7개가 2020-12 메타스키마로 유효하고 `$ref`가 전부 해석된다. **네트워크 접근 없이**(`Registry` 사전 등록 + `offline()`) | 위 명령 (`schemas_valid_offline`). 증거에 검증한 스키마 개수(7)가 드러날 것 | server | AC-5(a) |
| SC-13 | 유효 fixture **4건**이 역직렬화 → 재직렬화 후 원본과 `serde_json::Value` 비교로 동일하고, 재직렬화 결과가 스키마 검증을 통과한다 | 위 명령 (`fixtures_roundtrip`). 증거에 4건 파일명 | server | AC-5(b) |
| SC-14 | **[층①]** `invalid/` **7건 전부가 스키마 검증에서 거부**된다 | 위 명령 (`invalid_rejected_by_schema`). 증거에 7건 파일명과 개수 | server | AC-5(c), I-4 |
| SC-15 | **[층②]** `invalid/` 7건을 **Rust 타입으로 역직렬화**한 결과가 §0.4 표의 "Rust serde" 열과 **일치**(현재 7건 전부 거부) | 위 명령 (`invalid_serde_matrix`). 증거는 7행 결과표. 보조: `grep -rn "serde(flatten)\|serde(tag *=" server/crates/contracts/src`가 peek 구조체 외 0건 | server | AC-5(d), I-6 |
| SC-16 | `producers` **또는** `consumers`에 `server`가 있는 타입이 전부 이름→Rust 타입 대응표에 있고, 레지스트리에 없는 fixture 디렉토리가 없다 | 위 명령 (`registry_server_types_mapped`) | server | AC-5(e) |
| SC-17 | 레지스트리·스키마 상수·fixture의 타입 이름과 `schema_version`이 일치하고, `types.json`이 `types.schema.json`으로 검증되며, 모든 `$id`가 경로 규칙과 일치한다 | 위 명령 (`registry_consistency`, `schema_ids_match_paths`) | server | AC-5(f) |
| SC-18 | 유효 fixture에서 `required` 필드를 하나씩 제거한 변이가 **전부** 역직렬화에 실패한다 | 위 명령 (`required_field_mutations`). 증거에 변이 개수 | server | AC-5(g) |

- `contracts/` 경로 해석 실패나 fixture 0건 순회는 **테스트 실패**여야 한다(T2 지시). 개수가 증거에 없으면 §0.3에 따라 PASS로 인정하지 않는다.
- `uuid` 재직렬화가 소문자 하이픈 표기인지는 SC-13의 왕복 비교가 자동으로 잡는다(별도 항목 아님).

### E. 계약 → C# 생성기 (AC-6)

| ID | 검증 항목 | 검증 방법 | 담당 | 근거 AC |
|----|----------|----------|------|--------|
| SC-19 | 생성기를 **두 번** 실행해도 생성 파일의 해시가 변하지 않는다(결정적 출력) | 명령 E-1 | client | AC-6 |
| SC-20 | 생성 파일 한 줄을 고치고 `--check` → **0이 아닌 종료 코드**와 **다른 파일의 경로**가 출력된다 | 명령 E-2 | client | AC-6, I-2 |
| SC-21 | `Generated/`에 생성기가 만들지 않는 `.cs`를 추가하고 `--check` → **0이 아닌 종료 코드**와 **그 파일 경로**가 출력된다(내용뿐 아니라 **파일 집합** 비교) | 명령 E-3 | client | AC-6, I-2 |

```bash
cd /c/WorkSpace/SpaceHistoric
GEN=client/Assets/_Project/Scripts/Contracts/Generated
SCRATCH=/c/Users/CHOISO~1/AppData/Local/Temp/claude/C--WorkSpace-SpaceHistoric/scratch-ac6; mkdir -p "$SCRATCH"

# E-1
dotnet run tools/codegen/ContractsCodegen.cs -- --contracts contracts --out "$GEN"
find "$GEN" -name '*.cs' | sort | xargs sha256sum > "$SCRATCH/h1.txt"
dotnet run tools/codegen/ContractsCodegen.cs -- --contracts contracts --out "$GEN"
find "$GEN" -name '*.cs' | sort | xargs sha256sum > "$SCRATCH/h2.txt"
diff "$SCRATCH/h1.txt" "$SCRATCH/h2.txt" && echo "deterministic OK"

# E-2 / E-3 — QA 재현은 client 파일을 건드리지 않고 스크래치 사본에서 한다
cp -r "$GEN" "$SCRATCH/gen"
dotnet run tools/codegen/ContractsCodegen.cs -- --contracts contracts --out "$SCRATCH/gen" --check; echo "clean exit=$?"     # 0 기대
echo "// qa mutation" >> "$(find "$SCRATCH/gen" -name '*.cs' | head -1)"
dotnet run tools/codegen/ContractsCodegen.cs -- --contracts contracts --out "$SCRATCH/gen" --check; echo "mutated exit=$?"   # E-2: 비0 + 파일 경로
cp -r "$GEN"/. "$SCRATCH/gen"/ && printf 'class QaOrphan {}\n' > "$SCRATCH/gen/QaOrphan.cs"
dotnet run tools/codegen/ContractsCodegen.cs -- --contracts contracts --out "$SCRATCH/gen" --check; echo "orphan exit=$?"    # E-3: 비0 + QaOrphan.cs 경로
rm -rf "$SCRATCH/gen"
```

- 생성기는 `.meta`를 만들거나 지우지 않는다(Unity 관리). 실물 `Generated/`에서 시연한 경우 client가 재생성으로 원상 복구하고, QA가 만든 임시 파일은 QA가 지운다.
- `--out`에 임의 경로를 줄 수 없는 구현이면 client가 실물에서 시연하고 증거를 `03_client_impl.md`에 남긴다(QA는 그 증거를 인용하고 재현 불가 사유를 적는다).

### F. Unity 클라이언트 (AC-7, AC-8)

공통 명령 F-1 (레포 루트):

```bash
unity test client --mode EditMode --report-format nunit,junit \
  --output _workspace/p0-01-bootstrap/unity-tests/EditMode.nunit.xml \
  --junit-output _workspace/p0-01-bootstrap/unity-tests/EditMode.xml
echo "exit=$?"
ls -l _workspace/p0-01-bootstrap/unity-tests/EditMode.nunit.xml _workspace/p0-01-bootstrap/unity-tests/EditMode.xml
grep -o 'failures="[0-9]*"' _workspace/p0-01-bootstrap/unity-tests/EditMode.xml | head -1
grep -c "<test-case" _workspace/p0-01-bootstrap/unity-tests/EditMode.nunit.xml
```

| ID | 검증 항목 | 검증 방법 | 담당 | 근거 AC |
|----|----------|----------|------|--------|
| SC-22 | F-1이 **종료 코드 0**, 실패 **0건**, 그리고 **두 리포트 파일이 실제로 존재**한다 | 명령 F-1. **`--report-format nunit,junit`**(목록). 형식을 하나만 주면 `--junit-output`이 무시되고, **`both`는 이 CLI에서 유효하지 않아 종료 코드 2로 실패한다**(client 실측, 2026-09-18 정정 — 스펙 AC-7도 같이 고쳐졌다) | client | AC-7 |
| SC-23 | 유효 fixture **4건**이 DTO 역직렬화 → 재직렬화 → 원본과 동일(`Strict`: `DateParseHandling.None` + `MissingMemberHandling.Error`) | F-1 결과의 해당 테스트 (`Fixtures_RoundTrip_*`). 증거에 4건 파일명 | client | AC-7(a) |
| SC-24 | §0.4 표에서 **C# 책임인 반례 5건**(#1,3,4,5,6)이 예외로 거부된다 | F-1 결과의 해당 테스트 (`Invalid_Rejected_*`). 증거에 5건 파일명 + 예외 메시지 | client | AC-7(b) |
| SC-25 | **[기록·판정 제외]** `command-id-not-v7`, `tick-above-safe-integer` 2건은 C# Strict에서 **통과한다**(`Guid`·`long`이 담는다). 이것이 **기록된 설계 비대칭**이며 스키마 검증과 Rust serde가 서버 경계에서 막는다 | F-1의 테스트가 두 건의 "감지 불가"를 명시적으로 드러낸다(테스트 이름·주석·Assert). QA는 결과를 리포트에 그대로 옮긴다. **결과가 표와 달라지면 FAIL이 아니라 architect 통지 사유**(계약의 설계가 바뀐 것이다) | client | 스펙 §5, AC-7(b) |
| SC-26 | `PING_SERVER/basic.json`의 `client_sent_at`을 **디스패치 헬퍼**로 열면 `2026-09-17T14:05:09.123Z`가 그대로 유지되고, 기본 설정 `JObject.Parse`가 같은 값을 `09/17/2026 14:05:09`로 바꾸는 것도 **같은 테스트가 함께 보여 준다**(회귀 가드) | F-1 결과의 해당 테스트 (`Dispatch_PreservesRealTime`). 두 값이 모두 출력/Assert에 있어야 함 | client | AC-7(c) |
| SC-27 | UUIDv7 헬퍼가 만든 ID가 `UuidV7` 패턴(`…-7xxx-[89ab]xxx-…`)을 만족하고, **같은 밀리초 안에서 N개를 연속 생성해도 중복이 없다** | F-1 결과의 해당 테스트 (`UuidV7_*`). 증거에 생성 개수 N | client | AC-7(d) |
| SC-28 | fixture 로더가 레포 루트를 `contracts/registry/types.json` 마커로 찾고, **유효 fixture를 4건 미만 발견하면 테스트가 실패**한다(Skip 아님) | F-1 결과의 해당 테스트 (`FixtureLoader_*`). 가드 자체를 증명하려면 로더가 세는 개수(4)가 증거에 있어야 한다 | client | AC-7(e), I-4 |
| SC-29 | `client/Library/`를 삭제한 **콜드 임포트** 상태에서 F-1이 종료 코드 0이고, **`client/Logs/Editor.log`**에 **컴파일 에러 0건**, `Assets/_Project/**`에서 발생한 **컴파일 경고 0건** | 명령 F-2 (로그 경로 2026-09-18 정정: `%LOCALAPPDATA%\Unity\Editor\Editor.log`가 아니라 **프로젝트 안 `client/Logs/Editor.log`**이며 실행마다 갱신된다 — 스펙 AC-8도 같이 고쳐졌다) | client | AC-8 |
| SC-30 | `client/ProjectSettings/ProjectVersion.txt`의 `m_EditorVersion`이 **`6000.6.1f1`과 문자열로 같다** | 명령 F-3 | client | AC-8 |

```bash
# F-2 (콜드 임포트) — 시간이 오래 걸린다. 타임아웃으로 오판하지 않는다.
rm -rf /c/WorkSpace/SpaceHistoric/client/Library
# ... F-1 실행 ...
LOG="/c/WorkSpace/SpaceHistoric/client/Logs/Editor.log"              # 프로젝트 안. 실행마다 갱신된다
grep -c "error CS" "$LOG"                                            # 0 기대
grep "warning CS" "$LOG" | grep -c "Assets/_Project"                 # 0 기대

# F-3
sed -n 's/^m_EditorVersion: //p' /c/WorkSpace/SpaceHistoric/client/ProjectSettings/ProjectVersion.txt | tr -d '\r'
# 출력이 정확히 6000.6.1f1 이어야 한다
```

### G. QA 통합·경계면·저장소 상태 (AC-9, AC-10, AC-11)

| ID | 검증 항목 | 검증 방법 | 담당 | 근거 AC |
|----|----------|----------|------|--------|
| SC-31 | 계약 커버리지 스크립트가 `--strict`에서 **종료 코드 0**(errors 0, **warnings 0**) | `cd /c/WorkSpace/SpaceHistoric && python .claude/skills/integration-qa/scripts/check_contract_coverage.py --strict; echo "exit=$?"` — **T2·T7 완료 후에만 실행**(§3 게이트). 그 전 결과는 판정에 쓰지 않는다 | qa | AC-9 |
| SC-32 | 스키마 · Rust 타입 · C# DTO를 나란히 읽어 만든 비교표에서 **필드 이름·필수 여부·널 가능 여부가 3자 간 동일**하고 불일치 **0건** | QA가 `contracts/**` ↔ `server/crates/contracts/src/**` ↔ `client/.../Generated/**`를 함께 읽고 §2의 필드 목록으로 표를 만들어 `04_qa_report_r{N}.md`에 싣는다 | qa | AC-10 |
| SC-33 | 각 정수 필드의 언어 타입이 스키마의 `minimum`~`maximum`을 **손실 없이 표현**한다(타입 이름이 같을 필요는 없다). 범위 밖 값은 최소한 스키마 검증이 거부한다 | 같은 표의 정수 행 3개(§2). 범위 밖 거부는 SC-14 결과를 인용 | qa | AC-10 |
| SC-34 | 층별 거부 범위가 **§0.4 표와 일치**하고, 3층 매트릭스가 리포트에 기록된다 | SC-14 / SC-15 / SC-24 / SC-25 결과를 한 표로 합쳐 리포트에 싣는다 | qa | AC-10, 스펙 §5 |
| SC-35 | 이 슬라이스가 만든 **새 커밋이 0건**이고 HEAD가 원격 초기 커밋과 같다 | 명령 G-1 | qa | AC-11(a), I-8 |
| SC-36 | `contracts/`·`docs/`·`_workspace/`·`server/`·`client/…/Generated/**`의 `.cs`와 `.meta`·`tools/`가 **추적 후보(untracked)로 보인다** | 명령 G-2 | qa | AC-11(b) |
| SC-37 | `client/Library/`·`client/Temp/`·`client/Logs/`·`client/obj/`·`client/UserSettings/`·`server/target/`·`tools/**/obj/`·`.env`가 **무시된다** | 명령 G-3 (각 경로에서 종료 코드 0 + 매칭 규칙 출력) | qa | AC-11(c) |
| SC-38 | **`server/.sqlx/`는 무시되지 않는다**(다음 슬라이스의 DB 없는 오프라인 빌드 근거) | 명령 G-4 (종료 코드 **1**) | qa | AC-11(d) |

```bash
cd /c/WorkSpace/SpaceHistoric

# G-1
git log --oneline
git rev-list --count HEAD                       # 1 기대
git rev-parse HEAD; git rev-parse origin/main   # 두 값이 같아야 한다 (계약 작성 시점: 2d9cf08…)

# G-2
git status --porcelain | sort

# G-3  (전부 종료 코드 0 + 매칭한 .gitignore 규칙이 출력되어야 한다)
for p in client/Library/x client/Temp/x client/Logs/x client/obj/x client/UserSettings/x \
         server/target/x tools/codegen/obj/x .env; do
  out=$(git check-ignore -v "$p"); echo "rc=$? $p $out"
done

# G-4  (무시되면 안 되므로 rc=1 이어야 한다)
git check-ignore -v server/.sqlx/query-example.json; echo "rc=$?"
git check-ignore -v client/Assets/_Project/Scripts/Contracts/Generated/PingServerCommand.cs; echo "rc=$?"
git check-ignore -v client/Assets/_Project/Scripts/Contracts/Generated/PingServerCommand.cs.meta; echo "rc=$?"
```

**SC-35 해석 노트(미리 합의).** 현재 워킹트리에는 `M LICENSE`(architect의 독점 라이선스 교체)가 있다. 이것은 **커밋되지 않은 변경**이므로 AC-11(a)("새 커밋 0건")에 위배되지 않는다. 마찬가지로 `.claude/`·`.gitattributes`·`.gitignore`·`CLAUDE.md`·`기획안/`이 untracked로 보이는 것도 위반이 아니다. AC-11(b)는 "열거한 경로가 보일 것"을 요구할 뿐 "그 외가 없을 것"을 요구하지 않는다.

---

## 2. 경계면 비교표의 고정 대상 (SC-32 / SC-33의 채점 기준)

QA가 리포트에 실을 표의 **행 목록**을 지금 고정한다(구현 후 임의 확대 금지).

**PING_SERVER** — `command_id`(UuidV7, required, non-null) · `command_type`(TypeName const, required) · `schema_version`(const 1, required) · `client_sent_at`(RealTime **또는 null**, required 키) · `payload.probe_seq`(integer 0..4294967295, required)

**PING_REPLY** — `message_id`(UuidV7, required) · `message_type`(const, required) · `schema_version`(const 1, required) · `tick`(integer 0..9007199254740991, required) · `correlation_id`(UuidV7 **또는 null**, required 키) · `payload.command_id`(UuidV7, required) · `payload.probe_seq`(integer 0..4294967295, required)

**정수 3행(SC-33)**

| 필드 | 스키마 범위 | Rust(기대) | C#(기대) | 판정 기준 |
|------|------------|-----------|---------|----------|
| `probe_seq` | 0 … 4294967295 | `u32`(+범위 newtype) | `uint` | 범위를 손실 없이 담으면 PASS. 이름 동일 요구 없음 |
| `tick` | 0 … 9007199254740991 | 범위 검증 newtype(`u64` 기반) | `long` | 위와 같음. C#은 상한 위반을 못 잡는다(§0.4 #7, SC-25) |
| `schema_version` | 1 … 2147483647 | `u32`/`i32` + const 검증 | `int` | 위와 같음 |

**널 가능 2행** — `client_sent_at`, `correlation_id`는 **키가 항상 존재하고 값이 null일 수 있다**(I-5). Rust는 `Option<T>` + `skip_serializing_if` **금지**, C#은 `Required.AllowNull`. 한쪽이라도 "키 없음"으로 처리하면 SC-32 불일치다.

---

## 3. 실행 순서와 게이트

| 게이트 | 조건 | 해제 전 실행한 결과의 취급 |
|-------|------|--------------------------|
| **G-a** | SC-31(커버리지 `--strict`)은 **T2·T7 완료 후**에만 실행한다 | 그 전 결과는 판정에 쓰지 않는다. (현재 상태에서 `--strict`는 "source root missing" 경고 4건으로 반드시 실패한다 — 2026-09-18 실측: 비-strict 실행 시 errors 0 / warnings 4) |
| **G-b** | SC-32·SC-33·SC-34는 server T2와 client T7이 모두 끝난 뒤 | 한쪽만 있으면 "미검증(환경)"이 아니라 **대기**로 두고 라운드 판정에서 제외 후 다음 라운드에 판정 |
| **G-c** | SC-05~SC-11은 `docker compose up -d`가 healthy에 도달한 뒤 | 첫 실행은 이미지 pull로 수 분 걸린다 — **타임아웃이 아니다** |
| **G-d** | SC-01~SC-18은 레포 안 첫 `cargo` 실행(툴체인 1.98.1 다운로드, 수 분) 이후 | 다운로드 실패는 FAIL이 아니라 미검증(환경) |
| **G-e** | SC-29(콜드 임포트)는 다른 Unity 항목을 웜 상태에서 통과시킨 **뒤** 마지막에 | 콜드 임포트로 Library를 날리면 재임포트 비용이 크다 |

모듈 완료 알림 시 QA가 즉시 보는 경계면(Phase 4, 라운드 판정과 별개):
T2 완료 → SC-12~SC-18 + `contracts/` ↔ Rust 필드 비교 / T7 완료 → SC-22~SC-28 + `contracts/` ↔ C# 필드 비교 / T3 완료 → SC-04~SC-11.

---

## 4. "미검증(환경)" 처리 기준 (평가 전에 합의)

아래 조건이면 해당 항목은 **FAIL이 아니라 미검증(환경)**으로 기록하고, 필요한 조치를 리더에게 보고한다.

| 조건 | 영향 항목 | 리포트 표기 |
|------|----------|------------|
| 레포 안 첫 `cargo`의 툴체인 1.98.1 다운로드 실패(오프라인·네트워크) | SC-01 ~ SC-18 | 미검증(환경) — 툴체인 다운로드 필요 |
| Docker Desktop 미가동 / 이미지 pull 실패 | SC-05 ~ SC-11 | 미검증(환경) — `postgres:18.6-trixie`, `redis:8.10.1-trixie` pull 필요 |
| 포트 8080 / 15432 / 16379 점유 | SC-04 ~ SC-11 | 미검증(환경) — 점유 프로세스 기록(`netstat -ano`) |
| Unity Editor 라이선스 실패 / `unity test`가 에디터를 띄우지 못함 | SC-22 ~ SC-30 | 미검증(환경) — CLI 로그 첨부 |
| .NET 10 SDK 미가용 | SC-19 ~ SC-21 | 미검증(환경) |
| T2 또는 T7 미완 | SC-31 ~ SC-34 | **대기**(판정 제외, 다음 라운드) |
| 구현물 자체가 없음(태스크 미착수) | 해당 항목 | **FAIL**(미검증 아님) — 환경 문제가 아니다 |

**주의:** "환경이 없어서"와 "구현이 없어서"를 섞지 않는다. 구현이 없으면 FAIL이다.

---

## 5. 기록 항목 (판정하지 않음)

스펙 §8과 architect가 "미확인"으로 남긴 사실. 값이 나오지 않아도 FAIL이 아니지만, **리포트와 `03_*_impl.md`에 숫자·결과가 있어야** 다음 슬라이스가 비교 기준을 갖는다.

| # | 기록할 것 | 담당 | 근거 |
|---|----------|------|------|
| M-1 | 서버 클린 빌드 시간 **2회**: T1 시점(axum만), T3 시점(sqlx·redis 추가) | server | 스펙 §8 |
| M-2 | 레포 안 첫 `cargo` 실행의 툴체인 다운로드 시간 | server | 스펙 §8 |
| M-3 | 첫 `docker compose up -d`의 이미지 pull 시간 | server | 스펙 §8 |
| M-4 | `docker image inspect postgres:18.6-trixie`의 `PGDATA`·`Config.Volumes` 실측값 | server | T3 지시(미확인 사실) |
| M-5 | `unevaluatedProperties`가 `allOf` 애너테이션을 수집해 `actor-field-injected`를 거부하는지 **T2 첫 작업에서 재현** | server | T2 지시 |
| M-6 | `sqlx` 0.9.0 vs 0.8.x 핀 결정과 이유 / `redis` 1.7.0 | server | ADR-0003 §3.1 |
| M-7 | Unity EditMode 테스트 시간: **콜드(Library 삭제 포함) 1회 + 웜 1회** | client | 스펙 §8 |
| M-8 | `unity projects new` 직후의 `companyName`/`productName` 원래 값과 설정 후 값, Hub 등록 여부 | client | T5 지시(미확인 사실) |
| M-9 | `collab-proxy`·`visualscripting` 제거의 실제 영향(해석 오류 여부) | client | T5 지시 |
| M-10 | C#이 잡지 못하는 반례 2건(SC-25)의 관찰 결과 — 설계 비대칭으로 그대로 기록 | qa | 스펙 §5 |

---

## 6. 판정·라운드 규칙

- 라운드는 **최대 3회**. 각 라운드 결과는 `_workspace/p0-01-bootstrap/04_qa_report_r{N}.md`.
- FAIL 항목은 **파일:라인 + 재현 명령 + 기대/실제**를 담아 담당자에게 수정 요청한다. QA는 구현 코드를 직접 고치지 않는다(`tests/e2e/`, `tools/bots/`만 QA 소유).
- 경계면 불일치는 **생산자와 소비자 양쪽**에 알리고, 계약 자체가 모호하면 architect에게도 알린다.
- 3라운드 후에도 FAIL이 남으면 남은 항목·원인 추정·선택지(범위 축소 / 스펙 수정 / 추가 라운드)를 리더에게 보고한다.
- 봇 시나리오 테스트는 **이번 슬라이스 범위가 아니다**(게임플레이 없음). `tools/bots/`는 만들지 않는다.

---

## 7. 구현자 확인란

각자 "이 방법으로 완료를 증명할 수 있다"에 표시한다. 이의가 있으면 아래 표에 적고, QA가 §8에 반영한 뒤 다시 확인을 받는다.

| 영역 | 항목 | 담당 | 확인 | 서명/날짜 |
|------|------|------|------|----------|
| A 빌드·품질 | SC-01 ~ SC-03 | server | ☐ | |
| B 기동·엔드포인트 | SC-04 ~ SC-08 | server | ☐ | |
| C 로컬 인프라 | SC-09 ~ SC-11 | server | ☐ | |
| D 계약 ↔ Rust | SC-12 ~ SC-18 | server | ☐ | |
| E 코드 생성기 | SC-19 ~ SC-21 | client | ☐ | |
| F Unity 클라이언트 | SC-22 ~ SC-30 | client | ☐ | |
| G QA 통합·저장소 | SC-31 ~ SC-38 | qa | ☑ | qa / 2026-09-18 |
| 부록 | §0.4 층별 표, §2 비교표 대상, §4 미검증 기준 | server·client 공통 | ☐ ☐ | |

**이의 제기**

| # | SC ID | 제기자 | 이의 내용 (무엇이 증명 불가능한가 / 어떤 문구로 바꾸면 되는가) | 처리 |
|---|-------|-------|--------------------------------------------------|------|
| 1 | | | | |
| 2 | | | | |

QA가 미리 예상하는 쟁점(합의 시 함께 답해 주면 좋다):

1. **SC-19~SC-21의 `--out` 임의 경로.** 생성기가 `--out`에 스크래치 경로를 받아 `--check`할 수 있는가? 불가능하면 client가 실물에서 시연하고 QA는 증거를 인용한다(§E 각주).
2. **SC-29의 Editor 로그 경로.** `unity test`가 쓰는 로그 파일 경로가 `%LOCALAPPDATA%\Unity\Editor\Editor.log`가 맞는지, 아니면 CLI가 별도 경로를 출력하는지. 경로가 다르면 F-2의 `$LOG`를 그 값으로 바꾼다.
3. **SC-06/SC-07의 PID 확인 방법.** 프로세스 이름이 `starfall-game-server.exe`가 맞는지(바이너리 이름이 다르면 `Get-Process` 인자를 바꾼다).
4. **SC-10의 psql 자격 증명.** `.env.example` 기준 `starfall/starfall`이 맞는지(다르면 명령 C-2의 `-U`/`-d`를 바꾼다).
5. **D 항목의 테스트 이름.** 권장 이름을 그대로 쓸지, 다른 이름을 쓰고 대응표만 남길지.

---

## 8. 계약 변경 이력

| 날짜 | 변경 | 사유 |
|------|------|------|
| 2026-09-18 (2차) | **실행으로 드러난 오류 4건 + 보강 1건 정정**: SC-22 `--report-format both` → **`nunit,junit`**(`both`는 exit 2), SC-29 로그 경로 → **`client/Logs/Editor.log`**, SC-03 `git status` 보조 증거 폐기(커밋 1건뿐이라 항상 `??`), SC-11 판정을 **before/after diff**로(타 프로젝트 익명 볼륨 43개가 이미 존재해 "없음" 분기가 도달 불가), SC-10에 `pg_isready` 대기 루프 추가(up 직후 psql 실패는 절차 오류이지 비결정성이 아니다) | server·client 구현 중 실측. 스펙 AC-7·AC-8도 architect가 같은 내용으로 갱신 |
| 2026-09-18 | 최초 작성 (SC-01 ~ SC-38) | 스펙 §7 AC 전면 개정판을 실행 항목으로 변환. 반례 거부를 스키마층(SC-14)과 serde층(SC-15)으로 분리, C# 미감지 2건을 기록 항목(SC-25)으로 고정, AC-9 실행 순서를 게이트 G-a로, AC-4를 마커 잔존 + 익명 볼륨 미생성으로, `server/.sqlx/` 미무시를 SC-38로 독립 |
