# p0-01-bootstrap — 검토 반영 결정 기록

- 작성: architect, 2026-09-18
- 입력: `01_server_adr_review.md`, `01_client_adr_review.md`, `00_request.md`의 사용자 결정 절
- 결과: ADR-0001/0002/0003 갱신(accepted), ADR-0004 신설, 스펙·태스크 개정, `contracts/` 2건 추가 + 정수 `format` 제거, `LICENSE` 교체, `.gitattributes` 신설, `.gitignore` 보강

두 검토 모두 실행 증거를 붙여 왔고, 지적 중 **차단급 5건은 전부 타당했다**(cargo 작업 디렉토리 누락, AC-10 증명 불가, `overrideReferences` 누락, `unity test` 리포트 경로, `Required.AllowNull`). 아래 표의 판정은 리뷰어가 근거를 되짚을 수 있도록 "어디에 반영했는가"까지 적는다.

## 1. server 리뷰 처리

| # | 요청 | 판정 | 반영 위치 / 이유 |
|---|------|------|-----------------|
| R-1 | cargo 명령에 실행 위치(`cd server`), `--all`, `--locked` | 수용 | 스펙 §2·§7(AC-1·AC-2·AC-5), ADR-0003 §1, T1·T2 지시. 레포 루트에 `Cargo.toml`이 없다는 지적이 정확하다 |
| R-2 | AC-10 "정수 타입 3자 동일" 교체 | 수용(권장안 + client 제안 병합) | 스펙 AC-10. "범위를 손실 없이 표현 + 범위 밖 거부"로 교체. client의 5)와 같은 요구였다 |
| R-3 | `/readyz` 유지 + 구현 제약 4 + AC-3 복구 확인 | 수용(전부) | ADR-0003 §3.1 신설, 스펙 AC-3. "healthy는 컨테이너 안에서의 점검일 뿐"이라는 논거가 이 엔드포인트를 남길 이유를 스펙보다 잘 설명해서 ADR 본문에 그대로 반영했다 |
| R-4 | AC-4 증거를 마커 테이블 잔존 + 익명 볼륨 미생성으로 | 수용 | 스펙 AC-4. "AC-4가 잡으려는 실패를 AC-4가 항상 잡지 못한다"는 지적이 결정적이다 |
| R-4 말미 | 전역 prune 금지, "깨끗한 Docker" 범위 축소 | 수용 | 스펙 AC-4, ADR-0003 §3, T3·T10 지시 |
| R-5 | 계약 테스트 5종 → 9종 | 수용(전부) | ADR-0002 §3 표, 스펙 AC-5(a)~(g), T2 지시. 특히 T-6(serde 거부표)은 "운영 중 명령을 막는 것은 스키마 검증기가 아니라 serde"라는 이유가 옳다 — 스펙 §5에 Rust serde 열을 추가했다 |
| R-6 | ADR-0002 §3.5 태그를 `server`만으로 | 수용 | ADR-0002 §3("해당 크레이트가 존재하는 태그에 대해서만, p0-01에서는 `server`") |
| R-7 | AC-5(e) "producers 또는 consumers" | 수용 | 스펙 AC-5(e), ADR-0002 §3 테스트 5 |
| R-8 | `curl` → `curl.exe` | 수용 | 스펙 §7 머리말·AC-2, ADR-0003 §5, T10 지시(PowerShell `-SkipHttpErrorCheck` 포함) |
| R-9 | 상한 위반 반례 fixture 2건 | 수용 | `contracts/fixtures/PING_SERVER/invalid/probe-seq-above-u32.json`, `PING_REPLY/invalid/tick-above-safe-integer.json` 추가. 독립 검증기로 7건 전부 거부 확인. 스펙 §5 표·AC-5(c)를 7건으로 갱신 |
| R-10 | "재직렬화 = 원본"의 4가지 조건 명문화 | 수용 | ADR-0002 §3, T2 지시. `skip_serializing_if` 함정은 실제로 가장 먼저 밟을 함정이라 T2 상단에 올렸다 |
| R-11 | gateway 오염 방지 한 줄 | 수용 | ADR-0001 §2, T1 지시 |
| A-1 보완 | `[lib] name` 분리 금지 + `use starfall_sim as sim;` | 수용 | ADR-0001 §2 |
| A-3 | 툴체인 첫 다운로드 안내 | 수용(위치 조정) | ADR-0003 §1, 스펙 §2 주석·§8. §2 흐름 본문은 짧게 유지하고 주석으로 넣었다 |
| A-7 | `jsonschema` features 설명 정정 + `offline()` | 수용 | ADR-0002 §3. "기본에 들어 있어서"를 "리졸버 feature를 켜지 않아 실패로 드러난다"로 정정 |
| AC-11 관련 | `.sqlx/` 제외 금지 | 수용 | `.gitignore` 주석 + 스펙 AC-11(d)에 검증 항목으로 명시 |
| 질문 5 | Redis 크레이트 `redis` 1.7.0 + ConnectionManager | 수용 | ADR-0003 §3.1. `fred`/풀 크레이트를 버린 이유(원칙 3·7)가 타당하다 |
| — | sqlx 0.9.0 파괴적 변경 시 0.8.x 핀 | 수용(재량 위임) | ADR-0003 §3.1, T3 지시. 부트스트랩이 라이브러리 마이그레이션을 떠안을 이유가 없다는 데 동의 |
| — | 빌드 시간 2회 측정 | 수용 | 스펙 §8, T3·T4 지시 |

