# tools/bots — QA 봇 하네스 (p0-02, T12)

스프린트 계약 `_workspace/p0-02-networking-spike/02_sprint_contract.md` §3 의 구현.
**server 워크스페이스와 분리된 별도 Cargo 워크스페이스**이고, 레포 루트 `rust-toolchain.toml`(1.98.1)을 상속한다.

## 빌드와 자체 검증 (서버 없이 된다)

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cd /c/WorkSpace/SpaceHistoric/tools/bots
cargo build  --offline
cargo test   --offline          # 33건: 계측기 13 / 루프백 6 / 와이어 6 / 토큰 8
cargo clippy --offline --all-targets
cargo fmt --check
```

`cargo test` 가 검증하는 것은 "잘 도는가"가 아니라 **"틀렸을 때 빨간불이 켜지는가"**다.
가짜 서버가 응답을 1건 빠뜨리면 손실 1로, 순서를 뒤집으면 I-15 위반으로 잡히는지 본다.
이게 없으면 부하 실행의 "손실 0"이 신호인지 침묵인지 구분할 수 없다.

## 실행

```bash
export STARFALL_DEV_AUTH_SECRET=dev_only_not_a_secret   # 기본값을 코드에 두지 않는다 (ADR-0008 §3)

# 신원 — Unity 주체(01a0b1c2-7e57-7c11-8e57-000000000001)와 겹치지 않는지
./target/debug/bots identities --count 30 --disjoint-from 01a0b1c2-7e57-7c11-8e57-000000000001
./target/debug/bots token --label bot-000

# 부하 단계 (스펙 §7). EV 는 증거 디렉토리
EV=/c/WorkSpace/SpaceHistoric/_workspace/p0-02-networking-spike/evidence/load
./target/debug/bots run --scenario a --bots 30 --seed 42 --duration 60 --ramp 5 --interval 500 \
    --out $EV/a --live-corr $EV/a/correlations.live.txt
./target/debug/bots run --scenario b --bots 30 --seed 42 --cycles 5 --pings 3 --out $EV/b
./target/debug/bots run --scenario c --bots 30 --seed 42 --duration 60 --burst 2000 --out $EV/c
./target/debug/bots run --scenario d --bots 30 --seed 42 --duration 150 \
    --out $EV/../durability/d --live-corr $EV/../durability/d/correlations.live.txt

# 서버 단독 항목 (SC-14·19·20·21·22·24·25·26)
./target/debug/bots probe --case auth-ok|order|duplicate|inflight|slow-consumer|oversize|binary|idle
```

보통은 손으로 부르지 않고 `python tests/e2e/run_block.py load|durability|probes` 가 순서·증거까지 같이 처리한다.

## 종료 코드

| 코드 | 뜻 |
|-----:|----|
| 0 | 실행 성공 + 게이트 통과 |
| 1 | 게이트 위반 (손실·중복·순서 위반·서버가 먼저 닫음 등 — 증거는 `--out` 에) |
| 2 | 사용법·설정 오류 (비밀 미설정 포함) |
| 3 | 실행 실패 (연결 0건 — 서버가 없거나 401) |

## 산출물 (`--out`)

| 파일 | 내용 |
|------|------|
| `summary.json` | `gates` + `aggregate`(손실·중복·미대응·거부 사유별) + `rtt`/`ack` 분포 + 시드·시각 |
| `sessions.json` | 세션별 `correlation_id`·`actor_id`·`tick_hz`·close code/사유/주체 |
| `commands.csv` | 명령 1건 = 1행. `sent_us,result_us,reply_us,order,rtt_ms` |
| `correlations.txt` | 종료 시점의 correlation 집합 (§0.6) |
| `correlations.live.txt` | `--live-corr` 지정 시 **실행 중** 갱신 (SC-61) |

`*_us` 는 실행 시작 기준 마이크로초다. `summary.json` 의 `clock_base_unix_ms` 를 더하면 벽시계가 된다
(AC-19 의 DB 중단 구간 슬라이싱에 쓴다).

## 설계상 일부러 그렇게 한 것

- **계약 타입을 독립으로 썼다.** `starfall-contracts` 를 path 의존하지 않는다 — 봇은 3자 대조의
  독립 축이어야 하고, 서버와 같은 타입을 쓰면 서버가 틀렸을 때 봇도 같이 틀린다(I-25).
  대신 `tests/wire_fixtures.rs` 가 `contracts/fixtures/**` 로 모양을 고정한다. (T12 지시와 다르므로
  **architect 통지 사항**이다.)
- **왕복은 봇의 단조 시계로만** 잰다. `client_sent_at` 은 `null` 로 보낸다(I-11).
- **분위수는 nearest-rank.** 보간하면 p99 를 `commands.csv` 에서 되짚을 수 없다.
- **거부는 손실이 아니다.** `TOO_MANY_IN_FLIGHT` 도 응답이므로 1:1 게이트는 유지된다(AC-18a).
- **시드**는 초기 위상 지터에만 쓴다. 판정에 쓰이는 값은 시드에서 나오지 않는다.

## server 구현이 끝나면 맞춰야 할 것

계약 §3.3 의 10개 항목(토큰 HMAC 입력, 환경 변수 이름, `/debug/stats` 키 이름, 503 `reason`,
백로그 임계 등). **서버가 정본이고 이 도구를 고친다.**
