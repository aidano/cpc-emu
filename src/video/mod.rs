// Video core: renders CPC screen memory + gate array state into RGBA frames,
// consumed by pluggable display backends (terminal, Slint window, ...).

pub mod gate_array;
pub mod renderer;
pub mod terminal;
#[cfg(feature = "gui")]
pub mod slint_backend;

pub use gate_array::GateArray;

/// One rendered display frame: width x height RGBA8 pixels.
#[derive(Debug, Clone)]
pub struct Frame {
    pub width: u16,
    pub height: u16,
    pub rgba: Vec<u8>,
}

impl Frame {
    pub fn new(width: u16, height: u16) -> Frame {
        Frame {
            width,
            height,
            rgba: vec![0; (width as usize) * (height as usize) * 4],
        }
    }

    pub fn set_pixel(&mut self, x: u16, y: u16, (r, g, b): (u8, u8, u8)) {
        let index = ((y as usize) * (self.width as usize) + (x as usize)) * 4;
        self.rgba[index] = r;
        self.rgba[index + 1] = g;
        self.rgba[index + 2] = b;
        self.rgba[index + 3] = 0xFF;
    }

    pub fn pixel(&self, x: u16, y: u16) -> (u8, u8, u8) {
        let index = ((y as usize) * (self.width as usize) + (x as usize)) * 4;
        (self.rgba[index], self.rgba[index + 1], self.rgba[index + 2])
    }
}

/// Anything that can display a frame.
pub trait VideoSink {
    fn present(&mut self, frame: &Frame);
}

/// The authentic Amstrad CPC hardware palette: the 5-bit gate array colour
/// byte indexes directly into this table. Each channel has three levels
/// (0 / 0.5 / 1.0); note hardware colour 0 is grey, not black — true black
/// is index 0x14. Table from the standard emulator palette (Caprice32).
const HARDWARE_PALETTE: [(u8, u8, u8); 32] = [
    (127, 127, 127), (127, 127, 127), (0, 255, 127),   (255, 255, 127), // 0-3
    (0, 0, 127),     (255, 0, 127),   (0, 127, 127),   (255, 127, 127), // 4-7
    (255, 0, 127),   (255, 255, 127), (255, 255, 0),   (255, 255, 255), // 8-11
    (255, 0, 0),     (255, 0, 255),   (255, 127, 0),   (255, 127, 255), // 12-15
    (0, 0, 127),     (0, 255, 127),   (0, 255, 0),     (0, 255, 255),   // 16-19
    (0, 0, 0),       (0, 0, 255),     (0, 127, 0),     (0, 127, 255),   // 20-23
    (127, 0, 127),   (127, 255, 127), (127, 255, 0),   (127, 255, 255), // 24-27
    (127, 0, 0),     (127, 0, 255),   (127, 127, 0),   (127, 127, 255), // 28-31
];

/// Converts a gate array colour byte (0-31) to RGB via the hardware palette.
pub fn hardware_color(value: u8) -> (u8, u8, u8) {
    HARDWARE_PALETTE[(value & 0x1F) as usize]
}

#[cfg(test)]
mod tests {
    use super::{hardware_color, Frame};

    #[test]
    fn test_hardware_color_table() {
        assert_eq!(hardware_color(0x00), (127, 127, 127)); // grey (hardware colour 0 is NOT black)
        assert_eq!(hardware_color(0x01), (127, 127, 127)); // grey duplicate
        assert_eq!(hardware_color(0x04), (0, 0, 127));     // royal blue (firmware boot background)
        assert_eq!(hardware_color(0x0A), (255, 255, 0));   // yellow (firmware boot text)
        assert_eq!(hardware_color(0x0B), (255, 255, 255)); // white
        assert_eq!(hardware_color(0x0C), (255, 0, 0));     // bright red
        assert_eq!(hardware_color(0x12), (0, 255, 0));     // bright green
        assert_eq!(hardware_color(0x14), (0, 0, 0));       // true black
        assert_eq!(hardware_color(0x15), (0, 0, 255));     // bright blue
        assert_eq!(hardware_color(0x1F), (127, 127, 255)); // pastel blue
        // values above 31 mask to 5 bits
        assert_eq!(hardware_color(0x2A), hardware_color(0x0A));
    }

    #[test]
    fn test_frame_pixels() {
        let mut frame = Frame::new(4, 3);
        frame.set_pixel(3, 2, (0x12, 0x34, 0x56));
        assert_eq!(frame.pixel(3, 2), (0x12, 0x34, 0x56));
        assert_eq!(frame.pixel(0, 0), (0, 0, 0));
        assert_eq!(frame.rgba.len(), 4 * 3 * 4);
    }
}
