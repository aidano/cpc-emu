// MC6845 CRTC: register file plus live scanline counters. The CRTC counts
// 4 T-cycles per character, (R0+1) characters per scanline, (R9+1) scanlines
// per character row and (R4+1) rows per frame; the VSYNC output is asserted
// on character row R7 for the R3 high-nibble width in scanlines (0 => 16).
// The renderer reads the register file; the gate array consumes scanline
// edges for its 52-line interrupt counter.

/// T-cycles per CRTC character: the CPC character clock is 1MHz at a 4MHz CPU.
pub const CHAR_T: u32 = 4;

#[derive(Debug, Clone)]
pub struct Crtc {
    pub reg_select: u8,
    pub registers: [u8; 16],
    /// T-cycles elapsed within the current character cell.
    char_t: u32,
    /// Live CRTC counters: character column, raster line within the row,
    /// character row within the frame.
    char_col: u32,
    raster_line: u32,
    char_row: u32,
    /// VSYNC output state, read live by the PPI port B bit 0.
    pub vsync_active: bool,
    /// Scanlines remaining in the VSYNC pulse (the pulse spans row
    /// boundaries; the width can exceed the lines in a character row).
    vsync_lines_left: u32,
    /// Edge flag: VSYNC became active during the last advance. Consumed by
    /// the runtime (the gate array's 2-line-after-VSYNC rule keys off it).
    vsync_start_flag: bool,
}

impl Crtc {
    pub fn default() -> Crtc {
        Crtc {
            reg_select: 0,
            // Power-on screen base per the CPC firmware convention: R12 = 0x30
            // (video page bits 5-4 = 11 -> 0xC000-0xFFFF, word offset 0).
            registers: {
                let mut registers = [0x00; 16];
                registers[12] = 0x30;
                registers
            },
            char_t: 0,
            char_col: 0,
            raster_line: 0,
            char_row: 0,
            vsync_active: false,
            vsync_lines_left: 0,
            vsync_start_flag: false,
        }
    }

    fn chars_per_line(&self) -> u32 {
        (self.registers[0] as u32) + 1
    }

    fn lines_per_row(&self) -> u32 {
        (self.registers[9] as u32 & 0x1F) + 1
    }

    fn rows_per_frame(&self) -> u32 {
        (self.registers[4] as u32 & 0x7F) + 1
    }

    /// VSYNC pulse width in scanlines; a programmed 0 means 16 on the CRTC
    /// types used in the CPC.
    fn vsync_width(&self) -> u32 {
        let width = (self.registers[3] as u32) >> 4;
        if width == 0 { 16 } else { width }
    }

    /// T-cycles remaining until the next scanline boundary.
    pub fn t_to_line_end(&self) -> u32 {
        self.chars_per_line() * CHAR_T - self.char_t
    }

    /// Advances the CRTC timing by `cycles` T-cycles. Returns the number of
    /// scanline boundaries crossed (the gate array counts one HSYNC falling
    /// edge per scanline).
    pub fn advance(&mut self, cycles: u32) -> u32 {
        let htotal = self.chars_per_line();
        let mut remaining = cycles;
        let mut lines = 0u32;
        self.vsync_start_flag = false;
        while remaining > 0 {
            let step = remaining.min(htotal * CHAR_T - self.char_t);
            self.char_t += step;
            remaining -= step;
            if self.char_t >= htotal * CHAR_T {
                self.char_t -= htotal * CHAR_T;
                self.char_col = 0;
                lines += 1;
                self.raster_line += 1;
                if self.raster_line >= self.lines_per_row() {
                    self.raster_line = 0;
                    self.char_row += 1;
                    if self.char_row >= self.rows_per_frame() {
                        self.char_row = 0;
                    }
                }
                // VSYNC starts when the vertical counter arrives at row R7
                // and lasts `width` scanlines across row boundaries.
                let row_start = self.raster_line == 0;
                if row_start && self.char_row == self.registers[7] as u32 {
                    self.vsync_lines_left = self.vsync_width();
                }
                let was_active = self.vsync_active;
                self.vsync_active = self.vsync_lines_left > 0;
                if self.vsync_active {
                    self.vsync_lines_left -= 1;
                }
                if self.vsync_active && !was_active {
                    self.vsync_start_flag = true;
                }
            }
        }
        lines
    }

    /// Consumes the VSYNC-start edge (true exactly once per frame).
    pub fn take_vsync_start(&mut self) -> bool {
        let flag = self.vsync_start_flag;
        self.vsync_start_flag = false;
        flag
    }

    /// The video origin, split the way the hardware wires it: the 16K video
    /// page (A15-A14, from R12 bits 5-4) and the word offset within it
    /// (A10-A1, from R12 bits 1-0 + R13).
    ///
    /// Per the CPC video address mapping: A15 = MA13, A14 = MA12, A13-A11 =
    /// the raster line (RA2-RA0), A10-A1 = MA9-MA0, A0 = the byte within
    /// the word (driven by the gate array). MA10/MA11 (R12 bits 3-2) are
    /// not connected on the CPC, so the word offset wraps at 1024 words
    /// (2KB per raster block) instead of addressing further memory.
    pub fn video_origin(&self) -> (u16, u16) {
        let page = (((self.registers[12] as u16) >> 4) & 0x03) << 14;
        let offset = (((self.registers[12] as u16) & 0x03) << 8) | self.registers[13] as u16;
        (page, offset)
    }

