use std::{
    collections::{BTreeSet, HashSet},
    fs::File,
    io::{Read, Write},
    path::Path,
};

use crate::{
    preset::Texturable,
    resolver::Resolver,
    structs::{FileAction, PresetAsset, TextureType, Version},
};
use anyhow::Context;

const ESRC: &str = "packer";

enum PresetFileType {}

pub struct PresetInfo {
    preset_file: String,
    resolver:    Resolver,
    pub files:   HashSet<PresetAsset>,
}

impl PresetInfo {
    fn get_textures(&mut self, t: &impl Texturable) {
        if let Some(texture) = t.diffuse() {
            self.files
                .insert(self.file_from_kn(TextureType::Diffuse, texture));
        }

        if let Some(texture) = t.emissive() {
            self.files
                .insert(self.file_from_kn(TextureType::Emissive, texture));
        }

        if let Some(texture) = t.mask() {
            self.files
                .insert(self.file_from_kn(TextureType::Mask, texture));
        }

        if let Some(texture) = t.metalness() {
            self.files
                .insert(self.file_from_kn(TextureType::Metalness, texture));
        }

        if let Some(texture) = t.normal() {
            self.files
                .insert(self.file_from_kn(TextureType::Normal, texture));
        }

        if let Some(texture) = t.roughness() {
            self.files
                .insert(self.file_from_kn(TextureType::Roughness, texture));
        }

        if let Some(texture) = t.specular() {
            self.files
                .insert(self.file_from_kn(TextureType::Specular, texture));
        }
    }

    pub fn new(v: &str, resv: Resolver) -> Self {
        Self {
            preset_file: v.to_owned(),
            resolver:    resv,
            files:       HashSet::new(),
        }
    }

    pub fn load(&mut self) -> anyhow::Result<()> {
        let data: crate::preset::KeysightPresetElement = {
            let f = File::open(
                &self
                    .resolver
                    .get_preset(&self.preset_file)
                    .context("the given preset file does not exist")?,
            )
            .context("failed to open preset file")?;
            serde_json::from_reader(&f).context("failed to parse preset file")?
        };

        self.get_textures(&data.effects.keypresses.keypress_material);
        self.get_textures(&data.effects.note_objects.note_border_material);
        self.get_textures(&data.effects.note_objects.note_object_material);
        self.get_textures(&data.scene.backdrop_material);
        self.get_textures(&data.scene.damper_material);
        self.get_textures(&data.scene.octave_material);
        self.get_textures(&data.scene.overlay_material);
        self.get_textures(&data.scene.piano_black_key_material);
        self.get_textures(&data.scene.piano_white_key_material);

        if data.effects.particles.particles_enabled {
            for particle in data.effects.particles.particle_v2_array {
                if !particle.enabled {
                    continue;
                }

                self.files
                    .insert(self.file_from_kn(TextureType::Shape, &particle.shape));
            }
        }

        if data.effects.pulses.pulses_enabled {
            for pulse in data.effects.pulses.pulse_array_v2 {
                if !pulse.enabled {
                    continue;
                }

                self.files
                    .insert(self.file_from_kn(TextureType::Stencil, &pulse.stencil));
                self.files
                    .insert(self.file_from_kn(TextureType::WorldStencil, &pulse.world_stencil));
            }
        }

        Ok(())
    }

    fn file_from_kn(&self, typ: TextureType, file: &str) -> PresetAsset {
        let path = self.resolver.stem_from_name(typ);
        let action = self.resolver.action_for_file(&path, file);

        PresetAsset {
            name: file.to_owned(),
            typ,
            src: action,
        }
    }

    pub fn pack_to(&self, path: &str) -> anyhow::Result<()> {
        let f = File::create(path).context("failed to create output file")?;
        let mut zipfile = zip::write::ZipWriter::new(f);

        let options = zip::write::FileOptions::default()
            .compression_method(zip::CompressionMethod::Zstd)
            .compression_level(Some(19))
            .large_file(false);

        let mut hashes_written: BTreeSet<[u8; blake3::OUT_LEN]> = BTreeSet::new();
        let mut file_entries = Vec::new();

        zipfile
            .add_directory("extra", options)
            .context("failed to create directory in zip")?;

        for (preset_file, preset_path, file_src_random) in self.files.iter().filter_map(|file| {
            if let FileAction::Pack { path, random } = &file.src {
                Some((file, path, random))
            } else {
                None
            }
        }) {
            let mut f =
                File::open(&preset_path).context("failed to read source file for compression")?;

            let file_size = f.metadata().map(|v| v.len()).unwrap_or(1024 * 64);
            let file_ext = Path::new(&preset_path)
                .extension()
                .map(|v| v.to_string_lossy())
                .map(|fv| format!(".{}", fv))
                .unwrap_or_else(String::new);

            let mut file_buffer = Vec::<u8>::with_capacity(file_size as usize);
            let mut readbuf = [0u8; 1024 * 64];
            let mut hasher = blake3::Hasher::new();
            loop {
                let read = f
                    .read(&mut readbuf)
                    .context("failed to read source file for compression")?;

                if read == 0 {
                    break;
                }

                hasher.update(&readbuf[..read]);
                file_buffer.extend_from_slice(&readbuf[..read]);
            }

            let hash = hasher.finalize();

            if hashes_written.contains(hash.as_bytes()) {
                println!("already wrote {}, skipping", hash.to_hex());
                continue;
            }

            hashes_written.insert(hash.clone().into());

            zipfile
                .start_file(format!("extra/{}", hash.to_hex()), options)
                .context("failed to create file in zip")?;
            zipfile
                .write_all(&file_buffer)
                .context("failed to write file to zip")?;

            file_entries.push(MetaEntry {
                hash:   format!("{}", hash.to_hex()),
                name:   preset_file.name.clone(),
                ext:    file_ext,
                typ:    preset_file.typ,
                random: *file_src_random,
            });
        }

        {
            let mut presetf = File::open(
                &self
                    .resolver
                    .get_preset(&self.preset_file)
                    .context("the given preset file does not exist")?,
            )
            .context("failed to open preset file")?;

            zipfile
                .start_file("preset.json", options)
                .context("failed to start file")?;
            std::io::copy(&mut presetf, &mut zipfile).context("failed to copy preset file")?;
        }

        zipfile
            .start_file("metadata.json", options)
            .context("failed to start file")?;

        let meta_data = Meta {
            preset:  self.preset_file.clone(),
            author:  "Example".to_string(),
            version: self.resolver.identify().unwrap(),
            assets:  file_entries,
        };

        serde_json::to_writer(&mut zipfile, &meta_data).context("failed to serialize metadata")?;

        Ok(())
    }
}

#[derive(Debug, Eq, PartialEq, Clone, Hash, serde::Serialize, serde::Deserialize)]
pub struct Meta {
    pub preset:  String,
    pub author:  String,
    pub version: Version,
    pub assets:  Vec<MetaEntry>,
}

#[derive(Debug, Eq, PartialEq, Clone, Hash, serde::Serialize, serde::Deserialize)]
pub struct MetaEntry {
    pub hash:   String,
    pub name:   String,
    pub ext:    String,
    pub typ:    TextureType,
    pub random: bool,
}
