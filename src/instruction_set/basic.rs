///////////////////////
//
// Instructions 
//
///////////////////////

use log::{debug, error};

use crate::{memory::{Memory, Registers, FlagValue}, utils::{combine_to_double_byte, split_double_byte}};
use super::{Instruction, Operands};

// #00 to 0F
#[derive(Debug, Clone)]
pub struct InstNOP {}
impl Instruction for InstNOP {
    fn execute(&self, mem: &mut Memory, reg: &mut Registers, operands: Operands) {
        debug!("nop");
        // No op. Maybe later this will implementing something to account for the time this instruction takes
    }

    fn operand_count(&self) -> u8 {
        0
    }

    fn op_code(&self) -> u8 {
        0x00
    }
}

#[derive(Debug, Copy, Clone)]
pub struct InstLdBCnn {}
impl Instruction for InstLdBCnn {
    fn execute(&self, mem: &mut Memory, reg: &mut Registers, operands: Operands) {
        debug!("ld_bc {:?}", operands);
        match operands {
            Operands::Two(first, second) => {
                reg.b.set(first);
                reg.c.set(second);
            }
            _ => error!("Wrong operands used for ld_bc"),
        }
        
    }

    fn operand_count(&self) -> u8 {
        2
    }

    fn op_code(&self) -> u8 {
        0x01
    }
}

pub struct InstLdBCa {}
impl Instruction for InstLdBCa {
    fn execute(&self, mem: &mut Memory, reg: &mut Registers, operands: Operands) {
        debug!("ld_(bc)_a {:?}", operands);
        let addr = combine_to_double_byte(reg.b.get(), reg.c.get());
        reg.a.set(mem.locations[addr as usize]);
    }

    fn operand_count(&self) -> u8 {
        0
    }

    fn op_code(&self) -> u8 {
        0x02
    }
}

pub struct InstIncBC {}
impl Instruction for InstIncBC {
    fn execute(&self, mem: &mut Memory, reg: &mut Registers, operands: Operands) {
        println!("inc bc {:?}", operands);
        let mut value = combine_to_double_byte(reg.b.get(), reg.c.get());
        value += 1;
        let split = split_double_byte(value);
        reg.b.set(split.0);
        reg.c.set(split.1);
    }

    fn operand_count(&self) -> u8 {
        0
    }

    fn op_code(&self) -> u8 {
        0x03
    }
}

pub struct InstIncB {}
impl Instruction for InstIncB {
    fn execute(&self, mem: &mut Memory, reg: &mut Registers, operands: Operands) {
        reg.b.set(reg.b.get() + 1);
    }

    fn operand_count(&self) -> u8 {
        0
    }

    fn op_code(&self) -> u8 {
        0x04
    }
}

pub struct InstDecB {}
impl Instruction for InstDecB {
    fn execute(&self, mem: &mut Memory, reg: &mut Registers, operands: Operands) {
        reg.b.set(reg.b.get() -1);
    }

    fn operand_count(&self) -> u8 {
        0
    }

    fn op_code(&self) -> u8 {
        0x05
    }
}

pub struct InstLdB {}
impl Instruction for InstLdB {
    fn execute(&self, mem: &mut Memory, reg: &mut Registers, operands: Operands) {
        debug!("ld_bc {:?}", operands);
        match operands {
            Operands::One(operand) => {
                reg.b.set(operand);
            }
            _ => error!("Wrong operands used for ld_b"),
        }
        
    }

    fn operand_count(&self) -> u8 {
        1
    }

    fn op_code(&self) -> u8 {
        0x06
    }
}


pub struct InstRlca {}
impl Instruction for InstRlca {
    // The contents of A are rotated left one bit position. 
    // The contents of bit 7 are copied to the carry flag and bit 0.
    fn execute(&self, mem: &mut Memory, reg: &mut Registers, operands: Operands) {
        let value = reg.a.get();
        let bit_7 = (value & 0x80) >> 7; // left-most bit (i.e. 128)
        reg.a.set((value << 1) | bit_7);
        match bit_7 {
            0 => reg.set_carry(FlagValue::Unset),
            1 => reg.set_carry(FlagValue::Set),
            _ => error!("bit 7 incorrectly set for InstRlca")
        }
    }

