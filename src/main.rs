#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#[macro_use]
extern crate tracing;

mod pack;
mod structs;

use eframe::{
	egui::{self, RichText},
	epaint::Color32,
};
use pack::{
	packer::{ExtraMeta, PackablePreset},
	unpacker::PackedFile,
	Version,
};

const PRESET_EXT: &str = "kspreset";
const PRESET_EXT_NAME: &str = "Keysight Preset";
const APP_PERSIST_KEY: &str = "ks-packer-data";
const DEFAULT_EXPORT_KEY: &str = "[Select Preset]";

fn main() {
	#[cfg(debug_assertions)]
	{
		std::env::set_var("RUST_LOG", concat!("info,", env!("CARGO_PKG_NAME"), "=trace"));
	}

	tracing_subscriber::fmt()
		.with_ansi(true)
		.with_file(true)
		.with_line_number(true)
		.with_thread_names(true)
		.with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
		.init();

	let egui_opts = eframe::NativeOptions {
		resizable: false,
		initial_window_size: Some(eframe::emath::vec2(600.0, 800.0)),
		vsync: true,
		..Default::default()
	};

	eframe::run_native("kspacker", egui_opts, Box::new(|cc| Box::new(App::new(cc))));
}

struct App {
	persisted:          PersistedState,
	current_error:      Option<String>,
	current_ks_version: Option<Version>,
	current_tab:        ActionTab,
	debug:              bool,
	help: bool,

	status_message: Option<Message>,

	known_presets: Vec<String>,

	import: ImportState,
	export: ExportState,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct PersistedState {
	keysight_path: String,
	firstrun: bool,
}

impl Default for PersistedState {
	fn default() -> Self {
		Self { keysight_path: String::default(), firstrun: true }
	}
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum ActionTab {
	Import,
	Export,
}

#[derive(Default)]
struct ImportState {
	path: String,
	pack: Option<PackedFile>,

	error_confirmed: bool,
}

#[derive(Default)]
struct ExportState {
	current_preset_selection: usize,

	e_name:        String,
	e_author:      String,
	e_description: String,
	e_version:     u32,

