///////////////////////
//
// Instructions 
//
///////////////////////

use std::str::FromStr;

use log::{debug, error};

use crate::{memory::{Memory, Registers, FlagValue, AddressBus, DataBus, RegisterOperations, Register, DefaultRegister}, utils::{combine_to_double_byte, split_double_byte, self, signed}, runtime::{RuntimeComponents}};
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
        RegisterOperations::ld_register_from_addr_with_register_pair(&mut components.mem, &mut components.registers.a, (&components.registers.b, &components.registers.c));
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
            _ => error!("bit 7 incorrectly set for {}", self.assembly())
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

pub struct _0x10 {}
impl Instruction for _0x10 {
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        // If the zero flag is unset, the signed value d is added to PC. The jump is measured from the start of the instruction opcode.
        match operands {
            Operands::One(value) => {
                let b = components.registers.b.get();
                components.registers.b.set(b - 1);
                if b-1 != 0 {
                    let jump_val = signed(value);
                    let val = components.registers.pc.get().wrapping_add(jump_val as u16); 
                    components.registers.pc.set(val);
                    return 13;
                }
            }
            _ => error!("Wrong operands used for {}", self.assembly()),
        }
        8
    }

    inst_metadata!(1, "10 *1", "DJNZ *1");
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

pub struct _0x0C {}
impl Instruction for _0x0C {
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        let mut registers = &mut components.registers;
        RegisterOperations::inc(&mut registers.c,  &mut registers.f);
        4
    }

    inst_metadata!(0, "0C", "INC C");
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

pub struct _0x0E {}
impl Instruction for _0x0E {
    // Loads n into C.
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        match operands {
            Operands::One(value) => {
                RegisterOperations::ld_register_with_value(&mut components.registers.c, value)
            }
            _ => error!("Wrong operands used for {}", self.assembly()),
        }
        7
    }

    inst_metadata!(1, "0E *1", "LD C,*1");
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

pub struct _0x13 {}
impl Instruction for _0x13 {
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        RegisterOperations::inc_register_pair((&mut components.registers.d, &mut components.registers.e), &mut components.registers.f);
        6
    }

    inst_metadata!(0, "13", "INC DE");
}

pub struct _0x19 {}
impl Instruction for _0x19 {
    // The value of DE is added to HL.
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        let registers = &mut components.registers;
        RegisterOperations::add_register_pairs((&mut registers.h, &mut registers.l), (&mut registers.d, &mut registers.e), &mut registers.f);
        11
    }

    inst_metadata!(0, "19", "ADD HL,DE");
}

pub struct _0x1A {}
impl Instruction for _0x1A {
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        //Loads the value pointed to by BC into A.
        RegisterOperations::ld_register_from_addr_with_register_pair(&components.mem, &mut components.registers.a, (&components.registers.b, &components.registers.c));
        7
    }

    inst_metadata!(0, "1A", "LD A,(DE)");
}

// #20 to 2F

pub struct _0x20 {}
impl Instruction for _0x20 {
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        // If the zero flag is unset, the signed value d is added to PC. The jump is measured from the start of the instruction opcode.
        match operands {
            Operands::One(op1) => {
                if components.registers.f.get_zero() == FlagValue::Unset {
                    let jump_val = signed(op1);
                    let val = components.registers.pc.get().wrapping_add(jump_val as u16);
                    components.registers.pc.set(val);
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


pub struct _0x22 {}
impl Instruction for _0x22 {
    // //Stores HL into the memory location pointed to by nn.
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        match operands {
            Operands::Two(op1, op2) => {
                RegisterOperations::ld_addr_from_value_with_register_pair(&mut components.mem, combine_to_double_byte(op1, op2), (&components.registers.h, &components.registers.l));
            }
            _ => error!("Wrong operands used for {}", self.assembly()),
        }
        16
    }

    inst_metadata!(2, "22 *1 *2", "LD (*2*1),HL");
}

pub struct _0x23 {}
impl Instruction for _0x23 {
    // inc hl
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        RegisterOperations::inc_register_pair((&mut components.registers.h, &mut components.registers.l), &mut components.registers.f);
        6
    }

    inst_metadata!(0, "23", "INC HL");
}


pub struct _0x29 {}
impl Instruction for _0x29 {
    // The value of HL is added to HL.
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        RegisterOperations::dbl_register_pair((&mut components.registers.h, &mut components.registers.l),  &mut components.registers.f);
        11
    }

    inst_metadata!(0, "29", "ADD HL,HL");
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

pub struct _0x2D {}
impl Instruction for _0x2D {
    // dec l
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        RegisterOperations::dec(&mut components.registers.l, &mut components.registers.f);
        4
    }

    inst_metadata!(0, "2D", "DEC L");
}

pub struct _0x2F {}
impl Instruction for _0x2F {
    // Contents of A are inverted
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        components.registers.a.set(0xFF - components.registers.a.get());
        4
    }

