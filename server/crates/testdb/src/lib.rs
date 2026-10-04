//! 테스트 전용 격리 DB 헬퍼 (T0, `_workspace/p1-02-mining/02_server_ack.md` §3.2, Q-2).
//!
//! # 왜 이 크레이트가 있는가
//!
//! DB 통합 테스트(S4·H2)는 CI 에서 **반드시** 돌아야 하고, 동시에 증거 DB(`starfall` —
//! `domain_events` 는 슬라이스를 넘나드는 추가 전용 기록이다, CLAUDE.md)를 **절대**
//! 건드리면 안 된다. 이 둘을 동시에 만족하려고 `#[sqlx::test]`(DB 없으면 패닉 — "로컬
//! 무DB 건너뜀 + 건수 출력"을 만들 수 없다) 대신 이 크레이트를 쓴다.
//!
//! # 구조적 보호 (SC-114)
//!
//! 풀을 여는 함수는 [`open_pool`] **하나뿐**이고, DB 이름이 [`TEST_DB_PREFIX`] 로 시작하지
//! 않으면 **패닉**한다. 테스트 코드는 [`TestDb::create`] 가 돌려준 풀만 받는다 — 원본
//! `DATABASE_URL` 의 DB 이름(`starfall`)을 테스트에 넘기지 않는다.
//!
//! # 모드 (`STARFALL_DB_TESTS`)
//!
//! - `required`(CI): 접속·생성·마이그레이션 실패 → **패닉**(테스트 실패). 실행 수 0 도 CI
//!   게이트에서 FAIL 이다(그 대조는 qa 몫).
//! - 미설정(로컬): 실패 시 `None` 반환 + `STARFALL_DB_TEST SKIPPED <name>` 한 줄.
//! - 성공한 모든 테스트: `STARFALL_DB_TEST RAN <name>` 한 줄.
//!
//! `name` 은 **계약이 지명한 테스트 이름 문자열 그대로**다 — CI 게이트가 이 문자열로
//! RAN 목록을 대조한다(개수만 맞는 거짓 통과를 막는다).
//!
//! # 출력 경로 — `eprintln!` 이 아니라 `write_all`
//!
//! libtest 는 `print!`/`eprint!` **매크로**만 캡처해 통과한 테스트의 출력을 숨긴다. RAN 줄은
//! 통과한 테스트에서도 보여야 하므로(SC-113 선행 조건 — server 검토 §3.2-2) 매크로를 거치지
//! 않고 [`std::io::Stderr::write_all`] 로 직접 쓴다. **이 판단은 Phase 4 T0 의 첫 RED 에서
//! `--nocapture` 없이 실측해 확인한다** — 안 보이면 방식을 바꾼다(qa 통보 필요).

#![forbid(unsafe_code)]
// 이 크레이트는 테스트 하네스다(운영 요청 처리 경로가 아니다) — `rust-authoritative-server`
// §8 의 "요청 처리 경로에서 panic 금지"는 게임 서버 코드에 적용되는 규칙이고, 여기서
// `panic!` 은 **의도된 동작**이다: SC-114 구조적 보호(증거 DB 접속 시도를 테스트 실패로
// 만든다)와 `STARFALL_DB_TESTS=required` 의 "DB 없음 = 실패"가 전부 panic 으로 테스트를
// 실패시키는 것이 목적이다. `clippy.toml` 의 `allow-panic-in-tests` 는 `#[test]` 함수
// 안에서만 적용돼 이 크레이트의 라이브러리 코드는 덮지 못한다.
#![allow(clippy::panic)]

use std::io::Write as _;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use sqlx::Row;
use sqlx::postgres::{PgPool, PgPoolOptions};
use uuid::Uuid;

/// `starfall_test_` 로 시작하지 않는 DB 에는 절대 접속하지 않는다(SC-114).
pub const TEST_DB_PREFIX: &str = "starfall_test_";

/// 패닉으로 남은 테스트 DB 를 청소하는 기준 나이.
const STALE_AGE: Duration = Duration::from_secs(3600);

/// `sqlx::migrate!` 가 보는 것과 같은 디렉터리 — `CARGO_MANIFEST_DIR`(이 크레이트,
/// `crates/testdb`) 기준 상대 경로. 컴파일 시점 상수지만 **경로 문자열일 뿐**이다 —
/// 실제 파일 수는 [`count_migration_files`] 가 매번 실행 시점에 디스크를 읽어서 센다
/// (아래 [`assert_migration_count_matches_directory`] 문서 참고).
const MIGRATIONS_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../migrations");

