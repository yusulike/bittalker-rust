//! Main window. Port of `src/ui/main_window.py` on egui:
//! frameless dark ticker window with two layouts (normal / compact ticker),
//! live price + direction indicator, connection status pill, mute / settings
//! / close buttons, hourly chime and threshold-crossing voice alerts.

use std::time::Duration;

use chrono::{DateTime, Local, Timelike};
use eframe::egui;
use egui::{Color32, CornerRadius, FontId, RichText, Sense, Stroke, Vec2};

use crate::core::interval::{Direction, IntervalTracker};
use crate::core::monitor::{MonitorEvent, PriceMonitor};
use crate::i18n::{announcement_text, voice_test_text};
use crate::settings::{lang_code, Settings, SettingsManager};
use crate::tts::service::{TtsEvent, TtsService};
use crate::ui::clock::show_clock;
use crate::ui::icons::{icon_button, IconKind};
use crate::ui::settings_dialog::SettingsDialog;

const BG: Color32 = Color32::from_rgb(0x12, 0x12, 0x12);
const BORDER: Color32 = Color32::from_rgb(0x44, 0x44, 0x44);
const SEPARATOR: Color32 = Color32::from_rgb(0x44, 0x44, 0x44);
const GREEN: Color32 = Color32::from_rgb(0x00, 0xE6, 0x76);
const RED: Color32 = Color32::from_rgb(0xFF, 0x52, 0x52);
const GRAY_TEXT: Color32 = Color32::from_rgb(0x88, 0x88, 0x88);
const DATE_TEXT: Color32 = Color32::from_rgb(0xaa, 0xaa, 0xaa);

#[derive(Clone, Copy, PartialEq)]
enum Status {
    Connecting,
    Connected,
    Disconnected,
    TtsError,
}

impl Status {
    fn text(self) -> &'static str {
        match self {
            Status::Connecting => "Connecting...",
            Status::Connected => "Connected",
            Status::Disconnected => "Disconnected",
            Status::TtsError => "TTS Error",
        }
    }

    fn color(self) -> Color32 {
        match self {
            Status::Connecting => GRAY_TEXT,
            Status::Connected => GREEN,
            Status::Disconnected => RED,
            Status::TtsError => RED,
        }
    }
}

pub struct BitTalkerApp {
    settings: SettingsManager,
    // live mirror of the settings the UI can change
    current_voice: String,
    current_language: String,
    always_on_top: bool,
    ticker_mode: bool,
    is_muted: bool,

    monitor: PriceMonitor,
    monitor_rx: Option<std::sync::mpsc::Receiver<MonitorEvent>>,

    tracker: IntervalTracker,
    tts: TtsService,

    price_text: String,
    indicator: Option<Direction>,
    status: Status,
    current_hour: Option<u32>,

    dialog: Option<SettingsDialog>,

    // viewport commands deferred to the next update (sent after layout)
    pending_resize: Option<Vec2>,
    pending_level: Option<bool>,

    screenshot_path: Option<std::path::PathBuf>,
    screenshot_frames: usize,

    current_window_rect: Option<egui::Rect>,
    pending_initial_pos: Option<[f32; 2]>,
}

