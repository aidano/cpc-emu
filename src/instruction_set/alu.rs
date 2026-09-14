// Flag-accurate ALU core shared by all instruction families.

use crate::memory::FlagsRegister;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Alu8Result {
    pub value: u8,
    pub c: bool,
    pub h: bool,
    pub pv: bool,
}

pub fn parity(value: u8) -> bool {
    value.count_ones() % 2 == 0
}

pub fn add8(a: u8, v: u8, c_in: bool) -> Alu8Result {
    let total = a as u16 + v as u16 + c_in as u16;
    let value = total as u8;
    Alu8Result {
        value,
        c: total > 0xFF,
        h: ((a & 0x0F) as u16 + (v & 0x0F) as u16 + c_in as u16) > 0x0F,
        pv: ((a ^ value) & (v ^ value) & 0x80) != 0,
    }
}

pub fn sub8(a: u8, v: u8, c_in: bool) -> Alu8Result {
    let borrow_total = v as u16 + c_in as u16;
    let value = (a as u16).wrapping_sub(borrow_total) as u8;
    Alu8Result {
        value,
        c: (a as u16) < borrow_total,
        h: ((a & 0x0F) as u16) < ((v & 0x0F) as u16 + c_in as u16),
        pv: ((a ^ v) & (a ^ value) & 0x80) != 0,
    }
}

pub fn inc8(v: u8) -> (u8, bool, bool) {
    let value = v.wrapping_add(1);
    (value, (v & 0x0F) == 0x0F, v == 0x7F)
}

