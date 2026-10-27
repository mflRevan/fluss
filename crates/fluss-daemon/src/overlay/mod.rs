//! The dictation overlay: a small pill at the bottom of the active monitor.
//! Native layered window (no webview), never takes focus, click-through,
//! animates only while visible and sleeps otherwise.

pub mod render;

use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender};
use fluss_core::config::{OverlayPosition, Ui};

use crate::audio::Level;
use render::{Fonts, Kind, Scene};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OvState {
    Hidden,
    Listening,
    HandsFree,
    Finishing,
    Loading { progress: Option<f32> },
}

enum Msg {
    State(OvState),
    Text(String),
    Notice(String, bool),
    Configure(Ui),
}

#[derive(Clone)]
pub struct Overlay {
    tx: Sender<Msg>,
    pub level: Level,
}

impl Overlay {
    pub fn spawn() -> Self {
        let (tx, rx) = crossbeam_channel::unbounded();
        let level = Level::default();
        let l2 = level.clone();
        std::thread::Builder::new().name("fluss-overlay".into()).spawn(move || run(rx, l2)).expect("spawn overlay");
        Self { tx, level }
    }

    pub fn show(&self, s: OvState) {
        let _ = self.tx.send(Msg::State(s));
    }
    pub fn hide(&self) {
        let _ = self.tx.send(Msg::State(OvState::Hidden));
    }
    pub fn text(&self, t: &str) {
        let _ = self.tx.send(Msg::Text(t.to_string()));
    }
    pub fn notice(&self, t: &str, error: bool) {
        let _ = self.tx.send(Msg::Notice(t.to_string(), error));
    }
    /// End of a session with a message: the notice shows briefly, then the overlay
    /// fades out (it must not fall back to the session's last state).
    pub fn flash(&self, t: &str, error: bool) {
        let _ = self.tx.send(Msg::State(OvState::Hidden));
        let _ = self.tx.send(Msg::Notice(t.to_string(), error));
    }
    pub fn configure(&self, ui: &Ui) {
        let _ = self.tx.send(Msg::Configure(ui.clone()));
    }
}

struct Anim {
    state: OvState,
    text: String,
    notice: Option<(String, bool, Instant)>,
    appear: f32,
    width: f32,
    levels: Vec<f32>,
    peak: f32,
    open: f32,
    spin: f32,
    t0: Instant,
    ui: Ui,
}

impl Anim {
    fn kind(&self) -> Option<Kind> {
        if let Some((_, err, _)) = &self.notice {
            return Some(if *err { Kind::NoticeErr } else { Kind::NoticeOk });
        }
        match self.state {
            OvState::Hidden => None,
            OvState::Listening => Some(Kind::Listening),
            OvState::HandsFree => Some(Kind::HandsFree),
            OvState::Finishing => Some(Kind::Finishing),
            OvState::Loading { progress } => Some(Kind::Loading(progress)),
        }
    }

    fn visible_target(&self) -> bool {
        self.ui.overlay && self.kind().is_some()
    }
}

fn hint(state: OvState) -> &'static str {
    match state {
        OvState::HandsFree => "Hands-free",
        _ => "",
    }
}

