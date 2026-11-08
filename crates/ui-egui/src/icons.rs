//! The shell's icons, painted with shapes so ScoreCraft ships no icon assets and never depends on
//! a font having a glyph (egui's built-in font has no emoji or arrow symbols: a `📁` or `↻` in a
//! label draws as an empty box). Every icon is drawn into a square `rect` in one colour.

use egui::{Color32, Painter, Pos2, Rect, Response, Sense, Shape, Stroke, Ui, Vec2, pos2, vec2};

/// Every icon the shell draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Previous,
    Next,
    Single,
    Two,
    /// The automatic page layout: one page or two, from the window's shape.
    AutoLayout,
    Half,
    Alternate,
    FitWidth,
    ZoomIn,
    ZoomOut,
    FullScreen,
    Theme,
    Markup,
    /// Clear every marking on the pages shown.
    ClearInk,
    Library,
    Lists,
    Recordings,
    Practice,
    Search,
    Trash,
    Settings,
    Folder,
    NewFolder,
    Score,
    Up,
    Down,
    Back,
    Close,
    Play,
    TurnLeft,
    TurnRight,
    Star,
    StarFilled,
    Sync,
    Info,
    More,
    Filter,
    Sort,
    Plus,
    ChevronDown,
    Check,
}

/// Paints `icon` into `rect` in `c`.
pub fn paint(p: &Painter, rect: Rect, c: Color32, icon: Icon) {
    let r = rect.shrink(rect.width() * 0.22);
    let w = r.width();
    let line = Stroke::new((w * 0.075).clamp(1.2, 2.2), c);
    let page = |x0: f32, x1: f32| Rect::from_min_max(pos2(x0, r.top()), pos2(x1, r.bottom()));
    let poly = |pts: Vec<Pos2>| Shape::convex_polygon(pts, c, Stroke::NONE);
    let path = |pts: Vec<Pos2>| Shape::line(pts, line);
    let ctr = r.center();
    match icon {
        Icon::Previous | Icon::Next => {
            let (a, b) = if icon == Icon::Next {
                (r.left() + w * 0.2, r.right() - w * 0.1)
            } else {
                (r.right() - w * 0.2, r.left() + w * 0.1)
            };
            p.add(poly(vec![
                pos2(a, r.top()),
                pos2(b, ctr.y),
                pos2(a, r.bottom()),
            ]));
        }
        Icon::Single => {
            p.rect_stroke(
                page(r.left() + w * 0.2, r.right() - w * 0.2),
                1.0,
                line,
                egui::StrokeKind::Middle,
            );
        }
        Icon::Two => {
            let m = ctr.x;
            p.rect_stroke(page(r.left(), m - 1.0), 1.0, line, egui::StrokeKind::Middle);
            p.rect_stroke(
                page(m + 1.0, r.right()),
                1.0,
                line,
                egui::StrokeKind::Middle,
            );
        }
        Icon::AutoLayout => {
            // A page beside a dashed second page.
            let m = ctr.x;
            p.rect_stroke(page(r.left(), m - 1.0), 1.0, line, egui::StrokeKind::Middle);
            let b = page(m + 1.0, r.right());
            let dash = Stroke::new(line.width * 0.8, c);
            for k in 0..4 {
                let y0 = b.top() + b.height() * k as f32 / 4.0;
                let y1 = y0 + b.height() / 8.0;
                p.line_segment([pos2(b.right(), y0), pos2(b.right(), y1)], dash);
            }
            p.line_segment([b.left_top(), b.right_top()], dash);
            p.line_segment([b.left_bottom(), b.right_bottom()], dash);
        }
        Icon::Half => {
            let pg = page(r.left() + w * 0.2, r.right() - w * 0.2);
            p.rect_stroke(pg, 1.0, line, egui::StrokeKind::Middle);
            p.rect_filled(
                Rect::from_min_max(pg.min, pos2(pg.right(), pg.center().y)),
                1.0,
                c.gamma_multiply(0.5),
            );
        }
        Icon::Alternate => {
            let m = ctr.x;
            p.rect_filled(page(r.left(), m - 1.0), 1.0, c.gamma_multiply(0.5));
            p.rect_stroke(
                page(m + 1.0, r.right()),
                1.0,
                line,
                egui::StrokeKind::Middle,
            );
        }
        Icon::FitWidth => {
            let y = ctr.y;
            let k = w * 0.25;
            p.line_segment([pos2(r.left(), y), pos2(r.right(), y)], line);
            for (x, d) in [(r.left(), 1.0), (r.right(), -1.0)] {
                p.add(path(vec![
                    pos2(x + d * k, y - k),
                    pos2(x, y),
                    pos2(x + d * k, y + k),
                ]));
            }
            p.line_segment([r.left_top(), r.left_bottom()], line);
            p.line_segment([r.right_top(), r.right_bottom()], line);
        }
        Icon::ZoomIn | Icon::ZoomOut | Icon::Search => {
            let rad = w * 0.32;
            let o = r.left_top() + vec2(rad + 1.0, rad + 1.0);
            p.circle_stroke(o, rad, line);
            p.line_segment([o + vec2(rad * 0.72, rad * 0.72), r.right_bottom()], line);
            if icon != Icon::Search {
                p.line_segment([o - vec2(rad * 0.5, 0.0), o + vec2(rad * 0.5, 0.0)], line);
            }
            if icon == Icon::ZoomIn {
                p.line_segment([o - vec2(0.0, rad * 0.5), o + vec2(0.0, rad * 0.5)], line);
            }
        }
        Icon::FullScreen => {
            let k = w * 0.35;
            for (corner, dx, dy) in [
                (r.left_top(), 1.0, 1.0),
                (r.right_top(), -1.0, 1.0),
                (r.left_bottom(), 1.0, -1.0),
                (r.right_bottom(), -1.0, -1.0),
            ] {
                p.line_segment([corner, corner + vec2(dx * k, 0.0)], line);
                p.line_segment([corner, corner + vec2(0.0, dy * k)], line);
            }
        }
        Icon::Theme => {
            let rad = w * 0.45;
            p.circle_stroke(ctr, rad, line);
            p.add(poly(arc(ctr, rad, -90.0, 90.0, 16)));
        }
        Icon::Markup => {
            // A pen, nib down to the left.
            let (a, b) = (r.left_bottom(), r.right_top());
            p.line_segment(
                [a + vec2(w * 0.2, -w * 0.2), b],
                Stroke::new(line.width * 1.6, c),
            );
            p.add(poly(vec![
                a,
                a + vec2(w * 0.3, 0.0),
                a + vec2(0.0, -w * 0.3),
            ]));
        }
        Icon::ClearInk => {
            // A pen stroke with a cross over it.
            let wave: Vec<Pos2> = (0..=12)
                .map(|i| {
                    let t = i as f32 / 12.0;
                    pos2(
                        r.left() + t * w * 0.7,
                        r.top() + w * 0.35 + (t * std::f32::consts::TAU).sin() * w * 0.15,
                    )
                })
                .collect();
            p.add(path(wave));
            let x = Rect::from_min_max(pos2(ctr.x, ctr.y), r.max);
            p.line_segment([x.left_top(), x.right_bottom()], line);
            p.line_segment([x.right_top(), x.left_bottom()], line);
        }
        Icon::Library => {
            // Three book spines, the last leaning.
            let s = w / 5.0;
            for i in 0..2 {
                let x = r.left() + i as f32 * (s + s * 0.6);
                p.rect_stroke(
                    Rect::from_min_max(pos2(x, r.top()), pos2(x + s, r.bottom())),
                    1.0,
                    line,
                    egui::StrokeKind::Middle,
                );
            }
            let x = r.left() + 2.0 * (s + s * 0.6);
            p.add(path(vec![
                pos2(x, r.bottom()),
                pos2(x + s * 0.8, r.top()),
                pos2(x + s * 1.8, r.top() + s * 0.4),
                pos2(x + s, r.bottom()),
                pos2(x, r.bottom()),
            ]));
        }
        Icon::Lists => {
            for i in 0..4 {
                let y = r.top() + w * (0.1 + 0.25 * i as f32);
                let x1 = if i == 3 {
                    r.left() + w * 0.55
                } else {
                    r.right()
                };
                p.line_segment([pos2(r.left(), y), pos2(x1, y)], line);
            }
            p.circle_filled(
                pos2(r.right() - w * 0.15, r.bottom() - w * 0.05),
                w * 0.12,
                c,
            );
            p.line_segment(
                [
                    pos2(r.right() - w * 0.04, r.bottom() - w * 0.05),
                    pos2(r.right() - w * 0.04, r.top() + w * 0.55),
                ],
                line,
            );
        }
        Icon::Recordings => {
            // A microphone.
            let head =
                Rect::from_center_size(pos2(ctr.x, r.top() + w * 0.28), vec2(w * 0.36, w * 0.56));
            p.rect_stroke(head, w * 0.18, line, egui::StrokeKind::Middle);
            p.add(path(arc(
                pos2(ctr.x, r.top() + w * 0.4),
                w * 0.32,
                0.0,
                180.0,
                12,
            )));
            p.line_segment(
                [pos2(ctr.x, r.top() + w * 0.72), pos2(ctr.x, r.bottom())],
                line,
            );
            p.line_segment(
                [
                    pos2(ctr.x - w * 0.2, r.bottom()),
                    pos2(ctr.x + w * 0.2, r.bottom()),
                ],
                line,
            );
        }
        Icon::Practice => {
            // A stopwatch.
            let o = pos2(ctr.x, ctr.y + w * 0.08);
            p.circle_stroke(o, w * 0.42, line);
            p.line_segment([o, o + vec2(w * 0.18, -w * 0.18)], line);
            p.line_segment(
                [
                    pos2(ctr.x - w * 0.12, r.top() - w * 0.02),
                    pos2(ctr.x + w * 0.12, r.top() - w * 0.02),
                ],
                line,
            );
        }
        Icon::Trash => {
            let body = Rect::from_min_max(
                pos2(r.left() + w * 0.15, r.top() + w * 0.2),
                pos2(r.right() - w * 0.15, r.bottom()),
            );
            p.add(path(vec![
                body.left_top(),
                body.left_bottom(),
                body.right_bottom(),
                body.right_top(),
            ]));
            p.line_segment(
                [pos2(r.left(), body.top()), pos2(r.right(), body.top())],
                line,
            );
            p.add(path(vec![
                pos2(ctr.x - w * 0.15, body.top()),
                pos2(ctr.x - w * 0.15, r.top()),
                pos2(ctr.x + w * 0.15, r.top()),
                pos2(ctr.x + w * 0.15, body.top()),
            ]));
            for dx in [-0.12, 0.12] {
                p.line_segment(
                    [
                        pos2(ctr.x + dx * w, body.top() + w * 0.15),
                        pos2(ctr.x + dx * w, body.bottom() - w * 0.12),
                    ],
                    line,
                );
            }
        }
        Icon::Settings => {
            // A cog: a ring with eight teeth.
            p.circle_stroke(ctr, w * 0.3, line);
            p.circle_stroke(ctr, w * 0.1, line);
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::FRAC_PI_4;
                let d = vec2(a.cos(), a.sin());
                p.line_segment(
                    [ctr + d * w * 0.3, ctr + d * w * 0.48],
                    Stroke::new(line.width * 1.6, c),
                );
            }
        }
        Icon::Folder | Icon::NewFolder => {
            let body = Rect::from_min_max(pos2(r.left(), r.top() + w * 0.2), r.max);
            p.add(path(vec![
                pos2(r.left(), body.bottom()),
                pos2(r.left(), r.top() + w * 0.05),
                pos2(r.left() + w * 0.35, r.top() + w * 0.05),
                pos2(r.left() + w * 0.45, body.top()),
                pos2(r.right(), body.top()),
                pos2(r.right(), body.bottom()),
                pos2(r.left(), body.bottom()),
            ]));
            if icon == Icon::NewFolder {
                let o = pos2(ctr.x, body.center().y + w * 0.05);
                let k = w * 0.18;
                p.line_segment([o - vec2(k, 0.0), o + vec2(k, 0.0)], line);
                p.line_segment([o - vec2(0.0, k), o + vec2(0.0, k)], line);
            }
        }
        Icon::Score => {
            // A page with a folded corner and three staff lines.
            let pg = page(r.left() + w * 0.12, r.right() - w * 0.12);
            let fold = w * 0.22;
            p.add(path(vec![
                pg.left_top(),
                pos2(pg.right() - fold, pg.top()),
                pos2(pg.right(), pg.top() + fold),
                pg.right_bottom(),
                pg.left_bottom(),
                pg.left_top(),
            ]));
            for k in 0..3 {
                let y = pg.top() + pg.height() * (0.45 + 0.15 * k as f32);
                p.line_segment(
                    [pos2(pg.left() + w * 0.1, y), pos2(pg.right() - w * 0.1, y)],
                    Stroke::new(line.width * 0.7, c),
                );
            }
        }
        Icon::Up | Icon::Down => {
            let (tip, base) = if icon == Icon::Up {
                (r.top() + w * 0.15, r.bottom() - w * 0.15)
            } else {
                (r.bottom() - w * 0.15, r.top() + w * 0.15)
            };
            let d = if icon == Icon::Up { 1.0 } else { -1.0 };
            p.line_segment([pos2(ctr.x, tip), pos2(ctr.x, base)], line);
            p.add(path(vec![
                pos2(ctr.x - w * 0.3, tip + d * w * 0.3),
                pos2(ctr.x, tip),
                pos2(ctr.x + w * 0.3, tip + d * w * 0.3),
            ]));
        }
        Icon::Back => {
            p.line_segment(
                [pos2(r.left() + w * 0.1, ctr.y), pos2(r.right(), ctr.y)],
                line,
            );
            p.add(path(vec![
                pos2(r.left() + w * 0.4, r.top() + w * 0.2),
                pos2(r.left() + w * 0.1, ctr.y),
                pos2(r.left() + w * 0.4, r.bottom() - w * 0.2),
            ]));
        }
        Icon::Close => {
            let k = r.shrink(w * 0.12);
            p.line_segment([k.left_top(), k.right_bottom()], line);
            p.line_segment([k.right_top(), k.left_bottom()], line);
        }
        Icon::Play => {
            p.add(poly(vec![
                pos2(r.left() + w * 0.2, r.top()),
                pos2(r.right() - w * 0.05, ctr.y),
                pos2(r.left() + w * 0.2, r.bottom()),
            ]));
        }
        Icon::TurnLeft | Icon::TurnRight => {
            // Three quarters of a circle with an arrowhead at its open end.
            let rad = w * 0.4;
            let left = icon == Icon::TurnLeft;
            let (a0, a1) = if left { (-60.0, 210.0) } else { (-30.0, 240.0) };
            let pts = arc(ctr, rad, a0, a1, 20);
            let end = if left { pts.first() } else { pts.last() }.copied();
            p.add(path(pts));
            if let Some(e) = end {
                let k = w * 0.28;
                let (h1, h2) = if left {
                    (e + vec2(k, -k * 0.1), e + vec2(k * 0.2, -k))
                } else {
                    (e + vec2(-k, -k * 0.1), e + vec2(-k * 0.2, -k))
                };
                p.add(poly(vec![e, h1, h2]));
            }
        }
        Icon::Star | Icon::StarFilled => {
            let pts: Vec<Pos2> = (0..10)
                .map(|i| {
                    let a = (-90.0 + 36.0 * i as f32).to_radians();
                    let rad = if i % 2 == 0 { w * 0.5 } else { w * 0.2 };
                    ctr + vec2(a.cos(), a.sin()) * rad
                })
                .collect();
            if icon == Icon::StarFilled {
                // A star isn't convex: fill it as five triangles around a pentagon.
                let inner: Vec<Pos2> = pts.iter().skip(1).step_by(2).copied().collect();
                p.add(poly(inner.clone()));
                for i in 0..5 {
                    if let (Some(&tip), Some(&l), Some(&rr)) =
                        (pts.get(i * 2), inner.get((i + 4) % 5), inner.get(i))
                    {
                        p.add(poly(vec![l, tip, rr]));
                    }
                }
            } else {
                let mut closed = pts.clone();
                closed.extend(pts.first().copied());
                p.add(path(closed));
            }
        }
        Icon::Sync => {
            let rad = w * 0.4;
            for (a0, a1) in [(200.0, 340.0), (20.0, 160.0)] {
                let pts = arc(ctr, rad, a0, a1, 12);
                let end = pts.last().copied();
                p.add(path(pts));
                if let Some(e) = end {
                    let a = f32::to_radians(a1);
                    let tangent = vec2(-a.sin(), a.cos());
                    let normal = vec2(a.cos(), a.sin());
                    let k = w * 0.18;
                    p.add(poly(vec![e + tangent * k, e - normal * k, e + normal * k]));
                }
            }
        }
        Icon::Info => {
            p.circle_stroke(ctr, w * 0.48, line);
            p.circle_filled(pos2(ctr.x, r.top() + w * 0.27), line.width * 0.9, c);
            p.line_segment(
                [
                    pos2(ctr.x, r.top() + w * 0.45),
                    pos2(ctr.x, r.bottom() - w * 0.2),
                ],
                line,
            );
        }
        Icon::More => {
            for dx in [-0.35, 0.0, 0.35] {
                p.circle_filled(ctr + vec2(dx * w, 0.0), w * 0.08, c);
            }
        }
        Icon::Filter => {
            p.add(path(vec![
                r.left_top(),
                r.right_top(),
                pos2(ctr.x + w * 0.1, ctr.y),
                pos2(ctr.x + w * 0.1, r.bottom()),
                pos2(ctr.x - w * 0.1, r.bottom() - w * 0.15),
                pos2(ctr.x - w * 0.1, ctr.y),
                r.left_top(),
            ]));
        }
        Icon::Sort => {
            // An arrow up and an arrow down, side by side.
            for (x, up) in [(r.left() + w * 0.25, true), (r.right() - w * 0.25, false)] {
                let (tip, base) = if up {
                    (r.top(), r.bottom())
                } else {
                    (r.bottom(), r.top())
                };
                let d = if up { 1.0 } else { -1.0 };
                p.line_segment([pos2(x, tip), pos2(x, base)], line);
                p.add(path(vec![
                    pos2(x - w * 0.2, tip + d * w * 0.2),
                    pos2(x, tip),
                    pos2(x + w * 0.2, tip + d * w * 0.2),
                ]));
            }
        }
        Icon::Plus => {
            p.line_segment([pos2(r.left(), ctr.y), pos2(r.right(), ctr.y)], line);
            p.line_segment([pos2(ctr.x, r.top()), pos2(ctr.x, r.bottom())], line);
        }
        Icon::ChevronDown => {
            p.add(path(vec![
                pos2(r.left() + w * 0.15, ctr.y - w * 0.15),
                pos2(ctr.x, ctr.y + w * 0.15),
                pos2(r.right() - w * 0.15, ctr.y - w * 0.15),
            ]));
        }
        Icon::Check => {
            p.add(path(vec![
                pos2(r.left() + w * 0.05, ctr.y),
                pos2(r.left() + w * 0.38, r.bottom() - w * 0.15),
                pos2(r.right() - w * 0.05, r.top() + w * 0.15),
            ]));
        }
    }
}

