//! Shortcut descriptions: a set of modifiers plus an optional main key.
//! `Ctrl+Win` (modifier-only, hold-friendly), `Ctrl+Win+Space`, `F9`, `RightAlt`, `Mouse4`.
//! Serialized as the human string, so `config.toml` stays readable.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Mods {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    /// Win on Windows, Cmd on macOS, Super on Linux.
    pub meta: bool,
}

impl Mods {
    pub fn is_empty(&self) -> bool {
        !(self.ctrl || self.shift || self.alt || self.meta)
    }
    pub fn count(&self) -> usize {
        [self.ctrl, self.shift, self.alt, self.meta].iter().filter(|b| **b).count()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    Char(char), // A-Z, 0-9 (uppercase)
    F(u8),      // F1..F24
    Space,
    Enter,
    Tab,
    Escape,
    Backspace,
    Delete,
    Insert,
    Home,
    End,
    PageUp,
    PageDown,
    Left,
    Right,
    Up,
    Down,
    CapsLock,
    ScrollLock,
    Pause,
    PrintScreen,
    Grave,
    Minus,
    Equal,
    BracketLeft,
    BracketRight,
    Backslash,
    Semicolon,
    Quote,
    Comma,
    Period,
    Slash,
    /// A single side modifier used on its own (`RightAlt`, `RightCtrl`).
    RightAlt,
    RightCtrl,
    RightShift,
    Mouse4,
    Mouse5,
    MouseMiddle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Hotkey {
    pub mods: Mods,
    pub key: Option<Key>,
}

impl Hotkey {
    pub fn parse(s: &str) -> Result<Self, String> {
        let mut mods = Mods::default();
        let mut key = None;
        let parts: Vec<&str> = s.split('+').map(str::trim).filter(|p| !p.is_empty()).collect();
        if parts.is_empty() {
            return Err("empty shortcut".into());
        }
        for p in parts {
            let l = p.to_ascii_lowercase();
            match l.as_str() {
                "ctrl" | "control" | "ctl" | "strg" => mods.ctrl = true,
                "shift" => mods.shift = true,
                "alt" | "option" | "opt" => mods.alt = true,
                "win" | "windows" | "cmd" | "command" | "super" | "meta" | "mod" => mods.meta = true,
                _ => {
                    if key.is_some() {
                        return Err(format!("`{s}` has more than one non-modifier key"));
                    }
                    key = Some(parse_key(&l).ok_or_else(|| format!("unknown key `{p}`"))?);
                }
            }
        }
        if key.is_none() && mods.count() < 2 {
            return Err(format!("`{s}`: a modifier-only shortcut needs at least two modifiers (e.g. Ctrl+Win)"));
        }
        Ok(Self { mods, key })
    }

    pub fn is_modifier_only(&self) -> bool {
        self.key.is_none()
    }
}

fn parse_key(l: &str) -> Option<Key> {
    use Key::*;
    if l.len() == 1 {
        let c = l.chars().next()?.to_ascii_uppercase();
        if c.is_ascii_alphanumeric() {
            return Some(Char(c));
        }
        return match c {
            '`' => Some(Grave),
            '-' => Some(Minus),
            '=' => Some(Equal),
            '[' => Some(BracketLeft),
            ']' => Some(BracketRight),
            '\\' => Some(Backslash),
            ';' => Some(Semicolon),
            '\'' => Some(Quote),
            ',' => Some(Comma),
            '.' => Some(Period),
            '/' => Some(Slash),
            _ => None,
        };
    }
    if let Some(n) = l.strip_prefix('f').and_then(|n| n.parse::<u8>().ok()) {
        return (1..=24).contains(&n).then_some(F(n));
    }
    Some(match l {
        "space" | "spacebar" => Space,
        "enter" | "return" => Enter,
        "tab" => Tab,
        "esc" | "escape" => Escape,
        "backspace" => Backspace,
        "delete" | "del" => Delete,
        "insert" | "ins" => Insert,
        "home" => Home,
        "end" => End,
        "pageup" | "pgup" => PageUp,
        "pagedown" | "pgdn" => PageDown,
        "left" => Left,
        "right" => Right,
        "up" => Up,
        "down" => Down,
        "capslock" | "caps" => CapsLock,
        "scrolllock" => ScrollLock,
        "pause" => Pause,
        "printscreen" | "print" | "prtsc" => PrintScreen,
        "grave" | "backtick" => Grave,
        "minus" => Minus,
        "equal" | "equals" => Equal,
        "comma" => Comma,
        "period" => Period,
        "slash" => Slash,
        "rightalt" | "ralt" | "altgr" => RightAlt,
        "rightctrl" | "rctrl" => RightCtrl,
        "rightshift" | "rshift" => RightShift,
        "mouse4" | "xbutton1" | "mouseback" => Mouse4,
        "mouse5" | "xbutton2" | "mouseforward" => Mouse5,
        "mousemiddle" | "middleclick" | "mouse3" => MouseMiddle,
        _ => return None,
    })
}

fn key_name(k: Key) -> String {
    use Key::*;
    match k {
        Char(c) => c.to_string(),
        F(n) => format!("F{n}"),
        Space => "Space".into(),
        Enter => "Enter".into(),
        Tab => "Tab".into(),
        Escape => "Escape".into(),
        Backspace => "Backspace".into(),
        Delete => "Delete".into(),
        Insert => "Insert".into(),
        Home => "Home".into(),
        End => "End".into(),
        PageUp => "PageUp".into(),
        PageDown => "PageDown".into(),
        Left => "Left".into(),
        Right => "Right".into(),
        Up => "Up".into(),
        Down => "Down".into(),
        CapsLock => "CapsLock".into(),
        ScrollLock => "ScrollLock".into(),
        Pause => "Pause".into(),
        PrintScreen => "PrintScreen".into(),
        Grave => "`".into(),
        Minus => "-".into(),
        Equal => "=".into(),
        BracketLeft => "[".into(),
        BracketRight => "]".into(),
        Backslash => "\\".into(),
        Semicolon => ";".into(),
        Quote => "'".into(),
        Comma => "Comma".into(),
        Period => "Period".into(),
        Slash => "/".into(),
        RightAlt => "RightAlt".into(),
        RightCtrl => "RightCtrl".into(),
        RightShift => "RightShift".into(),
        Mouse4 => "Mouse4".into(),
        Mouse5 => "Mouse5".into(),
        MouseMiddle => "MouseMiddle".into(),
    }
}

impl fmt::Display for Hotkey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let meta = if cfg!(target_os = "macos") { "Cmd" } else { "Win" };
        let mut parts: Vec<String> = Vec::new();
        if self.mods.ctrl {
            parts.push("Ctrl".into());
        }
        if self.mods.meta {
            parts.push(meta.into());
        }
        if self.mods.alt {
            parts.push("Alt".into());
        }
        if self.mods.shift {
            parts.push("Shift".into());
        }
        if let Some(k) = self.key {
            parts.push(key_name(k));
        }
        f.write_str(&parts.join("+"))
    }
}

impl Serialize for Hotkey {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Hotkey {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Hotkey::parse(&s).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_display() {
        let m = if cfg!(target_os = "macos") { "Cmd" } else { "Win" };
        assert_eq!(Hotkey::parse("ctrl+win").unwrap().to_string(), format!("Ctrl+{m}"));
        assert_eq!(Hotkey::parse("Win + Ctrl + space").unwrap().to_string(), format!("Ctrl+{m}+Space"));
        assert_eq!(Hotkey::parse("cmd+ctrl").unwrap().to_string(), format!("Ctrl+{m}"));
        assert_eq!(Hotkey::parse("f9").unwrap().to_string(), "F9");
        assert_eq!(Hotkey::parse("RightAlt").unwrap().key, Some(Key::RightAlt));
        assert_eq!(Hotkey::parse("Ctrl+Shift+r").unwrap().key, Some(Key::Char('R')));
        assert!(Hotkey::parse("ctrl").is_err());
        assert!(Hotkey::parse("ctrl+a+b").is_err());
        assert!(Hotkey::parse("f25").is_err());
        assert!(Hotkey::parse("").is_err());
    }
}