    inst_metadata!(0, "2F", "CPL");
}


// #30 to 3F


pub struct _0x30 {}
impl Instruction for _0x30 {
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        // If the carry flag is unset, the signed value d is added to PC. The jump is measured from the start of the instruction opcode.
        match operands {
            Operands::One(op1) => {
                if components.registers.f.get_carry() == FlagValue::Unset {
                    let jump_val = signed(op1);
                    let val = components.registers.pc.get().wrapping_add(jump_val as u16);
                    components.registers.pc.set(val);
                    return 12;
                }
            }
            _ => error!("Wrong operands used for {}", self.assembly()),
        }
        7
    }

    inst_metadata!(1, "30 *1", "JR NC,*1");
}

pub struct _0x31 {}
impl Instruction for _0x31 {
    // load nn into sp
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        match operands {
            Operands::Two(op1, op2) => {
                components.registers.sp.set(combine_to_double_byte(op2, op1) as usize);
            }
            _ => error!("Wrong operands used for {}", self.assembly()),
        }
        10
    }

    inst_metadata!(2, "31 *1 *2", "LD SP,*2*1");
}

pub struct _0x32 {}
impl Instruction for _0x32 {
    // Stores A into the memory location pointed to by nn.
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        match operands {
            Operands::Two(op1, op2) => {
                RegisterOperations::ld_addr_from_value_with_register(&mut components.mem, combine_to_double_byte(op2, op1), &components.registers.a);
            }
            _ => error!("Wrong operands used for {}", self.assembly()),
        }
        13
    }

    inst_metadata!(2, "32 *1 *2", "LD (*2*1),A");
}

pub struct _0x36 {}
impl Instruction for _0x36 {
    // Loads n into (HL).
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        match operands {
            Operands::One(value) => {
                RegisterOperations::ld_addr_from_reg_pair_with_value(&mut components.mem,(&mut components.registers.h, &mut components.registers.l), value);
            }
            _ => error!("Wrong operands used for {}", self.assembly()),
        }
        10
    }

    inst_metadata!(1, "36 *1", "LD (HL),*1");
}

pub struct _0x3A {}
impl Instruction for _0x3A {
    // Loads the value pointed to by nn into A.
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        match operands {
            Operands::Two(op1, op2) => {
                RegisterOperations::ld_register_from_addr(&components.mem, &mut components.registers.a, combine_to_double_byte(op2, op1));
            }
            _ => error!("Wrong operands used for {}", self.assembly()),
        }
        13
    }

    inst_metadata!(2, "3A *1 *2", "LD A,(*2*1)");
}

pub struct _0x3C {}
impl Instruction for _0x3C {
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        RegisterOperations::inc(&mut components.registers.a, &mut components.registers.f);
        4
    }

    inst_metadata!(0, "3C", "INC A");
}

pub struct _0x3E {}
impl Instruction for _0x3E {
    // load nn into hl
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        match operands {
            Operands::One(value) => {
                RegisterOperations::ld_register_with_value(&mut components.registers.a, value);
            }
            _ => error!("Wrong operands used for {}", self.assembly()),
        }
        7
    }

    inst_metadata!(1, "3E *1", "LD A,*1");
}


// #40 to 4F

pub struct _0x41 {}
impl Instruction for _0x41 {
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        // The contents of C are loaded into B.
        RegisterOperations::ld_register_from_register(&components.registers.c, &mut components.registers.b);
        4
    }

    inst_metadata!(0, "41", "LD B,C");
}

pub struct _0x47 {}
impl Instruction for _0x47 {
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        // The contents of A are loaded into B.
        RegisterOperations::ld_register_from_register(&components.registers.a, &mut components.registers.b);
        4
    }

    inst_metadata!(0, "47", "LD B,A");
}

