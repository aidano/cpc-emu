///////////////////////
//
// Instructions 
//
///////////////////////

use log::{debug, error};

use crate::{memory::{Memory, Registers, FlagValue, AddressBus, DataBus}, utils::{combine_to_double_byte, split_double_byte, self}, runtime::{RuntimeComponents}};
use super::{Instruction, Operands};

// #00 to 0F
#[derive(Debug, Clone)]
pub struct InstNOP {}
impl Instruction for InstNOP {
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) {
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
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) {
        debug!("ld_bc {:?}", operands);
        match operands {
            Operands::Two(first, second) => {
                components.registers.b.set(first);
                components.registers.c.set(second);
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
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) {
        let reg = &mut components.registers;
        debug!("ld_(bc)_a {:?}", operands);
        let addr = combine_to_double_byte(reg.b.get(), reg.c.get());
        reg.a.set(components.mem.locations[addr as usize]);
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
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) {
        let reg = &mut components.registers;
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
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) {
        components.registers.b.set(components.registers.b.get() + 1);
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
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) {
        components.registers.b.set(components.registers.b.get() -1);
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
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) {
        debug!("ld_bc {:?}", operands);
        match operands {
            Operands::One(operand) => {
                components.registers.b.set(operand);
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
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) {
        let value = components.registers.a.get();
        let bit_7 = (value & 0x80) >> 7; // left-most bit (i.e. 128)
        components.registers.a.set((value << 1) | bit_7);
        match bit_7 {
            0 => components.registers.set_carry(FlagValue::Unset),
            1 => components.registers.set_carry(FlagValue::Set),
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
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) {
        let mut registers = &mut components.registers;
        let a_val = registers.a.get();
        let f_val = registers.f.get();
        registers.a.set(registers.a_.get());
        registers.f.set(registers.f_.get());
        registers.a_.set(a_val);
        registers.f.set(f_val);
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
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) {
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
    }

    fn operand_count(&self) -> u8 {
        0
    }

    fn op_code(&self) -> u8 {
        0x09
    }
}


// #10 to 1F










// #C0 to CF
pub struct InstJpNZ {}
impl Instruction for InstJpNZ {
    
    // Jump to address provided in operands if zero flag is set
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) {
        if components.registers.get_zero() ==  FlagValue::Unset {
            if let Operands::Two(high, low) = operands {
                components.registers.pc.set(utils::combine_to_double_byte(high, low));
            }
        }
    }

    fn operand_count(&self) -> u8 {
        2
    }

    fn op_code(&self) -> u8 {
        0xC2
    }
}

pub struct InstJp {}
impl Instruction for InstJp {
    
    // Jump to address provided in operands
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) {
        if let Operands::Two(high, low) = operands {
            components.registers.pc.set(utils::combine_to_double_byte(high, low));
        }
    }

    fn operand_count(&self) -> u8 {
        2
    }

    fn op_code(&self) -> u8 {
        0xC3
    }
}

pub struct InstPushBC {}
impl Instruction for InstPushBC {

    // Push contents of B and C onto stack.
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) {
        let bc = combine_to_double_byte(components.registers.b.get(), components.registers.c.get());
        components.registers.sp.push(&mut components.mem, bc);
    }

    fn operand_count(&self) -> u8 {
        0
    }

    fn op_code(&self) -> u8 {
        0xC5
    }
}


// Tests

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::{instruction_set::{Instruction, Operands, InstructionSet, basic::{InstJpNZ, InstPushBC}, self}, memory::{Memory, Registers, AddressBus, DataBus, FlagValue}, runtime::{Runtime, RuntimeComponents}, utils::split_double_byte};

    use super::{InstIncB, InstDecB, InstRlca};

    fn default_mem_reg() -> (Memory, Registers, AddressBus, DataBus) {
        return (Memory::default(), Registers::default(), AddressBus { value: 0 }, DataBus { value: 0 });
    }

    fn runtime_components_with_instructions(instruction_set: InstructionSet) -> RuntimeComponents {
        RuntimeComponents { mem: Memory::default(), registers: Registers::default(), address_bus: AddressBus { value: 0 }, data_bus: DataBus { value: 0 } }
    }

    #[test]
    fn inc_b() {
        let mut basic_instruction_set: HashMap<u8, Box<dyn Instruction>> = HashMap::new();
        basic_instruction_set.insert(0x0, Box::new(InstIncB {}));
        let mut components = runtime_components_with_instructions(InstructionSet { basic_instructions: basic_instruction_set, extended_instructions: HashMap::new() });
        
        assert!(components.registers.b.get() == 0);
        
        InstIncB {}.execute(&mut components, Operands::None);

        assert!(components.registers.b.get() == 1);
    }

    #[test]
    fn dec_b() {
        let mut basic_instruction_set: HashMap<u8, Box<dyn Instruction>> = HashMap::new();
        basic_instruction_set.insert(0x0, Box::new(InstDecB {}));
        let mut components = runtime_components_with_instructions(InstructionSet { basic_instructions: basic_instruction_set, extended_instructions: HashMap::new() });

        components.registers.b.set(1);
        assert!(components.registers.b.get() == 1);

        InstDecB {}.execute(&mut components, Operands::None);

        assert!(components.registers.b.get() == 0);
    }

    #[test]
    fn rlca_doubling() {
        // The contents of A are rotated left one bit position. 
        // The contents of bit 7 are copied to the carry flag and bit 0.
        let mut basic_instruction_set: HashMap<u8, Box<dyn Instruction>> = HashMap::new();
        basic_instruction_set.insert(0x0, Box::new(InstRlca {}));
        let instruction_set = InstructionSet { basic_instructions: basic_instruction_set, extended_instructions: HashMap::new() };
        let mut components = runtime_components_with_instructions(instruction_set);

        components.registers.a.set(1);

        InstRlca {}.execute(&mut components, Operands::None);
        assert!(components.registers.a.get() == 2);


        components.registers.a.set(35);

        InstRlca {}.execute(&mut components, Operands::None);
        assert!(components.registers.a.get() == 70);
    }


    #[test]
    fn rlca_overflow() {
        // The contents of A are rotated left one bit position. 
        // The contents of bit 7 are copied to the carry flag and bit 0.
        let mut basic_instruction_set: HashMap<u8, Box<dyn Instruction>> = HashMap::new();
        basic_instruction_set.insert(0x0, Box::new(InstRlca {}));
        let instruction_set = InstructionSet { basic_instructions: basic_instruction_set, extended_instructions: HashMap::new() };
        let mut components = runtime_components_with_instructions(instruction_set);

        components.registers.a.set(255);

        InstRlca {}.execute(&mut components, Operands::None);
        assert!(components.registers.a.get() == 255);

        components.registers.a.set(254);

        InstRlca {}.execute(&mut components, Operands::None);
        assert!(components.registers.a.get() == 253);
    }

    #[test]
    fn jpnz() {
        let mut basic_instruction_set: HashMap<u8, Box<dyn Instruction>> = HashMap::new();
        basic_instruction_set.insert(0x0, Box::new(InstJpNZ {}));
        let instruction_set = InstructionSet { basic_instructions: basic_instruction_set, extended_instructions: HashMap::new() };
        let mut components = runtime_components_with_instructions(instruction_set);
        components.registers.set_zero(FlagValue::Unset);
        InstJpNZ {}.execute(&mut components, Operands::Two(0xAA, 0xFF));
        assert!(components.registers.pc.get() == 0xAAFF);
    }

    #[test]
    fn push_bc() {
        let mut basic_instruction_set: HashMap<u8, Box<dyn Instruction>> = HashMap::new();
        basic_instruction_set.insert(0x0, Box::new(InstPushBC {}));
        let instruction_set = InstructionSet { basic_instructions: basic_instruction_set, extended_instructions: HashMap::new() };
        let mut components = runtime_components_with_instructions(instruction_set);

        components.registers.b.set(0xA);
        components.registers.c.set(0xB);
        InstPushBC {}.execute(&mut components, Operands::None);
        
        let value = components.registers.sp.pop(&components.mem);

        let (high, low) = split_double_byte(value);
        assert!(high == 0xA);
        assert!(low == 0xB);
    }



}

