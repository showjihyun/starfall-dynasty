# 스튜디오 파이프라인 (AI 개발을 3D 스튜디오 규모로)

3D 게임 스튜디오가 쓰는 기능별 도구와 이 저장소의 현재 상태를 한 표로 둔다. 도구는 OSS를 먼저 쓰고, 맞는 게 없으면 `tools/`에 작게 만든다. 원칙은 `docs/AI_게임_개발_지침.md`와 같다: 사람 손 없이 숫자로 판정하고, 한 라운드에 한 목표.

## 한 명령: `python tools/gate.py`

바뀐 파일을 보고 CLAUDE.md가 요구하는 확인을 골라 돌린 뒤 한 화면으로 요약한다(`target/gate/last.json`도 남김). 커밋 전에 사람이든 AI든 이것 하나를 돌린다.

| 바뀐 곳 | 도는 확인 |
|---|---|
| `shared`·`history`·`sim`·`view`·`tuning/` | 로직 테스트(시나리오 전체·결정성 포함) |
| `shared`·`sim`·`tuning/` | `docs/BALANCE.md`가 최신인지(다시 뽑아 비교) |
| `client`·`view`·`shared` | 클라이언트 ECS 헤드리스 테스트 |
| `server`·`bot`·프로토콜·게임 규칙 | 서버 빌드(실행 중이면 종료) + 봇 테스트 |
| Rust 전체 | clippy — 경고 자리 수가 기준(43)보다 늘면 실패 |
| 모델·예산·Blender 스크립트 | 에셋 예산(`tools/assetcheck.py`) |
| 렌더링 | artcheck — 창이 뜨므로 `--visual`일 때만, 아니면 TODO로 표시 |

`--all`(전부), `--base REF`(그 커밋 이후 변경), `--list`(무엇이 돌지만). 전체 실행 약 4분(대부분 봇 테스트).

## 기능별 현황

