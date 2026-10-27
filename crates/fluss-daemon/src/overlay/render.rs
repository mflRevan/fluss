//! Pure overlay renderer (tiny-skia + fontdue into an RGBA pixmap). Separate
//! from the window so every state can be rendered to PNG for design review
//! (`fluss-daemon --render-overlay <dir>`).

use std::collections::HashMap;

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
pub const CANVAS_W: f32 = 640.0;
pub const CANVAS_H: f32 = 104.0;
const PILL_H: f32 = 42.0;
const BARS: usize = 13;
const MAX_TEXT_W: f32 = 420.0;

pub struct Scene<'a> {
    pub kind: Kind,
    pub text: &'a str,
    pub hint: &'a str,
    /// Seconds since start (drives all motion).
    pub t: f32,
    /// Recent mic levels (newest last), 0..1 after normalization.
    pub levels: &'a [f32],
    /// 0..1 fade/slide.
    pub appear: f32,
    /// Animated pill width (logical px).
    pub width: f32,
    pub scale: f32,
    pub reduce_motion: bool,
}

pub struct Fonts {
    font: Option<fontdue::Font>,
    cache: HashMap<(char, u32), (fontdue::Metrics, Vec<u8>)>,
}

impl Fonts {
    pub fn load() -> Self {
        let candidates: &[&str] = if cfg!(windows) {
            &["C:\\Windows\\Fonts\\segoeui.ttf", "C:\\Windows\\Fonts\\arial.ttf"]
        } else if cfg!(target_os = "macos") {
            &["/System/Library/Fonts/SFNS.ttf", "/Library/Fonts/Arial.ttf"]
        } else {
            &["/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf", "/usr/share/fonts/TTF/DejaVuSans.ttf"]
        };
        let font = candidates.iter().find_map(|p| {
            let bytes = std::fs::read(p).ok()?;
            fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default()).ok()
        });
        Self { font, cache: HashMap::new() }
    }

    fn glyph(&mut self, c: char, px: f32) -> Option<&(fontdue::Metrics, Vec<u8>)> {
        let font = self.font.as_ref()?;
        let key = (c, (px * 10.0) as u32);
        Some(self.cache.entry(key).or_insert_with(|| font.rasterize(c, px)))
    }

    pub fn measure(&mut self, s: &str, px: f32) -> f32 {
        let mut w = 0.0;
        let mut prev = None;
        for c in s.chars() {
            if let (Some(p), Some(f)) = (prev, self.font.as_ref()) {
                w += f.horizontal_kern(p, c, px).unwrap_or(0.0);
            }
            if let Some((m, _)) = self.glyph(c, px) {
                w += m.advance_width;
            }
            prev = Some(c);
        }
        w
    }

    /// Draw text with its baseline at `y`; `fade_left` fades the first `fade_w`
    /// pixels when the text is clipped on the left.
    #[allow(clippy::too_many_arguments)]
    fn draw(&mut self, pm: &mut Pixmap, s: &str, x: f32, y: f32, px: f32, rgba: [f32; 4], clip: (f32, f32), fade_w: f32) {
        let (w, h) = (pm.width() as i32, pm.height() as i32);
        let mut cx = x;
        let mut prev = None;
        let kern: Vec<f32> = {
            let f = self.font.as_ref();
            let mut v = Vec::new();
            let mut p: Option<char> = None;
            for c in s.chars() {
                v.push(match (p, f) {
                    (Some(a), Some(f)) => f.horizontal_kern(a, c, px).unwrap_or(0.0),
                    _ => 0.0,
                });
                p = Some(c);
            }
            v
        };
        for (i, c) in s.chars().enumerate() {
            let _ = prev;
            cx += kern[i];
            let Some((m, cov)) = self.glyph(c, px).map(|(m, c)| (*m, c.clone())) else { continue };
            let gx = (cx + m.xmin as f32).round() as i32;
            let gy = (y - m.height as f32 - m.ymin as f32).round() as i32;
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
                    let mut a = cov[(row * m.width as i32 + col) as usize] as f32 / 255.0 * rgba[3];
                    if fade_w > 0.0 {
                        a *= ((pxx as f32 - clip.0) / fade_w).clamp(0.0, 1.0);
                    }
                    if a <= 0.0 {
                        continue;
                    }
                    let o = ((py * w + pxx) * 4) as usize;
                    for k in 0..3 {
                        let d = data[o + k] as f32;
                        data[o + k] = (rgba[k] * 255.0 * a + d * (1.0 - a)).round().min(255.0) as u8;
                    }
                    let d = data[o + 3] as f32;
                    data[o + 3] = (255.0 * a + d * (1.0 - a)).round().min(255.0) as u8;
                }
            }
            cx += m.advance_width;
            prev = Some(c);
        }
    }
}

