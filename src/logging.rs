use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::EnvFilter;

pub fn init(verbose: bool) -> Option<WorkerGuard> {
    let dir = crate::config::cache_dir()?;
    std::fs::create_dir_all(&dir).ok()?;
    let appender = tracing_appender::rolling::never(&dir, "tesoro.log");
    let (writer, guard) = tracing_appender::non_blocking(appender);
    let default = if verbose { "tesoro=debug" } else { "tesoro=info" };
    let filter = EnvFilter::try_from_env("TESORO_LOG").unwrap_or_else(|_| EnvFilter::new(default));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(writer)
        .with_ansi(false)
        .try_init();
    Some(guard)
}
