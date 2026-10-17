# STARFALL DYNASTY — 아트 디렉션

이 문서는 게임의 **시각 스타일의 기준**이다. 새 함선·환경·이펙트·UI를 만들 때, 그리고 사람이나 에이전트가 생성 프롬프트를 쓸 때 여기서 출발한다.

- 색 값의 단일 출처는 코드에 있다: `crates/view/src/palette.rs`. UI 색은 `Color32`로, 3D 아군·적 색은 `side`로 정의되어 있다.
- 승인된 모습의 단일 출처는 참고 이미지 `docs/art/reference/*.png`이다. `python tools/artcheck.py`가 현재 렌더를 이 이미지와 비교한다.
- 수치는 2026-10-17 코드 기준이다. 코드가 바뀌면 이 문서도 같은 커밋에서 고친다.

---

## 1. 비전과 기둥

**한 줄 비전:** 닳고 그을린 산업 함선들이, 별빛에 물든 거대한 성운 속에서 위아래 없이 싸우고, 그 흔적이 역사로 남는다.

| 기둥 | 뜻 | 화면에서 지키는 규칙 |
|---|---|---|
| **P1. 닳은 산업, 살아 있는 하늘** | 함선은 도구다. 패널 이음새, 그을음, 칠 벗겨짐이 있다. 하늘은 반대로 회화적이고 부드럽다. | 선체는 무광에 가깝다(거칠기 0.38–0.62). 성운은 항성 색에서 파생한다. 깨끗한 SF 흰 선체는 쓰지 않는다. |
| **P2. 빛은 정보다** | 빛나는 것은 의미가 있는 것뿐이다: 엔진, 무기, 실드, 결정, 유적 문양, 위험. | 블룸은 진짜 밝은 것만 받는다(threshold 0.9). 아군·적 색은 관계를 뜻하고 장식으로 쓰지 않는다. |
| **P3. 규모가 읽힌다** | 전투기와 모함은 실루엣만 보고도 구분돼야 한다. | 림 라이트로 실루엣을 살린다. 크기 등급마다 엔진 표현이 다르다(소형은 토치, 대형은 이온 뱅크). |
| **P4. 3D 공간감** (최종 목표) | 평면이 아니다. 위·아래·깊이가 보인다. | 거리 안개, 고도 기둥, 기울어진 궤도. 기준면이 없는 연출을 피한다. |
| **P5. 흔적이 남는다** | 전투와 시간이 표면에 기록된다. | 피해에 따라 그을음과 불씨가 생긴다. 잔해는 남는다. 오래된 것(유적)은 다른 빛(청록 문양)을 낸다. |

---

## 2. 마스터 스타일 프롬프트

모든 생성 프롬프트(콘셉트 이미지, Blender 스크립트 지시, 텍스처)는 이 블록으로 시작한다. 그 뒤에 4~9절의 카테고리 프롬프트를 붙인다.

```text
STARFALL DYNASTY art direction. Grounded, used-future space opera: weathered industrial
starships with panel seams, edge wear, streak grime and scorch, matte painted metal
(roughness 0.4-0.6), never glossy-white sci-fi. Set against painterly, volumetric nebulae
tinted by the local star, dense small star fields, a faint galactic band.
Light carries meaning: only engines, weapons, shields, crystals, ancient glyphs and danger
glow; friendly glow is cool blue-cyan, hostile glow is red-orange, legendary/selection is
warm gold, deep space and the exotic are violet. Strong silhouette rim light, one hard key
light from the star, soft coloured fill from the nebula, black space (no grey haze).
Cinematic but readable: true 3D space with height and depth, ships at varied altitudes.
Color grading: TonyMcMapface tonemapping, restrained bloom.
```

```text
(한국어 요약) 닳은 산업 우주 + 별빛에 물든 회화적 성운. 무광 금속, 패널 이음새·그을음.
빛 = 의미(엔진·무기·실드·결정·유적·위험). 아군 청록, 적 적주황, 선택·전설 금색, 심우주·이색 보라.
강한 림 라이트, 항성 키라이트 하나, 성운 색 채움광, 검은 우주. 높이와 깊이가 보이는 진짜 3D.
```

