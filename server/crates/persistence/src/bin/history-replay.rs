//! 읽기 전용 재생 도구 (SC-59, AC-12(b), H-05·H-06).
//!
//! 지정한 월드의 `domain_events` 를 **빈 판정 상태**에서 처음부터 판정해, 저장된
//! `historical_events`·`evidence` 와 내용(`recorded_at` 제외)·id 까지 같은지 비교한다.
//! **아무 것도 쓰지 않는다** — 이 도구가 붙는 DB 의 역사 테이블을 비우지 않는다
//! (CLAUDE.md 금지 사항, 스펙 §10-10 — 재구축 테스트는 복사한 로그로 별도 DB 에서
//! 한다는 전제와 같다).
//!
//! ```text
//! cd server && cargo run -p starfall-persistence --bin history-replay -- \
//!     --world <world-id> --read-only
//! ```
//!
//! `--read-only` 는 **필수 플래그**다 — 이 도구에는 쓰기 경로가 아예 없지만, 나중에
//! 다른 모드가 추가되더라도 실수로 이 이름을 잘못 쓰는 사고를 막기 위해 명시적으로
//! 요구한다.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use starfall_contracts::data::SignificanceRuleTable;
use starfall_contracts::primitives::UuidV7;
use starfall_persistence::history::replay_and_compare;

fn main() -> ExitCode {
    let Ok(runtime) = tokio::runtime::Runtime::new() else {
        eprintln!("tokio 런타임 생성 실패");
        return ExitCode::FAILURE;
    };
    runtime.block_on(run())
}

async fn run() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some((world_id, read_only)) = parse_args(&args) else {
        eprintln!("사용법: history-replay --world <world-id> --read-only");
        return ExitCode::FAILURE;
    };
    if !read_only {
        eprintln!("--read-only 가 필요하다 — 이 도구는 쓰기 모드가 없다");
        return ExitCode::FAILURE;
    }

    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("DATABASE_URL 이 설정돼 있지 않다");
        return ExitCode::FAILURE;
    };
    let pool = match starfall_persistence::history::read_only_pool(&database_url) {
        Ok(pool) => pool,
        Err(error) => {
            eprintln!("풀 생성 실패: {error}");
            return ExitCode::FAILURE;
        }
    };

    let rule = match load_rule() {
        Ok(rule) => rule,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };

    let report = match replay_and_compare(&pool, world_id, rule).await {
        Ok(report) => report,
        Err(error) => {
            eprintln!("재생 실패: {error}");
            return ExitCode::FAILURE;
        }
    };

    // ⊘(SC-59): 재생 대상이 0건이면 비교할 것이 없다 — 항상 두 수를 찍어서 그런
    // 실행이 조용히 통과하지 않게 한다.
    println!(
        "재생한 domain_events = {}, 재생한 역사 기록 = {}",
        report.domain_events_replayed, report.records_replayed
    );
    if report.records_replayed == 0 {
        eprintln!(
            "경고: 재생한 역사 기록이 0건이다 — 이 실행은 회귀를 아무 것도 못 잡는다 \
             (무효, §0.6 배제 조건과 같은 이유)"
        );
    }

    if report.is_match() {
        println!("PASS — 재생 결과가 저장된 기록·증거와 id 까지 같다");
        ExitCode::SUCCESS
    } else {
        eprintln!("FAIL — {}건 불일치", report.mismatches.len());
        for mismatch in &report.mismatches {
            eprintln!("  - {mismatch}");
        }
        ExitCode::FAILURE
    }
}

fn parse_args(args: &[String]) -> Option<(UuidV7, bool)> {
    let mut world_id = None;
    let mut read_only = false;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--world" => world_id = iter.next().and_then(|value| UuidV7::parse(value)),
            "--read-only" => read_only = true,
            _ => {}
        }
    }
    Some((world_id?, read_only))
}

/// `data/history/rules/mineral-discovery.json` 을 찾는다. `bins/game-server/src/data.rs`
/// 의 `STARFALL_DATA_DIR` 해석(env 우선, 없으면 `data` → `../data`)과 같은 규칙이다 —
/// 그 모듈은 이 바이너리(별개 크레이트)에서 재사용할 수 없어 최소한만 옮겨 적었다.
fn load_rule() -> Result<SignificanceRuleTable, String> {
    let path = resolve_rule_path()?;
    let text =
        std::fs::read_to_string(&path).map_err(|e| format!("{} 읽기 실패: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("{} 파싱 실패: {e}", path.display()))
}

fn resolve_rule_path() -> Result<PathBuf, String> {
    const RULE_RELATIVE: &str = "history/rules/mineral-discovery.json";

    if let Ok(override_dir) = std::env::var("STARFALL_DATA_DIR") {
        let path = Path::new(&override_dir).join(RULE_RELATIVE);
        return if path.is_file() {
            Ok(path)
        } else {
            Err(format!(
                "STARFALL_DATA_DIR={override_dir} 아래에 {RULE_RELATIVE} 가 없다"
            ))
        };
    }

    for candidate in ["data", "../data"] {
        let path = Path::new(candidate).join(RULE_RELATIVE);
        if path.is_file() {
            return Ok(path);
        }
    }
    Err(format!(
        "{RULE_RELATIVE} 를 'data/'·'../data/' 어디에서도 못 찾았다(STARFALL_DATA_DIR 로 지정 가능)"
    ))
}