impl BitTalkerApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        Self::new_with_options(cc, None, None, false)
    }

    pub fn new_with_options(
        cc: &eframe::CreationContext<'_>,
        screenshot_path: Option<std::path::PathBuf>,
        override_ticker_mode: Option<bool>,
        open_settings_dialog: bool,
    ) -> Self {
        install_fonts(&cc.egui_ctx);

        let settings = SettingsManager::new(crate::paths::settings_path());
        let s = settings.get().clone();

        let ctx = cc.egui_ctx.clone();
        let mut monitor = PriceMonitor::start("btcusdt", ctx.clone());
        let monitor_rx = monitor.take_receiver();

        let tts = TtsService::new(
            crate::paths::onnx_dir(),
            crate::paths::voice_styles_dir(),
            crate::paths::cache_dir(),
            ctx,
        );

        // match the dark compact look of the original stylesheet
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = Color32::TRANSPARENT;
        visuals.window_fill = Color32::from_rgb(0x18, 0x18, 0x18);
        visuals.extreme_bg_color = Color32::from_rgb(0x25, 0x25, 0x25);
        cc.egui_ctx.set_visuals(visuals);

        let ticker_mode = override_ticker_mode.unwrap_or(s.ticker_mode);
        let dialog = if open_settings_dialog {
            Some(SettingsDialog::open(&s, None))
        } else {
            None
        };

        Self {
            current_voice: s.voice.clone(),
            current_language: s.language.clone(),
            always_on_top: s.always_on_top,
            ticker_mode,
            is_muted: s.muted,
            settings,
            monitor,
            monitor_rx,
            tracker: IntervalTracker::new(s.interval as f64),
            tts,
            price_text: "Waiting...".to_string(),
            indicator: None,
            status: Status::Connecting,
            current_hour: None,
            dialog,
            pending_resize: None,
            pending_level: None,
            screenshot_path,
            screenshot_frames: 0,
            current_window_rect: None,
            pending_initial_pos: s.window_pos,
        }
    }

    // ---------- event handling (pyqtSlot ports) ----------

    fn poll_monitor(&mut self) {
        let mut events = Vec::new();
        if let Some(rx) = &self.monitor_rx {
            while let Ok(event) = rx.try_recv() {
                events.push(event);
            }
        }
        for event in events {
            match event {
                MonitorEvent::Price(price) => self.on_price_update(price),
                MonitorEvent::Connected(connected) => {
                    self.status = if connected {
                        Status::Connected
                    } else {
                        Status::Disconnected
                    };
                }
            }
        }
    }

    fn poll_tts_events(&mut self) {
        for event in self.tts.poll_events() {
            let TtsEvent::Error(msg) = event;
            self.status = Status::TtsError;
            println!("[UI] TTS Error: {msg}");
        }
    }

    fn on_price_update(&mut self, price: f64) {
        // Get previous price for direction indicator
        let prev_price = self.tracker.last_price;

        // Update price text (fixed position)
        self.price_text = format_price(price);

        // Update indicator separately (fixed width, no layout shift)
        if let Some(prev) = prev_price {
            if price > prev {
                self.indicator = Some(Direction::Up);
            } else if price < prev {
                self.indicator = Some(Direction::Down);
            }
            // else: keep previous indicator
        }

        for (boundary, direction) in self.tracker.process_price(price) {
            self.on_interval_crossed(boundary, direction);
        }
    }

    fn on_interval_crossed(&mut self, price: f64, direction: Direction) {
        let lang = lang_code(&self.current_language).to_string();
        let price_int = price as i64;
        let text = announcement_text(&self.current_language, price_int, direction);

        println!("Triggering TTS: {text} ({}, {lang})", self.current_voice);

        // Skip TTS if muted
        if !self.is_muted {
            self.tts.speak(text, self.current_voice.clone(), lang, true);
        }
    }

    fn run_voice_test(&mut self, voice: &str, language: &str) {
        let lang = lang_code(language).to_string();
        let text = voice_test_text(language, self.tracker.last_price);
        // Speak with cache=false
        self.tts.speak(text, voice.to_string(), lang, false);
    }

    fn toggle_mute(&mut self) {
        self.is_muted = !self.is_muted;
        let mut s = self.settings.get().clone();
        s.muted = self.is_muted;
        self.settings.update(s);
        println!("TTS Muted: {}", self.is_muted);
    }

    /// Hourly chime check (runs on every repaint; ~250ms cadence).
    fn tick_clock(&mut self, now: DateTime<Local>) {
        let current_hour = now.hour();
        if let Some(prev) = self.current_hour {
            if current_hour != prev {
                self.tts.chime();
            }
        }
        self.current_hour = Some(current_hour);
    }

    fn apply_new_settings(&mut self, new: Settings) {
        // Update interval (resets boundary state like the original)
        self.tracker.set_interval(new.interval as f64);

        self.current_voice = new.voice.clone();
        self.current_language = new.language.clone();

        // Persist (muted is kept at its live value, like the original which
        // only saves it on the mute toggle)
        let mut to_save = new.clone();
        to_save.muted = self.is_muted;
        to_save.window_pos = crate::ui::window_manager::platform::get_physical_window_pos()
            .or_else(|| self.current_window_rect.map(|r| [r.min.x, r.min.y]))
            .or(self.settings.get().window_pos);
        self.settings.update(to_save);

        if new.ticker_mode != self.ticker_mode {
            self.ticker_mode = new.ticker_mode;
            self.pending_resize = Some(if self.ticker_mode {
                Vec2::new(540.0, 45.0)
            } else {
                Vec2::new(375.0, 64.0)
            });
        }

        if new.always_on_top != self.always_on_top {
            self.always_on_top = new.always_on_top;
            self.pending_level = Some(self.always_on_top);
        }
    }

    fn save_window_pos(&mut self) {
        let pos = crate::ui::window_manager::platform::get_physical_window_pos()
            .or_else(|| self.current_window_rect.map(|r| [r.min.x, r.min.y]));
        if let Some(pos) = pos {
            if self.settings.get().window_pos != Some(pos) {
                let mut s = self.settings.get().clone();
                s.window_pos = Some(pos);
                self.settings.update(s);
            }
        }
    }

    fn open_settings(&mut self) {
        if self.dialog.is_some() {
            return;
        }
        let mut draft = self.settings.get().clone();
        draft.voice = self.current_voice.clone();
        draft.language = self.current_language.clone();
        draft.always_on_top = self.always_on_top;
        draft.ticker_mode = self.ticker_mode;
        draft.interval = self.tracker.interval as i64;
        self.dialog = Some(SettingsDialog::open(&draft, self.current_window_rect));
    }
}

