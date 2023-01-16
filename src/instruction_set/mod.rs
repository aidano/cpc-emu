pub mod basic;
pub mod extended;

use crate::{memory::{Memory, Registers, DataBus, AddressBus}, runtime::{Runtime, RuntimeComponents}};

use std::collections::HashMap;
use log::{debug, error, log_enabled, info, Level};

use self::{extended::{_0xED49, _0xED78}, basic::{_0x00, _0xC3, _0x01, _0xC5, _0xC9, _0x4C, _0xC0, _0xF3, _0x06, _0x78, _0xF5}};

#[derive(Debug)]
pub enum Operands {
    None,
    One(u8),
    Two(u8, u8)
}


pub trait Instruction {
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u8;
    fn operand_count(&self) -> u8;
    fn op_code(&self) -> u8;
    fn machine_code(&self) -> &str;
    fn assembly(&self) -> &str;
}

pub struct InstructionSet {
    basic_instructions: HashMap<u8, Box<dyn Instruction>>,
    extended_instructions: HashMap<u8, Box<dyn Instruction>>
}

impl InstructionSet {
    pub fn default() -> InstructionSet {
        let mut basic_instruction_set: HashMap<u8, Box<dyn Instruction>> = HashMap::new();
        basic_instruction_set.insert(0x0, Box::new(_0x00 {}));
        basic_instruction_set.insert(0x1, Box::new(_0x01 {}));
        basic_instruction_set.insert(0xC3, Box::new(_0xC3 {}));
        basic_instruction_set.insert(0xC5, Box::new(_0xC5 {}));
        basic_instruction_set.insert(0xC9, Box::new(_0xC9 {}));
        basic_instruction_set.insert(0x4C, Box::new(_0x4C {}));
        basic_instruction_set.insert(0xC0, Box::new(_0xC0 {}));
        basic_instruction_set.insert(0xF3, Box::new(_0xF3 {}));
        basic_instruction_set.insert(0x06, Box::new(_0x06 {}));
        basic_instruction_set.insert(0x78, Box::new(_0x78 {}));
        basic_instruction_set.insert(0xF5, Box::new(_0xF5 {}));


        let mut extended_instruction_set: HashMap<u8, Box<dyn Instruction>> = HashMap::new();
        extended_instruction_set.insert(0x49, Box::new(_0xED49 {}));
        extended_instruction_set.insert(0x78, Box::new(_0xED78 {}));
        
        // and so on...
        
        InstructionSet {basic_instructions: basic_instruction_set, extended_instructions: extended_instruction_set }
    }

    pub fn instruction_for(&self, byte: u8) -> &Box<dyn Instruction> {
        return &*self.basic_instructions.get(&byte).unwrap_or_else( || {
            // Stop immediately so that the instruction can be identified and implemented.
            error!("Unimplemented basic instruction: #{:02X?}", byte);
            std::process::exit(1);
        });
    }

    pub fn extended_instruction_for(&self, byte: u8) -> &Box<dyn Instruction> {
        return self.extended_instructions.get(&byte).unwrap_or_else(|| {
            // Stop immediately so that the instruction can be identified and implemented.
            error!("Unimplemented extended instruction: #{:02X?}", byte);
            std::process::exit(1);
        });
    }
}