use crate::{Keystroke, Modifiers};
use serde::Deserialize;

/// A keyboard position and modifiers before character/layout normalization.
/// Codes use the UI Events KeyboardEvent.code vocabulary.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Hash)]
pub struct PhysicalKeystroke {
    /// Layout-independent position using the UI Events KeyboardEvent.code spelling.
    pub code: String,
    /// Modifiers as pressed, including Shift on punctuation keys.
    pub modifiers: Modifiers,
}

const NAMED_KEYS: &[(&str, &str)] = &[
    ("down", "ArrowDown"),
    ("left", "ArrowLeft"),
    ("right", "ArrowRight"),
    ("up", "ArrowUp"),
    ("`", "Backquote"),
    ("\\", "Backslash"),
    ("[", "BracketLeft"),
    ("]", "BracketRight"),
    (",", "Comma"),
    ("=", "Equal"),
    ("-", "Minus"),
    (".", "Period"),
    ("'", "Quote"),
    (";", "Semicolon"),
    ("/", "Slash"),
    ("space", "Space"),
    ("enter", "Enter"),
    ("tab", "Tab"),
    ("backspace", "Backspace"),
    ("delete", "Delete"),
    ("escape", "Escape"),
    ("home", "Home"),
    ("end", "End"),
    ("insert", "Insert"),
    ("pageup", "PageUp"),
    ("pagedown", "PageDown"),
    ("numpadadd", "NumpadAdd"),
    ("numpadsubtract", "NumpadSubtract"),
    ("numpadmultiply", "NumpadMultiply"),
    ("numpaddivide", "NumpadDivide"),
    ("numpaddecimal", "NumpadDecimal"),
    ("numpadenter", "NumpadEnter"),
    ("numpadequal", "NumpadEqual"),
];
const SHIFTED: &str = "~!@#$%^&*()_+{}|:\"<>?";
const UNSHIFTED: &str = "`1234567890-=[]\\;',./";
const FUNCTION_KEYS: &[&str] = &[
    "F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10", "F11", "F12", "F13", "F14", "F15",
    "F16", "F17", "F18", "F19", "F20", "F21", "F22", "F23", "F24",
];
const NUMPAD_KEYS: &[&str] = &[
    "Numpad0", "Numpad1", "Numpad2", "Numpad3", "Numpad4", "Numpad5", "Numpad6", "Numpad7",
    "Numpad8", "Numpad9",
];

impl PhysicalKeystroke {
    /// Infer an ANSI key for programmatically dispatched strokes without a scan code.
    pub fn from_logical(stroke: &Keystroke) -> Option<Self> {
        let mut modifiers = stroke.modifiers;
        let mut key = stroke.key.to_ascii_lowercase();
        if key.len() == 1 {
            if let Some(index) = SHIFTED.find(&key) {
                key = UNSHIFTED[index..index + 1].to_owned();
                modifiers.shift = true;
            }
        }
        let code = if key.len() == 1 && key.as_bytes()[0].is_ascii_lowercase() {
            format!("Key{}", key.to_ascii_uppercase())
        } else if key.len() == 1 && key.as_bytes()[0].is_ascii_digit() {
            format!("Digit{key}")
        } else if let Some(code) = FUNCTION_KEYS
            .iter()
            .find(|code| code.eq_ignore_ascii_case(&key))
        {
            (*code).to_owned()
        } else if let Some(code) = NUMPAD_KEYS
            .iter()
            .find(|code| code.eq_ignore_ascii_case(&key))
        {
            (*code).to_owned()
        } else {
            NAMED_KEYS
                .iter()
                .find(|(name, _)| *name == key)?
                .1
                .to_owned()
        };
        Some(Self { code, modifiers })
    }

    /// A stable unshifted spelling for storage and display of physical shortcuts.
    pub fn unshifted_key(&self) -> Option<String> {
        let code = self.code.to_ascii_lowercase();
        if code.len() == 4 && code.starts_with("key") && code.as_bytes()[3].is_ascii_lowercase() {
            Some(code[3..].to_owned())
        } else if code.len() == 6
            && code.starts_with("digit")
            && code.as_bytes()[5].is_ascii_digit()
        {
            Some(code[5..].to_owned())
        } else if FUNCTION_KEYS
            .iter()
            .chain(NUMPAD_KEYS)
            .any(|name| name.eq_ignore_ascii_case(&code))
        {
            Some(code)
        } else {
            NAMED_KEYS
                .iter()
                .find(|(_, name)| name.eq_ignore_ascii_case(&code))
                .map(|(key, _)| (*key).to_owned())
        }
    }
}

