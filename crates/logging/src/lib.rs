//! Console logging setup built on `tracing-subscriber`.

use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

/// The most verbose level that is logged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl From<LogLevel> for LevelFilter {
    fn from(level: LogLevel) -> Self {
        match level {
            LogLevel::Trace => LevelFilter::TRACE,
            LogLevel::Debug => LevelFilter::DEBUG,
            LogLevel::Info => LevelFilter::INFO,
            LogLevel::Warn => LevelFilter::WARN,
            LogLevel::Error => LevelFilter::ERROR,
        }
    }
}

/// Installs a global subscriber that prints events at `level` and above to stdout.
///
/// ANSI colors are on in debug builds only. Fails if a global subscriber is already installed.
pub fn init_tracing(level: LogLevel) -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_ansi(cfg!(debug_assertions)))
        .with(LevelFilter::from(level))
        .try_init()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_map_to_filters() {
        assert_eq!(LevelFilter::from(LogLevel::Trace), LevelFilter::TRACE);
        assert_eq!(LevelFilter::from(LogLevel::Error), LevelFilter::ERROR);
    }

    #[test]
    fn a_second_init_is_an_error_instead_of_a_panic() {
        // Whichever call installs the subscriber first wins; the later one must report it.
        let _ = init_tracing(LogLevel::Info);
        assert!(init_tracing(LogLevel::Debug).is_err());
    }
}