| 스튜디오 기능 | 도구 | 상태 | 다음 |
|---|---|---|---|
| 빌드 검증(그린 빌드) | `tools/gate.py` | ● 2026-10-20 | CI에서 같은 명령 실행 |
| CI | GitHub Actions `.github/workflows/gate.yml` | ● 2026-10-20 | 푸시마다 Ubuntu에서 `gate.py --all`(artcheck 제외, 첫 실행 약 21분), 보고서 `last.json`을 아티팩트로. 로컬 재현은 아래 "CI를 로컬에서" |
| 바이너리 에셋 버전 관리 | Git LFS | ○ 결정 필요 | 모델 재생성마다 20–36 MB가 쌓여 저장소 팩 785 MB |
| 에셋 예산·검증 | `tools/assetcheck.py` + `tools/asset_budgets.json` | ● 2026-10-20 | VRAM 예산(`texture_gpu_mb`)까지. 예산을 낮춰 가며 최적화 |
| 텍스처 최적화 | `crates/assetpipe`(intel_tex_2 = Intel ISPC BC7/BC5, zstd, KTX2) | ● 2026-10-20 | Blender 내보내기가 자동 실행. 아래 "텍스처 파이프라인" |
| 탄젠트 | `crates/assetpipe`(`bevy_mikktspace` — Bevy와 같은 코드) | ● 2026-10-20 | 불러오기 CPU 약 1.4초 절약(27개 합, 정거장 269 ms), 화면 픽셀 동일 |
| 메시 LOD | `assetpipe --lod`(meshoptimizer, `meshopt` 크레이트) + `client/src/lod.rs`(Bevy `VisibilityRange`) | ● 2026-10-20 | 정거장·관문만(예산 규칙의 `"lod"`). 함선 LOD는 측정 결과 역효과(아래 "메시 LOD") — 1000척 병목은 CPU 엔티티 비용 |
| 모델 제작 | Blender 5.2 + `tools/blender/*.py`(절차적 생성) | ● | 경로는 tools/blender/README.md(PATH에 없음) |
| 아트 회귀 | `tools/artcheck.py`, `tools/imgdiff.py` | ● | |
| 게임플레이·밸런스 | 시나리오 사양, `balance` 시트 | ● | |
| 자동 플레이 QA | `bot` qa(25항목), 클라이언트 `--script` | ● | |
| 재현(버그 리포트) | `server --record`, `replay`, `client --replay` | ● | 클라이언트 크래시 시 기록 폴더를 자동으로 남기기 |
| 프로파일러 | Tracy 0.14.1(BSD) — `python tools/tracy.py [-- 클라이언트 인자]` | ● 2026-10-22 | `client --features profile`(Bevy `trace_tracy`)로 엔진·게임 시스템 전부를 기록, UI 없이 캡처·CSV로 프레임당 비용 표. 도구는 target/tools에 받아 SHA-256 확인. 흔적은 target/tracy/run.tracy(UI로 열 수 있음) |
| 성능 | `bench`, `client --bench`, `shared::prof`, `tools/perfcheck.py` | ● 2026-10-20 | 기계별 기준선(`tools/perf_baseline.json`) 대비 sim 20%·클라이언트 25% 넘게 느려지면 실패. CI는 기준선이 없어 SKIP |
| 오디오 | `client --dump-sfx`(절차적 효과음), 음악 에셋 | ◐ | 음량 정규화 확인(ffmpeg loudnorm) |
| 스토리·연출 | docs/STORY.md, `view::cinematic`/`comms` | ● 2026-10-21 | 오프닝 컷신·캠페인 진입·장 끝 통신창(C1–C4) |
| 음성(대사) | `tools/voice.py` + Docker `tools/voice/`(Chatterbox Multilingual V3, MIT + ffmpeg) | ● 2026-10-22 | 대사 30줄 140초 1.0 MB, 화자 9 프리셋(`assets/voice/voices.json`: 기준 목소리 mid/low/high/builtin·표현력·속도·무전/컴퓨터/잔향). 2026-10-21 MeloTTS판은 "기계가 읽는 느낌"(리드) → 교체: Whisper 오인식률 0.109 → 0.067, 억양 폭 3.5 → 6.1 반음. 기준 목소리는 우리 MeloTTS 출력이라 실제 사람 목소리 복제 없음. 자막을 고치면 테스트가 "not voiced yet"으로 알리고 도구를 다시 돌리면 그 줄만 생성. 2026-10-22 자동 재녹음: Whisper(faster-whisper small, MIT)가 매 테이크를 받아써 오인식률이 `voices.json` check.max_cer(0.12)를 넘으면 다음 시드로 최대 4번, 가장 잘 들린 테이크를 남김(매니페스트에 take·cer·heard). 30줄 평균 0.067 → 0.034, 3줄이 재녹음 채택(고르마 "와라"가 "알아"로 들리던 줄 포함), 남은 2줄은 경계값 0.125(사엘 "너머"↔"넘어"는 철자만 다름) |
| 현지화 | — (한국어 문자열이 코드에 직접) | ○ | Project Fluent(`fluent-rs`)로 문자열 분리 — 해외 출시 결정 후 |
| 라이선스·취약점 | `cargo-deny`(`deny.toml`) | ● 2026-10-20 | 허용 라이선스 목록, RustSec, 출처. 예외 하나(RUSTSEC-2026-0344, 이유 기록). 게임 자체 라이선스는 미정 |
| 배포·패키징 | — | ○ | `cargo-dist`/`cargo-packager`로 Windows 설치본 |
| 작업 관리 | GitHub(`gh`) | ◐ | 게이트 실패를 이슈로 남기기 |

● 있음 · ◐ 일부 · ○ 없음

## 에셋 예산 (`tools/asset_budgets.json`)

모델마다 파일 크기, 정점 수, 그리는 삼각형 수(인스턴스 포함), 재질 수, 가장 큰 텍스처 변, 텍스처 용량을 잰다. 첫 예산(2026-10-20)은 함급별 현재 최대 + 약 10% — 더 커지지 않게 막는 선이다. 측정(27개, 426 MB):

- 텍스처가 대부분: 주력함 텍스처 20–30 MB(4096 px), 소형함 7–10 MB(2048 px).
- 그리는 삼각형: 정거장 31만, 고대 관문 43만(같은 조각을 여러 번 인스턴스) — 성능 예산의 첫 후보.
- 예산을 올리는 건 그 모델과 같은 커밋에서, 이유와 함께만.

## 에셋 파이프라인 (`cargo run -p assetpipe --release -- MODEL.glb [OUT.glb] [--max-px N]`)

Bevy는 PNG를 압축 없는 RGBA에 밉맵 없이 올린다. 4096 px 한 장이 VRAM 67 MB이고 멀리서 반짝인다. `assetpipe`는 GLB 안의 PNG를 GPU가 바로 읽는 KTX2로 바꾼다.