fn run(rx: Receiver<Msg>, level: Level) {
    let mut fonts = Fonts::load();
    let mut win = imp::Window::new();
    let mut a = Anim {
        state: OvState::Hidden,
        text: String::new(),
        notice: None,
        appear: 0.0,
        width: 120.0,
        levels: vec![0.0; 32],
        peak: 0.02,
        open: 0.13,
        spin: 0.0,
        t0: Instant::now(),
        ui: Ui::default(),
    };
    let mut last = Instant::now();
    loop {
        let animating = a.appear > 0.0 || a.visible_target();
        let msg = if animating {
            match rx.recv_timeout(Duration::from_millis(14)) {
                Ok(m) => Some(m),
                Err(crossbeam_channel::RecvTimeoutError::Timeout) => None,
                Err(_) => return,
            }
        } else {
            match rx.recv() {
                Ok(m) => {
                    last = Instant::now();
                    Some(m)
                }
                Err(_) => return,
            }
        };
        let mut msgs: Vec<Msg> = msg.into_iter().collect();
        msgs.extend(rx.try_iter());
        for m in msgs {
            match m {
                Msg::State(s) => {
                    if a.state == OvState::Hidden && s != OvState::Hidden {
                        a.text.clear();
                        a.levels.iter_mut().for_each(|l| *l = 0.0);
                        win.place(a.ui.overlay_position);
                    }
                    if s == OvState::Hidden {
                        a.text.clear();
                    }
                    a.state = s;
                }
                Msg::Text(t) => a.text = t,
                Msg::Notice(t, err) => {
                    if a.appear <= 0.0 {
                        win.place(a.ui.overlay_position);
                    }
                    a.notice = Some((t, err, Instant::now()));
                }
                Msg::Configure(ui) => a.ui = ui,
            }
        }
        if let Some((_, err, at)) = &a.notice {
            if at.elapsed() > Duration::from_millis(if *err { 2600 } else { 1300 }) {
                a.notice = None;
            }
        }

        let now = Instant::now();
        let dt = (now - last).as_secs_f32().min(0.05);
        last = now;
        // Level history with automatic gain so quiet mics still animate nicely.
        let raw = level.get();
        a.peak = (a.peak * 0.995).max(raw).max(0.02);
        let norm = (raw / a.peak).sqrt().clamp(0.0, 1.0);
        let prev = *a.levels.last().unwrap_or(&0.0);
        a.levels.remove(0);
        a.levels.push(prev + (norm - prev) * 0.55);

        let target_appear = if a.visible_target() { 1.0 } else { 0.0 };
        let speed = if target_appear > a.appear { 8.5 } else { 11.0 };
        a.appear = if a.ui.reduce_motion {
            target_appear
        } else if target_appear > a.appear {
            (a.appear + dt * speed).min(1.0)
        } else {
            (a.appear - dt * speed).max(0.0)
        };
        let Some(kind) = a.kind().or(if a.appear > 0.0 { Some(Kind::Finishing) } else { None }) else {
            win.hide();
            continue;
        };
        let text = match &a.notice {
            Some((t, _, _)) => t.clone(),
            None if a.ui.overlay_preview => a.text.clone(),
            None => String::new(),
        };
        let target_w = render::content_width(&mut fonts, kind, &text, hint(a.state));
        a.width = if a.ui.reduce_motion || a.appear < 0.05 { target_w } else { a.width + (target_w - a.width) * (1.0 - (-dt * 26.0).exp()) };
        if a.appear <= 0.0 {
            win.hide();
            continue;
        }
        let t = a.t0.elapsed().as_secs_f32();
        let target_open = render::aperture_target(kind, &a.levels, t);
        if a.ui.reduce_motion {
            a.open = target_open;
        } else {
            a.open += (target_open - a.open) * (1.0 - (-dt * 18.0).exp());
            a.spin = (a.spin + dt * render::aperture_spin_speed(kind)) % std::f32::consts::TAU;
        }
        let scene = Scene {
            kind,
            text: &text,
            t,
            appear: a.appear,
            width: a.width,
            scale: win.scale(),
            reduce_motion: a.ui.reduce_motion,
            open: a.open,
            spin: a.spin,
        };
        win.present(&mut fonts, &scene);
    }
}