**금지:** 반짝이는 흰 플라스틱 선체, 네온 색 남용(의미 없는 발광), 회색 안개 낀 우주, 모든 함선이 같은 높이에 있는 구도, 만화풍 외곽선.

---

## 3. 색 (단일 출처: `palette.rs`)

### 3.1 의미 색 — 이 색들은 의미를 바꾸지 않는다

| 토큰 | 값 (sRGB) | 뜻 | 쓰는 곳 |
|---|---|---|---|
| `ACCENT` | (110,190,255) | 정보, 중립 함장, 아군 계열 UI | 제목, 링크, 워프 사거리 |
| `GOLD` | (255,205,110) | 나, 선택, 전설 | 내 함선, 선택 테두리, 전설 등급 |
| `GREEN` | (110,230,150) | 아군, 안전, 건강 | 아군 함선, Core 성계, 선체 > 60% |
| `RED` | (255,95,80) | 적대, 위험 | 적, War Zone, 선체 위험 |
| `LOSS` | (255,170,60) | 방금 잃은 것 | 게이지 손실 흔적 |
| `DIM` | (140,150,170) | 보조 정보 | 설명, 비활성, 일반 등급 |
| `SHIELD` | (80,160,255) | 실드 | 실드 게이지·바·표시 |
| `POI` | (130,230,255) | 아직 스캔하지 않은 장소 | 지도·HUD 표식 |

### 3.2 위험도 (`tier_color`): 안전한 초록에서 심우주 보라로

Core `GREEN` → Frontier (255,210,90) → Wild (255,140,60) → War Zone `RED` → Deep (190,120,255)

### 3.3 희귀도 (`rarity_color`): 가치가 오를수록 차갑다가 금색으로

일반 `DIM` → 고급 (215,222,235) → 희귀 (120,190,255) → 이색 (200,140,255) → 전설 `GOLD`

### 3.4 3D 아군·적 (`palette::side`, 선형 RGB)

함선의 발광, 엔진, 실드, 사격 색은 **나와의 관계**로 정한다(`relation::Relation::Hostile`이면 적 색). PvP 성계에서 적이 된 함장도 해적처럼 붉게 보인다. 관계가 바뀌면 즉시 다시 칠한다.

| 요소 | 아군 `FRIEND` | 적 `HOSTILE` |
|---|---|---|
| 선체 발광 | [.45,.8,1] | [1,.25,.15] |
| 엔진 외피 / 심 | [.3,.6,1] / [.85,.95,1] | [1,.3,.1] / [1,.8,.5] |
| 실드 | [.35,.7,1] | [1,.35,.2] |
| 레이저 | [.25,.75,1] | [1,.25,.12] |
| 포탑 펄스 | [1,.75,.3] (호박색: 포탑 사격은 따뜻하게 구분) | [1,.3,.15] |
| 주포 빔 | [.55,.8,1] | [1,.45,.3] |

명도와 채도가 요소마다 다른 것은 **의도된 램프**다. 같은 계열(청록 또는 적주황) 안에서만 움직인다. 새 이펙트는 이 표에 열을 추가해서 만들고, 표 밖의 색을 새로 만들지 않는다.

### 3.5 항성과 성운

| 항성 | 색 (선형) | 반경 | 분위기 |
|---|---|---|---|
| 황색 왜성 | [1,.86,.55] | 600 | 따뜻하고 안정적. 인류 수도 |
| 적색 거성 | [1,.45,.25] | 900 | 녹슨 붉은빛, 전쟁과 황무지 |
| 청색 거성 | [.55,.72,1] | 900 | 차갑고 장엄함. 기억과 전선 |
| 백색 왜성 | [.92,.95,1] | 260 | 창백하고 낡음. 옛 항로 |
| 중성자성 | [.7,.55,1] | 260 | 보라색, 이질적. 유적과 심우주 |

