use anyhow::Context;
use once_cell::unsync::OnceCell;
use std::{fs::File, path::PathBuf};
use zip::ZipArchive;

use crate::{
    packer::{Meta, MetaEntry},
    resolver::Resolver,
    structs::Version,
};

pub struct Unpacker {
    resolver: Resolver,
    source:   PathBuf,

    metadata: OnceCell<Meta>,
}

impl Unpacker {
    pub fn new(resolver: Resolver, src: impl Into<PathBuf>) -> Self {
        Unpacker {
            resolver,
            source: src.into(),
            metadata: OnceCell::new(),
        }
    }

    fn open_zip(&self) -> anyhow::Result<ZipArchive<File>> {
        let f = File::open(&self.source).context("failed to open zip file")?;
        ZipArchive::new(f).context("failed to parse zip file")
    }

    fn load_meta(&self) -> anyhow::Result<&Meta> {
        if let Some(meta) = self.metadata.get() {
            return Ok(meta);
        }

        let mut zipf = self.open_zip().context("zip failure")?;

        let metadata_file = zipf
            .by_name("metadata.json")
            .context("failed to read metadata")?;
        let metadata: Meta =
            serde_json::from_reader(metadata_file).context("failed to parse metadata")?;

        self.metadata
            .set(metadata)
            .expect("fatal contingency error");

        Ok(self.metadata.get().unwrap())
    }

    pub fn target_version(&self) -> anyhow::Result<Version> {
        Ok(self
            .load_meta()
            .context("failed to load metadata")?
            .version
            .clone())
    }

    pub fn author(&self) -> anyhow::Result<String> {
        Ok(self.load_meta().context("failed to load metadata")?.author.clone())
    }

    pub fn name(&self) -> anyhow::Result<String> {
        Ok(self.load_meta().context("failed to load metadata")?.preset.clone())
    }

    pub fn conflicts(&self) -> anyhow::Result<(bool, Vec<MetaEntry>)> {
        let metadata = self.load_meta().context("metadata failure")?;

        let preset_conflict = self.resolver.get_preset(&metadata.preset).is_some();

        let mut conflicts = Vec::new();
        for entry in &metadata.assets {
            let stem = self.resolver.stem_from_name(entry.typ);

            if self.resolver.test_file(&stem, &entry.name) {
                conflicts.push(entry.clone());
            }
        }

        Ok((preset_conflict, conflicts))
    }

    pub fn unpack(&self) -> anyhow::Result<()> {
        let metadata = self.load_meta().context("metadata failure")?;
        let mut zipf = self.open_zip().context("zip failure")?;

        let mut zipped_preset = zipf
            .by_name("preset.json")
            .context("preset file does not exist in package")?;

        // writing preset
        let mut out_file = File::create(self.resolver.preset_path(&metadata.preset))
            .context("failed to create preset file")?;
        std::io::copy(&mut zipped_preset, &mut out_file).context("failed to copy preset")?;

        // explictily close (drop) file and preset here
        drop(out_file);
        drop(zipped_preset);

        for asset in &metadata.assets {
            let stem = self.resolver.stem_from_name(asset.typ);

            println!("unpacking {:?}", asset);

            let mut source = zipf
                .by_name(&format!("extra/{}", asset.hash))
                .context("failed to export preset asset")?;
            let mut target = File::create(stem.custom.join(format!("{}{}", asset.name, asset.ext)))
                .context("failed to create asset")?;

            std::io::copy(&mut source, &mut target).context("failed to copy asset")?;
        }

        Ok(())
    }
}
