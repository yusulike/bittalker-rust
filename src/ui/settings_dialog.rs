//! Settings dialog. Port of `src/ui/settings_dialog.py` as an independent native window:
//! language / interval / voice pickers, always-on-top and ticker-mode
//! checkboxes, a Test Voice button and OK / Cancel.

use std::sync::{Arc, Mutex};

use egui::{Color32, ComboBox, Frame, Key, Margin, Ui, ViewportBuilder, ViewportId};

use crate::settings::{available_languages, Settings, VOICES};

pub const DIALOG_WIDTH: f32 = 330.0;
pub const DIALOG_HEIGHT: f32 = 345.0;

#[derive(Clone, Debug, PartialEq)]
pub enum AssetDownloadStatus {
    Idle,
    Downloading {
        file_name: String,
        current_file: usize,
        total_files: usize,
        progress: f32, // 0.0 ..= 1.0
    },
    Success,
    Failed(String),
}

/// Draft settings shown while the dialog is open.
pub struct SettingsDialog {
    pub open: bool,
    pub language: String,
    pub interval: i64,
    pub voice: String,
    pub always_on_top: bool,
    pub ticker_mode: bool,
    pub initial_pos: Option<[f32; 2]>,
    pub download_status: Arc<Mutex<AssetDownloadStatus>>,
}

impl SettingsDialog {
    pub fn open(current: &Settings, parent_rect: Option<egui::Rect>) -> Self {
        Self::open_with_state(current, parent_rect, None)
    }

    pub fn open_with_state(
        current: &Settings,
        parent_rect: Option<egui::Rect>,
        shared_download: Option<Arc<Mutex<AssetDownloadStatus>>>,
    ) -> Self {
        let initial_pos = parent_rect.map(|rect| {
            let x = rect.min.x + (rect.width() - DIALOG_WIDTH) / 2.0;
            let y = if rect.min.y > 450.0 {
                (rect.min.y - DIALOG_HEIGHT - 10.0).max(0.0)
            } else {
                rect.max.y + 10.0
            };
            [x, y]
        });

        Self {
            open: true,
            language: current.language.clone(),
            interval: current.interval,
            voice: current.voice.clone(),
            always_on_top: current.always_on_top,
            ticker_mode: current.ticker_mode,
            initial_pos,
            download_status: shared_download
                .unwrap_or_else(|| Arc::new(Mutex::new(AssetDownloadStatus::Idle))),
        }
    }

    fn draft(&self) -> Settings {
        Settings {
            interval: self.interval,
            voice: self.voice.clone(),
            language: self.language.clone(),
            always_on_top: self.always_on_top,
            muted: false, // filled in by the caller from live state
            ticker_mode: self.ticker_mode,
            window_pos: None,
        }
    }

