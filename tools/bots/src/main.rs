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
  bots run --scenario a|b|c|d --out <DIR> [옵션]
  bots probe --case <CASE> [옵션]
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

환경 변수:
  STARFALL_DEV_AUTH_SECRET   (필수) 개발용 토큰 서명 비밀. 기본값을 코드에 두지 않는다 (ADR-0008 §3)

단계 (스펙 §7):
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
        return usage_error(&format!("모르는 --scenario: {stage_str} (a|b|c|d)"));
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
        "GATES one_to_one={} accepted_reply_pairing={} all_ok={}",
        rep.gates.one_to_one, rep.gates.accepted_reply_pairing, rep.gates.all_ok
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
            "close: code={:?} reason_text={:?} initiator={}",
            sess.close_code, sess.close_reason_text, sess.close_initiator
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

    if outcome.connect_error.is_some() {
        ExitCode::from(3)
    } else {
        ExitCode::SUCCESS
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