- 성운 두 색은 항성 색에서 채널을 섞고 무작위로 감쇠해 만든다(`galaxy.rs` system_layout).
- 성운 밀도는 0.35 + 위험도 × 0.1이다. 위험한 성계일수록 하늘이 짙다.

### 3.6 도색(리버리)

티타늄 [.85,.85,.88], 네이비 [.15,.45,.95], 앰버 [.95,.55,.1], 제이드 [.1,.75,.55], 크림슨 [.75,.15,.2], 바이올렛 [.55,.3,.9], 골드 [.95,.85,.2]. 해적은 적색 [.8,.12,.1] 하나만 쓴다(플레이어는 고를 수 없음).

---

## 4. 종족 비주얼 언어 *(2026-10-17 승인 · 1단계 셰이더 구현)*

**구현 상태:**
- **1단계(완료):** 함장의 종족이 함선 정보(`EntityState.species`)로 전달된다. 호위함은 함장의 종족을 따른다. 선체 셰이더(`hull.wesl`)가 선체·리버리 판에 종족 스타일을 입힌다. 값은 `palette::species_style`에 있다.
  - Human: 기준 룩. 추가 처리가 없다.
  - Veyr: 진주빛 어두운 청록, 숨 쉬듯 맥동하는 가는 보라 맥, 보는 각도에 따라 보라에서 청록으로 바뀌는 림.
  - Kharz: 검게 그을린 쇠(밝기 ×0.33), 금속성 0.75 이상, 천천히 식는 호박색 열 균열.
  - 맥과 균열은 노이즈 등고선을 `fwidth`로 화면상 몇 픽셀 폭으로 고정해서, 거리와 관계없이 선으로 보인다.
  - 참고 이미지: `human.png`, `veyr.png`, `kharz.png` (`client --showroom 7 --species X`).
- **2단계(예정):** Blender에서 종족별 부착물을 만든다(Kharz 뿔·덧댄 장갑, Veyr 늑골, Human 해치·표기). 노드는 `Species_<종족>_*`로 두고 클라이언트가 해당 종족만 보이게 한다. 리버리 기본값도 종족별로 정한다.
- **규칙:** 종족 색은 관계 색(3.4절)과 겹치지 않는다. Kharz는 빨강이 아닌 호박색, Veyr는 아군 청록과 구분되는 보라를 쓴다.

| 종족 | 기획 키워드 | 형태 언어 | 재질·색 | 발광 | 프롬프트 |
|---|---|---|---|---|---|
| **Human** | 기록, 산업, 군사 | 각진 판재, 계층형 장갑, 리벳, 해치, 표기 문자 | 건메탈, 올리브, 네이비. 안전 호박 띠 | 차가운 청백, 항법등 | `human: modular industrial hull, stepped armour plates, stencilled hull numbers, hazard amber stripes, navy and gunmetal paint, cool white running lights` |
| **Veyr** | 기억, 생체기술 | 늑골, 고리, 곡면, 맥(血管) 같은 홈 | 거의 검은 청록, 진주빛 보라 | 보라·청록 맥동 (.6,.25,1) | `veyr: organic bio-tech hull, hoop ribs and carapace, vein grooves that pulse violet-teal, near-black teal shell with nacre sheen, eye-like canopies, no straight panels` |
| **Kharz** | 계약, 혈통, 전쟁, 용병 | 두꺼운 쐐기, 뿔, 톱니, 덧댄 장갑 | 거의 검은 철 (.13,.12,.115), 놋쇠 장식 | 열기 주황(달아오른 금속), 문장 | `kharz: blackened forged iron hull, wedge prow with horns, riveted patch armour, brass clan sigils, heat-glow orange seams like cooling metal, war-banner fins` |

