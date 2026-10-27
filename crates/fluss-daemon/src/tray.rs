//! Tray icon + menu. Lives on the main thread (Windows needs a message loop
//! there); the controller sends it status snapshots through a channel.

use crossbeam_channel::{Receiver, Sender};
use fluss_core::protocol::{DictationPhase, ModelPhase, Status};

use crate::app::Ev;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TrayCmd {
    Open,
    Dictionary,
    AddWord,
    ToggleHandsFree,
    LoadModel,
    UnloadModel,
    PauseHotkeys(bool),
    Quit,
}

#[derive(Clone)]
pub struct TrayHandle {
    tx: Sender<Status>,
}

impl TrayHandle {
    pub fn update(&self, st: &Status) {
        let _ = self.tx.send(st.clone());
    }
}

pub fn channel() -> (TrayHandle, Receiver<Status>) {
    let (tx, rx) = crossbeam_channel::unbounded();
    (TrayHandle { tx }, rx)
}

#[derive(Clone, Copy, PartialEq)]
enum Look {
    Idle,
    Off,
    Busy,
    Live,
    Problem,
}

fn look(st: &Status) -> Look {
    match (st.phase, st.model.phase) {
        (DictationPhase::Listening | DictationPhase::HandsFree, _) => Look::Live,
        (_, ModelPhase::Error) => Look::Problem,
        (_, ModelPhase::NotInstalled) if st.onboarded => Look::Problem,
        (DictationPhase::Finishing, _) | (_, ModelPhase::Loading | ModelPhase::Installing) => Look::Busy,
        (_, ModelPhase::Ready) => Look::Idle,
        _ => Look::Off,
    }
}

/// 32x32 RGBA: a small iris (radial ticks around a diamond) in gold.
/// Idle: dim ring. Live: full bright ring. Busy: half ring lit. Problem: red core.
fn icon_rgba(l: Look) -> Vec<u8> {
    use tiny_skia::*;
    const GOLD: (u8, u8, u8) = (227, 176, 75);
    const DIM: (u8, u8, u8) = (140, 107, 42);
    let mut pm = Pixmap::new(32, 32).unwrap();
    let (cx, cy) = (16.0f32, 16.0f32);
    let n = 16;
    for i in 0..n {
        let a = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::TAU / n as f32;
        let lit = match l {
            Look::Live => true,
            Look::Busy => i < n / 2,
            Look::Idle => i % 4 == 0,
            _ => false,
        };
        let (c, r2) = if lit { (GOLD, 15.0) } else { (DIM, 12.5) };
        let (s, co) = a.sin_cos();
        let mut pb = PathBuilder::new();
        pb.move_to(cx + co * 8.5, cy + s * 8.5);
        pb.line_to(cx + co * r2, cy + s * r2);
        let mut p = Paint::default();
        p.anti_alias = true;
        p.set_color_rgba8(c.0, c.1, c.2, if l == Look::Off { 150 } else { 255 });
        pm.stroke_path(&pb.finish().unwrap(), &p, &Stroke { width: 2.0, ..Default::default() }, Transform::identity(), None);
    }
    let core = if l == Look::Problem { (229, 72, 77) } else if l == Look::Off { DIM } else { GOLD };
    let mut pb = PathBuilder::new();
    pb.move_to(cx, cy - 5.5);
    pb.line_to(cx + 5.5, cy);
    pb.line_to(cx, cy + 5.5);
    pb.line_to(cx - 5.5, cy);
    pb.close();
    let mut p = Paint::default();
    p.anti_alias = true;
    p.set_color_rgba8(core.0, core.1, core.2, 255);
    pm.fill_path(&pb.finish().unwrap(), &p, FillRule::Winding, Transform::identity(), None);
    // Un-premultiply for the icon API.
    let mut out = pm.data().to_vec();
    for px in out.chunks_exact_mut(4) {
        let a = px[3] as f32 / 255.0;
        if a > 0.0 {
            for c in &mut px[..3] {
                *c = ((*c as f32 / a).round()).min(255.0) as u8;
            }
        }
    }
    out
}
fn tooltip(st: &Status) -> String {
    let model = match st.model.phase {
        ModelPhase::Ready => format!("{} ready", st.model.name),
        ModelPhase::Loading => format!("Loading {}…", st.model.name),
        ModelPhase::Installing => format!("Installing {}…", st.model.name),
        ModelPhase::NotInstalled => "Model not installed".into(),
        ModelPhase::Error => "Model error".into(),
        ModelPhase::Unloaded => format!("{} (sleeping)", st.model.name),
    };
    let mode = match st.phase {
        DictationPhase::Listening => "Listening · ",
        DictationPhase::HandsFree => "Hands-free · ",
        DictationPhase::Finishing => "Typing · ",
        DictationPhase::Idle => "",
    };
    format!("Fluss — {mode}{model}\nHold {} to dictate", st.hotkeys.push_to_talk)
}