pub struct _0x4C {}
impl Instruction for _0x4C {
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        // Contents of h are loaded into c
        RegisterOperations::ld_register_from_register(&components.registers.h, &mut components.registers.c);
        4
    }

    inst_metadata!(0, "4C", "LD C,H");
}

pub struct _0x4E {}
impl Instruction for _0x4E {
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        let reg = &mut components.registers;
        RegisterOperations::ld_register_from_addr_with_register_pair(&components.mem, &mut reg.c, (&reg.h, &reg.l));
        7
    }

    inst_metadata!(0, "4E", "LD C,(HL)");
}



// #50 to 5E

// ld d,(hl)
pub struct _0x56 {}
impl Instruction for _0x56 {
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        let reg = &mut components.registers;
        RegisterOperations::ld_register_from_addr_with_register_pair(&components.mem, &mut reg.d, (&reg.h, &reg.l));
        7
    }

    inst_metadata!(0, "56", "LD D,(HL)");
}

pub struct _0x5E {}
impl Instruction for _0x5E {
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        let reg = &mut components.registers;
        RegisterOperations::ld_register_from_addr_with_register_pair(&components.mem, &mut reg.e, (&reg.h, &reg.l));
        7
    }

    inst_metadata!(0, "5E", "LD E,(HL)");
}



// #60 to 6F

pub struct _0x67 {}
impl Instruction for _0x67 {
    // The contents of A are loaded into H.
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        RegisterOperations::ld_register_from_register(&components.registers.a, &mut components.registers.h);
        4
    }

    inst_metadata!(0, "67", "LD H,A");
}

pub struct _0x6F {}
impl Instruction for _0x6F {
    // The contents of A are loaded into L.
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        RegisterOperations::ld_register_from_register(&components.registers.a, &mut components.registers.l);
        4
    }

    inst_metadata!(0, "6F", "LD L,A");
}

// #70 to 7F

//The contents of B are loaded into (HL).
pub struct _0x70 {}
impl Instruction for _0x70 {
    // The contents of B are loaded into (HL).
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        RegisterOperations::ld_addr_from_reg_pair_with_register(&mut components.mem, (&components.registers.h, &components.registers.l), &components.registers.b);
        7
    }

    inst_metadata!(0, "70", "LD (HL),B");
}

pub struct _0x71 {}
impl Instruction for _0x71 {
    // The contents of C are loaded into (HL).
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        RegisterOperations::ld_addr_from_reg_pair_with_register(&mut components.mem, (&components.registers.h, &components.registers.l), &components.registers.c);
        7
    }

    inst_metadata!(0, "71", "LD (HL),C");
}


pub struct _0x72 {}
impl Instruction for _0x72 {
    // The contents of D are loaded into (HL).
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        RegisterOperations::ld_addr_from_reg_pair_with_register(&mut components.mem, (&components.registers.h, &components.registers.l), &components.registers.d);
        7
    }

    inst_metadata!(0, "72", "LD (HL),D");
}

pub struct _0x73 {}
impl Instruction for _0x73 {
    // The contents of E are loaded into (HL).
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        RegisterOperations::ld_addr_from_reg_pair_with_register(&mut components.mem, (&components.registers.h, &components.registers.l), &components.registers.e);
        7
    }

    inst_metadata!(0, "73", "LD (HL),E");
}

pub struct _0x77 {}
impl Instruction for _0x77 {
    // The contents of A are loaded into (HL).
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        RegisterOperations::ld_addr_from_reg_pair_with_register(&mut components.mem, (&components.registers.h, &components.registers.l), &components.registers.a);
        7
    }

    inst_metadata!(0, "77", "LD (HL),A");
}

pub struct _0x78 {}
impl Instruction for _0x78 {
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        RegisterOperations::ld_register_from_register(&components.registers.b, &mut components.registers.a);
        4
    }

    inst_metadata!(0, "78", "LD A,B");
}

pub struct _0x79 {}
impl Instruction for _0x79 {
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        RegisterOperations::ld_register_from_register(&components.registers.c, &mut components.registers.a);
        4
    }

    inst_metadata!(0, "79", "LD A,C");
}

