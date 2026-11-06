//! Settings dialog. Port of `src/ui/settings_dialog.py` as an independent native window:
//! language / interval / voice pickers, always-on-top and ticker-mode
//! checkboxes, a Test Voice button and OK / Cancel.

use egui::{Color32, ComboBox, Frame, Key, Margin, Ui, ViewportBuilder, ViewportId};

use crate::settings::{available_languages, Settings, VOICES};

/// Draft settings shown while the dialog is open.
pub struct SettingsDialog {
    pub open: bool,
    pub language: String,
    pub interval: i64,
    pub voice: String,
    pub always_on_top: bool,
    pub ticker_mode: bool,
    pub initial_pos: Option<[f32; 2]>,
}

impl SettingsDialog {
    pub fn open(current: &Settings, parent_rect: Option<egui::Rect>) -> Self {
        let initial_pos = parent_rect.map(|rect| {
            let dialog_w = 330.0;
            let dialog_h = 270.0;
            let x = rect.min.x + (rect.width() - dialog_w) / 2.0;
            let y = if rect.min.y > 450.0 {
                (rect.min.y - dialog_h - 10.0).max(0.0)
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
            .with_inner_size([330.0, 270.0])
            .with_min_inner_size([300.0, 240.0])
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

        ui.add_space(10.0);

        // Test Voice button
        let test = ui.add_sized(
            [ui.available_width(), 26.0],
            egui::Button::new("Test Voice"),
        );
        if test.clicked() {
            on_test_voice(&self.voice, &self.language);
        }

        ui.add_space(10.0);

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
        // Centered horizontally: 200 + (520 - 330)/2 = 295
        assert_eq!(x, 295.0);
        // Opens 10px below: 100 + 100 + 10 = 210
        assert_eq!(y, 210.0);

        // Bottom-half main window (min.y = 600 > 450) -> opens above
        let btm_rect = egui::Rect::from_min_size(egui::pos2(200.0, 600.0), egui::vec2(520.0, 100.0));
        let dlg_btm = SettingsDialog::open(&s, Some(btm_rect));
        let [_, y_btm] = dlg_btm.initial_pos.unwrap();
        // Opens 10px above: 600 - 270 - 10 = 320
        assert_eq!(y_btm, 320.0);
    }
}
