//! STARFALL DYNASTY — QA 봇 하네스 (p0-02-networking-spike T12)
//!
//! 스프린트 계약 `_workspace/p0-02-networking-spike/02_sprint_contract.md` §3.1 의 설계를 구현한다.
//! 이 도구가 틀리면 슬라이스의 판정이 통째로 거짓이 되므로, 계측 로직에는 자체 테스트가 있다
//! (`tests/ledger_accounting.rs`, `tests/wire_fixtures.rs`, `tests/token_vectors.rs`).
//!
//! 종료 코드: `0` 성공+게이트 통과 / `1` 게이트 위반 / `2` 사용법 오류 / `3` 실행 실패

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use starfall_bots::scenario::{ProbeCase, RunConfig, Stage};
use starfall_bots::{report, scenario, token};

const DEFAULT_URL: &str = "ws://127.0.0.1:8080/ws";

const USAGE: &str = r#"starfall bots — QA 봇 하네스 (p0-02)

사용법:
  bots run --scenario a|b|c|d|e --out <DIR> [옵션]
  bots probe --case <CASE> [옵션]
  bots resume --label bot-007 --observer bot-008 --out <FILE.json> [--fly 1.0] [--settle 1.5] [--gap 6] [--listen 3]
               SC-11: 관측자를 붙인 채 조작, 입력 정지, 끊기, 잔류 창 안 재접속. T0·T1·관측 행을 JSON 으로
  bots token [--label bot-000]
  bots subjects|identities [--bots 30 | --count 30] [--disjoint-from <uuid[,uuid…]>]
  bots help

run 옵션 (기본값):
  --url <URL>          ws://127.0.0.1:8080/ws
  --bots <N>           30
  --seed <U64>         42          위상 지터에만 쓴다. 요약에 기록된다
  --duration <SEC>     60          A·C·D 단계의 유지 시간
  --interval <MS>      500         ping 간격 (A·C·D)
  --ramp <SEC>         5           전원이 접속하기까지의 램프 (A·C·D)
  --cycles <N>         5           B 단계: 접속→ping→종료 반복 횟수
  --pings <N>          3           B 단계: 한 접속당 ping 수
  --burst <N>          2000        C 단계: 폭주 봇이 보낼 명령 수
  --out <DIR>          (필수) sessions.json / commands.csv / summary.json / correlations.txt
  --live-corr <FILE>   SESSION_READY 수신 즉시 correlation 을 덧붙일 파일 (SC-61: 실행 중 조회용)

probe case:
  auth-ok order duplicate inflight slow-consumer oversize binary idle
  fly cheat-position cheat-attitude cheat-range cheat-seq cheat-flood pre-ready   (p1-01)
  tick-burst         SC-89 (g) 양성 대조. 한 tick 에 9건 × 10회(0.5초 간격) → 서버가 위반을 계수하고
                     예산(10초 8건)이 차면 close 1002 = PROTOCOL_VIOLATION 으로 닫아야 한다.
                     --count <N> 으로 회수 조정(기본·최소 10). gap 이 평균 속도를 18 Hz 로 낮춰
                     rate_limit_hz(40)에는 걸리지 않는다 — 두 문턱을 갈라야 위반 경로를 밟는다
  cheat-range-turn   SC-24 (c)(d) 한 실행. 필수: --turn-rate-max-deg-s <data 값> --carry-forward-max-ticks <data 값>
                     --count <N> = 범위 초과 주입 수(1~6, 위반 예산 10초 8건 아래)  [--series-out <FILE.json>]

환경 변수:
  STARFALL_DEV_AUTH_SECRET   (필수) 개발용 토큰 서명 비밀. 기본값을 코드에 두지 않는다 (ADR-0008 §3)

단계 (스펙 §7):
  e  비행 부하   모든 봇이 SET_SHIP_CONTROL 로 전방 추력 (p1-01)
  a  정상 상태   봇 30 + (밖에서 붙은) Unity 1, 60초 유지
  b  회전        접속→ping 3→종료를 5회 반복 (세션 이벤트 300건)
  c  백프레셔    정상 29 + 폭주 1
  d  기록 내구성 A 와 같은 부하를 길게. 그 사이 QA 가 PostgreSQL 을 멈춘다
