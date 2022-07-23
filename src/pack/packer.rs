use std::{
	collections::BTreeSet,
	fs::File,
	io::{self, Read, Write},
	path::{Path, PathBuf},
};

use chrono::Utc;

use super::{helpers, ks_preset::Texturable, MetaEntry, PackMetaData, TextureType, Version};

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum PackError {
	#[error("preset `{name}` not found")]
	#[diagnostic(
		code(pack::preset::not_found),
		help("Make shure you have not renamed or deleted the preset after starting the program.")
	)]
	NotFound { name: String },

	#[error("cannot read preset")]
	#[diagnostic(code(pack::preset::unreadable))]
	Unreadable {
		#[source]
		reason: io::Error,
	},

	#[error("malformed preset")]
	#[diagnostic(code(pack::preset::malformed))]
	MalformedPreset {
		#[source]
		reason: serde_json::Error,
	},

	#[error("builtin preset")]
	#[diagnostic(
		code(pack::preset::builtin),
		help("The preset is a builtin preset, and no override was set")
	)]
	IsBuiltin,

	#[error("cannot create output file")]
	#[diagnostic(code(pack::pack::io))]
	PackIoError {
		#[source]
		reason: io::Error,
	},

	#[error("error while zipping")]
	#[diagnostic(code(pack::pack::zip))]
	ZipError {
		#[source]
		reason: zip::result::ZipError,
	},

	#[error("malformed metadata")]
	#[diagnostic(code(pack::meta::malformed))]
	MalformedMeta {
		#[source]
		reason: serde_json::Error,
	},

	#[error("wrong version")]
	#[diagnostic(code(pack::meta::invalid_version))]
	WrongVersion {
		wanted: Version,
		got: Version
	},
}

#[derive(Debug)]
pub struct Packer {
	root:   PathBuf,
	preset: String,
	ksv: Version
}

impl Packer {
	pub fn new(root: impl Into<PathBuf>, ksv: Version, preset: impl Into<String>) -> Self {
		Packer { root: root.into(), ksv, preset: preset.into() }
	}

	#[instrument(skip(self))]
	pub fn collect(&self, allow_builtin: bool) -> Result<PackablePreset, PackError> {
		if !allow_builtin && self.check_builtin_preset() {
			return Err(PackError::IsBuiltin);
		}

		info!("discovering assets");
		let preset_path = helpers::custom_preset_dir().join(format!("{}.json", self.preset));
		if !preset_path.exists() {
			warn!(preset_path=%preset_path.display(), "preset does not exist");
			return Err(PackError::NotFound { name: self.preset.clone() });
		}

		let ks_version = {
			#[derive(Debug, serde::Deserialize)]
			struct KsVersionOnly {
				#[serde(rename = "versionForUpdatePurposes")]
				pub version_for_update_purposes: u32,
			}

			let f = File::open(&preset_path).map_err(|reason| PackError::Unreadable { reason })?;

			serde_json::from_reader::<_, KsVersionOnly>(f).map_err(|reason| PackError::MalformedPreset { reason })
		}?;

		if ks_version.version_for_update_purposes != self.ksv {
			return Err(PackError::WrongVersion { wanted: self.ksv, got: ks_version.version_for_update_purposes });
		}

		let loaded_preset: super::ks_preset::KeysightPresetElement = {
			let f = File::open(&preset_path).map_err(|reason| PackError::Unreadable { reason })?;

			serde_json::from_reader(f).map_err(|reason| PackError::MalformedPreset { reason })
		}?;

		debug!("loaded preset");

		let mut files = Vec::with_capacity(5);

		debug!("discovering files");
		self.get_textures(&mut files, &loaded_preset.effects.keypresses.keypress_material);
		self.get_textures(&mut files, &loaded_preset.effects.note_objects.note_border_material);
		self.get_textures(&mut files, &loaded_preset.effects.note_objects.note_object_material);
		self.get_textures(&mut files, &loaded_preset.scene.backdrop_material);
		self.get_textures(&mut files, &loaded_preset.scene.damper_material);
		self.get_textures(&mut files, &loaded_preset.scene.octave_material);
		self.get_textures(&mut files, &loaded_preset.scene.overlay_material);
		self.get_textures(&mut files, &loaded_preset.scene.piano_black_key_material);
		self.get_textures(&mut files, &loaded_preset.scene.piano_white_key_material);

		if loaded_preset.effects.particles.particles_enabled {
			for particle in loaded_preset.effects.particles.particle_v2_array {
				if !particle.enabled {
					continue;
				}

				files.push(self.make_found_file(TextureType::Shape, &particle.shape));
			}
		}

		if loaded_preset.effects.pulses.pulses_enabled {
			for pulse in loaded_preset.effects.pulses.pulse_array_v2 {
				if !pulse.enabled {
					continue;
				}

				files.push(self.make_found_file(TextureType::Stencil, &pulse.stencil));
				files.push(self.make_found_file(TextureType::WorldStencil, &pulse.world_stencil));
			}
		}

		files.retain_mut(|asset| asset.action == AssetAction::Pack);
		Ok(PackablePreset { name: self.preset.clone(), path: preset_path, assets: files })
	}

