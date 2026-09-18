---
name: space-3d-rendering
description: "Unity 6 URP 3D 우주 게임 렌더링·테크아트 규약 (STARFALL DYNASTY). 대규모 좌표 처리(Floating Origin, 스케일 공간 + 카메라 스태킹, 은하 지도 분리), 스타필드·스카이박스, 행성·대기, 항성 조명, 소행성·함대 대량 렌더링(GPU 인스턴싱, GPU Resident Drawer), 모듈형 함선 조립과 전투 흔적(데칼, 마모), 워프·무기·폭발 VFX(Shader Graph, VFX Graph, Full Screen Pass), 3D 전투 가독성, 프레임 예산·품질 단계, WebGL 대체 경로를 다룬다. 3D 그래픽, 셰이더, 이펙트, 카메라 떨림·좌표 정밀도 문제, 렌더링 성능, 아트 파이프라인 작업 시 반드시 사용."
---

# Space 3D Rendering — 넓은 우주를 정밀도·프레임·가독성 문제 없이 그리는 규약

이 게임의 비주얼 목표는 초고급 그래픽이 아니라 **많은 함선, 넓은 공간, 많은 정보, 안정적인 프레임**이다 (기획안 TECH §6). 판단이 애매하면 가독성과 프레임을 먼저 택한다.

Unity 공식 플러그인 스킬을 함께 쓴다: `/unity:urp-postprocessing`, `/unity:shader-graph-create-custom-node`, `/unity:validate-urp-render-graph-renderer-feature`, `/unity:migrate-birp-to-urp`, `/unity:optimize-web`, `/unity:unity-cli`.

## 1. 작업 순서 원칙

1. **그레이박스 먼저.** 버티컬 슬라이스의 질문은 "재미있는가"다. 단순 도형과 명확한 색 구분으로 채굴·전투·역사 흔적이 읽히는지부터 확인한다.
2. **측정 후 최적화.** 성능 주장에는 Profiler / Frame Debugger / Rendering Debugger 수치를 붙인다.
3. **PC 기준, WebGL 대체 경로 병기.** 새 효과나 렌더링 기능마다 "WebGL에서는?"을 한 줄로 적는다.

## 2. URP 기본 설정 (ADR로 확정)

- Rendering Path: **Forward+** (다수 광원, GPU Resident Drawer 전제 조건)
- SRP Batcher 켬. GPU Resident Drawer 켬(지원 플랫폼). 둘 다 켠 상태에서 배칭이 깨지는 원인을 만들지 않는다 — 대표적으로 `MaterialPropertyBlock`은 SRP Batcher와 호환되지 않는다. 소수의 개별 표현(플레이어 함선 마모)은 머티리얼 인스턴스, 대량 오브젝트는 공유 머티리얼 + 인스턴스 속성으로 처리한다.
- HDR 켬 (엔진·항성·폭발 블룸). 톤매핑과 블룸 강도는 전투 가독성을 해치지 않는 선에서.
- 품질 단계별 URP 에셋 분리: `URP_PC_High`, `URP_PC_Low`, `URP_Web`. 효과 on/off를 코드 분기가 아닌 품질 단계로 관리한다.
- 안티앨리어싱은 측정 후 결정한다. 작은 별·얇은 레이저가 많아 TAA 번짐이 문제될 수 있다.

## 3. 대규모 좌표 — 가장 먼저 설계할 것

Unity Transform은 float32다. 원점에서 수 km만 멀어져도 정점 떨림·카메라 흔들림·물리 오차가 보이기 시작한다. 성계 규모 거리를 그대로 넣으면 반드시 깨진다. 서버는 성계 로컬 좌표를 `f64`로 가지고, 클라이언트는 아래 3계층으로 나눠 그린다.

| 계층 | 담는 것 | 좌표 | 카메라 |
|------|--------|------|-------|
| **Local** | 내 함선 주변 수십 km: 함선, 투사체, 채굴 대상 소행성, 잔해 | 실제 미터, Floating Origin | Overlay 카메라 (near 0.1~1m) |
| **Scaled** | 행성, 위성, 항성, 먼 정거장 | 축소 좌표 (예: 1:10,000 — ADR로 결정) | Base 카메라 (스카이박스 포함) |
| **Galaxy Map** | 성계·항로·영토 | 추상 좌표(광년 단위) | 별도 씬/뷰, UI 중심 |