impl eframe::App for BitTalkerApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.current_window_rect = ui.input(|i| i.viewport().outer_rect);

        if let Some(pos) = self.pending_initial_pos.take() {
            let (w, h) = if self.ticker_mode {
                (540.0, 45.0)
            } else {
                (375.0, 64.0)
            };
            crate::ui::window_manager::platform::set_physical_window_pos(pos, w, h);
        }

        self.poll_monitor();
        self.poll_tts_events();

        // consume pending viewport commands
        if let Some(size) = self.pending_resize.take() {
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
        }
        if let Some(always) = self.pending_level.take() {
            ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(
                if always {
                    egui::WindowLevel::AlwaysOnTop
                } else {
                    egui::WindowLevel::Normal
                },
            ));
        }

        let now = Local::now();
        self.tick_clock(now);

        let frame = egui::Frame::NONE
            .fill(BG)
            .stroke(Stroke::new(1.0, BORDER))
            .corner_radius(CornerRadius::same(10))
            .inner_margin(if self.ticker_mode {
                egui::Margin::symmetric(10, 2)
            } else {
                egui::Margin::symmetric(7, 4)
            });

        egui::CentralPanel::default()
            .frame(frame)
            .show(ui, |ui| {
                // Background drag region, registered first so real widgets
                // (buttons) stay on top for hit-testing
                let bg_rect = ui.max_rect();
                let drag = ui.interact(bg_rect, egui::Id::unique("window-drag"), Sense::drag());
                if drag.drag_started() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }
                if drag.drag_stopped() {
                    self.save_window_pos();
                }

                if self.ticker_mode {
                    self.ticker_layout(ui, now);
                } else {
                    self.normal_layout(ui, now);
                }
            });

        // Settings dialog (taken out of self so the test-voice callback can
        // borrow the app)
        if let Some(mut dialog) = self.dialog.take() {
            let result = dialog.show(&ctx, self.is_muted, &mut |voice: &str, lang: &str| {
                self.run_voice_test(voice, lang);
            });
            if let Some(new_settings) = result {
                self.apply_new_settings(new_settings);
            }
            if dialog.open {
                self.dialog = Some(dialog);
            }
        }

        // Screenshot automation
        if self.screenshot_path.is_some() {
            self.screenshot_frames += 1;
            let ready = self.tracker.last_price.is_some() || self.screenshot_frames >= 25;
            if ready {
                if let Some(path) = self.screenshot_path.take() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::ScreenshotCallback::new(
                        move |image: std::sync::Arc<egui::ColorImage>| {
                            let _ = image::save_buffer(
                                &path,
                                image.as_raw(),
                                image.size[0] as u32,
                                image.size[1] as u32,
                                image::ExtendedColorType::Rgba8,
                            );
                            println!("[Screenshot] Saved to {}", path.display());
                            std::process::exit(0);
                        },
                    )));
                }
            } else {
                ctx.request_repaint_after(Duration::from_millis(50));
            }
        }

        // Keep the clock/chime ticking without external events
        ctx.request_repaint_after(Duration::from_millis(250));
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.save_window_pos();
        self.monitor.stop();
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        egui::Rgba::TRANSPARENT.to_array()
    }
}

