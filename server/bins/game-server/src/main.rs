//! STARFALL 게임 서버 조립 바이너리.
//!
//! 이 바이너리가 하는 일은 조립뿐이다: 설정 로드 → tracing → 마이그레이션 → 월드 대조 →
//! tick 재개 → tick 스레드 + 영속화 태스크 + HTTP/WS 표면 → 종료. **게임 규칙은 여기 없다.**
//!
//! # 조립이 연결하는 두 방향
//!
//! ```text
//!   게이트웨이 --(SubmitHandle)--> tick 스레드 --(PersistBatch 채널)--> 영속화 태스크
//!                                       |
//!                                       +--(세션별 송신 큐)--> 게이트웨이 송신 태스크
//! ```
//!
//! `starfall-gateway` 는 `starfall-persistence` 를 **의존하지 않는다.** 채널 양 끝을
//! 붙이는 것이 이 바이너리의 역할이다.
//!
//! 실행 (워크스페이스 루트는 `server/` 다 — ADR-0001 §4):
//!
//! ```text
//! cd server && cargo run -p starfall-game-server
//! ```
//!
//! 정상 종료는 **Ctrl-C 또는 stdin 에 `shutdown` 한 줄**이다 (스펙 §5.5).

mod config;
mod data;
mod logsink;

use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use config::{Config, LogFormat};
use data::GameData;
use starfall_contracts::{ServerVersion, UuidV7};
use starfall_persistence::{PersistHandles, WorldBasics};
use starfall_sim::world::{BoundaryConstants, ShipClassConstants};
use starfall_sim::{ShipClassData, Simulation, WorldConstants};
use tokio::sync::{Notify, mpsc};
use tracing_subscriber::EnvFilter;

/// 서버 빌드 버전. `/healthz` 응답과 `SESSION_READY.server_version`.
const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() -> ExitCode {
    // 설정 로드는 tracing 초기화보다 먼저다(로그 형식이 설정에서 온다).
    let config = match Config::from_env() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("설정 로드 실패: {error}");
            return ExitCode::FAILURE;
        }
    };

    // 가드는 `main` 이 끝날 때까지 살아 있어야 한다 — 떨어뜨리는 순간 로그 싱크가 닫힌다.
    let _log_guard = init_tracing(config.log_format);

    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => {
            tracing::error!(%error, "tokio 런타임을 만들지 못했다");
            return ExitCode::FAILURE;
        }
    };

    match runtime.block_on(serve(config)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!(%error, "서버가 오류로 종료됐다");
            ExitCode::FAILURE
        }
    }
}

/// tracing 초기화. 반환한 가드는 **프로세스가 끝날 때까지** 들고 있어야 한다.
///
/// # 로그 쓰기는 이벤트를 낸 스레드에서 하지 않는다 (p1-01 라운드 2, S-B·S-C)
///
/// 기본 writer(`std::io::stdout()`)는 동기다. stdout 이 아무도 드레인하지 않는 파이프면
/// 4 KiB 뒤 그 `tracing::…!` 호출이 블로킹되고, 그 스레드가 tokio 워커라 **워커가 하나씩
/// 잠겨 서버 전체가 멎는다**(실측: 워커 12개 전부 대기, HTTP·stdin `shutdown` 무응답,
/// 세션이 `submit.close()` 에 도달하지 못해 유령으로 남음). [`logsink`] 모듈 문서 참고.
fn init_tracing(format: LogFormat) -> Option<logsink::FlushGuard> {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,starfall_gateway=debug"));

    // 드레인 스레드를 만들지 못했으면 동기 writer 로 돌아간다 — 로그가 아예 없는 것보다는
    // 낫다. 이 경로는 스레드 생성 실패뿐이라 실질적으로 일어나지 않는다.
    let Some((writer, guard)) = logsink::non_blocking_stdout() else {
        let builder = tracing_subscriber::fmt().with_env_filter(filter);
        match format {
            LogFormat::Pretty => builder.init(),
            LogFormat::Json => builder.json().init(),
        }
        tracing::warn!("로그 드레인 스레드를 만들지 못했다 — 동기 stdout 으로 돌아간다");
        return None;
    };

    let builder = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_ansi(false) // 파이프로 나가는 로그에 ANSI 이스케이프를 섞지 않는다.
        .with_writer(writer);
    match format {
        LogFormat::Pretty => builder.init(),
        LogFormat::Json => builder.json().init(),
    }
    Some(guard)
}