/// 디렉터리 안의 `.sql` 파일 수를 **지금 이 순간** 센다(컴파일 시점에 바이너리에 박힌
/// 값이 아니다).
fn count_migration_files(dir: &Path) -> usize {
    std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "sql"))
                .count()
        })
        .unwrap_or(0)
}

/// 규칙 6 가드 — DB 에 실제 적용된 마이그레이션 수(`applied`)가 **실행 시점에
/// `migrations_dir` 를 읽어 센** `.sql` 파일 수와 같은지 단언한다.
///
/// **왜 "이 바이너리가 컴파일 시점에 아는 수"와 비교하면 안 되는가(2026-09-28 정정,
/// team-lead 지적)**: 첫 버전은 `sqlx::migrate!` 매크로가 만드는 `Migrator::migrations`
/// (컴파일 시점에 embed된 목록)의 길이와 비교했다. 낡은 빌드 산출물(마이그레이션
/// 파일이 추가된 뒤 재컴파일되지 않은 것 — 이 크레이트에 `build.rs` 가 없어서 실제로
/// 일어났던 결함)은 **자기가 아는 수(예: 1)를 실제로 적용한 수(1)와 비교해 1==1로
/// 자명 통과한다** — 정확히 잡아야 할 결함을 못 잡는 가드였다. 이 함수는 디스크를
/// 직접 읽으므로 바이너리가 낡았어도 실제 파일 수는 정확하다.
fn assert_migration_count_matches_directory(
    applied: i64,
    migrations_dir: &Path,
    test_name: &str,
    database_name: &str,
) {
    let on_disk = count_migration_files(migrations_dir);
    let on_disk_i64 = i64::try_from(on_disk).unwrap_or(i64::MAX);
    assert_eq!(
        applied,
        on_disk_i64,
        "테스트 DB 에 실제 적용된 마이그레이션 수({applied})가 지금 {} 에 있는 .sql \
         파일 수({on_disk})와 다르다 (test={test_name}, db={database_name}) — 낡은 testdb \
         빌드 산출물이 재사용됐거나(build.rs 재컴파일 트리거 참고) 마이그레이션이 \
         일부만 적용됐을 가능성(2026-09-28 결함, 정정판)",
        migrations_dir.display()
    );
}

/// 격리된 테스트 DB. 테스트가 끝나면 [`TestDb::drop`] 을 명시적으로 호출한다 — `Drop` 이
/// 아니라 비동기 정리라서다. 패닉으로 남은 DB 는 **다음 `create` 호출**이 청소한다.
#[derive(Debug)]
pub struct TestDb {
    /// 격리 DB 에 연결된 풀. 테스트는 이 풀만 쓴다.
    pub pool: PgPool,
    database_name: String,
    admin_url: String,
}