pub struct _0x7C {}
impl Instruction for _0x7C {
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        RegisterOperations::ld_register_from_register(&components.registers.h, &mut components.registers.a);
        4
    }

    inst_metadata!(0, "7C", "LD A,H");
}

pub struct _0x7D {}
impl Instruction for _0x7D {
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        RegisterOperations::ld_register_from_register(&components.registers.l, &mut components.registers.a);
        4
    }

    inst_metadata!(0, "7D", "LD A,L");
}

pub struct _0x7E {}
impl Instruction for _0x7E {
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        RegisterOperations::ld_register_from_addr_with_register_pair(&components.mem, &mut components.registers.a, (&components.registers.h, &components.registers.l));
        7
    }

    inst_metadata!(0, "7E", "LD A,(HL)");
}



// #A0 to AF


pub struct _0xA9 {}
impl Instruction for _0xA9 {
    // Bitwise XOR on A with C.
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        let registers = &mut components.registers;
        registers.a.xor(&registers.c, &mut registers.f);
        4
    }

    inst_metadata!(0, "A9", "XOR C");
}


pub struct _0xAF {}
impl Instruction for _0xAF {
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        components.registers.a.xor_a(&mut components.registers.f);
        4
    }

    inst_metadata!(0, "AF", "XOR A");
}



// #B0 to BF


pub struct _0xB6 {}
impl Instruction for _0xB6 {
    // OR a with (hl)
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        components.registers.a.xor_address_from_reg_pair(&components.mem, (&components.registers.h, &components.registers.l), &mut components.registers.f);
        7
    }

    inst_metadata!(0, "B6", "OR (HL)");
}


pub struct _0xB7 {}
impl Instruction for _0xB7 {
    // Bitwise OR on A with A.
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        components.registers.a.xor_address_from_reg_pair(&components.mem, (&components.registers.h, &components.registers.l), &mut components.registers.f);
        4
    }

    inst_metadata!(0, "B7", "OR A");
}

pub struct _0xBB {}
impl Instruction for _0xBB {
    // Subtracts E from A and affects flags according to the result. A is not modified.
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        components.registers.a.compare_reg(&components.registers.e, &mut components.registers.f);
        4
    }

    inst_metadata!(0, "BB", "CP E");
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

pub struct _0xC8 {}
impl Instruction for _0xC8 {
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        // if zero flag is set, pop sp value onto pc
        if components.registers.f.get_zero() == FlagValue::Set {
            components.registers.pc.set(components.registers.sp.pop(&components.mem));
            return 11;
        }
        5
    }

    inst_metadata!(0, "C8", "RET Z");
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

pub struct _0xCD {}
impl Instruction for _0xCD {
    
    // The current PC value plus three is pushed onto the stack, then is loaded with nn.
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16{
        if let Operands::Two(low, high) = operands {
            RegisterOperations::call(utils::combine_to_double_byte(high, low), &mut components.registers.sp, &mut components.registers.pc, &mut components.mem);
        }
        17
    }

    inst_metadata!(2, "CD", "CALL *2*1");
}


// #D0 to DF

pub struct _0xC1 {}
impl Instruction for _0xC1 {
    // The memory location pointed to by SP is stored into B and SP is incremented. 
    // The memory location pointed to by SP is stored into C and SP is incremented again.   
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        RegisterOperations::pop_register_pair((&mut components.registers.b, &mut components.registers.c), &mut components.registers.sp, &mut components.mem);
        10
    }

    inst_metadata!(0, "D1", "POP BC");
}

pub struct _0xD1 {}
impl Instruction for _0xD1 {
    // The memory location pointed to by SP is stored into E and SP is incremented. 
    // The memory location pointed to by SP is stored into D and SP is incremented again.   
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        RegisterOperations::pop_register_pair((&mut components.registers.d, &mut components.registers.e), &mut components.registers.sp, &mut components.mem);
        10
    }

    inst_metadata!(0, "D1", "POP DE");
}

pub struct _0xD5 {}
impl Instruction for _0xD5 {
    // Push contents of H and L onto stack.
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        RegisterOperations::push_register_pair((&components.registers.d, &components.registers.e), &mut components.registers.sp, &mut components.mem);
        11
    }

    inst_metadata!(0, "D5", "PUSH DE");
}

