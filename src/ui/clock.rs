//! Flip-clock widget: three 42x42 compact digit panels (H/M/S with suffix letters),
//! an AM/PM label and a concise date row.

use chrono::{DateTime, Datelike, Local, Timelike};
use egui::{Color32, CornerRadius, FontId, RichText, Sense, Stroke, StrokeKind, Ui, Vec2};

const PANEL_BG: Color32 = Color32::from_rgb(0x20, 0x20, 0x20);
const PANEL_BORDER: Color32 = Color32::from_rgb(0x34, 0x34, 0x34);
const DATE_TEXT: Color32 = Color32::from_rgb(0x82, 0x82, 0x82);
const SUFFIX_TEXT: Color32 = Color32::from_rgb(0x66, 0x66, 0x66);

pub fn show_clock(ui: &mut Ui, now: DateTime<Local>) {
    let panel_size = 42.0;
    let clock_width = (panel_size * 3.0) + (5.0 * 2.0) + 26.0; // 3 panels + 2 spacings + AM/PM
    ui.allocate_ui_with_layout(
        Vec2::new(clock_width, 56.0),
        egui::Layout::top_down(egui::Align::Center),
        |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;

            // --- time row ---
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 5.0;

                let (h, ampm) = format12h(now);
                digit_panel(ui, &h, "H", panel_size);
                digit_panel(ui, &now.format("%M").to_string(), "M", panel_size);
                digit_panel(ui, &now.format("%S").to_string(), "S", panel_size);

                // AM/PM (centered vertically relative to the 42px panels)
                ui.allocate_ui_with_layout(
                    Vec2::new(24.0, panel_size),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        ui.label(
                            RichText::new(ampm)
                                .color(Color32::from_rgb(0xee, 0xee, 0xee))
                                .font(FontId::proportional(11.5))
                                .strong(),
                        );
                    },
                );
            });

            // --- date row ---
            let date = format!(
                "{}, {} {}",
                now.format("%a"),
                now.format("%b"),
                now.day()
            );
            ui.label(
                RichText::new(date)
                    .color(DATE_TEXT)
                    .font(FontId::proportional(10.5)),
            );
        },
    );
}

/// One compact digit panel with a small unit suffix in the bottom-right corner.
fn digit_panel(ui: &mut Ui, digit: &str, suffix: &str, size_px: f32) {
    let size = Vec2::splat(size_px);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, CornerRadius::same(6), PANEL_BG);
    painter.rect_stroke(
        rect,
        CornerRadius::same(6),
        Stroke::new(1.0, PANEL_BORDER),
        StrokeKind::Middle,
    );

    // digit, centered
    painter.text(
        rect.center() + egui::vec2(0.0, -0.5),
        egui::Align2::CENTER_CENTER,
        digit,
        FontId::monospace(21.0),
        Color32::WHITE,
    );

    // suffix, bottom-right
    painter.text(
        rect.right_bottom() + egui::vec2(-3.0, -1.0),
        egui::Align2::RIGHT_BOTTOM,
        suffix,
        FontId::proportional(8.5),
        SUFFIX_TEXT,
    );
}

/// (12-hour clock value, AM/PM) matching QTime handling in the original.
pub fn format12h(now: DateTime<Local>) -> (String, String) {
    let is_pm = now.hour() >= 12;
    let mut h = now.hour() % 12;
    if h == 0 {
        h = 12;
    }
    (format!("{h:02}"), if is_pm { "PM" } else { "AM" }.to_string())
}
