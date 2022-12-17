// # Extended Instructions #ED xx xx

use crate::memory::{Memory, Registers};
use super::{Instruction, Operands};

pub struct InstOutCC {}
impl Instruction for InstOutCC {
    fn execute(&self, mem: &mut Memory, reg: &mut Registers, operands: Operands) {
        todo!()
    }

    fn operand_count(&self) -> u8 {
        0
    }

    fn op_code(&self) -> u8 {
        0x49
    }
}