pub struct _0xD6 {}
impl Instruction for _0xD6 {
    // Subtract n from A
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16{
        if let Operands::One(value) = operands {
            components.registers.a.sub_value(value, &mut components.registers.f);
        } else {
            panic!("Wrong operand for {}", self.assembly());
        }
        17
    }

    inst_metadata!(1, "D6 *1", "SUB *1");
}


pub struct _0xD8 {}
impl Instruction for _0xD8 {

    // //If the carry flag is set, the top stack entry is popped into PC.
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16{
        if components.registers.f.get_carry() == FlagValue::Set {
            components.registers.pc.set(components.registers.sp.pop(&components.mem));
            return 11;
        }
        5
    }

    inst_metadata!(0, "D8", "RET C");
}
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


pub struct _0xDE {}
impl Instruction for _0xDE {
    //Subtracts n and the carry flag from A.
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16{
        if let Operands::One(value) = operands {
            components.registers.a.sub_value_and_carry(value, &mut components.registers.f);
        } else {
            panic!("Wrong operand for {}", self.assembly());
        }
        7
    }

    inst_metadata!(1, "DE *1", "SBC A,*1");
}

// #E0 to EF

pub struct _0xE5 {}
impl Instruction for _0xE5 {

    // Push contents of H and L onto stack.
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        RegisterOperations::push_register_pair((&components.registers.h, &components.registers.h), &mut components.registers.sp, &mut components.mem);
        11
    }

    inst_metadata!(0, "E5", "PUSH HL");
}

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

pub struct _0xEB {}
impl Instruction for _0xEB {
    // Exchanges the 16-bit contents of AF and AF'.
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        let mut registers = &mut components.registers;
        let d_val = registers.d.get();
        let e_val = registers.e.get();
        registers.d.set(registers.h.get());
        registers.e.set(registers.l.get());
        registers.h.set(d_val);
        registers.l.set(e_val);
        4
    }

    inst_metadata!(0, "EB", "EX DE,HL");
}


// #F0 to FF

pub struct _0xF0 {}
impl Instruction for _0xF0 {
    
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        if components.registers.f.get_sign() == FlagValue::Unset {
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
        components.registers.iff1 = false;
        components.registers.iff2 = false;
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


pub struct _0xF8 {}
impl Instruction for _0xF8 {
    // If the sign flag is set, the top stack entry is popped into PC.
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        if components.registers.f.get_sign() == FlagValue::Set {
            components.registers.pc.set(components.registers.sp.pop(&components.mem));
            return 11;
        }
        5
    }

    inst_metadata!(0, "F8", "RET M");
}


pub struct _0xFB {}
impl Instruction for _0xFB {
    // Sets both interrupt flip-flops, thus allowing maskable interrupts to occur. 
    // An interrupt will not occur until after the immediately following instruction.
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        components.registers.iff1 = true;
        components.registers.iff2 = true;
        4
    }

    inst_metadata!(0, "FB", "EI");
}


pub struct _0xFE {}
impl Instruction for _0xFE {
    // Subtracts n from A and affects flags according to the result. 
    // A is not modified.
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        if let Operands::One(val) = operands {
            &components.registers.a.compare_val(val, &mut components.registers.f);
        }
        7
    }

    inst_metadata!(1, "FE", "CP *1");
}


// Tests

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::{instruction_set::{Instruction, Operands, InstructionSet, self, basic::{_0xC9, _0xC5, _0xC2, _0xF5}}, memory::{Memory, Registers, AddressBus, DataBus, FlagValue, Register}, runtime::{Runtime, RuntimeComponents}, utils::split_double_byte};

    use super::{_0x04, _0x05, _0x07, _0xE6, _0x0B, _0xDE};

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

    #[test]
    fn sbc_a_n() {
        let mut components = runtime_components();
        components.registers.a.set(0x11);
        components.registers.f.set(0x01);
        let cycles = _0xDE {}.execute(&mut components, Operands::One(0x01));
        assert_eq!(cycles, 7);
        assert_eq!(components.registers.a.get(), 0x0F);

        components.registers.a.set(0x12);
        components.registers.f.set(0x00);
        let cycles = _0xDE {}.execute(&mut components, Operands::One(0x01));
        assert_eq!(cycles, 7);
        assert_eq!(components.registers.a.get(), 0x11);
    }


}

