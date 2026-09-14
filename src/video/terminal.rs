// Terminal display backend: renders frames as ANSI truecolor output.

use std::io::{self, Write};
use super::{Frame, VideoSink};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalScale {
    /// One character cell per pixel (literal 640x200, needs a very wide terminal)
    Full,
    /// Two vertical pixels per cell via the upper-half-block glyph (640x100 cells)
    Half,
    /// 2x4 dots per cell via braille patterns (320x50 cells)
    Braille,
}

impl TerminalScale {
    pub fn from_str(s: &str) -> Option<TerminalScale> {
        match s {
            "full" => Some(TerminalScale::Full),
            "half" => Some(TerminalScale::Half),
            "braille" => Some(TerminalScale::Braille),
            _ => None,
        }
    }

    fn cell_size(&self) -> (u16, u16) {
        match self {
            TerminalScale::Full => (1, 1),
            TerminalScale::Half => (1, 2),
            TerminalScale::Braille => (2, 4),
        }
    }
}

pub struct TerminalSink {
    scale: TerminalScale,
    out: io::Stdout,
    /// Last (fg, bg) colours emitted, to skip redundant escape sequences.
    last_colors: Option<((u8, u8, u8), (u8, u8, u8))>,
}

impl TerminalSink {
    pub fn new(scale: TerminalScale) -> TerminalSink {
        TerminalSink { scale, out: io::stdout(), last_colors: None }
    }

    /// Renders a frame to a string (used by tests; no terminal state involved).
    pub fn render_to_string(&mut self, frame: &Frame) -> String {
        let mut out = String::new();
        let (cw, ch) = self.scale.cell_size();
        match self.scale {
            TerminalScale::Full => self.write_full(frame, &mut out, cw, ch),
            TerminalScale::Half => self.write_half(frame, &mut out, cw, ch),
            TerminalScale::Braille => self.write_braille(frame, &mut out, cw, ch),
        }
        self.last_colors = None;
        out
    }

    fn color_escape(&mut self, out: &mut String, fg: (u8, u8, u8), bg: (u8, u8, u8)) {
        if self.last_colors == Some((fg, bg)) {
            return;
        }
        out.push_str(&format!("\x1b[38;2;{};{};{};48;2;{};{};{}m", fg.0, fg.1, fg.2, bg.0, bg.1, bg.2));
        self.last_colors = Some((fg, bg));
    }

    fn write_full(&mut self, frame: &Frame, out: &mut String, _cw: u16, _ch: u16) {
        for y in 0..frame.height {
            for x in 0..frame.width {
                let color = frame.pixel(x, y);
                self.color_escape(out, color, color);
                out.push(' ');
            }
            self.color_escape(out, (0, 0, 0), (0, 0, 0));
            out.push_str("\x1b[0m\n");
            self.last_colors = None;
        }
    }

    fn write_half(&mut self, frame: &Frame, out: &mut String, _cw: u16, ch: u16) {
        for cell_y in 0..frame.height / ch {
            for cell_x in 0..frame.width {
                let top = frame.pixel(cell_x, cell_y * 2);
                let bottom = frame.pixel(cell_x, cell_y * 2 + 1);
                self.color_escape(out, top, bottom);
                out.push('\u{2580}');
            }
            self.color_escape(out, (0, 0, 0), (0, 0, 0));
            out.push_str("\x1b[0m\n");
            self.last_colors = None;
        }
    }

    fn write_braille(&mut self, frame: &Frame, out: &mut String, cw: u16, ch: u16) {
        // Braille dot bit layout within a 2x4 cell:
        //   (col, row): (0,0)=0x01 (0,1)=0x02 (0,2)=0x04 (1,0)=0x08
        //               (1,1)=0x10 (1,2)=0x20 (0,3)=0x40 (1,3)=0x80
        const DOT_BITS: [[u8; 4]; 2] = [
            [0x01, 0x02, 0x04, 0x40],
            [0x08, 0x10, 0x20, 0x80],
        ];
        for cell_y in 0..frame.height / ch {
            for cell_x in 0..frame.width / cw {
                let (bg_r, bg_g, bg_b) = frame.pixel(cell_x * 2, cell_y * 4);
                let mut dots = 0u8;
                let mut fg = (bg_r, bg_g, bg_b);
                for dy in 0..4u16 {
                    for dx in 0..2u16 {
                        let (r, g, b) = frame.pixel(cell_x * 2 + dx, cell_y * 4 + dy);
                        if (r, g, b) != (bg_r, bg_g, bg_b) {
                            dots |= DOT_BITS[dx as usize][dy as usize];
                            fg = (r, g, b);
                        }
                    }
                }
                self.color_escape(out, fg, (bg_r, bg_g, bg_b));
                out.push(char::from_u32(0x2800 + dots as u32).unwrap());
            }
            self.color_escape(out, (0, 0, 0), (0, 0, 0));
            out.push_str("\x1b[0m\n");
            self.last_colors = None;
        }
    }
}

impl VideoSink for TerminalSink {
    fn present(&mut self, frame: &Frame) {
        // Move cursor home and redraw; avoids scrollback flooding between frames.
        let mut buffer = String::from("\x1b[H\x1b[J");
        buffer.push_str(&self.render_to_string(frame));
        let _ = self.out.write_all(buffer.as_bytes());
        let _ = self.out.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::{TerminalScale, TerminalSink};
    use super::super::Frame;

    #[test]
    fn test_full_scale_dimensions() {
        let mut frame = Frame::new(4, 2);
        frame.set_pixel(0, 0, (255, 0, 0));
        let mut sink = TerminalSink::new(TerminalScale::Full);
        let text = sink.render_to_string(&frame);
        // 2 rows, 4 cells each
        assert_eq!(text.lines().count(), 2);
        assert!(text.contains("48;2;255;0;0"));
    }

    #[test]
    fn test_half_scale_dimensions() {
        let mut frame = Frame::new(4, 4);
        frame.set_pixel(0, 0, (10, 20, 30)); // top half of cell (0,0)
        frame.set_pixel(0, 1, (40, 50, 60)); // bottom half of cell (0,0)
        let mut sink = TerminalSink::new(TerminalScale::Half);
        let text = sink.render_to_string(&frame);
        assert_eq!(text.lines().count(), 2);
        assert!(text.contains("38;2;10;20;30;48;2;40;50;60"));
    }

    #[test]
    fn test_braille_dimensions_and_dots() {
        let mut frame = Frame::new(4, 4);
        frame.set_pixel(1, 0, (255, 255, 255)); // top-right dot of cell (0,0)
        frame.set_pixel(0, 3, (255, 255, 255)); // bottom-left dot of cell (0,0)
        let mut sink = TerminalSink::new(TerminalScale::Braille);
        let text = sink.render_to_string(&frame);
        assert_eq!(text.lines().count(), 1);
        // dots 0x08 (1,0) + 0x40 (0,3) = 0x48 -> U+2848
        assert!(text.contains('\u{2848}'), "braille glyph with correct dots expected");
    }

    #[test]
    fn test_scale_parsing() {
        assert_eq!(TerminalScale::from_str("full"), Some(TerminalScale::Full));
        assert_eq!(TerminalScale::from_str("half"), Some(TerminalScale::Half));
        assert_eq!(TerminalScale::from_str("braille"), Some(TerminalScale::Braille));
        assert_eq!(TerminalScale::from_str("nope"), None);
    }
}