impl TestDb {
    /// 격리 DB 를 만들고 0001~0003 마이그레이션을 적용한다.
    ///
    /// `test_name` 은 스프린트 계약이 지명한 이름 그대로 넘긴다(RAN/SKIPPED 줄 대조용,
    /// server 검토 §3.2-6 — "이 헬퍼만 찍는다, 테스트 쪽에서 따로 찍지 않는다").
    ///
    /// # Panics
    ///
    /// `STARFALL_DB_TESTS=required` 인데 접속·생성·마이그레이션에 실패하면 패닉한다
    /// (CI 에서 DB 없음 = 실패).
    pub async fn create(test_name: &str) -> Option<Self> {
        let required = is_required_mode();
        let Ok(base_url) = std::env::var("DATABASE_URL") else {
            return skip_or_panic(required, test_name, "DATABASE_URL 미설정");
        };
        if base_url.is_empty() {
            return skip_or_panic(required, test_name, "DATABASE_URL 이 빈 문자열이다");
        }

        let admin_url = with_database(&base_url, "postgres");
        let admin_pool = match PgPoolOptions::new()
            .max_connections(2)
            .connect(&admin_url)
            .await
        {
            Ok(pool) => pool,
            Err(error) => {
                return skip_or_panic(required, test_name, &format!("관리 DB 접속 실패: {error}"));
            }
        };

        cleanup_stale(&admin_pool).await;

        let database_name = format!("{TEST_DB_PREFIX}{}", Uuid::now_v7().simple());
        if let Err(error) = sqlx::query(&format!("CREATE DATABASE \"{database_name}\""))
            .execute(&admin_pool)
            .await
        {
            return skip_or_panic(
                required,
                test_name,
                &format!("CREATE DATABASE 실패: {error}"),
            );
        }

        let test_db_url = with_database(&base_url, &database_name);
        let pool = match open_pool(&test_db_url, &database_name).await {
            Ok(pool) => pool,
            Err(error) => {
                return skip_or_panic(
                    required,
                    test_name,
                    &format!("테스트 DB 접속 실패: {error}"),
                );
            }
        };

        // 0001~0003 전부 — history(0003)도 같이 걸린다(server 검토 §3.2-1).
        let migrator = sqlx::migrate!("../../migrations");
        if let Err(error) = migrator.run(&pool).await {
            // 마이그레이션 실패는 required 여부와 무관하게 항상 치명적이다 — DB가
            // 있는데 스키마가 안 맞으면 "건너뜀"으로 넘어갈 이유가 없다.
            panic!("테스트 DB 마이그레이션 실패 (test={test_name}, db={database_name}): {error}");
        }

        // 규칙 6 가드(qa 진단, 2026-09-28 → team-lead 정정, 2026-09-28 둘째 라운드):
        // `build.rs`(재컴파일 트리거)가 없던 시절, 낡은 testdb 빌드 산출물이 새
        // 마이그레이션 파일을 못 보고도 조용히 "성공"을 보고했다. **처음 넣은 가드는
        // 틀렸다** — `migrator.migrations.len()`(이 바이너리가 컴파일 시점에 embed 한
        // 수)과 비교했는데, 바로 그 "컴파일 시점에 아는 수"가 낡았다는 것이 결함
        // 자체였다. 낡은 바이너리는 자기가 아는 수(예: 1)와 자기가 적용한 수(1)를
        // 비교해 **1==1로 자명 통과한다** — 아무것도 잡지 못했을 거라는 뜻이다.
        // **고친 가드**: `applied_migrations`(DB에 실제 적용된 행 수)를 **실행 시점에
        // `migrations/` 디렉터리를 직접 읽어 센 `.sql` 파일 수**와 비교한다
        // ([`assert_migration_count_matches_directory`]) — 컴파일된 바이너리가 아는
        // 값이 아니라 지금 이 순간 디스크에 있는 실제 파일 수가 기준이다. build.rs가
        // 다시 빠지거나 재컴파일이 안 일어난 경우에도 이 비교는 여전히 정확하다(디스크를
        // 직접 본다).
        let applied_migrations: i64 = sqlx::query_scalar("SELECT count(*) FROM _sqlx_migrations")
            .fetch_one(&pool)
            .await
            .unwrap_or(-1);
        assert_migration_count_matches_directory(
            applied_migrations,
            Path::new(MIGRATIONS_DIR),
            test_name,
            &database_name,
        );

        write_line(&format!("STARFALL_DB_TEST RAN {test_name}"));

        Some(Self {
            pool,
            database_name,
            admin_url,
        })
    }

    /// 격리 DB 를 지운다. 테스트 끝에서 호출한다.
    pub async fn drop(self) {
        self.pool.close().await;
        if let Ok(admin_pool) = PgPoolOptions::new()
            .max_connections(1)
            .connect(&self.admin_url)
            .await
        {
            let _ = sqlx::query(&format!(
                "DROP DATABASE IF EXISTS \"{}\" WITH (FORCE)",
                self.database_name
            ))
            .execute(&admin_pool)
            .await;
        }
    }

    /// 이 격리 DB 의 이름(`starfall_test_<uuidv7>`). 진단용.
    #[must_use]
    pub fn database_name(&self) -> &str {
        &self.database_name
    }
}

fn is_required_mode() -> bool {
    std::env::var("STARFALL_DB_TESTS").as_deref() == Ok("required")
}

/// `required` 면 패닉, 아니면 `SKIPPED` 한 줄을 찍고 `None`.
fn skip_or_panic(required: bool, test_name: &str, reason: &str) -> Option<TestDb> {
    if required {
        panic!("STARFALL_DB_TESTS=required 인데 실패했다 (test={test_name}): {reason}");
    }
    write_line(&format!(
        "STARFALL_DB_TEST SKIPPED {test_name} reason={reason}"
    ));
    None
}

/// 이 함수가 **유일한** 풀 개설 지점이다. `starfall_test_` 접두사가 아니면 패닉한다
/// (SC-114 — 증거 DB 를 구조적으로 못 건드린다).
async fn open_pool(database_url: &str, database_name: &str) -> Result<PgPool, sqlx::Error> {
    require_test_prefix(database_name);
    PgPoolOptions::new()
        .max_connections(4)
        .connect(database_url)
        .await
}

fn require_test_prefix(database_name: &str) {
    assert!(
        database_name.starts_with(TEST_DB_PREFIX),
        "거부: '{database_name}' 은 '{TEST_DB_PREFIX}' 로 시작하지 않는다 — 증거 DB 를 \
         테스트가 건드릴 뻔했다 (SC-114)"
    );
}

