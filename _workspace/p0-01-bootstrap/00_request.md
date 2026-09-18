# p0-01-bootstrap

## 요청 원문
Phase 0 부트스트랩 시작해줘

## 분류
- 유형: BOOT
- 로드맵 Phase: p0 (사전 제작)
- 범위 판단: 범위 안. 이번 슬라이스는 **개발 기반 확립**까지만 한다.
  - 포함: ADR-0001(레포·모듈 구조), ADR-0002(계약 형식·코드 생성), ADR-0003(로컬 개발 환경·툴체인 버전 고정), git 저장소 초기화와 .gitignore(커밋은 하지 않음), `contracts/` 골격(레지스트리, envelope 스키마, fixture), Rust 워크스페이스 골격(빌드·테스트 통과, 헬스 체크 수준), docker compose(PostgreSQL·Redis), Unity 6 프로젝트 골격(URP, asmdef 구조, 계약 fixture를 읽는 EditMode 테스트), 계약 커버리지 스크립트 통과
  - 제외(다음 슬라이스): Unity↔Rust WebSocket 네트워킹 스파이크, Historical Event 저장 파이프라인, 30명 동시 접속 검증, 렌더링·시뮬레이션 벤치마크, WebGL 가능성 테스트 → p0-02 이후

## 실행 모드
- 서브 에이전트 대체 모드. 이유: 이 세션의 Agent 도구에 `name` 파라미터가 없고 Task 도구가 없어 에이전트 팀 스폰 불가 (`CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1`은 설정됨).
- 팀원 간 메시지는 `_workspace/p0-01-bootstrap/` 파일과 리더 중계로 대체한다.

## 환경 (2026-09-17 리더 확인)
- 이 세션의 PATH에는 새로 설치한 도구가 없다. Bash 명령 앞에 다음을 붙이면 동작한다:
  `export PATH="$HOME/.cargo/bin:$HOME/.dotnet/tools:/c/Users/CHOISOOYEON/AppData/Local/Unity/bin:$PATH"`
- Rust stable 1.98.1 (msvc, clippy·rustfmt·rust-analyzer), .NET SDK 10.0.401, csharp-ls 0.28.0, Unity CLI 1.0.0-beta.8(로그인됨), 설치된 Unity Editor 6000.6.1f1(Documentation, Web Build Support 모듈), Docker 27.3.1 + Compose v2.29.7 실행 중, git 있음(저장소 아님)

## 사용자 결정 (2026-09-18, 리더가 질문 → 사용자 답변)
1. **Unity 에디터 버전: 6000.6.1f1 고정** (설치된 버전 사용). LTS가 아니므로 콘텐츠가 쌓이기 전에 LTS 이전을 재검토하는 항목을 ADR-0003에 남긴다.
2. **원격 저장소: `https://github.com/showjihyun/starfall-dynasty.git`** (사용자가 주소 제공). 리더가 `origin`으로 등록했고, 원격의 초기 커밋(LICENSE 1건) 위에 로컬 `main`을 올려두었다. **푸시는 이번 슬라이스에서 하지 않는다** — 사용자가 요청할 때만.
   - **공개 범위: Public 유지** (사용자 확인). 기획안·서버 로직·밸런스 수치가 공개된다는 점을 전제로 작업한다.
   - **브랜치: `main`으로 통일** — 원격 `master`를 `main`으로 이름 변경 완료, 로컬도 `main`.
   - **라이선스: AGPL-3.0 → 독점(proprietary)으로 교체** (사용자 결정). 공개 저장소 + 독점 라이선스 = 읽을 수는 있으나 사용·복제·2차 배포는 허가받아야 하는 형태. LICENSE 교체와 근거 기록은 architect 담당. 법률 자문이 아니므로 상업 출시 전 검토 필요를 ADR에 남긴다.
3. **Git LFS: 지금 설정.** `git-lfs 3.6.0` 설치 확인됨. `.gitattributes`에 Unity 바이너리 에셋(모델·텍스처·오디오·비디오 등) 패턴을 등록한다. 담당은 architect(.gitattributes는 루트 파일 소유 규칙에 따라 architect).
4. **Unity 프로젝트 이름: 회사명 `Starfall`, 제품명 `Starfall Dynasty`, 번들 ID `com.starfall.dynasty`.**
5. **첫 커밋: 이번 슬라이스에서는 커밋하지 않는다.** 사용자가 요청할 때만 커밋한다.

## 투입
- 팀원: architect, server, client, qa
- 제외한 레이어와 이유: designer — 게임 규칙 없음 / history — 역사 파이프라인은 다음 슬라이스 / techart — 렌더링 작업 없음(URP 기본 설정은 client가 프로젝트 생성 시 적용, 세부 렌더링 ADR은 이후)
