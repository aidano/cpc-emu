// Renders CPC video memory into a Frame, honouring the gate array mode and palette.
//
// With the default base 0xC000 the screen memory layout is:
//   address = base + (y / 8) * stride + (y % 8) * 0x800 + (x / pixels_per_byte)
// Consecutive scanlines within a character row are 0x800 bytes apart.
//
// Addresses are built the way the hardware wires them (see
// Crtc::video_origin): A15-A14 select the 16K video page, A13-A11 the raster
// line within the character row, A10-A1 a 10-bit word offset and A0 the byte
// within the word. MA10/MA11 are not connected, so the word offset wraps at
// 1024 words: a hardware-scrolled screen (the firmware scrolls text by
// stepping R12/R13) wraps within each 2KB raster block rather than running
// past the end of the address space.

use crate::memory::Memory;
use super::{Frame, GateArray};
use crate::devices::crtc::Crtc;

const HEIGHT: u16 = 200;

fn mode_dimensions(mode: u8) -> (u16, u16, u16) {
    // (width, height, pixels per byte)
    match mode {
        0 => (160, HEIGHT, 2),
        1 => (320, HEIGHT, 4),
        _ => (640, HEIGHT, 8),
    }
}

fn mode_stride(mode: u8) -> u16 {
    match mode {
        0 => 0x50, // 80 bytes: 2 pixels/byte
        1 => 0x50, // 80 bytes: 4 pixels/byte
        _ => 0x50, // 80 bytes: 8 pixels/byte
    }
}

/// Pen colour lookup: in modes 0/1 the pixel bits index pens directly.
/// In mode 2 only pens 0 (paper) and 1 (ink) are visible.
fn pen_color(ga: &GateArray, pen: usize) -> (u8, u8, u8) {
    super::hardware_color(ga.pens[pen])
}

/// Renders one frame into `out`, reusing its buffer. On a mode change the
/// buffer is reallocated to the mode's dimensions; otherwise the call is
/// allocation-free, which keeps the emulation thread's per-frame cost small
/// and predictable.
pub fn render_into(mem: &Memory, ga: &GateArray, crtc: &Crtc, out: &mut Frame) {
    let (width, height, pixels_per_byte) = mode_dimensions(ga.mode);
    // One character row of screen data is 80 bytes = 40 CRTC words in every
    // mode; consecutive scanlines of the row sit 0x800 apart (raster bits).
    let row_words = mode_stride(ga.mode) / 2;
    let (page, offset) = crtc.video_origin();
    if out.width != width || out.height != height {
        *out = Frame::new(width, height);
    }

    for y in 0..height {
        let row = y / 8;
        let scanline = y % 8;
        let row_word = (offset + row * row_words) & 0x3FF;
        let raster_base = page | (scanline << 11);
        for x in 0..width {
            let byte_index = x / pixels_per_byte;
            let word = (row_word + byte_index / 2) & 0x3FF;
            let byte = mem.read(raster_base | (word << 1) | (byte_index & 1));
            let color = match ga.mode {
                0 => {
                    // 4 bits per pixel, scrambled (see the Gate Array packing
                    // table): pixel 0 = bits 7,5,3,1 with pen weights 1,4,2,8;
                    // pixel 1 = bits 6,4,2,0 with the same weights.
                    let in_pixel = (x % 2) as u8;
                    let pen = if in_pixel == 0 {
                        ((byte >> 7) & 1) | (((byte >> 5) & 1) << 2) | (((byte >> 3) & 1) << 1) | (((byte >> 1) & 1) << 3)
                    } else {
                        ((byte >> 6) & 1) | (((byte >> 4) & 1) << 2) | (((byte >> 2) & 1) << 1) | ((byte & 1) << 3)
                    };
                    pen_color(ga, pen as usize)
                }
                1 => {
                    // 2 bits per pixel, scrambled: pixel n takes bit (3-n) as
                    // pen bit 1 (weight 2) and bit (7-n) as pen bit 0 (weight 1).
                    let in_pixel = (x % 4) as u8;
                    let pen = (((byte >> (3 - in_pixel)) & 1) << 1) | ((byte >> (7 - in_pixel)) & 1);
                    pen_color(ga, pen as usize)
                }
                _ => {
                    // 1 bit per pixel, MSB first: pen 0 = paper, pen 1 = ink
                    let in_pixel = (x % 8) as u8;
                    let pen = (byte >> (7 - in_pixel)) & 1;
                    pen_color(ga, pen as usize)
                }
            };
            out.set_pixel(x, y, color);
        }
    }
}

