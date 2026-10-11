# art/ 외부 에셋 라이선스 기록

규칙: **CC0만** 쓴다. 받기 전에 라이선스 페이지에서 CC0인지 확인하고, 에셋마다 아래 표에 한 줄을 남긴다. 유료·AI 생성 에셋은 쓰지 않는다.
스크립트로 만든 메시·텍스처(`art/scripts/*.py`의 산출물)는 이 프로젝트의 원작이다.

| 에셋 | 출처 URL | 라이선스 (확인한 페이지) | 받은 날짜 | 받은 파일 (`art/textures/polyhaven/`) | 쓴 곳 |
|---|---|---|---|---|---|
| Dark Rock 02 | https://polyhaven.com/a/dark_rock_02 | CC0 — https://polyhaven.com/license ("Our assets are all licensed as CC0") | 2026-10-07 | `dark_rock_02_{diff,nor_gl,rough,ao,disp}_2k.jpg` | 광맥 소행성 바위 재질 |
| Rock Boulder Dry | https://polyhaven.com/a/rock_boulder_dry | CC0 — 같은 페이지 | 2026-10-07 | `rock_boulder_dry_{diff,nor_gl,rough,ao,disp}_2k.jpg` | 장식 바위·위성 파편 재질 |
| Blue Metal Plate | https://polyhaven.com/a/blue_metal_plate | CC0 — 같은 페이지 | 2026-10-07 | `blue_metal_plate_{diff,nor_gl,rough,ao}_2k.jpg` | 함선·정거장 도장 긁힘(거칠기·노멀 디테일) |
| Metal Plate 02 | https://polyhaven.com/a/metal_plate_02 | CC0 — 같은 페이지 | 2026-10-07 | `metal_plate_02_{diff,nor_gl,rough,ao}_2k.jpg` | 함선·정거장 흑연 프레임 재질 |

받는 방법(재현): `https://api.polyhaven.com/files/{id}` 응답의 `{Diffuse,nor_gl,Rough,AO,Displacement}.2k.jpg.url`.
