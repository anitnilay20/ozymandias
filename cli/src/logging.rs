use tracing_subscriber::{EnvFilter, FmtSubscriber};

/// Set up logging according to verbosity level
pub fn setup_logging(verbosity: u8) -> Result<(), Box<dyn std::error::Error>> {
    let level = match verbosity {
        0 => "warn",
        1 => "info",
        2 => "debug",
        _ => "trace",
    };

    let subscriber = FmtSubscriber::builder()
        .with_env_filter(EnvFilter::new(level))
        .finish();

    tracing::subscriber::set_global_default(subscriber)?;
    Ok(())
}