/// Renders one frame, allocating a fresh Frame for it. Prefer `render_into`
/// in hot loops.
pub fn render(mem: &Memory, ga: &GateArray, crtc: &Crtc) -> Frame {
    let mut frame = Frame::new(1, 1);
    render_into(mem, ga, crtc, &mut frame);
    frame
}

#[cfg(test)]
mod tests {
    use super::render;
    use crate::memory::Memory;
    use super::super::{GateArray};
    use crate::devices::crtc::Crtc;

    fn setup() -> (Memory, GateArray, Crtc) {
        let (mut mem, mut ga) = (Memory::default(), GateArray::default());
        ga.pens[0] = 0x14; // true black paper by default in tests (hardware colour 0x00 is grey)
        (mem, ga, Crtc::default())
    }

    #[test]
    fn test_mode2_ink_spread_over_scrambled_rows() {
        let (mut mem, mut ga, crtc) = setup();
        ga.mode = 2;
        ga.pens[0] = 0x14; // black paper
        ga.pens[1] = 0x12; // bright green ink
        // Set every byte of scanline 0 of character row 0 (y = 0)
        for x_byte in 0..0x50 {
            mem.write(0xC000 + x_byte, 0xFF);
        }
        // Scanline 1 (y = 1) lives 0x800 bytes further
        for x_byte in 0..0x50 {
            mem.write(0xC000 + 0x800 + x_byte, 0xFF);
        }

        let frame = render(&mem, &ga, &crtc);
        assert_eq!((frame.width, frame.height), (640, 200));
        // Top line and second line fully ink
        assert_eq!(frame.pixel(0, 0), (0x00, 0xFF, 0x00));
        assert_eq!(frame.pixel(639, 1), (0x00, 0xFF, 0x00));
        // Third scanline untouched -> paper
        assert_eq!(frame.pixel(0, 2), (0x00, 0x00, 0x00));
        // Second character row (y = 8) starts at 0xC000 + 0x50
        for x_byte in 0..0x50 {
            mem.write(0xC000 + 0x50 + x_byte, 0xFF);
        }
        let frame = render(&mem, &ga, &crtc);
        assert_eq!(frame.pixel(100, 8), (0x00, 0xFF, 0x00));
        assert_eq!(frame.pixel(100, 9), (0x00, 0x00, 0x00));
    }

    #[test]
    fn test_mode2_bit_order() {
        let (mut mem, mut ga, crtc) = setup();
        ga.mode = 2;
        ga.pens[1] = 0x12;
        // Byte 0 of scanline 0: single bit set for pixel 3 (bit 4)
        mem.write(0xC000, 0b0001_0000);
        let frame = render(&mem, &ga, &crtc);
        assert_eq!(frame.pixel(2, 0), (0x00, 0x00, 0x00));
        assert_eq!(frame.pixel(3, 0), (0x00, 0xFF, 0x00));
        assert_eq!(frame.pixel(4, 0), (0x00, 0x00, 0x00));
    }

    #[test]
    fn test_mode1_pens() {
        let (mut mem, mut ga, crtc) = setup();
        ga.mode = 1;
        ga.pens[0] = 0x14; // black
        ga.pens[1] = 0x04; // royal blue
        ga.pens[2] = 0x12; // bright green
        ga.pens[3] = 0x0C; // bright red
        // Byte: pixel0 = bits (3,7) = (1,0) -> pen 2; pixel1 = (2,6) = (0,1) -> pen 1;
        //       pixel2 = (1,5) = (1,1) -> pen 3; pixel3 = (0,4) = (0,0) -> pen 0
        mem.write(0xC000, 0b0110_1010);
        let frame = render(&mem, &ga, &crtc);
        assert_eq!(frame.pixel(0, 0), (0x00, 255, 0x00)); // pen 2 bright green
        assert_eq!(frame.pixel(1, 0), (0x00, 0x00, 127)); // pen 1 blue
        assert_eq!(frame.pixel(2, 0), (255, 0x00, 0x00)); // pen 3 bright red
        assert_eq!(frame.pixel(3, 0), (0x00, 0x00, 0x00)); // pen 0 black
    }

