// 8255 PPI: glues the CPU to the PSG (ports A/C) and system status (port B).
// Semantics follow the 8255 datasheet modes and the standard CPC wiring.

use crate::devices::psg::{Psg, PSG_REG_READ};

pub const PPI_PORT_A: u8 = 0;
pub const PPI_PORT_B: u8 = 1;
pub const PPI_PORT_C: u8 = 2;
pub const PPI_CONTROL: u8 = 3;

/// Machine identification jumpers read on port B: OEM is Amstrad, video
/// refresh is 50Hz. The firmware uses these bits to
/// detect the machine model; the wrong value derails boot.
pub const PPI_JUMPERS: u8 = 0x1E;

#[derive(Debug, Clone)]
pub struct Ppi {
    pub control: u8,
    pub port_a: u8,
    pub port_b: u8,
    pub port_c: u8,
    /// Keyboard matrix line selected via port C lower half.
    pub keyboard_line: u8,
    /// Tape motor control bit (stored; no tape is emulated).
    pub tape_motor: bool,
}

impl Ppi {
    pub fn default() -> Ppi {
        Ppi {
            control: 0,
            port_a: 0,
            port_b: 0,
            port_c: 0,
            keyboard_line: 0,
            tape_motor: false,
        }
    }

    /// Handles an OUT to one of the PPI ports (0xF4xx-0xF7xx).
    pub fn write(&mut self, port: u8, value: u8, psg: &mut Psg) {
        match port {
            PPI_PORT_A => {
                self.port_a = value;
                if self.control & 0x10 == 0 {
                    // Port A in output mode: the value goes to the PSG data line.
                    psg.write_data(value);
                }
            }
            PPI_PORT_B => {
                self.port_b = value;
            }
            PPI_PORT_C => {
                self.port_c = value;
                self.port_c_side_effects(psg);
            }
            PPI_CONTROL => {
                if value & 0x80 != 0 {
                    // Mode set: store the control byte and clear the latches.
                    self.control = value;
                    self.port_a = 0;
                    self.port_b = 0;
                    self.port_c = 0;
                } else {
                    // Bit set/reset on port C.
                    let bit = (value >> 1) & 7;
                    let mask = 1 << bit;
                    if value & 1 != 0 {
                        self.port_c |= mask;
                    } else {
                        self.port_c &= !mask;
                    }
                    self.port_c_side_effects(psg);
                }
            }
            _ => unreachable!(),
        }
    }

    /// Port C writes (and bit set/reset operations) have side effects on the
    /// keyboard line selection and the PSG control byte.
    fn port_c_side_effects(&mut self, psg: &mut Psg) {
        if self.control & 1 == 0 {
            // Lower half is output: bits 0-3 select the keyboard matrix row.
            self.keyboard_line = self.port_c;
        }
        if self.control & 8 == 0 {
            // Upper half is output: bits 4-6 drive the PSG control lines.
            self.tape_motor = self.port_c & 0x10 != 0;
            psg.control = self.port_c;
            psg.write_data(self.port_a);
        }
    }