/// XKB key names describe positions independently of the active symbol layout.
pub fn xkb_physical_key(name: &str) -> Option<&'static str> {
    for (prefix, row) in [
        (
            "AE",
            &[
                "Digit1", "Digit2", "Digit3", "Digit4", "Digit5", "Digit6", "Digit7", "Digit8",
                "Digit9", "Digit0", "Minus", "Equal",
            ][..],
        ),
        (
            "AD",
            &[
                "KeyQ",
                "KeyW",
                "KeyE",
                "KeyR",
                "KeyT",
                "KeyY",
                "KeyU",
                "KeyI",
                "KeyO",
                "KeyP",
                "BracketLeft",
                "BracketRight",
            ][..],
        ),
        (
            "AC",
            &[
                "KeyA",
                "KeyS",
                "KeyD",
                "KeyF",
                "KeyG",
                "KeyH",
                "KeyJ",
                "KeyK",
                "KeyL",
                "Semicolon",
                "Quote",
            ][..],
        ),
        (
            "AB",
            &[
                "KeyZ", "KeyX", "KeyC", "KeyV", "KeyB", "KeyN", "KeyM", "Comma", "Period", "Slash",
            ][..],
        ),
        ("FK", FUNCTION_KEYS),
    ] {
        if let Some(index) = name
            .strip_prefix(prefix)
            .and_then(|index| index.parse::<usize>().ok())
        {
            return index
                .checked_sub(1)
                .and_then(|index| row.get(index))
                .copied();
        }
    }
    if let Some(index) = name
        .strip_prefix("KP")
        .and_then(|index| index.parse::<usize>().ok())
    {
        return NUMPAD_KEYS.get(index).copied();
    }
    Some(match name {
        "TLDE" => "Backquote",
        "BKSL" => "Backslash",
        "RTRN" => "Enter",
        "SPCE" => "Space",
        "TAB" => "Tab",
        "BKSP" => "Backspace",
        "ESC" => "Escape",
        "INS" => "Insert",
        "DELE" => "Delete",
        "HOME" => "Home",
        "END" => "End",
        "PGUP" => "PageUp",
        "PGDN" => "PageDown",
        "LEFT" => "ArrowLeft",
        "RGHT" => "ArrowRight",
        "UP" => "ArrowUp",
        "DOWN" => "ArrowDown",
        "KPDL" => "NumpadDecimal",
        "KPDV" => "NumpadDivide",
        "KPMU" => "NumpadMultiply",
        "KPSU" => "NumpadSubtract",
        "KPAD" => "NumpadAdd",
        "KPEN" => "NumpadEnter",
        "KPEQ" => "NumpadEqual",
        _ => return None,
    })
}

/// Windows set-1 scan codes, including the extended-key bit in bit eight.
pub fn windows_physical_key(scan: u16, virtual_key: u16) -> Option<&'static str> {
    if (0x70..=0x87).contains(&virtual_key) {
        return FUNCTION_KEYS.get((virtual_key - 0x70) as usize).copied();
    }
    Some(match scan {
        0x01 => "Escape",
        0x02 => "Digit1",
        0x03 => "Digit2",
        0x04 => "Digit3",
        0x05 => "Digit4",
        0x06 => "Digit5",
        0x07 => "Digit6",
        0x08 => "Digit7",
        0x09 => "Digit8",
        0x0a => "Digit9",
        0x0b => "Digit0",
        0x0c => "Minus",
        0x0d => "Equal",
        0x0e => "Backspace",
        0x0f => "Tab",
        0x10 => "KeyQ",
        0x11 => "KeyW",
        0x12 => "KeyE",
        0x13 => "KeyR",
        0x14 => "KeyT",
        0x15 => "KeyY",
        0x16 => "KeyU",
        0x17 => "KeyI",
        0x18 => "KeyO",
        0x19 => "KeyP",
        0x1a => "BracketLeft",
        0x1b => "BracketRight",
        0x1c => "Enter",
        0x1e => "KeyA",
        0x1f => "KeyS",
        0x20 => "KeyD",
        0x21 => "KeyF",
        0x22 => "KeyG",
        0x23 => "KeyH",
        0x24 => "KeyJ",
        0x25 => "KeyK",
        0x26 => "KeyL",
        0x27 => "Semicolon",
        0x28 => "Quote",
        0x29 => "Backquote",
        0x2b => "Backslash",
        0x2c => "KeyZ",
        0x2d => "KeyX",
        0x2e => "KeyC",
        0x2f => "KeyV",
        0x30 => "KeyB",
        0x31 => "KeyN",
        0x32 => "KeyM",
        0x33 => "Comma",
        0x34 => "Period",
        0x35 => "Slash",
        0x37 => "NumpadMultiply",
        0x39 => "Space",
        0x47 => "Numpad7",
        0x48 => "Numpad8",
        0x49 => "Numpad9",
        0x4a => "NumpadSubtract",
        0x4b => "Numpad4",
        0x4c => "Numpad5",
        0x4d => "Numpad6",
        0x4e => "NumpadAdd",
        0x4f => "Numpad1",
        0x50 => "Numpad2",
        0x51 => "Numpad3",
        0x52 => "Numpad0",
        0x53 => "NumpadDecimal",
        0x11c => "NumpadEnter",
        0x135 => "NumpadDivide",
        0x147 => "Home",
        0x148 => "ArrowUp",
        0x149 => "PageUp",
        0x14b => "ArrowLeft",
        0x14d => "ArrowRight",
        0x14f => "End",
        0x150 => "ArrowDown",
        0x151 => "PageDown",
        0x152 => "Insert",
        0x153 => "Delete",
        _ => return None,
    })
}