    /// Show the settings dialog as an independent native OS window.
    /// Returns `Some(new_settings)` when OK is pressed.
    /// `on_test_voice` is invoked with `(voice, language)`.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        muted: bool,
        on_test_voice: &mut dyn FnMut(&str, &str),
    ) -> Option<Settings> {
        if !self.open {
            return None;
        }

        let mut result = None;

        let mut builder = ViewportBuilder::default()
            .with_title("Settings")
            .with_inner_size([DIALOG_WIDTH, DIALOG_HEIGHT])
            .with_min_inner_size([300.0, 280.0])
            .with_resizable(false)
            .with_minimize_button(false)
            .with_maximize_button(false)
            .with_close_button(true);

        if let Some([x, y]) = self.initial_pos {
            builder = builder.with_position([x, y]);
        }

        if self.always_on_top {
            builder = builder.with_always_on_top();
        }

        ctx.show_viewport_immediate(
            ViewportId::from_hash_of("bittalker_settings_dialog"),
            builder,
            |ui, _class| {
                if ui.input(|i| i.viewport().close_requested() || i.key_pressed(Key::Escape)) {
                    self.open = false;
                }

                let frame = Frame::NONE
                    .fill(Color32::from_rgb(0x1e, 0x1e, 0x1e))
                    .inner_margin(Margin::symmetric(14, 12));

                egui::CentralPanel::default().frame(frame).show(ui, |ui| {
                    self.body(ui, &mut result, on_test_voice);
                });
            },
        );

        result.map(|mut s| {
            s.muted = muted;
            s
        })
    }

    fn body(
        &mut self,
        ui: &mut Ui,
        result: &mut Option<Settings>,
        on_test_voice: &mut dyn FnMut(&str, &str),
    ) {
        ui.add_space(4.0);

        egui::Grid::new("settings_grid")
            .num_columns(2)
            .spacing([12.0, 8.0])
            .min_col_width(125.0)
            .show(ui, |ui| {
                ui.label("Language:");
                ComboBox::from_id_salt("language_combo")
                    .selected_text(&self.language)
                    .width(135.0)
                    .show_ui(ui, |ui| {
                        for lang in available_languages() {
                            ui.selectable_value(&mut self.language, lang.to_string(), lang);
                        }
                    });
                ui.end_row();

                ui.label("Notification Interval:");
                ui.add(
                    egui::DragValue::new(&mut self.interval)
                        .range(1..=10000)
                        .speed(10.0)
                        .suffix(" USD"),
                );
                ui.end_row();

                ui.label("TTS Voice:");
                ComboBox::from_id_salt("voice_combo")
                    .selected_text(&self.voice)
                    .width(135.0)
                    .show_ui(ui, |ui| {
                        for voice in VOICES {
                            ui.selectable_value(&mut self.voice, voice.to_string(), *voice);
                        }
                    });
                ui.end_row();

                ui.label("Always on Top:");
                ui.checkbox(&mut self.always_on_top, "Enabled");
                ui.end_row();

                ui.label("Ticker Mode:");
                ui.checkbox(&mut self.ticker_mode, "Enabled");
                ui.end_row();
            });

        ui.add_space(8.0);

        // Test Voice button
        let test = ui.add_sized(
            [ui.available_width(), 26.0],
            egui::Button::new("Test Voice"),
        );
        if test.clicked() {
            on_test_voice(&self.voice, &self.language);
        }

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);

        self.render_assets_section(ui);

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(6.0);

        // OK / Cancel buttons (right-to-left puts Cancel right, OK left -> [OK] [Cancel])
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.add_sized([65.0, 24.0], egui::Button::new("Cancel")).clicked() {
                self.open = false;
            }
            if ui.add_sized([65.0, 24.0], egui::Button::new("OK")).clicked() {
                *result = Some(self.draft());
                self.open = false;
            }
        });
    }

    fn render_assets_section(&mut self, ui: &mut Ui) {
        let assets_dir = crate::paths::assets_dir();
        let assets_installed = crate::assets::verify_assets(&assets_dir).is_ok();

        let current_status = {
            if let Ok(guard) = self.download_status.lock() {
                guard.clone()
            } else {
                AssetDownloadStatus::Idle
            }
        };

        ui.horizontal(|ui| {
            ui.label("Voice Models:");
            if assets_installed {
                ui.colored_label(Color32::from_rgb(0x00, 0xE6, 0x76), "✔ Installed");
            } else {
                ui.colored_label(Color32::from_rgb(0xFF, 0xAA, 0x00), "⚠ Not Installed");
            }
        });

        match current_status {
            AssetDownloadStatus::Idle => {
                let btn_text = if assets_installed {
                    "Re-download Models (~70MB)"
                } else {
                    "⬇ Download Models (~70MB)"
                };
                if ui.add_sized([ui.available_width(), 24.0], egui::Button::new(btn_text)).clicked() {
                    self.start_download(ui.ctx().clone());
                }
            }
            AssetDownloadStatus::Downloading {
                file_name,
                current_file,
                total_files,
                progress,
            } => {
                let overall = ((current_file.saturating_sub(1) as f32 + progress)
                    / total_files.max(1) as f32)
                    .clamp(0.0, 1.0);
                ui.add(
                    egui::ProgressBar::new(overall)
                        .show_percentage()
                        .animate(true),
                );
                ui.label(
                    egui::RichText::new(format!(
                        "[{}/{}] {}",
                        current_file, total_files, file_name
                    ))
                    .size(11.0)
                    .color(Color32::GRAY),
                );
            }
            AssetDownloadStatus::Success => {
                ui.horizontal(|ui| {
                    ui.colored_label(
                        Color32::from_rgb(0x00, 0xE6, 0x76),
                        "✔ Download Complete!",
                    );
                    if ui.button("Re-download").clicked() {
                        self.start_download(ui.ctx().clone());
                    }
                });
            }
            AssetDownloadStatus::Failed(err) => {
                ui.colored_label(Color32::from_rgb(0xFF, 0x55, 0x55), format!("Error: {err}"));
                if ui.button("Retry Download").clicked() {
                    self.start_download(ui.ctx().clone());
                }
            }
        }
    }

    fn start_download(&self, ctx: egui::Context) {
        let status_lock = Arc::clone(&self.download_status);
        if let Ok(mut st) = status_lock.lock() {
            *st = AssetDownloadStatus::Downloading {
                file_name: "Connecting...".to_string(),
                current_file: 0,
                total_files: crate::assets::REQUIRED_FILES.len(),
                progress: 0.0,
            };
        }
        let assets_dir = crate::paths::assets_dir();
        std::thread::spawn(move || {
            let res = crate::assets::download_all_with_progress(&assets_dir, |name, idx, total, pct| {
                if let Ok(mut st) = status_lock.lock() {
                    *st = AssetDownloadStatus::Downloading {
                        file_name: name.to_string(),
                        current_file: idx,
                        total_files: total,
                        progress: pct,
                    };
                }
                ctx.request_repaint();
            });

            if let Ok(mut st) = status_lock.lock() {
                match res {
                    Ok(()) => *st = AssetDownloadStatus::Success,
                    Err(e) => *st = AssetDownloadStatus::Failed(e.to_string()),
                }
            }
            ctx.request_repaint();
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_and_draft() {
        let original = Settings {
            interval: 100,
            voice: "M2".to_string(),
            language: "English".to_string(),
            always_on_top: false,
            muted: true,
            ticker_mode: true,
            window_pos: None,
        };

        let mut dialog = SettingsDialog::open(&original, None);
        assert!(dialog.open);
        assert_eq!(dialog.interval, 100);
        assert_eq!(dialog.voice, "M2");
        assert_eq!(dialog.language, "English");
        assert!(!dialog.always_on_top);
        assert!(dialog.ticker_mode);

        // Modify in dialog
        dialog.interval = 250;
        dialog.voice = "F3".to_string();
        dialog.language = "Korean".to_string();
        dialog.always_on_top = true;
        dialog.ticker_mode = false;

        let mut draft = dialog.draft();
        draft.muted = original.muted; // Caller preserves live muted state

        assert_eq!(draft.interval, 250);
        assert_eq!(draft.voice, "F3");
        assert_eq!(draft.language, "Korean");
        assert!(draft.always_on_top);
        assert!(!draft.ticker_mode);
        assert!(draft.muted);
    }

    #[test]
    fn relative_positioning() {
        let s = Settings::default();
        // Top-half main window (min.y = 100 <= 450) -> opens below
        let top_rect = egui::Rect::from_min_size(egui::pos2(200.0, 100.0), egui::vec2(520.0, 100.0));
        let dlg_top = SettingsDialog::open(&s, Some(top_rect));
        assert!(dlg_top.initial_pos.is_some());
        let [x, y] = dlg_top.initial_pos.unwrap();
        // Centered horizontally: 200 + (520 - DIALOG_WIDTH)/2
        assert_eq!(x, 200.0 + (520.0 - DIALOG_WIDTH) / 2.0);
        // Opens 10px below: 100 + 100 + 10 = 210
        assert_eq!(y, 210.0);

        // Bottom-half main window (min.y = 600 > 450) -> opens above
        let btm_rect = egui::Rect::from_min_size(egui::pos2(200.0, 600.0), egui::vec2(520.0, 100.0));
        let dlg_btm = SettingsDialog::open(&s, Some(btm_rect));
        let [_, y_btm] = dlg_btm.initial_pos.unwrap();
        // Opens 10px above: 600 - DIALOG_HEIGHT - 10
        assert_eq!(y_btm, 600.0 - DIALOG_HEIGHT - 10.0);
    }
}
