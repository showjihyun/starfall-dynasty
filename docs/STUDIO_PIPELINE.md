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
| CI | GitHub Actions(OSS 러너) | ○ 결정 필요 | 헤드리스 확인만 Linux 러너에서; 클라이언트는 빌드만 |
| 바이너리 에셋 버전 관리 | Git LFS | ○ 결정 필요 | 모델 재생성마다 20–36 MB가 쌓여 저장소 팩 785 MB |
| 에셋 예산·검증 | `tools/assetcheck.py` + `tools/asset_budgets.json` | ● 2026-10-20 | VRAM 예산(`texture_gpu_mb`)까지. 예산을 낮춰 가며 최적화 |
| 텍스처 최적화 | `crates/assetpipe`(intel_tex_2 = Intel ISPC BC7/BC5, zstd, KTX2) | ● 2026-10-20 | Blender 내보내기가 자동 실행. 아래 "텍스처 파이프라인" |
| 메시 최적화 | gltf-transform(OSS, `npx @gltf-transform/cli@4.1.1`), meshoptimizer | ○ | 탄젠트 미포함 — Bevy가 불러올 때마다 CPU로 계산(`validate`의 MESH_PRIMITIVE_GENERATED_TANGENT_SPACE). 내보낼 때 탄젠트를 넣고 불러오기 시간 측정 |
| 모델 제작 | Blender 5.2 + `tools/blender/*.py`(절차적 생성) | ● | 경로는 tools/blender/README.md(PATH에 없음) |
| 아트 회귀 | `tools/artcheck.py`, `tools/imgdiff.py` | ● | |
| 게임플레이·밸런스 | 시나리오 사양, `balance` 시트 | ● | |
| 자동 플레이 QA | `bot` qa(25항목), 클라이언트 `--script` | ● | |
| 재현(버그 리포트) | `server --record`, `replay`, `client --replay` | ● | 클라이언트 크래시 시 기록 폴더를 자동으로 남기기 |
| 성능 | `bench`, `client --bench`, `shared::prof` | ● | 성능 예산 게이트(이전 수치보다 느려지면 실패) |
| 오디오 | `client --dump-sfx`(절차적 효과음), 음악 에셋 | ◐ | 음량 정규화 확인(ffmpeg loudnorm) |
| 현지화 | — (한국어 문자열이 코드에 직접) | ○ | Project Fluent(`fluent-rs`)로 문자열 분리 — 해외 출시 결정 후 |
| 라이선스 준수 | — | ○ | `cargo-deny`(의존성 라이선스·취약점) |
| 배포·패키징 | — | ○ | `cargo-dist`/`cargo-packager`로 Windows 설치본 |
| 작업 관리 | GitHub(`gh`) | ◐ | 게이트 실패를 이슈로 남기기 |

● 있음 · ◐ 일부 · ○ 없음

## 에셋 예산 (`tools/asset_budgets.json`)

모델마다 파일 크기, 정점 수, 그리는 삼각형 수(인스턴스 포함), 재질 수, 가장 큰 텍스처 변, 텍스처 용량을 잰다. 첫 예산(2026-10-20)은 함급별 현재 최대 + 약 10% — 더 커지지 않게 막는 선이다. 측정(27개, 426 MB):

- 텍스처가 대부분: 주력함 텍스처 20–30 MB(4096 px), 소형함 7–10 MB(2048 px).
- 그리는 삼각형: 정거장 31만, 고대 관문 43만(같은 조각을 여러 번 인스턴스) — 성능 예산의 첫 후보.
- 예산을 올리는 건 그 모델과 같은 커밋에서, 이유와 함께만.

## 텍스처 파이프라인 (`cargo run -p assetpipe --release -- MODEL.glb [OUT.glb] [--max-px N]`)

Bevy는 PNG를 압축 없는 RGBA에 밉맵 없이 올린다. 4096 px 한 장이 VRAM 67 MB이고 멀리서 반짝인다. `assetpipe`는 GLB 안의 PNG를 GPU가 바로 읽는 KTX2로 바꾼다.

- 색·발광·금속-거칠기·AO는 BC7, 노멀은 BC5(Bevy가 Z를 복원). 밉맵 전체를 넣는다. 색은 선형 광에서 평균, 노멀은 다시 정규화. 레벨마다 zstd.
- Blender 내보내기(`shiplib.export_glb`)가 자동으로 실행한다. 이미 KTX2인 이미지는 건너뛴다(두 번 돌려도 그대로).
- 표준 glTF는 아니다(KHR_texture_basisu는 Basis 전용, Bevy도 그 확장을 읽지 않음). Bevy 런타임 에셋이고, 원본은 Blender 스크립트다. `gltf-transform validate`는 `image/ktx2` MIME 오류를 내는데, 예상된 것이다.
- 측정(2026-10-20, 27개): 텍스처 VRAM 2264 → 755 MB(1/3), 디스크 426 → 431 MB(순양함은 4096 px BC7이 PNG보다 커서 +3 MB씩, 나머지는 줄어듦). 디코드 4096 px 한 장 약 110 → 30 ms. artcheck 9장면 모두 OK(최대 0.66% 변화, 주력함 장면).
- 다음 지렛대: 순양함 4096 → 2048(`--max-px 2048`, VRAM 1/4·디스크 감소, 화면 확인 필요). 디스크를 크게 줄이려면 Basis UASTC/ETC1S(불러올 때 트랜스코딩, Bevy `basis-universal` 기능 필요).
