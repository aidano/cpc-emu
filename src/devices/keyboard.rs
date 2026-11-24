// CPC keyboard matrix: 16 lines (10 used) of 8 keys each, read active-low
// through PSG register 14. Scancodes are 0xXY where X = line, Y = bit mask
// index.

pub const SCANCODE_SHIFT: u8 = 0x25;
const SCANCODE_RETURN: u8 = 0x06;
const SCANCODE_SPACE: u8 = 0x57;
const SCANCODE_DEL: u8 = 0x97;
const SCANCODE_CLR: u8 = 0x20;
const SCANCODE_ESCAPE: u8 = 0x82;
const SCANCODE_TAB: u8 = 0x84;
const SCANCODE_CAPS: u8 = 0x86;
const SCANCODE_UP: u8 = 0x00;
const SCANCODE_DOWN: u8 = 0x02;
const SCANCODE_LEFT: u8 = 0x10;
const SCANCODE_RIGHT: u8 = 0x01;

/// Joystick pressed bits, in the bit positions the joysticks occupy in the
/// keyboard matrix: joystick 0 is matrix line 9, joystick 1 line 6, with
/// Up/Down/Left/Right on bits 0-3 and Fire 2/Fire 1 on bits 4/5 (active low,
/// like keys). Joy 1 shares line 6 with the keys G, 5, F, 6 and T/R.
pub const JOY_UP: u8 = 1 << 0;
pub const JOY_DOWN: u8 = 1 << 1;
pub const JOY_LEFT: u8 = 1 << 2;
pub const JOY_RIGHT: u8 = 1 << 3;
pub const JOY_FIRE2: u8 = 1 << 4;
pub const JOY_FIRE1: u8 = 1 << 5;
/// Start is not part of the joystick ports' matrix entries; it only takes
/// part in the guest-keyboard mirror (as Return).
pub const JOY_START: u8 = 1 << 6;

const JOY_LINE: [u8; 2] = [9, 6];

/// The mirror maps joystick masks onto the guest keyboard's own matrix
/// entries: the cursor keys, Space for fire and Return for Start. Entries
/// are (mask bits, matrix line, line bits); fire1/fire2 both press Space.
const MIRROR_KEYS: [(u8, u8, u8); 6] = [
    (JOY_UP, 0, 0x01),     // cursor up: line 0, bit 0
    (JOY_DOWN, 0, 0x04),   // cursor down: line 0, bit 2
    (JOY_RIGHT, 0, 0x02),  // cursor right: line 0, bit 1
    (JOY_LEFT, 1, 0x01),   // cursor left: line 1, bit 0
    (JOY_FIRE1 | JOY_FIRE2, 5, 0x80), // space: line 5, bit 7
    (JOY_START, 0, 0x40),  // return: line 0, bit 6
];

#[derive(Debug, Clone)]
pub struct Keyboard {
    /// matrix[line] with active-low key bits.
    matrix: [u8; 16],
    /// Per-player joystick pressed bits (only the line's low 6 bits count).
    joystick: [u8; 2],
    /// Per-player mirror masks: joystick bits read as guest keys.
    mirror: [u8; 2],
    /// The scancodes synthesized for the currently-held shifted character:
    /// the guest's Shift bit plus the key. The host releases a physical
    /// key under a different text when its own Shift state changed between
    /// the press and the release (press '"', release '2'), so the release
    /// is matched by identity, not text.
    synthetic: Option<(String, Vec<u8>)>,
}

