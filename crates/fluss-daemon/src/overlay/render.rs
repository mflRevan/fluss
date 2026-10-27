//! Pure overlay renderer (tiny-skia + fontdue into an RGBA pixmap). Separate
//! from the window so every state can be rendered to PNG for design review
//! (`fluss-daemon --render-overlay <dir>`).
//!
//! Look (docs/DESIGN.md): a black chamfered plate with a gold hairline, the
//! "iris" voice visual (radial ticks around a gold diamond), a mono status
//! label and the live text in Geist. Solid colours only.

use std::collections::HashMap;
use std::f32::consts::{PI, TAU};

use tiny_skia::*;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    Listening,
    HandsFree,
    Finishing,
    Loading(Option<f32>),
    NoticeOk,
    NoticeErr,
}

/// Logical canvas (scaled by DPI).
pub const CANVAS_W: f32 = 680.0;
pub const CANVAS_H: f32 = 96.0;
const PLATE_H: f32 = 44.0;
const CHAMFER: f32 = 9.0;
const IRIS_R: f32 = 14.0;
const TICKS: usize = 40;
const MAX_TEXT_W: f32 = 440.0;
const TEXT_PX: f32 = 14.5;
const LABEL_PX: f32 = 10.5;

type Rgb = [u8; 3];
const PLATE: Rgb = [11, 10, 8];
const GOLD: Rgb = [227, 176, 75];
const GOLD_DIM: Rgb = [140, 107, 42];
const RULE: Rgb = [58, 49, 31];
const TEXT: Rgb = [237, 230, 214];
const TEXT_2: Rgb = [154, 146, 132];
const DANGER: Rgb = [229, 72, 77];

pub struct Scene<'a> {
    pub kind: Kind,
    pub text: &'a str,
    /// Seconds since start (drives all motion).
    pub t: f32,
    /// Recent mic levels (newest last), 0..1 after normalization.
    pub levels: &'a [f32],
    /// 0..1 entrance.
    pub appear: f32,
    /// Animated plate width (logical px).
    pub width: f32,
    pub scale: f32,
    pub reduce_motion: bool,
}

struct Face {
    font: fontdue::Font,
    cache: HashMap<(char, u32), (fontdue::Metrics, Vec<u8>)>,
}

impl Face {
    fn new(bytes: &'static [u8]) -> Self {
        Self { font: fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default()).expect("embedded font"), cache: HashMap::new() }
    }

    fn glyph(&mut self, c: char, px: f32) -> (fontdue::Metrics, &[u8]) {
        let font = &self.font;
        let g = self.cache.entry((c, (px * 10.0) as u32)).or_insert_with(|| font.rasterize(c, px));
        (g.0, &g.1)
    }

    fn measure(&mut self, s: &str, px: f32, tracking: f32) -> f32 {
        let mut w = 0.0;
        let mut prev = None;
        for c in s.chars() {
            if let Some(p) = prev {
                w += self.font.horizontal_kern(p, c, px).unwrap_or(0.0);
            }
            w += self.glyph(c, px).0.advance_width + tracking;
            prev = Some(c);
        }
        w
    }

    /// Baseline at `y`. `clip` limits x; `fade` fades the first pixels after clip.0.
    #[allow(clippy::too_many_arguments)]
    fn draw(&mut self, pm: &mut Pixmap, s: &str, x: f32, y: f32, px: f32, tracking: f32, rgb: Rgb, alpha: f32, clip: (f32, f32), fade: f32) {
        let (w, h) = (pm.width() as i32, pm.height() as i32);
        let mut cx = x;
        let mut prev = None;
        let chars: Vec<char> = s.chars().collect();
        for c in chars {
            if let Some(p) = prev {
                cx += self.font.horizontal_kern(p, c, px).unwrap_or(0.0);
            }
            prev = Some(c);
            let (m, cov) = self.glyph(c, px);
            let cov = cov.to_vec();
            let gx = (cx + m.xmin as f32).round() as i32;
            let gy = (y - m.height as f32 - m.ymin as f32).round() as i32;
            cx += m.advance_width + tracking;
            let data = pm.data_mut();
            for row in 0..m.height as i32 {
                let py = gy + row;
                if py < 0 || py >= h {
                    continue;
                }
                for col in 0..m.width as i32 {
                    let pxx = gx + col;
                    if pxx < 0 || pxx >= w || (pxx as f32) < clip.0 || (pxx as f32) > clip.1 {
                        continue;
                    }
                    let mut a = cov[(row * m.width as i32 + col) as usize] as f32 / 255.0 * alpha;
                    if fade > 0.0 {
                        a *= ((pxx as f32 - clip.0) / fade).clamp(0.0, 1.0);
                    }
                    if a <= 0.0 {
                        continue;
                    }
                    let o = ((py * w + pxx) * 4) as usize;
                    for k in 0..3 {
                        let d = data[o + k] as f32;
                        data[o + k] = (rgb[k] as f32 * a + d * (1.0 - a)).round().min(255.0) as u8;
                    }
                    let d = data[o + 3] as f32;
                    data[o + 3] = (255.0 * a + d * (1.0 - a)).round().min(255.0) as u8;
                }
            }
        }
    }
}

