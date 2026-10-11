# art/ — p1-04 비주얼 기준선 스테이징

Unity(`client/`) 밖에서 만든 3D 에셋. Unity 임포트는 p1-03 r1 종료 뒤 Phase 3에서 한다(`_workspace/p1-04-visual-baseline/01_art_direction.md` §5·§6).
모든 메시·텍스처는 `scripts/`의 Blender 스크립트가 만든다. 외부 에셋은 CC0 텍스처 4종뿐이다(`LICENSES.md`).

## 구조

| 경로 | 내용 |
|---|---|
| `scripts/common.py` | 공용: 씬·Cycles 설정, 재질(CC0 텍스처 기반 도장·금속), 메시 도우미, FBX 내보내기(+ 형상 다이제스트) |
| `scripts/skybox.py`, `skybox_post.py` | 별·성운 equirect 스카이박스 (v2: 사용자 요청으로 성운 강화, 2026-10-08) |
| `scripts/ship_scout.py` | Scout S-01 선체 + 변형 3종(Standard/Prospector/Courier), LOD 3단, 소켓, 도색 6종 |
| `scripts/asteroids.py` | 광맥 바위 4종(고해상 조각 → 저폴리 + 노멀 베이크), 광물 결정 4종 + 고갈 그루터기 |
| `scripts/station.py` | Cradle Exchange 정거장(정적부 + 회전 거주 링), LOD 3단, 도킹 소켓 |
| `scripts/planet_star.py` | 행성 Vela 텍스처(알베도·노멀·거칠기·구름), 항성 Cradle 표면·코로나 |
| `scripts/scene_mining.py` | 채굴 빔 VFX 텍스처 + 실제 스케일 합성 미리보기 |
| `scripts/contact_sheet.py` | 미리보기 타일링(시스템 Python + Pillow) |
| `source/*.blend` | 스크립트가 저장한 원본(압축) |
| `export/*.fbx` | Unity 임포트용 |
| `textures/` | `skybox/` `planet/` `star/` `asteroids/` `vfx/` 생성물, `polyhaven/` CC0 원본 |
| `previews/*.png` | 품질 판단용 Cycles 렌더 |

## v3 (2026-10-09, LoGH풍 디자인 언어 — 현재 방향)

| 스크립트 | 산출 |
|---|---|
| `scripts/logh_ships.py` | `export/v3/ship_lancet_imperial.fbx`, `ship_cutter_alliance.fbx`, `bg_bs_imperial.fbx`, `bg_bs_alliance.fbx`, `previews/v3_lancet_*`, `v3_cutter_*`, `v3_faction_compare.png`, `v3_bg_bs_*` |
| `scripts/station_v3.py` | `export/v3/station_cradle_exchange_v3.fbx`, `previews/v3_station_*` (`logh_ships.py`의 재질·도우미를 import) |
| `scripts/planet_star.py` | Vela 고리 `textures/planet/vela_ring.png` 추가 |
| `scripts/scene_hero_v3.py` | `previews/v3_hero.png`, `v3_hero_fleet.png` (위 .blend 전부 필요, 연출 컷) |

디자인 언어만 참조한 오리지널 디자인이다. 근거는 `01_art_direction.md` §11.1.
v1·v2 산출(`export/*.fbx`)은 대체됐지만 지우지 않았다.

## 재생성

```
export B="/c/Program Files/Blender Foundation/Blender 5.2/blender.exe"
"$B" --background --factory-startup --python art/scripts/skybox.py        # 약 3분 (8K 성운·별 원본 -> textures/_tmp)
python art/scripts/skybox_post.py                                         # 밝은 별 스파이크·글로우, 8K + 4K 최종본
SKY_STAGE=previews "$B" --background --factory-startup --python art/scripts/skybox.py   # skybox_view_0..3
"$B" --background --factory-startup --python art/scripts/ship_scout.py    # 약 2분
"$B" --background --factory-startup --python art/scripts/asteroids.py     # 약 1.5분 (노멀 베이크 4장)
"$B" --background --factory-startup --python art/scripts/station.py       # ship_scout.blend 필요(미리보기용)
"$B" --background --factory-startup --python art/scripts/planet_star.py
"$B" --background --factory-startup --python art/scripts/scene_mining.py  # 위 .blend 전부 필요
python art/scripts/contact_sheet.py art/previews/ship_scout_liveries.png 3 amber=art/textures/_tmp/livery_amber.png ...
```

