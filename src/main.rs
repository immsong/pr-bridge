mod config;
mod db;
mod git;
mod scheduler;
mod ws;
use tracing::{debug, error, info};
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{EnvFilter, fmt::time::ChronoLocal, fmt::writer::MakeWriterExt};

use crate::{git::GitClient, scheduler::scheduler::Scheduler};

/// 로그 파일 쓰기 작업을 위한 백그라운드 워커 가드 전역 저장소
///
/// non-blocking 로그 작성기의 생명주기 관리 및 프로그램 종료 시까지 유지
static mut GUARD: Option<tracing_appender::non_blocking::WorkerGuard> = None;

/// 로깅 시스템 초기화 및 설정
///
/// 콘솔 및 파일 동시 출력, 로컬 시간대 적용
/// 최대 30일 로그 파일 저장
///
/// # Arguments
///
/// * `display_terminal_log` - 콘솔 로그 출력 여부
fn setup_logger(display_terminal_log: bool) {
    // 날짜별 로그 파일 설정
    let file_appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .max_log_files(30)
        .filename_prefix("pr-bridge")
        .filename_suffix("log")
        .build("./logs")
        .expect("failed to create log file");

    let filter = if cfg!(debug_assertions) {
        // 디버그 빌드일 때는 모든 레벨 출력
        EnvFilter::from_default_env()
            .add_directive(tracing::Level::TRACE.into())
            .add_directive("tokio_tungstenite=info".parse().unwrap())
            .add_directive("tungstenite=info".parse().unwrap())
    } else {
        // 릴리즈 빌드일 때는 INFO 이상만 출력
        EnvFilter::from_default_env()
            .add_directive(tracing::Level::INFO.into())
            .add_directive("tokio_tungstenite=info".parse().unwrap())
            .add_directive("tungstenite=info".parse().unwrap())
    };

    // non-blocking writer 설정
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    // guard를 전역 변수에 저장
    unsafe {
        GUARD = Some(guard);
    }

    // 로컬 시간 포맷 설정
    let timer = ChronoLocal::new("%Y-%m-%d %H:%M:%S%.3f".to_string());

    if display_terminal_log {
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_writer(non_blocking.and(std::io::stdout))
            .with_ansi(false)
            .with_thread_ids(true)
            .with_thread_names(true)
            .with_file(true)
            .with_line_number(true)
            .with_target(false)
            .with_timer(timer)
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_writer(non_blocking)
            .with_ansi(false)
            .with_thread_ids(true)
            .with_thread_names(true)
            .with_file(true)
            .with_line_number(true)
            .with_target(false)
            .with_timer(timer)
            .init();
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 로깅 초기화
    setup_logger(cfg!(debug_assertions));
    info!("Starting PR Bridge Server...");

    // 설정 로드
    let config = match config::Config::from_env() {
        Ok(cfg) => {
            info!("Configuration loaded successfully");
            cfg
        }
        Err(e) => {
            error!("Failed to load configuration: {}", e);
            return Err(e);
        }
    };

    // DB 연결
    let pool = db::Pool::create_pool(&config.database_url).await?;
    let migration_result = db::Migration::run(&pool).await;
    match migration_result {
        Ok(()) => {
            info!("Migrations completed successfully");
        }
        Err(e) => {
            error!("Failed to run migrations: {}", e);
            return Err(e);
        }
    }

    let mut github_api_poll_interval = 300;
    let mut sync_refs_interval = 180;

    let system_settings = db::Queries::get_system_settings(&pool).await?;
    debug!("System settings: {:?}", system_settings);
    if system_settings.get("github_api_poll_interval").is_none() {
        error!("github_api_poll_interval is not set");
    } else {
        github_api_poll_interval = system_settings
            .get("github_api_poll_interval")
            .unwrap()
            .parse()
            .unwrap();
    }

    if system_settings.get("sync_refs_interval").is_none() {
        error!("sync_refs_interval is not set");
    } else {
        sync_refs_interval = system_settings
            .get("sync_refs_interval")
            .unwrap()
            .parse()
            .unwrap();
    }

    let git_client = GitClient::new(config.clone());

    // 스케줄러 시작
    let scheduler = Scheduler::new(pool.clone(), github_api_poll_interval, sync_refs_interval, git_client);
    scheduler.start().await;

    // WebSocket 서버 시작
    let ws_server = ws::ws_server::WsServer::new(&pool);
    ws_server.run(config.server_port).await;

    info!("Server started. Press Ctrl+C to stop.");

    // Ctrl+C 대기
    tokio::signal::ctrl_c().await?;
    info!("Shutting down...");

    Ok(())
}
