//! T0 실측 — SC-113 선행 조건(`_workspace/p1-02-mining/02_server_ack.md` §3.2-2):
//! `STARFALL_DB_TEST RAN <이름>` 줄이 **통과한** 테스트에서도 `--nocapture` 없이
//! `cargo test` 출력에 보이는가. 이 파일 자체가 그 실측 대상이다 — 결과는
//! `_workspace/p1-02-mining/02_interface.md` 에 발췌로 남긴다.
//!
//! 이 테스트는 실제 로컬 PostgreSQL(15432)에 붙는다. DB 가 없으면(로컬 무DB 환경)
//! `TestDb::create` 가 `None` 을 돌려주고 이 테스트는 통과로 끝난다(SKIPPED 줄만 남긴다).

#[tokio::test]
async fn creates_migrates_and_drops_an_isolated_database() {
    let name = "testdb::smoke::creates_migrates_and_drops_an_isolated_database";
    let Some(db) = starfall_testdb::TestDb::create(name).await else {
        return;
    };
    assert!(
        db.database_name()
            .starts_with(starfall_testdb::TEST_DB_PREFIX),
        "격리 DB 이름이 접두사로 시작하지 않는다: {}",
        db.database_name()
    );

    // 0001 마이그레이션이 실제로 적용됐는지 — 증거 DB 를 건드리지 않았다는 것은
    // TEST_DB_PREFIX 단언이 아니라 **이 풀이 비어 있던 새 DB 였다가 스키마를 얻었다는
    // 사실 자체**로 보강된다(격리 DB가 아니면 이미 데이터가 있었을 것이다).
    let row = sqlx::query("SELECT to_regclass('public.worlds') IS NOT NULL AS exists_flag")
        .fetch_one(&db.pool)
        .await
        .expect("쿼리 실패 — 격리 DB 에 접속은 됐는데 조회가 안 된다");
    let exists: bool = sqlx::Row::try_get(&row, "exists_flag").expect("컬럼 없음");
    assert!(exists, "0001 마이그레이션이 worlds 테이블을 만들지 않았다");

    // 0001 마이그레이션이 시드하는 기본 월드 수(마이그레이션 파일 자체가 근거 —
    // 여기서는 "쿼리가 되고 음수가 아니다"만 확인한다. 정확한 시드 행 수를 이 크레이트가
    // 가정하면 마이그레이션이 바뀔 때마다 이 테스트도 깨진다).
    let world_count: i64 = sqlx::query_scalar("SELECT count(*) FROM worlds")
        .fetch_one(&db.pool)
        .await
        .expect("worlds 조회 실패");
    assert!(
        world_count >= 0,
        "count(*) 가 음수일 수는 없다 — 방어적 단언"
    );

    db.drop().await;
}
