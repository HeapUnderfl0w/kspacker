#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#[macro_use]
extern crate tracing;
mod packer;
mod preset;
mod resolver;
mod settings;
mod structs;
mod unpacker;

use std::borrow::Cow;

use eframe::egui;
use packer::MetaEntry;
use resolver::Resolver;
use settings::Settings;
use tracing_subscriber::fmt::format::DefaultFields;
use unpacker::Unpacker;

const PRESET_EXT: &str = "kspreset";

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
        .with_env_filter(tracing_subscriber::EnvFilter::default())
        .init();

    let settings = Settings::load().unwrap();

    let egui_opts = eframe::NativeOptions {
        resizable: false,
        initial_window_size: Some(eframe::emath::vec2(600.0, 800.0)),
        ..Default::default()
    };

    eframe::run_native(
        "KS-Packernel",
        egui_opts,
        Box::new(|_| Box::new(App::new(settings))),
    );
}

struct App {
    settings: Settings,
    resolver: Option<Resolver>,

    // direct ui state
    general: GeneralState,
    export:  ExportState,
    import:  ImportState,
}

struct GeneralState {
    current_version_dsp: String,
    current_tab:         CurrentTab,
    current_error:       Option<anyhow::Error>,
    found_keysight:      bool,
}

struct ExportState {
    current_selection: usize,
    preset_list:       Vec<String>,

    export_path: String,
    export_ok:   bool,
}

impl ExportState {
    fn valid(&self) -> bool {
        !self.export_path.is_empty()
            && self.current_selection > 0
            && !self.preset_list.is_empty()
            && !self.export_ok
    }
}

impl Default for ExportState {
    fn default() -> Self {
        ExportState {
            preset_list:       vec![String::from("[No Preset Selected]")],
            current_selection: 0,
            export_path:       String::default(),
            export_ok:         false,
        }
    }
}

struct ImportState {
    unpacker:        Option<Unpacker>,
    import_path:     String,
    conflicts:       Vec<MetaEntry>,
    preset_conflict: bool,
    checked:         bool,
    success:         bool,
}

