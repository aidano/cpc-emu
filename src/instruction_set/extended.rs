// # Extended Instructions #ED xx xx

use log::error;

use crate::{memory::{Memory, Registers, AddressBus, DataBus, Register, RegisterOperations}, utils::{self, combine_to_double_byte, split_double_byte}, runtime::{Runtime, RuntimeComponents}, inst_metadata};
use super::{Instruction, Operands};

pub struct _0xED46 {}
impl Instruction for _0xED46 {
    // Set interrupt mode 0
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        components.registers.interrupt_mode = 0;
        10
    }

    inst_metadata!(0, "ED 46", "IM 0");
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
    // Set interrupt mode 1
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        components.registers.interrupt_mode = 1;
        10
    }

    inst_metadata!(0, "ED 56", "IM 1");
}

pub struct _0xED5B {}
impl Instruction for _0xED5B {
    // Loads the value pointed to by nn into DE.
    fn execute(&self, components: &mut RuntimeComponents, operands: Operands) -> u16 {
        match operands {
            Operands::Two(op1, op2) => {
                RegisterOperations::ld_register_pair_from_addr(&components.mem, (&mut components.registers.d, &mut components.registers.e), combine_to_double_byte(op2, op1));
            }
            _ => error!("Wrong operands used for {}", self.assembly()),
        }
        20
    }

    inst_metadata!(2, "ED 5B *1 *2", "LD DE,(*2*1)");
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

    inst_metadata!(0, "ED 78", "IN A,(C)");
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

    inst_metadata!(0, "ED 79", "OUT (C),A");
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

    inst_metadata!(0, "BED 0", "LDIR");
}


