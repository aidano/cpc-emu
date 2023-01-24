///////////////////////
//
// Instructions 
//
///////////////////////

use std::str::FromStr;

use log::{debug, error};

use crate::{memory::{Memory, Registers, FlagValue, AddressBus, DataBus, RegisterOperations, Register, DefaultRegister}, utils::{combine_to_double_byte, split_double_byte, self}, runtime::{RuntimeComponents}};
use super::{Instruction, Operands};


#[macro_export]
macro_rules! inst_metadata {
    ( $op_count:expr,$op_code:expr,$assem:expr) => {
        fn operand_count(&self) -> u8 {
            $op_count
        }
    
        fn machine_code(&self) -> &str {
            $op_code
        }
    
        fn assembly(&self) -> &str {
            $assem
        }
    };
}

// #00 to 0F
#[derive(Debug, Clone)]
pub struct _0x00 {}
impl Instruction for _0x00 {
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        4
    }

    inst_metadata!(0, "00", "nop");
}





#[derive(Debug, Copy, Clone)]
pub struct _0x01 {}
impl Instruction for _0x01 {
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        match operands {
            Operands::Two(first, second) => {
                RegisterOperations::ld_register_pair_with_value((&mut components.registers.b, &mut components.registers.c), combine_to_double_byte(second, first));
            }
            _ => error!("Wrong operands used for ld_bc"),
        }
        10
    }

    inst_metadata!(2, "01 *1 *2", "LD BC,*2*1");
}

pub struct _0x02 {}
impl Instruction for _0x02 {
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        RegisterOperations::ld_register_from_addr(&mut components.mem, &mut components.registers.a, (&components.registers.b, &components.registers.c));
        7
    }

    inst_metadata!(0, "02", "LD (BC),A");
}

pub struct _0x03 {}
impl Instruction for _0x03 {
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        RegisterOperations::inc_register_pair((&mut components.registers.b, &mut components.registers.c), &mut components.registers.f);
        6
    }

    inst_metadata!(0, "03", "INC BC");
}

pub struct _0x04 {}
impl Instruction for _0x04 {
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        RegisterOperations::inc(&mut components.registers.b, &mut components.registers.f);
        4
    }

    inst_metadata!(0, "04", "INC B");
}

pub struct _0x05 {}
impl Instruction for _0x05 {
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        RegisterOperations::dec(&mut components.registers.b, &mut components.registers.f);
        4
    }

    inst_metadata!(0, "05", "DEC B");
}

pub struct _0x06 {}
impl Instruction for _0x06 {
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        match operands {
            Operands::One(value) => {
                RegisterOperations::ld_register_with_value(&mut components.registers.b, value)
            }
            _ => error!("Wrong operands used for ld_b"),
        }
        7
    }

    inst_metadata!(1, "06 *1", "LD B,*1");
}


pub struct _0x07 {}
impl Instruction for _0x07 {
    // The contents of A are rotated left one bit position. 
    // The contents of bit 7 are copied to the carry flag and bit 0.
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        let value = components.registers.a.get();
        let bit_7 = (value & 0x80) >> 7; // left-most bit (i.e. 128)
        components.registers.a.set((value << 1) | bit_7);
        match bit_7 {
            0 => components.registers.f.set_carry(FlagValue::Unset),
            1 => components.registers.f.set_carry(FlagValue::Set),
            _ => error!("bit 7 incorrectly set for InstRlca")
        }
        4
    }

    inst_metadata!(0, "07", "RCLA");
}

pub struct _0x08 {}
impl Instruction for _0x08 {
    // Exchanges the 16-bit contents of AF and AF'.
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        let mut registers = &mut components.registers;
        let a_val = registers.a.get();
        let f_val = registers.f.get();
        registers.a.set(registers.a_.get());
        registers.f.set(registers.f_.get());
        registers.a_.set(a_val);
        registers.f_.set(f_val);
        4
    }

    inst_metadata!(0, "08", "EX AF,AF'");
}

pub struct _0x09 {}
impl Instruction for _0x09 {
    // The value of BC is added to HL.
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        let registers = &mut components.registers;
        RegisterOperations::add_register_pairs((&mut registers.h, &mut registers.l), (&mut registers.b, &mut registers.c), &mut registers.f);
        11
    }

    inst_metadata!(0, "09", "ADD HL,BC");
}

#[derive(Debug, Clone)]
pub struct _0x0B {}
impl Instruction for _0x0B {
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        let mut registers = &mut components.registers;
        RegisterOperations::dec_register_pair((&mut registers.b, &mut registers.c), &mut registers.f);
        6
    }

    inst_metadata!(0, "0B", "DEC BC");
}

#[derive(Debug, Clone)]
pub struct _0x0D {}
impl Instruction for _0x0D {
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        let mut registers = &mut components.registers;
        RegisterOperations::dec(&mut registers.c, &mut registers.f);
        4
    }

    inst_metadata!(0, "0D", "DEC C");
}


// #10 to 1F


