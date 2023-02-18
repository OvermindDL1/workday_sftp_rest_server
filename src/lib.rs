extern crate core;

use once_cell::sync::OnceCell;

pub mod args;
pub mod canvas;
pub mod configuration;
pub mod csv_row_definitions;
pub mod logging;
pub mod utils;

#[derive(Default, Debug, serde::Deserialize, serde::Serialize)]
pub struct Settings {
	pub canvas: canvas::Settings,
	pub email: utils::email::Settings,
}

impl Settings {
	pub fn rebuild_cache(self) -> anyhow::Result<Self> {
		Ok(Self {
			canvas: self.canvas.rebuild_cache()?,
			email: self.email.rebuild_cache()?,
		})
	}
}

static SETTINGS: OnceCell<Settings> = OnceCell::new();