async fn serve(config: Config) -> Result<(), Box<dyn std::error::Error>> {
    let server_version = ServerVersion::parse(VERSION)
        .ok_or("CARGO_PKG_VERSION 이 계약의 server_version 패턴에 맞지 않는다")?;

    // ── 0. 게임 데이터 (`data/`) ────────────────────────────────────────────
    // 마이그레이션·DB 접속보다 먼저다 — DB 가 죽어 있어도 데이터 자체가 깨졌으면
    // 그 사실을 먼저 안다. 실패는 기동 거부다(I-38).
    let cwd = std::env::current_dir()?;
    let resolved_data_dir = data::resolve_data_dir(&cwd, config.data_dir_override.as_deref())
        .map_err(|error| {
            tracing::error!(%error, "데이터 디렉토리 해석 실패");
            error
        })?;
    let game_data = data::load(&resolved_data_dir, config.tick_hz).map_err(|error| {
        tracing::error!(%error, "게임 데이터 로딩 실패");
        error
    })?;
    tracing::info!(
        data_dir = %game_data.data_dir.display(),
        ship_classes = game_data.ship_classes.len(),
        spawn_points = game_data.spawn_point_count(),
        snapshot_interval_ticks = game_data.snapshot_interval_ticks,
        // SC-06/SC-07(server-db 요청) — 파일별 "데이터 파일 적재 data_file=…" 줄은
        // data::load() 안에서 이미 찍힌다(qa 요구 형식). 이 요약 줄은 그 개수를
        // 한눈에 보여준다 — "10개 전부 읽었다"를 실서버 로그만으로 확인할 수 있게.
        loaded_files = game_data.loaded_files.len(),
        "게임 데이터 로딩 완료"
    );

    // ── 1. 스키마와 월드 ────────────────────────────────────────────────────
    // 여기서만 DB 를 기다린다. 마이그레이션과 월드 대조는 **기동 조건**이다 —
    // `/readyz` 의 지연 연결(ADR-0003 §3.1 제약 1)과는 성격이 다르다.
    let pool = starfall_persistence::pool(&config.database_url)?;
    starfall_persistence::run_migrations(&pool).await?;
    let (world_basics, last_tick) =
        starfall_persistence::load_world(&pool, config.world_id, config.tick_hz, server_version)
            .await?;
    let start_tick = starfall_persistence::resume_tick(&pool, config.world_id).await?;

    // S8(qa 발견, ADR-0013 §7 2단계) — 경제 상태(인벤토리·매장지·처리 장부)를 sim의
    // 초기값으로 적재한다. **S4가 놓쳤던 자리가 여기다** — 이 호출이 없으면 재기동한
    // sim이 지속 기억을 전혀 모른 채 시작해, 첫 채굴이 DB의 실제 값과 어긋나 CAS
    // 불일치로 월드가 정지한다(K5) — 옛 command_id 재전송도 sim에서는 처음 보는
    // 것으로 통과해 DB PK(23505)에서야 걸려 정지한다(03_server_impl.md S4절 "누락"
    // 참고). 빈 월드(아직 아무도 채굴 안 함)는 세 목록이 전부 비어 있는 채로 성공한다
    // — 실패가 아니라 정상 시작 상태다(server-db 확인).
    let economic_state = starfall_persistence::load_economic_state(&pool, config.world_id)
        .await
        .map_err(|error| {
            tracing::error!(%error, "경제 상태 적재 실패");
            error
        })?;
    tracing::debug!(
        inventory_rows = economic_state.inventory.len(),
        deposit_rows = economic_state.deposits.len(),
        processed_command_ids = economic_state.processed_command_ids.len(),
        "경제 상태 적재 완료"
    );

    // T0 순서(server 검토 §1.9, ADR-0013 §7 개정): 마이그레이션 → 월드 대조 → **경제 상태
    // 적재**(S4, 위) → **여기, 역사 적재**(H2) → tick 드라이버 생성. `SignificanceRuleTable`
    // 은 S2가 `data/history/rules/`에서 이미 적재·검증한 값 — 이 슬라이스는 규칙이
    // 정확히 1개임을 `GameData::sole_rule_version`과 같은 전제로 삼는다(S2 §4.7 검산).
    let history_rule = game_data
        .significance_rules
        .values()
        .next()
        .cloned()
        .ok_or("data/history/rules/ 에 역사 규칙이 없다 — S2 가 이미 막았어야 한다")?;
    let history_boot = starfall_persistence::history::load(&pool, config.world_id, history_rule)
        .await
        .map_err(|error| {
            tracing::error!(%error, "역사 상태 적재 실패");
            error
        })?;
    tracing::debug!(
        initial_backfill = history_boot.initial_backfill.len(),
        "역사 상태 적재 완료"
    );
    tracing::info!(
        world_id = %config.world_id,
        tick_hz = config.tick_hz,
        calendar_epoch = %world_basics.calendar.epoch(),
        calendar_scale = world_basics.calendar.scale(),
        last_tick = ?last_tick,
        start_tick,
        "월드 확인 — tick 을 이어서 시작한다 (ADR-0006 §2.3)"
    );

    // DB(worlds 행)와 data/(게임 데이터)를 합쳐 시뮬레이션이 쓰는 전체 WorldConstants 를
    // 만든다. 두 출처가 이렇게 나뉜 이유는 각자의 크레이트 경계 때문이다 —
    // `starfall-persistence` 는 DB만, `data.rs`(이 바이너리)는 `data/` 만 안다.
    let world = build_world_constants(config.world_id, world_basics, &game_data)?;

    // ── 2. 관측과 제출 경로 ─────────────────────────────────────────────────
    let stats = starfall_gateway::Stats::new();
    stats.set_start_tick(start_tick);
    stats.set_data_loaded(
        &game_data.data_dir.display().to_string(),
        game_data.ship_classes.len() as u64,
        game_data.spawn_point_count() as u64,
        u64::from(game_data.snapshot_interval_ticks),
        world.max_entities_per_snapshot as u64,
        game_data.minerals.len() as u64,
        game_data.deposits.len() as u64,
        game_data.sole_rule_version().unwrap_or_default(),
        game_data.loaded_files.len() as u64,
        game_data.markets.len() as u64,
        game_data.deposit_reserves.len() as u64,
        game_data
            .sole_market()
            .map_or("", |market| market.id.as_str()),
        game_data
            .sole_market()
            .map_or("", |market| market.pricing_version.as_str()),
        game_data
            .sole_market()
            .map_or(0, |market| market.listings.len() as u64),
    );
    let (persisted, failed, last_committed, halted, ambiguous_commits) = stats.persist_handles();
    // p1-02(S7, qa SC-04 발견) — 종료 로그가 "커밋 완료"라고 주장할 값은 이 프로세스가
    // 마지막으로 실행한 tick(`stats.current_tick()`)이 아니라 **실제로 DB에 커밋된
    // 워터마크**여야 한다(둘이 다를 수 있었던 이유는 아래 `spawn_tick_thread`의
    // 종료 스윕 수정 참고). `PersistHandles`로 옮기기 전에 복제해 둔다 — 그쪽은
    // 소유권을 가져간다.
    let last_committed_for_shutdown_log = Arc::clone(&last_committed);

    let (persist_tx, persist_rx) = mpsc::channel(starfall_gateway::runtime::PERSIST_QUEUE_CAPACITY);
    let shutdown_flag = Arc::new(AtomicBool::new(false));

    // T0 LIVE 채널(bounded 256, §4 합의) — S5가 수신단을 tick 드라이버에 넘긴다(아래
    // `runtime::build`). 송신단은 역사 러너가 커밋 직후에 쓴다.
    let (history_live_tx, history_live_rx) = mpsc::channel(256);
    // `initial_backfill`은 `history_boot`가 `run_runner`로 옮겨지기 **전에** 복제해야
    // 한다(history H2 인계 — `run_runner`가 `HistoryBoot`를 소비한다). 이 목록이
    // 게이트웨이의 세션 시작 BACKFILL 씨앗이다(I-65).
    let initial_backfill = history_boot.initial_backfill.clone();

    let mut simulation = Simulation::new(world, start_tick);
    // S8 — DB에서 적재한 경제 상태를 sim의 초기값으로 주입한다. `runtime::build`가
    // `simulation`의 소유권을 가져가기 **전**이어야 한다.
    seed_economic_state(&mut simulation, economic_state);

    let tick_period = Duration::from_nanos(1_000_000_000 / u64::from(config.tick_hz));
    let (submit, tick_thread) = starfall_gateway::runtime::build(
        simulation,
        persist_tx,
        stats.clone(),
        tick_period,
        Arc::clone(&shutdown_flag),
        initial_backfill,
        history_live_rx,
    );

    // T0 커밋 알림(`02_server_ack.md` §4) — 깨우기일 뿐, 워터마크는 `worlds.last_tick`.
    // `Sender` 가 drop 되면(persistence 태스크 종료) 러너가 마지막으로 따라잡고 끝난다.
    let (commit_notify_tx, commit_notify_rx) = tokio::sync::watch::channel(None);

    let persistence = tokio::spawn(starfall_persistence::run(
        pool.clone(),
        config.world_id,
        persist_rx,
        PersistHandles {
            persisted_total: persisted,
            failed_total: failed,
            last_committed_tick: last_committed,
            halted,
            ambiguous_commits_total: ambiguous_commits,
        },
        commit_notify_tx,
    ));
    let history_handles = starfall_persistence::history::HistoryHandles::new();
    let history_runner = tokio::spawn(starfall_persistence::history::run_runner(
        pool,
        config.world_id,
        history_boot,
        commit_notify_rx,
        history_live_tx,
        history_handles,
    ));

    // ── 3. HTTP/WS 표면 ────────────────────────────────────────────────────
    let auth = starfall_gateway::DevAuth::from_secret(config.dev_auth_secret);
    if !auth.is_enabled() {
        tracing::warn!(
            "STARFALL_DEV_AUTH_SECRET 이 설정되지 않았다 — /ws 는 503 auth_not_configured 다. \
             /healthz·/readyz·/debug/stats 는 영향받지 않는다 (ADR-0008 §3)"
        );
    }
    // 의존성 핸들은 여기서 **연결하지 않는다** (ADR-0003 §3.1 제약 1).
    let probes = starfall_gateway::Probes::new(&config.database_url, &config.redis_url)?;
    let state =
        starfall_gateway::AppState::new(VERSION, probes).with_realtime(auth, stats.clone(), submit);
    let app = starfall_gateway::realtime_router(state);

    let listener = tokio::net::TcpListener::bind(config.http_addr).await?;
    let bound = listener.local_addr()?;
    tracing::info!(
        version = VERSION,
        addr = %bound,
        tick_hz = config.tick_hz,
        start_tick,
        ws = %format_args!("ws://{bound}/ws"),
        healthz = %format_args!("http://{bound}/healthz"),
        stats = %format_args!("http://{bound}/debug/stats"),
        "starfall game-server 기동 완료 — 연결을 받는다"
    );

    // ── 4. 종료 ────────────────────────────────────────────────────────────
    let shutdown_stats = stats.clone();
    let shutdown_signal_flag = Arc::clone(&shutdown_flag);
    let signal_stats = stats.clone();
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            shutdown_signal(&signal_stats).await;
            // 새 연결을 먼저 막는다. 기존 세션은 아래 스윕이 닫는다.
            shutdown_stats.set_accepting(false);
            shutdown_signal_flag.store(true, Ordering::Release);
        })
        .await?;

    // 여기 오면 tick 스레드의 종료 스윕이 모든 세션을 SERVER_SHUTDOWN 으로 닫았고,
    // 송신 태스크가 Close 프레임을 보내 연결이 전부 끝난 것이다 (I-16, AC-8d).
    tokio::task::spawn_blocking(move || tick_thread.join())
        .await?
        .map_err(|_| "tick 스레드가 패닉했다")?;
    // 송신 태스크들이 Close 프레임을 내보낼 시간을 준다. 기다리지 않으면 프로세스가 먼저
    // 죽어 **클라이언트가 close code(1001)를 보지 못한다** — 실측으로 확인한 경로다.
    // `ws_connections` 는 스윕 직후 0이 되므로 소켓 수(`live_connections`)를 본다.
    for _ in 0..60u32 {
        if stats.live_connections() == 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    // 영속화 태스크는 tick 스레드가 채널을 닫을 때 끝난다. **커밋을 기다린다** —
    // 여기서 기다리지 않으면 마지막 tick 의 SESSION_CLOSED 가 사라진다.
    persistence.await?;
    // 역사 러너는 persistence 가 끝나면(= commit_notify Sender drop) 스스로 끝난다
    // (조정 3 — 러너 정지는 서버를 멈추지 않는다. 여기서는 정상 종료 경로이므로 그냥
    // 기다린다).
    history_runner.await?;

    // S7 — `stats.current_tick()`(이 프로세스가 실행한 마지막 tick)이 아니라 실제로
    // 커밋된 워터마크를 찍는다(qa SC-04: 이벤트 없는 종료 tick은 예전엔 커밋되지
    // 않아 이 로그가 거짓이었다 — 아래 `spawn_tick_thread` 종료 스윕 수정이 지금은
    // 항상 마지막 tick을 커밋해 둘이 같아지지만, 로그 자체는 여전히 "실행한 값"이
    // 아니라 "커밋된 값"을 봐야 한다는 원칙을 지킨다).
    let committed_watermark = last_committed_for_shutdown_log.load(Ordering::Acquire);
    // p1-02(S10, qa SC-25 발견) — "정상 종료" 로그는 원인이 stdin/Ctrl-C 일 때만
    // 참이다. `stats.persist_halted()` 가 true 면 이 종료는 K5(영속화 회복 불가 정지,
    // ADR-0013 §5) 때문이지 사람이 끈 게 아니다 — 로그도 exit code 도 그 사실을
    // 반영해야 한다(AC-5(b)). 분기 자체는 `shutdown_outcome`(순수 함수, 아래 단위
    // 테스트)에 있다 — 여기서는 그 결과를 로그·반환값으로 옮기기만 한다.
    if let Err(reason) = shutdown_outcome(stats.persist_halted()) {
        tracing::error!(
            tick = committed_watermark,
            "starfall game-server 비정상 종료 — {reason}. 마지막 tick 까지 커밋 시도 완료"
        );
        return Err(reason.into());
    }
    tracing::info!(
        tick = committed_watermark,
        "starfall game-server 정상 종료 — 마지막 tick 까지 커밋 완료"
    );
    Ok(())
}

