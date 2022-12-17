///////////////////////
//
// Runtime components - memory, registers, instruction set 
//
///////////////////////
use crate::memory::{Memory, Registers};
use crate::instruction_set::{InstructionSet, Instruction, Operands};

use log::{debug, error, log_enabled, info, Level};

pub struct Runtime {
    mem: Memory,
    registers: Registers,
    instruction_set: InstructionSet
}

impl Runtime {

    pub fn default() -> Runtime {
        let mem = Memory::default();
        let regs = Registers::default();
        let instruction_set = InstructionSet::default();
        Runtime::new(mem, regs, instruction_set)
    }

    fn new(mem: Memory, regs: Registers, instruction_set: InstructionSet) -> Runtime {
        Runtime { mem, registers: regs, instruction_set: instruction_set }
    }

    fn execute(&mut self, inst: Box<dyn Instruction>, operands: Operands) {
        inst.execute(&mut self.mem, &mut self.registers, operands)
    }

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
            self.mem.locations[i] = bytes[i];
            i += 1;
        }
    }

    fn load_expansion_rom(&mut self, bytes: &[u8]) {
        let mut i = 0xC000;
        while i < 0xFFFF {
            self.mem.locations[i] = bytes[i-0xC000];
            i += 1;
        }
    }


    pub fn run(&mut self, start_address: u32) {
        let mut program_counter = start_address as usize;
        loop {
            let instruction_byte = self.mem.locations[program_counter];
            
            //let instruction:  
            let instruction:&Box<dyn Instruction>;
            match instruction_byte {
                0xED => {
                    let extended_instruction_byte = self.mem.locations[program_counter];
                    program_counter += 1;
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
                    let operand1 = self.mem.locations[program_counter];
                    operands = Operands::One(operand1);
                }
                2 => {
                    program_counter += 1;
                    let operand1 = self.mem.locations[program_counter];
                    program_counter += 1;
                    let operand2 = self.mem.locations[program_counter];
                    operands = Operands::Two(operand1, operand2);
                }
                _ => {
                    operands = Operands::None;
                    error!("Wrong op count returned for instruction at {}", program_counter);
                    assert!(false);
                }
            }
            program_counter += 1;
            let mem = &mut self.mem;
            let registers = &mut self.registers;
            instruction.execute(mem, registers, operands)
        } 
    }
}

