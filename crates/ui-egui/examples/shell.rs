//! Renders the whole shell (toolbars, library, dialogs) to a PNG without a window or GPU, for
//! checking a UI change. egui's own tessellated triangles are filled on the CPU here.
//!
//! ```sh
//! cargo run -p scorecraft-ui-egui --example shell -- --out /tmp/shell.png \
//!   --pages 4 --steps "cmd annotate.mode.set {\"on\":true}; click 600,300; type Hello"
//! ```
//!
//! Options: `--width W --height H` (points), `--ppp X`, `--pages N` (a synthetic score of N
//! pages, opened in the reader; 0 for none), `--theme light|dark`, `--steps` (`;`-separated:
//! `cmd <id> <json>`, `click x,y`, `rclick x,y`, `drag x0,y0 x1,y1`, `pen x0,y0 x1,y1`,
//! `type <text>`, `key <Name>`, `frames N`).

#![allow(clippy::indexing_slicing)] // a dev tool: pixel arithmetic on buffers it sized itself

use std::collections::HashMap;

use egui::epaint::{ClippedPrimitive, ImageData, Primitive};
use egui::{Color32, Context, Event, Pos2, RawInput, Rect, TextureId, pos2, vec2};
use scorecraft_testkit::pdf::{PageSpec, PdfBuilder};
use scorecraft_ui_egui::{App, NoPlatform, Settings};
use serde_json::Value;

fn arg<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
}

struct Image {
    w: usize,
    h: usize,
    px: Vec<Color32>,
}

struct Canvas {
    w: usize,
    h: usize,
    px: Vec<[f32; 4]>,
    textures: HashMap<TextureId, Image>,
}