pub struct Fonts {
    sans: Face,
    mono: Face,
}

impl Fonts {
    pub fn load() -> Self {
        Self {
            sans: Face::new(include_bytes!("../../../../assets/fonts/geist/Geist-Regular.ttf")),
            mono: Face::new(include_bytes!("../../../../assets/fonts/geist/GeistMono-Medium.ttf")),
        }
    }
}

fn paint(c: Rgb, a: f32) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color_rgba8(c[0], c[1], c[2], (a.clamp(0.0, 1.0) * 255.0).round() as u8);
    p.anti_alias = true;
    p
}

/// Plate with two chamfered corners (top-left, bottom-right).
fn plate(x: f32, y: f32, w: f32, h: f32, c: f32) -> Option<Path> {
    let mut p = PathBuilder::new();
    p.move_to(x + c, y);
    p.line_to(x + w, y);
    p.line_to(x + w, y + h - c);
    p.line_to(x + w - c, y + h);
    p.line_to(x, y + h);
    p.line_to(x, y + c);
    p.close();
    p.finish()
}

fn line(pm: &mut Pixmap, a: (f32, f32), b: (f32, f32), width: f32, c: Rgb, alpha: f32) {
    let mut pb = PathBuilder::new();
    pb.move_to(a.0, a.1);
    pb.line_to(b.0, b.1);
    if let Some(p) = pb.finish() {
        let st = Stroke { width, line_cap: LineCap::Butt, ..Default::default() };
        pm.stroke_path(&p, &paint(c, alpha), &st, Transform::identity(), None);
    }
}

fn label_for(kind: Kind) -> String {
    match kind {
        Kind::Listening => "REC".into(),
        Kind::HandsFree => "HANDS-FREE".into(),
        Kind::Finishing => "TYPING".into(),
        Kind::Loading(Some(p)) if p > 0.0 && p < 1.0 => format!("LOADING {:>2}%", (p * 100.0).round()),
        Kind::Loading(_) => "LOADING".into(),
        Kind::NoticeOk => "DONE".into(),
        Kind::NoticeErr => "ERROR".into(),
    }
}

fn layout(fonts: &mut Fonts, kind: Kind, text: &str, scale: f32) -> (f32, f32, f32) {
    // (label width, text width, total width) in logical px.
    let lw = fonts.mono.measure(&label_for(kind), LABEL_PX * scale, 1.2 * scale) / scale;
    let tw = if text.trim().is_empty() { 0.0 } else { fonts.sans.measure(text.trim(), TEXT_PX * scale, 0.0) / scale };
    let tw = tw.min(MAX_TEXT_W);
    let base = 14.0 + IRIS_R * 2.0 + 12.0 + lw + 16.0;
    let total = if tw > 0.0 { base + 1.0 + 14.0 + tw + 18.0 } else { base };
    (lw, tw, total)
}