    /// Handles an IN from one of the PPI ports. `keyboard_row` is the keyboard
    /// matrix row for the currently selected line; `vsync` is the CRTC VSYNC
    /// status.
    pub fn read(&mut self, port: u8, keyboard_row: Option<u8>, vsync: bool, psg: &Psg) -> u8 {
        match port {
            PPI_PORT_A => {
                if self.control & 0x10 != 0 {
                    // Port A set to input: route through the PSG, but only
                    // while the PSG control lines are in read mode; otherwise
                    // the port floats high (no keys pressed on the matrix).
                    if psg.control & 0xC0 == PSG_REG_READ {
                        psg.read_data(keyboard_row)
                    } else {
                        0xFF
                    }
                } else {
                    self.port_a
                }
            }
            PPI_PORT_B => {
                // Tape level (0), printer ready (bit 6 set), machine jumpers
                // and the CRTC VSYNC status (bit 0).
                0x40 | PPI_JUMPERS | (vsync as u8)
            }
            PPI_PORT_C => {
                let mut value = self.port_c;
                if self.control & 9 != 0 {
                    if self.control & 8 != 0 {
                        // Upper half input: blank it and merge hardware status.
                        value &= 0x0F;
                        let mut psg_control = self.port_c & 0xC0;
                        if psg_control == 0xC0 {
                            psg_control = 0x80;
                        }
                        value |= psg_control | 0x20; // cassette write data always set
                        if self.tape_motor {
                            value |= 0x10;
                        }
                    }
                    if self.control & 1 == 0 {
                        // Lower half output: reads return all set (invalid).
                        value |= 0x0F;
                    }
                }
                value
            }
            PPI_CONTROL => 0, // the control register is write-only
            _ => unreachable!(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Ppi;
    use crate::devices::psg::Psg;

    #[test]
    fn test_mode_set_clears_latches() {
        let mut ppi = Ppi::default();
        let mut psg = Psg::default();
        ppi.port_c = 0xAB;
        ppi.port_a = 0xCD;
        ppi.write(3, 0x92, &mut psg); // mode set
        assert_eq!(ppi.control, 0x92);
        assert_eq!(ppi.port_c, 0);
        assert_eq!(ppi.port_a, 0);
    }

    #[test]
    fn test_keyboard_line_selection() {
        let mut ppi = Ppi::default();
        let mut psg = Psg::default();
        ppi.write(3, 0x80, &mut psg); // all ports output
        ppi.write(2, 0x05, &mut psg); // port C lower half: keyboard line 5
        assert_eq!(ppi.keyboard_line, 0x05);
        ppi.write(3, 0x03, &mut psg); // bit set/reset: SET bit 1 of port C
        assert_eq!(ppi.port_c, 0x07);
        assert_eq!(ppi.keyboard_line, 0x07, "line selection follows port C updates");
    }

    #[test]
    fn test_port_b_status() {
        let mut ppi = Ppi::default();
        let mut psg = Psg::default();
        let value = ppi.read(1, None, true, &psg);
        assert_eq!(value, 0x5F); // printer ready (0x40) + jumpers (0x1E) + VSYNC (bit 0)
        let value = ppi.read(1, None, false, &psg);
        assert_eq!(value, 0x5E);
    }

    #[test]
    fn test_port_b_jumper_bits_are_stable() {
        // Regression: the firmware detects the machine model from these bits.
        // Missing jumpers made the 6128 boot misidentify itself and reset-loop.
        let mut ppi = Ppi::default();
        let mut psg = Psg::default();
        assert_eq!(ppi.read(1, None, false, &psg) & 0x1E, 0x1E);
    }

    #[test]
    fn test_port_a_floating_high_outside_psg_read_mode() {
        let mut ppi = Ppi::default();
        let mut psg = Psg::default();
        // Port A input (0x92) but the PSG control lines are NOT in read mode:
        // the port must float high (0xFF), not expose register contents.
        ppi.write(3, 0x92, &mut psg);
        psg.control = 0xC0; // latch-address mode, not read mode
        let value = ppi.read(0, Some(0x00), false, &psg);
        assert_eq!(value, 0xFF, "port A must float high outside PSG read mode");
    }

    #[test]
    fn test_port_a_keyboard_read() {
        let mut ppi = Ppi::default();
        let mut psg = Psg::default();
        // Port A must be output to program the PSG register select
        ppi.write(3, 0x80, &mut psg); // all ports output
        ppi.write(2, 0xC0, &mut psg); // PSG control = 0xC0 (latch address)
        ppi.write(0, 14, &mut psg);   // PSG reg select = 14
        // Now switch port A to input for the keyboard read
        ppi.write(3, 0x92, &mut psg); // port A input, port C output
        ppi.write(2, 0x40, &mut psg); // PSG control = 0x40 (read)
        // Keyboard row 0xFB passes through (reg 7 = 0 -> input mode)
        let value = ppi.read(0, Some(0xFB), false, &psg);
        assert_eq!(value, 0xFB);
    }

    #[test]
    fn test_port_a_output_goes_to_psg() {
        let mut ppi = Ppi::default();
        let mut psg = Psg::default();
        ppi.write(3, 0x80, &mut psg); // all output
        ppi.write(2, 0xC0, &mut psg); // PSG latch address mode
        ppi.write(0, 7, &mut psg);    // select register 7 via port A
        ppi.write(2, 0x80, &mut psg); // PSG write mode
        ppi.write(0, 0x3F, &mut psg); // write value via port A
        assert_eq!(psg.registers[7], 0x3F);
    }
}