**고대 문명(유적·게이트·센티널):** 매끈한 단일체, 각인 문양, 청록 빛 (.2,.9,.75)–(.25,.85,1). 프롬프트: `ancient: monolithic seamless stone-metal, engraved glyph bands glowing teal-cyan, impossibly precise geometry, no wear`.

---

## 5. 함선

### 5.1 선체 계열: 실루엣이 먼저다

| 계열 | 길이 (소/중/대, m) | 형태 언어 | 도색 | 대형(Capital) |
|---|---|---|---|---|
| Wren | 20 / 130 / 360 | 앞 카나드, 후퇴익, 날개 끝 포드 | 밝은 회색, 붉은 줄 | 바늘형, 등 돛 |
| Talon | 24 / 150 / 420 | 모따기 링 동체, 전진익, 쌍 나셀, 쌍 기수포 | 짙은 건메탈 | 뱃머리 집게, 레일 주포 |
| Bastion | 36 / 180 / 600 | 층층 장갑, 2×2 엔진, 함교 탑 | 올리브 드랩, 심한 마모 | 망치머리 드레드노트 |
| Mule | 30 / 170 / 520 | 운전실, 트러스 척추, 컨테이너, 채굴 집게 | 카키, 위험 호박, 가장 지저분함 | 모함, 조선소 |
| Seer | 26 / 150 / 450 | 고리 늑골, 빛나는 맥, 눈 캐노피 (Veyr 생체) | 검은 청록과 보라 | 쥐가오리 초승달, 뿔, 집게턱 |

**함선 프롬프트 템플릿:**

```text
[마스터 블록] + [종족 블록] +
Hull family <Wren|Talon|Bastion|Mule|Seer>, size class <Small|Cruiser|Capital> (~<len> m).
Shape: <형태 언어 칸>. Paint: <도색 칸>. Readable silhouette from 2 km; engines at the stern
on Engine_N nodes, guns on Gun_N, turrets on Turret_N with Muzzle_N; glow only on engine
cores, running lights and weapon ports.
```

### 5.2 재질 계약 (`tools/blender/shiplib.py`, README)

| 슬롯 | 값 | 비고 |
|---|---|---|
| Hull | 2048² 베이크 (대형 4096²) | 패널 이음새, 보로노이 서브패널, AO, 줄 오염, 모서리 마모 |
| Livery | 회색 0.8, 금속 0.3, 거칠기 0.4 | 게임에서 리버리 색을 곱함 |
| Glow | 방출 흰색 | 게임에서 관계 색으로 칠함, 세기 4.0 |
| EngineCore | (1,.62,.32) | 게임 세기 1.6 |
| Glass | (.015,.03,.045), 거칠기 0.05 | 캐노피 |

노드 계약: `Engine_N`, `Gun_N`, `Main_N`, `Bridge`, `Turret_N`/`Muzzle_N`, `Rank3_/Rank6_/Rank9_`.

### 5.3 게임 안 선체 셰이더 (`hull.wesl`)

- 리버리 곱셈.
- 발광 맥동 0.75 + 0.25·sin(2t), 부스트하면 더 밝다.
- 피해: (.02,.018,.016)으로 그을리고, 피해가 0.45를 넘으면 불씨 (3,.9,.2).
- 프레넬 림: (.35,.5,.8)과 리버리를 섞어 rim⁴ × 0.35.
- 피격 섬광 (1.5,1.7,2.2).

### 5.4 엔진: 크기 등급이 표현을 정한다

| 등급 | 길이 | 폭 | 이득 | 형태 |
|---|---|---|---|---|
| 소형 | 13 | 0.9 | 1.0 | 토치(충격파 다이아몬드, 40 Hz 깜박임) |
| 중형 | 10 | 0.75 | 0.9 | 이온(일렁임, 필라멘트) |
| 대형 | 9 | 0.7 | 0.8 | 이온 |

대형 함선의 엔진은 "넓게 빛나는 뱅크"로 보여야 한다. 킬로미터 길이의 불꽃이 되어서는 안 된다.

---

## 6. 환경

