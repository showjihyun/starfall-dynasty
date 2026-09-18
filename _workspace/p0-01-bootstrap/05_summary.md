# p0-01-bootstrap 요약

- 완료일: 2026-09-18
- 실행 모드: 서브 에이전트 대체 모드 (이 세션에 에이전트 팀 도구 없음)
- 투입: architect, server, client, qa
- 판정: **스프린트 계약 38항목 중 판정 대상 37항목 전부 PASS, FAIL 0, 미검증(환경) 0, 기록 1**
- 증거: `04_qa_report_r1.md` (QA가 전 항목 직접 실행)

## 만든 것

| 영역 | 산출물 |
|------|-------|
| 결정 기록 | `docs/adr/0001` 레포·모듈 구조, `0002` 계약 형식·코드 생성, `0003` 로컬 개발 환경, `0004` 저장소·원격·라이선스 |
| 스펙 | `docs/specs/p0-01-bootstrap.md` (수용 기준 11개, 구현 중 실측으로 2회 정정) |
| 계약 | `contracts/` — 레지스트리, envelope 3종, `PING_SERVER`/`PING_REPLY`, 유효 fixture 4건 + 반례 7건 (JSON 19개) |
| 서버 | `server/` Cargo 워크스페이스 (`starfall-contracts`, `starfall-gateway`, `starfall-game-server`), `/healthz`·`/readyz`, 계약 테스트 9종, Rust 13파일 |
| 인프라 | `docker-compose.yml` (PostgreSQL 18 / Redis, 127.0.0.1의 15432·16379), `.env.example`, `rust-toolchain.toml` |
| 클라이언트 | `client/` Unity 6000.6.1f1 URP 프로젝트, asmdef 2종, 계약 DTO, EditMode 테스트 18건 |
| 도구 | `tools/codegen/` C# DTO 생성기 (결정적 출력, `--check`, 범위 기반 정수 매핑) |
| 저장소 | git init, `origin` 연결, `main` 통일, 독점 라이선스, `.gitignore`, `.gitattributes`(LFS·eol=lf) |

## 검증된 것 (증거 있음)

- 서버 게이트 3종 exit 0, 테스트 32건 통과
- 계약 검증 2층 분리: 스키마 거부 7건, **Rust serde 거부 7건**, 필수 필드 변이 28건 전부 실패 확인
- C# 측 유효 fixture 4건 왕복, 반례 5건 거부, EditMode 18건 통과 (콜드 임포트 63초, 에러 0)
- `/readyz`가 PostgreSQL 중단 시 503·프로세스 생존, 재시작 없이 1초 만에 복구
- 컨테이너 재생성 후 데이터 유지(마커 테이블 잔존), 추가된 볼륨은 `starfall_postgres-data` 하나뿐
- QA 독립 교차 검증: 제3자 Python 검증기로 fixture·반례·**C# 재직렬화 출력**까지 스키마 통과 확인

## 기록으로 남긴 설계 비대칭

`command-id-not-v7`, `tick-above-safe-integer` 두 반례는 C# Strict 설정에서 수락된다. 스키마 검증과 Rust serde가 서버 경계에서 막으므로 실패가 아니라 의도된 비대칭이며, 스펙 §5 표와 정확히 일치한다.

## 슬라이스 종료 시점에 처리한 것 (리더)

- **Git LFS 훅 오설치 수정** — 훅 4개가 `dev/null/`에 생성되어 `.git/hooks`가 비어 있었다. `dev/` 삭제 후 `git lfs install --local --force`로 재설치, `pre-push` 훅 확인. 이대로 두면 바이너리 에셋이 원격에 LFS 객체로 올라가지 않을 수 있었다.
- `.env`를 `.env.example`에서 생성 (무시 대상, 스펙의 Given과 실제 조건 일치시킴)

## 다음 슬라이스로 넘기는 것

| 항목 | 내용 | 담당 |
|------|------|------|
| p0-02 네트워킹 스파이크 | Unity↔Rust WebSocket, 명령→이벤트 왕복, 30명 동시 접속 + 실시간 이벤트 기록(= Phase 0 종료 기준) | server, client |
| `/readyz` 첫 호출 503 | 컨테이너 재생성 직후 Redis 점검이 broken pipe를 1회 노출(자동 복구). 재시도 정책을 p0-02에서 결정. CI·스모크는 폴링으로 | server |
| CI (GitHub Actions) | ADR-0003에서 다음 슬라이스로 미룬 항목 | server |
| Historical Event 스키마 | p0 범위지만 이번 슬라이스 밖 | architect, history |
| Q6~Q10 | LTS 이전 시점, 라이선스 법률 검토, 공개 불리 데이터 분리, Defender 예외, `unityyamlmerge` | 조건 충족 시 |

## 커밋 상태

커밋 0건 추가 (HEAD = 원격 초기 커밋 `2d9cf08`). `LICENSE` 수정과 모든 신규 파일이 미커밋 상태다. 첫 커밋·푸시는 사용자 결정 사항.