/// p1-02(S10, qa SC-25) — 종료 사유에 따른 로그 문구·exit code 분기를 순수 함수로 뺐다.
/// 실제 프로세스를 띄워 exit code 를 보는 것은 qa e2e(`tests/e2e/restart_cases.py`)의
/// 몫이다 — 여기서는 "그 분기 로직 자체가 맞는가"만 단위 테스트로 고정한다.
fn shutdown_outcome(persist_halted: bool) -> Result<(), &'static str> {
    if persist_halted {
        Err("영속화 정지(K5, ADR-0013 §5)로 서버가 스스로 종료했다 — SC-25")
    } else {
        Ok(())
    }
}

/// Ctrl-C 또는 stdin `shutdown` 한 줄 (스펙 §5.5).
///
/// # 왜 stdin 경로가 필요한가
///
/// Windows 에는 SIGTERM 이 없고, 에이전트가 백그라운드로 띄운 프로세스에 CTRL_C_EVENT 를
/// 보낼 표준 경로도 없다. 하드 킬은 graceful shutdown 경로를 **전혀 타지 않으면서**
/// outbox 부재로 인한 손실을 재현해, 결과가 "graceful shutdown 이 깨졌다"로 보인다.
/// 이 경로가 없으면 AC-8(d) 는 이 PC 에서 자동 검증이 불가능하다.
///
/// p1-02(S10, qa SC-25 발견) — 세 번째 경로로 `stats.persist_halted()`(K5, ADR-0013 §5)
/// 를 함께 기다린다. `runtime.rs`의 tick 루프도 같은 값을 보고 스스로 멈추지만, 그것만
/// 으로는 **이 프로세스**가 끝나지 않는다 — `axum::serve`는 이 함수가 반환해야 graceful
/// shutdown을 시작한다. 이 경로가 없으면(수정 전 상태) 영속화가 회복 불가로 정지한
/// 뒤에도 프로세스는 사람이 stdin `shutdown`을 보낼 때까지 계속 살아 있었다(qa 실측:
/// tick 469 정지 → tick 1779까지 계속 ACCEPT, 그 결과는 DB에 영영 남지 않는다).
async fn shutdown_signal(stats: &starfall_gateway::Stats) {
    let notify = Arc::new(Notify::new());
    spawn_stdin_listener(Arc::clone(&notify));

    tokio::select! {
        result = tokio::signal::ctrl_c() => match result {
            Ok(()) => tracing::info!("종료 신호(Ctrl-C) 수신 — graceful shutdown 시작"),
            Err(error) => {
                tracing::error!(%error, "종료 신호 처리기를 설치하지 못했다");
                // 신호를 못 받으면 stdin 만 남는다. 여기서 반환하면 즉시 종료되므로
                // stdin 을 계속 기다린다.
                notify.notified().await;
                tracing::info!("stdin 'shutdown' 수신 — graceful shutdown 시작");
            }
        },
        () = notify.notified() => {
            tracing::info!("stdin 'shutdown' 수신 — graceful shutdown 시작");
        }
        () = wait_for_persist_halted(stats) => {
            tracing::error!(
                "영속화 정지(K5, ADR-0013 §5) 감지 — 서버가 스스로 종료를 시작한다(SC-25)"
            );
        }
    }
}