// ---------- layouts ----------

impl BitTalkerApp {
    fn price_label(&self, ui: &mut egui::Ui, font_size: f32) -> egui::Response {
        let text = RichText::new(&self.price_text)
            .color(GREEN)
            .font(FontId::monospace(font_size))
            .strong();
        ui.label(text)
    }

    /// Direction indicator with a fixed width so the layout never shifts.
    fn indicator_label(&self, ui: &mut egui::Ui, font_size: f32) {
        let (glyph, color) = match self.indicator {
            Some(Direction::Up) => ("▲", GREEN),
            Some(Direction::Down) => ("▼", RED),
            None => ("", GREEN),
        };
        let (rect, _) = ui.allocate_exact_size(Vec2::new(25.0, font_size), Sense::hover());
        ui.painter().text(
            rect.left_center(),
            egui::Align2::LEFT_CENTER,
            glyph,
            FontId::proportional(font_size),
            color,
        );
    }

    fn status_pill(&self, ui: &mut egui::Ui) {
        let color = self.status.color();
        let galley = ui
            .painter()
            .layout_no_wrap(self.status.text().to_string(), FontId::proportional(10.0), color);
        let pad = Vec2::new(6.0, 1.5);
        let size = galley.size() + pad * 2.0;
        let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, CornerRadius::same(4), Color32::from_rgb(0x1e, 0x1e, 0x1e));
        painter.rect_stroke(rect, CornerRadius::same(4), Stroke::new(1.0, color), egui::StrokeKind::Inside);
        let pos = rect.left_top() + pad;
        painter.galley(pos, galley, color);
    }

    fn buttons(&mut self, ui: &mut egui::Ui, small: bool) {
        let size = if small { Vec2::new(22.0, 18.0) } else { Vec2::new(24.0, 18.0) };
        if icon_button(
            ui,
            egui::Id::unique("mute"),
            IconKind::Speaker { muted: self.is_muted },
            "Mute/Unmute TTS",
            size,
        )
        .clicked()
        {
            self.toggle_mute();
        }
        if icon_button(ui, egui::Id::unique("settings"), IconKind::Gear, "Settings", size).clicked() {
            self.open_settings();
        }
        if icon_button(ui, egui::Id::unique("close"), IconKind::Close, "Close", size).clicked() {
            self.save_window_pos();
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    /// TICKER MODE: single-line layout `price ▲ | 12:00:00 PM Thu, Nov 06 | btn btn ✕`
    fn ticker_layout(&mut self, ui: &mut egui::Ui, now: DateTime<Local>) {
        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 10.0;

            // Price with indicator
            self.price_label(ui, 18.0);
            self.indicator_label(ui, 14.0);

            vseparator(ui, 16.0);

            // Time and Date (horizontal, side by side)
            ui.label(
                RichText::new(now.format("%I:%M:%S %p").to_string())
                    .color(Color32::WHITE)
                    .font(FontId::monospace(18.0)),
            );
            ui.label(
                RichText::new(now.format("%a, %b %d").to_string())
                    .color(DATE_TEXT)
                    .font(FontId::proportional(14.0)),
            );

            vseparator(ui, 16.0);

            // Buttons (no status label in ticker mode)
            self.buttons(ui, true);
        });
    }

    /// NORMAL MODE: compact clock | price + info rows
    fn normal_layout(&mut self, ui: &mut egui::Ui, now: DateTime<Local>) {
        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 8.0;

            // Left: compact clock
            show_clock(ui, now);

            vseparator(ui, 46.0);

            // Right: price + info
            let right_width = ui.available_width();
            ui.allocate_ui_with_layout(
                Vec2::new(right_width, 54.0),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;

                    // Price row
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        self.price_label(ui, 24.0);
                        self.indicator_label(ui, 15.0);
                    });

                    // Info row (status + buttons)
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        self.status_pill(ui);
                        self.buttons(ui, true);
                    });
                },
            );
        });
    }
}