    fn operand_count(&self) -> u8 {
        0
    }

    fn op_code(&self) -> u8 {
        0x07
    }
}

pub struct InstExAfAf_ {}
impl Instruction for InstExAfAf_ {
    // Exchanges the 16-bit contents of AF and AF'.
    fn execute(&self, mem: &mut Memory, reg: &mut Registers, operands: Operands) {
        let a_val = reg.a.get();
        let f_val = reg.f.get();
        reg.a.set(reg.a_.get());
        reg.f.set(reg.f_.get());
        reg.a_.set(a_val);
        reg.f.set(f_val);
    }

    fn operand_count(&self) -> u8 {
        0
    }

    fn op_code(&self) -> u8 {
        0x08
    }
}

pub struct InstAddHlBc {}
impl Instruction for InstAddHlBc {
    // The value of BC is added to HL.
    fn execute(&self, mem: &mut Memory, reg: &mut Registers, operands: Operands) {
        let hl = combine_to_double_byte(reg.h.get(), reg.l.get());
        let bc = combine_to_double_byte(reg.b.get(), reg.c.get());
        let total_as_u32 = (hl as u32 + bc as u32);
        let carry = if (hl as u32 + bc as u32) > u16::MAX as u32 {
             FlagValue::Set 
            } else {
                 FlagValue::Unset 
            };
        let half_carry = if (hl & 8 == 1) && (bc & 8 == 1) {
                FlagValue::Set
            } else {
                FlagValue::Unset
            };
        let total_as_u16 = (total_as_u32 & 0xFFFF) as u16;
        let hl = split_double_byte(total_as_u16);
        reg.h.set(hl.0);
        reg.l.set(hl.1);
        reg.set_carry(carry);
        reg.set_half_carry(half_carry);
        reg.set_add_subtract(FlagValue::Set);
    }

    fn operand_count(&self) -> u8 {
        0
    }

    fn op_code(&self) -> u8 {
        0x09
    }
}


// #10 to 1F














// Tests

#[cfg(test)]
mod tests {
    use crate::{instruction_set::{Instruction, Operands}, memory::{Memory, Registers}};

    use super::{InstIncB, InstDecB, InstRlca};

    fn default_mem_reg() -> (Memory, Registers) {
        return (Memory::default(), Registers::default());
    }

    #[test]
    fn inc_b() {
        let (mut memory, mut registers) = default_mem_reg();
        assert!(registers.b.get() == 0);
        
        let inc_b = InstIncB{};
        inc_b.execute(&mut memory,&mut registers, Operands::None);

        assert!(registers.b.get() == 1);
    }

    #[test]
    fn dec_b() {
        let (mut memory, mut registers) = default_mem_reg();

        registers.b.set(1);
        assert!(registers.b.get() == 1);

        
        let dec_b = InstDecB{};
        dec_b.execute(&mut memory,&mut registers, Operands::None);

        assert!(registers.b.get() == 0);
    }

    #[test]
    fn rlca_doubling() {
        // The contents of A are rotated left one bit position. 
        // The contents of bit 7 are copied to the carry flag and bit 0.

        let (mut memory, mut registers) = default_mem_reg();

        registers.a.set(1);

        let rcla = InstRlca {};
        rcla.execute(&mut memory, &mut registers, Operands::None);
        assert!(registers.a.get() == 2);


        registers.a.set(35);

        let rcla = InstRlca {};
        rcla.execute(&mut memory, &mut registers, Operands::None);
        assert!(registers.a.get() == 70);
    }


    #[test]
    fn rlca_overflow() {
        // The contents of A are rotated left one bit position. 
        // The contents of bit 7 are copied to the carry flag and bit 0.

        let (mut memory, mut registers) = default_mem_reg();

        registers.a.set(255);

        let rcla = InstRlca {};
        rcla.execute(&mut memory, &mut registers, Operands::None);
        assert!(registers.a.get() == 255);

        registers.a.set(254);

        let rcla = InstRlca {};
        rcla.execute(&mut memory, &mut registers, Operands::None);
        assert!(registers.a.get() == 253);
    }

}

