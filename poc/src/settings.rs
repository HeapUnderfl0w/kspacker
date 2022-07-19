use std::{
    fs::{self, File},
    path::{Path, PathBuf},
};

use anyhow::Context;

#[derive(Debug, Clone, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Settings {
    pub keysight_path: String,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            keysight_path: String::new(),
        }
    }
}

impl Settings {
    fn ensure_dir(p: &Path) -> anyhow::Result<()> {
        if !p.exists() {
            fs::create_dir_all(p).context("failed to ensure dir")?;
        }
        Ok(())
    }

    pub fn load() -> anyhow::Result<Self> {
        let file = dirs::data_local_dir()
            .expect("expected localappdata to exist")
            .join("ks-packer")
            .join("settings.json");

        if file.exists() {
            let data = fs::read_to_string(file)
                .context("failed to read settings file even when it exists")?;
            serde_json::from_str(&data).context("failed to parse settings")
        } else {
            return Ok(Settings::default());
        }
    }

    pub fn store(&self) -> anyhow::Result<()> {
        let root = dirs::data_local_dir()
            .expect("expected localappdata to exist")
            .join("ks-packer");
        Self::ensure_dir(&root)?;
        let file = root.join("settings.json");

        let f = File::create(file).context("failed to open settings for writing")?;
        serde_json::to_writer(f, &self).context("failed to serialize settings")?;

        Ok(())
    }
}