	packable_preset: Option<PackablePreset>,
}

#[derive(Debug, Clone)]
enum Message {
	Success { message: String },
	Error { message: String },
}

macro_rules! format_error {
	($e:expr) => {
		format!("---- Error ----\n{:#?}", $e)
	};
}

impl App {
	pub fn new(cc: &eframe::CreationContext) -> Self {
		let mut pers_state = if let Some(storage) = cc.storage {
			eframe::get_value(storage, APP_PERSIST_KEY).unwrap_or_default()
		} else {
			PersistedState::default()
		};

		let is_first_run = pers_state.firstrun;
		pers_state.firstrun = false;

		Self {
			import:             ImportState::default(),
			export:             ExportState::default(),
			current_error:      None,
			current_ks_version: None,
			current_tab:        ActionTab::Import,
			status_message:     None,
			known_presets:      vec![DEFAULT_EXPORT_KEY.to_string()],
			persisted:          pers_state,
			debug:              std::env::var("KSPACKER_DEBUG").map(|v| v == "1").unwrap_or(false),
			help:               is_first_run
		}
	}
}

impl eframe::App for App {
	fn update(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
		_frame.set_window_title("Keysight Preset Packer");
		egui::CentralPanel::default().show(ctx, |ui| {
			if self.debug {
				egui::Window::new("Path Debug").show(ctx, |ui| {
					ui.label(format!(
						"custom preset = {}\ncustom asset = {}",
						pack::helpers::custom_preset_dir().display(),
						pack::helpers::custom_asset_dir(false).display()
					));
				});
			}

			if self.help {
				egui::Window::new("Help").collapsible(false).min_width(ui.available_width() * 0.8).resizable(false).show(ctx, |ui| {
					ui.horizontal_wrapped(|ui| {
						// trick from https://github.com/emilk/egui/blob/898f4804b7b998ffeb1ff9f457b935e1364d6827/egui_demo_lib/src/demo/misc_demo_window.rs#L205
						let width = ui.fonts().glyph_width(&egui::TextStyle::Body.resolve(ui.style()), ' ');
						ui.spacing_mut().item_spacing.x = width;

						ui.label("Welcome to the ");
						ui.label(RichText::new("Help Screen").underline());
						ui.label(". This will give you some helpfull information on how and where things work.");
					});
					ui.separator();
					ui.heading("P and Set");
					ui.horizontal_wrapped(|ui| {
						ui.label("Around this tool you will see several text-inputs with a");
						let _ = ui.small_button("Pick");
						ui.label("and a");
						let _ = ui.small_button("Set");
						ui.label("next to it. These usually indicate a input for a path. You can use the");
						let _ = ui.small_button("Pick");
						ui.label("to open a file and/or folder picker. After you have made your input you will");
						ui.label(RichText::new("need").strong());
						ui.label("to confirm your input with the");
						let _ = ui.small_button("Set");
						ui.label("Button");
					});

					ui.separator();
					ui.heading("Keysight Path");
					ui.horizontal_wrapped(|ui| {
						ui.label("The keysight path is the full path to your Keysight installation directory. By default this will be");
						ui.code(r"C:\Program Files (x86)\Steam\steamapps\common\Keysight");
						ui.label("The correct directory will have only 2 subdirectories");
						ui.code("Engine");
						ui.label("and");
						ui.code("Keysight");
					});

					ui.separator();
					if ui.button("Close").clicked() {
						self.help = false;
					}
				});
			}

			ui.heading("Keysight Preset Packer - by HeapUnderflow");

			if let Some(message) = self.status_message.clone() {
				ui.vertical_centered(|ui| {
					match message {
						Message::Success { message: m } => {
							ui.label(RichText::new("Success!").color(Color32::GREEN).size(32.0));
							ui.label(&*m);
						},
						Message::Error { message: m } => {
							ui.label(RichText::new("Error!").color(Color32::RED).size(32.0));
							ui.label(&*m);
						},
					}

					if ui.button("Dismiss").clicked() {
						self.status_message = None;
					}
				});

				return;
			}

			ui.horizontal(|ui| {
				ui.label("Keysight Path:");
				ui.text_edit_singleline(&mut self.persisted.keysight_path);
				if ui.button("Pick").clicked() {
					if let Some(path) = rfd::FileDialog::new().pick_folder() {
						self.persisted.keysight_path = path.display().to_string();
					}
				}

				if ui.button("Set").clicked() && !self.persisted.keysight_path.is_empty() {
					match pack::get_ks_version(&self.persisted.keysight_path) {
						Ok(v) => match pack::helpers::list_all_presets() {
							Ok(presets) => {
								self.current_ks_version = Some(v);
								self.known_presets = presets;
								self.known_presets.insert(0, DEFAULT_EXPORT_KEY.to_string());
							},
							Err(why) => self.current_error = Some(format_error!(why)),
						},
						Err(why) => self.current_error = Some(format_error!(why)),
					}
				};

				if ui.button("Help").clicked() {
					self.help = !self.help;
				}

				ui.allocate_space(egui::Vec2::new(ui.available_width(), 0.0));
			});

			ui.label(format!(
				"Keysight Version: {}",
				pack::helpers::maybe_format_version(self.current_ks_version)
			));

			if let Some(error) = self.current_error.clone() {
				ui.group(|ui| {
					ui.label(RichText::new(error).color(Color32::RED));
					if ui.button("Clear Error").on_hover_text("This will clear the error messsage. If the error persists pleases check your path and presets").clicked() {
						self.current_error = None;
					}
					ui.allocate_space(egui::Vec2::new(ui.available_width(), 0.0));
				});
			}

			ui.separator();

			ui.horizontal(|ui| {
				ui.radio_value(&mut self.current_tab, ActionTab::Import, "Import");
				ui.radio_value(&mut self.current_tab, ActionTab::Export, "Export");
			});

			ui.add_enabled_ui(self.current_ks_version.is_some(), |ui| {
				ui.group(|ui| {
					ui.set_min_size(ui.available_size());
					egui::ScrollArea::vertical().show(ui, |ui| {
						match self.current_tab {
							ActionTab::Import => self.import_ui(ui),
							ActionTab::Export => self.export_ui(ui),
						}

						ui.allocate_space(egui::Vec2::new(ui.available_width(), 0.0));
					});
				});
			});
		});
	}