### Floating Origin
- 카메라(또는 내 함선)가 원점에서 임계 거리(예: 2~5 km)를 넘으면 원점을 옮긴다: `originOffset += shift`, Local 계층 루트 오브젝트를 `-shift`만큼 이동.
- 변환은 한 곳에서만: `unityPos = (Vector3)(serverPosF64 - originOffsetF64)`. 서버 좌표를 float로 저장하지 않는다.
- 원점 이동 시 월드 공간에 위치를 기억하는 것들을 함께 옮긴다: World 시뮬레이션 공간 파티클, TrailRenderer/LineRenderer 점, 보간 버퍼의 과거 위치, 캐시된 목표 지점. `IOriginShiftListener` 같은 인터페이스로 등록받아 누락을 막는다. **이 누락이 원점 이동 직후 이펙트가 튀는 가장 흔한 원인이다.**
- `FloatingOrigin` 컴포넌트는 techart가 만들고 공개 API를 제공한다. 서버 좌표 → Unity 좌표 변환과 보간은 client의 World 계층이 그 API를 사용한다.

### Scaled Space + 카메라 스태킹
- Base 카메라는 Scaled 계층을 그리고, 회전은 메인 카메라와 같게, 위치는 `메인 카메라의 f64 위치 / scale`로 맞춘다.
- Overlay 카메라는 Local 계층을 그리고 depth를 비운다. near/far를 계층별로 나누면 z-fighting이 사라진다.
- 행성에 접근해 Local로 넘어와야 하는 경우(정거장 도킹 등)는 해당 Phase 스펙에서 전환 규칙을 따로 정한다. MVP(1 성계·1 행성)에는 행성 착륙이 없다.

## 4. 배경과 천체

- **스타필드:** 큐브맵 스카이박스(원경) + 소수의 가까운 별 레이어(GPU 인스턴싱 쿼드, 약한 시차). 별 하나당 GameObject를 만들지 않는다.
- **성운:** 스카이박스 레이어나 원거리 반투명 메시. 전투 가독성을 위해 대비를 낮게 유지한다.
- **행성:** 큐브 구체 메시 + LOD, Shader Graph 표면(노이즈/트라이플래너). 대기는 약간 큰 셸 메시의 프레넬 림(additive).
- **항성 조명:** 성계의 항성 = Directional Light 1개(방향 = 항성 → 카메라 영역). 우주답게 앰비언트는 낮게, 그림자는 가까운 함선에만(짧은 shadow distance, 적은 cascade).
- 엔진·창문·무기는 emissive + 블룸으로 광원 수를 늘리지 않고 밝기를 표현한다.

## 5. 대량 오브젝트

- 소행성대·잔해·원거리 함대는 개별 GameObject 대신 `Graphics.RenderMeshInstanced`/`RenderMeshIndirect` 또는 GPU Resident Drawer에 맡긴다.
- **상호작용 대상만 GameObject**: 채굴 가능한 가까운 소행성, 조준 가능한 함선. 나머지는 인스턴싱 비주얼. 거리 기준으로 승격·강등한다.
- Entities Graphics(ECS)는 위 방법으로 예산을 못 맞춘다는 측정이 있을 때 ADR로 도입한다.
- 투사체·폭발은 풀링한다. 발사마다 `Instantiate`/`Destroy`하지 않는다.

## 6. 함선 비주얼 — "각 함선은 영구적인 역사를 가진다"

