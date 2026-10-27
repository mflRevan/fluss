//! Global shortcuts via low-level keyboard/mouse hooks on a dedicated thread.
//!
//! The hook callback does nothing but set lookups and a channel send, so it
//! never adds input latency. Modifier-only push-to-talk (Ctrl+Win) passes the
//! modifiers through to the system (so Ctrl+Win+Arrow etc. keep working) and
//! injects a neutral key so releasing Win never opens the Start menu. Keys
//! that belong to Fluss (Space/R/Esc while dictating, F-keys, mouse buttons)
//! are swallowed.
//!
//! It also tracks a "user input epoch": any key or click that isn't ours
//! bumps it, which tells continuation that the text field may have changed.

use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};

use crossbeam_channel::Sender;
use fluss_core::config::Hotkeys as HotkeyCfg;
use fluss_core::hotkey::{Hotkey, Key, Mods};
use parking_lot::RwLock;

use crate::app::Ev;

#[derive(Debug, Clone, PartialEq)]
pub enum HotkeyEv {
    PttDown,
    PttUp,
    HandsFree,
    AddRule,
    Cancel,
    /// Another key was pressed while holding push-to-talk (a system shortcut).
    OtherKey,
    Recorded(Option<Hotkey>),
}

/// A dictation is running (enables the cancel key).
pub static ACTIVE: AtomicBool = AtomicBool::new(false);
pub static PAUSED: AtomicBool = AtomicBool::new(false);
pub static RECORDING: AtomicBool = AtomicBool::new(false);
/// While the rule popup is open, typing in it must not count as user input.
pub static EPOCH_FROZEN: AtomicBool = AtomicBool::new(false);
pub static USER_EPOCH: AtomicU64 = AtomicU64::new(0);
/// Physically held modifiers: 1 ctrl, 2 shift, 4 alt, 8 meta.
pub static MODS_DOWN: AtomicU8 = AtomicU8::new(0);

static BINDINGS: RwLock<Option<HotkeyCfg>> = RwLock::new(None);

pub fn set_bindings(cfg: &HotkeyCfg) {
    *BINDINGS.write() = Some(cfg.clone());
}

pub fn epoch() -> u64 {
    USER_EPOCH.load(Ordering::SeqCst)
}

fn bump_epoch() {
    if !EPOCH_FROZEN.load(Ordering::Relaxed) {
        USER_EPOCH.fetch_add(1, Ordering::SeqCst);
    }
}

/// Modifiers that make typed text turn into shortcuts.
pub fn blocking_mods_down() -> bool {
    MODS_DOWN.load(Ordering::SeqCst) & (1 | 4 | 8) != 0
}

// ------------------------------------------------------------------ pure state machine

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Input {
    Ctrl,
    Shift,
    Alt,
    Meta,
    Key(Key),
    /// A key Fluss has no name for (still counts for "other key" / input epoch).
    Other(u32),
}

impl Input {
    fn is_mod(self) -> bool {
        matches!(self, Input::Ctrl | Input::Shift | Input::Alt | Input::Meta)
    }
}

fn mods_of(down: &[Input]) -> Mods {
    let mut m = Mods::default();
    for i in down {
        match i {
            Input::Ctrl => m.ctrl = true,
            Input::Shift => m.shift = true,
            Input::Alt => m.alt = true,
            Input::Meta => m.meta = true,
            _ => {}
        }
    }
    m
}

#[derive(Debug, Default)]
pub struct Machine {
    down: Vec<Input>,
    swallowed: Vec<Input>,
    ptt_active: bool,
    ptt_key: Option<Input>,
    rec_mods: Mods,
    rec_key: Option<Key>,
    rec_any: bool,
}

#[derive(Debug, Default, PartialEq)]
pub struct Outcome {
    pub swallow: bool,
    pub events: Vec<HotkeyEv>,
    /// Inject the neutral mask key (prevents Start menu / menu bar on release).
    pub mask: bool,
    pub user_input: bool,
}

