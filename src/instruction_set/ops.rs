// Execute layer: applies a decoded Op to the runtime components.

use log::error;

use crate::memory::{Cond, FlagsRegister, Registers, R16, R8};
use crate::runtime::{RuntimeComponents, IRQ_PERIOD};
use super::{alu, AluOp, Op, RotOp, RotTarget};

pub fn execute(op: &Op, components: &mut RuntimeComponents) -> u32 {
    match op {
        Op::Unimplemented { prefix, opcode } => {
            match prefix {
                Some(p) => error!("Unimplemented instruction: {:02X} {:02X}", p, opcode),
                None => error!("Unimplemented instruction: {:02X}", opcode),
            }
            std::process::exit(1);
        }

        Op::Nop => 4,
        Op::Halt => {
            components.registers.halted = true;
            4
        }
        Op::Di => {
            components.registers.iff1 = false;
            components.registers.iff2 = false;
            4
        }
        Op::Ei => {
            components.registers.iff1 = true;
            components.registers.iff2 = true;
            4
        }

        // ---- 8-bit loads ----
        Op::LdR8R8(dst, src) => {
            let value = components.registers.get8(*src);
            components.registers.set8(*dst, value);
            4
        }
        Op::LdR8N(r, n) => {
            components.registers.set8(*r, *n);
            7
        }
        Op::LdR8FromPtr(r, ptr) => {
            let addr = components.registers.get16(*ptr);
            let value = components.mem.read(addr);
            components.registers.set8(*r, value);
            7
        }
        Op::LdPtrFromR8(ptr, r) => {
            let addr = components.registers.get16(*ptr);
            let value = components.registers.get8(*r);
            components.mem.write(addr, value);
            7
        }
        Op::LdPtrFromN(ptr, n) => {
            let addr = components.registers.get16(*ptr);
            components.mem.write(addr, *n);
            10
        }
        Op::LdR8FromIndexD(r, idx, d) => {
            let addr = components.registers.get16(*idx).wrapping_add(*d as i16 as u16);
            let value = components.mem.read(addr);
            components.registers.set8(*r, value);
            19
        }
        Op::LdIndexDFromR8(idx, d, r) => {
            let addr = components.registers.get16(*idx).wrapping_add(*d as i16 as u16);
            let value = components.registers.get8(*r);
            components.mem.write(addr, value);
            19
        }
        Op::LdIndexDFromN(idx, d, n) => {
            let addr = components.registers.get16(*idx).wrapping_add(*d as i16 as u16);
            components.mem.write(addr, *n);
            19
        }
        Op::LdAddrFromA(addr) => {
            let value = components.registers.get8(R8::A);
            components.mem.write(*addr, value);
            13
        }
        Op::LdAFromAddr(addr) => {
            let value = components.mem.read(*addr);
            components.registers.set8(R8::A, value);
            13
        }

        // ---- 16-bit loads ----
        Op::LdR16N(r, nn) => {
            components.registers.set16(*r, *nn);
            10
        }
        Op::LdR16FromAddr(r, addr) => {
            let value = components.mem.read16(*addr);
            components.registers.set16(*r, value);
            20
        }
        Op::LdAddrFromR16(addr, r) => {
            let value = components.registers.get16(*r);
            components.mem.write16(*addr, value);
            20
        }
        Op::LdSpFromPtr(r) => {
            let value = components.registers.get16(*r);
            components.registers.set16(R16::SP, value);
            6
        }

        // ---- stack + exchanges ----
        Op::Push(r) => {
            let value = components.registers.get16(*r);
            let sp = &mut components.registers.sp;
            sp.push(&mut components.mem, value);
            11
        }
        Op::Pop(r) => {
            let value = components.registers.sp.pop(&components.mem);
            components.registers.set16(*r, value);
            10
        }
        Op::ExDeHl => {
            let d = components.registers.get16(R16::DE);
            let hl = components.registers.get16(R16::HL);
            components.registers.set16(R16::DE, hl);
            components.registers.set16(R16::HL, d);
            4
        }
        Op::ExAfAlt => {
            let a = components.registers.get16(R16::AF);
            let a_alt = combine_alt(components.registers.a_.get(), components.registers.f_.get());
            set_alt(&mut components.registers, a);
            components.registers.set16(R16::AF, a_alt);
            4
        }
        Op::ExSpPtr(r) => {
            let sp = components.registers.get16(R16::SP);
            let stack_value = components.mem.read16(sp);
            let reg_value = components.registers.get16(*r);
            components.mem.write16(sp, reg_value);
            components.registers.set16(*r, stack_value);
            19
        }
        Op::Exx => {
            let registers = &mut components.registers;
            let b = registers.b.get(); let c = registers.c.get();
            let d = registers.d.get(); let e = registers.e.get();
            let h = registers.h.get(); let l = registers.l.get();
            registers.b.set(registers.b_.get()); registers.c.set(registers.c_.get());
            registers.d.set(registers.d_.get()); registers.e.set(registers.e_.get());
            registers.h.set(registers.h_.get()); registers.l.set(registers.l_.get());
            registers.b_.set(b); registers.c_.set(c);
            registers.d_.set(d); registers.e_.set(e);
            registers.h_.set(h); registers.l_.set(l);
            4
        }

        // ---- ALU ----
        Op::AluR8(op, r) => {
            let value = components.registers.get8(*r);
            alu_a(components, *op, value);
            4
        }
        Op::AluPtr(op, ptr) => {
            let addr = components.registers.get16(*ptr);
            let value = components.mem.read(addr);
            alu_a(components, *op, value);
            7
        }
        Op::AluIndexD(op, idx, d) => {
            let addr = components.registers.get16(*idx).wrapping_add(*d as i16 as u16);
            let value = components.mem.read(addr);
            alu_a(components, *op, value);
            19
        }
        Op::AluN(op, n) => {
            alu_a(components, *op, *n);
            7
        }

        // ---- inc / dec ----
        Op::IncR8(r) => {
            let (value, h, pv) = alu::inc8(components.registers.get8(*r));
            components.registers.set8(*r, value);
            alu::set_incdec_flags(&mut components.registers.f, value, h, pv, false);
            4
        }
        Op::DecR8(r) => {
            let (value, h, pv) = alu::dec8(components.registers.get8(*r));
            components.registers.set8(*r, value);
            alu::set_incdec_flags(&mut components.registers.f, value, h, pv, true);
            4
        }
        Op::IncPtr(ptr) => {
            let addr = components.registers.get16(*ptr);
            let (value, h, pv) = alu::inc8(components.mem.read(addr));
            components.mem.write(addr, value);
            alu::set_incdec_flags(&mut components.registers.f, value, h, pv, false);
            11
        }
        Op::DecPtr(ptr) => {
            let addr = components.registers.get16(*ptr);
            let (value, h, pv) = alu::dec8(components.mem.read(addr));
            components.mem.write(addr, value);
            alu::set_incdec_flags(&mut components.registers.f, value, h, pv, true);
            11
        }
        Op::IncIndexD(idx, d) => {
            let addr = components.registers.get16(*idx).wrapping_add(*d as i16 as u16);
            let (value, h, pv) = alu::inc8(components.mem.read(addr));
            components.mem.write(addr, value);
            alu::set_incdec_flags(&mut components.registers.f, value, h, pv, false);
            23
        }
        Op::DecIndexD(idx, d) => {
            let addr = components.registers.get16(*idx).wrapping_add(*d as i16 as u16);
            let (value, h, pv) = alu::dec8(components.mem.read(addr));
            components.mem.write(addr, value);
            alu::set_incdec_flags(&mut components.registers.f, value, h, pv, true);
            23
        }
        Op::IncR16(r) => {
            let value = components.registers.get16(*r).wrapping_add(1);
            components.registers.set16(*r, value);
            6
        }
        Op::DecR16(r) => {
            let value = components.registers.get16(*r).wrapping_sub(1);
            components.registers.set16(*r, value);
            6
        }

        // ---- 16-bit arithmetic ----
        Op::AddR16(dst, src) => {
            let a = components.registers.get16(*dst);
            let v = components.registers.get16(*src);
            let (value, c, h) = alu::add16(a, v);
            components.registers.set16(*dst, value);
            alu::set_add16_flags(&mut components.registers.f, c, h);
            11
        }
        Op::AdcR16(dst, src) => {
            let c_in = components.registers.f.carry();
            let a = components.registers.get16(*dst);
            let v = components.registers.get16(*src);
            let r = alu::adc16(a, v, c_in);
            components.registers.set16(*dst, r.value);
            alu::set_adcsbc16_flags(&mut components.registers.f, &r, false);
            15
        }
        Op::SbcR16(dst, src) => {
            let c_in = components.registers.f.carry();
            let a = components.registers.get16(*dst);
            let v = components.registers.get16(*src);
            let r = alu::sbc16(a, v, c_in);
            components.registers.set16(*dst, r.value);
            alu::set_adcsbc16_flags(&mut components.registers.f, &r, true);
            15
        }

        // ---- control flow ----
        Op::Jp(addr) => {
            components.registers.pc.set(*addr);
            10
        }
        Op::JpCond(cond, addr) => {
            if cond.evaluate(&components.registers.f) {
                components.registers.pc.set(*addr);
            }
            10
        }
        Op::JpPtr(r) => {
            let addr = components.registers.get16(*r);
            components.registers.pc.set(addr);
            4
        }
        Op::Jr(d) => {
            let pc = components.registers.pc.get().wrapping_add(*d as i16 as u16);
            components.registers.pc.set(pc);
            12
        }
        Op::JrCond(cond, d) => {
            if cond.evaluate(&components.registers.f) {
                let pc = components.registers.pc.get().wrapping_add(*d as i16 as u16);
                components.registers.pc.set(pc);
                return 12;
            }
            7
        }
        Op::Djnz(d) => {
            let b = components.registers.b.get();
            components.registers.b.set(b.wrapping_sub(1));
            if b.wrapping_sub(1) != 0 {
                let pc = components.registers.pc.get().wrapping_add(*d as i16 as u16);
                components.registers.pc.set(pc);
                return 13;
            }
            8
        }
        Op::Call(addr) => {
            let pc = components.registers.pc.get();
            components.registers.sp.push(&mut components.mem, pc);
            components.registers.pc.set(*addr);
            17
        }
        Op::CallCond(cond, addr) => {
            if cond.evaluate(&components.registers.f) {
                let pc = components.registers.pc.get();
                components.registers.sp.push(&mut components.mem, pc);
                components.registers.pc.set(*addr);
                return 17;
            }
            10
        }
        Op::Ret => {
            let addr = components.registers.sp.pop(&components.mem);
            components.registers.pc.set(addr);
            10
        }
        Op::RetCond(cond) => {
            if cond.evaluate(&components.registers.f) {
                let addr = components.registers.sp.pop(&components.mem);
                components.registers.pc.set(addr);
                return 11;
            }
            5
        }
        Op::Reti => {
            let addr = components.registers.sp.pop(&components.mem);
            components.registers.pc.set(addr);
            14
        }
        Op::Retn => {
            let addr = components.registers.sp.pop(&components.mem);
            components.registers.iff1 = components.registers.iff2;
            components.registers.pc.set(addr);
            14
        }
        Op::Rst(v) => {
            let pc = components.registers.pc.get();
            components.registers.sp.push(&mut components.mem, pc);
            components.registers.pc.set(*v as u16);
            11
        }

        // ---- accumulator / flag ops ----
        Op::Daa => {
            let a = components.registers.a.get();
            let (value, c, h) = alu::daa(a, &components.registers.f);
            components.registers.a.set(value);
            alu::set_daa_flags(&mut components.registers.f, value, c, h);
            4
        }
        Op::Cpl => {
            let a = !components.registers.a.get();
            components.registers.a.set(a);
            let f = &mut components.registers.f;
            f.set_half_carry(true);
            f.set_add_subtract(true);
            4
        }
        Op::Scf => {
            let f = &mut components.registers.f;
            f.set_carry(true);
            f.set_half_carry(false);
            f.set_add_subtract(false);
            4
        }
        Op::Ccf => {
            let f = &mut components.registers.f;
            f.set_carry(!f.carry());
            f.set_half_carry(false);
            f.set_add_subtract(false);
            4
        }
        Op::Rlca => {
            let a = components.registers.a.get();
            let (value, c) = alu::rot(RotOp::Rlc, a, false);
            components.registers.a.set(value);
            alu::set_rot_a_flags(&mut components.registers.f, c);
            4
        }
        Op::Rrca => {
            let a = components.registers.a.get();
            let (value, c) = alu::rot(RotOp::Rrc, a, false);
            components.registers.a.set(value);
            alu::set_rot_a_flags(&mut components.registers.f, c);
            4
        }
        Op::Rla => {
            let a = components.registers.a.get();
            let c_in = components.registers.f.carry();
            let (value, c) = alu::rot(RotOp::Rl, a, c_in);
            components.registers.a.set(value);
            alu::set_rot_a_flags(&mut components.registers.f, c);
            4
        }
        Op::Rra => {
            let a = components.registers.a.get();
            let c_in = components.registers.f.carry();
            let (value, c) = alu::rot(RotOp::Rr, a, c_in);
            components.registers.a.set(value);
            alu::set_rot_a_flags(&mut components.registers.f, c);
            4
        }

        // ---- CB set ----
        Op::Rot(rot_op, target) => rot_execute(components, *rot_op, target),
        Op::Bit(bit, target) => {
            let value = read_rot_target(components, target);
            let bit_set = value & (1 << bit) != 0;
            let f = &mut components.registers.f;
            let mut flags = (f.get() & FlagsRegister::CARRY)
                | (value & (FlagsRegister::FLAG_X1 | FlagsRegister::FLAG_X2));
            if !bit_set { flags |= FlagsRegister::ZERO | FlagsRegister::PARITY_OVERFLOW; }
            if *bit == 7 && bit_set { flags |= FlagsRegister::SIGN; }
            f.set(flags);
            bit_cycles(target)
        }
        Op::Res(bit, target) => {
            let value = read_rot_target(components, target);
            let result = value & !(1 << bit);
            write_rot_target(components, target, result);
            rot_cycles(target)
        }
        Op::Set(bit, target) => {
            let value = read_rot_target(components, target);
            let result = value | (1 << bit);
            write_rot_target(components, target, result);
            rot_cycles(target)
        }

        // ---- ED set ----
        Op::Im(mode) => {
            components.registers.interrupt_mode = *mode;
            8
        }
        Op::Neg => {
            let a = components.registers.a.get();
            let r = alu::sub8(0, a, false);
            components.registers.a.set(r.value);
            alu::set_arithmetic_flags(&mut components.registers.f, &r, true);
            8
        }
        Op::LdAFromI => {
            let value = components.registers.i.get();
            components.registers.a.set(value);
            let iff2 = components.registers.iff2;
            let f = &mut components.registers.f;
            let mut flags = (f.get() & FlagsRegister::CARRY)
                | (value & (FlagsRegister::FLAG_X1 | FlagsRegister::FLAG_X2));
            if iff2 { flags |= FlagsRegister::PARITY_OVERFLOW; }
            if value & 0x80 != 0 { flags |= FlagsRegister::SIGN; }
            if value == 0 { flags |= FlagsRegister::ZERO; }
            f.set(flags);
            9
        }
        Op::LdAFromR => {
            let value = components.registers.r.get();
            components.registers.a.set(value);
            let iff2 = components.registers.iff2;
            let f = &mut components.registers.f;
            let mut flags = (f.get() & FlagsRegister::CARRY)
                | (value & (FlagsRegister::FLAG_X1 | FlagsRegister::FLAG_X2));
            if iff2 { flags |= FlagsRegister::PARITY_OVERFLOW; }
            if value & 0x80 != 0 { flags |= FlagsRegister::SIGN; }
            if value == 0 { flags |= FlagsRegister::ZERO; }
            f.set(flags);
            9
        }
        Op::LdIFromA => {
            let a = components.registers.a.get();
            components.registers.i.set(a);
            9
        }
        Op::LdRFromA => {
            let a = components.registers.a.get();
            components.registers.r.set(a);
            9
        }
        Op::Rrd => {
            let hl = components.registers.get16(R16::HL);
            let mem_value = components.mem.read(hl);
            let a = components.registers.a.get();
            let new_a = (a & 0xF0) | (mem_value & 0x0F);
            let new_mem = ((mem_value & 0x0F) << 4) | (mem_value >> 4);
            components.mem.write(hl, new_mem);
            components.registers.a.set(new_a);
            alu::set_logic_flags(&mut components.registers.f, new_a, false);
            18
        }
        Op::Rld => {
            let hl = components.registers.get16(R16::HL);
            let mem_value = components.mem.read(hl);
            let a = components.registers.a.get();
            let new_a = (a & 0xF0) | (mem_value >> 4);
            let new_mem = ((mem_value & 0x0F) << 4) | (a & 0x0F);
            components.mem.write(hl, new_mem);
            components.registers.a.set(new_a);
            alu::set_logic_flags(&mut components.registers.f, new_a, false);
            18
        }
        Op::InR8C(r) => {
            let port = components.registers.get16(R16::BC);
            let value = components.port_in(port);
            components.registers.set8(*r, value);
            let f = &mut components.registers.f;
            let mut flags = (f.get() & FlagsRegister::CARRY)
                | (value & (FlagsRegister::FLAG_X1 | FlagsRegister::FLAG_X2));
            if alu::parity(value) { flags |= FlagsRegister::PARITY_OVERFLOW; }
            if value & 0x80 != 0 { flags |= FlagsRegister::SIGN; }
            if value == 0 { flags |= FlagsRegister::ZERO; }
            f.set(flags);
            12
        }
        Op::OutCR8(r) => {
            let port = components.registers.get16(R16::BC);
            let value = components.registers.get8(*r);
            components.port_out(port, value);
            12
        }
        Op::Ldi => block_ld(components, false, false),
        Op::Ldir => block_ld(components, false, true),
        Op::Ldd => block_ld(components, true, false),
        Op::Lddr => block_ld(components, true, true),
        Op::Cpi => block_cp(components, false, false),
        Op::Cpir => block_cp(components, false, true),
        Op::Cpd => block_cp(components, true, false),
        Op::Cpdr => block_cp(components, true, true),

        // ---- base IO ----
        Op::InAN(n) => {
            let a = components.registers.get8(R8::A);
            let port = ((a as u16) << 8) | *n as u16;
            let value = components.port_in(port);
            components.registers.set8(R8::A, value);
            11
        }
        Op::OutAN(n) => {
            let a = components.registers.get8(R8::A);
            let port = ((a as u16) << 8) | *n as u16;
            let value = components.registers.get8(R8::A);
            components.port_out(port, value);
            11
        }
    }
}

