// # Extended Instructions #ED xx xx

use crate::{memory::{Memory, Registers, AddressBus, DataBus, Register}, utils, runtime::{Runtime, RuntimeComponents}, inst_metadata};
use super::{Instruction, Operands};

pub struct _0xED49 {}
impl Instruction for _0xED49 {
    // The value of c or written to port bc
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u8 {
        let addr_low_and_val = components.registers.c.get();
        let b_val = components.registers.b.get();
        let port = utils::combine_to_double_byte(b_val, addr_low_and_val);
        components.data_bus.write(port, addr_low_and_val);
        12
    }

    inst_metadata!(0, "ED 49", "OUT (C),C");
}


pub struct _0xED78 {}
impl Instruction for _0xED78 {
    // A byte from port bc is written to a
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u8 {
        let addr_low_and_val = components.registers.c.get(); 
        let b_val = components.registers.b.get();
        let port = utils::combine_to_double_byte(b_val, addr_low_and_val);
        components.registers.a.set(components.data_bus.read(port));
        12
    }

    inst_metadata!(0, "78", "IN A,(C)");
}

pub struct _0xED79 {}
impl Instruction for _0xED79 {
    // The value of a or written to port bc
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u8 {
        let a_val = components.registers.a.get(); 
        let b_val = components.registers.b.get();
        let c_val = components.registers.c.get();
        let port = utils::combine_to_double_byte(b_val, c_val);
        components.data_bus.write(port, a_val);
        12
    }

    inst_metadata!(0, "79", "OUT (C),A");
}