## 2. client 리뷰 처리

| # | 요청 | 판정 | 반영 위치 / 이유 |
|---|------|------|-----------------|
| A-1 | `overrideReferences: true` + 테스트 어셈블리 `nunit.framework.dll` | 수용 | ADR-0001 §3 표와 설명, T5 지시. "자동 참조로 우연히 동작하다가 나중에 깨진다"는 함정 설명이 정확하다 |
| A-2 | `noEngineReferences` 근거 문구 정정 | 수용 | ADR-0001 §3. 얻지 못하는 이득(Unity 없는 컴파일)을 근거로 쓰면 나중에 시간을 버린다는 지적이 맞다 |
| A-2 대안 | `tools/codegen/verify/` netstandard2.1 하네스 | 부분 수용 | T6에 **선택·권장**으로 기록. Safe Mode 예방 가치는 인정하지만 AC로 만들면 이번 슬라이스의 완료 조건이 늘어난다 |
| A-3 | IL2CPP 스트리핑 대비 기록 | 부분 수용(기록만) | ADR-0001 §3에 한 줄, 스펙 §3 제외 목록. 실제 플레이어 빌드가 생기는 슬라이스의 과제 |
| B-1 | 시리얼라이저 프로필 2종 | 부분 수용 | ADR-0002 §4 표에 두 프로필과 비대칭 원칙을 명문화. **코드는 이번 슬라이스에 `Strict`만**(수신 경로가 없다) — 요청자도 같은 범위를 제안했다 |
| C-1 | `required` × 널 가능 → `Required` 매핑 표 | 수용 | ADR-0002 §4. 유효 fixture가 거부되는 실행 증거가 있어 논쟁 여지가 없다 |
| C-2 | `RealTime`/`GameTime` → `string` 고정 | 수용 | ADR-0002 §4. 게임 달력이 실제 시간 축에 올라타는 것은 원칙 위반이 타입 하나로 들어오는 경로다 |
| C-3 | 키워드 allowlist 3분류 | 수용(4분류로 확장) | ADR-0002 §4. `minimum`/`maximum`은 이제 **정수 매핑 근거**라 "무시" 분류에서 분리했다 |
| C-4 | 정수 매핑을 범위 기반으로 | **수용** | ADR-0002 §4(+§1에서 정수 `format` 제거), T6 지시. 공짜로 얻는 방어를 버릴 이유가 없다. `probe_seq` → `uint`, C# 책임 반례 3건 → 5건 |
| C-5(a) | 생성 파일 `#nullable disable` | 수용 | ADR-0002 §4. `CS8618` 9건이라는 실측이 AC-8("경고 0")과 직접 충돌한다 |
| C-5(b) | `.gitattributes`에 `*.cs`/`*.json` `eol=lf` | 수용 | `.gitattributes` 신설(+Unity YAML·LFS 패턴), ADR-0004 §4 |
| D-1 | UUIDv7은 정규 문자열 → `new Guid(string)` | 수용 | ADR-0002 §5, T7 지시. "variant는 살아남고 version만 깨진다"는 재현 결과를 함정 설명에 포함 |
| D-1 추가 | 같은 밀리초 연속 생성 중복 없음 테스트, `RandomNumberGenerator` | 수용 | 스펙 AC-7(d), T7 지시 |
| D-2 | 헬퍼 소속 어셈블리 명시 | 수용 | ADR-0001 §3 표("`Generated/`와 손으로 쓴 시리얼라이저·ID 헬퍼가 함께 산다") |
| E-1 | T5의 "Hub 등록 포함" 삭제 | **철회(2026-09-18)** | T5 실측으로 뒤집혔다: `unity projects new`는 **Hub에 등록된다**(`DefaultCompany`/`client`). 도움말에 문구가 없다는 이유로 삭제한 내 판단이 틀렸다. T5 지시와 ADR-0003 §2를 확인된 사실로 되돌렸다 |
| E-2 | `unity test` 리포트 명령 교체 | 수용(문법 정정 2026-09-18) | 스펙 AC-7, T7 지시. 근거(형식 하나만 주면 `--junit-output` 무시)는 유효하나 **`--report-format both`는 이 CLI에서 유효하지 않다**(`Allowed values: nunit, junit`, 종료 코드 2). `nunit,junit` 쉼표 목록으로 정정 |
| E-3 | T5·T7의 Q1 의존 | 해소 | 사용자가 6000.6.1f1로 확정. 태스크 표의 선행 조건에서 제거 |
| E-4 | 템플릿 패키지 정리 범위 | **부분 수용(보수안보다 한 칸 더)** | T5 지시: `collab-proxy` **+ `visualscripting` 제거**, `timeline`·`ai.navigation`·`inputsystem` 유지. collab-proxy는 git 결정과 중복이고 visualscripting은 MVP에서 쓰지 않는 큰 서브시스템이다. 나머지는 이후 사용 가능성이 있어 제거 이득이 작다. 제거가 해석 오류를 내면 되돌리고 기록 |
| E-4 추가 | `TutorialInfo` 삭제, 식별자 사후 설정 | 수용 | T5 지시 |
| E-5 | fixture 경로를 마커 기반으로 + 0건 방지 | 수용(둘 다) | 스펙 AC-7(e), T7 지시. 대안(개수 가드만)이 아니라 둘 다 요구한다 — "조용히 0건 통과"는 I-4와 같은 성격의 실패다 |
| 1) | AC-6에 파일 집합 비교 추가 | 수용 | 스펙 AC-6, ADR-0002 §4(`--check`는 내용 + 파일 집합), T6 지시(`.meta` 미건드림 포함) |
| 2) | AC-7 (c) 문구 조정 | 수용 | 스펙 AC-7(c). 디스패치 헬퍼를 p0-02가 쓸 같은 함수로 만든다는 제안까지 T7에 반영 |
| 3) | AC-8을 콜드 임포트 + 종료 코드로 | 수용 | 스펙 AC-8. "클론할 대상이 없다"(I-8과 모순)는 지적이 정확하다. 경고 범위도 `Assets/_Project/**`로 한정 |
| 4) | AC-9 실행 순서 명시 | 수용 | 스펙 AC-9, T11 지시 |
| 6) | AC-11 보강 | 수용 | 스펙 AC-11(b)(c)(d). `.meta` 추적, `tools/**/obj/` 무시, `.sqlx/` 미무시까지 한 항목에 모았다 |
| 7) | §8 콜드/웜 구분 | 수용 | 스펙 §8 |
| F-1~F-3 | 계약에 대한 의견(변경 요청 아님) | 기록 | F-3(정수 `format` 모순)은 C-4·R-2와 함께 계약 변경으로 처리 |