/// `stats.persist_halted()`를 짧은 간격으로 폴링한다(p1-02 S10). 폴 주기(50ms)는
/// 기본 tick 주기와 같은 자릿수로 잡아, "tick 루프는 멈췄는데 프로세스만 살아 있는"
/// 창을 tick 한두 개 폭 정도로 좁힌다 — qa가 실측한 66초+ 지연(사람이 stdin으로 끌
/// 때까지)과 대비된다.
async fn wait_for_persist_halted(stats: &starfall_gateway::Stats) {
    let mut interval = tokio::time::interval(Duration::from_millis(50));
    loop {
        interval.tick().await;
        if stats.persist_halted() {
            return;
        }
    }
}

/// stdin 한 줄을 읽는 전용 블로킹 스레드.
///
/// **EOF 는 종료 신호가 아니다.** 리다이렉트 없이 띄우거나 stdin 이 `NUL` 인 환경에서
/// 즉시 종료되면 안 된다 — 그 경우 이 스레드는 조용히 끝나고 종료는 Ctrl-C 로만 가능하다.
fn spawn_stdin_listener(notify: Arc<Notify>) {
    let spawned = std::thread::Builder::new()
        .name("starfall-stdin".to_owned())
        .spawn(move || {
            let stdin = std::io::stdin();
            let mut line = String::new();
            loop {
                line.clear();
                match std::io::BufRead::read_line(&mut stdin.lock(), &mut line) {
                    Ok(0) => {
                        tracing::debug!("stdin EOF — stdin 종료 경로는 비활성이다 (Ctrl-C 만 가능)");
                        return;
                    }
                    Ok(_) => {
                        // BOM 을 걷어낸다. PowerShell 의 `RedirectStandardInput` 으로 쓰면
                        // StreamWriter 가 첫 줄 앞에 U+FEFF 를 붙여 `\u{feff}shutdown` 이 온다
                        // (실측 2026-09-19). 쓰는 쪽을 고치게 하는 것보다 여기서 관용하는 편이
                        // 낫다 — 이 경로는 QA·에이전트가 쓰는 종료 수단이기 때문이다.
                        let command = line.trim().trim_matches('\u{feff}').trim();
                        if command.eq_ignore_ascii_case("shutdown") {
                            notify.notify_one();
                            return;
                        }
                        if !command.is_empty() {
                            tracing::warn!(%command, "알 수 없는 stdin 명령 — 'shutdown' 만 받는다");
                        }
                    }
                    Err(error) => {
                        tracing::debug!(%error, "stdin 읽기 종료");
                        return;
                    }
                }
            }
        });
    if let Err(error) = spawned {
        tracing::warn!(%error, "stdin 리스너 스레드를 만들지 못했다 — Ctrl-C 만 쓸 수 있다");
    }
}