fn combine_alt(a: u8, f: u8) -> u16 {
    ((a as u16) << 8) | f as u16
}

fn set_alt(registers: &mut Registers, value: u16) {
    registers.a_.set((value >> 8) as u8);
    registers.f_.set(value as u8);
}

fn alu_a(components: &mut RuntimeComponents, op: AluOp, value: u8) {
    let a = components.registers.a.get();
    match op {
        AluOp::Add => {
            let r = alu::add8(a, value, false);
            components.registers.a.set(r.value);
            alu::set_arithmetic_flags(&mut components.registers.f, &r, false);
        }
        AluOp::Adc => {
            let c_in = components.registers.f.carry();
            let r = alu::add8(a, value, c_in);
            components.registers.a.set(r.value);
            alu::set_arithmetic_flags(&mut components.registers.f, &r, false);
        }
        AluOp::Sub => {
            let r = alu::sub8(a, value, false);
            components.registers.a.set(r.value);
            alu::set_arithmetic_flags(&mut components.registers.f, &r, true);
        }
        AluOp::Sbc => {
            let c_in = components.registers.f.carry();
            let r = alu::sub8(a, value, c_in);
            components.registers.a.set(r.value);
            alu::set_arithmetic_flags(&mut components.registers.f, &r, true);
        }
        AluOp::And => {
            let result = a & value;
            components.registers.a.set(result);
            alu::set_logic_flags(&mut components.registers.f, result, true);
        }
        AluOp::Xor => {
            let result = a ^ value;
            components.registers.a.set(result);
            alu::set_logic_flags(&mut components.registers.f, result, false);
        }
        AluOp::Or => {
            let result = a | value;
            components.registers.a.set(result);
            alu::set_logic_flags(&mut components.registers.f, result, false);
        }
        AluOp::Cp => {
            let r = alu::sub8(a, value, false);
            alu::set_arithmetic_flags(&mut components.registers.f, &r, true);
        }
    }
}