| 요소 | 기준 | 프롬프트 키워드 |
|---|---|---|
| 하늘 | 반경 400,000 구. 은하띠(법선 .25,1,.35). 도메인 왜곡 fbm 성운, 먼지 띠(최대 55% 어둡게). 별 3층(180/420/900), 반짝임 ±15% | `painterly volumetric nebula tinted by the star, dark dust lanes, three depths of small stars, faint galactic band` |
| 항성 | 쌀알 무늬, 흑점, 주연 감광 .22+.78μ^.55, 채층 테. 코로나 2.6r. 플레어(6 스파이크, 아나모픽 줄) | `granulated star surface, limb darkening, soft corona, restrained anamorphic flare` |
| 행성 | 암석, 바다(구름·극관·대기 [.35,.6,1]), 가스(띠 18개, 폭풍, 고리 1.35–2.3r), 얼음(균열), 용암(방출 맥 [2.2,.6,.1]). 대기 테 1.12r | `readable planet types, thin atmosphere rim, ring with shadow` |
| 소행성 | 광석별 색과 결정 발광: Ferrite [.78,.64,.54]/[1.2,.5,.2], Lumen [.62,.72,.86]/[.3,1.6,2.2], Xenite [.62,.52,.76]/[1.6,.3,2.4] | `dark rock with glowing mineral crystal veins in the ore's colour` |
| 정거장 | 회전 고리 0.04 rad/s, 따뜻한 점광원 (1,.85,.6) | `industrial hub, rotating habitat ring, warm window strips` |
| 점프 게이트 | 방출 (1,3,8), 고리 회전 0.6 | `blue energy ring` |
| 먼지 | 320개 가산 큐브 (.55,.65,.85), 속도에 따라 최대 31배 늘어남 | `speed streak dust` |
| 안개 | 지수 1/16000. 성운 평균 색 × (.05 + .06·밀도) | `depth haze tinted by the nebula, never grey` |

---

## 7. 조명과 후처리

- **키 라이트:** 항성 방향의 방향광, 항성 색, 2600 lx, 그림자 켬.
- **채움광:** 성운 색 방향광 380 lx, 그림자 없음. 선체가 오려 붙인 것처럼 보이지 않게 한다.
- **주변광:** (.5,.55,.7), 밝기 12. 환경맵 세기 700(CPU 큐브맵 64²).
- **카메라:**
  - HDR, 톤매핑 TonyMcMapface, FOV 62°.
  - 블룸: NATURAL, 세기 0.2, 저주파 0.55, 임계값 0.9, 부드러움 0.45.
  - 원칙: **진짜 밝은 것만 블룸**. 새 발광체는 방출 세기로 조절하고, 블룸 설정을 바꾸지 않는다.
- **레이 트레이싱 계층** (2026-10-17 스파이크, Bevy Solari): Vulkan에서만 동작한다(DX12는 wgpu에 ray query가 없음). 노이즈 제거(DLSS RR)가 있어야 쓸 수 있다. 래스터 화면이 기준이고, RT는 같은 아트 디렉션을 더 정확히 비추는 선택 옵션이다.

---

## 8. 애니메이션 원칙

1. **시계의 함수다.** 클라이언트 애니메이션은 시간 t의 함수로 계산하고, 프레임마다 누적하지 않는다(CLAUDE.md 불변식). 예: 회전 `base·euler(spin·t)`, 셰이더 `globals.time`.
2. **완만하게 시작해서 결정적으로 끝난다.** 폭발 크기는 ease-out cubic, 워프 충전은 c².
3. **크기가 클수록 느리다.** 대형 함선은 회전이 느리고(선회 0.7), 엔진이 묵직하다.

**알려진 예외(정리 대상):** 화구 크기와 섬광 세기(`fx.rs`), 워프 발광, 스로틀 보간, 포탑 slerp은 프레임 누적이다. 워프 터널은 `Time<Real>`을 쓴다. 쇼룸 렌더에서 엔진 깜박임이 실행마다 약 0.16% 픽셀 다른 원인이 이것이다.

