use std::{
	fs,
	io,
	path::{Path, PathBuf},
};

use super::Version;

pub fn root_preset_dir(install_path: impl AsRef<Path>) -> PathBuf {
	install_path.as_ref().join("Keysight").join("Default presets").join("Standard")
}

pub fn root_asset_dir(install_path: impl AsRef<Path>) -> PathBuf {
	install_path.as_ref().join("Keysight").join("Default textures")
}

pub fn custom_preset_dir() -> PathBuf {
	data_local_dir().join("Keysight").join("Saved").join("Presets")
}

pub fn custom_asset_dir(random: bool) -> PathBuf {
	data_local_dir()
		.join("Keysight")
		.join("Saved")
		.join(if random { "Textures (randomizer enabled)" } else { "Textures" })
}

fn data_local_dir() -> PathBuf {
	#[cfg(feature = "proton-steam-comptime")]
	{
		std::path::PathBuf::from(env!("PROTON_PATH_OVR"))
	}
	#[cfg(not(feature = "proton-steam-comptime"))]
	{
		dirs::data_local_dir().unwrap()
	}
}

pub fn maybe_format_version(version: Option<Version>) -> String {
	if let Some(ver) = version {
		format!("{:#X}", ver)
	} else {
		String::from("Unknown")
	}
}

pub fn list_all_presets() -> io::Result<Vec<String>> {
	let mut presets = Vec::new();
	for f in fs::read_dir(self::custom_preset_dir())? {
		let file = f?;
		if file.metadata()?.is_file() {
			if let Some(name) = file.path().file_stem() {
				presets.push(name.to_string_lossy().to_string());
			}
		}
	}

	Ok(presets)
}
