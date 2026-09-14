// AY-3-8912 PSG behind the PPI. Only the register file and the I/O ports are
// modelled; tone generation (audio) is out of scope for now.

pub const PSG_REG_SELECT: u8 = 0xC0;
pub const PSG_REG_WRITE: u8 = 0x80;
pub const PSG_REG_READ: u8 = 0x40;

pub const PSG_PORT_A: usize = 14;
pub const PSG_PORT_B: usize = 15;

#[derive(Debug, Clone)]
pub struct Psg {
    pub reg_select: u8,
    pub registers: [u8; 16],
    /// The PSG control byte, maintained from PPI port C upper-half writes.
    pub control: u8,
}

impl Psg {
    pub fn default() -> Psg {
        Psg {
            reg_select: 0,
            registers: [0x00; 16],
            control: 0,
        }
    }

    /// Handles a PSG data write (triggered by PPI port A / port C sequences).
    pub fn write_data(&mut self, value: u8) {
        match self.control & 0xC0 {
            PSG_REG_SELECT => self.reg_select = value & 0x0F,
            PSG_REG_WRITE => {
                let reg = self.reg_select as usize;
                if reg < 16 {
                    self.registers[reg] = value;
                }
            }
            _ => {}
        }
    }

    /// Handles a PSG data read (triggered by a PPI port A read while the PSG
    /// control byte is in read mode). `keyboard_row` is the currently selected
    /// keyboard matrix row, if the PPI wants the keyboard path.
    pub fn read_data(&self, keyboard_row: Option<u8>) -> u8 {
        let value = match self.reg_select as usize {
            PSG_PORT_A => match keyboard_row {
                Some(row) => {
                    if self.registers[7] & 0x40 == 0 {
                        row // port A configured as input: read the matrix
                    } else {
                        self.registers[PSG_PORT_A] & row
                    }
                }
                None => self.registers[PSG_PORT_A],
            },
            PSG_PORT_B => self.registers[PSG_PORT_B],
            reg => self.registers[reg],
        };
        value
    }
}

#[cfg(test)]
mod tests {
    use super::{Psg, PSG_REG_READ, PSG_REG_SELECT, PSG_REG_WRITE};

    #[test]
    fn test_select_then_write() {
        let mut psg = Psg::default();
        psg.control = PSG_REG_SELECT;
        psg.write_data(7); // latch register 7
        psg.control = PSG_REG_WRITE;
        psg.write_data(0x3F); // all ports output
        assert_eq!(psg.registers[7], 0x3F);
        assert_eq!(psg.reg_select, 7);
    }

    #[test]
    fn test_read_keyboard_row() {
        let mut psg = Psg::default();
        // Register 7 bit 6 clear: port A is input, so reads pass the matrix through
        psg.control = PSG_REG_SELECT;
        psg.write_data(14); // select register 14 (keyboard port A)
        psg.control = PSG_REG_READ;
        assert_eq!(psg.read_data(Some(0xFB)), 0xFB);
        // Register 7 bit 6 set: port A is output; stored value is ANDed with the matrix
        psg.control = PSG_REG_SELECT;
        psg.write_data(7);
        psg.control = PSG_REG_WRITE;
        psg.write_data(0x40);
        psg.control = PSG_REG_SELECT;
        psg.write_data(14);
        psg.control = PSG_REG_READ;
        assert_eq!(psg.read_data(Some(0xFB)), 0x00);
    }
}
