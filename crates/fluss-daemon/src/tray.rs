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

/// 32x32 RGBA: a flowing wave mark with a status dot.
fn icon_rgba(l: Look) -> Vec<u8> {
    use tiny_skia::*;
    let mut pm = Pixmap::new(32, 32).unwrap();
    let mut pb = PathBuilder::new();
    pb.move_to(4.0, 18.0);
    pb.cubic_to(8.0, 10.0, 12.0, 10.0, 16.0, 16.0);
    pb.cubic_to(20.0, 22.0, 24.0, 22.0, 28.0, 14.0);
    let wave = pb.finish().unwrap();
    let mut pb = PathBuilder::new();
    pb.move_to(4.0, 25.0);
    pb.cubic_to(8.0, 18.0, 12.0, 18.0, 16.0, 23.0);
    pb.cubic_to(20.0, 28.0, 24.0, 28.0, 28.0, 21.0);
    let wave2 = pb.finish().unwrap();
    let mut p = Paint::default();
    p.anti_alias = true;
    p.shader = LinearGradient::new(
        Point::from_xy(4.0, 0.0),
        Point::from_xy(28.0, 0.0),
        vec![GradientStop::new(0.0, Color::from_rgba8(45, 212, 191, 255)), GradientStop::new(1.0, Color::from_rgba8(167, 139, 250, 255))],
        SpreadMode::Pad,
        Transform::identity(),
    )
    .unwrap();
    if l == Look::Off {
        p.set_color_rgba8(150, 156, 168, 255);
    }
    let st = Stroke { width: 3.4, line_cap: LineCap::Round, ..Default::default() };
    pm.stroke_path(&wave, &p, &st, Transform::identity(), None);
    let st2 = Stroke { width: 2.4, line_cap: LineCap::Round, ..Default::default() };
    let mut p2 = p.clone();
    if l != Look::Off {
        p2.shader = LinearGradient::new(
            Point::from_xy(4.0, 0.0),
            Point::from_xy(28.0, 0.0),
            vec![GradientStop::new(0.0, Color::from_rgba8(45, 212, 191, 150)), GradientStop::new(1.0, Color::from_rgba8(167, 139, 250, 150))],
            SpreadMode::Pad,
            Transform::identity(),
        )
        .unwrap();
    }
    pm.stroke_path(&wave2, &p2, &st2, Transform::identity(), None);
    let dot = match l {
        Look::Live => Some((255, 99, 125)),
        Look::Busy => Some((56, 189, 248)),
        Look::Problem => Some((251, 191, 36)),
        _ => None,
    };
    if let Some((r, g, b)) = dot {
        let mut d = Paint::default();
        d.anti_alias = true;
        d.set_color_rgba8(20, 22, 28, 255);
        pm.fill_path(&PathBuilder::from_circle(25.0, 7.0, 6.5).unwrap(), &d, FillRule::Winding, Transform::identity(), None);
        d.set_color_rgba8(r, g, b, 255);
        pm.fill_path(&PathBuilder::from_circle(25.0, 7.0, 4.8).unwrap(), &d, FillRule::Winding, Transform::identity(), None);
    }
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