impl Default for ImportState {
    fn default() -> Self {
        ImportState {
            import_path:     String::default(),
            unpacker:        None,
            conflicts:       Vec::new(),
            preset_conflict: false,
            checked:         false,
            success:         false,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum CurrentTab {
    Export,
    Import,
}

impl App {
    pub fn new(settings: Settings) -> Self {
        Self {
            settings,
            resolver: None,
            general: GeneralState {
                current_version_dsp: String::new(),
                current_tab:         CurrentTab::Export,
                found_keysight:      false,
                current_error:       None,
            },
            export: ExportState::default(),
            import: ImportState::default(),
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Keysight Preset Packer - by HeapUnderflow");

            ui.group(|grp| {
                grp.horizontal(|pick_ui| {
                    pick_ui.label("Keysight Path:");
                    pick_ui.text_edit_singleline(&mut self.settings.keysight_path);
                    if pick_ui.button("P").clicked() {
                        if let Some(path) = rfd::FileDialog::new().pick_folder() {
                            self.settings.keysight_path = path.display().to_string();
                        }
                    }

                    if pick_ui.button("Set").clicked() && !self.settings.keysight_path.is_empty() {
                        self.resolver = Some(Resolver::new(&self.settings.keysight_path));
                        self.general.current_version_dsp =
                            format!("{:?}", self.resolver.as_ref().unwrap().identify());
                        if let Err(why) = self.settings.store() {
                            error!(?why, "failed to write settings");
                            self.general.found_keysight = false;
                        } else {
                            self.general.found_keysight = true;

                            match self.resolver.as_ref().unwrap().list_presets() {
                                Ok(mut v) => {
                                    v.insert(0, String::from("[No Preset Selected]"));
                                    self.export.current_selection = 0;
                                    self.export.preset_list = v
                                },
                                Err(why) => self.general.current_error = Some(why),
                            };
                        }
                    };

                    pick_ui.allocate_space(egui::Vec2::new(pick_ui.available_width(), 0.0));
                });

                grp.label(format!(
                    "Keysight Version: {}",
                    self.general.current_version_dsp
                ));
            });

            if let Some(err) = &self.general.current_error {
                ui.scope(|ui| {
                    ui.label(
                        egui::RichText::new(format!("-------------Error-----------\n{:#?}", err))
                            .color(egui::Color32::RED),
                    );
                });
            }

            // ui.group(|ui| {
            //     ui.heading("Event Log");
            //     ui.label("")
            // });

            ui.horizontal(|sel_ui| {
                sel_ui.radio_value(&mut self.general.current_tab, CurrentTab::Export, "Export");
                sel_ui.radio_value(&mut self.general.current_tab, CurrentTab::Import, "Import");
            });

            ui.add_enabled_ui(self.general.found_keysight, |ui| {
                ui.group(|ui| {
                    ui.set_min_size(ui.available_size());
                    egui::ScrollArea::vertical().show(ui, |ui| match self.general.current_tab {
                        CurrentTab::Export => {
                            self.ui_export(ctx, ui);
                        },
                        CurrentTab::Import => {
                            self.ui_import(ctx, ui);
                        },
                    });
                });
            });
        });
    }
}

impl App {
    fn ui_export(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        ui.heading("Export Preset");

        egui::ComboBox::from_id_source("id.select-export-preset-combobox")
            .width(ui.available_width() * 0.8)
            .show_index(
                ui,
                &mut self.export.current_selection,
                self.export.preset_list.len(),
                |i| self.export.preset_list[i].clone(),
            );

        ui.horizontal(|pick_ui| {
            pick_ui.label("Export to:");
            pick_ui.text_edit_singleline(&mut self.export.export_path);
            if pick_ui.button("P").clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Keysight Preset", &[PRESET_EXT])
                    .save_file()
                {
                    self.export.export_path = path.display().to_string();
                }
            }
        });

        if ui
            .add_enabled(self.export.valid(), egui::Button::new("Export"))
            .clicked()
        {
            let mut pck = packer::PresetInfo::new(
                &self.export.preset_list[self.export.current_selection],
                self.resolver.clone().unwrap(),
            );
            pck.load().unwrap();
            pck.pack_to(&ensure_file_ext(&self.export.export_path))
                .unwrap();

            self.export = ExportState {
                export_ok: true,
                ..Default::default()
            };
        }

        if self.export.export_ok {
            egui::Window::new("Export")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.label(
                            egui::RichText::new("Export Successfull!")
                                .color(egui::Color32::GREEN)
                                .size(42.0),
                        );
                        if ui.button("Ok").clicked() {
                            self.export.export_ok = false;
                        }
                    });
                });
        }
    }

    fn ui_import(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        ui.heading("Import Preset");

        ui.horizontal(|pick_ui| {
            pick_ui.label("Import from:");
            pick_ui.text_edit_singleline(&mut self.import.import_path);
            if pick_ui.button("P").clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Keysight Preset", &[PRESET_EXT])
                    .pick_file()
                {
                    self.import.import_path = path.display().to_string();
                }
            }
            if pick_ui.button("Set").clicked()
                && self.import.import_path.len() > 0
                && !self.import.success
            {
                self.import.unpacker = Some(Unpacker::new(
                    self.resolver.clone().unwrap(),
                    &self.import.import_path,
                ));
            }
        });

        ui.add_enabled_ui(self.import.unpacker.is_some(), |ui| {
            if self.import.checked && self.import.preset_conflict {
                ui.label(
                    egui::RichText::new(
                        "The given preset already exists!, press Load again to overwrite it and \
                         its assets.",
                    )
                    .color(egui::Color32::WHITE)
                    .background_color(egui::Color32::DARK_RED),
                );
            }
            if self.import.checked && self.import.conflicts.len() > 0 {
                ui.label(
                    egui::RichText::new(
                        "Encountered conflicting files, press Load again to overwrite",
                    )
                    .color(egui::Color32::WHITE)
                    .background_color(egui::Color32::DARK_RED),
                );
                egui::ScrollArea::vertical().show(ui, |ui| {
                    egui::Grid::new("kspack-import-conflict-list")
                        .striped(true)
                        .num_columns(3)
                        .show(ui, |ui| {
                            ui.label("Name");
                            ui.label("Ext");
                            ui.label("Type");
                            ui.end_row();

                            for entry in self.import.conflicts.iter() {
                                ui.label(&entry.name);
                                ui.label(&entry.ext);
                                ui.label(format!("{:?}", entry.typ));
                                ui.end_row();
                            }
                        });
                });
            }
            if self.import.checked
                && !(self.import.preset_conflict || self.import.conflicts.len() > 0)
            {
                ui.label("No conflicts found, press Load again to import");
            }

            if ui.button("Load").clicked() {
                if !self.import.checked {
                    let (pcf, cf) = self
                        .import
                        .unpacker
                        .as_mut()
                        .unwrap()
                        .conflicts()
                        .expect("fatal");
                    self.import.preset_conflict = pcf;
                    self.import.conflicts = cf;
                    self.import.checked = true;
                } else {
                    self.import
                        .unpacker
                        .as_mut()
                        .unwrap()
                        .unpack()
                        .expect("fatal");
                    self.import = ImportState::default();
                    self.import.success = true;
                }
            }

            if self.import.success {
                egui::Window::new("Import")
                    .collapsible(false)
                    .resizable(false)
                    .show(ctx, |ui| {
                        ui.vertical_centered(|ui| {
                            ui.label(
                                egui::RichText::new("Import Successfull!")
                                    .color(egui::Color32::GREEN)
                                    .size(42.0),
                            );
                            if ui.button("Ok").clicked() {
                                self.import.success = false;
                            }
                        });
                    });
            }
        });
    }
}

fn ensure_file_ext(s: &str) -> Cow<str> {
    if s.ends_with(".kspreset") {
        Cow::Borrowed(s)
    } else {
        Cow::Owned(format!("{}.{}", s, PRESET_EXT))
    }
}