"#;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprint!("{USAGE}");
        return ExitCode::from(2);
    }
    match args[0].as_str() {
        "run" => cmd_run(&args[1..]).await,
        "probe" => cmd_probe(&args[1..]).await,
        "resume" => cmd_resume(&args[1..]).await,
        "token" => cmd_token(&args[1..]),
        // client 의 `02_client_ack.md` 가 `identities --count 30` 으로 적었다. 그 명령이
        // 그대로 동작하도록 이름과 인자를 둘 다 받는다 — 문서와 도구가 어긋나지 않게.
        "subjects" | "identities" => cmd_subjects(&args[1..]),
        "help" | "--help" | "-h" => {
            print!("{USAGE}");
            ExitCode::SUCCESS
        }
        other => {
            eprintln!("모르는 하위 명령: {other}\n");
            eprint!("{USAGE}");
            ExitCode::from(2)
        }
    }
}

struct Args {
    map: std::collections::BTreeMap<String, String>,
}

impl Args {
    fn parse(raw: &[String]) -> Result<Self, String> {
        let mut map = std::collections::BTreeMap::new();
        let mut i = 0;
        while i < raw.len() {
            let key = raw[i].clone();
            if !key.starts_with("--") {
                return Err(format!("옵션이 아니다: {key}"));
            }
            let Some(val) = raw.get(i + 1) else {
                return Err(format!("{key} 에 값이 없다"));
            };
            map.insert(key.trim_start_matches("--").to_owned(), val.clone());
            i += 2;
        }
        Ok(Self { map })
    }

    fn str_or(&self, key: &str, default: &str) -> String {
        self.map
            .get(key)
            .cloned()
            .unwrap_or_else(|| default.to_owned())
    }

    fn req(&self, key: &str) -> Result<String, String> {
        self.map
            .get(key)
            .cloned()
            .ok_or_else(|| format!("--{key} 가 필요하다"))
    }

    fn num_or<T: std::str::FromStr>(&self, key: &str, default: T) -> Result<T, String> {
        match self.map.get(key) {
            None => Ok(default),
            Some(v) => v
                .parse::<T>()
                .map_err(|_| format!("--{key} 값이 숫자가 아니다: {v}")),
        }
    }
}