	fn save(&mut self, _storage: &mut dyn eframe::Storage) {
		eframe::set_value(_storage, APP_PERSIST_KEY, &self.persisted);
	}

	fn persist_egui_memory(&self) -> bool { false }

	fn persist_native_window(&self) -> bool { true }
}

impl App {
	fn import_ui(&mut self, ui: &mut egui::Ui) {
		ui.heading("Import Preset");

		ui.horizontal(|pick_ui| {
			pick_ui.label("Import from:");
			pick_ui.text_edit_singleline(&mut self.import.path);
			if pick_ui.button("Pick").clicked() {
				if let Some(path) =
					rfd::FileDialog::new().add_filter(PRESET_EXT_NAME, &[PRESET_EXT]).pick_file()
				{
					self.import.path = path.display().to_string();
				}
			}
			if pick_ui.button("Set").clicked() && !self.import.path.is_empty() {
				match pack::unpacker::Unpacker::new(&self.import.path).load() {
					Ok(preset) => self.import.pack = Some(preset),
					Err(why) => self.current_error = Some(format_error!(why)),
				}
			}
		});

		if let Some(preset) = self.import.pack.as_ref() {
			let meta = preset.metadata();

			ui.separator();

			ui.label("Loaded Preset:");
			egui::Grid::new("kspack-import-preset-info").num_columns(2).striped(true).show(
				ui,
				|ui| {
                    ui.wrap_text();

					ui.label("Name");
					ui.add(egui::Label::new(&meta.name).wrap(true));
					ui.end_row();

					ui.label("Version");
					ui.label(format!("{:#X}", meta.preset_version));
					ui.end_row();

					ui.label("Keysight Version");
					ui.label(RichText::new(format!("{:#X}", meta.target_version)).color(
						if meta.target_version != self.current_ks_version.unwrap() {
							Color32::RED
						} else {
							Color32::BLACK
						},
					));
					ui.end_row();

					ui.label("Author");
					ui.add(egui::Label::new(&meta.author).wrap(true));
					ui.end_row();

					ui.label("Description");
					ui.add(egui::Label::new(&meta.description).wrap(true));
					ui.end_row();

					ui.label("Packed on");
					ui.label(&meta.packed.format("%F %T").to_string());
					ui.end_row();
				},
			);

			let exists = preset.exists();
			let has_errors = !preset.conflicts().is_empty() || exists;

			if has_errors {
				ui.separator();
			}

			if exists {
				ui.label(
					RichText::new(
						"Warning! A preset already exists under this name.\n    Please make sure \
						 that you really want to overwrite it.",
					)
					.color(Color32::RED),
				);
			}

			if !preset.conflicts().is_empty() {
				ui.label(
					RichText::new(
						"Warning! This preset has conflicting assets.\n    Please make sure that \
						 you want to overwrite these assets!",
					)
					.color(Color32::RED),
				);
				ui.label("Conflicting Assets");
				egui::Grid::new("kspack-import-conflict-list").num_columns(3).striped(true).show(
					ui,
					|ui| {
						ui.label(RichText::new("File").strong().underline());
						ui.label(RichText::new("Type").strong().underline());
						ui.label(RichText::new("Hash").strong().underline());
						ui.end_row();

						for entry in preset.conflicts() {
							ui.label(format!("{}.{}", entry.name, entry.extension));
							ui.label(format!("{:?}", entry.texture_type));
							ui.label(format!(
								"{}...",
								entry.hash.chars().take(16).collect::<String>()
							));
							ui.end_row();
						}
					},
				);
			}

			ui.separator();

			if has_errors {
				ui.checkbox(
					&mut self.import.error_confirmed,
					"I have understood above conflicts and aknowledge that i want to overwrite \
					 all specified files.",
				);
			}

			if ui
				.add_enabled(
					!has_errors || self.import.error_confirmed,
					egui::Button::new("Import"),
				)
				.clicked()
			{
				if let Err(why) = preset.unpack() {
					self.current_error = Some(format_error!(why));
					self.import.error_confirmed = false;
				} else {
					let name = meta.name.clone();
					self.import = ImportState::default();
					self.status_message = Some(Message::Success {
						message: format!("Successfully imported preset {}", name),
					});
				}
			}
		}
	}

