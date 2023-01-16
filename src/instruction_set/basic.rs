///////////////////////
//
// Instructions 
//
///////////////////////

use std::str::FromStr;

use log::{debug, error};

use crate::{memory::{Memory, Registers, FlagValue, AddressBus, DataBus}, utils::{combine_to_double_byte, split_double_byte, self}, runtime::{RuntimeComponents}};
use super::{Instruction, Operands};

// #00 to 0F
#[derive(Debug, Clone)]
pub struct _0x00 {}
impl Instruction for _0x00 {
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u8 {
        4
    }

    fn operand_count(&self) -> u8 {
        0
    }

    fn op_code(&self) -> u8 {
        0x00
    }

    fn machine_code(&self) -> &str {
        "00"
    }

    fn assembly(&self) -> &str {
        "nop"
    }
}


#[derive(Debug, Copy, Clone)]
pub struct _0x01 {}
impl Instruction for _0x01 {
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u8 {
        match operands {
            Operands::Two(first, second) => {
                components.registers.b.set(first);
                components.registers.c.set(second);
            }
            _ => error!("Wrong operands used for ld_bc"),
        }
        10
    }

    fn operand_count(&self) -> u8 {
        2
    }

    fn op_code(&self) -> u8 {
        0x01
    }

    fn assembly(&self) -> &str {
        "LD BC,*2*1"
    }

    fn machine_code(&self) -> &str {
        "01 *1 *2"
    }
}

pub struct _0x02 {}
impl Instruction for _0x02 {
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u8 {
        let reg = &mut components.registers;
        debug!("ld_(bc)_a {:?}", operands);
        let addr = combine_to_double_byte(reg.b.get(), reg.c.get());
        reg.a.set(components.mem.locations[addr as usize]);
        7
    }

    fn operand_count(&self) -> u8 {
        0
    }

    fn op_code(&self) -> u8 {
        0x02
    }

    fn machine_code(&self) -> &str {
        "02"
    }

    fn assembly(&self) -> &str {
        "LD (BC),A"
    }
}

pub struct _0x03 {}
impl Instruction for _0x03 {
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u8 {
        let reg = &mut components.registers;
        let mut value = combine_to_double_byte(reg.b.get(), reg.c.get());
        value += 1;
        let split = split_double_byte(value);
        reg.b.set(split.0);
        reg.c.set(split.1);
        6
    }

    fn operand_count(&self) -> u8 {
        0
    }

    fn op_code(&self) -> u8 {
        0x03
    }

    fn machine_code(&self) -> &str {
        "03"
    }

    fn assembly(&self) -> &str {
        "INC BC"
    }
}

pub struct _0x04 {}
impl Instruction for _0x04 {
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u8 {
        components.registers.b.set(components.registers.b.get() + 1);
        4
    }

    fn operand_count(&self) -> u8 {
        0
    }

    fn op_code(&self) -> u8 {
        0x04
    }

    fn machine_code(&self) -> &str {
        "04"
    }

    fn assembly(&self) -> &str {
        "INC B"
    }
}

pub struct _0x05 {}
impl Instruction for _0x05 {
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u8 {
        components.registers.b.set(components.registers.b.get() -1);
        4
    }

    fn operand_count(&self) -> u8 {
        0
    }

    fn op_code(&self) -> u8 {
        0x05
    }

    fn machine_code(&self) -> &str {
        "05"
    }

    fn assembly(&self) -> &str {
       "DEC B"
    }
}

pub struct _0x06 {}
impl Instruction for _0x06 {
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u8 {
        match operands {
            Operands::One(operand) => {
                components.registers.b.set(operand);
            }
            _ => error!("Wrong operands used for ld_b"),
        }
        7
    }

    fn operand_count(&self) -> u8 {
        1
    }

    fn op_code(&self) -> u8 {
        0x06
    }

    fn machine_code(&self) -> &str {
        "06 *1"
    }

    fn assembly(&self) -> &str {
        "LD B,*1"
    }
}


pub struct _0x07 {}
impl Instruction for _0x07 {
    // The contents of A are rotated left one bit position. 
    // The contents of bit 7 are copied to the carry flag and bit 0.
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u8 {
        let value = components.registers.a.get();
        let bit_7 = (value & 0x80) >> 7; // left-most bit (i.e. 128)
        components.registers.a.set((value << 1) | bit_7);
        match bit_7 {
            0 => components.registers.set_carry(FlagValue::Unset),
            1 => components.registers.set_carry(FlagValue::Set),
            _ => error!("bit 7 incorrectly set for InstRlca")
        }
        4
    }

    fn operand_count(&self) -> u8 {
        0
    }

    fn op_code(&self) -> u8 {
        0x07
    }

    fn machine_code(&self) -> &str {
        "07"
    }

    fn assembly(&self) -> &str {
       "RCLA"
    }
}