impl Machine {
    /// `phys` is the raw input (left/right modifiers already folded, except when
    /// a side-specific key like RightAlt is bound — callers pass it as `Key`).
    pub fn on(&mut self, b: &HotkeyCfg, input: Input, is_down: bool, active: bool, recording: bool) -> Outcome {
        let mut o = Outcome::default();
        if recording {
            return self.record(input, is_down);
        }
        if is_down {
            let repeat = self.down.contains(&input);
            if !repeat {
                self.down.push(input);
            }
            if self.swallowed.contains(&input) {
                o.swallow = true;
                return o;
            }
            let others: Vec<Input> = self.down.iter().copied().filter(|i| *i != input).collect();
            let mods_before = mods_of(&others);
            let mods_now = mods_of(&self.down);
            let key = match input {
                Input::Key(k) => Some(k),
                _ => None,
            };

            // Push-to-talk on a real key (F9, Mouse4, Ctrl+Shift+Space, RightAlt...).
            if let Some(pk) = b.push_to_talk.key {
                if key == Some(pk) && mods_before == b.push_to_talk.mods {
                    if !repeat && !self.ptt_active {
                        self.ptt_active = true;
                        self.ptt_key = Some(input);
                        o.events.push(HotkeyEv::PttDown);
                    }
                    self.swallowed.push(input);
                    o.swallow = true;
                    return o;
                }
            }
            if input.is_mod() {
                if b.push_to_talk.key.is_none() && !self.ptt_active && !repeat && mods_now == b.push_to_talk.mods {
                    self.ptt_active = true;
                    self.ptt_key = None;
                    o.events.push(HotkeyEv::PttDown);
                    o.mask = mods_now.meta || mods_now.alt;
                } else if self.ptt_active && !repeat {
                    o.events.push(HotkeyEv::OtherKey);
                }
                return o;
            }
            // A non-modifier key.
            if self.ptt_active {
                let hit = |h: &Hotkey| h.key.is_some() && h.key == key;
                let ev = if hit(&b.hands_free) {
                    Some(HotkeyEv::HandsFree)
                } else if hit(&b.add_rule) {
                    Some(HotkeyEv::AddRule)
                } else if hit(&b.cancel) {
                    Some(HotkeyEv::Cancel)
                } else {
                    None
                };
                match ev {
                    Some(e) => {
                        if !repeat {
                            o.events.push(e);
                        }
                        self.swallowed.push(input);
                        o.swallow = true;
                    }
                    None => {
                        if !repeat {
                            o.events.push(HotkeyEv::OtherKey);
                        }
                        o.user_input = true;
                    }
                }
                return o;
            }
            let exact = |h: &Hotkey| h.key.is_some() && h.key == key && mods_before == h.mods;
            let ev = if exact(&b.hands_free) {
                Some(HotkeyEv::HandsFree)
            } else if exact(&b.add_rule) {
                Some(HotkeyEv::AddRule)
            } else if active && exact(&b.cancel) {
                Some(HotkeyEv::Cancel)
            } else {
                None
            };
            match ev {
                Some(e) => {
                    if !repeat {
                        o.events.push(e);
                    }
                    self.swallowed.push(input);
                    o.swallow = true;
                    o.mask = mods_before.meta || mods_before.alt;
                }
                None => o.user_input = true,
            }
            o
        } else {
            self.down.retain(|i| *i != input);
            let was_swallowed = self.swallowed.contains(&input);
            self.swallowed.retain(|i| *i != input);
            if self.ptt_active {
                if let Some(k) = self.ptt_key {
                    if k == input {
                        self.ptt_active = false;
                        self.ptt_key = None;
                        o.events.push(HotkeyEv::PttUp);
                    }
                } else if input.is_mod() {
                    let m = b.push_to_talk.mods;
                    let part = match input {
                        Input::Ctrl => m.ctrl,
                        Input::Shift => m.shift,
                        Input::Alt => m.alt,
                        Input::Meta => m.meta,
                        _ => false,
                    };
                    if part {
                        self.ptt_active = false;
                        o.events.push(HotkeyEv::PttUp);
                    }
                }
            }
            o.swallow = was_swallowed;
            o
        }
    }