---

## 9. VFX

| 효과 | 기준 |
|---|---|
| 레이저 | 폭 1.8, 0.14 s, 명중 시 작은 폭발 0.15 |
| 포탑 펄스 | 폭 = 반경 × 0.012 (2–6), 0.12 s |
| 주포 빔 | 폭 = 반경 × 0.05 (5–26), 0.75 s. 포구·명중 폭발 |
| 빔 셰이더 | 흰 심 × 4 + 색 발광 × 2.5, 감쇠 pow 1.5 |
| 폭발 | 연기 → 적 → 주황 → 흰 램프. 6 → 36 × 크기 ease-out cubic, 1.6 s. 섬광 (1,.7,.4) 0.9 s. 파편 고리 |
| 실드 | 타원체 (1.15, .7, 1.35)·r. 육각 격자. 충격 물결 4 rad/s, e^(-3t) 감쇠. 25% 아래면 9 Hz 깜박임 |
| 워프 | 진입·이탈 구 (.5,.75,1) 6r, 0.9 s. 터널 (.55,.75,1)/(.35,.2,.9), 줄 96개 |
| 채굴 빔 | (.3,1,.5), 폭 2.5, 일정 |
| 잔해 | 천천히 회전, 불씨 (3,.7,.1) |

**새 VFX 프롬프트 템플릿:** `[마스터 블록] + effect: <무엇>, side colour from palette::side (<friend|hostile>), lifetime <s>, shape <beam|sphere|ring|streak>, white-hot core fading to the side colour, no colour outside the side ramp.`

---

## 10. UI와 HUD

- **글꼴:** egui 기본 + 한글 대체(맑은 고딕 → 나눔고딕 → Apple SD 고딕).
- **크기:** 제목 44(`ACCENT`), 성계명 22, 버튼 18, 라벨 11–12, 지도 격자 모노 10–11.
- **패널:**
  - 다크 테마. 창 채움 (12,16,26,235), 패널 채움 (12,16,26,220).
  - 테두리 1px (50,80,120), 선택색 (40,90,150).
- **조준선:**
  - 원 r14와 눈금 4개, 색 (170,220,255,160).
  - 게이지 호: r92, 폭 9, 범위 0.55π. 왼쪽이 실드 `SHIELD`, 오른쪽이 선체(초록 → 노랑 → 맥동하는 빨강).
- **3D 단서:**
  - 목표의 고도 차와 고도각(▲/▼).
  - 화면 밖에 있는 목표와 적에는 화면 가장자리 화살표.
  - 지도의 기준면 격자와 고도 기둥.
- **지도:** 배경 (4,7,13), 격자 (22,34,52), 격자 글자 (70,95,130).

---

## 11. 참고 이미지 (승인된 모습)

| 파일 | 장면 | 고정하는 것 |
|---|---|---|
| `docs/art/reference/fleet.png` | `--showroom 7` | 모든 선체·리버리, 사격, 실드 피격, 폭발, War Zone 청색 거성 하늘 |
| `docs/art/reference/core.png` | `--showroom 0` | Sol Haven: 황색 왜성, 안전 성계 팔레트, 정거장 |
| `docs/art/reference/gate.png` | `--showroom 9 --look gate` | 심우주: 중성자성 보라 하늘, 고대 게이트 |
| `docs/art/reference/ruin.png` | `--showroom 6 --look ruin` | Hollow Crown: 유적 발광, Wild 성운 |
| `docs/art/reference/derelict.png` | `--showroom 4 --look derelict` | Vesta Corridor: 백색 왜성, 버려진 호송선 |
| `docs/art/reference/human.png` | `--showroom 7 --species human` | Human 기준 선체 |
| `docs/art/reference/veyr.png` | `--showroom 7 --species veyr` | Veyr: 진주빛 청록, 보라 맥, 무지갯빛 림 |
| `docs/art/reference/kharz.png` | `--showroom 7 --species kharz` | Kharz: 그을린 쇠, 호박색 열 균열 |