impl Canvas {
    fn apply(&mut self, delta: &egui::TexturesDelta) {
        for (id, ds) in &delta.set {
            for d in ds {
                let (w, h, px): (usize, usize, Vec<Color32>) = match &d.image {
                    ImageData::Color(c) => (c.size[0], c.size[1], c.pixels.clone()),
                };
                match d.pos {
                    None => {
                        self.textures.insert(*id, Image { w, h, px });
                    }
                    Some([x0, y0]) => {
                        if let Some(t) = self.textures.get_mut(id) {
                            for y in 0..h {
                                for x in 0..w {
                                    if let (Some(dst), Some(src)) =
                                        (t.px.get_mut((y0 + y) * t.w + x0 + x), px.get(y * w + x))
                                    {
                                        *dst = *src;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    fn sample(&self, tex: TextureId, u: f32, v: f32) -> [f32; 4] {
        let Some(t) = self.textures.get(&tex) else {
            return [1.0; 4];
        };
        let x = ((u * t.w as f32) as isize).clamp(0, t.w as isize - 1) as usize;
        let y = ((v * t.h as f32) as isize).clamp(0, t.h as isize - 1) as usize;
        let c = t.px.get(y * t.w + x).copied().unwrap_or(Color32::WHITE);
        [
            c.r() as f32 / 255.0,
            c.g() as f32 / 255.0,
            c.b() as f32 / 255.0,
            c.a() as f32 / 255.0,
        ]
    }

    fn draw(&mut self, prims: &[ClippedPrimitive], ppp: f32) {
        for p in prims {
            let Primitive::Mesh(mesh) = &p.primitive else {
                continue;
            };
            let clip = Rect::from_min_max(p.clip_rect.min * ppp, p.clip_rect.max * ppp);
            for tri in mesh.indices.as_chunks::<3>().0 {
                let v: Vec<_> = tri
                    .iter()
                    .filter_map(|&i| mesh.vertices.get(i as usize))
                    .collect();
                if v.len() < 3 {
                    continue;
                }
                let pts: Vec<Pos2> = v
                    .iter()
                    .map(|v| pos2(v.pos.x * ppp, v.pos.y * ppp))
                    .collect();
                let (a, b, c) = (pts[0], pts[1], pts[2]);
                let area = (b - a).x * (c - a).y - (b - a).y * (c - a).x;
                if area.abs() < 1e-6 {
                    continue;
                }
                let x0 = a.x.min(b.x).min(c.x).max(clip.min.x).max(0.0).floor() as usize;
                let x1 =
                    a.x.max(b.x)
                        .max(c.x)
                        .min(clip.max.x)
                        .min(self.w as f32)
                        .ceil() as usize;
                let y0 = a.y.min(b.y).min(c.y).max(clip.min.y).max(0.0).floor() as usize;
                let y1 =
                    a.y.max(b.y)
                        .max(c.y)
                        .min(clip.max.y)
                        .min(self.h as f32)
                        .ceil() as usize;
                for y in y0..y1 {
                    for x in x0..x1 {
                        let q = pos2(x as f32 + 0.5, y as f32 + 0.5);
                        let w0 = ((b - q).x * (c - q).y - (b - q).y * (c - q).x) / area;
                        let w1 = ((c - q).x * (a - q).y - (c - q).y * (a - q).x) / area;
                        let w2 = 1.0 - w0 - w1;
                        if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                            continue;
                        }
                        let u = v[0].uv.x * w0 + v[1].uv.x * w1 + v[2].uv.x * w2;
                        let t = v[0].uv.y * w0 + v[1].uv.y * w1 + v[2].uv.y * w2;
                        let tex = self.sample(mesh.texture_id, u, t);
                        let col = |k: usize| {
                            (v[0].color.to_array()[k] as f32 * w0
                                + v[1].color.to_array()[k] as f32 * w1
                                + v[2].color.to_array()[k] as f32 * w2)
                                / 255.0
                        };
                        let src = [
                            col(0) * tex[0],
                            col(1) * tex[1],
                            col(2) * tex[2],
                            col(3) * tex[3],
                        ];
                        if let Some(d) = self.px.get_mut(y * self.w + x) {
                            for k in 0..4 {
                                d[k] = src[k] + d[k] * (1.0 - src[3]);
                            }
                        }
                    }
                }
            }
        }
    }
}

fn point(s: &str) -> Result<Pos2, String> {
    let (x, y) = s.split_once(',').ok_or(format!("{s}: want x,y"))?;
    Ok(pos2(
        x.trim().parse().map_err(|_| format!("{s}: x"))?,
        y.trim().parse().map_err(|_| format!("{s}: y"))?,
    ))
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let out = arg(&args, "--out").unwrap_or("shell.png").to_owned();
    let num = |n: &str, d: f32| arg(&args, n).and_then(|v| v.parse().ok()).unwrap_or(d);
    let (w, h, ppp) = (
        num("--width", 1280.0),
        num("--height", 800.0),
        num("--ppp", 1.0),
    );
    let pages = num("--pages", 3.0) as usize;
    let mut settings = Settings::default();
    if arg(&args, "--theme") == Some("dark") {
        settings.theme = scorecraft_ui_egui::Theme::Dark;
    }
    let mut app = App::new(Box::new(NoPlatform), settings);
    let dir = std::env::temp_dir().join(format!("scorecraft-shell-{}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    if pages > 0 {
        let path = dir.join("Sonata in G.pdf");
        std::fs::write(
            &path,
            PdfBuilder::new()
                .pages(pages, PageSpec::default().with_staves(8))
                .build(),
        )
        .map_err(|e| e.to_string())?;
        app.open_path(&path.to_string_lossy())
            .map_err(|e| e.to_string())?;
    }
    let ctx = Context::default();
    let mut canvas = Canvas {
        w: (w * ppp) as usize,
        h: (h * ppp) as usize,
        px: Vec::new(),
        textures: HashMap::new(),
    };
    let mut time = 0.0;
    let mut last = Vec::new();
    let mut frame = |app: &mut App, events: Vec<Event>, canvas: &mut Canvas| {
        time += 0.05;
        let mut input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(w, h))),
            time: Some(time),
            events,
            ..Default::default()
        };
        if let Some(v) = input.viewports.get_mut(&egui::ViewportId::ROOT) {
            v.native_pixels_per_point = Some(ppp);
        }
        let mut o = ctx.run_ui(input, |ui| app.ui(ui));
        canvas.apply(&o.textures_delta);
        o.textures_delta.clear();
        last = ctx.tessellate(o.shapes, o.pixels_per_point);
    };
    for _ in 0..3 {
        frame(&mut app, Vec::new(), &mut canvas);
    }
    let steps = arg(&args, "--steps").unwrap_or("").to_owned();
    for step in steps.split(';').map(str::trim).filter(|s| !s.is_empty()) {
        let (verb, rest) = step.split_once(' ').unwrap_or((step, ""));
        let press = |p: Pos2, button, pressed| Event::PointerButton {
            pos: p,
            button,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        match verb {
            "cmd" => {
                let (id, p) = rest.split_once(' ').unwrap_or((rest, "{}"));
                let p: Value = serde_json::from_str(p).map_err(|e| format!("{step}: {e}"))?;
                if let Err(e) = app.command(id, &p) {
                    eprintln!("{id}: {e}");
                }
            }
            "click" | "rclick" => {
                let p = point(rest)?;
                let b = if verb == "click" {
                    egui::PointerButton::Primary
                } else {
                    egui::PointerButton::Secondary
                };
                frame(&mut app, vec![Event::PointerMoved(p)], &mut canvas);
                frame(&mut app, vec![press(p, b, true)], &mut canvas);
                frame(&mut app, vec![press(p, b, false)], &mut canvas);
            }
            "drag" | "pen" => {
                let (a, b) = rest.split_once(' ').ok_or("drag wants two points")?;
                let (a, b) = (point(a)?, point(b)?);
                if verb == "pen" {
                    let t = |id, phase, p: Pos2| Event::Touch {
                        device_id: egui::TouchDeviceId(1),
                        id: egui::TouchId(id),
                        phase,
                        pos: p,
                        force: Some(0.6),
                    };
                    frame(
                        &mut app,
                        vec![t(1, egui::TouchPhase::Start, a)],
                        &mut canvas,
                    );
                    for k in 1..10 {
                        let p = a + (b - a) * (k as f32 / 10.0);
                        frame(&mut app, vec![t(1, egui::TouchPhase::Move, p)], &mut canvas);
                    }
                    frame(&mut app, vec![t(1, egui::TouchPhase::End, b)], &mut canvas);
                } else {
                    let l = egui::PointerButton::Primary;
                    frame(
                        &mut app,
                        vec![Event::PointerMoved(a), press(a, l, true)],
                        &mut canvas,
                    );
                    for k in 1..10 {
                        let p = a + (b - a) * (k as f32 / 10.0);
                        frame(&mut app, vec![Event::PointerMoved(p)], &mut canvas);
                    }
                    frame(&mut app, vec![press(b, l, false)], &mut canvas);
                }
            }
            "type" => frame(&mut app, vec![Event::Text(rest.to_owned())], &mut canvas),
            "key" => {
                let key = egui::Key::from_name(rest).ok_or(format!("no key {rest}"))?;
                let k = |pressed| Event::Key {
                    key,
                    physical_key: None,
                    pressed,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                };
                frame(&mut app, vec![k(true), k(false)], &mut canvas);
            }
            "frames" => {
                for _ in 0..rest.parse::<usize>().unwrap_or(1) {
                    frame(&mut app, Vec::new(), &mut canvas);
                }
            }
            other => return Err(format!("unknown step {other}")),
        }
        frame(&mut app, Vec::new(), &mut canvas);
    }
    // Let page renders and covers arrive.
    for _ in 0..20 {
        std::thread::sleep(std::time::Duration::from_millis(30));
        frame(&mut app, Vec::new(), &mut canvas);
    }
    canvas.px = vec![[0.0, 0.0, 0.0, 1.0]; canvas.w * canvas.h];
    let prims = std::mem::take(&mut last);
    canvas.draw(&prims, ppp);
    let bytes: Vec<u8> = canvas
        .px
        .iter()
        .flat_map(|p| p.map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8))
        .collect();
    let png =
        scorecraft_codecs::png::encode_premultiplied(canvas.w as u32, canvas.h as u32, &bytes)?;
    std::fs::write(&out, png).map_err(|e| e.to_string())?;
    println!(
        "{out}: {}x{}; message: {:?}",
        canvas.w, canvas.h, app.state.message
    );
    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("shell: {e}");
        std::process::exit(1);
    }
}
