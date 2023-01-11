use std::ops::Add;

///////////////////////
//
// Runtime components - memory, registers, instruction set 
//
///////////////////////
use crate::memory::{Memory, Registers, AddressBus, DataBus};
use crate::instruction_set::{InstructionSet, Instruction, Operands};

use log::{debug, error, log_enabled, info, Level};

pub struct RuntimeComponents {
    pub mem: Memory,
    pub registers: Registers,
    pub address_bus: AddressBus,
    pub data_bus: DataBus
}

impl RuntimeComponents {
    pub fn default() -> RuntimeComponents {
        let mem = Memory::default();
        let registers = Registers::default();
        let address_bus = AddressBus { value: 0 };
        let data_bus = DataBus { value: 0};
        RuntimeComponents { mem, registers, address_bus, data_bus }
    }
}

pub struct Runtime {
    instruction_set: InstructionSet,
    pub components: RuntimeComponents
}

impl Runtime {

    pub fn default() -> Runtime {
        Runtime::new(InstructionSet::default(), RuntimeComponents::default())
    }

    fn new(instruction_set: InstructionSet, components: RuntimeComponents) -> Runtime {
        Runtime { instruction_set, components }
    }

    // fn execute(&mut self, inst: Box<dyn Instruction>, operands: Operands) {
    //     inst.execute(&mut self, operands);
    // }

    pub fn load_rom_from_bytes(&mut self, bytes: &[u8]) {
        match bytes.len() {
            0x4000 => self.load_os_rom(bytes),
            0x8000 => {
                self.load_os_rom(&bytes[..=0x3FFF]);
                self.load_expansion_rom(&bytes[0x4000..]);
            },
            _ => {
                error!("Unexpected ROM size: {}", bytes.len());
                assert!(false);
            }
        }
    }

    fn load_os_rom(&mut self, bytes: &[u8]) {
        let mut i = 0;
        while i < 0x4000 {
            self.components.mem.locations[i] = bytes[i];
            i += 1;
        }
    }

    fn load_expansion_rom(&mut self, bytes: &[u8]) {
        let mut i = 0xC000;
        while i < 0xFFFF {
            self.components.mem.locations[i] = bytes[i-0xC000];
            i += 1;
        }
    }


    pub fn run(&mut self, start_address: u32) {
        let mut program_counter = start_address as usize;
        loop {
            let instruction_byte = self.components.mem.locations[program_counter];
            
            //let instruction:  
            let instruction:&Box<dyn Instruction>;
            match instruction_byte {
                0xED => {
                    program_counter += 1;
                    let extended_instruction_byte = self.components.mem.locations[program_counter];
                    instruction = self.instruction_set.extended_instruction_for(extended_instruction_byte);
                },
                non_extended_byte => {
                    instruction = self.instruction_set.instruction_for(non_extended_byte)
                }
            };
            
            let op_count = instruction.operand_count();
            
            let operands: Operands;
            match op_count {
                0 => operands = Operands::None,
                1 => {
                    program_counter += 1;
                    let operand1 = self.components.mem.locations[program_counter];
                    operands = Operands::One(operand1);
                }
                2 => {
                    program_counter += 1;
                    let operand1 = self.components.mem.locations[program_counter];
                    program_counter += 1;
                    let operand2 = self.components.mem.locations[program_counter];
                    operands = Operands::Two(operand1, operand2);
                }
                _ => {
                    operands = Operands::None;
                    error!("Wrong op count returned for instruction at {}", program_counter);
                    assert!(false);
                }
            }
            program_counter += 1;
            let mem = &mut self.components.mem;
            let registers = &mut self.components.registers;
            instruction.execute(&mut self.components, operands);
        } 
    }
}

