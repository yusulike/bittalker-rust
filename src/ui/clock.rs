//! Flip-clock widget. Port of `src/ui/clock_widget.py`: three 64x64 digit
//! panels (H/M/S with suffix letters), an AM/PM label and a date row.

use chrono::{DateTime, Datelike, Local, Timelike};
use egui::{Color32, CornerRadius, FontId, RichText, Sense, Stroke, StrokeKind, Ui, Vec2};

use crate::core::weather::WeatherInfo;
use crate::ui::icons::draw_weather_icon;

const PANEL_BG: Color32 = Color32::from_rgb(0x25, 0x25, 0x25);
const PANEL_BORDER: Color32 = Color32::from_rgb(0x33, 0x33, 0x33);
const DATE_TEXT: Color32 = Color32::from_rgb(0x88, 0x88, 0x88);
const SUFFIX_TEXT: Color32 = Color32::from_rgb(0xaa, 0xaa, 0xaa);

pub const PANEL_SIZE: f32 = 64.0;
pub const PANEL_GAP: f32 = 8.0;
pub const AMPM_WIDTH: f32 = 36.0;
pub const CLOCK_WIDTH: f32 = (PANEL_SIZE * 3.0) + (PANEL_GAP * 2.0) + AMPM_WIDTH; // 244.0
pub const CLOCK_HEIGHT: f32 = 82.0;
pub const CLOCK_ROW_GAP_Y: f32 = 5.0;

/// Side Column (AM/PM + Weather) Layout
pub const SIDE_COLUMN_TOP_PAD: f32 = 4.0; // 상단 여백 (값을 늘리면 전체가 아래로 내려옵니다)
pub const AMPM_WEATHER_GAP_Y: f32 = 8.0; // AM/PM과 날씨 아이콘 사이의 간격
pub const WEATHER_TEMP_GAP_Y: f32 = 4.0; // 날씨 아이콘과 기온 사이의 수직 간격
pub const SIDE_COLUMN_FALLBACK_PAD: f32 = 24.0; // 날씨 데이터가 없을 때 AM/PM 상단 여백
pub const WEATHER_ICON_SIZE: f32 = 16.0; // 날씨 아이콘 크기
pub const AMPM_FONT_SIZE: f32 = 14.0; // AM/PM 폰트 크기
pub const TEMP_FONT_SIZE: f32 = 12.0; // 기온 폰트 크기

pub fn show_clock(ui: &mut Ui, now: DateTime<Local>, weather: Option<&WeatherInfo>) {
    ui.allocate_ui_with_layout(
        Vec2::new(CLOCK_WIDTH, CLOCK_HEIGHT),
        egui::Layout::top_down(egui::Align::Center),
        |ui| {
            ui.spacing_mut().item_spacing.y = CLOCK_ROW_GAP_Y;

            // --- time row ---
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = PANEL_GAP;

                let (h, ampm) = format12h(now);
                digit_panel(ui, &h, "H");
                digit_panel(ui, &now.format("%M").to_string(), "M");
                digit_panel(ui, &now.format("%S").to_string(), "S");

                // Side column: AM/PM + Weather Icon + Temperature (3 rows)
                ui.allocate_ui_with_layout(
                    Vec2::new(AMPM_WIDTH, PANEL_SIZE),
                    egui::Layout::top_down(egui::Align::Center),
                    |ui| {
                        if weather.is_some() {
                            ui.add_space(SIDE_COLUMN_TOP_PAD);
                        } else {
                            ui.add_space(SIDE_COLUMN_FALLBACK_PAD);
                        }

                        // Row 1: AM / PM
                        ui.add(
                            egui::Label::new(
                                RichText::new(ampm)
                                    .color(Color32::WHITE)
                                    .font(FontId::proportional(AMPM_FONT_SIZE))
                                    .strong(),
                            )
                            .selectable(false),
                        );

                        // Row 2 & 3: Weather Icon & Temperature
                        if let Some(w) = weather {
                            ui.add_space(AMPM_WEATHER_GAP_Y);
                            ui.spacing_mut().item_spacing.y = WEATHER_TEMP_GAP_Y;

                            let tooltip_text = format!(
                                "{}: {} ({:.1}°C)",
                                w.location_name,
                                w.condition.description(),
                                w.temperature
                            );

                            let (rect, resp) = ui.allocate_exact_size(
                                Vec2::splat(WEATHER_ICON_SIZE),
                                Sense::hover(),
                            );
                            draw_weather_icon(&ui.painter_at(rect), rect, w.condition);

                            let temp_resp = ui.add(
                                egui::Label::new(
                                    RichText::new(format!("{:.0}°C", w.temperature))
                                        .color(DATE_TEXT)
                                        .font(FontId::proportional(TEMP_FONT_SIZE)),
                                )
                                .selectable(false),
                            );

                            resp.union(temp_resp).on_hover_text(tooltip_text);
                        }
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
        FontId::proportional(44.0),
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
    (
        format!("{h:02}"),
        if is_pm { "PM" } else { "AM" }.to_string(),
    )
}