async fn cmd_run(raw: &[String]) -> ExitCode {
    let args = match Args::parse(raw) {
        Ok(a) => a,
        Err(e) => return usage_error(&e),
    };
    let secret = match token::secret_from_env() {
        Ok(s) => s,
        Err(e) => return usage_error(&e),
    };
    let stage_str = match args.req("scenario") {
        Ok(v) => v,
        Err(e) => return usage_error(&e),
    };
    let Some(stage) = Stage::parse(&stage_str) else {
        return usage_error(&format!("모르는 --scenario: {stage_str} (a|b|c|d|e)"));
    };
    let out = match args.req("out") {
        Ok(v) => PathBuf::from(v),
        Err(e) => return usage_error(&e),
    };

    let cfg = RunConfig {
        stage,
        url: args.str_or("url", DEFAULT_URL),
        secret,
        bots: match args.num_or("bots", 30usize) {
            Ok(v) => v,
            Err(e) => return usage_error(&e),
        },
        seed: match args.num_or("seed", 42u64) {
            Ok(v) => v,
            Err(e) => return usage_error(&e),
        },
        duration: Duration::from_secs(match args.num_or("duration", 60u64) {
            Ok(v) => v,
            Err(e) => return usage_error(&e),
        }),
        interval: Duration::from_millis(match args.num_or("interval", 500u64) {
            Ok(v) => v,
            Err(e) => return usage_error(&e),
        }),
        ramp: Duration::from_secs(match args.num_or("ramp", 5u64) {
            Ok(v) => v,
            Err(e) => return usage_error(&e),
        }),
        cycles: match args.num_or("cycles", 5u32) {
            Ok(v) => v,
            Err(e) => return usage_error(&e),
        },
        pings: match args.num_or("pings", 3u32) {
            Ok(v) => v,
            Err(e) => return usage_error(&e),
        },
        burst: match args.num_or("burst", 2000u32) {
            Ok(v) => v,
            Err(e) => return usage_error(&e),
        },
        out,
        live_corr: args.map.get("live-corr").map(PathBuf::from),
    };

    eprintln!(
        "[bots] stage={} url={} bots={} seed={} duration={}s interval={}ms",
        cfg.stage.as_str(),
        cfg.url,
        cfg.bots,
        cfg.seed,
        cfg.duration.as_secs(),
        cfg.interval.as_millis()
    );

    let (clock, outcomes) = scenario::run(&cfg).await;
    if outcomes.is_empty() {
        eprintln!("[bots] 연결이 하나도 만들어지지 않았다.");
        return ExitCode::from(3);
    }

    let rep = report::build(
        report::RunMeta {
            stage: cfg.stage.as_str(),
            url: &cfg.url,
            bots: cfg.bots,
            seed: cfg.seed,
            duration_secs: cfg.duration.as_secs(),
            interval_ms: cfg.interval.as_millis() as u64,
            clock,
        },
        &outcomes,
    );
    let sessions = report::sessions(clock, &outcomes);
    if let Err(e) = report::write_all(&cfg.out, &rep, &sessions, &outcomes) {
        eprintln!("[bots] 산출물 쓰기 실패: {e}");
        return ExitCode::from(3);
    }

    print_run_summary(&rep, &cfg.out);

    if rep.gates.sessions_ready == 0 {
        return ExitCode::from(3);
    }
    if rep.gates.all_ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

fn print_run_summary(rep: &report::RunReport, out: &std::path::Path) {
    let a = &rep.aggregate;
    println!("=== bots {} ===", rep.stage);
    println!(
        "connections attempted : {}",
        rep.gates.connections_attempted
    );
    println!("sessions ready        : {}", rep.gates.sessions_ready);
    println!(
        "correlations collected: {}",
        rep.gates.correlations_collected
    );
    println!("commands sent         : {}", a.sent_total);
    println!(
        "COMMAND_RESULT        : {} (accepted {} / rejected {})",
        a.results_total, a.accepted, a.rejected
    );
    println!("PING_REPLY            : {}", a.replies_total);
    println!(
        "loss(missing results) : {}  missing replies: {}",
        a.missing_results, a.missing_replies
    );
    println!(
        "duplicates            : results {} / replies {}   unmatched: results {} / replies {}",
        a.duplicate_results, a.duplicate_replies, a.unmatched_results, a.unmatched_replies
    );
    println!("order violations (I-15): {}", a.order_violations);
    println!(
        "tick mismatches (AC-6c) : {}  (compared {})",
        a.tick_mismatches, a.ticks_compared
    );
    println!("probe_seq mismatches   : {}", a.probe_seq_mismatches);
    if !a.rejected_by_reason.is_empty() {
        println!("rejected by reason     : {:?}", a.rejected_by_reason);
    }
    if !a.unknown_message_types.is_empty() {
        println!("unknown message types  : {:?}", a.unknown_message_types);
    }
    println!(
        "rtt ms  n={} p50={:?} p99={:?} max={:?}",
        a.rtt.count, a.rtt.p50_ms, a.rtt.p99_ms, a.rtt.max_ms
    );
    println!(
        "ack ms  n={} p50={:?} p99={:?} max={:?}",
        a.ack.count, a.ack.p50_ms, a.ack.p99_ms, a.ack.max_ms
    );
    println!(
        "connect ms p50={:?} max={:?}   ready ms p50={:?} max={:?}",
        rep.connect_ms.p50_ms, rep.connect_ms.max_ms, rep.ready_ms.p50_ms, rep.ready_ms.max_ms
    );
    println!(
        "server-initiated closes: {}",
        rep.gates.server_initiated_closes
    );
    println!("wire errors            : {}", rep.gates.wire_errors);
    if !a.errors.is_empty() {
        println!("errors ({}):", a.errors.len());
        for e in a.errors.iter().take(10) {
            println!("  - {e}");
        }
    }
    println!(
        "GATES one_to_one={} results_partition={} accepted_reply_pairing={} (ping pairs {}) accepted_exercised={} (accepted {}) all_ok={}",
        rep.gates.one_to_one,
        rep.gates.results_partition,
        rep.gates.accepted_reply_pairing,
        a.accepted_expecting_reply,
        rep.gates.accepted_exercised,
        a.accepted,
        rep.gates.all_ok
    );
    println!("evidence -> {}", out.display());
    println!(
        "clock_base_unix_ms={} (commands.csv 의 *_us 에 이 값을 더하면 벽시계가 된다)",
        rep.clock_base_unix_ms
    );
}

async fn cmd_probe(raw: &[String]) -> ExitCode {
    let args = match Args::parse(raw) {
        Ok(a) => a,
        Err(e) => return usage_error(&e),
    };
    let secret = match token::secret_from_env() {
        Ok(s) => s,
        Err(e) => return usage_error(&e),
    };
    let case_str = match args.req("case") {
        Ok(v) => v,
        Err(e) => return usage_error(&e),
    };
    let Some(case) = ProbeCase::parse(&case_str) else {
        return usage_error(&format!("모르는 --case: {case_str}"));
    };
    let url = args.str_or("url", DEFAULT_URL);
    // 판정에 쓰는 신원은 언제나 server 정본의 `bot-NNN` 이다(§3.3 정렬).
    let label = args.str_or("label", "bot-000");
    let count: u32 = match args.num_or("count", 10u32) {
        Ok(v) => v,
        Err(e) => return usage_error(&e),
    };

    println!("=== probe {} ===", case.as_str());
    println!("계약 항목: {}", case.contract_item());
    let outcome = scenario::run_probe(case, &url, &secret, &label, count).await;
    let s = outcome.ledger.finish();
    if let Some(sess) = &outcome.ledger.session {
        println!(
            "session_id={} correlation_id={:?} actor_id={} tick_hz={} server_version={} ready_tick={}",
            sess.session_id,
            sess.correlation_id,
            sess.actor_id,
            sess.tick_hz,
            sess.server_version,
            sess.ready_tick
        );
        println!(
            "close: code={:?} reason_text={:?} initiator={} meaning={}",
            sess.close_code,
            sess.close_reason_text,
            sess.close_initiator,
            sess.close_code
                .map(starfall_bots::ledger::close_code_meaning)
                .unwrap_or("-")
        );
        // SC-14 의 증거: actor_id 가 토큰 주체와 같은가.
        let subject = token::subject_for(&label);
        println!(
            "token subject={} actor_id_matches_subject={}",
            subject,
            sess.actor_id.to_string() == subject
        );
    } else {
        println!(
            "SESSION_READY 를 받지 못했다 (connect_error={:?})",
            outcome.connect_error
        );
    }
    println!(
        "sent={} results={} (accepted {} / rejected {}) replies={} order_violations={} missing_results={}",
        s.sent_total,
        s.results_total,
        s.accepted,
        s.rejected,
        s.replies_total,
        s.order_violations,
        s.missing_results
    );
    println!(
        "tick_mismatches={} (compared {}) same_tick_holds={}",
        s.tick_mismatches,
        s.ticks_compared,
        s.same_tick_holds()
    );
    if !s.rejected_by_reason.is_empty() {
        println!("rejected_by_reason={:?}", s.rejected_by_reason);
    }
    if !s.errors.is_empty() {
        println!("errors={:?}", s.errors);
    }
    if !s.wire_errors.is_empty() {
        println!("wire_errors={:?}", s.wire_errors);
    }
    println!(
        "rtt ms n={} p50={:?} max={:?}",
        s.rtt.count, s.rtt.p50_ms, s.rtt.max_ms
    );
    let snap = outcome.snapshots.finish();
    // SC-88 ③: 넘겨받은 세션이 같은 함선을 조종하는가 — 두 probe 의 이 줄을 대조한다.
    println!("controlled_ship_id={:?}", snap.controlled_ship_id);
    if snap.snapshots_received > 0 {
        println!(
            "snapshots={} bytes={} interval={:?} violations(interval/order/controlled/ack)={}/{}/{}/{} excluded_first={} max_ships={}",
            snap.snapshots_received,
            snap.snapshot_bytes_received,
            snap.snapshot_interval_ticks,
            snap.interval_violations,
            snap.ship_order_violations,
            snap.controlled_ship_missing,
            snap.ack_regressions,
            snap.first_snapshots_excluded,
            snap.max_ships_seen
        );
        println!(
            "own: path={:.2} m displacement={:.2} m speed_max={:.2} m/s radius_max={:.2} m nearest_min={:?} presence={:?}",
            snap.own_path_length_m,
            snap.own_displacement_m,
            snap.own_speed_max_mps,
            snap.own_max_radius_m,
            snap.nearest_ship_min_m,
            snap.presence_values
        );
        println!(
            "ack_last={:?} gates_ok={}",
            snap.ack_last,
            outcome.snapshots.gates_ok()
        );
    }

    if outcome.connect_error.is_some() {
        return ExitCode::from(3);
    }
    if case == ProbeCase::CheatRangeTurn {
        return report_range_turn(&args, &outcome);
    }
    ExitCode::SUCCESS
}

/// SC-11 재개 시나리오. **판정하지 않는다** — T0(끊기기 직전 마지막 자기 스냅샷)·T1(재개 후 첫 자기
/// 스냅샷)·관측자가 본 그 함선의 행을 JSON 으로 남기고, 독립 계산과 대조는
/// `tests/e2e/resume_check.py` 가 한다(서버가 아닌 client C# 적분기로 — I-25).
async fn cmd_resume(raw: &[String]) -> ExitCode {
    use starfall_bots::conn::{
        Behavior, BotSpec, Clock, ConnectionOutcome, MARK_LAST_INPUT, MARK_SEQ1, run_connection,
    };

    let args = match Args::parse(raw) {
        Ok(a) => a,
        Err(e) => return usage_error(&e),
    };
    let secret = match token::secret_from_env() {
        Ok(s) => s,
        Err(e) => return usage_error(&e),
    };
    let out = match args.req("out") {
        Ok(v) => PathBuf::from(v),
        Err(e) => return usage_error(&e),
    };
    let url = args.str_or("url", DEFAULT_URL);
    let label = args.str_or("label", "bot-007");
    let observer = args.str_or("observer", "bot-008");
    let secs = |k: &str, d: f64| -> Result<Duration, String> {
        args.num_or(k, d).map(Duration::from_secs_f64)
    };
    let (fly, settle, gap, listen) = match (
        secs("fly", 1.0),
        secs("settle", 1.5),
        secs("gap", 6.0),
        secs("listen", 3.0),
    ) {
        (Ok(a), Ok(b), Ok(c), Ok(d)) => (a, b, c, d),
        _ => return usage_error("--fly/--settle/--gap/--listen 은 초 단위 숫자다"),
    };

    let clock = Clock::start();
    let spec = |lbl: &str, behavior: Behavior| BotSpec {
        label: lbl.to_owned(),
        url: url.clone(),
        token: token::identity(&secret, lbl).1,
        behavior,
        clock,
        live_corr: None,
    };
    // 관측자가 먼저 붙고, 재개 2구간이 끝날 때까지 남는다.
    let observe_for = fly + settle + gap + listen + Duration::from_secs(3);
    let obs_task = tokio::spawn(run_connection(spec(
        &observer,
        Behavior::Listen {
            listen: observe_for,
            seq1_after: None,
        },
    )));
    tokio::time::sleep(Duration::from_millis(800)).await;
    let leg1 = run_connection(spec(&label, Behavior::ResumeLeg1 { fly, settle })).await;
    tokio::time::sleep(gap).await;
    let leg2 = run_connection(spec(
        &label,
        Behavior::Listen {
            listen,
            seq1_after: Some(Duration::from_secs(1)),
        },
    ))
    .await;
    let obs = match obs_task.await {
        Ok(o) => o,
        Err(e) => {
            eprintln!("[bots] 관측자 태스크 실패: {e}");
            return ExitCode::from(3);
        }
    };

    let ship1 = leg1.snapshots.finish().controlled_ship_id;
    let ship2 = leg2.snapshots.finish().controlled_ship_id;
    let t0 = leg1.snapshots.own_series().last().cloned();
    let t1 = leg2.snapshots.own_series().first().cloned();
    let last_input = leg1
        .ledger
        .marked_outcomes(MARK_LAST_INPUT)
        .into_iter()
        .find_map(|(_, o)| o);
    let seq1 = leg2
        .ledger
        .marked_outcomes(MARK_SEQ1)
        .into_iter()
        .find_map(|(_, o)| o);
    let ship_rows: Vec<&String> = match &ship1 {
        Some(id) => obs
            .snapshots
            .csv_rows()
            .iter()
            .filter(|r| r.split(',').nth(2) == Some(id.as_str()))
            .collect(),
        None => Vec::new(),
    };
    let sess = |o: &ConnectionOutcome| {
        o.ledger.session.as_ref().map(|s| {
            serde_json::json!({
                "session_id": s.session_id, "correlation_id": s.correlation_id,
                "actor_id": s.actor_id, "tick_hz": s.tick_hz, "ready_tick": s.ready_tick,
                "close_initiator": s.close_initiator,
            })
        })
    };
    let outcome_json = |o: &Option<starfall_bots::ledger::CommandOutcome>| {
        o.as_ref().map(
            |o| serde_json::json!({"status": o.status, "reason": o.reason_code, "tick": o.tick}),
        )
    };
    let doc = serde_json::json!({
        "label": label, "observer": observer,
        "fly_s": fly.as_secs_f64(), "settle_s": settle.as_secs_f64(),
        "gap_s": gap.as_secs_f64(), "listen_s": listen.as_secs_f64(),
        "leg1": {
            "session": sess(&leg1), "controlled_ship_id": ship1,
            "last_input_result": outcome_json(&last_input),
            "own_samples": leg1.snapshots.own_series().len(), "t0": t0,
            "summary": leg1.ledger.finish(),
        },
        "leg2": {
            "session": sess(&leg2), "controlled_ship_id": ship2,
            "own_samples": leg2.snapshots.own_series().len(), "t1": t1,
            "own_series_head": leg2.snapshots.own_series().iter().take(20).collect::<Vec<_>>(),
            "seq1_result": outcome_json(&seq1),
            "ack_last": leg2.snapshots.finish().ack_last,
            "summary": leg2.ledger.finish(),
        },
        "observer_session": sess(&obs),
        // SC-32: 관측자가 디스폰 **뒤에도** 보고 있었는가(마지막으로 받은 스냅샷 tick).
        "observer_first_tick": obs.snapshots.own_series().first().map(|s| s.tick),
        "observer_last_tick": obs.snapshots.own_series().last().map(|s| s.tick),
        // SC-68 (f): 다른 actor 인 관측자는 잔류 함선을 넘겨받지 않고 자기 함선을 받아야 한다.
        "observer_controlled_ship_id": obs.snapshots.finish().controlled_ship_id,
        "observer_rows_header": starfall_bots::snapshot::SNAPSHOT_CSV_HEADER,
        "observer_rows": ship_rows,
    });
    let text = match serde_json::to_string_pretty(&doc) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("[bots] 직렬화 실패: {e}");
            return ExitCode::from(3);
        }
    };
    if let Err(e) = std::fs::write(&out, text) {
        eprintln!("[bots] 쓰기 실패 {}: {e}", out.display());
        return ExitCode::from(3);
    }
    println!(
        "resume: ship leg1={ship1:?} leg2={ship2:?} same={} t0_tick={:?} t1_tick={:?} last_input={:?} seq1={:?} observer_rows={}",
        ship1.is_some() && ship1 == ship2,
        t0.as_ref().map(|s| s.tick),
        t1.as_ref().map(|s| s.tick),
        last_input,
        seq1,
        ship_rows.len()
    );
    println!("evidence -> {}", out.display());
    ExitCode::SUCCESS
}