/// `worlds` 행(`WorldBasics`)과 `data/`(`GameData`)를 합쳐 시뮬레이션이 쓰는
/// `WorldConstants` 를 만든다. 두 크레이트(`persistence`·이 바이너리의 `data`)가 서로를
/// 모르므로 이 조립은 여기, 바이너리에서만 할 수 있다.
///
/// # Errors
///
/// `game_data.ship_classes` 가 비어 있으면 실패한다 — S2 의 로더가 이미 막으므로 정상
/// 경로에서는 일어나지 않는다(방어적 처리).
fn build_world_constants(
    world_id: UuidV7,
    basics: WorldBasics,
    game_data: &GameData,
) -> Result<WorldConstants, Box<dyn std::error::Error>> {
    let (_, ship_class_table) = game_data
        .ship_classes
        .iter()
        .next()
        .ok_or("data/ships/ 에 함선 클래스가 없다 — S2 가 이미 막았어야 한다")?;
    let movement = &ship_class_table.movement;
    let ship_class = ShipClassData {
        movement: ShipClassConstants {
            max_speed_mps: movement.max_speed_mps,
            main_thrust_mps2: movement.main_thrust_mps2,
            reverse_thrust_mps2: movement.reverse_thrust_mps2,
            lateral_thrust_mps2: movement.lateral_thrust_mps2,
            brake_decel_mps2: movement.brake_decel_mps2,
            assist_linear_decel_mps2: movement.assist_linear_decel_mps2,
            assist_lateral_decel_mps2: movement.assist_lateral_decel_mps2,
            turn_rate_max_deg_s: movement.turn_rate_max_deg_s,
            turn_accel_deg_s2: movement.turn_accel_deg_s2,
            turn_gain_deg_s_per_sin_half: movement.turn_gain_deg_s_per_sin_half,
            turn_deadzone_sin_half: movement.turn_deadzone_sin_half,
            roll_rate_max_deg_s: movement.roll_rate_max_deg_s,
            roll_accel_deg_s2: movement.roll_accel_deg_s2,
            auto_level_rate_deg_s: movement.auto_level_rate_deg_s,
            auto_level_deadzone_sin: movement.auto_level_deadzone_sin,
        },
        hull_radius_m: ship_class_table.geometry.hull_radius_m,
    };

    let play_area = &game_data.star_system.play_area;
    let spawn = &game_data.star_system.spawn;
    let presence = &game_data.star_system.presence;
    let sync = &game_data.sync_tuning;

    let tick_hz = basics.calendar.tick_hz();
    let rate_limit_hz = u32::try_from(sync.input.rate_limit_hz).unwrap_or(1).max(1);
    let rate_limit_per_tick_cap = rate_limit_hz.div_ceil(tick_hz.max(1)).max(1);

    Ok(WorldConstants {
        world_id: basics.world_id,
        calendar: basics.calendar,
        server_version: basics.server_version,
        star_system_id: game_data.star_system.id.clone(),
        boundary: BoundaryConstants {
            soft_boundary_radius_m: play_area.soft_boundary_radius_m,
            hard_boundary_radius_m: play_area.hard_boundary_radius_m,
            boundary_pull_mps2: play_area.boundary_pull_mps2,
        },
        spawn_points_m: spawn.points_m.clone(),
        spawn_clearance_m: spawn.clearance_m,
        spawn_max_probe_attempts: u32::try_from(spawn.max_probe_attempts).unwrap_or(1),
        spawn_radial_offset_step_m: spawn.radial_offset_step_m,
        // world_seed_le8 — world_id 의 하위 8바이트(judgement call, data.rs 모듈 문서 참고
        // — designer 데이터에 별도 시드 필드가 없다).
        spawn_world_seed: world_seed_from_world_id(world_id),
        linger_seconds: presence.linger_seconds as f64,
        reconnect_resume_window_seconds: presence.reconnect_resume_window_seconds as f64,
        ship_class_id: ship_class_table.id.clone(),
        ship_class,
        snapshot_interval_ticks: game_data.snapshot_interval_ticks,
        carry_forward_max_ticks: u32::try_from(sync.input.carry_forward_max_ticks).unwrap_or(0),
        rate_limit_per_tick_cap,
        max_entities_per_snapshot: usize::try_from(sync.snapshot.max_entities_per_snapshot)
            .unwrap_or(1),
        // p1-02(S2) — `data/minerals`·`data/world/deposits`·`data/mining/mining-rules`.
        // `game_data` 가 기동 시 §4.7 검산까지 마친 값이라 여기서는 sim 의 상수 모양으로
        // 옮겨 담기만 한다(판정 로직 없음).
        minerals: game_data
            .minerals
            .values()
            .map(|table| {
                (
                    table.id.clone(),
                    starfall_sim::MineralConstants {
                        yield_per_extraction_kg: i64::from(
                            table.extraction.yield_per_extraction_kg.get(),
                        ),
                        regen_kg: i64::from(table.regeneration.regen_kg.get()),
                        regen_interval_ticks: u64::try_from(table.regeneration.regen_interval_s)
                            .unwrap_or(0)
                            .saturating_mul(u64::from(tick_hz)),
                    },
                )
            })
            .collect(),
        // p1-03(S2) — `Deposit`(공개부, v2)에는 `mineral_id`·`initial_reserve_kg` 가 없다
        // (ADR-0015 §2). 서버 전용 `deposit_reserves`(같은 `deposit_id` 키 공간 — S2 의
        // §4.8 d01·d02 검산이 두 집합의 양방향 일치를 이미 보장한다)에서 끌어온다.
        deposits: game_data
            .deposits
            .values()
            .map(|deposit| {
                let reserve = game_data.deposit_reserves.get(deposit.id.as_str()).ok_or_else(
                    || {
                        format!(
                            "deposit_id={} 의 DEPOSIT_RESERVE 짝이 없다 — S2 검산이 이미 막았어야 한다",
                            deposit.id.as_str()
                        )
                    },
                )?;
                Ok::<_, Box<dyn std::error::Error>>((
                    deposit.id.clone(),
                    starfall_sim::DepositConstants {
                        mineral_id: reserve.mineral_id.clone(),
                        position_m: starfall_sim::world::Vec3 {
                            x: deposit.position_m[0],
                            y: deposit.position_m[1],
                            z: deposit.position_m[2],
                        },
                        radius_m: deposit.radius_m,
                        initial_reserve_kg: i64::from(reserve.initial_reserve_kg.get()),
                    },
                ))
            })
            .collect::<Result<_, _>>()?,
        mining_rules: starfall_sim::MiningRuleConstants {
            mining_range_from_surface_m: game_data
                .mining_rules
                .extraction
                .mining_range_from_surface_m,
            max_ship_speed_mps: game_data.mining_rules.extraction.max_ship_speed_mps,
            cooldown_ticks: game_data.mining_cooldown_ticks,
        },
        // p1-03(S2→S3) — `MarketTable`(공개부, 기동 검산이 광물 당 상장 정확히 하나·
        // 초 단위 값이 tick_hz 로 정수가 됨을 이미 보장했다) 을 sim 의 상수 모양으로
        // 그대로 옮긴다(판정 로직 없음, `minerals`·`deposits` 와 같은 자리).
        markets: game_data
            .markets
            .values()
            .map(|market| {
                let listings = market
                    .listings
                    .iter()
                    .map(|listing| {
                        (
                            listing.mineral_id.clone(),
                            starfall_sim::ListingConstants {
                                price: starfall_sim::PriceRule {
                                    base_price_cr_per_kg: listing.base_price_cr_per_kg.get(),
                                    band_kg: i64::from(listing.band_kg.get()),
                                    price_floor_pct: market.rules.price_floor_pct,
                                    price_drop_pct_per_band: market.rules.price_drop_pct_per_band,
                                },
                                recovery_kg: i64::from(listing.recovery_kg.get()),
                            },
                        )
                    })
                    .collect();
                (
                    market.id.clone(),
                    starfall_sim::MarketConstants {
                        position_m: starfall_sim::world::Vec3 {
                            x: market.position_m[0],
                            y: market.position_m[1],
                            z: market.position_m[2],
                        },
                        radius_m: market.radius_m,
                        dock_range_from_surface_m: market.rules.dock_range_from_surface_m,
                        max_ship_speed_mps: market.rules.max_ship_speed_mps,
                        sell_cooldown_ticks: u64::try_from(market.rules.sell_cooldown_s)
                            .unwrap_or(0)
                            .saturating_mul(u64::from(tick_hz)),
                        recovery_interval_ticks: u64::try_from(market.rules.recovery_interval_s)
                            .unwrap_or(0)
                            .saturating_mul(u64::from(tick_hz)),
                        pricing_version: market.pricing_version.clone(),
                        listings,
                    },
                )
            })
            .collect(),
    })
}