pub struct _0x08 {}
impl Instruction for _0x08 {
    // Exchanges the 16-bit contents of AF and AF'.
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u8 {
        let mut registers = &mut components.registers;
        let a_val = registers.a.get();
        let f_val = registers.f.get();
        registers.a.set(registers.a_.get());
        registers.f.set(registers.f_.get());
        registers.a_.set(a_val);
        registers.f.set(f_val);
        4
    }

    fn operand_count(&self) -> u8 {
        0
    }

    fn op_code(&self) -> u8 {
        0x08
    }

    fn machine_code(&self) -> &str {
        "08"
    }

    fn assembly(&self) -> &str {
        "EX AF,AF"
    }
}

pub struct _0x09 {}
impl Instruction for _0x09 {
    // The value of BC is added to HL.
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u8 {
        let mut registers = &mut components.registers;
        let hl = combine_to_double_byte(registers.h.get(), registers.l.get());
        let bc = combine_to_double_byte(registers.b.get(), registers.c.get());
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
        registers.h.set(hl.0);
        registers.l.set(hl.1);
        registers.set_carry(carry);
        registers.set_half_carry(half_carry);
        registers.set_add_subtract(FlagValue::Set);
        11
    }

    fn operand_count(&self) -> u8 {
        0
    }

    fn op_code(&self) -> u8 {
        0x09
    }

    fn machine_code(&self) -> &str {
        "09"
    }

    fn assembly(&self) -> &str {
        "ADD HL,BC"
    }
}





// #40 to 4F
pub struct _0x4C {}
impl Instruction for _0x4C {
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u8 {
        // Contents of h are loaded into c
        components.registers.c.set(components.registers.h.get());
        4
    }

    fn operand_count(&self) -> u8 {
        0
    }

    fn op_code(&self) -> u8 {
        0x4C
    }

    fn machine_code(&self) -> &str {
        "4C"
    }

    fn assembly(&self) -> &str {
        "LD C,H"
    }
}



// #70 to fF
pub struct _0x78 {}
impl Instruction for _0x78 {
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u8 {
        // Contents of h are loaded into c
        components.registers.a.set(components.registers.b.get());
        4
    }

    fn operand_count(&self) -> u8 {
        0
    }

    fn op_code(&self) -> u8 {
        0x78
    }

    fn machine_code(&self) -> &str {
        "78"
    }

    fn assembly(&self) -> &str {
        "LD A,B"
    }
}








// #C0 to CF

pub struct _0xC0 {}
impl Instruction for _0xC0 {

    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u8 {
        // if zero flag is not set, pop sp value onto pc
        if components.registers.get_zero() == FlagValue::Unset {
            components.registers.pc.set(components.registers.sp.pop(&components.mem));
            return 11;
        }
        5
    }

    fn operand_count(&self) -> u8 {
        0
    }

    fn op_code(&self) -> u8 {
        0xC0
    }

    fn machine_code(&self) -> &str {
        "C0"
    }

    fn assembly(&self) -> &str {
        "RET NZ"
    }
}

pub struct _0xC2 {}
impl Instruction for _0xC2 {
    
    // Jump to address provided in operands if zero flag is set
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u8 {
        if components.registers.get_zero() ==  FlagValue::Unset {
            if let Operands::Two(low, high) = operands {
                components.registers.pc.set(utils::combine_to_double_byte(high, low));
            }
        }
        10
    }

    fn operand_count(&self) -> u8 {
        2
    }

    fn op_code(&self) -> u8 {
        0xC2
    }

    fn machine_code(&self) -> &str {
        "C2 *1 *2"
    }

    fn assembly(&self) -> &str {
        "JP NZ,*2*1"
    }
}

pub struct _0xC3 {}
impl Instruction for _0xC3 {
    
    // Jump to address provided in operands
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u8{
        if let Operands::Two(low, high) = operands {
            components.registers.pc.set(utils::combine_to_double_byte(high, low));
        }
        10
    }

    fn operand_count(&self) -> u8 {
        2
    }

    fn op_code(&self) -> u8 {
        0xC3
    }

    fn machine_code(&self) -> &str {
       "C3 *1 *2"
    }

    fn assembly(&self) -> &str {
        "JP *2*1"
    }
}

pub struct _0xC5 {}
impl Instruction for _0xC5 {

    // Push contents of B and C onto stack.
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u8 {
        let bc = combine_to_double_byte(components.registers.b.get(), components.registers.c.get());
        components.registers.sp.push(&mut components.mem, bc);
        11
    }

    fn operand_count(&self) -> u8 {
        0
    }

    fn op_code(&self) -> u8 {
        0xC5
    }

    fn machine_code(&self) -> &str {
        "C5"
    }

    fn assembly(&self) -> &str {
        "PUSH BC"
    }
}

pub struct _0xC9 {}
impl Instruction for _0xC9 {
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u8 {
        let addr = components.registers.sp.pop(&&components.mem);
        components.registers.pc.set(addr);
        10
    }

    fn operand_count(&self) -> u8 {
        0
    }

    fn op_code(&self) -> u8 {
        0xC9
    }

    fn machine_code(&self) -> &str {
        "C9"
    }

