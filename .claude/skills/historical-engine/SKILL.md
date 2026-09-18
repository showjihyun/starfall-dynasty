---
name: historical-engine
description: "STARFALL DYNASTY 역사 엔진(Historical Simulation Engine) 구현 규약. Domain Event → Historical Event 중요도 판정(Level 0~5, rule_version), Evidence 출처·불변성·파생, Claim(주어-술어-목적어), Interpretation 분리, 가시성·Fog of History, 인과·집계 이벤트(Correlation Window), Chronicle·Biography 프로젝션, 멱등·순서·결정성, MVP 역사 이벤트 10종, 필수 속성 테스트, LLM 배치 원칙. 역사 사건·증거·주장·해석·연대기·전기·뉴스·중요도 규칙·역사 조회 API를 설계·구현·검증할 때 반드시 사용."
---

# Historical Engine — 사실, 기록, 증거, 주장, 해석을 섞지 않는 엔진

> **"The simulation creates facts. Players create history."**

역사 엔진은 사실을 만들지 않는다. 시뮬레이션이 만든 사실을 받아 **무엇이 기록되는지, 무엇이 그것을 뒷받침하는지, 누가 어떻게 해석하는지**를 구조화하고, 그 기록이 다음 플레이로 이어지게 한다.

데이터 모델·테이블은 `references/data-model.md`를 읽는다. 기획안 원문 근거는 HSE(역사 엔진 설계) 문서이며 절 위치는 `starfall-spec`의 gdd-index에 있다.

## 1. 책임 분리

| 계층 | 질문 | 누가 만드나 |
|------|------|-----------|
| Simulation / Domain Event | 실제로 무슨 일이 일어났나 | 서버 시뮬레이션 (rust-server-engineer) |
| Historical Event | 그중 무엇이 역사로 기록되나 | Historical Detector (규칙) |
| Evidence | 그것을 무엇으로 알 수 있나 | 시스템 자동 생성 + 플레이어 행동(일지, 보고서) |
| Claim | 누가 무엇이라고 주장하나 | 플레이어·NPC·세력 |
| Interpretation | 사건을 어떻게 이해하나 | 플레이어(역사학자) |
| Consequence | 그래서 세계가 어떻게 변하나 | 각 시스템이 **새 이벤트로** 만든다 |
| Narrative (뉴스·요약) | 사람이 읽는 문장 | 템플릿 / LLM — 엔진 바깥 |

이 표의 경계를 넘는 코드를 쓰지 않는다. 특히:
- 플레이어의 Claim이 Fact로 승격되는 경로를 만들지 않는다.
- Consequence가 다른 시스템의 테이블을 직접 수정하지 않는다. `WAR_DECLARED → (무역 시스템) TRADE_RESTRICTION_CREATED → (시장) PRICE_CHANGED`처럼 각 시스템이 자기 책임으로 이벤트를 만든다.
- LLM은 역사 이벤트 ID를 근거로 받아 문장만 만든다. 결과 문장에는 근거 참조(`event_id`, `evidence_id`)를 남긴다.

## 2. Historical Detector

- 판정은 **명시적인 Rust 함수**로 시작한다. 범용 룰 엔진이나 DSL은 규칙이 많아져 필요가 측정된 뒤 ADR로 도입한다.
- 입력: 도메인 이벤트 + 판정에 필요한 읽기 전용 세계 상태 조회(예: "이 광물의 첫 발견인가"). 출력: `Option<HistoricalEvent>` 또는 기존 집계 이벤트 갱신 요청.
- 모든 Historical Event에 `rule_version`을 저장한다. 규칙을 바꾸면 버전을 올리고 과거 이벤트를 새 규칙으로 다시 계산하지 않는다. 다시 계산하면 역사 왜곡이 된다.
- 기준값(중요도 가중치, 임계값)은 game-designer의 초안을 받아 코드 상수가 아닌 버전 붙은 설정으로 둔다.

### 중요도 레벨

| 레벨 | 의미 | 예 |
|------|------|----|
| 0 | Ephemeral — 역사 아님 | 일반 채굴 10톤 |
| 1 | Local | 소규모 교전 |
| 2 | Regional | 새 광물 최초 발견 |
| 3 | Political | 세력 간 충돌 |
| 4 | Galactic | 전쟁 |
| 5 | Epoch-defining | 문명 붕괴 |

- 중요도 점수는 내부 판단값이다. 플레이어에게 "역사적 가치 87점"처럼 노출하지 않는다.
- 나중에 사건의 의미가 커지면(발견이 전쟁의 원인이 됨) 원본 이벤트를 고치지 않고 **현재 중요도 프로젝션**을 갱신한다.

### MVP 역사 이벤트 10종 (HSE §89)

`MINERAL_DISCOVERED`, `ARTIFACT_DISCOVERED`, `SHIP_DESTROYED`, `PLAYER_DIED`, `PLAYER_CREATED_FACTION`, `TERRITORY_CAPTURED`, `CONTRACT_SIGNED`, `CONTRACT_BROKEN`, `WAR_DECLARED`, `TREATY_SIGNED`

이 밖의 타입은 스펙 + 계약 레지스트리 등록 + "이어지는 플레이" 정의 없이 추가하지 않는다. 모든 행동을 역사로 만들면 역사가 소음이 된다 (목표 압축비: 수백만 행동 → 수천 사건).

### 집계 이벤트

여러 도메인 이벤트(함선 10척 파괴, 사망 3건, 화물 탈취)를 하나의 `BATTLE_OCCURRED`로 묶는다.
- 묶는 기준(Correlation Window: 시간 범위, 거리, 같은 전투 인스턴스, 같은 세력 충돌)을 규칙으로 명시하고 `rule_version`에 포함한다.
- 원자 도메인 이벤트는 감사·재생용으로 남기고, 집계 이벤트는 `source_event_ids`로 연결한다. Event ID와 Historical Event ID는 별개다.
- 창이 닫히기 전 들어온 이벤트로 집계를 갱신하는 것은 허용되지만, 창이 닫힌 뒤(확정 후)에는 새 레코드로만 보완한다.