## 3. architect가 스스로 내린 계약 변경

| 변경 | 이유 |
|------|------|
| 반례 fixture 2건 추가(`probe-seq-above-u32`, `tick-above-safe-integer`) | server R-9. 기존 반례 5건은 구조 위반만 봤고 **상한 위반이 하나도 없었다**. 상한은 `MoneyMinor`·`Tick`에서 화폐 버그의 입구다 |
| 정수 정의에서 `format` 제거(`SchemaVersion`, `SafeInteger`, `NonNegativeSafeInteger`, 두 payload의 `probe_seq`) | C-4를 수용해 정수 매핑 근거가 `minimum`/`maximum`으로 일원화되면 `format`은 어느 소비자도 쓰지 않는 **두 번째 진실**이 된다. 실제로 `probe_seq`(`0..4294967295`인데 `format: int64`)에서 두 검토자가 같은 모순을 지적했다. server R-2가 이 방향을 "덜 권함"으로 본 이유는 "생성기와 ADR을 동시에 건드린다"였는데, C-4 채택으로 생성기 매핑 규칙을 어차피 바꾸게 되어 그 부담이 사라졌다. 문자열의 `format: uuid`는 유지(생성기가 `Guid` 매핑에 쓴다) |

검증: 유효 fixture 4건은 그대로 통과하고 반례 7건은 전부 거부됨을 독립 검증기(Python `jsonschema`, 오프라인 레지스트리)로 재확인했다. 커버리지 스크립트도 오류 0으로 통과한다(유효 fixture 수는 타입당 2로 변동 없음).