    fn assembly(&self) -> &str {
        "RET"
    }
}



// #F0 to FF

pub struct _0xF0 {}
impl Instruction for _0xF0 {
    
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u8 {
        if components.registers.get_sign() == FlagValue::Set {
            components.registers.pc.set(components.registers.sp.pop(&components.mem));
            return 11;
        }
        5
    }

    fn operand_count(&self) -> u8 {
        0
    }

    fn op_code(&self) -> u8 {
        0xF0
    }

    fn machine_code(&self) -> &str {
        "F0"
    }

    fn assembly(&self) -> &str {
        "RET P"
    }
}

pub struct _0xF3 {}
impl Instruction for _0xF3 {
    
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u8 {
        components.registers.maskable_interrupt_enabled = false;
        4
    }

    fn operand_count(&self) -> u8 {
        0
    }

    fn op_code(&self) -> u8 {
        0xF3
    }

    fn machine_code(&self) -> &str {
        "F3"
    }

    fn assembly(&self) -> &str {
        "DI"
    }
}

pub struct _0xF5 {}
impl Instruction for _0xF5 {
    
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u8 {
        let value = combine_to_double_byte(components.registers.a.get(), components.registers.f.get());
        components.registers.sp.push(&mut components.mem, value);
        11
    }

    fn operand_count(&self) -> u8 {
        0
    }

    fn op_code(&self) -> u8 {
        0xF5
    }

    fn machine_code(&self) -> &str {
        "F5"
    }

    fn assembly(&self) -> &str {
        "PUSH AF"
    }
}


// Tests

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::{instruction_set::{Instruction, Operands, InstructionSet, self, basic::{_0xC9, _0xC5, _0xC2, _0xF5}}, memory::{Memory, Registers, AddressBus, DataBus, FlagValue}, runtime::{Runtime, RuntimeComponents}, utils::split_double_byte};

    use super::{_0x04, _0x05, _0x07};

    fn runtime_components() -> RuntimeComponents {
        RuntimeComponents { mem: Memory::default(), registers: Registers::default(), address_bus: AddressBus { value: 0 }, data_bus: DataBus { } }
    }

    #[test]
    fn inc_b() {
        let mut components = runtime_components();
        
        assert!(components.registers.b.get() == 0);
        _0x04 {}.execute(&mut components, Operands::None);
        assert!(components.registers.b.get() == 1);
    }

    #[test]
    fn dec_b() {
        let mut components = runtime_components();

        components.registers.b.set(1);
        assert!(components.registers.b.get() == 1);
        _0x05 {}.execute(&mut components, Operands::None);
        assert!(components.registers.b.get() == 0);
    }

    #[test]
    fn rlca_doubling() {
        // The contents of A are rotated left one bit position. 
        // The contents of bit 7 are copied to the carry flag and bit 0.
        let mut components = runtime_components();

        components.registers.a.set(1);
        _0x07 {}.execute(&mut components, Operands::None);
        assert!(components.registers.a.get() == 2);

        components.registers.a.set(35);
        _0x07 {}.execute(&mut components, Operands::None);
        assert!(components.registers.a.get() == 70);
    }


    #[test]
    fn rlca_overflow() {
        // The contents of A are rotated left one bit position. 
        // The contents of bit 7 are copied to the carry flag and bit 0.
        let mut components = runtime_components();

        components.registers.a.set(255);
        _0x07 {}.execute(&mut components, Operands::None);
        assert!(components.registers.a.get() == 255);

        components.registers.a.set(254);
        _0x07 {}.execute(&mut components, Operands::None);
        assert!(components.registers.a.get() == 253);
    }

    #[test]
    fn jpnz() {
        let mut components = runtime_components();

        components.registers.set_zero(FlagValue::Unset);
        _0xC2 {}.execute(&mut components, Operands::Two(0xAA, 0xFF));
        assert!(components.registers.pc.get() == 0xFFAA);
    }

    #[test]
    fn push_bc() {
        let mut components = runtime_components();

        components.registers.b.set(0xA);
        components.registers.c.set(0xB);
        _0xC5 {}.execute(&mut components, Operands::None);
        
        let value = components.registers.sp.pop(&components.mem);

        let (high, low) = split_double_byte(value);
        assert!(high == 0xA);
        assert!(low == 0xB);
    }

    #[test]
    fn ret() {
        let mut components = runtime_components();

        components.registers.sp.push(&mut components.mem, 0xABCD);
        _0xC9{}.execute(&mut components, Operands::None);
        assert!(components.registers.pc.get() == 0xABCD); 
    }

    #[test]
    fn push_af() {
        let mut components = runtime_components();

        components.registers.a.set(0xEF);
        components.registers.f.set(0x8C);
        _0xF5 {}.execute(&mut components, Operands::None);
        
        let value = components.registers.sp.pop(&components.mem);

        let (high, low) = split_double_byte(value);
        assert!(high == 0xEF);
        assert!(low == 0x8C);
    }


}