/// Render every overlay state to PNGs (design review / docs).
pub fn render_gallery(dir: &std::path::Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(dir)?;
    let mut fonts = Fonts::load();
    let levels: Vec<f32> = (0..32).map(|i| (0.35 + 0.6 * ((i as f32 * 0.7).sin() * 0.5 + 0.5)).min(1.0)).collect();
    let cases: Vec<(&str, Kind, &str)> = vec![
        ("listening", Kind::Listening, ""),
        ("listening-text", Kind::Listening, "Hey team, quick update on the release"),
        ("handsfree", Kind::HandsFree, ""),
        ("long-text", Kind::Listening, "so the build passed this morning and we can ship on Thursday if nothing else comes up in review"),
        ("finishing", Kind::Finishing, ""),
        ("loading", Kind::Loading(Some(0.42)), ""),
        ("notice-ok", Kind::NoticeOk, "Saved: Omarchy"),
        ("notice-err", Kind::NoticeErr, "Microphone unavailable"),
    ];
    for scale in [1.0f32, 1.5] {
        for (name, kind, text) in &cases {
            let w = render::content_width(&mut fonts, *kind, text, if *kind == Kind::HandsFree { "Hands-free" } else { "" });
            let mut pm = tiny_skia::Pixmap::new((render::CANVAS_W * scale) as u32, (render::CANVAS_H * scale) as u32).unwrap();
            // A desktop-ish backdrop: half light, half dark, to check contrast on both.
            pm.fill(tiny_skia::Color::from_rgba8(232, 230, 226, 255));
            let half = tiny_skia::Rect::from_xywh(pm.width() as f32 / 2.0, 0.0, pm.width() as f32 / 2.0, pm.height() as f32).unwrap();
            let mut dark = tiny_skia::Paint::default();
            dark.set_color_rgba8(30, 30, 32, 255);
            pm.fill_rect(half, &dark, tiny_skia::Transform::identity(), None);
            let mut fg = tiny_skia::Pixmap::new(pm.width(), pm.height()).unwrap();
            let open = render::aperture_target(*kind, &levels, 0.35);
            let sc = Scene { kind: *kind, text, t: 0.35, appear: 1.0, width: w, scale, reduce_motion: false, open, spin: 0.4 };
            render::draw(&mut fg, &mut fonts, &sc);
            pm.draw_pixmap(0, 0, fg.as_ref(), &tiny_skia::PixmapPaint::default(), tiny_skia::Transform::identity(), None);
            pm.save_png(dir.join(format!("{name}@{scale}x.png")))?;
        }
    }
    Ok(())
}

#[cfg(windows)]
mod imp {
    use super::*;
    use windows::core::w;
    use windows::Win32::Foundation::*;
    use windows::Win32::Graphics::Gdi::*;
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
    use windows::Win32::UI::WindowsAndMessaging::*;

    pub struct Window {
        hwnd: HWND,
        scale: f32,
        pos: (i32, i32),
        shown: bool,
        pm: Option<tiny_skia::Pixmap>,
        dib: Option<(HDC, HBITMAP, *mut u8, i32, i32)>,
    }

