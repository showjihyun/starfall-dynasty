//! STARFALL DYNASTY — QA 봇 하네스의 라이브러리 면.
//!
//! 바이너리(`src/main.rs`)와 **통합 테스트(`tests/`)가 같은 코드를 쓴다.** 계측기를 테스트할 수
//! 없으면 "봇이 손실 0을 봤다"는 말에 근거가 없다 — 그래서 로직은 전부 여기 있고 바이너리는
//! 인자 파싱과 출력만 한다.

pub mod conn;
pub mod extra_cases;
pub mod ledger;
pub mod mine_cases;
pub mod mine_load;
pub mod mine_run;
pub mod mining;
pub mod nav;
pub mod notice_gap;
pub mod range_turn;
pub mod report;
pub mod restart_cases;
pub mod scenario;
pub mod snapshot;
pub mod sql;
pub mod stats;
pub mod token;
pub mod trace;
pub mod wire;

/// `mine_run::find_deposit` 의 재수출 — 채굴 케이스가 쓰는 길 찾기 입력.
pub fn mine_cases_site(
    data_dir: &std::path::Path,
    deposit_id: &str,
) -> Result<mine_run::DepositSite, String> {
    mine_run::find_deposit(data_dir, deposit_id)
}