/// DB에서 적재한 경제 상태(인벤토리·매장지·지갑·시장 상태·처리 장부) 전부를
/// `Simulation`의 초기값으로 주입한다(S8, ADR-0013 §7 2단계, 지갑·시장은
/// ADR-0016 §2·§4). 호출자(`serve`)는 `runtime::build`가 `simulation` 소유권을
/// 가져가기 **전**에 이 함수를 불러야 한다.
///
/// p1-03(qa3 2026-10-07 실서버 발견) — `02_interface.md` "호출 예"가 지정한
/// `seed_wallet`/`seed_market_state` 두 루프가 S2→S3 로 코드를 옮기며 빠져 있어서,
/// 재기동한 actor 의 첫 판매가 비교 후 쓰기(K3) 불일치로 월드를 멈췄다(DB엔
/// 지갑이 있는데 sim은 모르는 채로 `expected=None`을 보냈다). 함수로 뽑아낸
/// 것 자체가 그 재발 방지 조치다 — `main()` 안에 흩어져 있으면 단위 테스트가
/// 닿지 못한다(아래 `seed_economic_state_tests`).
fn seed_economic_state(
    simulation: &mut Simulation,
    economic_state: starfall_persistence::EconomicState,
) {
    for row in economic_state.inventory {
        simulation.seed_inventory(row.actor_id, row.mineral_id, row.quantity_kg);
    }
    for row in economic_state.deposits {
        simulation.seed_deposit_state(
            row.deposit_id,
            row.remaining_kg,
            row.as_of_tick,
            row.first_extracted_tick,
        );
    }
    for row in economic_state.wallets {
        simulation.seed_wallet(row.actor_id, row.balance_cr);
    }
    for row in economic_state.market_states {
        simulation.seed_market_state(
            row.market_id,
            row.mineral_id,
            row.saturation_kg,
            row.as_of_tick,
            row.last_seller_actor_id,
            row.last_sale_quantity_kg,
        );
    }
    simulation.seed_processed_command_ids(economic_state.processed_command_ids);
}