- **모듈 조립:** 선체 프리팹에 `Socket_{ModuleType}_{index}` Transform을 두고, 서버가 보낸 장착 정보로 모듈 프리팹을 붙인다. 모듈 종류는 계약의 모듈 타입과 1:1로 맞춘다.
- **스케일 기준:** Scout S-01 같은 1인 함선의 크기를 먼저 정하고(designer와 합의) 모든 모듈·정거장·소행성 크기를 그 기준으로 맞춘다. 스케일이 흔들리면 카메라·속도감·조준이 전부 어색해진다.
- **역사의 흔적:** 서버의 함선 역사(전투 횟수, 대수리 이력, 마지막 피격 방향)를 마모·그을음·패치 강도 파라미터로 매핑한다. 매핑 표를 `03_techart_impl.md`에 남긴다. 필요한 데이터가 계약에 없으면 architect에게 요청한다.
- **데칼:** URP Decal Renderer Feature + Decal Projector로 탄흔·그을음. 함선당 개수 상한을 둔다.
- **파괴:** 서버의 `SHIP_DESTROYED` 이벤트로만 재생. 미리 쪼갠 잔해 프리팹을 풀에서 꺼낸다. 잔해 일부는 "사건 현장"으로 남아 다른 플레이어가 조사할 수 있어야 하므로(역사 조사 플레이), 표시 수명은 스펙을 따른다.

## 7. VFX

### WebGL 호환 기준
- **VFX Graph는 compute shader가 필요해 WebGL(WebGL 2)에서 동작하지 않는다.** 핵심 효과(워프, 무기, 폭발, 채굴 빔)는 Shader Graph + Particle System으로 만들고, VFX Graph는 PC 품질 단계의 추가 연출로만 쓴다.
- 풀스크린 효과(워프 왜곡, 피격 비네트)는 URP Full Screen Pass Renderer Feature 또는 Render Graph 호환 커스텀 Renderer Feature로 만든다. 커스텀 기능은 `/unity:validate-urp-render-graph-renderer-feature`로 점검한다.

### 워프 연출 (기획안 GDD §9)
`중력장 형성 → 별빛 왜곡 → 엔진 압축 → 공간 균열 → 워프 터널 → 목적지 성계 출현`
- 단계 타이밍은 서버 이벤트(워프 시작·완료)로 구동한다. 성계 전환 로딩 시간이 가변이므로 **터널 단계는 루프 가능**하게 만든다.
- 별빛 왜곡은 스타필드 셰이더의 방향성 늘이기 + 방사형 블러 풀스크린 패스.
- 터널 진입 시 Floating Origin과 Scaled 계층을 목적지 기준으로 재설정하고, 출현 플래시 뒤에 드러낸다.

### 전투 가독성
- 아군/적/중립/위험 지역을 색으로 구분하되 색각 이상 사용자를 위해 모양·아이콘을 함께 쓴다.
- 조준 표시, 리드 인디케이터, 스캔 하이라이트(아웃라인 Renderer Feature)는 효과보다 우선순위가 높다. 폭발·블룸이 표시를 가리지 않게 정렬 순서를 정한다.

## 8. 성능 예산 (초기값 — 측정으로 조정)

| 항목 | PC 기준 (1080p, 중급 GPU) | Web |
|------|-------------------------|-----|
| 목표 프레임 | 60 fps (프레임 16.6ms) | 30 fps 이상 |
| 스트레스 씬 | 소행성 200(인스턴싱), 함선 30, 투사체 500, 폭발 동시 10 | 소행성 100, 함선 15, 투사체 200 |
| 필수 기록 | CPU 메인·렌더 스레드 ms, GPU ms, SetPass·배치 수, GC 할당/프레임 | 동일 + 힙 메모리, 초기 다운로드 크기 |

- 스트레스 씬 `Scenes/Benchmarks/CombatStress`를 만들고, 변경 전후를 같은 씬·같은 카메라 경로로 비교한다.
- 수치는 `_workspace/{slice-id}/03_techart_impl.md`에 표로 남긴다. qa가 이 표를 평가 증거로 쓴다.

## 9. 에셋 파이프라인

- 폴더: `Art/{Ships,Modules,Asteroids,Planets,Stations}`, `Shaders/`, `VFX/`, `Rendering/`(URP 에셋, Volume 프로필, Renderer Feature).
- 임포트 설정은 프리셋으로 통일하고, 함선·정거장에는 LODGroup을 둔다.
- Addressables 그룹은 성계 단위로 나눠 워프 시 로드·언로드한다.
- Editor가 켜져 있으면 머티리얼·프리팹·볼륨 설정을 `unity command`/Unity MCP로 조작하고, 가능하면 결과 스크린샷을 `_workspace/{slice-id}/screenshots/`에 남긴다.
