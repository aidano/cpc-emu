use log::error;

use crate::{memory::{Memory, Registers, AddressBus, DataBus, Register, RegisterOperations}, utils::{self, combine_to_double_byte, split_double_byte}, runtime::{Runtime, RuntimeComponents}, inst_metadata};
use super::{Instruction, Operands};

pub struct _0xDDE1 {}
impl Instruction for _0xDDE1 {
    // Set interrupt mode 0
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        let reg = &mut components.registers;
        RegisterOperations::pop_register_pair((&mut reg.i, &mut reg.x), &mut reg.sp, &mut components.mem);
        14
    }

    inst_metadata!(0, "DD E1", "POP IX");
}
pub struct _0xDDE5 {}
impl Instruction for _0xDDE5 {
    // Set interrupt mode 0
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        let reg = &mut components.registers;
        RegisterOperations::push_register_pair((&reg.i, &reg.x), &mut reg.sp, &mut components.mem);
        15
    }

    inst_metadata!(0, "DD E5", "PUSH IX");
}