fn vseparator(ui: &mut egui::Ui, height: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(1.0, height.max(1.0)), Sense::hover());
    ui.painter()
        .rect_filled(rect, CornerRadius::ZERO, SEPARATOR);
}

// ---------- helpers ----------

/// `f"${price:,.2f}"` (Python format) equivalent.
pub fn format_price(price: f64) -> String {
    let neg = price < 0.0;
    let cents = (price.abs() * 100.0).round() as i64;
    let int_part = cents / 100;
    let frac = cents % 100;
    let int_str = thousands(int_part);
    format!("{}${}.{:02}", if neg { "-" } else { "" }, int_str, frac)
}

fn thousands(n: i64) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    let digits = s.len();
    for (i, c) in s.chars().enumerate() {
        out.push(c);
        let rem = digits - i - 1;
        if rem > 0 && rem % 3 == 0 {
            out.push(',');
        }
    }
    out
}

fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let mut installed_mono = Vec::new();
    let mut installed_prop = Vec::new();

    // High quality modern font priority:
    // 1. Bahnschrift (crisp German DIN 1451 geometric numerals for price & clock)
    // 2. Segoe UI Bold / Segoe UI (clean modern typography)
    // 3. Malgun Gothic (Korean typography fallback)
    // 4. Segoe UI Symbol (icons/symbols)
    let candidates: &[(&str, &str, bool, bool)] = &[
        ("bahnschrift", "C:\\Windows\\Fonts\\bahnschrift.ttf", true, false),
        ("segoe_bold", "C:\\Windows\\Fonts\\segoeuib.ttf", false, true),
        ("segoe", "C:\\Windows\\Fonts\\segoeui.ttf", false, true),
        ("malgun_bold", "C:\\Windows\\Fonts\\malgunbd.ttf", true, true),
        ("malgun", "C:\\Windows\\Fonts\\malgun.ttf", true, true),
        ("segoe_symbol", "C:\\Windows\\Fonts\\seguisym.ttf", true, true),
    ];
    for &(name, path, is_mono, is_prop) in candidates {
        if let Ok(bytes) = std::fs::read(path) {
            fonts
                .font_data
                .insert(name.to_string(), std::sync::Arc::new(egui::FontData::from_owned(bytes)));
            if is_prop {
                installed_prop.push(name.to_string());
            }
            if is_mono {
                installed_mono.push(name.to_string());
            }
        }
    }

    if let Some(list) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
        for name in installed_prop.iter().rev() {
            list.insert(0, name.clone());
        }
    }
    if let Some(list) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
        for name in installed_mono.iter().rev() {
            list.insert(0, name.clone());
        }
    }
    ctx.set_fonts(fonts);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn price_format_matches_python() {
        assert_eq!(format_price(95500.12), "$95,500.12");
        assert_eq!(format_price(95500.0), "$95,500.00");
        assert_eq!(format_price(1234567.891), "$1,234,567.89");
        assert_eq!(format_price(0.5), "$0.50");
    }
}
