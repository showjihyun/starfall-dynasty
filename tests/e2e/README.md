# tests/e2e — p0-02 검증 절차 (QA 소유)

스프린트 계약 `_workspace/p0-02-networking-spike/02_sprint_contract.md` 의 §3.2·§4 를 실행 가능한
형태로 옮긴 것이다. **판정은 이 스크립트가 하지 않는다** — 증거를 재현 가능한 형태로 만들고,
게이트 순서를 지키고, 실행 중에만 볼 수 있는 것을 놓치지 않는 것까지가 여기 역할이다.

## 종료 코드 규약

| 코드 | 뜻 | 리포트 표기 |
|-----:|----|------------|
| 0 | 통과 | PASS |
| 1 | 기대와 다름 | FAIL |
| 2 | docker/psql/서버에 닿지 못함 | **미검증(환경)** |
| 3 | 테이블·로그·엔드포인트가 아직 없음 | **FAIL(구현 없음)** — 환경 문제가 아니다 |

2와 3을 섞지 않는 것이 이 규약의 전부다(계약 §5).

## 사전 준비

```bash
export PATH="$HOME/.cargo/bin:$HOME/.dotnet/tools:/c/Users/CHOISOOYEON/AppData/Local/Unity/bin:$PATH"
export STARFALL_DEV_AUTH_SECRET=dev_only_not_a_secret     # .env.example 의 공개 개발값
cd /c/WorkSpace/SpaceHistoric/tools/bots && cargo build   # bots.exe
```

## 블록 러너

```bash
cd /c/WorkSpace/SpaceHistoric

# 환경·도구 점검. 어떤 E-코드가 걸리는지 먼저 본다
python tests/e2e/run_block.py preflight

# 블록 5: 부하 A → B → C  (Unity PlayMode 접속을 **먼저** 붙여 둔 상태에서)
python tests/e2e/unity_corr.py --out _workspace/p0-02-networking-spike/evidence/unity-corr.txt
python tests/e2e/run_block.py load --unity-corr _workspace/p0-02-networking-spike/evidence/unity-corr.txt

# 블록 6: D 기록 내구성 (AC-19)
python tests/e2e/run_block.py durability --token "$(tools/bots/target/debug/bots token --label bot-000 | sed -n 's/^token   : //p')"

# 서버 단독 항목 probe (SC-14·19·20·21·22·24·25·26)
python tests/e2e/run_block.py probes
```

증거는 `_workspace/p0-02-networking-spike/evidence/<block>/` 에 쌓인다.

## 개별 스크립트

| 스크립트 | 항목 | 비고 |
|---------|------|------|
| `run_block.py` | 블록 순서 | preflight / load / durability / probes |
| `check_in_flight.py` | SC-61 (AC-17a) | **A 가 도는 동안에만** 측정 가능. `correlations.live.txt` 를 쓴다 |
| `poll_until.py rows` | SC-62 (AC-17b) | 호스트 `count(*)` 폴링. `recorded_at` 과 호스트 시계를 비교하지 않는다 |
| `poll_until.py backlog` | SC-66/68 (AC-19a·c) | `--above 600` 으로 임계 도달을 기다린다(시간이 아니라 상태) |
| `check_sessions.py` | SC-57/58 (AC-16a·b) | correlation 집합 기반. 짝 없음·중복·누락·null actor 를 함께 본다 |
| `check_sequence_gaps.py` | SC-59 (AC-16c) | 스펙 SQL 원문. **전 테이블** 대상 |
| `check_occurred_at.py` | SC-60 (AC-16d) | `--selftest` 로 DB 없이 공식만 검증할 수 있다 |
| `append_only_probe.py` | SC-11/12/13 (AC-4) | **서버 정지 상태**에서. 탐침 규칙 내장 |
| `three_way.py` | SC-56 (AC-15e/AC-9c) | 봇 / `/debug/stats` / DB 3자 대조 |
| `unity_corr.py` | SC-49/50/51 · §0.6 | client 가 고정한 로그 문구를 읽는다 |
| `db.py` | 공통 | psql 호출·집합 읽기·종료 코드 매핑 |

## 실행 순서에서 틀리기 쉬운 것

1. **`docker compose down -v` 는 측정 세션의 첫 DB 단계여야 한다**(게이트 G-a). 그 전에 모은
   DB 증거는 전부 무효다. 마이그레이션 `.sql` 을 고친 뒤에도 필요하다(체크섬 불일치).
