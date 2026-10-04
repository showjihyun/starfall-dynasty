//! `sqlx::migrate!` 의 재컴파일 트리거 — `crates/persistence/build.rs` 와 같은 이유.
//!
//! server 실측(2026-09-18, persistence 에서 먼저 발견): sqlx 0.8.6 에서 **기존 마이그레이션
//! 파일을 수정하면** 재컴파일되지만 **새 파일을 추가하면 재컴파일되지 않는다** — 새
//! 마이그레이션이 테스트 바이너리에 들어가지 않은 채 낡은 산출물이 재사용된다.
//!
//! **이 크레이트에 이 파일이 없었던 것이 실제 결함이었다**(qa 진단, 2026-09-28): T0에서
//! `starfall-testdb`를 새로 만들 때 이 build.rs를 빠뜨렸다. 그 결과 `migrations/0002_*`가
//! 추가된 뒤에도 낡은 testdb 빌드 산출물(0001만 추적)이 `cargo test --workspace`에서
//! 재사용됐고, 그 산출물로 만든 테스트 DB에는 `inventory_items`·`deposit_states`·
//! `processed_commands`도 `domain_events_payload_is_object` CHECK도 없었다 — "CHECK
//! 제약이 우회됐다"로 보였던 증상의 정체다(server가 처음에 `cleanup_stale()`의
//! `WITH (FORCE)`를 오인 지목한 원인, `03_server_impl.md` S4 절 정정 참고).
fn main() {
    println!("cargo:rerun-if-changed=../../migrations");
}
