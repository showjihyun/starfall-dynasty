# STARFALL DYNASTY — Rust 3D 우주 멀티플레이어

> THE GALAXY IS NOT A MAP. IT IS A HISTORY.

`기획안/`의 설계를 Rust로 구현한 수직 슬라이스. 권위 서버(Rust/tokio) + 3D 클라이언트(Bevy 0.20) + 역사 엔진.
함선 모델은 Blender 5.2에서 헤드리스 Python으로 모델링·베이크했고(`tools/blender/`), 우주·함선 셰이더와 이펙트는 WESL로 작성했다.

## 빠르게 실행

```
cargo run -p server --release
cargo run -p client --release
```

다른 PC에서 접속: `cargo run -p client --release -- --server ws://<서버IP>:7777`

## 게임 개요

- **함선은 단 한 척** — 캐릭터 생성 때 5종 중 하나(Wren 정찰기 / Talon 요격기 / Bastion 프리깃 / Mule 채굴선 / Seer 조사선)를 고르면 평생 교체할 수 없다. 8개 모듈(선체·반응로·엔진·워프코어·실드·무장·스캐너·화물)을 Lv1→Lv10으로 올려 Scout → … → Leviathan급까지 키운다. 등급이 오르면 모델에 추가 파츠(Rank3/6/9)가 붙는다.
- **업그레이드 재료**는 일일 미션, 교전(해적 격파·함대전 승리·현상금), 시나리오 돌파, 채굴·정제·판매, 탐험(스캔)으로 얻는다. 모듈 상한은 Lv5에서 시작하고 **시나리오 한 장을 돌파할 때마다 +1**.
- **탐험** — 10개 성계(위험도 CORE → FRONTIER → WILD → WAR ZONE → DEEP). 워프 거리는 워프 코어 레벨에 비례하고, The Deep은 업그레이드해야 닿는다. 이상현상·유적·잔해·신호·고대 게이트를 스캔하면 보상이 있고, 최초 발견은 연대기에 남는다.
- **네트워크 교전** — 같은 성계의 모든 함장이 실시간으로 보인다(20 Hz 서버 권위 시뮬레이션). 함대(최대 5척)는 아군 사격 없이 보상을 공유한다. WILD에서 무고한 함장을 공격하면 수배·현상금, WAR ZONE은 자유 교전. 함대 대 함대 교전이 끝나면 「제N차 ○○ 전투」로 연대기에 기록된다.
- **역사 엔진** — 행동 → DomainEvent → 중요도 판정 → 연대기/전기/Herald 뉴스. 격침된 함선의 잔해는 7일간 남고, 다른 함장이 잔해를 조사해 봉인된 함선 로그(증거)를 얻은 뒤 연구를 발표하거나, 증거를 쥔 함장이 반박할 수 있다. 반박이 쌓이면 「역사 논쟁」이 새 사건이 된다. 이벤트 소싱(JSONL)으로 저장되어 재시작 시 리플레이로 복원된다.

## 조작

| 키 | 동작 |
|---|---|
| W / S | 추진 / 역추진 |
| A / D, R / F, Q / E | 좌우 선회, 상하, 롤 |
| 우클릭 유지 | 마우스 방향으로 조종 |
| Shift | 부스트 |
| Space / 좌클릭 | 레이저 |
| T / X | 목표 지정 / 미사일 |
| G | 도킹·출항 (도킹 시 수리) |
| N | 가장 가까운 소행성 채굴 |
| V | 가장 가까운 지점 스캔 / 잔해 조사 |
| M U J H K | 은하 지도 · 정거장(업그레이드/판매) · 임무 · 연대기 · 함대 |
| F1 | 도움말 |

## 구조

```
crates/shared   프로토콜(JSON over WebSocket), 함선/모듈/업그레이드 규칙, 결정적 은하 생성, 일일 미션·시나리오
crates/history  역사 엔진: 중요도, 증거(봉인/발견), 연구·반박, 인과 연결, 전기, Herald, JSONL 리플레이
crates/server   권위 서버: 비행 물리, 무기, NPC AI, PvP 규칙, 교전 추적, 채굴/시장, 함대, 영속화
crates/client   Bevy 0.20 클라이언트, src/shaders/*.wesl (스카이박스·항성·행성·고리·선체·엔진·실드·빔·폭발·워프·글로우)
crates/bot      헤드리스 봇(실제 소켓) — E2E/부하 테스트
tools/blender   함선·프롭 모델링/베이크 스크립트 (README 참조)
```

## 테스트

```
cargo test -p shared -p history -p server
cargo build -p server && cargo test -p bot
```

- `server/tests/game.rs` — 함선 1척 제한, 워프 거리, PvE 보상/일일 진행, PvP 규칙·전투 기록·잔해 조사·연구, 수배·현상금, 함대, 업그레이드 상한, 시나리오 1장 완주, 채굴/시장, 영속화
- `bot/tests/e2e.rs` — 실제 서버 프로세스에서 두 함장이 3회 워프 후 War Zone에서 교전 → 연대기 확인, 30명 동시 접속

## 시각 확인 도구

```
client --showroom [성계번호] [--hull talon]      서버 없이 모델·셰이더·이펙트 확인
client --name X --auto --script "undock 1;warp 3;attack 40" --shot out.png --shot-at 30
```
