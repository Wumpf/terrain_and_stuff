use std::path::Path;

use crate::atmosphere::{AtmosphereParams, SunAngles};

/// Config file that allows to store all relevant application state into a single file.
///
/// Kept intentionally simple: extending this is meant to be explicit with minimal use of stringly typed dictionaries.
/// Dynamicism & extensibility is for large projects & big teams. We're doing neither here! ;)
#[derive(Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct Config {
    pub sun_angles: SunAngles,
    pub atmosphere_params: AtmosphereParams,
}

impl Config {
    pub fn save_to_ron_file(&self, path: impl AsRef<Path>) -> anyhow::Result<()> {
        let contents = ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::new())?;
        std::fs::write(path, contents)?;
        Ok(())
    }

    pub fn save_to_ron_file_or_log_error(&self, path: impl AsRef<Path>) {
        if let Err(err) = self.save_to_ron_file(path) {
            log::error!("Failed to save config.ron: {}", err);
        }
    }

    pub fn load_from_ron_file(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let file_contents = std::fs::read_to_string(path)?;
        Ok(ron::de::from_str(&file_contents)?)
    }

    pub fn load_from_ron_file_or_default_and_log_error(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref();
        Self::load_from_ron_file(path).unwrap_or_else(|err| {
            log::warn!("Failed to load config.ron: {}", err);
            let default = Self::default();
            default.save_to_ron_file_or_log_error(path);
            default
        })
    }
}
