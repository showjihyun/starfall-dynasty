//! STARFALL 게임 서버 조립 바이너리.
//!
//! 이 바이너리가 하는 일은 조립뿐이다: 설정 로드 → tracing 초기화 → 상태 구성 →
//! 라우터 기동 → 종료 신호 대기. 게임 규칙은 여기에 없다.
//!
//! 실행 (워크스페이스 루트는 `server/` 다 — ADR-0001 §4):
//!
//! ```text
//! cd server && cargo run -p starfall-game-server
//! ```

mod config;

use std::process::ExitCode;

use config::{Config, LogFormat};
use tracing_subscriber::EnvFilter;

/// 서버 빌드 버전. `/healthz` 응답의 `version`.
const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() -> ExitCode {
    // 설정 로드는 tracing 초기화보다 먼저다(로그 형식이 설정에서 온다).
    // 이 단계의 실패는 로거 없이 stderr 로 알린다.
    let config = match Config::from_env() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("설정 로드 실패: {error}");
            return ExitCode::FAILURE;
        }
    };

    init_tracing(config.log_format);

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

fn init_tracing(format: LogFormat) {
    // 기본 레벨은 info. RUST_LOG 로 덮을 수 있다.
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,starfall_gateway=debug"));
    let builder = tracing_subscriber::fmt().with_env_filter(filter);

    match format {
        LogFormat::Pretty => builder.init(),
        LogFormat::Json => builder.json().init(),
    }
}

async fn serve(config: Config) -> Result<(), Box<dyn std::error::Error>> {
    // 의존성 핸들은 여기서 **연결하지 않는다**. 접속 문자열 해석만 한다.
    // PostgreSQL·Redis 가 꺼져 있어도 서버는 기동하고, /readyz 가 503 으로 알린다
    // (ADR-0003 §3.1 제약 1).
    let probes = starfall_gateway::Probes::new(&config.database_url, &config.redis_url)?;

    let state = starfall_gateway::AppState::new(VERSION, probes);
    let app = starfall_gateway::router(state);

    let listener = tokio::net::TcpListener::bind(config.http_addr).await?;
    // 바인딩 실패 시 OS 가 고른 포트를 쓸 수도 있으므로 실제 주소를 로그에 남긴다.
    // SC-04 가 이 줄을 증거로 쓴다.
    let bound = listener.local_addr()?;
    tracing::info!(
        version = VERSION,
        addr = %bound,
        healthz = %format_args!("http://{bound}/healthz"),
        "starfall game-server 기동 — HTTP 바인딩 완료"
    );

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    tracing::info!("starfall game-server 정상 종료");
    Ok(())
}

async fn shutdown_signal() {
    match tokio::signal::ctrl_c().await {
        Ok(()) => tracing::info!("종료 신호(Ctrl-C) 수신 — graceful shutdown 시작"),
        Err(error) => tracing::error!(%error, "종료 신호 처리기를 설치하지 못했다"),
    }
}
