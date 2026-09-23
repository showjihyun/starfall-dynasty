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
    );
    let (persisted, failed, last_committed) = stats.persist_handles();

    let (persist_tx, persist_rx) = mpsc::channel(starfall_gateway::runtime::PERSIST_QUEUE_CAPACITY);
    let shutdown_flag = Arc::new(AtomicBool::new(false));

    let simulation = Simulation::new(world, start_tick);
    let tick_period = Duration::from_nanos(1_000_000_000 / u64::from(config.tick_hz));
    let (submit, tick_thread) = starfall_gateway::runtime::build(
        simulation,
        persist_tx,
        stats.clone(),
        tick_period,
        Arc::clone(&shutdown_flag),
    );

    let persistence = tokio::spawn(starfall_persistence::run(
        pool,
        config.world_id,
        persist_rx,
        PersistHandles {
            persisted_total: persisted,
            failed_total: failed,
            last_committed_tick: last_committed,
        },
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
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            shutdown_signal().await;
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

    tracing::info!(
        tick = stats.current_tick(),
        "starfall game-server 정상 종료 — 마지막 tick 까지 커밋 완료"
    );
    Ok(())
}

/// Ctrl-C 또는 stdin `shutdown` 한 줄 (스펙 §5.5).
///
/// # 왜 stdin 경로가 필요한가
///
/// Windows 에는 SIGTERM 이 없고, 에이전트가 백그라운드로 띄운 프로세스에 CTRL_C_EVENT 를
/// 보낼 표준 경로도 없다. 하드 킬은 graceful shutdown 경로를 **전혀 타지 않으면서**
/// outbox 부재로 인한 손실을 재현해, 결과가 "graceful shutdown 이 깨졌다"로 보인다.
/// 이 경로가 없으면 AC-8(d) 는 이 PC 에서 자동 검증이 불가능하다.
async fn shutdown_signal() {
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
    })
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
