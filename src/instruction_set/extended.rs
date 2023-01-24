// # Extended Instructions #ED xx xx

use crate::{memory::{Memory, Registers, AddressBus, DataBus, Register}, utils::{self, combine_to_double_byte, split_double_byte}, runtime::{Runtime, RuntimeComponents}, inst_metadata};
use super::{Instruction, Operands};

pub struct _0xED46 {}
impl Instruction for _0xED46 {
    // The value of a or written to port bc
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        components.registers.interrupt_mode = 0;
        10
    }

    inst_metadata!(0, "46", "IM 0");
}

pub struct _0xED49 {}
impl Instruction for _0xED49 {
    // The value of c or written to port bc
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        let addr_low_and_val = components.registers.c.get();
        let b_val = components.registers.b.get();
        let port = utils::combine_to_double_byte(b_val, addr_low_and_val);
        components.data_bus.write(port, addr_low_and_val);
        12
    }

    inst_metadata!(0, "ED 49", "OUT (C),C");
}

pub struct _0xED56 {}
impl Instruction for _0xED56 {
    // The value of a or written to port bc
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        components.registers.interrupt_mode = 1;
        10
    }

    inst_metadata!(0, "56", "IM 1");
}

pub struct _0xED78 {}
impl Instruction for _0xED78 {
    // A byte from port bc is written to a
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
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
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        let a_val = components.registers.a.get(); 
        let b_val = components.registers.b.get();
        let c_val = components.registers.c.get();
        let port = utils::combine_to_double_byte(b_val, c_val);
        components.data_bus.write(port, a_val);
        12
    }

    inst_metadata!(0, "79", "OUT (C),A");
}



pub struct _0xEDB0 {}
impl Instruction for _0xEDB0 {
    // Transfers a byte of data from the memory location pointed to by HL to the memory location pointed to by DE. 
    // Then HL and DE are incremented and BC is decremented. 
    // If BC is not zero, this operation is repeated. 
    // Interrupts can trigger while this instruction is processing.
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        let mut repeats: u16 = 0;
        loop {
            let source_addr = combine_to_double_byte(components.registers.h.get(), components.registers.l.get());
            let target_addr = combine_to_double_byte(components.registers.d.get(), components.registers.e.get());
            components.mem.locations[target_addr as usize] = components.mem.locations[source_addr as usize];
            let mut bc = combine_to_double_byte(components.registers.b.get(), components.registers.c.get());
            bc -= 1;
            let (b, c) = split_double_byte(bc);
            components.registers.b.set(b);
            components.registers.c.set(c);
            if bc == 0  { break; }
            repeats += 1;
        } 

        16 + (repeats * 21)
    }

    inst_metadata!(0, "B0", "LDIR");
}


