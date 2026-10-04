# p1-02-mining — designer 노트 (결정하지 못한 트레이드오프)

작성: designer · 2026-09-27 · 본문: `docs/design/p1-02-mining-design.md`

## 산출물
- `docs/design/p1-02-mining-design.md`
- `data/minerals/{ferrosite,glacine,cobaltine,starfall-glass}.json`
- `data/world/deposits/cradle.json` (서버 전용 — 미확인 광맥의 광물·매장량은 클라이언트로 가면 안 된다)
- `data/mining/mining-rules.json`

## 아직 아무것도 이 파일들을 검사하지 않는다
- 스키마 없음(`contracts/data/` 에 mineral / deposit-field / mining-rules 필요 — architect).
- `server/bins/game-server/src/data.rs` 가 새 경로를 읽지 않는다. `tests/e2e/validate_data_files.py` 의 `TARGETS` 에도 없다. 둘 다 소스를 읽어 확인한 부정이다(규칙 9).

## 열린 트레이드오프 (디자인 문서 §9 Q1~Q6 과 같은 것)
1. 인벤토리 주인 actor vs 함선, 상한 — 추천 actor·상한 없음.
2. 같은 tick 채굴 순서 — 추천 서버 수락 순서(`ship_id` 순은 영구 불공정).
3. 발견이 같은 광물의 다른 광맥도 드러내는가 — 추천 아니오(시뮬레이션이 역사 판정을 읽는 첫 경로가 된다).
4. 발견 시나리오 반복에 새 `world_id` 가 필요 — `down -v` 금지 때문에 절차가 있어야 한다.
5. 발견 배너 대상 — 추천 월드 전원(PUBLIC).
6. 회복 포함 — 추천 포함(월드 tick 배수, 상태 추가 없음).

## 약하게 확신하는 수치
- 쿨다운 3 s·사거리 150 m·속도 10 m/s 는 p1-01 이동 수치에서 유도했지만 사람 세션 한 번 없이 정했다. §7.2 거절 비율이 30% 를 넘으면 가장 먼저 사거리를 의심한다.
- Starfall Glass 거리(스폰에서 ~47 s)가 "5~15분 안에 발견" 목표를 만드는지는 모른다. 미확인 광맥이 8개뿐이라 외딴 광맥 하나가 오히려 먼저 의심받을 수 있다.