pub struct _0x11 {}
impl Instruction for _0x11 {

    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        // The signed value d is added to PC. The jump is measured from the start of the instruction opcode.
        match operands {
            Operands::Two(low, high) => {
                RegisterOperations::ld_register_pair_with_value((&mut components.registers.d, &mut components.registers.e), combine_to_double_byte(high, low));
            }
            _ => error!("Wrong operands used for {}", self.assembly()),
        }
        10
    }

    inst_metadata!(2, "11 *1 *2", "LD DE,*2*1");
}


pub struct _0x18 {}
impl Instruction for _0x18 {

    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        // The signed value d is added to PC. The jump is measured from the start of the instruction opcode.
        match operands {
            Operands::One(op1) => {
                components.registers.pc.set(components.registers.pc.get() + (op1 as u16));
            }
            _ => error!("Wrong operands used for {}", self.assembly()),
        }
        12
    }

    inst_metadata!(1, "18 *1", "JR *1");
}




// #20 to 2F

pub struct _0x20 {}
impl Instruction for _0x20 {
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        // If the zero flag is unset, the signed value d is added to PC. The jump is measured from the start of the instruction opcode.
        match operands {
            Operands::One(op1) => {
                if components.registers.f.get_zero() == FlagValue::Set {
                    components.registers.pc.set(components.registers.pc.get() + (op1 as u16));
                    return 12;
                }
            }
            _ => error!("Wrong operands used for {}", self.assembly()),
        }
        7
    }

    inst_metadata!(1, "20 *1", "JR NZ,*1");
}

pub struct _0x21 {}
impl Instruction for _0x21 {
    // load nn into hl
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        match operands {
            Operands::Two(op1, op2) => {
                RegisterOperations::ld_register_pair_with_value((&mut components.registers.h, &mut components.registers.l), combine_to_double_byte(op2, op1));
            }
            _ => error!("Wrong operands used for {}", self.assembly()),
        }
        10
    }

    inst_metadata!(2, "21 *1 *2", "LD HL,*2*1");
}

pub struct _0x2B {}
impl Instruction for _0x2B {
    // dec hl
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        RegisterOperations::dec_register_pair((&mut components.registers.h, &mut components.registers.l), &mut components.registers.f);
        6
    }

    inst_metadata!(0, "2B", "DEC HL");
}



// #30 to 3F
pub struct _0x36 {}
impl Instruction for _0x36 {
    // load nn into hl
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        match operands {
            Operands::One(value) => {
                RegisterOperations::ld_addr_with_value(&mut components.mem,(&mut components.registers.h, &mut components.registers.l), value);
            }
            _ => error!("Wrong operands used for {}", self.assembly()),
        }
        10
    }

    inst_metadata!(1, "36 *1", "LD (HL),*1");
}



// #40 to 4F
pub struct _0x4C {}
impl Instruction for _0x4C {
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        // Contents of h are loaded into c
        RegisterOperations::ld_register_from_register(&components.registers.h, &mut components.registers.c);
        4
    }

    inst_metadata!(0, "4C", "LD C,H");
}



// #70 to fF
pub struct _0x78 {}
impl Instruction for _0x78 {
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        RegisterOperations::ld_register_from_register(&components.registers.b, &mut components.registers.a);
        4
    }

    inst_metadata!(0, "78", "LD A,B");
}


pub struct _0x7E {}
impl Instruction for _0x7E {
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        RegisterOperations::ld_register_from_addr(&components.mem, &mut components.registers.a, (&components.registers.h, &components.registers.l));
        7
    }

    inst_metadata!(0, "7E", "LD A,(HL)");
}





// #C0 to CF

pub struct _0xC0 {}
impl Instruction for _0xC0 {

    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        // if zero flag is not set, pop sp value onto pc
        if components.registers.f.get_zero() == FlagValue::Unset {
            components.registers.pc.set(components.registers.sp.pop(&components.mem));
            return 11;
        }
        5
    }

    inst_metadata!(0, "C0", "RET NZ");
}

pub struct _0xC2 {}
impl Instruction for _0xC2 {
    
    // Jump to address provided in operands if zero flag is set
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        if components.registers.f.get_zero() ==  FlagValue::Unset {
            if let Operands::Two(low, high) = operands {
                components.registers.pc.set(utils::combine_to_double_byte(high, low));
            }
        }
        10
    }

    inst_metadata!(2, "C2 *1 *2", "JP NZ,*2*1");
}

pub struct _0xC3 {}
impl Instruction for _0xC3 {
    
    // Jump to address provided in operands
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16{
        if let Operands::Two(low, high) = operands {
            components.registers.pc.set(utils::combine_to_double_byte(high, low));
        }
        10
    }

    inst_metadata!(2, "C3 *1 *2", "JP *2*1");
}

pub struct _0xC5 {}
impl Instruction for _0xC5 {

    // Push contents of B and C onto stack.
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        RegisterOperations::push_register_pair((&components.registers.b, &components.registers.c), &mut components.registers.sp, &mut components.mem);
        11
    }

    inst_metadata!(0, "C5", "PUSH BC");
}