    #[test]
    fn test_mode0_pens() {
        let (mut mem, mut ga, crtc) = setup();
        ga.mode = 0;
        ga.pens[9] = 0x0C; // bright red
        ga.pens[6] = 0x12; // bright green
        // Byte: pixel0 = bits (7,5,3,1) = (1,0,0,1) -> 1+8 = pen 9; pixel1 = bits (6,4,2,0) = (0,1,1,0) -> 4+2 = pen 6
        mem.write(0xC000, 0b1001_0110);
        let frame = render(&mem, &ga, &crtc);
        assert_eq!((frame.width, frame.height), (160, 200));
        assert_eq!(frame.pixel(0, 0), (255, 0x00, 0x00)); // pen 9
        assert_eq!(frame.pixel(1, 0), (0x00, 0xFF, 0x00)); // pen 6
    }

    #[test]
    fn test_scrolled_screen_renders_from_offset() {
        // One firmware text scroll steps the base by one character row:
        // R12/R13 = 0x30/0x28 -> word offset 0x28 -> byte base 0xC050.
        let (mut mem, mut ga, mut crtc) = setup();
        ga.mode = 2;
        ga.pens[1] = 0x12;
        crtc.registers[12] = 0x30;
        crtc.registers[13] = 0x28;
        for x_byte in 0..0x50 {
            mem.write(0xC050 + x_byte, 0xFF);
        }
        let frame = render(&mem, &ga, &crtc);
        assert_eq!(frame.pixel(0, 0), (0x00, 0xFF, 0x00));
        // Row 1 renders from the scrolled base 0xC0A0, not from 0xC000.
        assert_eq!(frame.pixel(0, 8), (0x00, 0x00, 0x00));
    }

    #[test]
    fn test_word_offset_wraps_within_2k_blocks() {
        // R12/R13 = 0x33/0xE0 -> word offset 0x3E0: row 0 starts 0x7C0 bytes
        // in and its last 16 bytes wrap to the start of the 2KB raster block.
        // (Under the old byte-arithmetic this base overflowed u16 on the
        // lower rows.)
        let (mut mem, mut ga, mut crtc) = setup();
        ga.mode = 2;
        ga.pens[1] = 0x12;
        crtc.registers[12] = 0x33;
        crtc.registers[13] = 0xE0;
        // Words 0x3E0..0x3FF of row 0 live at 0xC7C0-0xC7FF; zero them so
        // only the wrapped part is ink.
        for x_byte in 0..0x40 {
            mem.write(0xC7C0 + x_byte, 0x00);
        }
        // Word 0x400 wraps to word 0: bytes 64.. of row 0 come from 0xC000.
        for x_byte in 0..0x10 {
            mem.write(0xC000 + x_byte, 0xFF);
        }
        let frame = render(&mem, &ga, &crtc);
        assert_eq!(frame.pixel(496, 0), (0x00, 0x00, 0x00)); // word 0x3FF: not yet wrapped
        assert_eq!(frame.pixel(512, 0), (0x00, 0xFF, 0x00)); // word 0x400 -> 0xC000
        // Row 1's word base 0x408 wraps too: its first bytes come from 0xC010.
        mem.write(0xC010, 0xFF);
        let frame = render(&mem, &ga, &crtc);
        assert_eq!(frame.pixel(0, 8), (0x00, 0xFF, 0x00));
    }

    #[test]
    fn test_pen_changes_reflect_in_render() {
        let (mut mem, mut ga, crtc) = setup();
        ga.mode = 2;
        for x_byte in 0..0x50 {
            mem.write(0xC000 + x_byte, 0xFF);
        }
        ga.pens[1] = 0x04; // royal blue ink
        let frame = render(&mem, &ga, &crtc);
        assert_eq!(frame.pixel(0, 0), (0x00, 0x00, 127));
    }
}