## 3. Evidence

- Evidence는 "진실도 점수"가 아니다. 군사 보고서는 전투 위치에는 믿을 만하지만 정치적 책임에는 편향될 수 있다. 신뢰성은 **출처(provenance)와 범위(scope)** 로 표현한다.
- 원본 Evidence는 절대 바뀌지 않는다. 기밀 해제, 탈취, 편집, 위조, 출판은 모두 `derived_from_evidence_ids`를 가진 **새 Evidence**다.
- 위조품은 내부적으로 `authenticity_status`로 원본과 구분된다. 플레이어에게 보이는 것과 시스템이 아는 것은 다를 수 있다.
- 시스템 자동 증거(함선 로그, 스캐너 기록)는 도메인 이벤트 처리 시 결정적으로 생성한다.

## 4. Claim과 Interpretation

- Claim은 자연어만 저장하지 않는다. `subject / predicate / object / qualifiers`(시간, 장소) 구조 + 선택적 자연어 문장. 구조가 있어야 검색·모순 탐지·반박 연결이 된다.
- Claim은 `evidence_ids`와 `counter_evidence_ids`를 가진다.
- Interpretation은 여러 Claim·Evidence를 묶은 해석이다. 시스템은 어느 해석이 "정답"인지 자동 판정하지 않는다. 새 증거가 모순을 만들면 해석의 **상태**를 `DISPUTED`로 바꾸는 기록을 추가할 뿐, 해석을 지우지 않는다.
- 평판 높은 역사학자의 주장도 사실이 아니다. 평판과 진실을 연결하는 규칙을 만들지 않는다.

## 5. 가시성과 지식

- 가시성: `PUBLIC`, `FACTION_ONLY`, `PARTICIPANTS_ONLY`, `CLASSIFIED`, `SECRET`, `DISCOVERABLE`.
- 가시성과 증거 존재는 별개다. 비밀 사건에도 증거가 존재할 수 있고, 증거 발견이 공개로 이어지는 것이 게임이다 (Fog of History).
- 역사 조회 API의 응답은 요청자의 권한과 지식 상태로 필터링한다. 조회 자체가 게임 시스템이다.
- 플레이어별 지식 상태는 MVP에서 **중요 발견만** 영속하고 나머지는 파생·캐시로 계산한다. 모든 조합을 행으로 저장하면 규모가 폭발한다.
- 소문(RUMOR)은 자동으로 사실(CONFIRMED)이 되지 않는다.

## 6. 멱등성·순서·결정성

- 같은 도메인 이벤트를 두 번 받아도 역사 이벤트는 하나다: `processed_events(consumer, event_id)` + 역사 이벤트의 근거 이벤트에 대한 유일성 제약.
- 도착 순서를 믿지 않는다. `tick`, `sequence`, `causation_id`로 정렬·연결한다. 원인 이벤트보다 결과가 먼저 오면 보류 후 재처리하거나, 연결을 나중에 추가하는 레코드로 보완한다.
- 판정에 현재 시각, `HashMap` 순회 순서, 시드 없는 난수를 쓰지 않는다.
- 게임 시간(`occurred_at`)과 실제 시간(`recorded_at`)을 분리해 둘 다 저장한다.

## 7. 프로젝션

- Galactic Chronicle, Player Biography, Ship Biography, Faction History는 **원본이 아니라 읽기 모델**이다. 역사 이벤트에서 다시 만들 수 있어야 한다.
- UI가 원본 이벤트 테이블을 직접 가공하지 않게 프로젝션 테이블/뷰를 둔다.
- 프로젝션 재구축 테스트: 프로젝션을 비우고 이벤트를 재생하면 같은 결과가 나와야 한다.
- 프로젝션은 결과적 일관성(eventual consistency)을 허용한다. 돈·인벤토리와 달리 몇 초 늦어도 된다.

## 8. History → Content

역사 엔진의 목적은 데이터 축적이 아니라 새 플레이다. 새 역사 이벤트 타입이나 프로젝션을 만들 때마다 확인한다.
- 이 사건은 누가 **발견**할 수 있는가? (조사, 뉴스, 현장 잔해, 함선 로그)
- 발견한 플레이어가 **할 수 있는 행동**은? (현상금, 복수, 유물 추적, 반박 연구, 정치 개입)
- 그 행동이 다시 도메인 이벤트를 만드는가?

셋 중 답이 없는 항목이 있으면 스펙의 "열린 질문"으로 올린다.

## 9. 필수 테스트 (HSE §91–92)

| 테스트 | 기준 |
|-------|------|
| 규칙 단위 테스트 | 레벨 경계값마다 판정 결과 |
| 통합: 이벤트 → 역사 | 도메인 이벤트 입력 → 역사 이벤트·증거·프로젝션 행 |
| 속성: 멱등 | 같은 이벤트 N회 처리 = 1회 처리 결과 |
| 속성: 순서 | 허용 범위 안에서 순서를 섞어도 유효한 결과, 최종 역사 집합 동일 |
| 결정성 | 같은 입력 + 같은 `rule_version` = 같은 결과(해시 비교) |
| 재생 | 이벤트 체인 재생 → 같은 프로젝션 |
| 불변성 | 역사·증거 테이블 UPDATE/DELETE 시도가 실패 |
| 부하 | 100 / 1,000 / 10,000 봇 행동에서 역사 이벤트 생성률과 처리 지연 |