pub struct _0xC9 {}
impl Instruction for _0xC9 {
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        let addr = components.registers.sp.pop(&&components.mem);
        components.registers.pc.set(addr);
        10
    }

    inst_metadata!(0, "C9", "RET");
}

// #D0 to DF
pub struct _0xD9 {}
impl Instruction for _0xD9 {
    // Bitwise AND a with operand. Set flags accordingly.
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        let b = components.registers.b.get();
        let c = components.registers.c.get();
        let d = components.registers.d.get();
        let e = components.registers.e.get();
        let h = components.registers.h.get();
        let l = components.registers.l.get();
        components.registers.b.set(components.registers.b_.get());
        components.registers.c.set(components.registers.c_.get());
        components.registers.d.set(components.registers.d_.get());
        components.registers.e.set(components.registers.e_.get());
        components.registers.h.set(components.registers.h_.get());
        components.registers.l.set(components.registers.l_.get());
        components.registers.b_.set(b);
        components.registers.c_.set(c);
        components.registers.d_.set(d);
        components.registers.e_.set(e);
        components.registers.h_.set(h);
        components.registers.l_.set(l);
        4
    }

    inst_metadata!(0, "D9", "EXX");
}


// #E0 to EF
pub struct _0xE6 {}
impl Instruction for _0xE6 {
    
    // Bitwise AND a with operand. Set flags accordingly.
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        if let Operands::One(val) = operands {
            components.registers.a.and(val, &mut components.registers.f)
        }
        7
    }

    inst_metadata!(1, "E6 *1", "AND *1");
}

// #F0 to FF

pub struct _0xF0 {}
impl Instruction for _0xF0 {
    
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        if components.registers.f.get_sign() == FlagValue::Set {
            components.registers.pc.set(components.registers.sp.pop(&components.mem));
            return 11;
        }
        5
    }

    inst_metadata!(0, "F0", "RET P");
}

pub struct _0xF2 {}
impl Instruction for _0xF2 {
    
    // Jump to address provided in operands if sign flag is set
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        if components.registers.f.get_sign() ==  FlagValue::Set {
            if let Operands::Two(low, high) = operands {
                components.registers.pc.set(utils::combine_to_double_byte(high, low));
            }
        }
        10
    }

    inst_metadata!(2, "F2 *1 *2", "JP P,*2*1");
}

pub struct _0xF3 {}
impl Instruction for _0xF3 {
    
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        components.registers.maskable_interrupt_enabled = false;
        4
    }

    inst_metadata!(0, "F3", "DI");
}

pub struct _0xF5 {}
impl Instruction for _0xF5 {
    
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        RegisterOperations::push_register_pair((&components.registers.a, &components.registers.f), &mut components.registers.sp, &mut components.mem);
        11
    }

    inst_metadata!(0, "F5", "PUSH AF");
}


// Tests

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::{instruction_set::{Instruction, Operands, InstructionSet, self, basic::{_0xC9, _0xC5, _0xC2, _0xF5}}, memory::{Memory, Registers, AddressBus, DataBus, FlagValue, Register}, runtime::{Runtime, RuntimeComponents}, utils::split_double_byte};

    use super::{_0x04, _0x05, _0x07, _0xE6, _0x0B};

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

        components.registers.f.set_zero(FlagValue::Unset);
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

    #[test]
    fn and_n() {
        let mut components = runtime_components();

        components.registers.a.set(120);
        components.registers.f.set(0);
        _0xE6 {}.execute(&mut components, Operands::One(105));
        assert!(components.registers.f.get_carry() == FlagValue::Unset);
        assert!(components.registers.f.get_add_subtract() == FlagValue::Unset);
        assert!(components.registers.f.get_parity_overflow() == FlagValue::Unset);
        assert!(components.registers.f.get_half_carry() == FlagValue::Set);
        assert!(components.registers.f.get_zero() == FlagValue::Unset);
        assert!(components.registers.f.get_sign() == FlagValue::Unset);

        components.registers.a.set(128);
        components.registers.f.set(0);
        _0xE6 {}.execute(&mut components, Operands::One(135));
        assert!(components.registers.f.get_carry() == FlagValue::Unset);
        assert!(components.registers.f.get_add_subtract() == FlagValue::Unset);
        assert!(components.registers.f.get_parity_overflow() == FlagValue::Set);
        assert!(components.registers.f.get_half_carry() == FlagValue::Set);
        assert!(components.registers.f.get_zero() == FlagValue::Unset);
        assert!(components.registers.f.get_sign() == FlagValue::Unset);
    }


    #[test]
    fn dec_bc() {
        let mut components = runtime_components();
        components.registers.b.set(0xFF);
        components.registers.c.set(0x3F);

        let cycles = _0x0B {}.execute(&mut components, Operands::None);
        assert!(cycles == 6);
        assert!(components.registers.b.get() == 0xFF);
        assert!(components.registers.c.get() == 0x3E);
    }


}

