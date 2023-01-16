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
        let data_bus = DataBus { };
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


    pub fn run(&mut self, start_address: u16) {
        self.components.registers.pc.set(start_address);
        loop {
            let pc = self.components.registers.pc.get();
            let instruction_byte = self.components.mem.locations[self.components.registers.pc.get() as usize];
            
            
            let instruction:&Box<dyn Instruction>;
            match instruction_byte {
                0xED => {
                    self.components.registers.pc.inc();
                    let extended_instruction_byte = self.components.mem.locations[self.components.registers.pc.get() as usize];
                    instruction = self.instruction_set.extended_instruction_for(extended_instruction_byte);
                },
                non_extended_byte => {
                    instruction = self.instruction_set.instruction_for(non_extended_byte);
                }
            };
            
            let inst_machine_code: String;
            let inst_assembly: String;

            let op_count = instruction.operand_count();
            let operands: Operands;
            match op_count {
                0 => { 
                    operands = Operands::None;
                    inst_machine_code = instruction.machine_code().to_string();
                    inst_assembly = instruction.assembly().to_string();
                }
                1 => {
                    self.components.registers.pc.inc();
                    let operand1 = self.components.mem.locations[self.components.registers.pc.get() as usize];
                    operands = Operands::One(operand1);
                    let op1 = format!("{:0>2X}", &operand1);
                    inst_machine_code = instruction.machine_code().replace("*1", &op1);
                    inst_assembly = instruction.assembly().replace("*1", &op1);
                }
                2 => {
                    self.components.registers.pc.inc();
                    let operand1 = self.components.mem.locations[self.components.registers.pc.get() as usize];
                    self.components.registers.pc.inc();
                    let operand2 = self.components.mem.locations[self.components.registers.pc.get() as usize];
                    operands = Operands::Two(operand1, operand2);
                    let op1 = format!("{:0>2X}", &operand1);
                    let op2 = format!("{:0>2X}", &operand2);
                    inst_machine_code = instruction.machine_code().replace("*1", &op1).replace("*2", &op2);
                    inst_assembly = instruction.assembly().replace("*1", &op1).replace("*2", &op2);
                }
                _ => {
                    operands = Operands::None;
                    inst_machine_code = "".to_string();
                    inst_assembly = "".to_string();
                    error!("Wrong op count returned for instruction at {}", self.components.registers.pc.get());
                    assert!(false);
                }
            }
            self.components.registers.pc.inc();
            let mem = &mut self.components.mem;
            let registers = &mut self.components.registers;
            
            debug!("{:0>4X}\t{: <8}\t{}", pc, inst_machine_code, inst_assembly);
            instruction.execute(&mut self.components, operands);
        } 
    }
}