fn scancodes_for_char(ch: char) -> Option<Vec<u8>> {
    let letters = "abcdefghijklmnopqrstuvwxyz";
    let digits = "0123456789";
    if let Some(idx) = letters.find(ch.to_ascii_lowercase()) {
        let letter = LETTER_SCANCODES[idx];
        let mut codes = Vec::new();
        if ch.is_ascii_uppercase() {
            codes.push(SCANCODE_SHIFT);
        }
        codes.push(letter);
        return Some(codes);
    }
    if let Some(idx) = digits.find(ch) {
        return Some(vec![DIGIT_SCANCODES[idx]]);
    }
    let simple = match ch {
        ' ' => Some(SCANCODE_SPACE),
        '\n' | '\r' => Some(SCANCODE_RETURN),
        '\u{8}' | '\u{7f}' => Some(SCANCODE_DEL),
        ',' => Some(0x47),
        '.' => Some(0x37),
        '/' => Some(0x36),
        ';' => Some(0x34),
        '-' => Some(0x31),
        '=' => Some(0x31),
        '@' => Some(0x32),
        '^' => Some(0x30),
        '\\' => Some(0x26),
        '[' => Some(0x21),
        ']' => Some(0x23),
        _ => None,
    };
    let shifted = match ch {
        '!' => Some(0x80),
        '"' => Some(0x81),
        '#' => Some(0x71),
        '$' => Some(0x70),
        '%' => Some(0x61),
        '&' => Some(0x60),
        '\'' => Some(0x51),
        '(' => Some(0x50),
        ')' => Some(0x41),
        '*' => Some(0x35),
        '+' => Some(0x34),
        ':' => Some(0x35),
        '<' => Some(0x47),
        '>' => Some(0x37),
        '?' => Some(0x36),
        '_' => Some(0x40),
        '|' => Some(0x32),
        _ => None,
    };
    match (simple, shifted) {
        (Some(code), _) => Some(vec![code]),
        (None, Some(code)) => Some(vec![SCANCODE_SHIFT, code]),
        (None, None) => None,
    }
}

const DIGIT_SCANCODES: [u8; 10] = [0x40, 0x80, 0x81, 0x71, 0x70, 0x61, 0x60, 0x51, 0x50, 0x41];
const LETTER_SCANCODES: [u8; 26] = [
    0x85, // a
    0x66, // b
    0x76, // c
    0x75, // d
    0x72, // e
    0x65, // f
    0x64, // g
    0x54, // h
    0x43, // i
    0x55, // j
    0x45, // k
    0x44, // l
    0x46, // m
    0x56, // n
    0x42, // o
    0x33, // p
    0x83, // q
    0x62, // r
    0x74, // s
    0x63, // t
    0x52, // u
    0x67, // v
    0x73, // w
    0x77, // x
    0x53, // y
    0x87, // z
];

fn named_key_scancode(text: &str) -> Option<u8> {
    match text {
        "Enter" | "\n" | "\r" => Some(SCANCODE_RETURN),
        " " | "Space" => Some(SCANCODE_SPACE),
        "Backspace" => Some(SCANCODE_DEL),
        "Delete" => Some(SCANCODE_CLR),
        "Escape" => Some(SCANCODE_ESCAPE),
        "Tab" => Some(SCANCODE_TAB),
        "CapsLock" => Some(SCANCODE_CAPS),
        "Up" | "UpArrow" => Some(SCANCODE_UP),
        "Down" | "DownArrow" => Some(SCANCODE_DOWN),
        "Left" | "LeftArrow" => Some(SCANCODE_LEFT),
        "Right" | "RightArrow" => Some(SCANCODE_RIGHT),
        _ => None,
    }
}

impl Keyboard {
    pub fn default() -> Keyboard {
        Keyboard { matrix: [0xFF; 16], joystick: [0; 2], mirror: [0; 2], synthetic: None }
    }

    /// Updates the held state of a joystick port (0 or 1): the mask bits
    /// stay applied on every matrix read until changed — the joystick is
    /// polled, not event-driven like keys. Start (bit 6) never reaches the
    /// ports; it exists only for the guest-key mirror.
    pub fn set_joystick(&mut self, index: u8, mask: u8) {
        let slot = index as usize;
        if slot < self.joystick.len() {
            self.joystick[slot] = mask & (JOY_FIRE1 | JOY_FIRE2 | JOY_RIGHT | JOY_LEFT | JOY_DOWN | JOY_UP);
        }
    }

    /// Updates the mirrored guest keys of a joystick port (0 or 1): the
    /// mask bits read as held cursor keys, Space (fire) and Return (Start)
    /// until changed. Applied at matrix-read time, so it can never desync
    /// from the keys a physical press/release left in the matrix.
    pub fn set_mirror(&mut self, index: u8, mask: u8) {
        let slot = index as usize;
        if slot < self.mirror.len() {
            self.mirror[slot] =
                mask & (JOY_UP | JOY_DOWN | JOY_LEFT | JOY_RIGHT | JOY_FIRE1 | JOY_FIRE2 | JOY_START);
        }
    }

