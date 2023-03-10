use std::{fmt, ops::Add};

use crate::{utils::{split_double_byte, combine_to_double_byte}, instruction_set::Instruction};

pub struct Memory {
    pub locations: [u8; 0xFFFF]
}

impl Memory {
    pub fn default() -> Memory {
        Memory { locations: [0x01; 0xFFFF] }
    }
}

pub trait Register {
    fn set(&mut self, value: u8);
    fn get(&self) -> u8;
    fn name(&self) -> &str;
}
pub struct DefaultRegister {
    name: String,
    value: u8
}

impl Register for DefaultRegister {
    fn set(&mut self, value: u8) {
        self.value = value;
    }

    fn get(&self) -> u8 {
        self.value
    }

    fn name(&self) -> &str {
        self.name.as_str()
    }
}

pub struct Accumulator {
    name: String,
    value: u8
}

impl Register for Accumulator {
    fn set(&mut self, value: u8) {
        self.value = value;
    }

    fn get(&self) -> u8 {
        self.value
    }

    fn name(&self) -> &str {
        self.name.as_str()
    }
}

impl Accumulator {
    pub fn sub_reg<R : Register>(&mut self, reg: &R, flags: &mut FlagsRegister) {
        self.set(self.get() - reg.get());
        flags.set_parity_overflow( if reg.get() & 128 == 128 { FlagValue::Set } else { FlagValue::Unset });
    }

    pub fn sub_value(&mut self, value: u8, flags: &mut FlagsRegister) {
        let carry = if (self.value as u32 + value as u32) > u16::MAX as u32 {
            FlagValue::Set 
           } else {
                FlagValue::Unset 
           };
        self.set(self.get() - value);
        flags.set_parity_overflow( if value & 128 == 128 { FlagValue::Set } else { FlagValue::Unset });
        flags.set_carry(carry);
    }

    pub fn sub_value_and_carry(&mut self, value: u8, flags: &mut FlagsRegister) {
        let value = value + if flags.get_carry() == FlagValue::Set { 1 } else { 0 };
        self.sub_value(value, flags);
    }

    pub fn and(&mut self, value: u8, flags: &mut FlagsRegister) {
        self.set(self.get() & value);
        // todo: set flags
        flags.set_carry(FlagValue::Unset);
        flags.set_add_subtract(FlagValue::Unset);
        flags.set_half_carry(FlagValue::Set);

        let overflow = if self.get() & 128 > 1 {
            FlagValue::Set
        } else {
            FlagValue::Unset
        };
        flags.set_parity_overflow(overflow);
    }

    pub fn or<R : Register>(&mut self, reg: &R, flags: &mut FlagsRegister) {
        self.set(self.get() | reg.get());
        flags.set_parity_overflow( if reg.get() & 128 == 128 { FlagValue::Set } else { FlagValue::Unset });
    }

    pub fn or_a(&mut self, flags: &mut FlagsRegister) {
        self.set(self.get() | self.get());
        flags.set_parity_overflow( if self.get() & 128 == 128 { FlagValue::Set } else { FlagValue::Unset });
    }


    pub fn compare_reg<R: Register>(&self, reg: &R, flags: &mut FlagsRegister) {
        flags.set_parity_overflow(if self.get() as i16 - (reg.get() as i16) < -128 { FlagValue::Set } else { FlagValue::Unset });        
    }

    pub fn compare_val(&self, val: u8, flags: &mut FlagsRegister) {
        flags.set_parity_overflow(if self.get() as i16 - (val as i16) < -128 { FlagValue::Set } else { FlagValue::Unset });        
    }

    pub fn xor<R : Register>(&mut self, reg: &R, flags: &mut FlagsRegister) {
        self.set(self.get() ^ reg.get());
        flags.set_parity_overflow( if reg.get() & 128 == 128 { FlagValue::Set } else { FlagValue::Unset });
        flags.set_zero(if self.value == 0 { FlagValue::Set } else { FlagValue::Unset });
        flags.set_sign(if self.value & 128 == 128 { FlagValue::Set } else { FlagValue::Unset });
    }

