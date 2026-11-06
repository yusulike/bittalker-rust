//! Small painter-drawn button icons (speaker / gear / close).
//! Drawn with the egui painter instead of emoji glyphs so they render
//! identically regardless of installed fonts.

use egui::{Color32, CornerRadius, Painter, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2};

use crate::core::weather::WeatherCondition;

pub fn icon_button(
    ui: &mut Ui,
    _id: egui::Id,
    kind: IconKind,
    tooltip: &str,
    size: Vec2,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());

    let hovered = response.hovered();
    let pressed = response.is_pointer_button_down_on();

    let (bg, fg) = if pressed {
        (color(0x333333), Color32::WHITE)
    } else if hovered {
        if kind == IconKind::Close {
            (color(0x3a2a2a), color(0xFF8888))
        } else {
            (color(0x2a2a2a), color(0xaaaaaa))
        }
    } else {
        (color(0x222222), color(0x888888))
    };

    let painter = ui.painter_at(rect);
    let round = CornerRadius::same(12);
    painter.rect_filled(rect, round, bg);
    let border = if kind == IconKind::Close && pressed {
        Color32::WHITE
    } else {
        color(0x333333)
    };
    painter.rect_stroke(rect, round, Stroke::new(1.0, border), StrokeKind::Middle);

    let inset = 6.0;
    let icon_rect = Rect::from_min_max(
        rect.min + Vec2::new(inset, inset * 0.6),
        rect.max - Vec2::new(inset, inset * 0.6),
    );
    let p = ui.painter_at(icon_rect);
    match kind {
        IconKind::Speaker { muted } => draw_speaker(&p, icon_rect, fg, muted),
        IconKind::Gear => draw_gear(&p, icon_rect, fg),
        IconKind::Close => draw_close(&p, icon_rect, fg),
    }

    response.on_hover_text(tooltip)
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum IconKind {
    Speaker { muted: bool },
    Gear,
    Close,
}

fn color(hex: u32) -> Color32 {
    Color32::from_rgb(
        ((hex >> 16) & 0xff) as u8,
        ((hex >> 8) & 0xff) as u8,
        (hex & 0xff) as u8,
    )
}

/// Speaker with sound waves (or a slash when muted).
fn draw_speaker(p: &Painter, rect: Rect, color: Color32, muted: bool) {
    let stroke = Stroke::new(1.6, color);
    let w = rect.width();
    let h = rect.height();

    // cabinet: small rect
    let cab_w = w * 0.16;
    let cab_h = h * 0.34;
    let cab_rect = Rect::from_min_size(
        Pos2::new(rect.left() + w * 0.06, rect.center().y - cab_h / 2.0),
        Vec2::new(cab_w, cab_h),
    );
    p.rect_stroke(cab_rect, CornerRadius::ZERO, stroke, StrokeKind::Middle);

    // cone: triangle from cabinet to the right
    let cone_tip_top = Pos2::new(cab_rect.right(), rect.center().y - h * 0.22);
    let cone_tip_bottom = Pos2::new(cab_rect.right(), rect.center().y + h * 0.22);
    let cone_right = Pos2::new(rect.left() + w * 0.42, rect.center().y - h * 0.42);
    let cone_right2 = Pos2::new(rect.left() + w * 0.42, rect.center().y + h * 0.42);
    p.add(egui::Shape::line(
        vec![cone_tip_top, cone_right, cone_right2, cone_tip_bottom],
        stroke,
    ));

    if muted {
        // X over the wave area
        let cx = rect.left() + w * 0.66;
        let arm = h * 0.18;
        p.line_segment(
            [
                Pos2::new(cx - arm, rect.center().y - arm),
                Pos2::new(cx + arm, rect.center().y + arm),
            ],
            stroke,
        );
        p.line_segment(
            [
                Pos2::new(cx + arm, rect.center().y - arm),
                Pos2::new(cx - arm, rect.center().y + arm),
            ],
            stroke,
        );
    } else {
        // two sound arcs, approximated with polylines (no arc shape in 0.37)
        let center = Pos2::new(rect.left() + w * 0.44, rect.center().y);
        for r in [h * 0.22, h * 0.40] {
            let mut pts = Vec::new();
            let steps = 10;
            for i in 0..=steps {
                let t = i as f32 / steps as f32;
                let angle = -0.9 + t * 1.8;
                pts.push(Pos2::new(
                    center.x + r * angle.cos(),
                    center.y + r * angle.sin(),
                ));
            }
            p.add(egui::Shape::line(pts, Stroke::new(1.4, color)));
        }
    }
}