    fn record(&mut self, input: Input, is_down: bool) -> Outcome {
        let mut o = Outcome { swallow: true, ..Default::default() };
        if is_down {
            if !self.down.contains(&input) {
                self.down.push(input);
            }
            self.rec_any = true;
            let m = mods_of(&self.down);
            // Remember the largest combination seen.
            if m.count() >= self.rec_mods.count() {
                self.rec_mods = m;
            }
            if let Input::Key(k) = input {
                self.rec_key = Some(k);
            }
            o.mask = true;
        } else {
            self.down.retain(|i| *i != input);
            if self.down.is_empty() && self.rec_any {
                let hk = Hotkey { mods: self.rec_mods, key: self.rec_key };
                let valid = hk.key.is_some() || hk.mods.count() >= 2;
                o.events.push(HotkeyEv::Recorded(valid.then_some(hk)));
                self.rec_mods = Mods::default();
                self.rec_key = None;
                self.rec_any = false;
            }
        }
        o
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

// ------------------------------------------------------------------ Windows hook

#[cfg(windows)]
pub use win::spawn;

#[cfg(windows)]
mod win {
    use super::*;
    use std::cell::RefCell;
    use std::sync::OnceLock;

    use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::Input::KeyboardAndMouse::*;
    use windows::Win32::UI::WindowsAndMessaging::*;

    static TX: OnceLock<Sender<Ev>> = OnceLock::new();

    thread_local! {
        static MACHINE: RefCell<Machine> = RefCell::new(Machine::default());
        static WAS_RECORDING: RefCell<bool> = const { RefCell::new(false) };
    }

    pub fn spawn(tx: Sender<Ev>) {
        let _ = TX.set(tx);
        std::thread::Builder::new()
            .name("fluss-hotkeys".into())
            .spawn(|| unsafe {
                let _ = windows::Win32::System::Threading::SetThreadPriority(
                    windows::Win32::System::Threading::GetCurrentThread(),
                    windows::Win32::System::Threading::THREAD_PRIORITY_TIME_CRITICAL,
                );
                let hinst = GetModuleHandleW(None).ok();
                let kb = SetWindowsHookExW(WH_KEYBOARD_LL, Some(kb_proc), hinst.map(|h| h.into()), 0);
                let ms = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), hinst.map(|h| h.into()), 0);
                if kb.is_err() || ms.is_err() {
                    log::error!("installing input hooks failed: {:?} {:?}", kb.err(), ms.err());
                }
                let mut msg = MSG::default();
                while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            })
            .expect("spawn hotkey thread");
    }

    fn vk_input(vk: u32, bindings: &HotkeyCfg) -> Input {
        // Side-specific keys bound on their own keep their identity.
        let side = |k: Key| [bindings.push_to_talk.key, bindings.hands_free.key, bindings.add_rule.key].contains(&Some(k));
        match vk {
            0xA5 if side(Key::RightAlt) => Input::Key(Key::RightAlt),
            0xA3 if side(Key::RightCtrl) => Input::Key(Key::RightCtrl),
            0xA1 if side(Key::RightShift) => Input::Key(Key::RightShift),
            0xA2 | 0xA3 | 0x11 => Input::Ctrl,
            0xA0 | 0xA1 | 0x10 => Input::Shift,
            0xA4 | 0xA5 | 0x12 => Input::Alt,
            0x5B | 0x5C => Input::Meta,
            0x41..=0x5A | 0x30..=0x39 => Input::Key(Key::Char(char::from_u32(vk).unwrap())),
            0x70..=0x87 => Input::Key(Key::F((vk - 0x70 + 1) as u8)),
            0x20 => Input::Key(Key::Space),
            0x0D => Input::Key(Key::Enter),
            0x09 => Input::Key(Key::Tab),
            0x1B => Input::Key(Key::Escape),
            0x08 => Input::Key(Key::Backspace),
            0x2E => Input::Key(Key::Delete),
            0x2D => Input::Key(Key::Insert),
            0x24 => Input::Key(Key::Home),
            0x23 => Input::Key(Key::End),
            0x21 => Input::Key(Key::PageUp),
            0x22 => Input::Key(Key::PageDown),
            0x25 => Input::Key(Key::Left),
            0x26 => Input::Key(Key::Up),
            0x27 => Input::Key(Key::Right),
            0x28 => Input::Key(Key::Down),
            0x14 => Input::Key(Key::CapsLock),
            0x91 => Input::Key(Key::ScrollLock),
            0x13 => Input::Key(Key::Pause),
            0x2C => Input::Key(Key::PrintScreen),
            0xC0 => Input::Key(Key::Grave),
            0xBD => Input::Key(Key::Minus),
            0xBB => Input::Key(Key::Equal),
            0xDB => Input::Key(Key::BracketLeft),
            0xDD => Input::Key(Key::BracketRight),
            0xDC => Input::Key(Key::Backslash),
            0xBA => Input::Key(Key::Semicolon),
            0xDE => Input::Key(Key::Quote),
            0xBC => Input::Key(Key::Comma),
            0xBE => Input::Key(Key::Period),
            0xBF => Input::Key(Key::Slash),
            other => Input::Other(other),
        }
    }