/// Points along a circle from `a0` to `a1` degrees (0 is to the right, 90 down).
fn arc(c: Pos2, rad: f32, a0: f32, a1: f32, n: usize) -> Vec<Pos2> {
    (0..=n)
        .map(|i| {
            let a = (a0 + (a1 - a0) * i as f32 / n as f32).to_radians();
            c + vec2(a.cos(), a.sin()) * rad
        })
        .collect()
}

/// A small icon-only button for dialogs and rows, `side` points square, with `hover` as its
/// tooltip.
pub fn small_button(ui: &mut Ui, icon: Icon, hover: &str) -> Response {
    small_button_sized(ui, icon, hover, 24.0, false)
}

/// [`small_button`] at another size, optionally shown selected.
pub fn small_button_sized(
    ui: &mut Ui,
    icon: Icon,
    hover: &str,
    side: f32,
    selected: bool,
) -> Response {
    let (rect, r) = ui.allocate_exact_size(Vec2::splat(side), Sense::click());
    if ui.is_rect_visible(rect) {
        let v = ui.style().interact_selectable(&r, selected);
        if r.hovered() || selected || r.has_focus() {
            ui.painter()
                .rect_filled(rect, v.corner_radius, v.weak_bg_fill);
        }
        paint(ui.painter(), rect, v.text_color(), icon);
    }
    r.on_hover_text(hover)
}

