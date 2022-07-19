#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#[macro_use]
extern crate tracing;

mod pack;
mod structs;

use eframe::{
    egui::{self, RichText},
    epaint::Color32,
};
use pack::{unpacker::PackedFile, MetaEntry, Version};

const PRESET_EXT: &str = "kspreset";
const APP_PERSIST_KEY: &str = "ks-packer-data";

fn main() {
    #[cfg(debug_assertions)]
    {
        std::env::set_var(
            "RUST_LOG",
            concat!("info,", env!("CARGO_PKG_NAME"), "=trace"),
        );
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
        ..Default::default()
    };

    eframe::run_native("kspacker", egui_opts, Box::new(|cc| Box::new(App::new(cc))));
}

struct App {
    persisted:          PersistedState,
    current_error:      Option<String>,
    current_ks_version: Option<Version>,
    current_tab:        ActionTab,

    known_presets: Vec<String>,

    import: ImportState,
    export: ExportState,
}

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
struct PersistedState {
    keysight_path: String,
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

    error_confirmed: bool
}

#[derive(Default)]
struct ExportState {
    current_preset_selection: usize,
}

macro_rules! format_error {
    ($e:expr) => {
        format!("---- Error ----\n{:#?}", $e)
    };
}

impl App {
    pub fn new(cc: &eframe::CreationContext) -> Self {
        let pers_state = if let Some(storage) = cc.storage {
            eframe::get_value(storage, APP_PERSIST_KEY).unwrap_or_default()
        } else {
            PersistedState::default()
        };

        Self {
            import:             ImportState::default(),
            export:             ExportState::default(),
            current_error:      None,
            current_ks_version: None,
            current_tab:        ActionTab::Import,
            known_presets:      Vec::new(),
            persisted:          pers_state,
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
        _frame.set_window_title("Keysight Preset Packer");
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Keysight Preset Packer - by HeapUnderflow");

            ui.horizontal(|ui| {
                ui.label("Keysight Path:");
                ui.text_edit_singleline(&mut self.persisted.keysight_path);
                if ui.button("P").clicked() {
                    if let Some(path) = rfd::FileDialog::new().pick_folder() {
                        self.persisted.keysight_path = path.display().to_string();
                    }
                }

                if ui.button("Set").clicked() && !self.persisted.keysight_path.is_empty() {
                    match pack::get_ks_version(&self.persisted.keysight_path) {
                        Ok(v) => {
                            self.current_ks_version = Some(v);
                            match pack::helpers::list_all_presets() {
                                Ok(presets) => {
                                    self.known_presets = presets;
                                    self.known_presets.insert(0, "[Select Preset]".to_string());
                                },
                                Err(why) => self.current_error = Some(format_error!(why)),
                            }
                        },
                        Err(why) => self.current_error = Some(format_error!(why)),
                    }
                };

                ui.allocate_space(egui::Vec2::new(ui.available_width(), 0.0));
            });

            ui.label(format!(
                "Keysight Version: {}",
                pack::helpers::maybe_format_version(self.current_ks_version)
            ));

            if let Some(error) = &self.current_error {
                
                ui.group(|ui| {
                    ui.label(RichText::new(error).color(Color32::RED));
                    ui.allocate_space(egui::Vec2::new(ui.available_width(), 0.0));
                });
            }

            // if ui.button("doit").clicked() {
            //     let packer =
            //         pack::packer::Packer::new(r"G:\lib\steam\steamapps\common\Keysight\", "Ori");
            //     let collected = packer.collect(false);
            //     let collected = collected.unwrap();
            //     collected
            //         .pack(
            //             "test.kspreset",
            //             pack::packer::ExtraMeta {
            //                 rename:             None,
            //                 author:             String::from("Example Author"),
            //                 description:        String::new(),
            //                 version:            0,
            //                 current_ks_version: 1500,
            //             },
            //         )
            //         .unwrap();
            // }
            
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
            if pick_ui.button("P").clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Keysight Preset", &[PRESET_EXT])
                    .pick_file()
                {
                    self.import.path = path.display().to_string();
                }
            }
            if pick_ui.button("Set").clicked()
                && self.import.path.len() > 0
            {
                match pack::unpacker::Unpacker::new(&self.import.path).load() {
                    Ok(preset) => self.import.pack = Some(preset),
                    Err(why) => self.current_error = Some(format_error!(why))
                }
            }
        });

        if let Some(preset) = self.import.pack.as_ref() {
            let meta = preset.metadata();

            ui.separator();

            ui.label("Loaded Preset:");
            egui::Grid::new("kspack-import-preset-info").num_columns(2).striped(true).show(ui, |ui| {
                ui.label("Name");
                ui.label(&meta.name);
                ui.end_row();

                ui.label("Version");
                ui.label(format!("{:#X}", meta.preset_version));
                ui.end_row();
            
                ui.label("Keysight Version");
                ui.label(RichText::new(format!("{:#X}", meta.target_version)).color(if meta.target_version != self.current_ks_version.unwrap() { Color32::RED } else { Color32::BLACK }));
                ui.end_row();

                ui.label("Author");
                ui.label(&meta.author);
                ui.end_row();

                ui.label("Description");
                ui.label(&meta.description);
                ui.end_row();

                ui.label("Packed on");
                ui.label(&meta.packed.format("%F %T").to_string());
                ui.end_row();
            });

            let exists = preset.exists();
            let has_errors = preset.conflicts().len() > 0 || exists;

            if has_errors {
                ui.separator();
            }

            if exists {
                ui.label(RichText::new("Warning! A preset already exists under this name.\n    Please make sure that you really want to overwrite it.").color(Color32::RED));
            }

            if preset.conflicts().len() > 0 {
                ui.label(RichText::new("Warning! This preset has conflicting assets.\n    Please make sure that you want to overwrite these assets!").color(Color32::RED));
                ui.label("Conflicting Assets");
                egui::Grid::new("kspack-import-conflict-list").num_columns(3).striped(true).show(ui, |ui| {
                    ui.label(RichText::new("File").strong().underline());
                    ui.label(RichText::new("Type").strong().underline());
                    ui.label(RichText::new("Hash").strong().underline());
                    ui.end_row();

                    for entry in preset.conflicts() {
                        ui.label(format!("{}.{}", entry.name, entry.extension));
                        ui.label(format!("{:?}", entry.texture_type));
                        ui.label(format!("{}...", entry.hash.chars().take(16).collect::<String>()));
                        ui.end_row();
                    }
                });
            }

            ui.separator();

            if has_errors {
                ui.checkbox(&mut self.import.error_confirmed, "I have understood above conflicts and aknowledge that i want to overwrite all specified files.");
            }

            if ui.add_enabled(!has_errors || (has_errors && self.import.error_confirmed) , egui::Button::new("Import")).clicked() {
                info!("import");
            }
        }
    }

    fn export_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Export Preset");
    }
}