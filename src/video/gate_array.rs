// Gate Array state: screen mode, palette pens, border colour, ROM banking config.
// Written via I/O port 0x7Fxx (selected when address bits 15-14 = 01).

pub const NUM_PENS: usize = 16;
pub const BORDER_PEN: usize = 16; // pseudo-pen target selected by register 0x50-0x5F

#[derive(Debug, Clone)]
pub struct GateArray {
    pub mode: u8,
    pub selected_pen: usize,
    pub pens: [u8; NUM_PENS],
    pub border: u8,
    pub lower_rom_enabled: bool,
    pub upper_rom_enabled: bool,
    /// Set when the firmware writes the ROM-config register with bit 4 set
    /// ("delay Z80 interrupt"): the frame interrupt counter must resync.
    pub pending_interrupt_resync: bool,
    /// The gate array's INT line: asserted when the 52-scanline counter
    /// reaches 52, held until the CPU acknowledges the interrupt. A boundary
    /// crossing while interrupts are disabled must latch (the real INT line
    /// stays asserted), never burst — back-to-back re-entrant tick handlers
    /// corrupt the firmware's config-mirror state during boot.
    pub irq_pending: bool,
    /// Lines remaining before the post-VSYNC alignment check: per the gate
    /// array, a VSYNC is followed by a 2-HSYNC wait, then the interrupt
    /// counter is reset (and an interrupt issued only if the counter is
    /// below 32).
    pub vsync_wait: u32,
}

impl GateArray {
    pub fn default() -> GateArray {
        GateArray {
            mode: 2, // hardware reset state is mode 2 per the service manual
            selected_pen: 0,
            pens: [0x00; NUM_PENS], // grey until the firmware configures pens
            border: 0x00,
            lower_rom_enabled: false,
            upper_rom_enabled: false,
            pending_interrupt_resync: false,
            irq_pending: false,
            vsync_wait: 0,
        }
    }

    /// Handles a byte written to the gate array port.
    /// Register select is bits 7-6 of the value:
    ///   00: select pen (bits 0-3; bit 4 selects the border as colour target)
    ///   01: set colour (bits 0-4, hardware value) of the selected pen/border
    ///   10: memory config: bits 0-1 screen mode, bit 2 lower ROM off, bit 3 upper ROM off
    ///   11: unused
    pub fn write(&mut self, value: u8) {
        match value & 0xC0 {
            0x00 => {
                self.selected_pen = if value & 0x10 != 0 {
                    BORDER_PEN
                } else {
                    (value & 0x0F) as usize
                };
            }
            0x40 => {
                let color = value & 0x1F;
                if self.selected_pen == BORDER_PEN {
                    self.border = color;
                } else {
                    self.pens[self.selected_pen] = color;
                }
            }
            0x80 => {
                self.mode = value & 0x03;
                self.lower_rom_enabled = value & 0x04 == 0;
                self.upper_rom_enabled = value & 0x08 == 0;
                // Bit 4 requests an interrupt resync: the frame interrupt
                // counter restarts so the 300Hz tick aligns with the firmware.
                self.pending_interrupt_resync = value & 0x10 != 0;
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::GateArray;

    #[test]
    fn test_pen_select_and_color() {
        let mut ga = GateArray::default();
        ga.write(0x03); // select pen 3
        ga.write(0x41); // pen 3 = hardware colour 1 (blue)
        assert_eq!(ga.pens[3], 0x01);
    }

    #[test]
    fn test_border_via_pseudo_pen() {
        let mut ga = GateArray::default();
        ga.write(0x10); // select border (the classic 0x7F10 idiom)
        ga.write(0x4A); // border = hardware colour 10
        assert_eq!(ga.border, 0x0A);
        ga.write(0x02); // select pen 2 again
        ga.write(0x44);
        assert_eq!(ga.pens[2], 0x04);
        assert_eq!(ga.border, 0x0A, "border must not change when a pen is targeted");
    }

    #[test]
    fn test_mode_and_rom_config() {
        let mut ga = GateArray::default();
        ga.write(0x8C); // mode 0, lower+upper ROM disabled
        assert_eq!(ga.mode, 0x00);
        assert!(!ga.lower_rom_enabled);
        assert!(!ga.upper_rom_enabled);

        ga.write(0x8E); // mode 2, ROMs disabled
        assert_eq!(ga.mode, 2);

        ga.write(0x80); // mode 0, ROMs re-enabled
        assert!(ga.lower_rom_enabled);
        assert!(ga.upper_rom_enabled);
    }
}
