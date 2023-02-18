use crate::{Settings, SETTINGS};
use clap::{Parser, ValueEnum};
use std::path::PathBuf;

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

	/// Path to the configuration file
	#[arg(short, long, default_value_os_t = PathBuf::from("config.toml"))]
	pub config: PathBuf,
}

impl Args {
	pub fn init_settings(&self) -> anyhow::Result<()> {
		if !self.config.exists() {
			let settings = Settings::default();
			std::fs::write(&self.config, toml::to_string(&settings)?)?;
		}
		let settings = toml::from_str::<Settings>(&std::fs::read_to_string(&self.config)?)?.rebuild_cache()?;
		SETTINGS
			.set(settings)
			.map_err(|_| anyhow::anyhow!("SETTINGS can only be loaded once"))?;
		Ok(())
	}
}