	#[instrument(skip(self, files, t))]
	fn get_textures(&self, files: &mut Vec<FoundAsset>, t: &impl Texturable) {
		if let Some(texture) = t.diffuse() {
			files.push(self.make_found_file(TextureType::Diffuse, texture));
		}

		if let Some(texture) = t.emissive() {
			files.push(self.make_found_file(TextureType::Emissive, texture));
		}

		if let Some(texture) = t.mask() {
			files.push(self.make_found_file(TextureType::Mask, texture));
		}

		if let Some(texture) = t.metalness() {
			files.push(self.make_found_file(TextureType::Metalness, texture));
		}

		if let Some(texture) = t.normal() {
			files.push(self.make_found_file(TextureType::Normal, texture));
		}

		if let Some(texture) = t.roughness() {
			files.push(self.make_found_file(TextureType::Roughness, texture));
		}

		if let Some(texture) = t.specular() {
			files.push(self.make_found_file(TextureType::Specular, texture));
		}
	}

	#[instrument(skip(self))]
	fn make_found_file(&self, typ: TextureType, file: &str) -> FoundAsset {
		fn test_exts(p: &Path, n: &str) -> Option<(PathBuf, &'static str)> {
			for ext in ["png", "jpg", "jpeg"] {
				let current = p.join(format!("{}.{}", n, ext));
				if current.exists() {
					return Some((current, ext));
				}
			}
			None
		}

		let pathinfo = test_exts(&helpers::root_asset_dir(&self.root).join(typ.path_name()), file);
		if let Some((path, ext)) = pathinfo {
			debug!(path=%path.display(), "found builtin asset");
			return FoundAsset {
				name: file.to_owned(),
				ext: ext.to_owned(),
				texture_type: typ,
				random: false,
				path,
				action: AssetAction::Ignore,
			};
		}

		let pathinfo = test_exts(&helpers::custom_asset_dir(false).join(typ.path_name()), file);
		if let Some((path, ext)) = pathinfo {
			debug!(path=%path.display(), "found custom asset");
			return FoundAsset {
				name: file.to_owned(),
				ext: ext.to_owned(),
				texture_type: typ,
				random: false,
				path,
				action: AssetAction::Pack,
			};
		}

		let pathinfo = test_exts(&helpers::custom_asset_dir(true).join(typ.path_name()), file);
		if let Some((path, ext)) = pathinfo {
			debug!(path=%path.display(), "found custom asset (random)");
			return FoundAsset {
				name: file.to_owned(),
				ext: ext.to_owned(),
				texture_type: typ,
				random: false,
				path,
				action: AssetAction::Pack,
			};
		}

		debug!("asset not found");
		FoundAsset {
			name:         file.to_owned(),
			ext:          String::new(),
			texture_type: typ,
			random:       false,
			path:         PathBuf::new(),
			action:       AssetAction::NotFound,
		}
	}

	fn check_builtin_preset(&self) -> bool {
		helpers::root_preset_dir(&self.root).join(format!("{}.json", self.preset)).exists()
	}
}