/// Simple gear: toothed ring + hub.
fn draw_gear(p: &Painter, rect: Rect, color: Color32) {
    let stroke = Stroke::new(1.5, color);
    let center = rect.center();
    let outer = rect.size().min_elem() * 0.36;
    let inner = outer * 0.22;

    p.circle_stroke(center, outer, stroke);
    p.circle_stroke(center, inner, stroke);

    let teeth = 8;
    for i in 0..teeth {
        let angle = i as f32 * std::f32::consts::TAU / teeth as f32;
        let dir = Vec2::new(angle.cos(), angle.sin());
        let a = center + dir * outer;
        let b = center + dir * (outer * 1.28);
        p.line_segment([a, b], stroke);
    }
}

fn draw_close(p: &Painter, rect: Rect, color: Color32) {
    let stroke = Stroke::new(1.8, color);
    let a = rect.left_top();
    let b = rect.right_bottom();
    let c = rect.right_top();
    let d = rect.left_bottom();
    p.line_segment([a, b], stroke);
    p.line_segment([c, d], stroke);
}

pub fn draw_weather_icon(p: &Painter, rect: Rect, condition: WeatherCondition) {
    match condition {
        WeatherCondition::Clear => draw_sun(p, rect),
        WeatherCondition::PartlyCloudy => draw_partly_cloudy(p, rect),
        WeatherCondition::Overcast => draw_overcast(p, rect),
        WeatherCondition::Fog => draw_fog(p, rect),
        WeatherCondition::Drizzle => draw_rain_icon(p, rect, true),
        WeatherCondition::Rain => draw_rain_icon(p, rect, false),
        WeatherCondition::Snow => draw_snow_icon(p, rect),
        WeatherCondition::Thunderstorm => draw_thunder_icon(p, rect),
    }
}

fn draw_sun(p: &Painter, rect: Rect) {
    let center = rect.center();
    let sun_color = Color32::from_rgb(0xFF, 0xB7, 0x4D);
    let r = rect.width().min(rect.height()) * 0.28;
    p.circle_filled(center, r, sun_color);

    let ray_stroke = Stroke::new(1.2, sun_color);
    let ray_start = r * 1.35;
    let ray_end = r * 1.75;
    for i in 0..8 {
        let angle = i as f32 * std::f32::consts::TAU / 8.0;
        let dir = Vec2::new(angle.cos(), angle.sin());
        p.line_segment([center + dir * ray_start, center + dir * ray_end], ray_stroke);
    }
}

fn draw_cloud_shape(p: &Painter, offset: Pos2, scale: f32, color: Color32) {
    // A soft cloud shape made of 3 overlapping filled circles and a flat bottom base
    let base_y = offset.y + 2.0 * scale;
    let r_center = 4.2 * scale;
    let r_left = 3.0 * scale;
    let r_right = 3.4 * scale;

    p.circle_filled(Pos2::new(offset.x, base_y - 1.5 * scale), r_center, color);
    p.circle_filled(Pos2::new(offset.x - 3.8 * scale, base_y), r_left, color);
    p.circle_filled(Pos2::new(offset.x + 3.8 * scale, base_y), r_right, color);

    let base_rect = Rect::from_min_max(
        Pos2::new(offset.x - 4.5 * scale, base_y - 1.5 * scale),
        Pos2::new(offset.x + 4.5 * scale, base_y + 3.2 * scale),
    );
    p.rect_filled(base_rect, CornerRadius::same(2), color);
}

fn draw_partly_cloudy(p: &Painter, rect: Rect) {
    let sun_color = Color32::from_rgb(0xFF, 0xB7, 0x4D);
    let sun_center = Pos2::new(rect.right() - rect.width() * 0.32, rect.top() + rect.height() * 0.32);
    let r = rect.width() * 0.20;
    p.circle_filled(sun_center, r, sun_color);

    let cloud_color = Color32::from_rgb(0xD8, 0xE2, 0xDC);
    let cloud_offset = Pos2::new(rect.left() + rect.width() * 0.42, rect.bottom() - rect.height() * 0.38);
    draw_cloud_shape(p, cloud_offset, rect.width() / 16.0, cloud_color);
}

