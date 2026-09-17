// Renders CPC video memory into a Frame, honouring the gate array mode and palette.
//
// The display window and the memory layout both come from the CRTC: R1
// characters per scanline, R6 character rows of R9+1 scanlines. Each
// character row of screen data spans R1 words (2·R1 bytes) — the MA counter
// only advances during displayed characters and is captured for the next
// char-line when the raster counter wraps — so with the firmware's R1 = 40
// the layout is:
//   address = base + (y / 8) * 0x50 + (y % 8) * 0x800 + (x / pixels_per_byte)
// Consecutive scanlines within a character row are 0x800 bytes apart.
// Narrow-screen games program R1 = 32 (plus their own R6/R7) and lay their
// screens out on the resulting 64-byte stride.
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

fn mode_pixels_per_byte(mode: u8) -> u16 {
    match mode {
        0 => 2,
        1 => 4,
        _ => 8,
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
///
/// Pixels are fetched through `Memory::read_video_ram`: like the real gate
/// array, the video fetch must bypass the ROM overlay and RAM paging, or
/// every ROM/bank reconfiguration flashes garbage over the screen.
pub fn render_into(mem: &Memory, ga: &GateArray, crtc: &Crtc, out: &mut Frame) {
    let pixels_per_byte = mode_pixels_per_byte(ga.mode);
    // The CRTC defines the visible window: R1 characters per scanline and R6
    // character rows of R9+1 scanlines each. A character cell is 2 bytes
    // wide, so the frame is 2·pixels_per_byte pixels per character.
    let chars = crtc.display_chars().max(1) as u16;
    let rows = crtc.display_rows().max(1) as u16;
    let lines_per_row = crtc.lines_per_row() as u16;
    let width = chars * 2 * pixels_per_byte;
    let height = rows * lines_per_row;
    let (page, offset) = crtc.video_origin();
    if out.width != width || out.height != height {
        *out = Frame::new(width, height);
    }

    for y in 0..height {
        let row = y / lines_per_row;
        let scanline = y % lines_per_row;
        let row_word = (offset + row * chars) & 0x3FF;
        let raster_base = page | ((scanline & 7) << 11);
        for x in 0..width {
            let byte_index = x / pixels_per_byte;
            let word = (row_word + byte_index / 2) & 0x3FF;
            let byte = mem.read_video_ram(raster_base | (word << 1) | (byte_index & 1));
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
    fn test_narrow_window_uses_r1_stride() {
        // R1 = 32: the frame is 128px wide in mode 0 (4px per char) and
        // character rows sit 0x40 bytes apart. Cybernoid II and Into the
        // Eagle's Nest lay their screens out on this stride.
        let (mut mem, mut ga, mut crtc) = setup();
        ga.mode = 0;
        ga.pens[15] = 0x12; // bright green ink
        crtc.registers[1] = 32;
        // Row 3, scanline 0 lives at 0xC000 + 3*0x40.
        for x_byte in 0..0x40 {
            mem.write(0xC000 + 3 * 0x40 + x_byte, 0xFF);
        }
        let frame = render(&mem, &ga, &crtc);
        assert_eq!((frame.width, frame.height), (128, 200));
        assert_eq!(frame.pixel(0, 24), (0x00, 0xFF, 0x00));
        assert_eq!(frame.pixel(127, 24), (0x00, 0xFF, 0x00));
        // Scanline 1 of row 3 comes from 0x800 further on.
        assert_eq!(frame.pixel(0, 25), (0x00, 0x00, 0x00));
        // The next char row starts one 0x40 stride later, not 0x50.
        for x_byte in 0..0x40 {
            mem.write(0xC000 + 4 * 0x40 + x_byte, 0xFF);
        }
        let frame = render(&mem, &ga, &crtc);
        assert_eq!(frame.pixel(0, 32), (0x00, 0xFF, 0x00));
    }

    #[test]
    fn test_r6_limits_frame_height() {
        // R6 = 24 displays 24 character rows: 192 scanlines (Cybernoid II's
        // window). A 25th row is never sampled.
        let (mem, ga, mut crtc) = setup();
        crtc.registers[6] = 24;
        let frame = render(&mem, &ga, &crtc);
        assert_eq!((frame.width, frame.height), (640, 192));
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

    #[test]
    fn test_render_ignores_upper_rom_overlay() {
        // A ROM paged in at &C000 overlays CPU reads only: the gate array's
        // video fetch reads the RAM underneath (the firmware screen displays
        // while the BASIC ROM is paged in). Rendering the ROM instead drew
        // garbage over the screen on every ROM reconfiguration.
        let (mut mem, mut ga, crtc) = setup();
        ga.mode = 2;
        ga.pens[1] = 0x12;
        let rom = [0x00u8; 0x4000];
        mem.load_expansion_rom_into_slot(&rom, 0); // also enables upper ROM
        for x_byte in 0..0x50 {
            mem.write(0xC000 + x_byte, 0xFF);
        }
        let frame = render(&mem, &ga, &crtc);
        assert_eq!(frame.pixel(0, 0), (0x00, 0xFF, 0x00), "screen RAM must render, not ROM bytes");
    }

    #[test]
    fn test_render_ignores_ram_config_paging() {
        // The RAM configuration pages an expansion bank over &C000, but VRAM
        // always lives in the base 64K: the screen stays visible while the
        // CPU sees the paged bank (128K games page banks for music/streaming).
        let (mut mem, mut ga, crtc) = setup();
        ga.mode = 2;
        ga.pens[1] = 0x12;
        for x_byte in 0..0x50 {
            mem.write(0xC000 + x_byte, 0xFF);
        }
        // Config 2 maps all four regions to the expansion bank; &C000 now
        // exposes block 7, which holds only the 0x01 fill.
        mem.set_ram_config(0xC2);
        let frame = render(&mem, &ga, &crtc);
        assert_eq!(frame.pixel(0, 0), (0x00, 0xFF, 0x00), "screen RAM must render, not the paged bank");
    }
}