    /// The visible screen start address, from registers 12/13.
    pub fn start_address(&self) -> u16 {
        let (page, offset) = self.video_origin();
        page | (offset << 1)
    }

    /// Handles a write to the CRTC data port (0xBDxx).
    pub fn write_register(&mut self, value: u8) {
        if self.reg_select < 16 {
            let masked = match self.reg_select {
                4 | 6 | 7 => value & 0x7F,
                5 | 9 | 11 => value & 0x1F,
                10 => value & 0x7F,
                12 => value & 0x3F,
                _ => value,
            };
            self.registers[self.reg_select as usize] = masked;
        }
    }

    /// Handles an IN from the CRTC status port (0xBFxx): registers 12-17 are
    /// readable, everything else returns 0.
    pub fn read_register(&self) -> u8 {
        if (12..=17).contains(&self.reg_select) {
            if self.reg_select < 16 {
                return self.registers[self.reg_select as usize];
            }
            return 0; // registers 16/17 are light-pen; not modelled
        }
        0
    }
}

#[cfg(test)]
mod tests {
    use super::{Crtc, CHAR_T};

    fn programmed_crtc() -> Crtc {
        // The 6128 boot's values: R0=63 (64 chars/line), R9=7 (8 lines/row),
        // R4=38 (39 rows = 312 lines), R7=30 (vsync row), R3=0x8E (vsync
        // width 8 lines).
        let mut crtc = Crtc::default();
        crtc.registers[0] = 63;
        crtc.registers[3] = 0x8E;
        crtc.registers[4] = 38;
        crtc.registers[7] = 30;
        crtc.registers[9] = 7;
        crtc
    }

    #[test]
    fn test_register_masks() {
        let mut crtc = Crtc::default();
        crtc.reg_select = 12;
        crtc.write_register(0xFF); // masked to 0x3F
        assert_eq!(crtc.registers[12], 0x3F);
        crtc.reg_select = 9;
        crtc.write_register(0xFF); // masked to 0x1F
        assert_eq!(crtc.registers[9], 0x1F);
        crtc.reg_select = 13;
        crtc.write_register(0xAB); // unmasked
        assert_eq!(crtc.registers[13], 0xAB);
    }

    #[test]
    fn test_start_address() {
        let mut crtc = Crtc::default();
        // Firmware default R12 = 0x30: video page 3 (0xC000), offset 0.
        assert_eq!(crtc.start_address(), 0xC000);
        // BASIC overscan example: R12 = 0x30 | page, R13 = word offset.
        // Word offset 0x128 -> byte offset 0x250 (A0 is always 0).
        crtc.reg_select = 12;
        crtc.write_register(0x31);
        crtc.reg_select = 13;
        crtc.write_register(0x28);
        assert_eq!(crtc.start_address(), 0xC250);
        // MA10/MA11 (R12 bits 3-2) are not connected and must not leak in.
        crtc.reg_select = 12;
        crtc.write_register(0x3C);
        crtc.reg_select = 13;
        crtc.write_register(0x00);
        assert_eq!(crtc.start_address(), 0xC000);
    }

    #[test]
    fn test_read_range() {
        let mut crtc = Crtc::default();
        crtc.registers[13] = 0x42;
        crtc.reg_select = 13;
        assert_eq!(crtc.read_register(), 0x42);
        crtc.reg_select = 3; // write-only
        assert_eq!(crtc.read_register(), 0);
        crtc.reg_select = 17; // readable per spec, not modelled
        assert_eq!(crtc.read_register(), 0);
    }

    #[test]
    fn test_line_and_frame_cadence() {
        let mut crtc = programmed_crtc();
        // One scanline = 64 chars x 4 T-cycles = 256.
        assert_eq!(crtc.advance(255), 0);
        assert_eq!(crtc.advance(1), 1);
        // A full programmed frame is 312 scanlines.
        let mut lines = 0;
        for _ in 0..312 {
            lines += crtc.advance(256);
        }
        assert_eq!(lines, 312);
    }

    #[test]
    fn test_vsync_window() {
        let mut crtc = programmed_crtc();
        // VSYNC row 30, 8 lines per row: the pulse spans lines 240-247.
        for _ in 0..239 {
            crtc.advance(256);
        }
        assert!(!crtc.vsync_active);
        crtc.advance(256);
        assert!(crtc.vsync_active, "vsync must start on row 30 (line 240)");
        assert!(crtc.take_vsync_start());
        assert!(!crtc.take_vsync_start(), "edge must be consumed once");
        for _ in 0..7 {
            crtc.advance(256);
            assert!(crtc.vsync_active, "pulse width is 8 scanlines");
        }
        crtc.advance(256);
        assert!(!crtc.vsync_active, "line 248 is past the 8-line pulse");
    }

    #[test]
    fn test_vsync_width_zero_means_sixteen() {
        let mut crtc = programmed_crtc();
        crtc.registers[3] = 0x0E; // vsync width nibble 0 => 16 lines
        for _ in 0..239 {
            crtc.advance(256);
        }
        crtc.advance(256);
        assert!(crtc.vsync_active);
        for _ in 0..15 {
            crtc.advance(256);
            assert!(crtc.vsync_active);
        }
        crtc.advance(256);
        assert!(!crtc.vsync_active);
    }

    #[test]
    fn test_char_t_to_line_end() {
        let crtc = programmed_crtc();
        assert_eq!(crtc.t_to_line_end(), 64 * CHAR_T);
    }
}