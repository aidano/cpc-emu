pub mod basic;
pub mod extended;

use crate::memory::{Memory, Registers};
use self::basic::{InstNOP, InstLdBCnn};

use std::collections::HashMap;
use log::{debug, error, log_enabled, info, Level};

#[derive(Debug)]
pub enum Operands {
    None,
    One(u8),
    Two(u8, u8)
}


pub trait Instruction {
    fn execute(&self, mem: &mut Memory, reg: &mut Registers, operands: Operands);
    fn operand_count(&self) -> u8;
    fn op_code(&self) -> u8;
}

pub struct InstructionSet {
    instructions: HashMap<u8, Box<dyn Instruction>>
}

impl InstructionSet {
    pub fn default() -> InstructionSet {
        let mut instruction_set: HashMap<u8, Box<dyn Instruction>> = HashMap::new();
        instruction_set.insert(0x0, Box::new(InstNOP {}));
        instruction_set.insert(0x1, Box::new(InstLdBCnn {}));
        // and so on...
        
        InstructionSet {instructions: instruction_set }
    }

    pub fn instruction_for(&self, byte: u8) -> &Box<dyn Instruction> {
        return &*self.instructions.get(&byte).unwrap_or_else( || {
            // Stop immediately so that the instruction can be identified and implemented.
            error!("Unimplemented instruction: #{:02X?}", byte);
            std::process::exit(1);
        });
    }

    pub fn extended_instruction_for(&self, byte: u8) -> &Box<dyn Instruction> {
        return self.instructions.get(&byte).unwrap_or_else(|| {
            // Stop immediately so that the instruction can be identified and implemented.
            error!("Unimplemented instruction: #{:02X?}", byte);
            std::process::exit(1);
        });
    }
}