pub fn dec8(v: u8) -> (u8, bool, bool) {
    let value = v.wrapping_sub(1);
    (value, (v & 0x0F) == 0x00, v == 0x80)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Alu16Result {
    pub value: u16,
    pub c: bool,
    pub h: bool,
    pub pv: bool,
}

pub fn add16(a: u16, v: u16) -> (u16, bool, bool) {
    let total = a as u32 + v as u32;
    (
        (total & 0xFFFF) as u16,
        total > 0xFFFF,
        ((a & 0x0FFF) as u32 + (v & 0x0FFF) as u32) > 0x0FFF,
    )
}

pub fn adc16(a: u16, v: u16, c_in: bool) -> Alu16Result {
    let total = a as u32 + v as u32 + c_in as u32;
    let value = (total & 0xFFFF) as u16;
    Alu16Result {
        value,
        c: total > 0xFFFF,
        h: (a & 0x0FFF) as u32 + (v & 0x0FFF) as u32 + c_in as u32 > 0x0FFF,
        pv: ((a ^ value) & (v ^ value) & 0x8000) != 0,
    }
}

pub fn sbc16(a: u16, v: u16, c_in: bool) -> Alu16Result {
    let borrow_total = v as u32 + c_in as u32;
    let value = (a as u32).wrapping_sub(borrow_total) as u16;
    Alu16Result {
        value,
        c: (a as u32) < borrow_total,
        h: ((a & 0x0FFF) as u32) < ((v & 0x0FFF) as u32 + c_in as u32),
        pv: ((a ^ v) & (a ^ value) & 0x8000) != 0,
    }
}

/// Rotate / shift. c_in is the current carry flag (used by RL/RR).
/// Returns (result, new carry).
pub fn rot(op: super::RotOp, v: u8, c_in: bool) -> (u8, bool) {
    use super::RotOp;
    match op {
        RotOp::Rlc => (v.rotate_left(1), v & 0x80 != 0),
        RotOp::Rrc => (v.rotate_right(1), v & 0x01 != 0),
        RotOp::Rl => ((v << 1) | c_in as u8, v & 0x80 != 0),
        RotOp::Rr => ((v >> 1) | ((c_in as u8) << 7), v & 0x01 != 0),
        RotOp::Sla => (v << 1, v & 0x80 != 0),
        RotOp::Sra => (((v as i8) >> 1) as u8, v & 0x01 != 0),
        RotOp::Sll => ((v << 1) | 0x01, v & 0x80 != 0),
        RotOp::Srl => (v >> 1, v & 0x01 != 0),
    }
}

/// Decimal adjust accumulator. Returns (result, new carry, new half-carry).
pub fn daa(a: u8, f: &FlagsRegister) -> (u8, bool, bool) {
    let c = f.carry();
    let h = f.half_carry();
    let n = f.add_subtract();

    let mut add: u8 = 0;
    if (a & 0x0F) > 0x09 || h {
        add |= 0x06;
    }
    if a > 0x99 || c {
        add |= 0x60;
    }
    let new_c = a > 0x99 || c;

    if n {
        let result = a.wrapping_sub(add);
        let new_h = (a & 0x0F) < (add & 0x0F);
        (result, new_c, new_h)
    } else {
        let result = a.wrapping_add(add);
        let new_h = (a & 0x0F) + (add & 0x0F) > 0x0F;
        (result, new_c, new_h)
    }
}

/// Flags for ADD/ADC/SUB/SBC/CP: S Z (X) H (X) P/V N C, X bits from result.
pub fn set_arithmetic_flags(f: &mut FlagsRegister, r: &Alu8Result, n: bool) {
    let mut flags = r.value & (FlagsRegister::FLAG_X1 | FlagsRegister::FLAG_X2);
    if r.c { flags |= FlagsRegister::CARRY; }
    if n { flags |= FlagsRegister::ADD_SUBTRACT; }
    if r.pv { flags |= FlagsRegister::PARITY_OVERFLOW; }
    if r.h { flags |= FlagsRegister::HALF_CARRY; }
    if r.value & 0x80 != 0 { flags |= FlagsRegister::SIGN; }
    if r.value == 0 { flags |= FlagsRegister::ZERO; }
    f.set(flags);
}

/// Flags for AND/OR/XOR: S Z (X) 0 P 0 0 0. h is true for AND, false for OR/XOR.
pub fn set_logic_flags(f: &mut FlagsRegister, value: u8, h: bool) {
    let mut flags = value & (FlagsRegister::FLAG_X1 | FlagsRegister::FLAG_X2);
    if h { flags |= FlagsRegister::HALF_CARRY; }
    if parity(value) { flags |= FlagsRegister::PARITY_OVERFLOW; }
    if value & 0x80 != 0 { flags |= FlagsRegister::SIGN; }
    if value == 0 { flags |= FlagsRegister::ZERO; }
    f.set(flags);
}

/// Flags for INC/DEC: S Z (X) H (X) P/V N C(unaffected).
pub fn set_incdec_flags(f: &mut FlagsRegister, value: u8, h: bool, pv: bool, n: bool) {
    let mut flags = (f.get() & FlagsRegister::CARRY)
        | (value & (FlagsRegister::FLAG_X1 | FlagsRegister::FLAG_X2));
    if h { flags |= FlagsRegister::HALF_CARRY; }
    if pv { flags |= FlagsRegister::PARITY_OVERFLOW; }
    if n { flags |= FlagsRegister::ADD_SUBTRACT; }
    if value & 0x80 != 0 { flags |= FlagsRegister::SIGN; }
    if value == 0 { flags |= FlagsRegister::ZERO; }
    f.set(flags);
}

/// Flags for 16-bit ADD: C H set, N=0, S Z P/V preserved.
pub fn set_add16_flags(f: &mut FlagsRegister, c: bool, h: bool) {
    let mut flags = (f.get() & (FlagsRegister::SIGN | FlagsRegister::ZERO | FlagsRegister::PARITY_OVERFLOW))
        | (f.get() & (FlagsRegister::FLAG_X1 | FlagsRegister::FLAG_X2));
    if c { flags |= FlagsRegister::CARRY; }
    if h { flags |= FlagsRegister::HALF_CARRY; }
    f.set(flags);
}

/// Flags for ADC/SBC HL: S Z (X) H (X) P/V N C all affected.
pub fn set_adcsbc16_flags(f: &mut FlagsRegister, r: &Alu16Result, n: bool) {
    // X/Y bits (3/5 of F) are undocumented and derived from the result; approximate from low byte bits.
    let mut flags = (r.value as u8) & (FlagsRegister::FLAG_X1 | FlagsRegister::FLAG_X2);
    if r.c { flags |= FlagsRegister::CARRY; }
    if n { flags |= FlagsRegister::ADD_SUBTRACT; }
    if r.pv { flags |= FlagsRegister::PARITY_OVERFLOW; }
    if r.h { flags |= FlagsRegister::HALF_CARRY; }
    if r.value & 0x8000 != 0 { flags |= FlagsRegister::SIGN; }
    if r.value == 0 { flags |= FlagsRegister::ZERO; }
    f.set(flags);
}

/// Flags for DAA: S Z (X) H (X) P(parity) N(preserved) C.
pub fn set_daa_flags(f: &mut FlagsRegister, value: u8, c: bool, h: bool) {
    let mut flags = (f.get() & FlagsRegister::ADD_SUBTRACT)
        | (value & (FlagsRegister::FLAG_X1 | FlagsRegister::FLAG_X2));
    if c { flags |= FlagsRegister::CARRY; }
    if h { flags |= FlagsRegister::HALF_CARRY; }
    if parity(value) { flags |= FlagsRegister::PARITY_OVERFLOW; }
    if value & 0x80 != 0 { flags |= FlagsRegister::SIGN; }
    if value == 0 { flags |= FlagsRegister::ZERO; }
    f.set(flags);
}

/// Flags for RLCA/RLA/RRCA/RRA: C set, H=0 N=0, S Z P/V preserved.
pub fn set_rot_a_flags(f: &mut FlagsRegister, c: bool) {
    let mut flags = (f.get() & (FlagsRegister::SIGN | FlagsRegister::ZERO | FlagsRegister::PARITY_OVERFLOW))
        | (f.get() & (FlagsRegister::FLAG_X1 | FlagsRegister::FLAG_X2));
    if c { flags |= FlagsRegister::CARRY; }
    f.set(flags);
}

/// Flags for shifts/rotates on a target: S Z (X) 0 P 0 0 C.
pub fn set_rot_flags(f: &mut FlagsRegister, value: u8, c: bool) {
    let mut flags = value & (FlagsRegister::FLAG_X1 | FlagsRegister::FLAG_X2);
    if c { flags |= FlagsRegister::CARRY; }
    if parity(value) { flags |= FlagsRegister::PARITY_OVERFLOW; }
    if value & 0x80 != 0 { flags |= FlagsRegister::SIGN; }
    if value == 0 { flags |= FlagsRegister::ZERO; }
    f.set(flags);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flags() -> FlagsRegister {
        FlagsRegister::new()
    }

    #[test]
    fn test_add8_flags() {
        // 0x7F + 0x01 = 0x80: carry unset, half-carry set, overflow set, sign set
        let r = add8(0x7F, 0x01, false);
        assert_eq!(r.value, 0x80);
        assert!(!r.c);
        assert!(r.h);
        assert!(r.pv);

        // 0xFF + 0x01 = 0x00: carry set, zero set
        let r = add8(0xFF, 0x01, false);
        assert_eq!(r.value, 0x00);
        assert!(r.c);
        assert!(r.h);

        // 0x0F + 0x01: half-carry only
        let r = add8(0x0F, 0x01, false);
        assert_eq!(r.value, 0x10);
        assert!(r.h);
        assert!(!r.c);

        // carry in
        let r = add8(0x00, 0x00, true);
        assert_eq!(r.value, 0x01);
    }

    #[test]
    fn test_sub8_flags() {
        // 0x00 - 0x01 = 0xFF: borrow set (C=1), half-borrow set, no overflow (-1 fits in i8)
        let r = sub8(0x00, 0x01, false);
        assert_eq!(r.value, 0xFF);
        assert!(r.c);
        assert!(r.h);
        assert!(!r.pv);

        // 0x80 - 0x01 = 0x7F: overflow set, half-borrow set (0 < 1 in low nibble)
        let r = sub8(0x80, 0x01, false);
        assert_eq!(r.value, 0x7F);
        assert!(r.pv);
        assert!(r.h);
        assert!(!r.c);

        // equal: zero
        let r = sub8(0x42, 0x42, false);
        assert_eq!(r.value, 0x00);
        assert!(!r.c);
        assert!(!r.h);

        // borrow in: 0x10 - 0 - 1 = 0x0F: half-borrow from low nibble, no full borrow
        let r = sub8(0x10, 0x00, true);
        assert_eq!(r.value, 0x0F);
        assert!(!r.c);
        assert!(r.h);
    }

    #[test]
    fn test_parity() {
        assert!(parity(0x00));
        assert!(!parity(0x01));
        assert!(parity(0x03));
        assert!(parity(0xFF));
    }

    #[test]
    fn test_rot() {
        use super::super::RotOp;
        // RLC 0x80 -> 0x01, carry set
        let (v, c) = rot(RotOp::Rlc, 0x80, false);
        assert_eq!(v, 0x01);
        assert!(c);

        // RL with carry in: 0x80 with c=1 -> 0x01, carry set
        let (v, c) = rot(RotOp::Rl, 0x80, true);
        assert_eq!(v, 0x01);
        assert!(c);

        // RR 0x01 with carry in -> 0x80, carry set
        let (v, c) = rot(RotOp::Rr, 0x01, true);
        assert_eq!(v, 0x80);
        assert!(c);

        // RR 0x01 without carry in -> 0x00, carry set
        let (v, c) = rot(RotOp::Rr, 0x01, false);
        assert_eq!(v, 0x00);
        assert!(c);

        // SRA 0x81 (negative) -> 0xC0, carry set (sign preserved)
        let (v, c) = rot(RotOp::Sra, 0x81, false);
        assert_eq!(v, 0xC0);
        assert!(c);

        // SLL 0x80 -> 0x01 (bit 0 set), carry set
        let (v, c) = rot(RotOp::Sll, 0x80, false);
        assert_eq!(v, 0x01);
        assert!(c);

        // SRL 0xFF -> 0x7F, carry set
        let (v, c) = rot(RotOp::Srl, 0xFF, false);
        assert_eq!(v, 0x7F);
        assert!(c);

        // SLA 0x80 -> 0x00, carry set
        let (v, c) = rot(RotOp::Sla, 0x80, false);
        assert_eq!(v, 0x00);
        assert!(c);
    }

    #[test]
    fn test_daa() {
        // After ADD 0x56 + 0x27 = 0x7D with half-carry: BCD 56+27 = 83 -> 0x83
        let mut f = flags();
        f.set_half_carry(true);
        let (r, c, _) = daa(0x7D, &f);
        assert_eq!(r, 0x83);
        assert!(!c);

        // 0x9A with no carry: a > 0x99 -> add 0x66 -> 0x00, carry set
        let f = flags();
        let (r, c, _) = daa(0x9A, &f);
        assert_eq!(r, 0x00);
        assert!(c);

        // with half-carry: 0x0A + 0x01 = 0x0B, H=1 -> add 0x06 -> 0x11
        let mut f = flags();
        f.set_half_carry(true);
        let (r, _, _) = daa(0x0B, &f);
        assert_eq!(r, 0x11);

        // subtraction: 0x10 - 0x01 with H=0, N=1 -> 0x0F -> add 0x06 -> 0x09
        let mut f = flags();
        f.set_add_subtract(true);
        let (r, _, _) = daa(0x0F, &f);
        assert_eq!(r, 0x09);
    }

    #[test]
    fn test_adc16_sbc16() {
        // 0x7FFF + 0x0001 = 0x8000: overflow
        let r = adc16(0x7FFF, 0x0001, false);
        assert_eq!(r.value, 0x8000);
        assert!(!r.c);
        assert!(r.pv);

        // 0xFFFF + 0x0001 = 0x0000: carry
        let r = adc16(0xFFFF, 0x0001, false);
        assert_eq!(r.value, 0x0000);
        assert!(r.c);

        // 0x0000 - 0x0001 = 0xFFFF: borrow
        let r = sbc16(0x0000, 0x0001, false);
        assert_eq!(r.value, 0xFFFF);
        assert!(r.c);
    }
}