    fn update_mods(input: Input, down: bool) {
        let bit = match input {
            Input::Ctrl => 1,
            Input::Shift => 2,
            Input::Alt => 4,
            Input::Meta => 8,
            _ => return,
        };
        if down {
            MODS_DOWN.fetch_or(bit, Ordering::SeqCst);
        } else {
            MODS_DOWN.fetch_and(!bit, Ordering::SeqCst);
        }
    }

    pub fn send_mask() {
        let ev = [
            INPUT { r#type: INPUT_KEYBOARD, Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: VIRTUAL_KEY(0xE8), ..Default::default() } } },
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: VIRTUAL_KEY(0xE8), dwFlags: KEYEVENTF_KEYUP, ..Default::default() } },
            },
        ];
        unsafe {
            SendInput(&ev, std::mem::size_of::<INPUT>() as i32);
        }
    }

    fn handle(input: Input, down: bool) -> bool {
        update_mods(input, down);
        let recording = RECORDING.load(Ordering::SeqCst);
        // Leaving record mode: forget its key state.
        WAS_RECORDING.with(|w| {
            let mut w = w.borrow_mut();
            if *w && !recording {
                MACHINE.with(|m| m.borrow_mut().reset());
            }
            *w = recording;
        });
        if PAUSED.load(Ordering::Relaxed) && !recording {
            if down && !input.is_mod() {
                bump_epoch();
            }
            return false;
        }
        let guard = BINDINGS.read();
        let Some(b) = guard.as_ref() else { return false };
        let out = MACHINE.with(|m| m.borrow_mut().on(b, input, down, ACTIVE.load(Ordering::Relaxed), recording));
        drop(guard);
        if out.user_input {
            bump_epoch();
        }
        if out.mask {
            send_mask();
        }
        if let Some(tx) = TX.get() {
            for e in out.events {
                let _ = tx.send(Ev::Hotkey(e));
            }
        }
        out.swallow
    }

    unsafe extern "system" fn kb_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code == HC_ACTION as i32 {
            let k = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
            let injected = k.flags.0 & LLKHF_INJECTED.0 != 0;
            if !injected && k.vkCode != 0xE8 {
                let msg = wparam.0 as u32;
                let down = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;
                let input = {
                    let g = BINDINGS.read();
                    match g.as_ref() {
                        Some(b) => vk_input(k.vkCode, b),
                        None => Input::Other(k.vkCode),
                    }
                };
                if handle(input, down) {
                    return LRESULT(1);
                }
            }
        }
        CallNextHookEx(None, code, wparam, lparam)
    }

    unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code == HC_ACTION as i32 {
            let m = &*(lparam.0 as *const MSLLHOOKSTRUCT);
            let injected = m.flags & LLMHF_INJECTED != 0;
            if !injected {
                let msg = wparam.0 as u32;
                let xbutton = (m.mouseData >> 16) & 0xffff;
                let (input, down) = match msg {
                    WM_XBUTTONDOWN | WM_XBUTTONUP => {
                        (Some(if xbutton == 1 { Key::Mouse4 } else { Key::Mouse5 }), msg == WM_XBUTTONDOWN)
                    }
                    WM_MBUTTONDOWN | WM_MBUTTONUP => (Some(Key::MouseMiddle), msg == WM_MBUTTONDOWN),
                    WM_LBUTTONDOWN | WM_RBUTTONDOWN => {
                        bump_epoch();
                        (None, true)
                    }
                    _ => (None, false),
                };
                if let Some(k) = input {
                    let bound = {
                        let g = BINDINGS.read();
                        g.as_ref().is_some_and(|b| [b.push_to_talk.key, b.hands_free.key, b.add_rule.key].contains(&Some(k)))
                    };
                    if bound || RECORDING.load(Ordering::SeqCst) {
                        if handle(Input::Key(k), down) {
                            return LRESULT(1);
                        }
                    } else if down {
                        bump_epoch();
                    }
                }
            }
        }
        CallNextHookEx(None, code, wparam, lparam)
    }
}

