pub mod basic;
pub mod extended;
pub mod index;
pub mod bit;

use crate::{memory::{Memory, Registers, DataBus, AddressBus}, runtime::{Runtime, RuntimeComponents}};

use std::collections::HashMap;
use log::{debug, error, log_enabled, info, Level};

use self::{extended::*, basic::*, index::*, bit::*};

#[derive(Debug)]
pub enum Operands {
    None,
    One(u8),
    Two(u8, u8)
}


pub trait Instruction {
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16;
    fn operand_count(&self) -> u8;
    fn machine_code(&self) -> &str;
    fn assembly(&self) -> &str;
}

pub struct InstructionSet {
    basic_instructions: HashMap<u8, Box<dyn Instruction>>,
    extended_instructions: HashMap<u8, Box<dyn Instruction>>,
    index_instructions: HashMap<u8, Box<dyn Instruction>>,
    bit_instructions: HashMap<u8, Box<dyn Instruction>>
}

macro_rules! instruction_set_map {
    ($( $key: expr => $val: expr),*) => {{
        let mut map: HashMap<u8, Box<dyn Instruction>> = HashMap::new();
        $( map.insert($key, Box::new($val)); )*
        map
    }}
}

impl InstructionSet {
    pub fn default() -> InstructionSet {

        let mut basic_instruction_set = instruction_set_map![
            0x00 => _0x00{},
            0x01 => _0x01{},
            0xC3 => _0xC3{},
            0xC5 => _0xC5{},
            0xC9 => _0xC9{},
            0x4C => _0x4C{},
            0xC0 => _0xC0{},
            0xF3 => _0xF3{},
            0x06 => _0x06{},
            0x78 => _0x78{},
            0xF5 => _0xF5{},
            0xE6 => _0xE6{},
            0x21 => _0x21{},
            0x20 => _0x20{},
            0x2B => _0x2B{},
            0x7E => _0x7E{},
            0x04 => _0x04{},
            0x05 => _0x05{},
            0x0D => _0x0D{},
            0xF2 => _0xF2{},
            0x18 => _0x18{},
            0x11 => _0x11{},
            0xD9 => _0xD9{},
            0x36 => _0x36{},
            0xAF => _0xAF{},
            0x08 => _0x08{},
            0x31 => _0x31{},
            0xE5 => _0xE5{},
            0xD5 => _0xD5{},
            0xCD => _0xCD{},
            0x2D => _0x2D{},
            0x77 => _0x77{},
            0x3E => _0x3E{},
            0x32 => _0x32{},
            0x3A => _0x3A{},
            0x23 => _0x23{},
            0x10 => _0x10{},
            0x47 => _0x47{},
            0x0E => _0x0E{},
            0xA9 => _0xA9{},
            0x71 => _0x71{},
            0x1A => _0x1A{},
            0x13 => _0x13{},
            0xEB => _0xEB{},
            0x79 => _0x79{},
            0x2F => _0x2F{},
            0x07 => _0x07{},
            0xB6 => _0xB6{},
            0x22 => _0x22{},
            0x67 => _0x67{},
            0x6F => _0x6F{},
            0x19 => _0x19{},
            0x7D => _0x7D{},
            0xD6 => _0xD6{},
            0x7C => _0x7C{},
            0xDE => _0xDE{},
            0xD8 => _0xD8{},
            0x0C => _0x0C{},
            0x4E => _0x4E{},
            0x5E => _0x5E{},
            0x56 => _0x56{},
            0xBB => _0xBB{},
            0xB7 => _0xB7{},
            0xC8 => _0xC8{},
            0x30 => _0x30{},
            0xFB => _0xFB{},
            0xD1 => _0xD1{},
            0xC1 => _0xC1{},
            0x70 => _0x70{},
            0x73 => _0x73{},
            0x72 => _0x72{},
            0x09 => _0x09{},
            0x3C => _0x3C{},
            0x29 => _0x29{},
            0xFE => _0xFE{},
            0x41 => _0x41{},
            0xF8 => _0xF8{}
        ];

        let mut extended_instruction_set = instruction_set_map![
            0x49 => _0xED49{},
            0x78 => _0xED78{},
            0x79 => _0xED79{},
            0x56 => _0xED56{},
            0x46 => _0xED46{},
            0xB0 => _0xEDB0{},
            0x5B => _0xED5B{}
        ];

        let mut index_instruction_set = instruction_set_map![
            0xE5 => _0xDDE5{},
            0xE1 => _0xDDE1{}
        ];

        let mut bit_instruction_set = instruction_set_map![
            0x38 => _0xCB38{}
        ];

        InstructionSet { 
            basic_instructions: basic_instruction_set,
            extended_instructions: extended_instruction_set,
            index_instructions: index_instruction_set,
            bit_instructions: bit_instruction_set
        }

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

    pub fn index_instruction_for(&self, byte: u8) -> &Box<dyn Instruction> {
        return self.index_instructions.get(&byte).unwrap_or_else(|| {
            // Stop immediately so that the instruction can be identified and implemented.
            error!("Unimplemented index instruction: #{:02X?}", byte);
            std::process::exit(1);
        });
    }

    pub fn bit_instruction_for(&self, byte: u8) -> &Box<dyn Instruction> {
        return self.bit_instructions.get(&byte).unwrap_or_else(|| {
            // Stop immediately so that the instruction can be identified and implemented.
            error!("Unimplemented bit instruction: #{:02X?}", byte);
            std::process::exit(1);
        });
    }


}