fn read_rot_target(components: &mut RuntimeComponents, target: &RotTarget) -> u8 {
    match target {
        RotTarget::R8(r) => components.registers.get8(*r),
        RotTarget::Ptr(ptr) => {
            let addr = components.registers.get16(*ptr);
            components.mem.read(addr)
        }
        RotTarget::IndexD(idx, d) => {
            let addr = components.registers.get16(*idx).wrapping_add(*d as i16 as u16);
            components.mem.read(addr)
        }
    }
}

fn write_rot_target(components: &mut RuntimeComponents, target: &RotTarget, value: u8) {
    match target {
        RotTarget::R8(r) => components.registers.set8(*r, value),
        RotTarget::Ptr(ptr) => {
            let addr = components.registers.get16(*ptr);
            components.mem.write(addr, value);
        }
        RotTarget::IndexD(idx, d) => {
            let addr = components.registers.get16(*idx).wrapping_add(*d as i16 as u16);
            components.mem.write(addr, value);
        }
    }
}

fn rot_cycles(target: &RotTarget) -> u32 {
    match target {
        RotTarget::R8(_) => 8,
        RotTarget::Ptr(_) => 15,
        RotTarget::IndexD(_, _) => 23,
    }
}

fn bit_cycles(target: &RotTarget) -> u32 {
    match target {
        RotTarget::R8(_) => 8,
        RotTarget::Ptr(_) => 12,
        RotTarget::IndexD(_, _) => 20,
    }
}