    pub fn row(&self, line: u8) -> u8 {
        let mut value = self.matrix.get(line as usize).copied().unwrap_or(0xFF);
        for (index, &joy_line) in JOY_LINE.iter().enumerate() {
            if line == joy_line {
                value &= !self.joystick[index];
            }
        }
        for player_mask in &self.mirror {
            for &(key_mask, key_line, key_bits) in MIRROR_KEYS.iter() {
                if line == key_line && player_mask & key_mask != 0 {
                    value &= !key_bits;
                }
            }
        }
        value
    }

    fn press_scancode(&mut self, code: u8) {
        let line = (code >> 4) as usize;
        let bit = code & 0x0F;
        if line < 16 && bit < 8 {
            self.matrix[line] &= !(1 << bit);
        }
    }

    fn release_scancode(&mut self, code: u8) {
        let line = (code >> 4) as usize;
        let bit = code & 0x0F;
        if line < 16 && bit < 8 {
            self.matrix[line] |= 1 << bit;
        }
    }

    /// Releases the scancodes synthesized for a held shifted character.
    fn release_synthetic(&mut self) {
        if let Some((_, codes)) = self.synthetic.take() {
            for code in codes {
                self.release_scancode(code);
            }
        }
    }

    /// Presses every key the given text maps to (called on key-down).
    pub fn press_text(&mut self, text: &str) {
        // A new press ends any shifted character still held: the host
        // released that key before this press (possibly under a different
        // text), so its guest bits must not survive.
        self.release_synthetic();
        if let Some(code) = named_key_scancode(text) {
            self.press_scancode(code);
            return;
        }
        for ch in text.chars() {
            if let Some(codes) = scancodes_for_char(ch) {
                if codes.first() == Some(&SCANCODE_SHIFT) {
                    self.synthetic = Some((ch.to_string(), codes.clone()));
                }
                for code in codes {
                    self.press_scancode(code);
                }
            }
        }
    }

