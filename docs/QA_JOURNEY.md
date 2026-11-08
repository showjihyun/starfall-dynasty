# 플레이어 여정 QA (랜딩 → 캠페인 완료)

`cargo run -p bot --release --example journey [-- --no-campaign] [--hours 10] [--hull Talon --species Human]` — 새 플레이어가 겪는
순서를 단계별로 PASS/FAIL로 확인한다(`bot::journey`). 창 없이, 실제 규칙(`sim::Game`)과 클라이언트의 화면 로직(`view`)으로:

| 단계 | 확인하는 것 |
|---|---|
| 랜딩 → 접속 | 이름을 보내면 토큰과 「캐릭터 없음」이 와서 생성 화면으로 (`client/src/state.rs`의 화면 전환과 같은 규칙, `journey::screen_after`) |
| 캐릭터 생성 → 첫 화면 | 프로필·성계 진입 → 게임 화면, 자기 함선이 몇 초 만에 화면에 보이는가 |
| 첫 안내 | 추적기가 할 일을 말하는가 (`view::hud::next_goal`) |
| 오프닝 영상 | 대본 검사(`view::cinematic::check`), 길이 ≤ 180초, 자막 수 |
| 1장 시작 → 첫 목표 | 시작하면 추적기에 첫 단계 |
| 장 끝 통신 | 마지막 장을 빼고 모든 장이 통신으로 끝나는가 (`comms.ron`) |
| 단계 조작 힌트 | 단계마다 어떤 키로 하는지 (`view::hud::step_hint`), 정박 중이면 「G 출항」부터 |
| 단계 문구 / 다음 장 안내 | 모든 단계에 할 일, 장을 끝내면 다음 장을 가리키는가 |
| 캠페인 완주 | 캠페인 조종사(`sim::playthrough`)가 1–5장을 끝내는가, 장별 시간·사망 |

테스트 `crates/bot/tests/journey.rs`는 첫 세션 단계를 매번 확인한다(캠페인 제외, 0.02초).

## 첫 측정 (2026-11-08, Human · CombatPilot · Talon)

발견하고 고친 것:
1. **새 함장에게 할 일이 안 보였다.** 캐릭터를 만들면 오른쪽 위에는 일일 미션뿐이고, 캠페인은 미션 창(J)을 열어 찾아야 했다.
   → 추적기 맨 위에 「▶ 캠페인 시작: 「1장 · 오리온의 신호」」와 **「시작」 버튼**(1장이면 오프닝 영상부터, 미션 창과 같은 흐름).
   장을 끝낼 때마다 다음 장을 같은 자리에서. 실제 클라이언트 화면으로 확인(새 함장 → 버튼 → 「Esc 건너뛰기」 오프닝).
2. **목표는 있는데 방법이 없었다.** 「Orion Drift 성계로 워프」만 있고 키가 없었다. → 단계 밑에 조작 힌트(「G 출항 → M 지도에서 성계를
   골라 워프」, 「화면의 표시를 따라가 가까이서 V 스캔」, 「T 목표 지정 · Space 사격 · 밀리면 G 도킹해 수리」).

```
PASS 랜딩 → 접속 — welcome true · token true · screen "Create"
PASS 캐릭터 생성 → 첫 화면 — own ship on screen after 0.1 s
PASS 첫 안내 — ▶ 캠페인 시작: 「1장 · 오리온의 신호」
PASS 오프닝 영상 — 90 s, 10 captions
PASS 1장 시작 → 첫 목표 — Orion Drift 성계로 워프
PASS 장 끝 통신 — 4 chapters close with a call
PASS 단계 조작 힌트 — first: G 출항 → M 지도에서 성계를 골라 워프
PASS 단계 문구 — 16 steps, each says what to do
PASS 다음 장 안내 — after each chapter the tracker names the next
PASS 캠페인 완주 — 5/5 chapters
first session: name → own ship 0.1 s · opening 90 s · 1장 0 min (0 deaths) · 2장 1 min · 3장 7 min (2) · 4장 34 min (8) · 5장 23 min (4)
campaign: 5/5 chapters in 65 min of game time
```

남은 것(흐름):
- **1–2장이 너무 짧고 4장에서 벽이 선다.** 조종사 기준 1장 1분 미만·2장 1분, 그다음 3장 7분, 4장 34분(사망 8). 사람은 조작을
  익히는 시간이 더해지지만, 1장이 조작을 차례로 가르치지 않는다(워프 → 스캔 → 해적 2척). 후보: 1장에 출항·비행·목표 지정·도킹
  단계를 넣는 튜토리얼화, 4장 앞의 성장 구간 완화(docs/QA_CAMPAIGN.md 중앙값과 함께 볼 것).
- 조종사 시간은 숙련자 기준이다. 사람의 첫 세션 시간은 이 하네스로 잴 수 없다 — 플레이 테스트 지표(첫 사망까지, 첫 장 완료까지)를
  서버 지표(`server --metrics`)에서 뽑는 것이 다음 단계.
