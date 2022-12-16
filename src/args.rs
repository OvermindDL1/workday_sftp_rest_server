use clap::{Parser, ValueEnum};

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum LogFormat {
    Compact,
    Pretty,
    JSON,
    // None,
}

#[derive(Parser, Debug)]
//#[clap(author, version, about, long_about = None)]
#[clap(long_about = None)]
pub struct Args {
    /// Verbosity logging level
    #[arg(short, long, action = clap::ArgAction::Count, default_value_t = 2)]
    pub verbosity: u8,

    /// Log format type on stderr.
    #[arg(value_enum, long, default_value_t = LogFormat::Compact)]
    pub log_format: LogFormat,
}