/// Width the plate wants for this content (logical px).
pub fn content_width(fonts: &mut Fonts, kind: Kind, text: &str, _hint: &str) -> f32 {
    layout(fonts, kind, text, 1.0).2
}

/// The iris: radial ticks around a diamond.
fn iris(pm: &mut Pixmap, cx: f32, cy: f32, s: f32, sc: &Scene, alpha: f32) {
    let t = if sc.reduce_motion { 0.0 } else { sc.t };
    let r0 = 6.2 * s;
    let max_len = (IRIS_R - 7.0) * s;
    let n_lv = sc.levels.len().max(1);
    let (spin, sweep_speed) = match sc.kind {
        Kind::HandsFree => (t * 0.55, 0.0),
        Kind::Finishing => (0.0, 7.5),
        Kind::Loading(_) => (0.0, 4.2),
        _ => (t * TAU / 40.0, 0.0),
    };
    for i in 0..TICKS {
        let a = -PI / 2.0 + i as f32 * TAU / TICKS as f32 + spin;
        let (len, bright, color) = match sc.kind {
            Kind::Listening | Kind::HandsFree => {
                // Newest level at 12 o'clock, history sweeping clockwise.
                let h = n_lv - 1 - (i * n_lv / TICKS).min(n_lv - 1);
                let lv = sc.levels.get(h).copied().unwrap_or(0.0).clamp(0.0, 1.0);
                (0.18 + 0.82 * lv, 0.32 + 0.68 * lv, if lv > 0.12 { GOLD } else { GOLD_DIM })
            }
            Kind::Finishing | Kind::Loading(_) => {
                let head = (t * sweep_speed).rem_euclid(TAU);
                let pos = (i as f32 * TAU / TICKS as f32).rem_euclid(TAU);
                let d = (head - pos).rem_euclid(TAU);
                let lit = (1.0 - d / 2.2).max(0.0);
                let done = match sc.kind {
                    Kind::Loading(Some(p)) => pos <= p * TAU,
                    _ => false,
                };
                let b = if done { 0.85f32 } else { 0.22 + 0.78 * lit };
                (0.25 + 0.35 * lit, b, if lit > 0.3 || done { GOLD } else { GOLD_DIM })
            }
            Kind::NoticeOk | Kind::NoticeErr => {
                let top = i == 0;
                let c = if sc.kind == Kind::NoticeErr { DANGER } else { GOLD };
                (if top { 1.0 } else { 0.12 }, if top { 1.0 } else { 0.25 }, if top { c } else { GOLD_DIM })
            }
        };
        let (sn, cs) = a.sin_cos();
        let r1 = r0 + 0.8 * s;
        let r2 = r1 + (1.2 * s).max(len * max_len);
        line(pm, (cx + cs * r1, cy + sn * r1), (cx + cs * r2, cy + sn * r2), 1.0 * s, color, bright * alpha);
    }
    // Diamond core.
    let d = 3.2 * s * (1.0 + if matches!(sc.kind, Kind::Listening) { 0.12 * sc.levels.last().copied().unwrap_or(0.0) } else { 0.0 });
    let mut pb = PathBuilder::new();
    pb.move_to(cx, cy - d);
    pb.line_to(cx + d, cy);
    pb.line_to(cx, cy + d);
    pb.line_to(cx - d, cy);
    pb.close();
    if let Some(p) = pb.finish() {
        let core = if sc.kind == Kind::NoticeErr { DANGER } else { GOLD };
        if sc.kind == Kind::HandsFree {
            pm.stroke_path(&p, &paint(core, alpha), &Stroke { width: 1.2 * s, ..Default::default() }, Transform::identity(), None);
        } else {
            pm.fill_path(&p, &paint(core, alpha), FillRule::Winding, Transform::identity(), None);
        }
    }
}

