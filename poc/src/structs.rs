use std::{fmt, path::PathBuf};

#[derive(Debug, Eq, PartialEq, Clone, Hash)]
pub struct PresetAsset {
    pub name: String,
    pub typ:  TextureType,
    pub src:  FileAction,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum FileAction {
    NotFound,
    DontPack,
    Pack { path: String, random: bool },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PathStem {
    pub default: PathBuf,
    pub custom:  PathBuf,
    pub random:  PathBuf,
}

#[derive(Debug, Eq, PartialEq, Clone, Hash, serde::Serialize, serde::Deserialize)]
pub struct Version(char, char, char, char);

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}.{}", self.0, self.1, self.2, self.3)
    }
}

impl Version {
    pub fn from_int(v: u16) -> Option<Version> {
        let raw = format!("{}", v);
        if raw.len() != 4 {
            warn!("unparsable version {}", v);
            return None;
        }

        let mut it = raw.chars();
        let maj = it.next()?;
        let min = it.next()?;
        let pat = it.next()?;
        let rev = it.next()?;

        Some(Version(maj, min, pat, rev))
    }
}

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
