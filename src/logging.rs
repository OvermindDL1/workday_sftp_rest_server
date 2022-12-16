use crate::args::{Args, LogFormat};
use tracing::Level;
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::FmtSubscriber;

pub fn setup_logger(args: &Args) -> anyhow::Result<()> {
    let builder = FmtSubscriber::builder()
        .with_writer(std::io::stderr)
        .with_max_level(match args.verbosity {
            0 => Level::ERROR,
            1 => Level::WARN,
            2 => Level::INFO,
            3 => Level::DEBUG,
            _ => Level::TRACE,
        })
        .with_level(true)
        //.with_span_events(FmtSpan::FULL)
        .with_span_events(match args.verbosity {
            0..=4 => FmtSpan::NEW | FmtSpan::CLOSE,
            _ => FmtSpan::FULL,
        })
        .with_target(true)
        .with_ansi(true);
    match args.log_format {
        LogFormat::Compact => builder.compact().finish().try_init()?,
        LogFormat::Pretty => builder.pretty().finish().try_init()?,
        LogFormat::JSON => builder.json().finish().try_init()?,
        // LogFormat::None => builder.finish().try_init()?,
    }
    tracing::info!("Initialized logging system");
    Ok(())
}
