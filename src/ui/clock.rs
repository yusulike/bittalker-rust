//! Flip-clock widget. Port of `src/ui/clock_widget.py`: three 64x64 digit
//! panels (H/M/S with suffix letters), an AM/PM label and a date row.

use chrono::{DateTime, Local, Datelike, Timelike};
use egui::{Color32, CornerRadius, FontId, RichText, Sense, Stroke, StrokeKind, Ui, Vec2};

const PANEL_BG: Color32 = Color32::from_rgb(0x25, 0x25, 0x25);
const PANEL_BORDER: Color32 = Color32::from_rgb(0x33, 0x33, 0x33);
const DATE_TEXT: Color32 = Color32::from_rgb(0x88, 0x88, 0x88);
const SUFFIX_TEXT: Color32 = Color32::from_rgb(0xaa, 0xaa, 0xaa);

pub const PANEL_SIZE: f32 = 64.0;
pub const PANEL_GAP: f32 = 8.0;
pub const AMPM_WIDTH: f32 = 36.0;
pub const CLOCK_WIDTH: f32 = (PANEL_SIZE * 3.0) + (PANEL_GAP * 2.0) + AMPM_WIDTH; // 244.0
pub const CLOCK_HEIGHT: f32 = 82.0;

pub fn show_clock(ui: &mut Ui, now: DateTime<Local>) {
    ui.allocate_ui_with_layout(
        Vec2::new(CLOCK_WIDTH, CLOCK_HEIGHT),
        egui::Layout::top_down(egui::Align::Center),
        |ui| {
            ui.spacing_mut().item_spacing.y = 3.0;

            // --- time row ---
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = PANEL_GAP;

                let (h, ampm) = format12h(now);
                digit_panel(ui, &h, "H");
                digit_panel(ui, &now.format("%M").to_string(), "M");
                digit_panel(ui, &now.format("%S").to_string(), "S");

                // AM/PM (centered vertically relative to the 64px panels)
                ui.allocate_ui_with_layout(
                    Vec2::new(30.0, PANEL_SIZE),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        ui.add(
                            egui::Label::new(
                                RichText::new(ampm)
                                    .color(Color32::WHITE)
                                    .font(FontId::proportional(14.0))
                                    .strong(),
                            )
                            .selectable(false),
                        );
                    },
                );
            });

            // --- date row ---
            let date = format!(
                "{}, {} {}, {}",
                now.format("%A"),
                now.format("%B"),
                now.day(),
                now.year()
            );
            ui.add(
                egui::Label::new(
                    RichText::new(date)
                        .color(DATE_TEXT)
                        .font(FontId::proportional(13.0)),
                )
                .selectable(false),
            );
        },
    );
}

/// One 64x64 digit panel with a small unit suffix in the bottom-right corner.
fn digit_panel(ui: &mut Ui, digit: &str, suffix: &str) {
    let size = Vec2::splat(64.0);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, CornerRadius::same(8), PANEL_BG);
    painter.rect_stroke(
        rect,
        CornerRadius::same(8),
        Stroke::new(1.0, PANEL_BORDER),
        StrokeKind::Middle,
    );

    // digit, centered
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        digit,
        FontId::proportional(29.0),
        Color32::WHITE,
    );

    // suffix, bottom-right
    painter.text(
        rect.right_bottom() + egui::vec2(-4.0, -1.0),
        egui::Align2::RIGHT_BOTTOM,
        suffix,
        FontId::proportional(10.0),
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