#[cfg(not(windows))]
pub fn spawn(_tx: Sender<Ev>) {
    log::warn!("global shortcuts are not implemented on this platform yet");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> HotkeyCfg {
        HotkeyCfg::default()
    }

    fn press(m: &mut Machine, b: &HotkeyCfg, i: Input, down: bool) -> Outcome {
        m.on(b, i, down, false, false)
    }

    #[test]
    fn ctrl_win_hold_and_release() {
        let b = cfg();
        let mut m = Machine::default();
        assert_eq!(press(&mut m, &b, Input::Ctrl, true).events, vec![]);
        let o = press(&mut m, &b, Input::Meta, true);
        assert_eq!(o.events, vec![HotkeyEv::PttDown]);
        assert!(o.mask && !o.swallow);
        // autorepeat does nothing
        assert_eq!(press(&mut m, &b, Input::Meta, true).events, vec![]);
        let o = press(&mut m, &b, Input::Meta, false);
        assert_eq!(o.events, vec![HotkeyEv::PttUp]);
        assert_eq!(press(&mut m, &b, Input::Ctrl, false).events, vec![]);
    }

    #[test]
    fn space_locks_and_is_swallowed() {
        let b = cfg();
        let mut m = Machine::default();
        press(&mut m, &b, Input::Meta, true);
        press(&mut m, &b, Input::Ctrl, true);
        let o = press(&mut m, &b, Input::Key(Key::Space), true);
        assert_eq!(o.events, vec![HotkeyEv::HandsFree]);
        assert!(o.swallow);
        assert!(press(&mut m, &b, Input::Key(Key::Space), false).swallow);
        let o = press(&mut m, &b, Input::Key(Key::Char('R')), true);
        assert_eq!(o.events, vec![HotkeyEv::AddRule]);
    }

    #[test]
    fn system_shortcut_reports_other_key() {
        let b = cfg();
        let mut m = Machine::default();
        press(&mut m, &b, Input::Ctrl, true);
        press(&mut m, &b, Input::Meta, true);
        let o = press(&mut m, &b, Input::Key(Key::Left), true);
        assert_eq!(o.events, vec![HotkeyEv::OtherKey]);
        assert!(!o.swallow);
    }

    #[test]
    fn plain_typing_bumps_input_and_escape_only_when_active() {
        let b = cfg();
        let mut m = Machine::default();
        let o = press(&mut m, &b, Input::Key(Key::Char('A')), true);
        assert!(o.user_input && !o.swallow && o.events.is_empty());
        press(&mut m, &b, Input::Key(Key::Char('A')), false);
        let o = m.on(&b, Input::Key(Key::Escape), true, false, false);
        assert!(!o.swallow);
        m.on(&b, Input::Key(Key::Escape), false, false, false);
        let o = m.on(&b, Input::Key(Key::Escape), true, true, false);
        assert!(o.swallow);
        assert_eq!(o.events, vec![HotkeyEv::Cancel]);
    }

    #[test]
    fn key_based_ptt() {
        let mut b = cfg();
        b.push_to_talk = Hotkey::parse("F9").unwrap();
        let mut m = Machine::default();
        let o = press(&mut m, &b, Input::Key(Key::F(9)), true);
        assert_eq!(o.events, vec![HotkeyEv::PttDown]);
        assert!(o.swallow);
        let o = press(&mut m, &b, Input::Key(Key::F(9)), false);
        assert_eq!(o.events, vec![HotkeyEv::PttUp]);
        assert!(o.swallow);
    }

    #[test]
    fn standalone_add_rule_combo() {
        let b = cfg();
        let mut m = Machine::default();
        // Ctrl+Win+R pressed quickly also goes through PTT first; that's fine:
        // the controller treats AddRule during a fresh hold as "open popup".
        press(&mut m, &b, Input::Ctrl, true);
        press(&mut m, &b, Input::Meta, true);
        let o = press(&mut m, &b, Input::Key(Key::Char('R')), true);
        assert_eq!(o.events, vec![HotkeyEv::AddRule]);
    }

    #[test]
    fn recording_captures_combo() {
        let b = cfg();
        let mut m = Machine::default();
        let rec = |m: &mut Machine, i, d| m.on(&b, i, d, false, true);
        assert!(rec(&mut m, Input::Ctrl, true).swallow);
        rec(&mut m, Input::Shift, true);
        rec(&mut m, Input::Key(Key::Char('D')), true);
        rec(&mut m, Input::Key(Key::Char('D')), false);
        rec(&mut m, Input::Shift, false);
        let o = rec(&mut m, Input::Ctrl, false);
        assert_eq!(o.events, vec![HotkeyEv::Recorded(Some(Hotkey::parse("Ctrl+Shift+D").unwrap()))]);
        // a lone modifier is not valid
        rec(&mut m, Input::Ctrl, true);
        let o = rec(&mut m, Input::Ctrl, false);
        assert_eq!(o.events, vec![HotkeyEv::Recorded(None)]);
    }
}
