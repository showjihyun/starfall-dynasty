# p1-05-basic-combat

## 요청 원문
"전투 스펙 초안 병행으로 맡겨줘" (2026-10-10) — p1-03 r1 진행 중 병행.

## 분류
- 유형: SPEC (지금은 Phase 2 만). 구현은 p1-03 마감·p1-04 비주얼 뒤.
- 로드맵: p1 코어 프로토타입의 "기본 전투"(GDD §36 MVP-1). p1 종료 기준 "핵심 행동이 서버 판정 + 역사 기록으로 동작"의 마지막 핵심 행동.
- 역사: MVP 역사 이벤트 10종(HSE §89) 중 `SHIP_DESTROYED`, `PLAYER_DIED` — 이 슬라이스가 처음 쓴다.

## 범위 초안 (Phase 2 에서 architect·designer·history 가 확정)
- 1인 함선 대 1인 함선. 서버가 명중·피해·파괴를 판정한다(원칙 1). 클라이언트는 조준·발사 의도만 보낸다.
- 무기 1종(GDD 6.1 Weapon 1)부터. 결정적 판정(원칙 9), 같은 명령 재전송 멱등.
- 피해·선체 내구도, 파괴 → 재생성(어디서·무엇을 잃는가: 인벤토리·크레딧 처리 규칙).
- 역사: 파괴·사망 중 무엇이 Historical Event 인가(중요도 판정 — 모든 파괴가 역사는 아니다, 원칙 4), 후속 플레이 연결, "다른 플레이어가 흔적을 발견하는가"(잔해·사건 현장).
- p1-04 비주얼 기준선(은하영웅전설풍 v3)과 맞물리는 화면 요소는 UI-nn 표로(screen-elements S1). VFX 는 techart 와 경계.
- 범위 밖: 함대전·대형 함선·NPC 해적·현상금(p2)·길드전.

## 병행 제약 (p1-03 r1 동결 중)
- `contracts/`, `data/`, `server/`, `client/` 는 동결 — **지금은 문서만**: `docs/specs/p1-05-basic-combat.md`, `docs/design/p1-05-basic-combat.md`, ADR 초안(docs/adr — 동결 대상 아님), `_workspace/p1-05-basic-combat/`.
- 계약 스키마·fixture·data 표는 r1 동결 해제 뒤 작성.

## 투입
architect5(스펙·ADR), designer5(규칙·수치·재미 시나리오·UI 표), history5(역사 검토, opus).