fn rrect(x: f32, y: f32, w: f32, h: f32, r: f32) -> Option<Path> {
    let r = r.min(w / 2.0).min(h / 2.0);
    let k = 0.552_284_8 * r;
    let mut p = PathBuilder::new();
    p.move_to(x + r, y);
    p.line_to(x + w - r, y);
    p.cubic_to(x + w - r + k, y, x + w, y + r - k, x + w, y + r);
    p.line_to(x + w, y + h - r);
    p.cubic_to(x + w, y + h - r + k, x + w - r + k, y + h, x + w - r, y + h);
    p.line_to(x + r, y + h);
    p.cubic_to(x + r - k, y + h, x, y + h - r + k, x, y + h - r);
    p.line_to(x, y + r);
    p.cubic_to(x, y + r - k, x + r - k, y, x + r, y);
    p.close();
    p.finish()
}

fn paint(c: [u8; 4]) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color_rgba8(c[0], c[1], c[2], c[3]);
    p.anti_alias = true;
    p
}

fn arc(cx: f32, cy: f32, r: f32, a0: f32, sweep: f32) -> Option<Path> {
    let mut p = PathBuilder::new();
    let n = 28;
    for i in 0..=n {
        let a = a0 + sweep * i as f32 / n as f32;
        let (x, y) = (cx + r * a.cos(), cy + r * a.sin());
        if i == 0 {
            p.move_to(x, y);
        } else {
            p.line_to(x, y);
        }
    }
    p.finish()
}