pub fn draw(pm: &mut Pixmap, fonts: &mut Fonts, sc: &Scene) {
    pm.fill(Color::TRANSPARENT);
    let s = sc.scale;
    // Quick-out easing: snappy with a little smoothness.
    let e = 1.0 - (1.0 - sc.appear).powi(4);
    if e <= 0.001 {
        return;
    }
    let alpha = e;
    let (lw, tw, _) = layout(fonts, sc.kind, sc.text, s);
    let w = sc.width * s;
    let h = PLATE_H * s;
    let x = ((CANVAS_W * s - w) / 2.0).round();
    let y = ((CANVAS_H * s - h) / 2.0 + (1.0 - e) * 6.0 * s).round();

    // Plate + hairline; a 1px black outer line keeps the edge crisp on light backgrounds.
    if let Some(p) = plate(x - 1.0, y - 1.0, w + 2.0, h + 2.0, CHAMFER * s + 0.5) {
        pm.fill_path(&p, &paint([0, 0, 0], 0.55 * alpha), FillRule::Winding, Transform::identity(), None);
    }
    if let Some(p) = plate(x, y, w, h, CHAMFER * s) {
        pm.fill_path(&p, &paint(PLATE, 0.97 * alpha), FillRule::Winding, Transform::identity(), None);
        let edge = match sc.kind {
            Kind::NoticeErr => DANGER,
            Kind::Listening | Kind::HandsFree => GOLD_DIM,
            _ => RULE,
        };
        pm.stroke_path(&p, &paint(edge, alpha), &Stroke { width: 1.0 * s, ..Default::default() }, Transform::identity(), None);
    }
    // Gold index mark on the chamfer (HUD detail).
    line(pm, (x + 2.0 * s, y + CHAMFER * s + 3.0 * s), (x + 2.0 * s, y + h - 6.0 * s), 1.0 * s, GOLD, 0.0);
    line(pm, (x + w - 10.0 * s, y + 3.5 * s), (x + w - 4.0 * s, y + 3.5 * s), 1.0 * s, GOLD_DIM, alpha);

    let cy = y + h / 2.0;
    let icx = x + (14.0 + IRIS_R) * s;
    iris(pm, icx, cy, s, sc, alpha);

    // Status label (mono, uppercase).
    let lx = icx + IRIS_R * s + 12.0 * s;
    let label = label_for(sc.kind);
    let lcol = match sc.kind {
        Kind::NoticeErr => DANGER,
        Kind::Listening | Kind::HandsFree | Kind::NoticeOk => GOLD,
        _ => TEXT_2,
    };
    let lpx = LABEL_PX * s;
    fonts.mono.draw(pm, &label, lx, cy + 3.6 * s, lpx, 1.2 * s, lcol, alpha, (0.0, f32::MAX), 0.0);

    // Divider + text.
    if tw > 0.0 {
        let dx = (lx + lw * s + 16.0 * s).round() + 0.5;
        line(pm, (dx, cy - 9.0 * s), (dx, cy + 9.0 * s), 1.0 * s, RULE, alpha);
        let tx0 = dx + 14.0 * s;
        let right = x + w - 18.0 * s;
        let px = TEXT_PX * s;
        let text = sc.text.trim();
        let full = fonts.sans.measure(text, px, 0.0);
        let (start, fade) = if full > right - tx0 { (right - full, 26.0 * s) } else { (tx0, 0.0) };
        let col = if matches!(sc.kind, Kind::NoticeErr | Kind::NoticeOk) { TEXT } else { TEXT };
        fonts.sans.draw(pm, text, start, cy + 5.0 * s, px, 0.0, col, alpha, (tx0, right), fade);
    }
}