- 순서: skybox → ship_scout → asteroids → station → planet_star → scene_mining (뒤 스크립트가 앞의 `.blend`·스카이박스를 읽는다).
- 결정성: 난수는 모두 시드 고정 `random.Random(SEED)`, 노이즈는 고정 오프셋. FBX 파일은 내보낸 시각이 들어가 바이트가 매번 다르므로, 내보낼 때 로그에 찍히는 `geo <digest>`(정점 좌표 mm 단위 정렬 해시)로 같은 형상인지 비교한다.
  - 2026-10-08 실측: ship_scout·asteroids·station을 각각 두 번 돌려 FBX 8개의 `geo` 다이제스트가 모두 같았다.
  - 2026-10-09: 텍스처는 비트 단위로 같지 않다. Cycles GPU 베이크·렌더에서 최대 1~2/255 차이가 나는 픽셀이 수십~수백 개 생긴다. 측정 표는 `01_art_direction.md` §10.5. 형상(FBX)은 결정적이다.
  - Blender Decimate(collapse)는 낮은 비율(약 0.08)에서 실행마다 결과가 달랐다(같은 프로세스 설정, `-t 1`에서도). 그래서 함선 LOD2와 바위 LOD는 Decimate 대신 손으로 만든 저폴리·거친 구 재평가로 바꿨다. 함선 LOD1(0.3)과 정거장 LOD(0.25, 0.06)는 반복 실행에서 같게 나왔다.
- `textures/_tmp/`는 시트용 중간 렌더라 추적하지 않는다(`.gitignore`).

## 좌표·스케일 규약

- 1 Blender unit = 1 m. 함선 기수 = Blender −Y, 위 = +Z. FBX는 forward −Z / up Y / `bake_space_transform`으로 내보낸다 → Unity에서 기수 +Z, 위 +Y가 되어야 한다. **Unity 임포트로 아직 확인하지 않았다(Phase 3 첫 작업).**
- 데이터 반경이 바운딩 반경의 상한이다(스크립트가 assert): Scout ≤ 12 m(`hull_radius_m`), 정거장 ≤ 120 m(`radius_m`), 광맥 바위는 단위 반경(본체 0.78 + 위성 ≤ 1.0) → Unity에서 `radius_m`으로 균일 스케일.
- 정거장 위 = Unity +Y. 거주 링(`Station_Ring_*`)은 따로 회전시킨다.

## 산출 요약 (2026-10-08 생성)

| 에셋 | 파일 | LOD 삼각형 | 비고 |
|---|---|---|---|
| Scout S-01 ×3 | `export/ship_scout_{standard,prospector,courier}.fbx` | 34~36k / 10~11k / 3.1~3.8k | 바운딩 11.3 m, 소켓 `Socket_*`, 도색 = `Livery` 재질 교체. LOD2는 손으로 만든 저폴리 선체(패널 틈·그리블 없음) |
| 광맥 바위 ×4 | `export/asteroid_rock_{0..3}.fbx` | 5.1k / 1.3k | 같은 조각 함수를 거친 구에 다시 평가(LOD0에만 노멀 2048² 베이크, `textures/asteroids/`), `Socket_Crystal_0..5` |
| 광물 결정 ×4 | `export/crystal_{ferrosite,glacine,cobaltine,starfall-glass}.fbx` | 0.2k~2k | 꼭짓점 색 `Emit` = 발광 마스크, `_stump` = 고갈 |
| 정거장 | `export/station_cradle_exchange.fbx` | 정적 73k / 18k / 4k, 링 48k / 12k / 3k | 바운딩 116.7 m, `Socket_Dock_0..3`(도킹 요람 28×21 m), `Socket_Beacon_*` |
| 행성 Vela | `textures/planet/vela_*.png` | (Unity 구) | 4096×2048 equirect |
| 항성 Cradle | `textures/star/cradle_surface.png`, `cradle_corona.png` | (Unity 구 + 빌보드) | |
| 스카이박스 | `textures/skybox/cradle_sky_equirect.png` (8K, 30.9 MB), `_4k.png` (Web) | — | Latitude-Longitude 큐브맵, 값 상한 0.97 |
| 채굴 빔 | `textures/vfx/mining_beam_gradient.png`, `mining_impact.png` | — | LineRenderer + 가산 스프라이트 |