fn rot_execute(components: &mut RuntimeComponents, op: RotOp, target: &RotTarget) -> u32 {
    let value = read_rot_target(components, target);
    let c_in = components.registers.f.carry();
    let (result, c) = alu::rot(op, value, c_in);
    write_rot_target(components, target, result);
    alu::set_rot_flags(&mut components.registers.f, result, c);
    rot_cycles(target)
}

/// The block transfer/search loops are interruptible on the real Z80: an
/// interrupt is accepted between iterations, and after the handler RETs the
/// instruction re-executes and continues from the current register state.
/// `iterations_done` is returned so the caller can roll PC back onto the
/// instruction when yielding, making the interrupt push the LDIR address.
fn block_ld(components: &mut RuntimeComponents, decrement: bool, repeat: bool) -> u32 {
    let delta: u16 = if decrement { u16::MAX } else { 1 };
    let mut cycles: u32 = 16;
    loop {
        let hl = components.registers.get16(R16::HL);
        let de = components.registers.get16(R16::DE);
        let value = components.mem.read(hl);
        components.mem.write(de, value);
        let bc_after = components.registers.get16(R16::BC).wrapping_sub(1);
        components.registers.set16(R16::HL, hl.wrapping_add(delta));
        components.registers.set16(R16::DE, de.wrapping_add(delta));
        components.registers.set16(R16::BC, bc_after);
        let f = &mut components.registers.f;
        let mut flags = f.get() & (FlagsRegister::SIGN | FlagsRegister::ZERO | FlagsRegister::CARRY);
        if bc_after != 0 { flags |= FlagsRegister::PARITY_OVERFLOW; }
        f.set(flags);
        if !repeat || bc_after == 0 {
            break;
        }
        cycles += 21;
        if block_should_yield(components, cycles) {
            components.registers.pc.set(components.registers.pc.get().wrapping_sub(2));
            return cycles;
        }
    }
    cycles
}