## 4. 두 리뷰 사이의 충돌

없었다. 두 리뷰가 겹친 지점(AC-10 문구, `probe_seq` 타입 모순, R-10 (2)(3)의 왕복 함정이 C#에도 동일하게 적용된다는 점)은 결론이 같았고, 병합해 반영했다.

## 5. 리뷰어가 추가로 확인해 줄 것

- **server**: ADR-0002 §3의 9종 표가 T2 지시와 1:1로 맞는지. 스펙 §5의 "Rust serde" 열이 실제 구현 결과와 다르면 **표를 고치지 말고** architect에게 알릴 것(표는 기대값이 아니라 계약이다).
- **client**: 정수 범위 기반 매핑으로 `probe_seq`가 `uint`가 되면서 C# 책임 반례가 5건이 되었다(스펙 §5·AC-7b). `tick`은 여전히 `long`이라 상한 위반을 C#이 잡지 못한다 — 의도된 비대칭이다.
- **둘 다**: 미확인으로 남긴 항목(PG18 볼륨 실측, `unevaluatedProperties` 재현, Hub 등록 여부, 패키지 제거 영향)은 각자 T3·T2·T5에서 실측해 `03_*_impl.md`에 기록할 것. 결과가 ADR과 다르면 ADR을 갱신한다.

## 6. 구현 중 확인된 문서 오류 정정 (2026-09-18, client 실행 보고 반영)

| 오류 | 정정 | 반영 위치 |
|------|------|----------|
| `--report-format both` (유효하지 않음, 종료 코드 2) | `--report-format nunit,junit` (쉼표 목록) | 스펙 AC-7, T7 지시, 위 E-2 행 |
| Editor 로그 경로를 `%LOCALAPPDATA%\Unity\Editor\Editor.log`로 읽을 수 있다고 가정 | **`client/Logs/Editor.log`**(프로젝트 안, 실행마다 갱신, 컴파일 로그 포함) | 스펙 AC-8, T7 지시 |
| "`unity projects new`는 Hub에 등록하지 않는다(미확인)" | **등록한다**(`DefaultCompany`/`client`) | T5 지시, ADR-0003 §2, 위 E-1 행 |
| 패키지 버전을 템플릿 기재값으로 적음 | Hub가 레지스트리 최신으로 해석(`inputsystem` 1.20.0, `ai.navigation` 2.0.14). 정본은 `client/Packages/manifest.json` | ADR-0003 §2 표 |

추가 실측: 패키지 2종(`collab-proxy`, `visualscripting`) 제거의 해석 영향 **없음**. 콜드 임포트 63초 / 웜 11초(ADR-0003 §2에 기준값으로 기록).

`client/.vscode/` 처리(판단 요청): **무시한다**(`.gitignore`). Unity의 VS Code 연동이 만드는 것은 절대 경로·파일 제외 같은 머신별 설정이고, 지금 공유할 설정이 없다. 나중에 공유 설정이 필요하면 해당 파일만 무시 해제한다(`!.vscode/extensions.json`). 루트와 하위 모두에 적용되도록 `.vscode/` 패턴 하나로 두었다.