/// `cheat-range-turn` 의 판정. 한도 값은 **`data/` 에서 읽지 않고 인자로 받는다** — 그 시점의
/// `data/` 값을 증거에 함께 적으라는 계약 §0.7 을 따르기 위해서다(출력에 그대로 찍힌다).
fn report_range_turn(args: &Args, outcome: &starfall_bots::conn::ConnectionOutcome) -> ExitCode {
    use starfall_bots::range_turn::{self, Injected, MARK_INJECTED, MARK_TURN_START, Params};

    let turn_rate: f64 = match args.req("turn-rate-max-deg-s").and_then(|v| {
        v.parse::<f64>()
            .map_err(|_| format!("--turn-rate-max-deg-s 값이 숫자가 아니다: {v}"))
    }) {
        Ok(v) => v,
        Err(e) => return usage_error(&e),
    };
    let carry: u64 = match args.req("carry-forward-max-ticks").and_then(|v| {
        v.parse::<u64>()
            .map_err(|_| format!("--carry-forward-max-ticks 값이 숫자가 아니다: {v}"))
    }) {
        Ok(v) => v,
        Err(e) => return usage_error(&e),
    };
    let Some(sess) = &outcome.ledger.session else {
        println!("RANGE_TURN 판정 불가: SESSION_READY 없음");
        return ExitCode::from(1);
    };
    let params = Params {
        tick_hz: sess.tick_hz,
        turn_rate_max_deg_s: turn_rate,
        carry_forward_max_ticks: carry,
        tolerance_deg_per_tick: 0.001,
    };
    let injected: Vec<Injected> = outcome
        .ledger
        .marked_outcomes(MARK_INJECTED)
        .into_iter()
        .map(|(seq, o)| Injected {
            input_seq: u64::from(seq),
            outcome: o,
        })
        .collect();
    let turn_start = outcome
        .ledger
        .marked_outcomes(MARK_TURN_START)
        .into_iter()
        .find_map(|(_, o)| o.map(|o| o.tick));
    let series = outcome.snapshots.own_series();
    let rep = range_turn::analyze(series, &injected, turn_start, params);
    // 원자료를 남긴다 — 판정 방식을 바꿔 다시 볼 수 있게(라운드 3 에서 한 번 필요했다).
    if let Some(path) = args.map.get("series-out") {
        let doc = serde_json::json!({
            "params": {"tick_hz": params.tick_hz, "turn_rate_max_deg_s": params.turn_rate_max_deg_s,
                       "carry_forward_max_ticks": params.carry_forward_max_ticks},
            "turn_start_tick": turn_start,
            "injected": injected.iter().map(|i| serde_json::json!({
                "input_seq": i.input_seq,
                "outcome": i.outcome.as_ref().map(|o| serde_json::json!({"status": o.status, "reason": o.reason_code, "tick": o.tick})),
            })).collect::<Vec<_>>(),
            "report": rep,
            "own_series": series,
        });
        if let Err(e) = std::fs::write(path, serde_json::to_string_pretty(&doc).unwrap_or_default())
        {
            eprintln!("[bots] series-out 쓰기 실패: {e}");
        }
    }
    println!(
        "params: tick_hz={} turn_rate_max_deg_s={} carry_forward_max_ticks={} own_samples={}",
        params.tick_hz,
        params.turn_rate_max_deg_s,
        params.carry_forward_max_ticks,
        series.len()
    );
    let c = &rep.carry;
    println!(
        "(c) injected={} rejected_malformed={} other={} missing={} reject_ticks={:?}..{:?} injected_seq_acked={}",
        c.injected,
        c.rejected_malformed,
        c.other_outcomes,
        c.missing_results,
        c.reject_tick_first,
        c.reject_tick_last,
        c.injected_seq_acked
    );
    println!(
        "(c) window_pairs={} speed {:?} -> {:?} m/s drops={} path={:.3} m span_within_carry={} holds={}",
        c.window_pairs,
        c.speed_first_mps,
        c.speed_last_mps,
        c.speed_drops,
        c.path_m,
        c.span_within_carry,
        c.holds
    );
    let t = &rep.turn;
    println!(
        "(d) 뱃머리 기준 turn_start_tick={:?} pairs={} moving={} saturated={} over_limit={} max={:.4}°/tick limit={:.4}°/tick",
        t.turn_start_tick,
        t.pairs,
        t.moving_pairs,
        t.saturated_pairs,
        t.over_limit_pairs,
        t.max_deg_per_tick,
        t.limit_deg_per_tick
    );
    println!(
        "(d) 참고: 롤 포함 전체 자세 회전 max={:.4}°/tick, 한도 초과 쌍 {} (판정 아님 — 롤은 별도 권한)",
        t.max_total_deg_per_tick, t.total_over_limit_pairs
    );
    println!(
        "(d) omega_samples={} |ω_aim|max={:.3}°/s over={} |ω_roll|max={:.3}°/s holds={}",
        t.omega_samples,
        t.omega_aim_max_deg_s,
        t.omega_aim_over_limit,
        t.omega_roll_max_deg_s,
        t.holds
    );
    for w in c.why_not.iter().chain(t.why_not.iter()) {
        println!("  why_not: {w}");
    }
    println!("RANGE_TURN carry_holds={} turn_holds={}", c.holds, t.holds);
    if c.holds && t.holds {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

fn cmd_token(raw: &[String]) -> ExitCode {
    let args = match Args::parse(raw) {
        Ok(a) => a,
        Err(e) => return usage_error(&e),
    };
    let secret = match token::secret_from_env() {
        Ok(s) => s,
        Err(e) => return usage_error(&e),
    };
    let label = args.str_or("label", "bot-000");
    let (subject, tok) = token::identity(&secret, &label);
    println!("label   : {label}");
    println!("subject : {subject}");
    println!("token   : {tok}");
    println!("header  : Authorization: Bearer {tok}");
    ExitCode::SUCCESS
}

fn cmd_subjects(raw: &[String]) -> ExitCode {
    let args = match Args::parse(raw) {
        Ok(a) => a,
        Err(e) => return usage_error(&e),
    };
    // `--bots` 와 `--count` 를 모두 받는다(client 문서가 `--count` 를 쓴다).
    let n: usize = match args.num_or("bots", 0usize).and_then(|b| {
        if b > 0 {
            Ok(b)
        } else {
            args.num_or("count", 30usize)
        }
    }) {
        Ok(v) => v,
        Err(e) => return usage_error(&e),
    };
    let mut subjects = Vec::with_capacity(n);
    for i in 0..n {
        let label = token::bot_label(i);
        let subject = token::subject_for(&label);
        println!("{label} {subject} uuidv7={}", token::is_uuid_v7(&subject));
        subjects.push(subject);
    }

    // client 요청: Unity 주체와 겹치지 않음을 한 줄로 확인한다(§0.6 집합 대조의 전제).
    let Some(others) = args.map.get("disjoint-from") else {
        return ExitCode::SUCCESS;
    };
    let mut collisions = Vec::new();
    for other in others.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        if subjects.iter().any(|s| s == other) {
            collisions.push(other.to_owned());
        }
    }
    if collisions.is_empty() {
        println!("disjoint=true checked={n} against={others}");
        ExitCode::SUCCESS
    } else {
        println!("disjoint=false COLLISION={}", collisions.join(","));
        ExitCode::from(1)
    }
}

fn usage_error(msg: &str) -> ExitCode {
    eprintln!("오류: {msg}\n");
    eprint!("{USAGE}");
    ExitCode::from(2)
}