/// `postgres://user:pass@host:port/<dbname>` 의 dbname 부분만 바꾼다.
fn with_database(url: &str, database: &str) -> String {
    match url.rfind('/') {
        Some(index) => format!("{}/{database}", &url[..index]),
        None => format!("{url}/{database}"),
    }
}

/// [`STALE_AGE`] 보다 오래된 `starfall_test_%` DB 를 지운다(패닉으로 남은 것 청소). 이름의
/// UUIDv7 타임스탬프로 나이를 재므로, 동시에 도는 다른 테스트 실행의 DB(방금 생성 —
/// 타임스탬프가 최근)는 지우지 않는다.
async fn cleanup_stale(admin_pool: &PgPool) {
    let Ok(rows) = sqlx::query(
        "SELECT datname FROM pg_database WHERE datname LIKE 'starfall\\_test\\_%' ESCAPE '\\'",
    )
    .fetch_all(admin_pool)
    .await
    else {
        return;
    };

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    for row in rows {
        let Ok(name) = row.try_get::<String, _>("datname") else {
            continue;
        };
        let Some(suffix) = name.strip_prefix(TEST_DB_PREFIX) else {
            continue;
        };
        let Ok(uuid) = Uuid::parse_str(suffix) else {
            continue;
        };
        let Some(created) = uuidv7_unix_millis(&uuid) else {
            continue;
        };
        let is_stale = now
            .checked_sub(Duration::from_millis(created))
            .is_some_and(|age| age > STALE_AGE);
        if is_stale {
            // **정정(qa 진단, 2026-09-28)**: 아래 `WITH (FORCE)` 미사용 판단의 원래 근거
            // ("동시 실행의 살아 있는 DB 를 FORCE 로 끊어서 플레이키가 났다")는 **틀렸다**
            // — `cleanup_stale` 은 1시간(`STALE_AGE`) 넘은 DB 만 대상으로 하므로 방금
            // 만든 살아 있는 DB 는 애초에 대상이 아니었다(코드로 확인, qa). 실제 원인은
            // 이 크레이트에 `build.rs`(마이그레이션 디렉터리 재컴파일 트리거)가 없어
            // 낡은 testdb 빌드 산출물이 0002 를 못 보고 재사용된 것이었다 — 지금은
            // `build.rs` + 아래 `TestDb::create` 의 적용 마이그레이션 수 단언으로 막는다.
            // `FORCE` 를 계속 빼 두는 것 자체는 무해하므로(연결이 남아 있으면 그냥
            // 실패하고 넘어갈 뿐이다) 되돌리지 않았다 — 원래 근거만 정정한다.
            //
            // (원문, 참고용): 동시에 도는 다른 테스트 실행의 DB 를 `FORCE` 로 강제
            // 종료하면 그 실행이 이상 동작한다고 봤었다. `TestDb::drop`(자신이 방금까지
            // 독점 사용한 DB)에서는 여전히 `FORCE` 를 쓴다 — 거기는 "동시 사용자"가
            // 없다는 것이 알려진 사실이라 이 정정과 무관하다.
            let _ = sqlx::query(&format!("DROP DATABASE IF EXISTS \"{name}\""))
                .execute(admin_pool)
                .await;
        }
    }
}

/// UUIDv7 의 상위 48비트 = 밀리초 단위 유닉스 타임스탬프.
fn uuidv7_unix_millis(uuid: &Uuid) -> Option<u64> {
    if uuid.get_version_num() != 7 {
        return None;
    }
    let bytes = uuid.as_bytes();
    Some(
        (u64::from(bytes[0]) << 40)
            | (u64::from(bytes[1]) << 32)
            | (u64::from(bytes[2]) << 24)
            | (u64::from(bytes[3]) << 16)
            | (u64::from(bytes[4]) << 8)
            | u64::from(bytes[5]),
    )
}

