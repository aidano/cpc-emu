// Renders CPC video memory into a Frame, honouring the gate array mode and palette.
//
// Screen memory layout (base 0xC000):
//   address = base + (y / 8) * stride + (y % 8) * 0x800 + (x / pixels_per_byte)
// Consecutive scanlines within a character row are 0x800 bytes apart.

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

/// The visible screen start address, from the CRTC start address registers.
fn screen_base(crtc: &Crtc) -> u16 {
    crtc.start_address()
}

/// Pen colour lookup: in modes 0/1 the pixel bits index pens directly.
/// In mode 2 only pens 0 (paper) and 1 (ink) are visible.
fn pen_color(ga: &GateArray, pen: usize) -> (u8, u8, u8) {
    super::hardware_color(ga.pens[pen])
}

pub fn render(mem: &Memory, ga: &GateArray, crtc: &Crtc) -> Frame {
    let (width, height, pixels_per_byte) = mode_dimensions(ga.mode);
    let stride = mode_stride(ga.mode);
    let base = screen_base(crtc);
    let mut frame = Frame::new(width, height);

    for y in 0..height {
        let row_base = base
            + ((y / 8) as u16) * stride
            + ((y % 8) as u16) * 0x800;
        for x in 0..width {
            let byte = mem.read(row_base + x / pixels_per_byte);
            let color = match ga.mode {
                0 => {
                    // 4 bits per pixel, scrambled: pixel 0 = bits 7,6,3,2; pixel 1 = bits 5,4,1,0
                    let in_pixel = (x % 2) as u8;
                    let pen = if in_pixel == 0 {
                        ((byte >> 7) & 1) << 3 | ((byte >> 6) & 1) << 2 | ((byte >> 3) & 1) << 1 | ((byte >> 2) & 1)
                    } else {
                        ((byte >> 5) & 1) << 3 | ((byte >> 4) & 1) << 2 | ((byte >> 1) & 1) << 1 | (byte & 1)
                    };
                    pen_color(ga, pen as usize)
                }
                1 => {
                    // 2 bits per pixel, scrambled: pixel n uses bits (7-n) and (3-n)
                    let in_pixel = (x % 4) as u8;
                    let pen = (((byte >> (7 - in_pixel)) & 1) << 1) | ((byte >> (3 - in_pixel)) & 1);
                    pen_color(ga, pen as usize)
                }
                _ => {
                    // 1 bit per pixel, MSB first: pen 0 = paper, pen 1 = ink
                    let in_pixel = (x % 8) as u8;
                    let pen = (byte >> (7 - in_pixel)) & 1;
                    pen_color(ga, pen as usize)
                }
            };
            frame.set_pixel(x, y, color);
        }
    }

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
        // Byte: pixel0 = bits (7,3) = (0,1) -> pen 1; pixel1 = (6,2) = (1,0) -> pen 2;
        //       pixel2 = (5,1) = (1,1) -> pen 3; pixel3 = (4,0) = (0,0) -> pen 0
        mem.write(0xC000, 0b0110_1010);
        let frame = render(&mem, &ga, &crtc);
        assert_eq!(frame.pixel(0, 0), (0x00, 0x00, 127)); // pen 1 blue
        assert_eq!(frame.pixel(1, 0), (0x00, 255, 0x00)); // pen 2 bright green
        assert_eq!(frame.pixel(2, 0), (255, 0x00, 0x00)); // pen 3 bright red
        assert_eq!(frame.pixel(3, 0), (0x00, 0x00, 0x00)); // pen 0 black
    }

    #[test]
    fn test_mode0_pens() {
        let (mut mem, mut ga, crtc) = setup();
        ga.mode = 0;
        ga.pens[9] = 0x0C; // bright red
        ga.pens[6] = 0x12; // bright green
        // Byte: pixel0 = bits 7,6,3,2 = (1,0,0,1) -> pen 9; pixel1 = bits 5,4,1,0 = (0,1,1,0) -> pen 6
        mem.write(0xC000, 0b1001_0110);
        let frame = render(&mem, &ga, &crtc);
        assert_eq!((frame.width, frame.height), (160, 200));
        assert_eq!(frame.pixel(0, 0), (255, 0x00, 0x00)); // pen 9
        assert_eq!(frame.pixel(1, 0), (0x00, 0xFF, 0x00)); // pen 6
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