- 색·발광·금속-거칠기·AO는 BC7, 노멀은 BC5(Bevy가 Z를 복원). 밉맵 전체를 넣는다. 색은 선형 광에서 평균, 노멀은 다시 정규화. 레벨마다 zstd.
- Blender 내보내기(`shiplib.export_glb`)가 자동으로 실행한다. 이미 KTX2인 이미지는 건너뛴다(두 번 돌려도 그대로).
- 표준 glTF는 아니다(KHR_texture_basisu는 Basis 전용, Bevy도 그 확장을 읽지 않음). Bevy 런타임 에셋이고, 원본은 Blender 스크립트다. `gltf-transform validate`는 `image/ktx2` MIME 오류를 내는데, 예상된 것이다.
- 측정(2026-10-20, 27개): 텍스처 VRAM 2264 → 755 MB(1/3), 디스크 426 → 431 MB(순양함은 4096 px BC7이 PNG보다 커서 +3 MB씩, 나머지는 줄어듦). 디코드 4096 px 한 장 약 110 → 30 ms. artcheck 9장면 모두 OK(최대 0.66% 변화, 주력함 장면).
- 탄젠트: Blender가 내보내지 않아 Bevy가 노멀맵 쓰는 프리미티브마다 불러올 때 MikkTSpace를 돌렸다. `assetpipe`가 같은 코드(`bevy_mikktspace`, `bevy_mesh`와 같은 부호 반전)로 한 번 계산해 TANGENT로 저장한다. 27개 합 약 1.4초(정거장 269 ms, 군벌 141 ms, 주력함 35–100 ms)를 불러오기에서 덜고, 디스크 +21 MB(정점당 16 B), 주력함 장면 픽셀 100% 동일.
- 다음 지렛대: 순양함 4096 → 2048(`--max-px 2048`, VRAM 1/4·디스크 감소, 화면 확인 필요). 디스크를 크게 줄이려면 Basis UASTC/ETC1S(불러올 때 트랜스코딩, Bevy `basis-universal` 기능 필요).

## CI를 로컬에서 (Docker Desktop)

CI가 Linux에서만 실패할 때, 푸시하지 않고 같은 환경(Rust 1.98.1, Debian)에서 게이트를 돌린다. Windows `target/`과 섞이지 않게 커밋된 HEAD를 컨테이너 안에 clone하고, 빌드는 이름 붙은 볼륨(`starfall-target`, `starfall-cargo`)에 둔다. 두 번째부터는 몇 분.

```
MSYS_NO_PATHCONV=1 docker run --rm -v "C:/WorkSpace/SpaceHistoric:/src:ro" -v starfall-cargo:/usr/local/cargo/registry -v starfall-target:/work/target rust:1.98.1-bookworm bash -c "apt-get update -qq && apt-get install -y -qq --no-install-recommends pkg-config libasound2-dev libudev-dev libwayland-dev libxkbcommon-dev fonts-nanum python3 >/dev/null; rustup component add clippy; git config --global --add safe.directory '*'; git clone -q /src /work/src && cd /work/src && ln -s /work/target target && CARGO_TERM_COLOR=always python3 tools/gate.py --all"
```

첫 CI(2026-10-20)에서 잡힌 Linux 전용 문제 셋: intel_tex_2의 C++ 객체가 `libstdc++` 링크를 요구(Windows는 자동), CI의 색 출력이 clippy 경고 수를 0으로 만듦(게이트가 색을 끔), 게이트가 `python`을 불렀는데 Debian은 `python3`뿐(이제 자기 인터프리터로 실행).

## 메시 LOD (`--lod RATIO`)

