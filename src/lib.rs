extern crate core;

use once_cell::sync::OnceCell;

pub mod args;
pub mod banner;
pub mod canvas;
pub mod configuration;
pub mod csv_row_definitions;
pub mod logging;
pub mod papercut;
pub mod utils;

#[derive(Default, Debug, serde::Deserialize, serde::Serialize)]
pub struct Settings {
	pub banner: banner::Settings,
	pub canvas: canvas::Settings,
	pub email: utils::email::Settings,
	pub papercut: papercut::Settings,
}

impl Settings {
	pub fn rebuild_cache(self) -> anyhow::Result<Self> {
		Ok(Self {
			banner: self.banner.rebuild_cache()?,
			canvas: self.canvas.rebuild_cache()?,
			email: self.email.rebuild_cache()?,
			papercut: self.papercut.rebuild_cache()?,
		})
	}
}

static SETTINGS: OnceCell<Settings> = OnceCell::new();