    unsafe extern "system" fn proc(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT {
        if m == WM_MOUSEACTIVATE {
            return LRESULT(MA_NOACTIVATE as isize);
        }
        DefWindowProcW(h, m, w, l)
    }

    impl Window {
        pub fn new() -> Self {
            unsafe {
                let hinst = GetModuleHandleW(None).unwrap_or_default();
                let wc = WNDCLASSW { lpfnWndProc: Some(proc), hInstance: hinst.into(), lpszClassName: w!("FlussOverlay"), ..Default::default() };
                RegisterClassW(&wc);
                let hwnd = CreateWindowExW(
                    WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                    w!("FlussOverlay"),
                    w!("Fluss"),
                    WS_POPUP,
                    0,
                    0,
                    10,
                    10,
                    None,
                    None,
                    Some(hinst.into()),
                    None,
                )
                .unwrap_or_default();
                Self { hwnd, scale: 1.0, pos: (0, 0), shown: false, pm: None, dib: None }
            }
        }

        pub fn scale(&self) -> f32 {
            self.scale
        }

        /// Position on the monitor the user is working on.
        pub fn place(&mut self, at: OverlayPosition) {
            unsafe {
                let fg = GetForegroundWindow();
                let mon = MonitorFromWindow(fg, MONITOR_DEFAULTTOPRIMARY);
                let mut mi = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
                let _ = GetMonitorInfoW(mon, &mut mi);
                let (mut dx, mut dy) = (96u32, 96u32);
                let _ = GetDpiForMonitor(mon, MDT_EFFECTIVE_DPI, &mut dx, &mut dy);
                let scale = dx as f32 / 96.0;
                if (scale - self.scale).abs() > 0.01 || self.pm.is_none() {
                    self.scale = scale;
                    self.alloc();
                }
                let w = (render::CANVAS_W * self.scale) as i32;
                let h = (render::CANVAS_H * self.scale) as i32;
                let r = mi.rcWork;
                let x = r.left + (r.right - r.left - w) / 2;
                let y = match at {
                    OverlayPosition::Bottom => r.bottom - h - (28.0 * self.scale) as i32,
                    OverlayPosition::Top => r.top + (20.0 * self.scale) as i32,
                };
                self.pos = (x, y);
            }
        }

        fn alloc(&mut self) {
            unsafe {
                if let Some((dc, bmp, _, _, _)) = self.dib.take() {
                    let _ = DeleteObject(bmp.into());
                    let _ = DeleteDC(dc);
                }
                let w = (render::CANVAS_W * self.scale) as i32;
                let h = (render::CANVAS_H * self.scale) as i32;
                self.pm = tiny_skia::Pixmap::new(w as u32, h as u32);
                let screen = GetDC(None);
                let dc = CreateCompatibleDC(Some(screen));
                ReleaseDC(None, screen);
                let bi = BITMAPINFO {
                    bmiHeader: BITMAPINFOHEADER {
                        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                        biWidth: w,
                        biHeight: -h,
                        biPlanes: 1,
                        biBitCount: 32,
                        biCompression: BI_RGB.0,
                        ..Default::default()
                    },
                    ..Default::default()
                };
                let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
                if let Ok(bmp) = CreateDIBSection(Some(dc), &bi, DIB_RGB_COLORS, &mut bits, None, 0) {
                    SelectObject(dc, bmp.into());
                    self.dib = Some((dc, bmp, bits as *mut u8, w, h));
                }
            }
        }

        pub fn present(&mut self, fonts: &mut Fonts, scene: &Scene) {
            if self.pm.is_none() {
                self.place(OverlayPosition::Bottom);
            }
            let (Some(pm), Some((dc, _, bits, w, h))) = (self.pm.as_mut(), self.dib) else { return };
            render::draw(pm, fonts, scene);
            unsafe {
                // tiny-skia: premultiplied RGBA -> GDI: premultiplied BGRA.
                let src = pm.data();
                let dst = std::slice::from_raw_parts_mut(bits, src.len());
                for (d, s) in dst.chunks_exact_mut(4).zip(src.chunks_exact(4)) {
                    d[0] = s[2];
                    d[1] = s[1];
                    d[2] = s[0];
                    d[3] = s[3];
                }
                let pt_dst = POINT { x: self.pos.0, y: self.pos.1 };
                let size = SIZE { cx: w, cy: h };
                let pt_src = POINT { x: 0, y: 0 };
                let blend = BLENDFUNCTION { BlendOp: AC_SRC_OVER as u8, BlendFlags: 0, SourceConstantAlpha: 255, AlphaFormat: AC_SRC_ALPHA as u8 };
                let screen = GetDC(None);
                let _ = UpdateLayeredWindow(self.hwnd, Some(screen), Some(&pt_dst), Some(&size), Some(dc), Some(&pt_src), COLORREF(0), Some(&blend), ULW_ALPHA);
                ReleaseDC(None, screen);
                if !self.shown {
                    let _ = ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
                    let _ = SetWindowPos(self.hwnd, Some(HWND_TOPMOST), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
                    self.shown = true;
                }
                // Drain window messages for our thread.
                let mut msg = MSG::default();
                while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                    DispatchMessageW(&msg);
                }
            }
        }

        pub fn hide(&mut self) {
            if self.shown {
                unsafe {
                    let _ = ShowWindow(self.hwnd, SW_HIDE);
                }
                self.shown = false;
            }
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;
    pub struct Window;
    impl Window {
        pub fn new() -> Self {
            Window
        }
        pub fn scale(&self) -> f32 {
            1.0
        }
        pub fn place(&mut self, _at: OverlayPosition) {}
        pub fn present(&mut self, _fonts: &mut Fonts, _scene: &Scene) {}
        pub fn hide(&mut self) {}
    }
}