    pub fn xor_address_from_reg_pair<R : Register>(&mut self, mem: &Memory, reg_pair: (&R, &R), flags: &mut FlagsRegister) {
        let location = combine_to_double_byte(reg_pair.0.get(), reg_pair.1.get());
        let val = mem.locations[location as usize];
        self.set(self.get() ^ val);
        flags.set_parity_overflow( if val & 128 == 128 { FlagValue::Set } else { FlagValue::Unset });
        flags.set_zero(if self.value == 0 { FlagValue::Set } else { FlagValue::Unset });
        flags.set_sign(if self.value & 128 == 128 { FlagValue::Set } else { FlagValue::Unset });
    }

    pub fn xor_a(&mut self, flags: &mut FlagsRegister) {
        self.set(self.get() ^ self.get());
        flags.set_parity_overflow( if self.get() & 128 == 128 { FlagValue::Set } else { FlagValue::Unset });
    }

    // Add the passed register to a
    pub fn add_a<R : Register>(&mut self, reg: &R, flags: &mut FlagsRegister) {
        let carry = flags.get_carry();
        self.set(self.get() + reg.get()); // todo: read up on this.
        flags.set_parity_overflow( if reg.get() & 128 == 128 { FlagValue::Set } else { FlagValue::Unset });
    }

    // Add the passed register and the carry flag to a
    pub fn adc_a<R : Register>(&mut self, reg: &R, flags: &mut FlagsRegister) {
        let carry = flags.get_carry();
        self.set(self.get() + reg.get() + carry); // todo: read up on this.
        flags.set_parity_overflow( if reg.get() & 128 == 128 { FlagValue::Set } else { FlagValue::Unset });
    }
}


impl fmt::Debug for dyn Register {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Register")
        .field("name", &self.name())
        .field("value", &self.get())
        .finish()
    }
}

pub struct FlagsRegister {
    value: u8
}

impl Register for FlagsRegister {
    fn set(&mut self, value: u8) {
        self.value = value;
    }

    fn get(&self) -> u8 {
        self.value
    }

    fn name(&self) -> &str {
        "F"
    }
}

impl FlagsRegister {
    


    //
    // Bit	    7	6	5	4	3	2	1	0
    // Position	S	Z	/	H	/	P/V	N	C
    //
    pub fn set_carry(&mut self, value: FlagValue) {
        match value {
            FlagValue::Set => self.value = self.value | 1,
            FlagValue::Unset => self.value = self.value & (255 - 1)
        }
    }

    pub fn set_add_subtract(&mut self, value: FlagValue) {
        match value {
            FlagValue::Set => self.value = self.value | 2,
            FlagValue::Unset => self.value = self.value & (255 - 2)
        }
    }

    pub fn set_parity_overflow(&mut self, value: FlagValue) {
        match value {
            FlagValue::Set => self.value = self.value | 4,
            FlagValue::Unset => self.value = self.value & (255 - 4)
        }
    }

    pub fn set_half_carry(&mut self, value: FlagValue) {
        match value {
            FlagValue::Set => self.value = self.value | 16,
            FlagValue::Unset => self.value = self.value & (255 - 16)
        }
    }

    pub fn set_zero(&mut self, value: FlagValue) {
        self.value = match value {
            FlagValue::Set => self.value | 64,
            FlagValue::Unset => self.value & (255 - 64)
        }
    }

    pub fn set_sign(&mut self, value: FlagValue) {
        self.value = match value {
            FlagValue::Set => self.value | 128,
            FlagValue::Unset => self.value & (255 - 128)
        }
    }

    pub fn get_carry(&mut self) -> FlagValue {
        match  self.value & 1 {
            1 => FlagValue::Set,
            0 => FlagValue::Unset,
            _ => panic!("Shouldn't happen")
        }
    }

