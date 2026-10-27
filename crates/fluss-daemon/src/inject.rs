//! Text insertion on its own thread. Typing waits until Ctrl/Alt/Win are
//! physically released (otherwise characters become shortcuts), then sends the
//! whole edit as one batched SendInput call, which is effectively instant.

use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender};
use fluss_core::config::{InsertMethod, NewlineKey};

use crate::app::Ev;

#[derive(Debug, Clone)]
pub struct Edit {
    pub backspaces: usize,
    pub text: String,
    pub method: InsertMethod,
    pub newline: NewlineKey,
    /// Correlates the completion event.
    pub tag: u64,
}

pub enum InjectEv {
    /// The edit has been delivered to the focused app.
    Done { tag: u64, at: Instant },
}

pub fn spawn(ev: Sender<Ev>) -> Sender<Edit> {
    let (tx, rx) = crossbeam_channel::unbounded::<Edit>();
    std::thread::Builder::new()
        .name("fluss-inject".into())
        .spawn(move || run(rx, ev))
        .expect("spawn inject thread");
    tx
}

fn run(rx: Receiver<Edit>, ev: Sender<Ev>) {
    #[cfg(windows)]
    crate::platform::raise_thread_priority();
    while let Ok(edit) = rx.recv() {
        let waited = Instant::now();
        while crate::hotkeys::blocking_mods_down() && waited.elapsed() < Duration::from_secs(20) {
            std::thread::sleep(Duration::from_millis(4));
        }
        apply(&edit);
        let _ = ev.send(Ev::Injected(InjectEv::Done { tag: edit.tag, at: Instant::now() }));
    }
}

fn apply(e: &Edit) {
    if e.backspaces == 0 && e.text.is_empty() {
        return;
    }
    // Test mode: a file stands in for the focused text field (never touches the desktop).
    if let Some(sink) = std::env::var_os("FLUSS_TEST_SINK") {
        let mut s = std::fs::read_to_string(&sink).unwrap_or_default();
        for _ in 0..e.backspaces {
            s.pop();
        }
        s.push_str(&e.text);
        let _ = std::fs::write(&sink, s);
        return;
    }
    match e.method {
        InsertMethod::Type => imp::type_edit(e.backspaces, &e.text, e.newline),
        InsertMethod::Paste => {
            imp::type_edit(e.backspaces, "", e.newline);
            if !e.text.is_empty() {
                imp::paste(&e.text);
            }
        }
    }
}

#[cfg(windows)]
mod imp {
    use super::*;
    use windows::Win32::Foundation::{HANDLE, HGLOBAL};
    use windows::Win32::System::DataExchange::*;
    use windows::Win32::System::Memory::*;
    use windows::Win32::UI::Input::KeyboardAndMouse::*;

    const CF_UNICODETEXT: u32 = 13;

    fn key(vk: VIRTUAL_KEY, up: bool) -> INPUT {
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT { wVk: vk, dwFlags: if up { KEYEVENTF_KEYUP } else { KEYBD_EVENT_FLAGS(0) }, ..Default::default() },
            },
        }
    }

    fn unit(u: u16, up: bool) -> INPUT {
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wScan: u,
                    dwFlags: if up { KEYEVENTF_UNICODE | KEYEVENTF_KEYUP } else { KEYEVENTF_UNICODE },
                    ..Default::default()
                },
            },
        }
    }

    fn send(inputs: &[INPUT]) {
        // Big batches can be dropped by some apps; 200 events per call is safe and still instant.
        for chunk in inputs.chunks(200) {
            unsafe {
                SendInput(chunk, std::mem::size_of::<INPUT>() as i32);
            }
        }
    }

    pub fn type_edit(backspaces: usize, text: &str, newline: NewlineKey) {
        let mut v = Vec::with_capacity(backspaces * 2 + text.len() * 2);
        for _ in 0..backspaces {
            v.push(key(VK_BACK, false));
            v.push(key(VK_BACK, true));
        }
        for ch in text.chars() {
            if ch == '\n' {
                let shift = newline == NewlineKey::ShiftEnter;
                if shift {
                    v.push(key(VK_SHIFT, false));
                }
                v.push(key(VK_RETURN, false));
                v.push(key(VK_RETURN, true));
                if shift {
                    v.push(key(VK_SHIFT, true));
                }
                continue;
            }
            let mut buf = [0u16; 2];
            for u in ch.encode_utf16(&mut buf) {
                v.push(unit(*u, false));
                v.push(unit(*u, true));
            }
        }
        send(&v);
    }

    fn get_clipboard() -> Option<Vec<u16>> {
        unsafe {
            if OpenClipboard(None).is_err() {
                return None;
            }
            let out = (|| {
                let h = GetClipboardData(CF_UNICODETEXT).ok()?;
                let p = GlobalLock(HGLOBAL(h.0)) as *const u16;
                if p.is_null() {
                    return None;
                }
                let mut n = 0;
                while *p.add(n) != 0 {
                    n += 1;
                }
                let v = std::slice::from_raw_parts(p, n + 1).to_vec();
                let _ = GlobalUnlock(HGLOBAL(h.0));
                Some(v)
            })();
            let _ = CloseClipboard();
            out
        }
    }

    fn set_clipboard(wide: &[u16]) -> bool {
        unsafe {
            if OpenClipboard(None).is_err() {
                return false;
            }
            let _ = EmptyClipboard();
            let ok = (|| {
                let bytes = wide.len() * 2;
                let h = GlobalAlloc(GMEM_MOVEABLE, bytes).ok()?;
                let p = GlobalLock(h) as *mut u16;
                if p.is_null() {
                    return None;
                }
                std::ptr::copy_nonoverlapping(wide.as_ptr(), p, wide.len());
                let _ = GlobalUnlock(h);
                SetClipboardData(CF_UNICODETEXT, Some(HANDLE(h.0))).ok()?;
                Some(())
            })()
            .is_some();
            let _ = CloseClipboard();
            ok
        }
    }

    pub fn paste(text: &str) {
        let saved = get_clipboard();
        let wide: Vec<u16> = text.replace('\n', "\r\n").encode_utf16().chain(std::iter::once(0)).collect();
        if !set_clipboard(&wide) {
            type_edit(0, text, NewlineKey::Enter);
            return;
        }
        send(&[key(VK_CONTROL, false), key(VK_V, false), key(VK_V, true), key(VK_CONTROL, true)]);
        // Give the app time to read the clipboard before restoring it.
        std::thread::sleep(Duration::from_millis(150));
        if let Some(s) = saved {
            set_clipboard(&s);
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;
    pub fn type_edit(_backspaces: usize, _text: &str, _newline: NewlineKey) {
        log::warn!("text insertion is not implemented on this platform yet");
    }
    pub fn paste(text: &str) {
        type_edit(0, text, NewlineKey::Enter);
    }
}
