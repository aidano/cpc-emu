// CPC keyboard matrix: 16 lines (10 used) of 8 keys each, read active-low
// through PSG register 14. Scancodes are 0xXY where X = line, Y = bit mask
// index. Mapping values from Caprice32's translation tables.

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

#[derive(Debug, Clone)]
pub struct Keyboard {
    /// matrix[line] with active-low key bits.
    matrix: [u8; 16],
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
        Keyboard { matrix: [0xFF; 16] }
    }

    pub fn row(&self, line: u8) -> u8 {
        self.matrix.get(line as usize).copied().unwrap_or(0xFF)
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

    /// Presses every key the given text maps to (called on key-down).
    pub fn press_text(&mut self, text: &str) {
        if let Some(code) = named_key_scancode(text) {
            self.press_scancode(code);
            return;
        }
        for ch in text.chars() {
            if let Some(codes) = scancodes_for_char(ch) {
                for code in codes {
                    self.press_scancode(code);
                }
            }
        }
    }

    /// Releases every key the given text maps to (called on key-up).
    pub fn release_text(&mut self, text: &str) {
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
    fn test_plus_requires_shift() {
        let mut kbd = Keyboard::default();
        kbd.press_text("+");
        assert_eq!(kbd.row(2), 0xDF);
        assert_eq!(kbd.row(3), 0xEF, "'+' is line 3, bit 4");
    }
}