/// True when the gate array's 52-scanline interrupt boundary will be crossed
/// within the block operation's already-accumulated cycles and the CPU would
/// accept the interrupt (IFF1 set, EI delay elapsed). The real Z80 samples
/// INT between block iterations, so the op yields for re-execution.
fn block_should_yield(components: &RuntimeComponents, cycles_so_far: u32) -> bool {
    components.cycles_since_irq + cycles_so_far >= IRQ_PERIOD
        && components.registers.iff1
        && !components.registers.ei_delay
}

fn block_cp(components: &mut RuntimeComponents, decrement: bool, repeat: bool) -> u32 {
    let delta: u16 = if decrement { u16::MAX } else { 1 };
    let mut cycles: u32 = 16;
    loop {
        let hl = components.registers.get16(R16::HL);
        let value = components.mem.read(hl);
        let a = components.registers.a.get();
        let compare = alu::sub8(a, value, false);
        let bc_after = components.registers.get16(R16::BC).wrapping_sub(1);
        components.registers.set16(R16::HL, hl.wrapping_add(delta));
        components.registers.set16(R16::BC, bc_after);
        let f = &mut components.registers.f;
        let mut flags = (f.get() & FlagsRegister::CARRY)
            | (compare.value & (FlagsRegister::FLAG_X1 | FlagsRegister::FLAG_X2))
            | FlagsRegister::ADD_SUBTRACT;
        if compare.h { flags |= FlagsRegister::HALF_CARRY; }
        if compare.value & 0x80 != 0 { flags |= FlagsRegister::SIGN; }
        if compare.value == 0 { flags |= FlagsRegister::ZERO; }
        if bc_after != 0 { flags |= FlagsRegister::PARITY_OVERFLOW; }
        f.set(flags);
        if !repeat || bc_after == 0 || compare.value == 0 {
            break;
        }
        cycles += 21;
        if block_should_yield(components, cycles) {
            components.registers.pc.set(components.registers.pc.get().wrapping_sub(2));
            return cycles;
        }
    }
    cycles
}
