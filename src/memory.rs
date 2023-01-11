use std::fmt;

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

pub struct DataBus {
    pub value: u8 // TODO: simple impl for now.
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

    pub pc: ProgramCounter
}


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
            pc: ProgramCounter { value: 0 }
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
        match value {
            FlagValue::Set => self.f.value = self.f.value | 64,
            FlagValue::Unset => self.f.value = self.f.value & (255 - 64)
        }
    }

    pub fn get_zero(&mut self) -> bool {
        self.f.value | 64 == 64
    }

    pub fn set_add_subtract(&mut self, value: FlagValue) {
        match value {
            FlagValue::Set => self.f.value = self.f.value | 2,
            FlagValue::Unset => self.f.value = self.f.value & (255 - 2)
        }
    }



}