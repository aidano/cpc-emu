// # Extended Instructions #ED xx xx

use crate::{memory::{Memory, Registers, AddressBus, DataBus}, utils, runtime::{Runtime, RuntimeComponents}};
use super::{Instruction, Operands};

pub struct InstOutCC {}
impl Instruction for InstOutCC {
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) {
        let addr_low_and_val = components.registers.c.get(); // Seems like an odd instruction which uses the same register for address and value! 8-)
        let b_val = components.registers.b.get();
        components.address_bus.value = utils::combine_to_double_byte(b_val, addr_low_and_val);
        components.data_bus.value = addr_low_and_val;
    }

    fn operand_count(&self) -> u8 {
        0
    }

    fn op_code(&self) -> u8 {
        0x49
    }
}