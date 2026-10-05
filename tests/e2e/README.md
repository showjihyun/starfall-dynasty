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

## 라운드 도구 (2026-10-05, p1-02 기술 부채 1·2)

| 스크립트 | 하는 일 | 비고 |
|---------|--------|------|
| `cargo_sc_map.py scan/run/map/selftest` | 계약 §1 방법 칸의 테스트 지명을 뽑아 **계약 명령 그대로** 필터마다 `cargo test` 를 돌린다. 필터마다 실행 수, 실패, 잡힌 테스트를 다른 SC 도 지명했는지(`also_named_by`)를 찍는다. 실행 0 = **유령**, 코드에 없는 맨 이름 = 정적 유령 | r1~r3 결함 셋을 고쳤다: 백틱 안의 맨 이름, `--test X` 같은 값 플래그, 열 밀림(`\|` 이스케이프·CR). 셸·TSV 를 거치지 않고 `subprocess` 인자 목록으로 실행한다. 종료 1 = 유령·실패 있음 |
| `run_round.py` | 라운드 판정 **입력**을 한 번에 모은다. 단계: freeze → gates → census → repeat → unity → filters → sources → offline → bots → judges → db_stop(사람 단계) → final | 슬라이스 값은 `round_configs/<슬라이스 폴더>.json` 에 둔다. 판정(PASS/FAIL)은 리포트가 계약 문구로 한다 |

```bash
# 계약 필터 전수 실행 (DB 테스트 포함 — .env 를 읽고 STARFALL_DB_TESTS=required)
python tests/e2e/cargo_sc_map.py run --contract _workspace/<slice>/02_sprint_contract.md --out <dir>

# 라운드 한 번 (출력: _workspace/<slice>/evidence/<round>_<YYYYMMDD>/summary.json)
python tests/e2e/run_round.py --slice p1-02 --contract _workspace/p1-02-mining/02_sprint_contract.md --round r4
# r2 부터는 범위를 좁힌다 — freeze·final 은 --only 에 없어도 늘 돈다
python tests/e2e/run_round.py ... --round r4 --only gates,census,filters,sources
# 명령이 실제로 있는지만 본다
python tests/e2e/run_round.py ... --round r4 --dry-run

# 기다리는 쪽은 같은 턴에서 (R1). DONE 은 summary.json 의 마지막 줄이다
until grep -qx DONE _workspace/<slice>/evidence/<round>_<날짜>/summary.json; do sleep 20; done
```

- **사람 단계(db_stop):** `--approve db_stop` 이 없으면 `<out>/WAITING_db_stop.txt` 를 쓰고, `<out>/go_db_stop`(또는 `skip_db_stop`) 파일을 `--gate-timeout` 초(기본 1800)까지 기다린다. 시간이 다 되면 `미검증(사람 대기)` 로 두고 계속한다. 진행 상황은 `<out>/progress.txt` 에서 본다.
- **동결 감시(R6):** 15 초마다 소스 mtime(바뀌면 sha256 확인)과, 실행기의 자손이 아닌 `cargo`·`rustc`·`Unity` 프로세스를 본다. 위반이 겹친 단계는 `무효(동결 위반)` 이 된다. 원래 결과는 `raw_status` 에 남는다. 소스 변화와 외부 `cargo`·`rustc` 는 모든 단계를 무효로 한다. `client/` 를 보는 Unity 프로세스(Editor·AssetImportWorker)는 client 범위 단계(`unity`, `offline` 의 codegen 검사)만 무효로 하고, 서버 범위 단계(gates·census·repeat·filters·sources·bots·judges·db_stop·freeze·final)에서는 기록만 한다(`run_round.py` 의 `STAGE_WATCH`). `--strict-unity` 로 이 완화를 끈다. `_workspace/<slice>/FREEZE` 가 없으면 경고한다.
- **summary.json 읽기:** 끝에 `DONE` 줄이 붙어 있어 `json.load` 로는 읽히지 않는다. `run_round.load_summary(path)` 나 `json.JSONDecoder().raw_decode(text)[0]` 를 쓴다.
- **하지 않는 것:** 서버 하드 킬(봇 그룹은 stop 파일 → `server_boot` 가 stdin `shutdown`. 안 내려가면 죽이지 않고 FAIL 로 남긴다) · `docker compose down` · 실행기 SQL 의 쓰기(SELECT/WITH 만, 세션 read-only) · `STARFALL_REPLAY_BLESS`(자식 환경에서 지운다) · `.env` 값 출력.
- **설정에 없는 것:** 부하 ×100 사본(SC-87·89)은 data 사본을 준비해야 해서 넣지 않았다. 정적 grep(SC-02·03·95)은 사람이 읽는 증거라 넣지 않았다. SC-109 탐침은 증거 DB 에 INSERT(롤백)하므로 넣지 않았다.

## 사람 Unity 세션 준비 — `unity_session.py`

빈 씬(환경 변수 미주입)과 세션 중 서버 종료를 막는 준비 도구다. 판정 도구가 아니다(`starfall-dev/references/screen-elements.md` S4).

    python tests/e2e/unity_session.py start  --slice p1-02-mining --tag SC-68 --new-world   # 또는 --world <id>, --hours 6
    python tests/e2e/unity_session.py status --slice p1-02-mining --tag SC-68   # Play 뒤: live_connections, 이 월드의 이벤트 종류별 개수
    python tests/e2e/unity_session.py stop   --slice p1-02-mining --tag SC-68   # stop 파일 → stdin shutdown. Editor 는 사람이 닫는다

- `start` 는 이 프로젝트의 Editor 가 열려 있거나 `client/Temp/UnityLockfile` 이 잠겨 있으면 "Editor 를 닫아 주세요" 하고 멈춘다(프로세스를 죽이지 않는다).
- Editor 는 스크립트가 띄운다. `STARFALL_GREYBOX_AUTOBUILD=1`·`STARFALL_NET_AUTOCONNECT=1`·`STARFALL_WS_URL`·`.env` 의 dev 비밀값은 **그 자식 프로세스 환경에만** 들어간다. `-logFile` 은 `_workspace/{slice}/evidence/{tag}/unity_editor.log`(절대 경로)이고, 파일이 생겼는지 확인한다.
- 증거: `evidence/{tag}/session.json`(월드·pid·stop/ready 파일·로그 경로·HEAD·porcelain, 비밀값은 출처만), `serve_stdout.log`, `server.log`(stop 뒤), `new_world.json`.
- **절전 주의:** 서버 실행 한도(`--hours`, 기본 6)는 절전 중에도 흐를 수 있다(SC-68 1차에 이것으로 서버가 꺼졌다). 세션 동안 PC 절전을 끄거나 한도를 넉넉히 잡는다. 꺼졌는지는 `status` 의 `server_pid_alive`·`stats` 로 보이고, 같은 월드로 `stop` → `start --world <id>` 하면 된다.
- `--skip-editor` 는 도구 자체 점검용(서버까지만).
- 한계: Editor 기동 단계(실제 Unity 실행·로그 생성·Play 시 씬 생성)는 2026-10-05 도구 점검 때 Editor 가 열려 있어 실측하지 못했다. 다음 사람 세션 전에 Editor 를 닫고 `start --new-world --tag TOOLCHECK2` 로 한 번 확인한다.