- 예산 규칙에 `"lod": 0.5`가 있는 모델(정거장·고대 관문)만 만든다. Blender 내보내기가 같은 규칙을 읽어 넘기고, assetcheck는 LOD가 빠진 모델을 실패시킨다(`far tris` 열 = 모든 LOD가 바뀐 뒤 그리는 삼각형).
- LOD는 같은 정점 위의 새 인덱스 버퍼라 탄젠트·재질·텍스처를 공유한다(디스크 +2 MB). 전환 거리 = 단순화 오차(m, meshoptimizer ErrorAbsolute) x 899(1080p, 세로 62°에서 1 px) + 경계 반지름. 정거장 고리 329 m, 본체 2.0 km, 관문 8–16 km.
- `STARFALL_LOD_SCALE=0 client --showroom 0 --shot X.png`로 LOD만 보고 imgdiff할 수 있다(정거장 장면 0.10% 차이).
- 측정(`client --bench 15`, 3회씩): 평균 7.5 vs 7.6 ms(차이 없음), 1% 저점 12–13 vs 14.5–15.5 ms. 벤치는 3000 메시 함대전이라 정거장이 병목이 아니다.
- **함선 LOD는 하지 않는다(2026-10-22 측정, 1000척 `client --bench 15`, 기준 평균 13.3 ms):**
  - 함선·NPC 20개 모델에 `--lod 0.25`: 평균 15.7–17.8 ms로 **느려짐**. 프리미티브마다 LOD 복사본이 생겨 메시 엔티티가 3.5만 → 6만.
  - 엔티티를 늘리지 않는 "작은 부품 숨김"(포탑·랭크 부품·종족 키트가 1080p에서 2 px 미만이면 그리지 않음, 함선당 11–37개): 평균 14.1–17.4 vs 대조 14.0–14.1 ms, **효과 없음** → 되돌림.
  - 해상도 640x360(픽셀 1/6)에서도 12.4–13.4 ms: 병목은 GPU(삼각형·픽셀)가 아니라 CPU 쪽 엔진의 엔티티당 비용(함선당 메시 약 34개, 빈 노드 포함 더 많음, 함선마다 고유 재질 인스턴스 → 배칭 불가 의심). 다음은 추측 대신 Tracy 프로파일로 확인.
  - 같은 조건 반복 사이 편차가 ±3 ms라, 판단은 여러 번의 최솟값(perfcheck 방식)으로.
- **Tracy로 찾은 원인과 수정(2026-10-22):** 렌더 스레드(14.7 ms/프레임)가 병목이고 그 안은 그리기 기록·제출·인스턴스 버퍼. 함선마다 선체 재질을 따로 복제(함선당 14–59개, 피해·피격 섬광·부스트를 재질에 담았기 때문)해서 배칭이 안 됐다. 선체 재질을 (모델 부품 재질, 도색, 종족)마다 하나로 공유하고, 함선별 값(피해·섬광·부스트·무늬 씨앗·발광색)은 저장 버퍼 한 개의 함선 슬롯에 두어 셰이더가 `MeshTag`로 읽게 했다(`ships::ShipStates`). 번갈아 4쌍 측정: 평균 최솟값 13.9 → 10.7 ms(−23%), 1% 저점 22.3 → 15.0 ms(−33%). artcheck 화면 동일.
  - **효과 재질도 공유(2026-10-22):** 엔진 분사·노즐 발광·실드도 함선마다 재질을 복제하던 것을 같은 슬롯 버퍼로 옮겼다(슬롯 = vec4 7개: 선체 상태·발광색·스로틀/부스트·분사 색·코어 색·실드 색/세기·피격 방향/경과). 분사는 크기 등급마다 하나(플룸 번호는 `MeshTag` 상위 8비트 → 깜빡임 위상), 노즐 발광은 등급마다 하나(새 `NozzleMat`), 실드는 전체에 하나. 적/아군 전환도 재질 교체 없이 슬롯의 색만 바꾼다. 모델이 늦게 로드돼도 상태가 슬롯에 이미 있어 `materials_added` 보정이 필요 없어졌다. 번갈아 4쌍(같은 기계, 이전 바이너리와 교대): 평균 10.47/10.12/9.34/10.06 → 8.97/9.34/8.64/8.88 ms(4쌍 모두 개선, 평균 −10%, 최솟값 9.34 → 8.64 −7.5%), 1% 저점 최솟값 12.72 → 11.64 ms(−8.5%). 캐피털 장면은 이전 바이너리와 픽셀 동일(참조 이미지와의 0.66% 차이는 이전부터).

## 성능 게이트 (`python tools/perfcheck.py [--client] [--update]`)

- 벤치를 3번 돌려 가장 좋은 값(다른 프로그램은 시간을 늘리기만 한다), 클라이언트는 첫 실행을 버린다(빌드 직후 셰이더 컴파일로 첫 실행만 17 ms대).
- 측정한 흔들림: sim 7% 이내, 클라이언트 평균 17%까지 → 허용치 sim 20%, 클라이언트 25%.
- 기준선은 하드웨어(OS·CPU·스레드) 키로 저장한다(공개 저장소라 호스트 이름은 넣지 않음). 빠르게 만든 커밋, 또는 이유 있게 느려진 커밋에서만 `--update`.
- 게이트: sim·규칙이 바뀌면 sim 벤치, `--visual`이면 클라이언트 벤치까지.