/// macOS hardware key codes from HIToolbox's Events.h virtual-key constants.
pub fn macos_physical_key(code: u16) -> Option<&'static str> {
    Some(match code {
        0x00 => "KeyA",
        0x01 => "KeyS",
        0x02 => "KeyD",
        0x03 => "KeyF",
        0x04 => "KeyH",
        0x05 => "KeyG",
        0x06 => "KeyZ",
        0x07 => "KeyX",
        0x08 => "KeyC",
        0x09 => "KeyV",
        0x0b => "KeyB",
        0x0c => "KeyQ",
        0x0d => "KeyW",
        0x0e => "KeyE",
        0x0f => "KeyR",
        0x10 => "KeyY",
        0x11 => "KeyT",
        0x12 => "Digit1",
        0x13 => "Digit2",
        0x14 => "Digit3",
        0x15 => "Digit4",
        0x16 => "Digit6",
        0x17 => "Digit5",
        0x18 => "Equal",
        0x19 => "Digit9",
        0x1a => "Digit7",
        0x1b => "Minus",
        0x1c => "Digit8",
        0x1d => "Digit0",
        0x1e => "BracketRight",
        0x1f => "KeyO",
        0x20 => "KeyU",
        0x21 => "BracketLeft",
        0x22 => "KeyI",
        0x23 => "KeyP",
        0x24 => "Enter",
        0x25 => "KeyL",
        0x26 => "KeyJ",
        0x27 => "Quote",
        0x28 => "KeyK",
        0x29 => "Semicolon",
        0x2a => "Backslash",
        0x2b => "Comma",
        0x2c => "Slash",
        0x2d => "KeyN",
        0x2e => "KeyM",
        0x2f => "Period",
        0x30 => "Tab",
        0x31 => "Space",
        0x32 => "Backquote",
        0x33 => "Backspace",
        0x35 => "Escape",
        0x40 => "F17",
        0x41 => "NumpadDecimal",
        0x43 => "NumpadMultiply",
        0x45 => "NumpadAdd",
        0x4b => "NumpadDivide",
        0x4c => "NumpadEnter",
        0x4e => "NumpadSubtract",
        0x4f => "F18",
        0x50 => "F19",
        0x51 => "NumpadEqual",
        0x52 => "Numpad0",
        0x53 => "Numpad1",
        0x54 => "Numpad2",
        0x55 => "Numpad3",
        0x56 => "Numpad4",
        0x57 => "Numpad5",
        0x58 => "Numpad6",
        0x59 => "Numpad7",
        0x5a => "F20",
        0x5b => "Numpad8",
        0x5c => "Numpad9",
        0x60 => "F5",
        0x61 => "F6",
        0x62 => "F7",
        0x63 => "F3",
        0x64 => "F8",
        0x65 => "F9",
        0x67 => "F11",
        0x69 => "F13",
        0x6a => "F16",
        0x6b => "F14",
        0x6d => "F10",
        0x6f => "F12",
        0x71 => "F15",
        0x72 => "Insert",
        0x73 => "Home",
        0x74 => "PageUp",
        0x75 => "Delete",
        0x76 => "F4",
        0x77 => "End",
        0x78 => "F2",
        0x79 => "PageDown",
        0x7a => "F1",
        0x7b => "ArrowLeft",
        0x7c => "ArrowRight",
        0x7d => "ArrowDown",
        0x7e => "ArrowUp",
        _ => return None,
    })
}