- **콘셉트·모델 기준:** `tools/blender/previews/*.png`. 함선별 히어로 렌더와 시트, 대형 함선, 소품.
- **생성 프롬프트에 붙일 이미지:** 참고 이미지 중 가장 가까운 장면 하나, 그리고 같은 계열 함선의 `previews` 시트.

---

## 12. 일관성을 지키는 절차

1. **색은 `palette.rs`에서만 가져온다.** 새 색이 필요하면 먼저 3절 표에 의미를 정하고, 그다음 토큰을 추가한다. 코드에 `Color32::from_rgb(..)`나 `[r,g,b]` 리터럴로 관계·위험·희귀를 표현하지 않는다.
2. **새 에셋:**
   1. 마스터 블록과 카테고리 프롬프트로 콘셉트를 정한다.
   2. Blender 스타일(`STYLES`)과 재질 계약을 따른다.
   3. `client --showroom`으로 렌더한다.
   4. 참고 이미지 옆에 놓고 비교한다.
3. **룩이 바뀌는 변경:**
   1. `python tools/artcheck.py`를 실행한다. DRIFT가 나면 새 렌더를 이 문서와 대조해 검토한다.
   2. 의도한 변경이면 `python tools/artcheck.py --update`로 승인하고, 이 문서와 함께 커밋한다.
4. **리뷰 질문 다섯 개:**
   1. 빛나는 것이 모두 의미가 있는가? (P2)
   2. 아군·적이 색만 보고 구분되는가?
   3. 2 km 거리에서 실루엣으로 무엇인지 알 수 있는가? (P3)
   4. 높이와 깊이가 보이는가? (P4)
   5. 표면에 시간과 전투의 흔적이 있는가? (P5)

---

## 13. 알려진 불일치와 후속 과제

**이번에 고친 것** (2026-10-17):
- 위험도 색이 두 곳에 다른 값으로 있던 것을 `tier_color` 하나로 합쳤다.
- 희귀도 색을 `rarity_color` 하나로 합쳤다. 지도에서 전설이 보라로 나오던 것을 금색으로 바로잡았다.
- 실드 파랑 세 가지를 `SHIELD` 하나로 합쳤다.
- PvP 적대 함장이 아군 색(파랑)으로 보이던 것을 고쳤다. 발광, 엔진, 실드, 사격을 관계로 칠한다.

**남은 것:**
1. **종족 비주얼 2단계**(4절): Blender 종족 부착물과 종족별 리버리 기본값이 남았다. 리버리 판에서 Kharz 균열이 잔점처럼 보이는 곳이 있다(UV 이음새).
7. **레이 트레이싱과 커스텀 재질:** Solari는 `StandardMaterial`만 추적한다. 선체(`HullMat` 확장 재질), 행성, 항성, 플룸, 실드는 RT 장면에 들어가지 않는다.
2. **고대 게이트·유적 문양:** Blender 원본은 청록 (.25,.85,1)인데, 게임은 발광을 아군 색 × 4로 덮어써서 거의 흰색으로 날아간다(`gate.png`). 고대 문명 색을 `side`와 별도로 지켜야 한다.
3. **'적 빨강'의 색조 차이:** 해적 리버리 [.8,.12,.1]과 Blender 해적 발광 (1,.25,.12)이 3.4절 램프 밖에 따로 있다.
4. **선체 바 색:** 상태 바 (70,190,110)와 임계값 0.35가 조준선 게이지(`GREEN`, 0.6/0.3)와 다르다. 목표 바 (220,90,70)도 `RED`가 아니다.
5. **팔레트 밖 리터럴:** `interior.rs`의 GOLD·DIM 복사본, 지도 배경·격자 상수, HUD의 POI 하늘색.
6. **프레임 누적 애니메이션**(8절의 예외)이 남아 있다. 쇼룸 렌더가 완전히 결정적이지 않다(0.16%).
