use log::error;

use crate::{memory::{Memory, Registers, AddressBus, DataBus, Register, RegisterOperations}, utils::{self, combine_to_double_byte, split_double_byte}, runtime::{Runtime, RuntimeComponents}, inst_metadata};
use super::{Instruction, Operands};

pub struct _0xCB38 {}
impl Instruction for _0xCB38 {
    // The contents of B are shifted right one bit position. 
    // The contents of bit 0 are copied to the carry flag and a zero is put into bit 7.
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        let reg = &mut components.registers;
        RegisterOperations::srl(&mut reg.b, &mut reg.f);
        8
    }

    inst_metadata!(1, "CB 38", "SRL B");
}