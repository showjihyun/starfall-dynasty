//! 산출물 4종. **스프린트 계약 §3.1 이 고정한 파일 이름과 열을 그대로 쓴다** —
//! QA 의 DB 대조(`tests/e2e/**`)가 이 이름들을 읽는다.
//!
//! | 파일 | 쓰이는 항목 |
//! |------|------------|
//! | `sessions.json` | SC-52, SC-57~59, SC-61, SC-69 |
//! | `commands.csv` | SC-53~55, SC-63~65, SC-67 |
//! | `summary.json` | SC-53~56, SC-63~65, M-3, M-5, M-7 |
//! | `correlations.txt` | §0.6 세션 집합 — **Unity 의 correlation 을 QA 가 뒤에 덧붙인다** |

use std::fs;
use std::io::Write;
use std::path::Path;

use serde::Serialize;

use crate::conn::{Clock, ConnectionOutcome};
use crate::ledger::{self, COMMANDS_CSV_HEADER, LedgerSummary};
use crate::snapshot::{self, SNAPSHOT_CSV_HEADER, SnapshotSummary};
use crate::stats;

#[derive(Debug, Clone, Serialize)]
pub struct SessionOut {
    pub bot: String,
    pub session_id: String,
    pub correlation_id: Option<String>,
    pub actor_id: String,
    pub tick_hz: u32,
    pub server_version: String,
    pub ready_tick: u64,
    pub connected_at_us: u64,
    pub ready_at_us: u64,
    pub closed_at_us: Option<u64>,
    pub connected_at_unix_ms: u64,
    pub ready_at_unix_ms: u64,
    pub closed_at_unix_ms: Option<u64>,
    pub close_code: Option<u16>,
    /// 서버가 보낸 Close 프레임의 code (SC-28).
    pub peer_close_code: Option<u16>,
    pub close_reason_text: Option<String>,
    /// `client` = 봇이 먼저 Close 를 보냈다 / `server` = 서버가 먼저 / `none` = 교환 없음.
    pub close_initiator: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Gates {
    /// SC-53: 보낸 수 == 받은 `COMMAND_RESULT` 수, 손실·중복·미대응 0
    pub one_to_one: bool,
    /// SC-54: ACCEPTED == PING_REPLY, probe_seq 전부 일치
    pub accepted_reply_pairing: bool,
    /// 스펙 §5.1a: `COMMAND_RESULT` 수 == accepted + rejected
    pub results_partition: bool,
    /// 스펙 §5.1a 의무: `accepted > 0`. 이것이 false 면 위 두 항등식은 `0 == 0` 이라 아무것도
    /// 검사하지 않은 것이다(라운드 2 의 거짓 초록불).
    pub accepted_exercised: bool,
    /// SC-55 / I-15
    pub order_violations: u64,
    /// SC-19 / AC-6(c)
    pub tick_mismatches: u64,
    pub ticks_compared: u64,
    /// SC-52: 서버가 먼저 닫은 연결 수 (0 이어야 한다)
    pub server_initiated_closes: u64,
    /// 연결 시도 중 SESSION_READY 까지 간 수
    pub connections_attempted: usize,
    pub sessions_ready: usize,
    /// correlation_id 가 null 이 아닌 세션 수 = 집합 대조에 쓸 수 있는 수
    pub correlations_collected: usize,
    pub wire_errors: usize,
    /// 위 전부가 통과했는가. **probe 실행에는 적용하지 않는다**(거부·강제 종료가 목적이므로).
    pub all_ok: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct RunReport {
    pub stage: String,
    pub url: String,
    pub bots: usize,
    pub seed: u64,
    pub duration_secs: u64,
    pub interval_ms: u64,
    pub clock_base_unix_ms: u64,
    pub finished_unix_ms: u64,
    pub connect_ms: stats::Summary,
    pub ready_ms: stats::Summary,
    pub gates: Gates,
    pub aggregate: LedgerSummary,
    /// p1-01: 스냅샷 관측 집계와 관측자별 요약.
    pub snapshots: SnapshotSummary,
    pub snapshots_per_bot: Vec<SnapshotSummary>,
    pub per_bot: Vec<LedgerSummary>,
    pub connect_errors: Vec<String>,
}

/// 실행 자체를 설명하는 값들. 리포트에 그대로 실려 다음 측정과 비교할 수 있게 한다
/// (스프린트 계약 §0.7 측정 환경 기록).
#[derive(Debug, Clone, Copy)]
pub struct RunMeta<'a> {
    pub stage: &'a str,
    pub url: &'a str,
    pub bots: usize,
    pub seed: u64,
    pub duration_secs: u64,
    pub interval_ms: u64,
    pub clock: Clock,
}

pub fn build(meta: RunMeta<'_>, outcomes: &[ConnectionOutcome]) -> RunReport {
    let RunMeta {
        stage,
        url,
        bots,
        seed,
        duration_secs,
        interval_ms,
        clock,
    } = meta;
    let per_bot: Vec<LedgerSummary> = outcomes.iter().map(|o| o.ledger.finish()).collect();
    let snap_per_bot: Vec<SnapshotSummary> =
        outcomes.iter().map(|o| o.snapshots.finish()).collect();
    let rtt: Vec<f64> = outcomes
        .iter()
        .flat_map(|o| o.ledger.rtt_samples_ms())
        .collect();
    let ack: Vec<f64> = outcomes
        .iter()
        .flat_map(|o| o.ledger.ack_samples_ms())
        .collect();
    let aggregate = ledger::aggregate(&per_bot, rtt, ack);

    let sessions_ready = outcomes
        .iter()
        .filter(|o| o.ledger.session.is_some())
        .count();
    let correlations_collected = outcomes
        .iter()
        .filter_map(|o| o.ledger.session.as_ref())
        .filter(|s| s.correlation_id.is_some())
        .count();
    let server_initiated_closes = outcomes
        .iter()
        .filter_map(|o| o.ledger.session.as_ref())
        .filter(|s| s.close_initiator == "server")
        .count() as u64;

    let gates = Gates {
        one_to_one: aggregate.one_to_one_holds(),
        accepted_reply_pairing: aggregate.accepted_reply_pairing_holds(),
        results_partition: aggregate.results_partition_holds(),
        accepted_exercised: aggregate.accepted_path_exercised(),
        order_violations: aggregate.order_violations,
        tick_mismatches: aggregate.tick_mismatches,
        ticks_compared: aggregate.ticks_compared,
        server_initiated_closes,
        connections_attempted: outcomes.len(),
        sessions_ready,
        correlations_collected,
        wire_errors: aggregate.wire_errors.len(),
        all_ok: false,
    };
    // 명령→응답 3단언(스펙 §5.1a)은 `LedgerSummary::command_reply_gates_hold` 와 같은 것이다.
    // 입력이 전부 0 이면 `accepted_exercised` 하나로 all_ok 가 false 가 된다.
    let all_ok = gates.one_to_one
        && gates.accepted_reply_pairing
        && gates.results_partition
        && gates.accepted_exercised
        && gates.order_violations == 0
        && gates.tick_mismatches == 0
        && gates.server_initiated_closes == 0
        && gates.sessions_ready == gates.connections_attempted
        && gates.correlations_collected == gates.sessions_ready
        && gates.wire_errors == 0;

    RunReport {
        stage: stage.to_owned(),
        url: url.to_owned(),
        bots,
        seed,
        duration_secs,
        interval_ms,
        clock_base_unix_ms: clock.wall_base_unix_ms,
        finished_unix_ms: clock.wall_ms_at(clock.us()),
        connect_ms: stats::summarize(outcomes.iter().map(|o| o.connect_ms).collect()),
        ready_ms: stats::summarize(outcomes.iter().filter_map(|o| o.ready_ms).collect()),
        gates: Gates { all_ok, ..gates },
        snapshots: snapshot::aggregate(&snap_per_bot),
        snapshots_per_bot: snap_per_bot,
        aggregate,
        per_bot,
        connect_errors: outcomes
            .iter()
            .filter_map(|o| o.connect_error.clone())
            .collect(),
    }
}

pub fn sessions(clock: Clock, outcomes: &[ConnectionOutcome]) -> Vec<SessionOut> {
    outcomes
        .iter()
        .filter_map(|o| o.ledger.session.as_ref())
        .map(|s| SessionOut {
            bot: s.bot.clone(),
            session_id: s.session_id.to_string(),
            correlation_id: s.correlation_id.map(|v| v.to_string()),
            actor_id: s.actor_id.to_string(),
            tick_hz: s.tick_hz,
            server_version: s.server_version.clone(),
            ready_tick: s.ready_tick,
            connected_at_us: s.connected_at_us,
            ready_at_us: s.ready_at_us,
            closed_at_us: s.closed_at_us,
            connected_at_unix_ms: clock.wall_ms_at(s.connected_at_us),
            ready_at_unix_ms: clock.wall_ms_at(s.ready_at_us),
            closed_at_unix_ms: s.closed_at_us.map(|v| clock.wall_ms_at(v)),
            close_code: s.close_code,
            peer_close_code: s.peer_close_code,
            close_reason_text: s.close_reason_text.clone(),
            close_initiator: s.close_initiator.clone(),
        })
        .collect()
}

pub fn write_all(
    out_dir: &Path,
    report: &RunReport,
    sessions: &[SessionOut],
    outcomes: &[ConnectionOutcome],
) -> std::io::Result<()> {
    fs::create_dir_all(out_dir)?;

    let summary = serde_json::to_string_pretty(report)
        .unwrap_or_else(|e| format!("{{\"serialize_error\":\"{e}\"}}"));
    fs::write(out_dir.join("summary.json"), summary)?;

    let sessions_json = serde_json::to_string_pretty(sessions)
        .unwrap_or_else(|e| format!("{{\"serialize_error\":\"{e}\"}}"));
    fs::write(out_dir.join("sessions.json"), sessions_json)?;

    let mut csv = fs::File::create(out_dir.join("commands.csv"))?;
    writeln!(csv, "{COMMANDS_CSV_HEADER}")?;
    for o in outcomes {
        for row in o.ledger.command_rows() {
            writeln!(csv, "{row}")?;
        }
    }

    let mut snap = fs::File::create(out_dir.join("snapshots.csv"))?;
    writeln!(snap, "{SNAPSHOT_CSV_HEADER}")?;
    for o in outcomes {
        for row in o.snapshots.csv_rows() {
            writeln!(snap, "{row}")?;
        }
    }

    let mut corr = fs::File::create(out_dir.join("correlations.txt"))?;
    for s in sessions {
        if let Some(c) = &s.correlation_id {
            writeln!(corr, "{c}")?;
        }
    }
    Ok(())
}