#[derive(Debug)]
pub struct FoundAsset {
	pub name:         String,
	pub ext:          String,
	pub texture_type: TextureType,
	pub random:       bool,
	pub path:         PathBuf,
	pub action:       AssetAction,
}

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub enum AssetAction {
	NotFound,
	Ignore,
	Pack,
}

pub struct ExtraMeta {
	pub rename:             Option<String>,
	pub author:             String,
	pub description:        String,
	pub version:            u32,
	pub current_ks_version: u32,
}

#[derive(Debug)]
pub struct PackablePreset {
	name:   String,
	path:   PathBuf,
	assets: Vec<FoundAsset>,
}

impl PackablePreset {
	pub fn name(&self) -> &str { &self.name }

	pub fn assets(&self) -> &[FoundAsset] { &self.assets }

	pub fn pack(&self, to: impl AsRef<Path>, extra_meta: ExtraMeta) -> Result<(), PackError> {
		let output = File::create(to).map_err(|reason| PackError::PackIoError { reason })?;
		let mut zipfile = zip::write::ZipWriter::new(output);

		let zipoptions = zip::write::FileOptions::default()
			.compression_method(zip::CompressionMethod::Zstd)
			.compression_level(Some(19))
			.large_file(false);

		let mut hashes_written: BTreeSet<[u8; blake3::OUT_LEN]> = BTreeSet::new();
		let mut asset_entries = Vec::new();

		zipfile
			.add_directory("assets", zipoptions)
			.map_err(|reason| PackError::ZipError { reason })?;

		for asset in &self.assets {
			let mut src =
				File::open(&asset.path).map_err(|reason| PackError::PackIoError { reason })?;

			// used as a size hint for the buffer vec
			let file_size = src.metadata().map(|v| v.len()).unwrap_or(1024 * 64);

			let mut full_file_buffer = Vec::with_capacity(file_size as usize);
			let mut read_buffer = [0u8; 1024 * 64];
			let mut hasher = blake3::Hasher::new();

			loop {
				let read = src
					.read(&mut read_buffer)
					.map_err(|reason| PackError::PackIoError { reason })?;

				if read == 0 {
					break;
				}

				hasher.update(&read_buffer[..read]);
				full_file_buffer.extend(&read_buffer[..read]);
			}

			let hash = hasher.finalize();

			if hashes_written.contains(hash.as_bytes()) {
				info!(%hash, "already wrote this hash");
				continue;
			}

			hashes_written.insert(hash.into());

			zipfile
				.start_file(format!("assets/{}", hash.to_hex()), zipoptions)
				.map_err(|reason| PackError::ZipError { reason })?;
			zipfile
				.write_all(&full_file_buffer)
				.map_err(|reason| PackError::PackIoError { reason })?;

			asset_entries.push(MetaEntry {
				hash:              format!("{}", hash.to_hex()),
				name:              asset.name.clone(),
				extension:         asset.ext.clone(),
				texture_type:      asset.texture_type,
				source_was_random: asset.random,
			});
		}

		let mut preset_file =
			File::open(&self.path).map_err(|reason| PackError::PackIoError { reason })?;
		zipfile
			.start_file("preset.json", zipoptions)
			.map_err(|reason| PackError::ZipError { reason })?;
		std::io::copy(&mut preset_file, &mut zipfile)
			.map_err(|reason| PackError::PackIoError { reason })?;

		let meta = PackMetaData {
			name:           extra_meta.rename.unwrap_or_else(|| self.name.clone()),
			author:         extra_meta.author,
			description:    extra_meta.description,
			packed:         Utc::now(),
			preset_version: extra_meta.version,
			target_version: extra_meta.current_ks_version,
			assets:         asset_entries,
		};

		zipfile
			.start_file("metadata.json", zipoptions)
			.map_err(|reason| PackError::ZipError { reason })?;
		serde_json::to_writer(&mut zipfile, &meta)
			.map_err(|reason| PackError::MalformedMeta { reason })?;

		Ok(())
	}
}