/// `world_id` 의 하위 8바이트를 리틀엔디안 `u64` 로 — 스폰 해시의 `world_seed_le8`
/// (ADR-0010 §4). `data/` 에 별도 시드 필드가 없고 `world_id` 가 월드마다 고정·고유하므로
/// 이것으로 충분하다(judgement call).
fn world_seed_from_world_id(world_id: UuidV7) -> u64 {
    let bytes = world_id.get().into_bytes();
    let mut low8 = [0u8; 8];
    low8.copy_from_slice(&bytes[8..16]);
    u64::from_le_bytes(low8)
}

#[cfg(test)]
mod seed_economic_state_tests {
    use std::collections::BTreeMap;

    use starfall_contracts::primitives::{DataId, GameCalendar, GameTime};
    use starfall_persistence::{
        DepositRow, EconomicState, InventoryRow, MarketStateRow, WalletRow,
    };
    use starfall_sim::world::{BoundaryConstants, ShipClassConstants, Vec3};
    use starfall_sim::{
        DepositConstants, ListingConstants, MarketConstants, MineralConstants, MiningRuleConstants,
        PriceRule, ServerMessage, ShipClassData, Simulation, Submission, WorldConstants,
    };

    use super::{UuidV7, VERSION, seed_economic_state};

    fn id(n: u64) -> UuidV7 {
        let text = format!(
            "01a0b1c2-0003-7{:03x}-8{:03x}-{:012x}",
            n & 0xfff,
            (n >> 12) & 0xfff,
            n
        );
        UuidV7::parse(&text).expect("정규 UUIDv7 이어야 한다")
    }

    /// `seed_economic_state_tests` 전용 최소 월드 — 함선 하나, 매장지 하나(시작
    /// 전부터 드러난 것으로 seed — I-86 재노출과 무관하게 바로 보려고), 시장
    /// 하나(상장 하나, `test-mineral`) — `MARKET_STATE` 의 `listings` 내용으로
    /// `seed_market_state`가 실제로 불렸는지까지 직접 확인할 수 있게 한다.
    fn minimal_world() -> WorldConstants {
        const SPAWN_POINT_M: [f64; 3] = [0.0, 0.0, 0.0];
        let mineral_id = DataId::parse("test-mineral").unwrap();
        let mut minerals = BTreeMap::new();
        minerals.insert(
            mineral_id.clone(),
            MineralConstants {
                yield_per_extraction_kg: 25,
                regen_kg: 5,
                regen_interval_ticks: 10,
            },
        );
        let mut deposits = BTreeMap::new();
        deposits.insert(
            DataId::parse("test-deposit").unwrap(),
            DepositConstants {
                mineral_id: mineral_id.clone(),
                position_m: Vec3::new(SPAWN_POINT_M[0], SPAWN_POINT_M[1], SPAWN_POINT_M[2]),
                radius_m: 10.0,
                initial_reserve_kg: 1_000,
            },
        );
        let mut listings = BTreeMap::new();
        listings.insert(
            mineral_id,
            ListingConstants {
                price: PriceRule {
                    base_price_cr_per_kg: 10,
                    band_kg: 2_000,
                    price_floor_pct: 30,
                    price_drop_pct_per_band: 10,
                },
                recovery_kg: 1_000,
            },
        );
        let mut markets = BTreeMap::new();
        markets.insert(
            DataId::parse("test-market").unwrap(),
            MarketConstants {
                position_m: Vec3::new(SPAWN_POINT_M[0], SPAWN_POINT_M[1], SPAWN_POINT_M[2]),
                radius_m: 10.0,
                dock_range_from_surface_m: 150.0,
                max_ship_speed_mps: 10.0,
                sell_cooldown_ticks: 20,
                recovery_interval_ticks: 1_200,
                pricing_version: starfall_contracts::primitives::RuleVersion::parse(
                    "test-market@1",
                )
                .unwrap(),
                listings,
            },
        );

        WorldConstants {
            world_id: id(1),
            calendar: GameCalendar::new(20, GameTime::parse("3800-01-01T00:00:00Z").unwrap(), 60)
                .unwrap(),
            server_version: starfall_contracts::primitives::ServerVersion::parse(VERSION)
                .unwrap_or_else(|| {
                    starfall_contracts::primitives::ServerVersion::parse("0.1.0").unwrap()
                }),
            star_system_id: DataId::parse("test-system").unwrap(),
            boundary: BoundaryConstants {
                soft_boundary_radius_m: 10_000.0,
                hard_boundary_radius_m: 12_000.0,
                boundary_pull_mps2: 25.0,
            },
            spawn_points_m: vec![SPAWN_POINT_M],
            spawn_clearance_m: 150.0,
            spawn_max_probe_attempts: 12,
            spawn_radial_offset_step_m: 100.0,
            spawn_world_seed: 1,
            linger_seconds: 30.0,
            reconnect_resume_window_seconds: 30.0,
            ship_class_id: DataId::parse("scout-s01").unwrap(),
            ship_class: ShipClassData {
                movement: ShipClassConstants {
                    max_speed_mps: 140.0,
                    main_thrust_mps2: 35.0,
                    reverse_thrust_mps2: 18.0,
                    lateral_thrust_mps2: 18.0,
                    brake_decel_mps2: 50.0,
                    assist_linear_decel_mps2: 7.0,
                    assist_lateral_decel_mps2: 22.0,
                    turn_rate_max_deg_s: 75.0,
                    turn_accel_deg_s2: 220.0,
                    turn_gain_deg_s_per_sin_half: 290.0,
                    turn_deadzone_sin_half: 0.0009,
                    roll_rate_max_deg_s: 90.0,
                    roll_accel_deg_s2: 300.0,
                    auto_level_rate_deg_s: 40.0,
                    auto_level_deadzone_sin: 0.002,
                },
                hull_radius_m: 12.0,
            },
            snapshot_interval_ticks: 2,
            carry_forward_max_ticks: 10_000,
            rate_limit_per_tick_cap: 5,
            max_entities_per_snapshot: 64,
            minerals,
            deposits,
            mining_rules: MiningRuleConstants {
                mining_range_from_surface_m: 150.0,
                max_ship_speed_mps: 10.0,
                cooldown_ticks: 10,
            },
            markets,
        }
    }

