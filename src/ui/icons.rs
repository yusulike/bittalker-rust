//! Small painter-drawn button icons (speaker / gear / close).
//! Drawn with the egui painter instead of emoji glyphs so they render
//! identically regardless of installed fonts.

use egui::{Color32, CornerRadius, Painter, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2};

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