fn draw_overcast(p: &Painter, rect: Rect) {
    let back_cloud = Color32::from_rgb(0x78, 0x90, 0x9C);
    let front_cloud = Color32::from_rgb(0xCF, 0xD8, 0xDC);
    let s = rect.width() / 16.0;

    let back_pos = Pos2::new(rect.center().x + 2.0 * s, rect.center().y - 2.0 * s);
    draw_cloud_shape(p, back_pos, s * 0.85, back_cloud);

    let front_pos = Pos2::new(rect.center().x - 1.5 * s, rect.center().y + 1.0 * s);
    draw_cloud_shape(p, front_pos, s, front_cloud);
}

fn draw_fog(p: &Painter, rect: Rect) {
    let color = Color32::from_rgb(0xB0, 0xBE, 0xC5);
    let stroke = Stroke::new(1.4, color);
    let w = rect.width();
    let cx = rect.center().x;
    let y = rect.center().y;

    for (dy, len_mult) in [(-3.0, 0.7), (0.0, 0.85), (3.0, 0.65)] {
        let half = (w * len_mult) / 2.0;
        p.line_segment([Pos2::new(cx - half, y + dy), Pos2::new(cx + half, y + dy)], stroke);
    }
}

fn draw_rain_icon(p: &Painter, rect: Rect, drizzle: bool) {
    let cloud_color = Color32::from_rgb(0x90, 0xA4, 0xAE);
    let s = rect.width() / 16.0;
    let cloud_pos = Pos2::new(rect.center().x, rect.top() + rect.height() * 0.38);
    draw_cloud_shape(p, cloud_pos, s * 0.9, cloud_color);

    let rain_color = Color32::from_rgb(0x4F, 0xC3, 0xF7);
    let stroke = Stroke::new(if drizzle { 1.0 } else { 1.4 }, rain_color);

    let drops = if drizzle {
        vec![-3.0 * s, 0.0, 3.0 * s]
    } else {
        vec![-3.5 * s, -0.5 * s, 2.5 * s]
    };

    let start_y = rect.top() + rect.height() * 0.68;
    let len = if drizzle { 2.0 * s } else { 3.5 * s };
    let tilt = 1.0 * s;

    for dx in drops {
        let x = rect.center().x + dx;
        p.line_segment([Pos2::new(x, start_y), Pos2::new(x - tilt, start_y + len)], stroke);
    }
}

fn draw_snow_icon(p: &Painter, rect: Rect) {
    let cloud_color = Color32::from_rgb(0x90, 0xA4, 0xAE);
    let s = rect.width() / 16.0;
    let cloud_pos = Pos2::new(rect.center().x, rect.top() + rect.height() * 0.38);
    draw_cloud_shape(p, cloud_pos, s * 0.9, cloud_color);

    let snow_color = Color32::from_rgb(0xE0, 0xF7, 0xFA);
    for dx in [-3.0 * s, 0.0, 3.0 * s] {
        let pt = Pos2::new(rect.center().x + dx, rect.bottom() - 3.0 * s);
        p.circle_filled(pt, 1.2 * s, snow_color);
    }
}

fn draw_thunder_icon(p: &Painter, rect: Rect) {
    let cloud_color = Color32::from_rgb(0x54, 0x6E, 0x7A);
    let s = rect.width() / 16.0;
    let cloud_pos = Pos2::new(rect.center().x, rect.top() + rect.height() * 0.38);
    draw_cloud_shape(p, cloud_pos, s * 0.9, cloud_color);

    let bolt_color = Color32::from_rgb(0xFF, 0xEB, 0x3B);
    let bolt_stroke = Stroke::new(1.4, bolt_color);

    let cx = rect.center().x;
    let cy = rect.top() + rect.height() * 0.60;
    let pts = vec![
        Pos2::new(cx + 0.5 * s, cy),
        Pos2::new(cx - 1.5 * s, cy + 2.5 * s),
        Pos2::new(cx + 0.5 * s, cy + 2.5 * s),
        Pos2::new(cx - 1.0 * s, cy + 5.5 * s),
    ];
    p.add(egui::Shape::line(pts, bolt_stroke));
}