    /// qa3(2026-10-07) 실서버 발견 재발 방지 — DB에서 읽은 지갑·시장 상태 행이
    /// `seed_economic_state`를 거쳐 세션 시작 `WALLET_STATE`/`MARKET_STATE` 로
    /// 그대로 나오는지 확인한다. 이 테스트가 처음 잡아낸 버그: `main()` 안에
    /// 인벤토리·매장지 두 루프만 있고 지갑·시장 상태 두 루프가 없었다(기동
    /// 경로를 실제로 타는 테스트가 없어서 qa3 가 실서버 재기동 봇으로 먼저
    /// 찾았다 — 이 테스트는 그 재발을 막는다).
    #[test]
    fn seeded_wallet_and_market_state_appear_at_session_start() {
        let actor_id = id(2);
        let market_id = DataId::parse("test-market").unwrap();
        let mineral_id = DataId::parse("test-mineral").unwrap();
        let last_seller = id(3);

        let economic_state = EconomicState {
            inventory: vec![InventoryRow {
                actor_id,
                mineral_id: mineral_id.clone(),
                quantity_kg: 10,
            }],
            // 드러난 매장지로 적재한다(실서버 기동과 같은 경로) — 그래야
            // `MARKET_STATE.listings` 에 `test-mineral` 이 보여서 `seed_market_state`
            // 가 실제로 불렸는지를 내용으로 직접 확인할 수 있다(I-86).
            deposits: vec![DepositRow {
                deposit_id: DataId::parse("test-deposit").unwrap(),
                remaining_kg: 1_000,
                as_of_tick: 0,
                first_extracted_tick: 0,
            }],
            processed_command_ids: Vec::new(),
            wallets: vec![WalletRow {
                actor_id,
                balance_cr: 2_000,
            }],
            market_states: vec![MarketStateRow {
                market_id: market_id.clone(),
                mineral_id: mineral_id.clone(),
                saturation_kg: 500,
                as_of_tick: 0,
                last_seller_actor_id: last_seller,
                last_sale_quantity_kg: 300,
            }],
        };

        let mut simulation = Simulation::new(minimal_world(), 0);
        seed_economic_state(&mut simulation, economic_state);

        let session_id = id(10);
        let mut next_id = 1_000u64;
        let outcome = simulation.step(
            vec![Submission::OpenSession {
                seq: 0,
                session_id,
                actor_id,
            }],
            &mut (|| {
                next_id += 1;
                id(next_id)
            }),
        );

        let wallet = outcome
            .outbound
            .iter()
            .find_map(|out| match &out.message {
                ServerMessage::WalletState(message) if out.session_id == session_id => {
                    Some(message.as_ref())
                }
                _ => None,
            })
            .expect("WALLET_STATE 가 세션 시작에 있어야 한다");
        assert_eq!(
            wallet.payload.balance_cr.get(),
            2_000,
            "seed_wallet 이 호출되지 않으면 여기가 0이 된다(qa3 재현 1과 같은 증상)"
        );

        let market_state = outcome
            .outbound
            .iter()
            .find_map(|out| match &out.message {
                ServerMessage::MarketState(message) if out.session_id == session_id => {
                    Some(message.as_ref())
                }
                _ => None,
            })
            .expect("MARKET_STATE 가 세션 시작에 있어야 한다");
        let listing = market_state
            .payload
            .listings
            .iter()
            .find(|listing| listing.mineral_id == mineral_id)
            .expect("test-mineral 상장이 있어야 한다(드러난 매장지 + 상장)");
        assert_eq!(
            listing.saturation_kg.get(),
            500,
            "seed_market_state 가 호출되지 않으면 포화가 0 이 된다(qa3 재현 2 — 재기동 뒤 \
             첫 판매가 expected=None 으로 DB 비교 후 쓰기 불일치를 내는 것과 같은 증상)"
        );
        let last_sale = listing
            .last_sale
            .as_ref()
            .expect("last_seller_actor_id 를 seed 했으므로 LastSale 이 있어야 한다");
        assert_eq!(last_sale.actor_id, last_seller);
        assert_eq!(last_sale.quantity_kg.get(), 300);
    }
}

#[cfg(test)]
mod shutdown_outcome_tests {
    use super::shutdown_outcome;

    #[test]
    fn clean_shutdown_is_ok() {
        assert_eq!(shutdown_outcome(false), Ok(()));
    }

    #[test]
    fn persist_halted_is_a_non_zero_exit_with_a_reason() {
        assert!(
            shutdown_outcome(true).is_err(),
            "정지 원인은 Err 여야 exit code 가 0이 아니게 된다"
        );
    }
}