    pub fn get_add_subtract(&mut self) -> FlagValue {
        match  self.value & 2 {
            2 => FlagValue::Set,
            0 => FlagValue::Unset,
            _ => panic!("Shouldn't happen")
        }
    }

    pub fn get_parity_overflow(&mut self) -> FlagValue {
        match  self.value & 4 {
            4 => FlagValue::Set,
            0 => FlagValue::Unset,
            _ => panic!("Shouldn't happen")
        }
    }

    pub fn get_half_carry(&mut self) -> FlagValue {
        match  self.value & 16 {
            16 => FlagValue::Set,
            0 => FlagValue::Unset,
            _ => panic!("Shouldn't happen")
        }
    }

    pub fn get_zero(&mut self) -> FlagValue {
        match  self.value & 64 {
            64 => FlagValue::Set,
            0 => FlagValue::Unset,
            _ => panic!("Shouldn't happen")
        }
    }

    pub fn get_sign(&self) -> FlagValue {
        match  self.value & 128 {
            128 => FlagValue::Set,
            0 => FlagValue::Unset,
            _ => panic!("Shouldn't happen")
        }
    }
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

    pub fn set(&mut self, value: usize) {
        self.location = value;
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
    pub a: Accumulator,
    pub f: FlagsRegister,
    pub b: DefaultRegister,
    pub c: DefaultRegister,
    pub d: DefaultRegister,
    pub e: DefaultRegister,
    pub h: DefaultRegister,
    pub l: DefaultRegister,
    pub a_: Accumulator,
    pub f_: FlagsRegister,
    pub b_: DefaultRegister,
    pub c_: DefaultRegister,
    pub d_: DefaultRegister,
    pub e_: DefaultRegister,
    pub h_: DefaultRegister,
    pub l_: DefaultRegister,

    pub i: DefaultRegister,
    pub x: DefaultRegister,

    pub pc: ProgramCounter,
    pub sp: StackPointer,
    pub iff1: bool,
    pub iff2: bool,
    pub interrupt_mode: u8
}

pub struct RegisterOperations {}

impl RegisterOperations {

    pub fn dec<R: Register>(reg: &mut R, flags: &mut FlagsRegister) {
        reg.set(reg.get() - 1);
        flags.set_parity_overflow( if reg.get() & 128 == 128 { FlagValue::Set } else { FlagValue::Unset });
        flags.set_add_subtract(FlagValue::Set);
        flags.set_zero(if reg.get() == 0 { FlagValue::Set } else { FlagValue::Unset});
        flags.set_sign(if (reg.get() as i8) < 0 { FlagValue::Set } else { FlagValue::Unset });
    }
    
    pub fn dec_register_pair<R: Register>(reg_pair: (&mut R, &mut R), flags: &mut FlagsRegister) {
        let value = combine_to_double_byte(reg_pair.0.get(), reg_pair.1.get()) - 1;
        let (high, low) = split_double_byte(value);
        reg_pair.0.set(high);
        reg_pair.1.set(low);
        flags.set_add_subtract(FlagValue::Set);
    }

    pub fn inc<R: Register>(reg: &mut R, flags: &mut FlagsRegister) {
        let half_carry = ((reg.get() & 0xf) + (1 & 0xf)) & 0x10 == 0x10;
        reg.set(reg.get() + 1);
        flags.set_parity_overflow( if reg.get() & 128 == 128 { FlagValue::Set } else { FlagValue::Unset });
        flags.set_half_carry( if half_carry { FlagValue::Set } else { FlagValue::Unset });
        flags.set_add_subtract(FlagValue::Unset);
    }

    pub fn inc_register_pair<R: Register>(reg_pair: (&mut R, &mut R), flags: &mut FlagsRegister) {
        let half_carry = ((reg_pair.0.get() & 0xf) + (1 & 0xf)) & 0x10 == 0x10;
        let value = combine_to_double_byte(reg_pair.0.get(), reg_pair.1.get()) + 1;
        let (high, low) = split_double_byte(value);
        reg_pair.0.set(high);
        reg_pair.1.set(low);
        // flags.set_add_subtract(FlagValue::Unset);
        // flags.set_parity_overflow( if reg_pair.0.get() & 128 == 128 { FlagValue::Set } else { FlagValue::Unset });
        // flags.set_half_carry( if half_carry { FlagValue::Set } else { FlagValue::Unset });
    }


