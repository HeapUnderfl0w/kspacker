use std::{fs::File, path::Path};

use anyhow::Context;
use chrono::{DateTime, Utc};

pub mod helpers;
pub(self) mod ks_preset;
pub mod packer;
pub mod unpacker;

pub type Version = u32;

pub fn get_ks_version(root: impl AsRef<Path>) -> anyhow::Result<Version> {
    let path = helpers::root_preset_dir(root).join("Plain (default).json");
    if !path.exists() {
        anyhow::bail!("unable to find keysight default presets, please recheck your path");
    }

    #[derive(serde::Deserialize)]
    struct KSPresetVersion {
        #[serde(rename = "versionForUpdatePurposes")]
        pub version_for_update_purposes: Version,
    }

    let data: KSPresetVersion =
        serde_json::from_reader(File::open(path).context("failed to open default preset file")?)
            .context("failed to load default preset file")?;

    Ok(data.version_for_update_purposes)
}

/// The metadata for the zip file
#[derive(Debug, Clone, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PackMetaData {
    pub name:           String,
    pub author:         String,
    pub description:    String,
    pub packed:         DateTime<Utc>,
    pub preset_version: Version,
    pub target_version: Version,

    pub assets: Vec<MetaEntry>,
}

/// A single asset packed
#[derive(Debug, Clone, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MetaEntry {
    pub hash:              String,
    pub name:              String,
    pub extension:         String,
    pub texture_type:      TextureType, // TODO: Add correct type
    pub source_was_random: bool,
}

/// Describes a Texture source for the given type
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
pub enum TextureType {
    Diffuse,
    Emissive,
    WorldStencil,
    Mask,
    Metalness,
    Normal,
    Roughness,
    Shape,
    Specular,
    Stencil,
}

impl TextureType {
    pub fn path_name(&self) -> &'static str {
        match *self {
            TextureType::Diffuse | TextureType::Emissive => "Colour",
            TextureType::WorldStencil | TextureType::Mask => "Mask",
            TextureType::Metalness => "Metal",
            TextureType::Normal => "Normal",
            TextureType::Roughness => "Roughness",
            TextureType::Shape => "Particle stencil",
            TextureType::Specular => "Specular",
            TextureType::Stencil => "Pulse stencil",
        }
    }
}