/// `eprintln!` 이 아니라 직접 `write_all` — 모듈 문서 "출력 경로" 참고.
fn write_line(line: &str) {
    let mut buf = String::with_capacity(line.len() + 1);
    buf.push_str(line);
    buf.push('\n');
    let _ = std::io::stderr().write_all(buf.as_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    /// SC-114 — 증거 DB(또는 그 밖의 비-`starfall_test_` 이름)를 향한 풀 개설은
    /// 구조적으로 패닉한다. 실제 접속을 시도하지 않고도(네트워크 없이) 잡히는지가
    /// 핵심이라 `#[should_panic]` 으로만 확인한다 — `require_test_prefix` 는 접속보다
    /// **먼저** 실행된다.
    #[test]
    #[should_panic(expected = "SC-114")]
    fn refuses_non_test_database() {
        require_test_prefix("starfall");
    }

    #[test]
    fn accepts_test_prefixed_database() {
        require_test_prefix("starfall_test_0123456789abcdef0123456789abcdef");
    }

    #[test]
    fn with_database_replaces_only_the_database_segment() {
        assert_eq!(
            with_database("postgres://u:p@127.0.0.1:15432/starfall", "postgres"),
            "postgres://u:p@127.0.0.1:15432/postgres"
        );
    }

    // `STARFALL_DB_TESTS=required` 의 패닉 경로(DATABASE_URL 없음 → panic)는 여기서
    // 프로세스 전역 `std::env` 를 건드려 테스트하지 않는다 — `cargo test` 는 같은 바이너리의
    // 테스트를 병렬로 돌리므로, 이 테스트가 세운 env var 가 동시에 실행 중인 DB 통합
    // 테스트를 오염시킬 수 있다(그 자체가 SC-114 정신 위반이다 — 격리를 재는 테스트가
    // 격리를 깬다). CI 게이트(`STARFALL_DB_TESTS=required cargo test --workspace`,
    // `02_server_ack.md` §3.2-3)가 이 경로를 **프로세스 수준**에서 실행한다: DB 서비스가
    // 없으면 그 실행 전체가 실패하는 것이 이 모드의 실제 증거다.

    // -------------------------------------------------------------------
    // 규칙 6 가드 — 마이그레이션 수 대조(2026-09-28 정정판, team-lead 요청)
    // -------------------------------------------------------------------

    /// `migrations_dir` 아래 `.sql` 파일을 `count`개 심은 임시 디렉터리를 만든다.
    /// DB 접속 없이 [`assert_migration_count_matches_directory`]만 검사하는 용도 —
    /// 이 함수 자체는 디스크만 본다(`Path` 인자로 주입 가능하게 뺀 이유).
    struct TempMigrationsDir {
        root: std::path::PathBuf,
    }

    impl TempMigrationsDir {
        fn with_sql_files(label: &str, count: usize) -> Self {
            let root = std::env::temp_dir().join(format!(
                "starfall-testdb-migrations-probe-{label}-{}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&root);
            std::fs::create_dir_all(&root).unwrap();
            for n in 0..count {
                std::fs::write(root.join(format!("{n:04}_probe.sql")), "-- probe\n").unwrap();
            }
            // 방해 요소(.sql 아닌 파일)를 하나 섞는다 — 확장자 필터가 실제로 거른다는
            // 것을 같은 실행에서 보인다(규칙 9의 음성 대조와 같은 정신).
            std::fs::write(root.join("README.md"), "not a migration\n").unwrap();
            Self { root }
        }
    }

    impl Drop for TempMigrationsDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn count_migration_files_ignores_non_sql_files() {
        let dir = TempMigrationsDir::with_sql_files("count", 3);
        assert_eq!(
            count_migration_files(&dir.root),
            3,
            "README.md 는 세지 않는다"
        );
    }

    /// 양성 대조 — 디렉터리 파일 수와 `applied`가 같으면 통과한다(가드가 항상 실패하는
    /// 것이 아님을 먼저 보인다).
    #[test]
    fn migration_count_guard_passes_when_counts_match() {
        let dir = TempMigrationsDir::with_sql_files("match", 3);
        assert_migration_count_matches_directory(3, &dir.root, "probe_match", "db_match");
    }

    /// 결함 재현(team-lead 지적) — **컴파일된 바이너리가 아는 수가 아니라 디스크의
    /// 실제 파일 수와 비교**하므로, `applied`(DB가 실제로 적용한 수, 여기선 낡은
    /// 실행을 흉내 내 3으로 고정)가 지금 디렉터리에 있는 실제 `.sql` 수(4)보다
    /// 적으면 — 즉 "새 마이그레이션 파일이 추가됐는데 그걸 모르고 실행됐다"는 상황을
    /// 흉내 내면 — 가드가 **반드시 패닉한다**. 첫 버전(컴파일 시점 수와 비교)이었다면
    /// 이 시나리오를 못 잡았을 것이다(그 바이너리도 "3"만 알았을 것이므로 3==3으로
    /// 자명 통과).
    #[test]
    #[should_panic(expected = "다르다")]
    fn migration_count_guard_fails_when_directory_has_a_file_the_applied_count_never_saw() {
        let dir = TempMigrationsDir::with_sql_files("mismatch", 4);
        assert_migration_count_matches_directory(3, &dir.root, "probe_mismatch", "db_mismatch");
    }
}