    pub fn ld_register_with_value<R: Register>(reg: &mut R, value: u8) {
        reg.set(value);
    }

    pub fn ld_register_from_register<R: Register, T: Register>(source: &R, target: &mut T) {
        target.set(source.get());
    }

    pub fn ld_register_from_addr<R: Register>(mem: &Memory, reg: &mut R, value: u16) {
        reg.set(mem.locations[value as usize]);
    }

    pub fn ld_register_from_addr_with_register_pair<R : Register, P: Register>(mem: &Memory, reg: &mut R, reg_pair: (&P, &P)) {
        let addr = combine_to_double_byte(reg_pair.0.get(), reg_pair.1.get());
        reg.set(mem.locations[addr as usize]);
    }

    pub fn ld_register_pair_with_value<R: Register>(reg_pair: (&mut R, &mut R), value: u16) {
        let (high, low) = split_double_byte(value);
        reg_pair.0.set(high);
        reg_pair.1.set(low);
    }

    pub fn ld_register_pair_from_addr<R: Register>(mem: &Memory, reg_pair: (&mut R, &mut R), addr: u16) {
        let value = mem.locations[addr as usize];
        RegisterOperations::ld_register_pair_with_value(reg_pair, combine_to_double_byte(0x0, value));
    }

    pub fn ld_addr_from_reg_pair_with_value<R : Register>(mem: &mut Memory, reg_pair: (&R, &R), value: u8) {
        let addr = combine_to_double_byte(reg_pair.0.get(), reg_pair.1.get());
        mem.locations[addr as usize] = value;
    }

    pub fn ld_addr_from_value_with_register<R : Register>(mem: &mut Memory, value: u16, reg: &R) {
        mem.locations[value as usize] = reg.get();
    }

    pub fn ld_addr_from_value_with_register_pair<R : Register>(mem: &mut Memory, value: u16, reg_pair: (&R, &R)) {
        mem.locations[value as usize] = reg_pair.1.get();
        // seems like we just store the low byte and ignore the high byte.
        //mem.locations[(value + 1) as usize] = reg_pair.1.get(); 

    }

    pub fn ld_addr_from_reg_pair_with_register<R : Register, P : Register>(mem: &mut Memory, reg_pair: (&R, &R), reg: (&P)) {
        let addr = combine_to_double_byte(reg_pair.0.get(), reg_pair.1.get());
        mem.locations[addr as usize] = reg.get();
    }

    pub fn dbl_register_pair<P: Register>(reg_pair: (&mut P, &mut P), flags: &mut FlagsRegister) {
        let val = combine_to_double_byte(reg_pair.0.get(), reg_pair.1.get());
        let total_as_u32 = (val as u32 + val as u32);
        let carry = if (val as u32 + val as u32) > u16::MAX as u32 {
             FlagValue::Set 
            } else {
                 FlagValue::Unset 
            };
        let half_carry = if (val & 8 == 1) && (val & 8 == 1) {
                FlagValue::Set
            } else {
                FlagValue::Unset
            };
        let total_as_u16 = (total_as_u32 & 0xFFFF) as u16;
        let (h, l) = split_double_byte(total_as_u16);
        reg_pair.0.set(h);
        reg_pair.1.set(l);
        flags.set_carry(carry);
        flags.set_half_carry(half_carry);
        flags.set_add_subtract(FlagValue::Set);
    }