#[cfg(windows)]
pub fn run_main_loop(rx: Receiver<Status>, ev: Sender<Ev>, quit: std::sync::Arc<std::sync::atomic::AtomicBool>) {
    use std::sync::atomic::Ordering;
    use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
    use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
    use windows::Win32::UI::WindowsAndMessaging::*;

    let menu = Menu::new();
    let header = MenuItem::new("Fluss", false, None);
    let open = MenuItem::new("Open Fluss", true, None);
    let dict = MenuItem::new("Dictionary", true, None);
    let add = MenuItem::new("Add a word…", true, None);
    let hands = MenuItem::new("Start hands-free dictation", true, None);
    let load = MenuItem::new("Load model now", true, None);
    let unload = MenuItem::new("Unload model (free memory)", true, None);
    let pause = CheckMenuItem::new("Pause shortcuts", true, false, None);
    let quit_item = MenuItem::new("Quit Fluss", true, None);
    let _ = menu.append_items(&[
        &header,
        &PredefinedMenuItem::separator(),
        &open,
        &dict,
        &add,
        &PredefinedMenuItem::separator(),
        &hands,
        &load,
        &unload,
        &pause,
        &PredefinedMenuItem::separator(),
        &quit_item,
    ]);
    let mut current = Look::Off;
    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_menu_on_left_click(false)
        .with_tooltip("Fluss")
        .with_icon(Icon::from_rgba(icon_rgba(current), 32, 32).unwrap())
        .build();
    let tray = match tray {
        Ok(t) => Some(t),
        Err(e) => {
            log::error!("tray icon: {e}");
            None
        }
    };

    let send = |c: TrayCmd| {
        let _ = ev.send(Ev::Tray(c));
    };
    while !quit.load(Ordering::Relaxed) {
        unsafe {
            MsgWaitForMultipleObjects(None, false, 60, QS_ALLINPUT);
            let mut msg = MSG::default();
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        while let Ok(e) = MenuEvent::receiver().try_recv() {
            let id = e.id;
            if id == *open.id() {
                send(TrayCmd::Open);
            } else if id == *dict.id() {
                send(TrayCmd::Dictionary);
            } else if id == *add.id() {
                send(TrayCmd::AddWord);
            } else if id == *hands.id() {
                send(TrayCmd::ToggleHandsFree);
            } else if id == *load.id() {
                send(TrayCmd::LoadModel);
            } else if id == *unload.id() {
                send(TrayCmd::UnloadModel);
            } else if id == *pause.id() {
                send(TrayCmd::PauseHotkeys(pause.is_checked()));
            } else if id == *quit_item.id() {
                send(TrayCmd::Quit);
            }
        }
        while let Ok(e) = TrayIconEvent::receiver().try_recv() {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e {
                send(TrayCmd::Open);
            }
        }
        let mut latest = None;
        while let Ok(st) = rx.try_recv() {
            latest = Some(st);
        }
        if let (Some(st), Some(t)) = (latest, tray.as_ref()) {
            let l = look(&st);
            if l != current {
                current = l;
                let _ = t.set_icon(Icon::from_rgba(icon_rgba(l), 32, 32).ok());
            }
            let _ = t.set_tooltip(Some(tooltip(&st)));
            header.set_text(match st.model.phase {
                ModelPhase::Ready => format!("Fluss · {} ready", st.model.name),
                _ => format!("Fluss · {}", tooltip(&st).lines().next().unwrap_or("").trim_start_matches("Fluss — ")),
            });
            hands.set_text(if matches!(st.phase, DictationPhase::HandsFree) { "Stop hands-free dictation" } else { "Start hands-free dictation" });
            pause.set_checked(st.hotkeys.paused);
            load.set_enabled(st.model.phase == ModelPhase::Unloaded || st.model.phase == ModelPhase::Error);
            unload.set_enabled(st.model.phase == ModelPhase::Ready);
        }
    }
}

#[cfg(not(windows))]
pub fn run_main_loop(rx: Receiver<Status>, _ev: Sender<Ev>, quit: std::sync::Arc<std::sync::atomic::AtomicBool>) {
    let _ = (icon_rgba, tooltip, look);
    while !quit.load(std::sync::atomic::Ordering::Relaxed) {
        let _ = rx.recv_timeout(std::time::Duration::from_millis(200));
    }
}