fn lerp_color(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

const TEAL: [f32; 3] = [0.176, 0.831, 0.749];
const SKY: [f32; 3] = [0.220, 0.741, 0.973];
const VIOLET: [f32; 3] = [0.655, 0.545, 0.980];

fn bar_color(i: usize) -> [f32; 3] {
    let t = i as f32 / (BARS - 1) as f32;
    if t < 0.5 { lerp_color(TEAL, SKY, t * 2.0) } else { lerp_color(SKY, VIOLET, (t - 0.5) * 2.0) }
}

/// Width the pill wants for this content (logical px).
pub fn content_width(fonts: &mut Fonts, kind: Kind, text: &str, hint: &str) -> f32 {
    let label = label_for(kind, text, hint);
    if matches!(kind, Kind::NoticeOk | Kind::NoticeErr) {
        return 46.0 + fonts.measure(&label, 14.0).min(MAX_TEXT_W) + 22.0;
    }
    let base = 20.0 + 14.0 + 16.0 + BARS as f32 * 5.0 - 2.0 + 18.0;
    if label.is_empty() {
        return base;
    }
    let tw = fonts.measure(&label, 14.0).min(MAX_TEXT_W);
    base + 4.0 + tw + 4.0
}

fn label_for(kind: Kind, text: &str, hint: &str) -> String {
    match kind {
        Kind::Loading(p) => match p {
            Some(p) if p > 0.0 && p < 1.0 => format!("Loading model · {}%", (p * 100.0).round()),
            _ => "Loading model…".into(),
        },
        Kind::NoticeOk | Kind::NoticeErr => text.to_string(),
        _ if !text.trim().is_empty() => text.trim().to_string(),
        _ => hint.to_string(),
    }
}

pub fn draw(pm: &mut Pixmap, fonts: &mut Fonts, sc: &Scene) {
    pm.fill(Color::TRANSPARENT);
    let s = sc.scale;
    let ease = 1.0 - (1.0 - sc.appear).powi(3);
    let alpha = ease;
    if alpha <= 0.001 {
        return;
    }
    let cw = CANVAS_W * s;
    let ch = CANVAS_H * s;
    let w = sc.width * s;
    let h = PILL_H * s;
    let x = (cw - w) / 2.0;
    let y = (ch - h) / 2.0 + (1.0 - ease) * 10.0 * s;
    let a8 = |a: f32| (a * alpha * 255.0).round().clamp(0.0, 255.0) as u8;

    // Soft shadow.
    for i in 0..8 {
        let g = i as f32 * 1.6 * s;
        if let Some(p) = rrect(x - g, y - g + 3.0 * s, w + 2.0 * g, h + 2.0 * g, h / 2.0 + g) {
            pm.fill_path(&p, &paint([0, 0, 0, a8(0.07 * (1.0 - i as f32 / 8.0))]), FillRule::Winding, Transform::identity(), None);
        }
    }
    // Body: dark glass with a subtle vertical gradient.
    if let Some(p) = rrect(x, y, w, h, h / 2.0) {
        let mut body = paint([0, 0, 0, 255]);
        body.shader = LinearGradient::new(
            Point::from_xy(0.0, y),
            Point::from_xy(0.0, y + h),
            vec![
                GradientStop::new(0.0, Color::from_rgba(0.135, 0.150, 0.190, 0.95 * alpha).unwrap()),
                GradientStop::new(1.0, Color::from_rgba(0.075, 0.085, 0.110, 0.95 * alpha).unwrap()),
            ],
            SpreadMode::Pad,
            Transform::identity(),
        )
        .unwrap_or(Shader::SolidColor(Color::BLACK));
        pm.fill_path(&p, &body, FillRule::Winding, Transform::identity(), None);
        let mut border = paint([255, 255, 255, a8(0.10)]);
        border.shader = LinearGradient::new(
            Point::from_xy(0.0, y),
            Point::from_xy(0.0, y + h),
            vec![
                GradientStop::new(0.0, Color::from_rgba(1.0, 1.0, 1.0, 0.16 * alpha).unwrap()),
                GradientStop::new(1.0, Color::from_rgba(1.0, 1.0, 1.0, 0.05 * alpha).unwrap()),
            ],
            SpreadMode::Pad,
            Transform::identity(),
        )
        .unwrap_or(Shader::SolidColor(Color::WHITE));
        pm.stroke_path(&p, &border, &Stroke { width: 1.0 * s, ..Default::default() }, Transform::identity(), None);
    }

    let cy = y + h / 2.0;
    let ix = x + 20.0 * s;
    let t = if sc.reduce_motion { 0.0 } else { sc.t };

    // Indicator.
    match sc.kind {
        Kind::Listening | Kind::HandsFree => {
            let col: [u8; 3] = if sc.kind == Kind::Listening { [255, 99, 125] } else { [255, 190, 80] };
            let ph = (t / 1.3).fract();
            if let Some(p) = PathBuilder::from_circle(ix, cy, (5.0 + 8.0 * ph) * s) {
                pm.fill_path(&p, &paint([col[0], col[1], col[2], a8(0.32 * (1.0 - ph))]), FillRule::Winding, Transform::identity(), None);
            }
            if let Some(p) = PathBuilder::from_circle(ix, cy, 5.0 * s) {
                pm.fill_path(&p, &paint([col[0], col[1], col[2], a8(1.0)]), FillRule::Winding, Transform::identity(), None);
            }
            if sc.kind == Kind::HandsFree {
                // Tiny lock shackle above the dot.
                if let Some(p) = arc(ix, cy - 5.5 * s, 3.2 * s, std::f32::consts::PI, std::f32::consts::PI) {
                    let st = Stroke { width: 1.6 * s, line_cap: LineCap::Round, ..Default::default() };
                    pm.stroke_path(&p, &paint([col[0], col[1], col[2], a8(1.0)]), &st, Transform::identity(), None);
                }
            }
        }
        Kind::Finishing | Kind::Loading(_) => {
            let col: [u8; 3] = if matches!(sc.kind, Kind::Finishing) { [56, 189, 248] } else { [167, 139, 250] };
            let st = Stroke { width: 2.2 * s, line_cap: LineCap::Round, ..Default::default() };
            if let Some(p) = PathBuilder::from_circle(ix, cy, 6.5 * s) {
                pm.stroke_path(&p, &paint([col[0], col[1], col[2], a8(0.18)]), &Stroke { width: 2.2 * s, ..Default::default() }, Transform::identity(), None);
            }
            let sweep = match sc.kind {
                Kind::Loading(Some(p)) if p > 0.02 => std::f32::consts::TAU * p.clamp(0.0, 1.0),
                _ => 4.2,
            };
            if let Some(p) = arc(ix, cy, 6.5 * s, t * 5.5, sweep) {
                pm.stroke_path(&p, &paint([col[0], col[1], col[2], a8(1.0)]), &st, Transform::identity(), None);
            }
        }
        Kind::NoticeOk | Kind::NoticeErr => {
            let col: [u8; 3] = if sc.kind == Kind::NoticeOk { [52, 211, 153] } else { [251, 113, 133] };
            if let Some(p) = PathBuilder::from_circle(ix, cy, 8.0 * s) {
                pm.fill_path(&p, &paint([col[0], col[1], col[2], a8(0.20)]), FillRule::Winding, Transform::identity(), None);
            }
            let st = Stroke { width: 2.0 * s, line_cap: LineCap::Round, line_join: LineJoin::Round, ..Default::default() };
            let mut pb = PathBuilder::new();
            if sc.kind == Kind::NoticeOk {
                pb.move_to(ix - 3.5 * s, cy + 0.2 * s);
                pb.line_to(ix - 1.0 * s, cy + 2.8 * s);
                pb.line_to(ix + 3.8 * s, cy - 2.6 * s);
            } else {
                pb.move_to(ix, cy - 3.8 * s);
                pb.line_to(ix, cy + 0.8 * s);
                pb.move_to(ix, cy + 3.6 * s);
                pb.line_to(ix, cy + 3.7 * s);
            }
            if let Some(p) = pb.finish() {
                pm.stroke_path(&p, &paint([col[0], col[1], col[2], a8(1.0)]), &st, Transform::identity(), None);
            }
        }
    }

    // Level bars: a scrolling waveform from recent mic levels.
    let bx = ix + 18.0 * s;
    let notice = matches!(sc.kind, Kind::NoticeOk | Kind::NoticeErr);
    let bars_end = if notice {
        bx - 6.0 * s
    } else {
        for i in 0..BARS {
            let lvl = sc.levels.get(sc.levels.len().saturating_sub(BARS) + i).copied().unwrap_or(0.0);
            let shape = 0.55 + 0.45 * (1.0 - ((i as f32 - (BARS - 1) as f32 / 2.0).abs() / ((BARS - 1) as f32 / 2.0)).powi(2));
            let amp = match sc.kind {
                Kind::Listening | Kind::HandsFree => lvl * shape,
                Kind::Finishing => 0.18 + 0.16 * ((t * 7.0 - i as f32 * 0.55).sin() * 0.5 + 0.5),
                Kind::Loading(_) => 0.10 + 0.10 * ((t * 4.0 - i as f32 * 0.45).sin() * 0.5 + 0.5),
                _ => 0.0,
            };
            let bh = (4.0 + 20.0 * amp.clamp(0.0, 1.0)) * s;
            let bw = 3.0 * s;
            let xx = bx + i as f32 * 5.0 * s;
            let c = bar_color(i);
            let dim = if matches!(sc.kind, Kind::Loading(_)) { 0.55 } else { 1.0 };
            if let Some(p) = rrect(xx, cy - bh / 2.0, bw, bh, bw / 2.0) {
                let mut pt = Paint::default();
                pt.anti_alias = true;
                pt.set_color(Color::from_rgba(c[0], c[1], c[2], alpha * dim).unwrap());
                pm.fill_path(&p, &pt, FillRule::Winding, Transform::identity(), None);
            }
        }
        bx + (BARS as f32 * 5.0 - 2.0) * s
    };

    // Text.
    let label = label_for(sc.kind, sc.text, sc.hint);
    if !label.is_empty() {
        let px = 14.0 * s;
        let tx0 = bars_end + 14.0 * s;
        let avail_right = x + w - 18.0 * s;
        let avail = (avail_right - tx0).max(0.0);
        let tw = fonts.measure(&label, px);
        let (start_x, fade) = if tw > avail { (avail_right - tw, 22.0 * s) } else { (tx0, 0.0) };
        let dim_hint = sc.text.trim().is_empty() && !notice && !matches!(sc.kind, Kind::Loading(_));
        let col = if dim_hint { [0.62, 0.66, 0.74, 1.0] } else { [0.93, 0.945, 0.965, 1.0] };
        let baseline = cy + 5.0 * s;
        fonts.draw(pm, &label, start_x, baseline, px, [col[0], col[1], col[2], alpha], (tx0, avail_right), fade);
    }
}