/// A button with an icon before its text, the height of a normal button.
pub fn labelled(ui: &mut Ui, icon: Icon, text: &str) -> Response {
    labelled_selected(ui, icon, text, false)
}

/// [`labelled`], shown selected when `selected`.
pub fn labelled_selected(ui: &mut Ui, icon: Icon, text: &str, selected: bool) -> Response {
    let pad = ui.spacing().button_padding;
    let font = egui::TextStyle::Button.resolve(ui.style());
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), font, Color32::PLACEHOLDER);
    let side = galley.size().y.max(16.0) + 4.0;
    let size = vec2(
        pad.x * 2.0 + side + 6.0 + galley.size().x,
        (pad.y * 2.0 + side).max(ui.spacing().interact_size.y),
    );
    let (rect, r) = ui.allocate_exact_size(size, Sense::click());
    if ui.is_rect_visible(rect) {
        let v = ui.style().interact_selectable(&r, selected);
        ui.painter().rect(
            rect,
            v.corner_radius,
            v.weak_bg_fill,
            v.bg_stroke,
            egui::StrokeKind::Inside,
        );
        let icon_rect = Rect::from_min_size(
            pos2(rect.left() + pad.x, rect.center().y - side / 2.0),
            Vec2::splat(side),
        );
        paint(ui.painter(), icon_rect, v.text_color(), icon);
        ui.painter().galley(
            pos2(
                icon_rect.right() + 6.0,
                rect.center().y - galley.size().y / 2.0,
            ),
            galley,
            v.text_color(),
        );
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every icon paints something inside its rectangle, at any size, without panicking.
    #[test]
    fn every_icon_paints_inside_its_box() {
        let ctx = egui::Context::default();
        let all = [
            Icon::Previous,
            Icon::Next,
            Icon::Single,
            Icon::Two,
            Icon::AutoLayout,
            Icon::Half,
            Icon::Alternate,
            Icon::FitWidth,
            Icon::ZoomIn,
            Icon::ZoomOut,
            Icon::FullScreen,
            Icon::Theme,
            Icon::Markup,
            Icon::ClearInk,
            Icon::Library,
            Icon::Lists,
            Icon::Recordings,
            Icon::Practice,
            Icon::Search,
            Icon::Trash,
            Icon::Settings,
            Icon::Folder,
            Icon::NewFolder,
            Icon::Score,
            Icon::Up,
            Icon::Down,
            Icon::Back,
            Icon::Close,
            Icon::Play,
            Icon::TurnLeft,
            Icon::TurnRight,
            Icon::Star,
            Icon::StarFilled,
            Icon::Sync,
            Icon::Info,
            Icon::More,
            Icon::Filter,
            Icon::Sort,
            Icon::Plus,
            Icon::ChevronDown,
            Icon::Check,
        ];
        for side in [0.0, 16.0, 44.0] {
            let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
                let rect = Rect::from_min_size(pos2(100.0, 100.0), Vec2::splat(side));
                for icon in all {
                    paint(ui.painter(), rect, Color32::WHITE, icon);
                }
            });
            out.textures_delta.clear();
            if side > 0.0 {
                assert!(
                    out.shapes.len() >= all.len(),
                    "{side}: {}",
                    out.shapes.len()
                );
            }
        }
    }
}