    /// Releases every key the given text maps to (called on key-up).
    pub fn release_text(&mut self, text: &str) {
        // The release of a key other than the shifted character currently
        // held ends that hold: the host released the physical key under a
        // different text when its own Shift state changed in between
        // (press '"', release '2'), leaving the synthesized Shift stuck.
        let held_matches = self.synthetic.as_ref().map_or(false, |(held, _)| held == text);
        if !held_matches {
            self.release_synthetic();
        }
        if let Some(code) = named_key_scancode(text) {
            self.release_scancode(code);
            return;
        }
        for ch in text.chars() {
            if let Some(codes) = scancodes_for_char(ch) {
                for code in codes {
                    self.release_scancode(code);
                }
            }
        }
        if held_matches {
            // The matched release just freed the hold's own bits.
            self.synthetic = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Keyboard;

    #[test]
    fn test_no_keys_pressed_reads_ff() {
        let kbd = Keyboard::default();
        for line in 0..16u8 {
            assert_eq!(kbd.row(line), 0xFF);
        }
    }

    #[test]
    fn test_press_and_release_char() {
        let mut kbd = Keyboard::default();
        kbd.press_text("a");
        assert_eq!(kbd.row(8), 0xDF, "'a' is line 8, bit 5");
        kbd.release_text("a");
        assert_eq!(kbd.row(8), 0xFF);
    }

    #[test]
    fn test_uppercase_presses_shift_too() {
        let mut kbd = Keyboard::default();
        kbd.press_text("A");
        assert_eq!(kbd.row(2), 0xDF, "shift is line 2, bit 5");
        assert_eq!(kbd.row(8), 0xDF);
        kbd.release_text("A");
        assert_eq!(kbd.row(2), 0xFF);
        assert_eq!(kbd.row(8), 0xFF);
    }

    #[test]
    fn test_named_key_return() {
        let mut kbd = Keyboard::default();
        kbd.press_text("Enter");
        assert_eq!(kbd.row(0), 0xBF, "return is line 0, bit 6");
        kbd.release_text("Enter");
        assert_eq!(kbd.row(0), 0xFF);
    }

    #[test]
    fn test_return_key_texts() {
        // Slint's key table delivers "\n" for the Return key (winit's
        // NamedKey::Enter), and backends that fall back to the raw event
        // text deliver "\r": both must map to the CPC's Return.
        for text in ["\n", "\r"] {
            let mut kbd = Keyboard::default();
            kbd.press_text(text);
            assert_eq!(kbd.row(0), 0xBF, "{text:?} must press return");
            kbd.release_text(text);
            assert_eq!(kbd.row(0), 0xFF, "{text:?} must release return");
        }
    }

    #[test]
    fn test_plus_requires_shift() {
        let mut kbd = Keyboard::default();
        kbd.press_text("+");
        assert_eq!(kbd.row(2), 0xDF);
        assert_eq!(kbd.row(3), 0xEF, "'+' is line 3, bit 4");
    }

    #[test]
    fn test_joystick0_masks_line_9() {
        let mut kbd = Keyboard::default();
        kbd.set_joystick(0, super::JOY_UP | super::JOY_FIRE1);
        assert_eq!(kbd.row(9), !(super::JOY_UP | super::JOY_FIRE1) & 0xFF);
        assert_eq!(kbd.row(9), 0xDE, "up is bit 0, fire 1 is bit 5");
        assert_eq!(kbd.row(0), 0xFF, "other lines are untouched");
        kbd.set_joystick(0, 0);
        assert_eq!(kbd.row(9), 0xFF);
    }

    #[test]
    fn test_joystick1_masks_line_6_alongside_keys() {
        let mut kbd = Keyboard::default();
        // Joy 1 fire 1 shares matrix line 6 bit 5 with the F key.
        kbd.press_text("f");
        kbd.set_joystick(1, super::JOY_UP | super::JOY_FIRE1);
        assert_eq!(kbd.row(6), 0xDE, "bits 0 (up) and 5 (fire 1) cleared");
        // Releasing the joystick must leave the held key in place.
        kbd.set_joystick(1, 0);
        assert_eq!(kbd.row(6), 0xDF, "the F key still holds bit 5");
        kbd.release_text("f");
        assert_eq!(kbd.row(6), 0xFF);
    }

    #[test]
    fn test_joystick_index_out_of_range_is_ignored() {
        let mut kbd = Keyboard::default();
        kbd.set_joystick(2, super::JOY_FIRE1);
        assert_eq!(kbd.row(9), 0xFF);
        assert_eq!(kbd.row(6), 0xFF);
    }

    #[test]
    fn test_mirror_maps_onto_guest_keys() {
        let mut kbd = Keyboard::default();
        // Up, down and right share line 0; left is line 1 bit 0.
        kbd.set_mirror(0, super::JOY_UP | super::JOY_DOWN | super::JOY_RIGHT | super::JOY_LEFT);
        assert_eq!(kbd.row(0), 0xF8, "bits 0 (up), 1 (right), 2 (down) cleared");
        assert_eq!(kbd.row(1), 0xFE, "bit 0 (left) cleared");
        // Fire mirrors onto Space (line 5, bit 7)...
        kbd.set_mirror(0, super::JOY_FIRE1);
        assert_eq!(kbd.row(5), 0x7F);
        // ...and Start onto Return (line 0, bit 6).
        kbd.set_mirror(0, super::JOY_START);
        assert_eq!(kbd.row(0), 0xBF);
        kbd.set_mirror(0, 0);
        assert_eq!(kbd.row(0), 0xFF);
        assert_eq!(kbd.row(1), 0xFF);
        assert_eq!(kbd.row(5), 0xFF);
    }

    #[test]
    fn test_mirror_survives_key_releases() {
        let mut kbd = Keyboard::default();
        // The physical Up arrow is held, then the pad also holds it: the
        // two sources share one matrix bit.
        kbd.press_text("Up");
        kbd.set_mirror(0, super::JOY_UP);
        assert_eq!(kbd.row(0), 0xFE);
        // The physical release must not free the bit while the pad holds
        // it — the mirror is applied per read, not as events.
        kbd.release_text("Up");
        assert_eq!(kbd.row(0), 0xFE, "the mirror still holds cursor up");
        kbd.set_mirror(0, 0);
        assert_eq!(kbd.row(0), 0xFF);
    }

    #[test]
    fn test_start_reaches_the_mirror_but_not_the_ports() {
        let mut kbd = Keyboard::default();
        kbd.set_joystick(0, super::JOY_UP | super::JOY_START);
        kbd.set_mirror(1, super::JOY_START);
        assert_eq!(kbd.row(9), 0xFE, "only up on the port line");
        assert_eq!(kbd.row(0), 0xBF, "player 2's start mirrors return");
        // The matrix is stateful bits: a second player's Return shares the
        // same bit rather than adding a second one.
        kbd.set_mirror(0, super::JOY_START);
        assert_eq!(kbd.row(0), 0xBF);
    }

    /// The host releases a shifted key under a different text when its own
    /// Shift state changed in between: press '"', release '2'. The old code
    /// released only the key and left the synthesized Shift stuck — typed
    /// letters turned uppercase and a game's menu read '3' as '#'. The
    /// mismatched release must free the whole synthesized hold.
    #[test]
    fn test_release_under_a_different_text_frees_the_shift() {
        let mut kbd = Keyboard::default();
        kbd.press_text("\""); // the host's Shift+2: guest Shift + 2 key
        assert_eq!(kbd.row(2), 0xDF, "synthesized shift held");
        assert_eq!(kbd.row(8), 0xFD, "the 2 key held");
        kbd.release_text("2"); // the release as the host labeled it
        assert_eq!(kbd.row(2), 0xFF, "no stuck shift");
        assert_eq!(kbd.row(8), 0xFF, "no stuck 2 key");
        // The next unshifted press must type as itself, not '#'.
        kbd.press_text("3");
        assert_eq!(kbd.row(7), 0xFD, "'3' is line 7, bit 1, unshifted");
        kbd.release_text("3");
        assert_eq!(kbd.row(7), 0xFF);
    }

    /// The US-layout twin: press '"', release '\''. The release frees the
    /// synthesized Shift but must also free the 2 key it was pressed with.
    #[test]
    fn test_release_as_the_unshifted_key_frees_the_pair() {
        let mut kbd = Keyboard::default();
        kbd.press_text("\"");
        kbd.release_text("'");
        assert_eq!(kbd.row(2), 0xFF, "no stuck shift");
        assert_eq!(kbd.row(8), 0xFF, "no stuck 2 key");
    }

    /// A clean press/release of the same text behaves exactly as before.
    #[test]
    fn test_shifted_release_under_the_same_text_is_unchanged() {
        let mut kbd = Keyboard::default();
        kbd.press_text("\"");
        assert_eq!(kbd.row(2), 0xDF);
        assert_eq!(kbd.row(8), 0xFD);
        kbd.release_text("\"");
        assert_eq!(kbd.row(2), 0xFF);
        assert_eq!(kbd.row(8), 0xFF);
    }

    /// The user's reported flow: the old bug turned letters uppercase
    /// after a mismatched '"' release. With the synthesized hold ended by
    /// the next press, the stuck state cannot survive to any keypress:
    /// the next letter always types unshifted, however the host labeled
    /// the earlier release.
    #[test]
    fn test_uppercase_typing_flow_recovers() {
        let mut kbd = Keyboard::default();
        kbd.press_text("\""); // Shift + 2 held
        assert_eq!(kbd.row(2), 0xDF);
        // No release arrives before the next press (a lost release): the
        // press itself must end the hold and type 'd' unshifted.
        kbd.press_text("d"); // 'd' is 0x75: line 7, bit 5
        assert_eq!(kbd.row(7), 0xDF, "the d key held");
        assert_eq!(kbd.row(2), 0xFF, "the stuck shift cannot ride along");
        assert_eq!(kbd.row(8), 0xFF, "the quote's 2 key is freed too");
        kbd.release_text("d");
        assert_eq!(kbd.row(7), 0xFF);
        // The host's eventual release for the quote key (as '2' or any
        // other label) must not resurrect anything.
        kbd.release_text("2");
        assert_eq!(kbd.row(8), 0xFF);
        assert_eq!(kbd.row(2), 0xFF);
    }
}
