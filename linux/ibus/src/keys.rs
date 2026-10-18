//! IBus key events to `ShanjieKey`, with the meaning of windows/src/bridge.rs `sj_translate`: specials by
//! kind, every other key as its unshifted keycap. The keycap comes from the physical key (evdev code on a
//! US layout, as the macOS shell does), so the desktop's layout never changes what the core receives.
use shanjie_core::ffi::ShanjieKey;

// IBusModifierType (ibustypes.h).
pub const SHIFT_MASK: u32 = 1 << 0;
pub const LOCK_MASK: u32 = 1 << 1;
pub const CONTROL_MASK: u32 = 1 << 2;
pub const MOD1_MASK: u32 = 1 << 3;
pub const MOD4_MASK: u32 = 1 << 6;
pub const SUPER_MASK: u32 = 1 << 26;
pub const META_MASK: u32 = 1 << 28;
pub const RELEASE_MASK: u32 = 1 << 30;

// Keyvals (ibuskeysyms.h).
pub const SHIFT_L: u32 = 0xffe1;
pub const SHIFT_R: u32 = 0xffe2;
const SUPER_L: u32 = 0xffeb;
const SUPER_R: u32 = 0xffec;

// evdev codes of the two Shift keys (linux/input-event-codes.h).
pub const KEY_LEFTSHIFT: u32 = 42;
pub const KEY_RIGHTSHIFT: u32 = 54;

pub fn is_shift(keyval: u32) -> bool {
    keyval == SHIFT_L || keyval == SHIFT_R
}

/// Super (the Windows key) and anything held with it belongs to the desktop.
pub fn system_shortcut(keyval: u32, state: u32) -> bool {
    keyval == SUPER_L || keyval == SUPER_R || state & (SUPER_MASK | MOD4_MASK | META_MASK) != 0
}

/// bit0 SHIFT, bit1 CONTROL, bit2 OPTION (Alt), bit3 COMMAND (Super), bit4 CAPSLOCK.
pub fn modifiers(state: u32) -> u32 {
    (state & SHIFT_MASK != 0) as u32
        | ((state & CONTROL_MASK != 0) as u32) << 1
        | ((state & MOD1_MASK != 0) as u32) << 2
        | ((state & (SUPER_MASK | MOD4_MASK) != 0) as u32) << 3
        | ((state & LOCK_MASK != 0) as u32) << 4
}

fn special(keyval: u32) -> Option<u32> {
    Some(match keyval {
        0x20 | 0xff80 => 2,          // space, KP_Space
        0xff0d | 0xff8d => 3,        // Return, KP_Enter
        0xff08 => 4,                 // BackSpace
        0xffff | 0xff9f => 5,        // Delete, KP_Delete
        0xff1b => 6,                 // Escape
        0xff51 | 0xff96 => 7,        // Left
        0xff53 | 0xff98 => 8,        // Right
        0xff52 | 0xff97 => 9,        // Up
        0xff54 | 0xff99 => 10,       // Down
        0xff50 | 0xff95 => 11,       // Home
        0xff57 | 0xff9c => 12,       // End
        0xff09 | 0xfe20 | 0xff89 => 13, // Tab, ISO_Left_Tab (Shift+Tab), KP_Tab
        _ => return None,
    })
}

/// The unshifted keycap of an evdev code on the ANSI layout.
fn keycap(keycode: u32) -> Option<char> {
    const ROW: [(u32, char); 47] = [
        (2, '1'), (3, '2'), (4, '3'), (5, '4'), (6, '5'), (7, '6'), (8, '7'), (9, '8'), (10, '9'), (11, '0'),
        (12, '-'), (13, '='), (16, 'q'), (17, 'w'), (18, 'e'), (19, 'r'), (20, 't'), (21, 'y'), (22, 'u'),
        (23, 'i'), (24, 'o'), (25, 'p'), (26, '['), (27, ']'), (30, 'a'), (31, 's'), (32, 'd'), (33, 'f'),
        (34, 'g'), (35, 'h'), (36, 'j'), (37, 'k'), (38, 'l'), (39, ';'), (40, '\''), (41, '`'), (43, '\\'),
        (44, 'z'), (45, 'x'), (46, 'c'), (47, 'v'), (48, 'b'), (49, 'n'), (50, 'm'), (51, ','), (52, '.'),
        (53, '/'),
    ];
    ROW.iter().find(|(code, _)| *code == keycode).map(|(_, ch)| *ch)
}

/// Keypad characters: Windows' ToUnicodeEx with an empty keyboard state gives these for the keypad.
fn keypad(keyval: u32) -> Option<char> {
    match keyval {
        0xffb0..=0xffb9 => char::from_u32(keyval - 0xffb0 + '0' as u32),
        0xffaa => Some('*'),
        0xffab => Some('+'),
        0xffad => Some('-'),
        0xffae => Some('.'),
        0xffaf => Some('/'),
        _ => None,
    }
}

pub fn translate(keyval: u32, keycode: u32, state: u32) -> Option<ShanjieKey> {
    let modifiers = modifiers(state);
    if let Some(kind) = special(keyval) {
        return Some(ShanjieKey { kind, ch: 0, modifiers });
    }
    let ch = keypad(keyval).or_else(|| keycap(keycode))?;
    Some(ShanjieKey { kind: 1, ch: ch as u32, modifiers })
}

/// The character the key types with the current layout and modifiers (Windows' ToUnicodeEx with the
/// real keyboard state), when it is printable ASCII.
pub fn typed(keyval: u32) -> Option<char> {
    (0x20..=0x7e).contains(&keyval).then(|| char::from_u32(keyval)).flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letters_follow_the_physical_key_not_the_keyval() {
        // Shift+A arrives as keyval 'A'; the core still gets the keycap 'a' with SHIFT.
        let k = translate('A' as u32, 30, SHIFT_MASK).unwrap();
        assert_eq!((k.kind, k.ch, k.modifiers), (1, 'a' as u32, 1));
        // A Dvorak user's physical 'a' key types 'a' there too, but 'q' position types an apostrophe.
        let k = translate('\'' as u32, 16, 0).unwrap();
        assert_eq!(k.ch, 'q' as u32);
    }

    #[test]
    fn specials_and_keypad() {
        assert_eq!(translate(0xff0d, 28, 0).unwrap().kind, 3);
        assert_eq!(translate(0xff8d, 96, 0).unwrap().kind, 3);
        assert_eq!(translate(0xfe20, 15, SHIFT_MASK).unwrap().kind, 13);
        let k = translate(0xffb1, 79, 0).unwrap();
        assert_eq!((k.kind, k.ch), (1, '1' as u32));
        assert!(translate(0xffbe, 59, 0).is_none()); // F1
    }

    #[test]
    fn modifier_bits() {
        assert_eq!(modifiers(SHIFT_MASK | CONTROL_MASK | MOD1_MASK | LOCK_MASK), 0b10111);
        assert_eq!(modifiers(SUPER_MASK), 8);
    }
}