    pub fn add_register_pairs<P: Register>(target_reg_pair: (&mut P, &mut P), source_reg_pair: (&P, &P), flags: &mut FlagsRegister) {
        let val1 = combine_to_double_byte(target_reg_pair.0.get(), target_reg_pair.1.get());
        let val2 = combine_to_double_byte(source_reg_pair.0.get(), source_reg_pair.1.get());
        let total_as_u32 = (val1 as u32 + val2 as u32);
        let carry = if (val1 as u32 + val2 as u32) > u16::MAX as u32 {
             FlagValue::Set 
            } else {
                 FlagValue::Unset 
            };
        let half_carry = if (val1 & 8 == 1) && (val2 & 8 == 1) {
                FlagValue::Set
            } else {
                FlagValue::Unset
            };
        let total_as_u16 = (total_as_u32 & 0xFFFF) as u16;
        let (h, l) = split_double_byte(total_as_u16);
        target_reg_pair.0.set(h);
        target_reg_pair.1.set(l);
        flags.set_carry(carry);
        flags.set_half_carry(half_carry);
        flags.set_add_subtract(FlagValue::Set);
    }


    pub fn push_register_pair<R: Register, P: Register>(reg_pair: (&R, &P), sp: &mut StackPointer, mem: &mut Memory) {
        let val = combine_to_double_byte(reg_pair.0.get(), reg_pair.1.get());
        sp.push(mem, val);
    }

    pub fn pop_register_pair<R: Register, P: Register>(reg_pair: (&mut R, &mut P), sp: &mut StackPointer, mem: &mut Memory) {
        let (val1, val2) = split_double_byte(sp.pop(&mem));
        reg_pair.0.set(val1);
        reg_pair.1.set(val2);
    }

    // Note: Official instruction behaviour is pc.value + 3. Maybe change this later with wider change to how pc is implemented w.r.t. instruction parsing.
    pub fn call(value: u16, sp: &mut StackPointer, pc: &mut ProgramCounter, mem: &mut Memory) {
        sp.push(mem, pc.value);
        pc.set(value);
    }

    // The contents of the passed register are shifted right one bit position. 
    // The contents of bit 0 are copied to the carry flag and a zero is put into bit 7.
    pub fn srl<R: Register>(reg: &mut R, flags: &mut FlagsRegister) {
        flags.set_carry(if reg.get() & 1 == 1 { FlagValue::Set } else { FlagValue::Unset });
        reg.set((reg.get()) >> 1 & 0x7F);
    }

}

impl Add<FlagValue> for u8 {
    type Output = u8;

    fn add(self, rhs: FlagValue) -> Self::Output {
        return if rhs == FlagValue::Set { self + 1 } else { self }
    }
}

#[derive(PartialEq)]
pub enum FlagValue {
    Set,
    Unset
}

impl Registers {
    pub fn default() -> Registers {
        Registers {
            a: Accumulator { name: "a".to_string(), value: 0},
            f: FlagsRegister { value: 0},
            b: DefaultRegister {name: "b".to_string(), value: 0},
            c: DefaultRegister {name: "c".to_string(), value: 0},
            d: DefaultRegister {name: "d".to_string(), value: 0},
            e: DefaultRegister {name: "e".to_string(), value: 0},
            h: DefaultRegister {name: "h".to_string(), value: 0},
            l: DefaultRegister {name: "l".to_string(), value: 0},
            a_: Accumulator { name: "a'".to_string(), value: 0},
            f_: FlagsRegister { value: 0},
            b_: DefaultRegister {name: "b'".to_string(), value: 0},
            c_: DefaultRegister {name: "c'".to_string(), value: 0},
            d_: DefaultRegister {name: "d'".to_string(), value: 0},
            e_: DefaultRegister {name: "e'".to_string(), value: 0},
            h_: DefaultRegister {name: "h'".to_string(), value: 0},
            l_: DefaultRegister {name: "l'".to_string(), value: 0},
            i: DefaultRegister {name: "i".to_string(), value: 0},
            x: DefaultRegister {name: "x".to_string(), value: 0},
            pc: ProgramCounter { value: 0 }, // PC normally begins at start of memory
            sp: StackPointer { location: 0xFFFF }, // SP normally begins at the end of memory and moves down.
            iff1: false,
            iff2: false,
            interrupt_mode: 0
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