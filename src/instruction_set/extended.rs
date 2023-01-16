// # Extended Instructions #ED xx xx

use crate::{memory::{Memory, Registers, AddressBus, DataBus}, utils, runtime::{Runtime, RuntimeComponents}};
use super::{Instruction, Operands};

pub struct _0xED49 {}
impl Instruction for _0xED49 {
    // The value of c or written to port bc
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u8 {
        let addr_low_and_val = components.registers.c.get(); // Seems like an odd instruction which uses the same register for address and value! 8-)
        let b_val = components.registers.b.get();
        let port = utils::combine_to_double_byte(b_val, addr_low_and_val);
        components.data_bus.write(port, addr_low_and_val);
        12
    }

    fn operand_count(&self) -> u8 {
        0
    }

    fn op_code(&self) -> u8 {
        0x49
    }

    fn machine_code(&self) -> &str {
        "ED 49"
    }

    fn assembly(&self) -> &str {
        "OUT (C),C"
    }

}


pub struct _0xED78 {}
impl Instruction for _0xED78 {
    // A byte from port bc is written to a
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u8 {
        let addr_low_and_val = components.registers.c.get(); // Seems like an odd instruction which uses the same register for address and value! 8-)
        let b_val = components.registers.b.get();
        let port = utils::combine_to_double_byte(b_val, addr_low_and_val);
        components.registers.a.set(components.data_bus.read(port));
        12
    }

    fn operand_count(&self) -> u8 {
        0
    }

    fn op_code(&self) -> u8 {
        0x78
    }

    fn machine_code(&self) -> &str {
        "ED 78"
    }

    fn assembly(&self) -> &str {
        "IN A,(C)"
    }

}