2. **탐침(`append_only_probe.py`)은 마지막에, 서버가 꺼진 상태에서.** 탐침 행이 다음 기동의
   `start_tick` 을 밀어 올리는 것은 정상이며(ADR-0006 §2.3) 리포트에 적는다.
3. **SC-61 은 A 가 끝나면 측정할 수 없다.** `run_block.py load` 가 A 중간(기본 T+25s)에 부른다.
4. **SC-62 는 마지막 봇이 끊긴 직후에 재야 한다.** 다른 조회를 먼저 하면 5초 판정이 망가진다.
5. **A 단계가 도는 동안 `client/` 아래 파일을 저장하지 않는다.** 도메인 리로드가 31번째
   연결을 끊는다(client 요청, 계약 §4 G-h).
6. **31번째 연결은 사람이 띄운 Editor 의 PlayMode** 여야 한다. `unity test --mode PlayMode` 는
   테스트가 끝나면 Editor 가 내려가 연결이 같이 죽는다(`03_client_impl.md` §1.4).

## 대기와 셸

- Bash 전경 `sleep` 은 이 세션 도구가 막는다. 대기는 `docker compose exec -T postgres pg_isready -t N`
  (컨테이너 안), PowerShell `Start-Sleep`, 또는 이 스크립트들의 폴링을 쓴다.
- SQL·HTTP 호출은 전부 파이썬 안에 있다. 셸 인용 규칙 차이로 같은 명령이 다르게 깨지지 않게 하려는 것이다.

## p1-01 라운드 3 추가 (qa)

| 스크립트 | 항목 | 비고 |
|---------|------|------|
| `log_pipe_backpressure.py run/selftest` | AC-2(i) | **드레인하지 않는** stdout 파이프로 서버를 띄운다(`server_boot.py` 는 드레인한다 — 경로 분리). ① `PeekNamedPipe` 로 파이프가 찼음을 단언, 안 찼으면 종료 코드 **4(관측 조건 미발생)**. ③ stderr 는 파일 |
| `make_red_logsink_binary.py` | AC-2(i) ② RED | 현재 소스를 레포 밖으로 복사해 `init_tracing` 한 줄만 바꿔(싱크 끔) 빌드. `server/` 는 건드리지 않는다 |
| `stats_delta.py snap/diff` | SC-33 | `/debug/stats` 전후 델타를 **라벨 전부** 펼치고 `--expect LABEL=N`(봇이 받은 거부 수)과 라벨별 일치 단언 |
| `ship_events.py shutdown` | SC-13 | 종료 디스폰을 (a) 잔류 / (b) 활성(같은 tick 원인)으로 분리. 둘 다 1건 이상이어야 PASS |
| `resume_check.py` + `resume_predict/` | SC-11 (3) | `bots resume` 결과를 **client C# 적분기**(서버 아님, I-25)로 독립 계산과 대조. 양자화 봉투 + 음성 대조(틀린 휴면 모델) |

종료 코드 4 는 이 표의 `log_pipe_backpressure.py` 에만 있다: **관찰이 겨냥한 조건이 생기지 않았다** — PASS 도 FAIL 도 아니다(계약 §7a).

## p1-01 라운드 4 추가 (qa, 계약 7차)

| 스크립트 | 항목 | 비고 |
|---------|------|------|
| `ship_events.py ledger` / `causation-selftest` | SC-81 | 인과 결함(null·**self**·dangling·타입·순서)을 표 전체에서 모아 **동결 장부 7건과 등식**. 구간을 주면 그 구간 결함 0. 존재만 보는 조인은 자기 참조를 통과시킨다. 7차: 세션 간선(`SUPERSEDED` ← 새 `SESSION_OPENED`, 나머지 원인 null)도 본다. `causation-selftest` 는 합성 17행을 CTE 로 덮어 같은 SQL 을 검사한다(DB 쓰기 없음) |
| `concurrent_session.py run/check` | SC-88 | 같은 라벨 봇 둘 겹침 접속 → (a) 함선 1척 (b) `SUPERSEDED` 원인 = 새 `SESSION_OPENED` (c) close 4001 (e) 유령 0·종료 디스폰 원인 실재. **(d) 재접속 안 함은 Unity 로만**(봇은 원래 재접속하지 않는다) |
