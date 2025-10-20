mod config;

use tracing::{error, info};
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{EnvFilter, fmt::time::ChronoLocal, fmt::writer::MakeWriterExt};

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
        EnvFilter::from_default_env().add_directive(tracing::Level::TRACE.into())
    } else {
        // 릴리즈 빌드일 때는 INFO 이상만 출력
        EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into())
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

    info!("Server will listen on: {}", config.server_addr());
    info!("Database URL: {}", config.database_url);
    info!("GitHub token: {:?}", config.github_token);
    info!("Git SSH key path: {:?}", config.git_ssh_key_path);
    info!("Git SSH key passphrase: {:?}", config.git_ssh_key_passphrase);
    info!("Git use SSH agent: {}", config.git_use_ssh_agent);
    info!("Jenkins URL: {}", config.jenkins_url);
    info!("Jenkins user: {}", config.jenkins_user);
    info!("Jenkins token: {}", config.jenkins_token);
    info!("Poll interval: {}s", config.poll_interval_seconds);

    // TODO: DB 연결
    // TODO: 스케줄러 시작
    // TODO: WebSocket 서버 시작

    Ok(())
}
