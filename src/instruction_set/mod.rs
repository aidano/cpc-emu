pub mod basic;
pub mod extended;

use crate::{memory::{Memory, Registers, DataBus, AddressBus}, runtime::{Runtime, RuntimeComponents}};
use self::{basic::{InstNOP, InstLdBCnn, InstJp, InstPushBC}, extended::InstOutCC};

use std::collections::HashMap;
use log::{debug, error, log_enabled, info, Level};

#[derive(Debug)]
pub enum Operands {
    None,
    One(u8),
    Two(u8, u8)
}


pub trait Instruction {
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands);
    fn operand_count(&self) -> u8;
    fn op_code(&self) -> u8;
}

pub struct InstructionSet {
    basic_instructions: HashMap<u8, Box<dyn Instruction>>,
    extended_instructions: HashMap<u8, Box<dyn Instruction>>
}

impl InstructionSet {
    pub fn default() -> InstructionSet {
        let mut basic_instruction_set: HashMap<u8, Box<dyn Instruction>> = HashMap::new();
        basic_instruction_set.insert(0x0, Box::new(InstNOP {}));
        basic_instruction_set.insert(0x1, Box::new(InstLdBCnn {}));
        basic_instruction_set.insert(0xC3, Box::new(InstJp {}));
        basic_instruction_set.insert(0xC5, Box::new(InstPushBC {}));

        let mut extended_instruction_set: HashMap<u8, Box<dyn Instruction>> = HashMap::new();
        extended_instruction_set.insert(0x49, Box::new(InstOutCC {}));
        
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