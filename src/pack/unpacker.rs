use std::{
    fs::File,
    io,
    path::{Path, PathBuf},
};

use zip::result::ZipError;

use super::{helpers, MetaEntry, PackMetaData, Version};

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum UnpackError {
    #[error("package does not contain asset {name}")]
    #[diagnostic(code(unpack::package::asset_missing))]
    AssetNotFound { name: String },

    #[error("unable to read package")]
    #[diagnostic(code(unpack::io::error))]
    PackIOError {
        #[source]
        reason: io::Error,
    },

    #[error("unable to parse package")]
    #[diagnostic(code(unpack::io::zip))]
    ZipIOError {
        #[source]
        reason: zip::result::ZipError,
    },

    #[error("malformed json in package")]
    #[diagnostic(code(unpack::io::json))]
    JsonError {
        #[source]
        reason: serde_json::Error,
    },
}

pub struct Unpacker {
    path: PathBuf,
}

impl Unpacker {
    pub fn new(src: impl AsRef<Path>) -> Self {
        Self {
            path: src.as_ref().to_owned(),
        }
    }

    fn test_file(e: &MetaEntry) -> bool {
        helpers::custom_asset_dir(false)
            .join(e.texture_type.path_name())
            .join(format!("{}.{}", e.name, e.extension))
            .exists()
    }

    /// Loads metadata and checks for conflicts
    pub fn load(self) -> Result<PackedFile, UnpackError> {
        let fopen = File::open(&self.path).map_err(|reason| UnpackError::PackIOError { reason })?;
        let mut zipfile = zip::read::ZipArchive::new(fopen)
            .map_err(|reason| UnpackError::ZipIOError { reason })?;

        let metadata_file = zipfile
            .by_name("metadata.json")
            .map_err(|reason| UnpackError::ZipIOError { reason })?;
        let metadata: PackMetaData = serde_json::from_reader(metadata_file)
            .map_err(|reason| UnpackError::JsonError { reason })?;

        let mut conflicts = Vec::new();
        for asset in &metadata.assets {
            if Self::test_file(&asset) {
                conflicts.push(asset.clone());
            }
        }

        Ok(PackedFile {
            path: self.path,
            metadata,
            conflicts,
        })
    }
}

pub struct PackedFile {
    path:      PathBuf,
    metadata:  PackMetaData,
    conflicts: Vec<MetaEntry>,
}

impl PackedFile {
    pub fn exists(&self) -> bool {
        helpers::custom_preset_dir().join(format!("{}.json", self.metadata.name)).exists()
    }

    pub fn metadata(&self) -> &PackMetaData { &self.metadata }

    pub fn conflicts(&self) -> &[MetaEntry] { &self.conflicts }

    pub fn unpack(self) -> Result<(), UnpackError> {
        let mut zipf = zip::read::ZipArchive::new(
            File::open(self.path).map_err(|reason| UnpackError::PackIOError { reason })?,
        )
        .map_err(|reason| UnpackError::ZipIOError { reason })?;

        debug!("unpacking preset.json");
        let mut preset = zipf.by_name("preset.json").map_err(|reason| match reason {
            ZipError::FileNotFound => UnpackError::AssetNotFound {
                name: "preset.json".to_owned(),
            },
            other => UnpackError::ZipIOError { reason: other },
        })?;

        let mut out_preset = File::create(helpers::custom_preset_dir().join(self.metadata.name))
            .map_err(|reason| UnpackError::PackIOError { reason })?;
        std::io::copy(&mut preset, &mut out_preset)
            .map_err(|reason| UnpackError::PackIOError { reason })?;

        drop(preset);
        drop(out_preset);

        for asset in &self.metadata.assets {
            debug!(?asset.hash, "unpacking asset");

            let mut src = zipf
                .by_name(&format!("assets/{}", asset.hash))
                .map_err(|reason| match reason {
                    ZipError::FileNotFound => UnpackError::AssetNotFound {
                        name: asset.hash.clone(),
                    },
                    other => UnpackError::ZipIOError { reason: other },
                })?;
            let mut dst = File::create(
                helpers::custom_asset_dir(false)
                    .join(asset.texture_type.path_name())
                    .join(format!("{}.{}", asset.name, asset.hash)),
            )
            .map_err(|reason| UnpackError::PackIOError { reason })?;
            std::io::copy(&mut src, &mut dst)
                .map_err(|reason| UnpackError::PackIOError { reason })?;
        }

        Ok(())
    }
}
