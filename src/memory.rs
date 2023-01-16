use std::fmt;

use crate::utils::{split_double_byte, combine_to_double_byte};

pub struct Memory {
    pub locations: [u8; 0xFFFF]
}

impl Memory {
    pub fn default() -> Memory {
        Memory { locations: [0x01; 0xFFFF] }
    }
}

pub struct Register {
    name: String,
    value: u8
}

pub struct ProgramCounter {
    value: u16
}

impl ProgramCounter {
    pub fn set(&mut self, value: u16) {
        self.value = value;
    }

    pub fn get(&self) -> u16 {
        self.value
    }

    pub(crate) fn inc(&mut self) {
        self.value = self.value + 1;
    }

    pub(crate) fn dec(&mut self) {
        self.value = self.value - 1;
    }
}

pub struct StackPointer {
    location: usize
}

impl StackPointer {
    pub fn push(&mut self, memory: &mut Memory, value: u16) {
        let (high, low) = split_double_byte(value);
        self.location -= 1;
        memory.locations[self.location] = high;
        self.location -= 1;
        memory.locations[self.location] = low;
    }

    pub fn pop(&mut self, memory: &Memory) -> u16 {
        let low = memory.locations[self.location];
        self.location += 1;
        let high = memory.locations[self.location];
        self.location += 1;
        combine_to_double_byte(high, low)
    }
}

impl Register {
    pub fn set(&mut self, value: u8) {
        self.value = value;
    }

    pub fn get(&self) -> u8 {
        self.value
    }
}

impl fmt::Debug for Register {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Register")
        .field("name", &self.name)
        .field("value", &self.value)
        .finish()
    }
}

pub struct AddressBus {
    pub value: u16 // TODO: simple impl for now.
}

// TODO: This struct might actually represent both the address and the data bus, in which case the above struct can go away.
pub struct DataBus {}
impl DataBus {
    
    pub fn write(&self, port: u16, value: u8) {
        // stub for now
    }

    pub fn read(&self, port: u16) -> u8 {
        0xEF // dummy value for now
    }
}

pub struct Registers {
    pub a: Register,
    pub f: Register,
    pub b: Register,
    pub c: Register,
    pub d: Register,
    pub e: Register,
    pub h: Register,
    pub l: Register,
    pub a_: Register,
    pub f_: Register,
    pub b_: Register,
    pub c_: Register,
    pub d_: Register,
    pub e_: Register,
    pub h_: Register,
    pub l_: Register,

    pub pc: ProgramCounter,
    pub sp: StackPointer,
    pub maskable_interrupt_enabled: bool
}

#[derive(PartialEq)]
pub enum FlagValue {
    Set,
    Unset
}

impl Registers {
    pub fn default() -> Registers {
        Registers {
            a: Register {name: "a".to_string(), value: 0},
            f: Register {name: "f".to_string(), value: 0},
            b: Register {name: "b".to_string(), value: 0},
            c: Register {name: "c".to_string(), value: 0},
            d: Register {name: "d".to_string(), value: 0},
            e: Register {name: "e".to_string(), value: 0},
            h: Register {name: "h".to_string(), value: 0},
            l: Register {name: "l".to_string(), value: 0},
            a_: Register {name: "a'".to_string(), value: 0},
            f_: Register {name: "f'".to_string(), value: 0},
            b_: Register {name: "b'".to_string(), value: 0},
            c_: Register {name: "c'".to_string(), value: 0},
            d_: Register {name: "d'".to_string(), value: 0},
            e_: Register {name: "e'".to_string(), value: 0},
            h_: Register {name: "h'".to_string(), value: 0},
            l_: Register {name: "l'".to_string(), value: 0},
            pc: ProgramCounter { value: 0 }, // PC normally begins at start of memory
            sp: StackPointer { location: 0xFFFF }, // SP normally begins at the end of memory and moves down.
            maskable_interrupt_enabled: true
        }
    }

    //
    // Bit	    7	6	5	4	3	2	1	0
    // Position	S	Z	x	H	y	P/V	N	C
    //
    pub fn set_carry(&mut self, value: FlagValue) {
        match value {
            FlagValue::Set => self.f.value = self.f.value | 1,
            FlagValue::Unset => self.f.value = self.f.value & (255 - 1)
        }
    }

    pub fn set_half_carry(&mut self, value: FlagValue) {
        match value {
            FlagValue::Set => self.f.value = self.f.value | 16,
            FlagValue::Unset => self.f.value = self.f.value & (255 - 16)
        }
    }

    pub fn set_zero(&mut self, value: FlagValue) {
        self.f.value = match value {
            FlagValue::Set => self.f.value | 64,
            FlagValue::Unset => self.f.value & (255 - 64)
        }
    }

    pub fn get_zero(&mut self) -> FlagValue {
        match  self.f.value & 64 {
            64 => FlagValue::Set,
            0 => FlagValue::Unset,
            _ => panic!("Shouldn't happen")
        }
    }

    pub fn set_add_subtract(&mut self, value: FlagValue) {
        match value {
            FlagValue::Set => self.f.value = self.f.value | 2,
            FlagValue::Unset => self.f.value = self.f.value & (255 - 2)
        }
    }

    pub fn set_sign(&mut self, value: FlagValue) {
        self.f.value = match value {
            FlagValue::Set => self.f.value | 128,
            FlagValue::Unset => self.f.value & (255 - 128)
        }
    }

    pub fn get_sign(&self) -> FlagValue {
        match  self.f.value & 128 {
            128 => FlagValue::Set,
            0 => FlagValue::Unset,
            _ => panic!("Shouldn't happen")
        }
    }

}

#[cfg(test)]
mod tests {
    use crate::{instruction_set::{Instruction, InstructionSet}, runtime::RuntimeComponents};

    use super::{Memory, Registers, AddressBus, DataBus, StackPointer};

    fn runtime_components() -> RuntimeComponents {
        RuntimeComponents { mem: Memory::default(), registers: Registers::default(), address_bus: AddressBus { value: 0 }, data_bus: DataBus { } }
    }
    
    #[test]
    fn test_stack_pointer() {
        let mut sp = StackPointer { location: 0x100 };
        let mut mem = Memory::default();

        sp.push(&mut mem, 0xABEF);
        assert!(sp.location == 0x100 - 0x2);

        sp.push(&mut mem, 0xCD89);
        assert!(sp.location == 0x100 - 0x4);

        let val = sp.pop(&mut mem);
        assert!(val == 0xCD89);
        assert!(sp.location == 0x100 - 0x2);

        let val = sp.pop(&mut mem);
        assert!(val == 0xABEF);
        assert!(sp.location == 0x100);
    }

}