	fn export_ui(&mut self, ui: &mut egui::Ui) {
		ui.heading("Export Preset");

		ui.horizontal(|ui| {
			ui.label("Select Preset: ");
			let cbc = egui::ComboBox::from_id_source("kspack-export-preset-select")
				.width(ui.available_width())
				.show_index(
					ui,
					&mut self.export.current_preset_selection,
					self.known_presets.len(),
					|idx| self.known_presets[idx].to_owned(),
				)
				.changed();

			if cbc {
				self.export.packable_preset = None;

				if self.export.current_preset_selection > 0 {
					self.export.e_name =
						self.known_presets[self.export.current_preset_selection].clone();

					let packer = pack::packer::Packer::new(
						&self.persisted.keysight_path,
						self.current_ks_version.unwrap(),
						&self.known_presets[self.export.current_preset_selection],
					);

					match packer.collect(true) {
						Err(why) => self.current_error = Some(format_error!(why)),
						Ok(preset) => self.export.packable_preset = Some(preset),
					}
				}
			}
		});

		if let Some(ppreset) = self.export.packable_preset.as_ref() {
			ui.separator();

			egui::Grid::new("kspack-export-preset-select").num_columns(2).show(ui, |ui| {
				ui.label("Name");
				ui.text_edit_singleline(&mut self.export.e_name);
				ui.end_row();

				ui.label("Author");
				ui.text_edit_singleline(&mut self.export.e_author);
				ui.end_row();

				ui.label("Description");
				ui.text_edit_multiline(&mut self.export.e_description);
				ui.end_row();

				ui.label("Version");
				ui.add(egui::DragValue::new(&mut self.export.e_version).prefix("v"));
				ui.end_row();
			});

			if self.export.e_name.len() > 64 {
				self.export.e_name = self.export.e_name.chars().take(64).collect();
			}

			if self.export.e_author.len() > 64 {
				self.export.e_author = self.export.e_author.chars().take(64).collect();
			}

			if self.export.e_description.len() > 512 {
				self.export.e_description = self.export.e_description.chars().take(512).collect();
			}

			if !ppreset.assets().is_empty() {
				ui.label("The preset references the following assets that will be included:");
				egui::Grid::new("kspack-export-found-assets").num_columns(2).show(ui, |ui| {
					ui.label(RichText::new("File").strong().underline());
					ui.label(RichText::new("Type").strong().underline());
					ui.end_row();

					for asset in ppreset.assets() {
						ui.label(format!("{}.{}", asset.name, asset.ext));
						ui.label(format!("{:?}", asset.texture_type));
						ui.end_row();
					}
				});
			}

			if ui.button("Export").clicked() {
				if let Some(path) = rfd::FileDialog::new().add_filter(PRESET_EXT_NAME, &[PRESET_EXT]).save_file() {
					let result = ppreset.pack(&path, ExtraMeta {
						rename:             if self.export.e_name.bytes().any(|v| !v.is_ascii_whitespace())
							&& self.export.e_name
								!= self.known_presets[self.export.current_preset_selection]
						{
							Some(self.export.e_name.clone())
						} else {
							None
						},
						author:             self.export.e_author.clone(),
						description:        self.export.e_description.clone(),
						version:            self.export.e_version,
						current_ks_version: self.current_ks_version.unwrap(),
					});

                    match result {
                        Ok(()) => {self.status_message = Some(Message::Success {
                            message: format!("Exported preset to {}", path.display())
                        });

                                  self.export = ExportState::default();
                        },
                        Err(why) => self.status_message = Some(Message::Error {
                            message: format!("Failed to export preset to {}:\n\n{:#?}", path.display(), why)
                        })
                    }
				}
			}
		}
	}
}
