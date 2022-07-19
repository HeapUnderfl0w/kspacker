use std::{
    fmt,
    fs::{self, File, FileType},
    path::{Path, PathBuf},
};

use anyhow::Context;

use crate::structs::{FileAction, PathStem, TextureType, Version};

#[derive(Debug, Clone)]
pub struct Resolver {
    install: PathBuf,
    custom:  PathBuf,
}

impl Resolver {
    pub fn new(install: impl AsRef<Path>) -> Self {
        let custom = Path::new(&dirs::data_local_dir().unwrap())
            .join("Keysight")
            .join("Saved");
        Resolver {
            install: install.as_ref().to_path_buf(),
            custom,
        }
    }

    pub fn identify(&self) -> anyhow::Result<Version> {
        let path = self
            .install
            .join("Keysight")
            .join("Default presets")
            .join("Standard")
            .join("Plain (default).json");
        if !path.exists() {
            anyhow::bail!("unable to find keysight default presets, please recheck your path");
        }

        #[derive(serde::Deserialize)]
        struct KSPresetVersion {
            #[serde(rename = "versionForUpdatePurposes")]
            pub version_for_update_purposes: u16,
        }

        let data: KSPresetVersion = serde_json::from_reader(
            File::open(path).context("failed to open default preset file")?,
        )
        .context("failed to load default preset file")?;
        Version::from_int(data.version_for_update_purposes)
            .ok_or_else(|| anyhow::anyhow!("unable to parse keysight version"))
    }

    pub fn list_presets(&self) -> anyhow::Result<Vec<String>> {
        let path = self.custom.join("Presets");

        let iter = fs::read_dir(path)
            .context("failed to read preset directory")?
            .filter_map(|v| match v {
                Ok(entry) => match entry.file_type() {
                    Ok(typ) if typ.is_file() => Some(entry),
                    _ => None,
                },
                Err(e) => {
                    warn!(?e, "failed to identify preset");
                    None
                },
            });

        let mut out = Vec::new();
        for file in iter {
            let v: String = file
                .file_name()
                .to_string_lossy()
                .split('.')
                .rev()
                .skip(1)
                .collect();
            out.push(v);
        }

        Ok(out)
    }

    pub fn preset_path(&self, name: &str) -> PathBuf {
        self.custom.join("Presets").join(format!("{}.json", name))
    }

    pub fn get_preset(&self, name: &str) -> Option<PathBuf> {
        let v = self.preset_path(name);
        if v.exists() {
            Some(v)
        } else {
            None
        }
    }

    pub fn test_file(&self, stem: &PathStem, name: &str) -> bool {
        matches!(self.action_for_file(stem, name), FileAction::Pack { .. })
    }

    pub fn action_for_file(&self, stem: &PathStem, name: &str) -> FileAction {
        fn test_exts(p: &PathBuf, n: &str) -> Option<PathBuf> {
            for ext in ["png", "jpg", "jpeg"] {
                let current = p.join(format!("{}.{}", n, ext));
                if current.exists() {
                    return Some(current);
                }
            }
            return None;
        }

        // default
        let path = test_exts(&stem.default, name);
        if let Some(_) = path {
            return FileAction::DontPack;
        }

        let path = test_exts(&stem.custom, name);
        if let Some(found) = path {
            return FileAction::Pack {
                path:   found.display().to_string(),
                random: false,
            };
        }

        let path = test_exts(&stem.random, name);
        if let Some(found) = path {
            return FileAction::Pack {
                path:   found.display().to_string(),
                random: true,
            };
        }

        FileAction::NotFound
    }

    pub fn stem_from_name(&self, name: TextureType) -> PathStem {
        let make_stem = |n: &'static str| -> PathStem {
            PathStem {
                default: self
                    .install
                    .join("Keysight")
                    .join("Default textures")
                    .join(n),
                custom:  self.custom.join("Textures").join(n),
                random:  self.custom.join("Textures (randomizer enabled)").join(n),
            }
        };

        match name {
            TextureType::Diffuse | TextureType::Emissive => make_stem("Colour"),
            TextureType::WorldStencil | TextureType::Mask => make_stem("Mask"),
            TextureType::Metalness => make_stem("Metal"),
            TextureType::Normal => make_stem("Normal"),
            TextureType::Roughness => make_stem("Roughness"),
            TextureType::Shape => make_stem("Particle stencil"),
            TextureType::Specular => make_stem("Specular"),
            TextureType::Stencil => make_stem("Pulse stencil"),
        }
    }
}
