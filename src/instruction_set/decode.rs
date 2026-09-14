// Decoder: turns opcode bytes (with prefix context) into typed Op values.

use crate::memory::{Memory, ProgramCounter, R8, R16, Cond};
use super::{AluOp, Op, RotOp, RotTarget};

pub struct Decoder<'a> {
    mem: &'a Memory,
    pc: &'a mut ProgramCounter,
}

impl<'a> Decoder<'a> {
    fn fetch8(&mut self) -> u8 {
        let byte = self.mem.read(self.pc.get());
        self.pc.inc();
        byte
    }

    fn fetch16(&mut self) -> u16 {
        let low = self.fetch8();
        let high = self.fetch8();
        ((high as u16) << 8) | low as u16
    }

    fn fetch_i8(&mut self) -> i8 {
        self.fetch8() as i8
    }
}

/// Decodes one instruction, advancing PC past all of its bytes.
pub fn decode(mem: &Memory, pc: &mut ProgramCounter) -> Op {
    let mut d = Decoder { mem, pc };
    let mut index: Option<R16> = None;

    let opcode = loop {
        let byte = d.fetch8();
        match byte {
            0xDD => index = Some(R16::IX),
            0xFD => index = Some(R16::IY),
            _ => break byte,
        }
    };

    match opcode {
        0xCB => match index {
            None => decode_cb(&mut d, None, None),
            Some(idx) => {
                let disp = d.fetch_i8();
                decode_cb(&mut d, Some(idx), Some(disp))
            }
        },
        0xED => decode_ed(&mut d),
        _ => decode_base(&mut d, opcode, index),
    }
}

/// Maps an r-table code (0-7) to a register. In index mode, codes 4/5 are
/// the high/low bytes of the index register. Code 6 means (HL)/(IX+d) and
/// must be handled by the caller before calling this.
fn map_r8(code: u8, index: Option<R16>) -> R8 {
    match code {
        0 => R8::B,
        1 => R8::C,
        2 => R8::D,
        3 => R8::E,
        4 => match index {
            Some(R16::IX) => R8::IXH,
            Some(R16::IY) => R8::IYH,
            _ => R8::H,
        },
        5 => match index {
            Some(R16::IX) => R8::IXL,
            Some(R16::IY) => R8::IYL,
            _ => R8::L,
        },
        7 => R8::A,
        _ => unreachable!("(HL) code 6 must be handled before map_r8"),
    }
}

/// Maps a dd-table code (0-3) to a 16-bit pair. In index mode, code 2 is the
/// index register instead of HL.
fn map_r16(code: u8, index: Option<R16>) -> R16 {
    match code & 0x03 {
        0 => R16::BC,
        1 => R16::DE,
        2 => index.unwrap_or(R16::HL),
        _ => R16::SP,
    }
}

fn decode_base(d: &mut Decoder, opcode: u8, index: Option<R16>) -> Op {
    let idx = index.unwrap_or(R16::HL);

    match opcode {
        0x00 => Op::Nop,
        0x01 => { let nn = d.fetch16(); Op::LdR16N(R16::BC, nn) }
        0x02 => Op::LdPtrFromR8(R16::BC, R8::A),
        0x03 => Op::IncR16(R16::BC),
        0x04 => Op::IncR8(R8::B),
        0x05 => Op::DecR8(R8::B),
        0x06 => { let n = d.fetch8(); Op::LdR8N(R8::B, n) }
        0x07 => Op::Rlca,
        0x08 => Op::ExAfAlt,
        0x09 => Op::AddR16(idx, R16::BC),
        0x0A => Op::LdR8FromPtr(R8::A, R16::BC),
        0x0B => Op::DecR16(R16::BC),
        0x0C => Op::IncR8(R8::C),
        0x0D => Op::DecR8(R8::C),
        0x0E => { let n = d.fetch8(); Op::LdR8N(R8::C, n) }
        0x0F => Op::Rrca,

        0x10 => { let e = d.fetch_i8(); Op::Djnz(e) }
        0x11 => { let nn = d.fetch16(); Op::LdR16N(R16::DE, nn) }
        0x12 => Op::LdPtrFromR8(R16::DE, R8::A),
        0x13 => Op::IncR16(R16::DE),
        0x14 => Op::IncR8(R8::D),
        0x15 => Op::DecR8(R8::D),
        0x16 => { let n = d.fetch8(); Op::LdR8N(R8::D, n) }
        0x17 => Op::Rla,
        0x18 => { let e = d.fetch_i8(); Op::Jr(e) }
        0x19 => Op::AddR16(idx, R16::DE),
        0x1A => Op::LdR8FromPtr(R8::A, R16::DE),
        0x1B => Op::DecR16(R16::DE),
        0x1C => Op::IncR8(R8::E),
        0x1D => Op::DecR8(R8::E),
        0x1E => { let n = d.fetch8(); Op::LdR8N(R8::E, n) }
        0x1F => Op::Rra,

        0x20 => { let e = d.fetch_i8(); Op::JrCond(Cond::NZ, e) }
        0x21 => { let nn = d.fetch16(); Op::LdR16N(idx, nn) }
        0x22 => { let nn = d.fetch16(); Op::LdAddrFromR16(nn, idx) }
        0x23 => Op::IncR16(idx),
        0x24 => Op::IncR8(map_r8(4, index)),
        0x25 => Op::DecR8(map_r8(4, index)),
        0x26 => { let n = d.fetch8(); Op::LdR8N(map_r8(4, index), n) }
        0x27 => Op::Daa,
        0x28 => { let e = d.fetch_i8(); Op::JrCond(Cond::Z, e) }
        0x29 => Op::AddR16(idx, idx),
        0x2A => { let nn = d.fetch16(); Op::LdR16FromAddr(idx, nn) }
        0x2B => Op::DecR16(idx),
        0x2C => Op::IncR8(map_r8(5, index)),
        0x2D => Op::DecR8(map_r8(5, index)),
        0x2E => { let n = d.fetch8(); Op::LdR8N(map_r8(5, index), n) }
        0x2F => Op::Cpl,

        0x30 => { let e = d.fetch_i8(); Op::JrCond(Cond::NC, e) }
        0x31 => { let nn = d.fetch16(); Op::LdR16N(R16::SP, nn) }
        0x32 => { let nn = d.fetch16(); Op::LdAddrFromA(nn) }
        0x33 => Op::IncR16(R16::SP),
        0x34 => match index {
            None => Op::IncPtr(R16::HL),
            Some(_) => { let disp = d.fetch_i8(); Op::IncIndexD(idx, disp) }
        },
        0x35 => match index {
            None => Op::DecPtr(R16::HL),
            Some(_) => { let disp = d.fetch_i8(); Op::DecIndexD(idx, disp) }
        },
        0x36 => match index {
            None => { let n = d.fetch8(); Op::LdPtrFromN(R16::HL, n) }
            Some(_) => {
                let disp = d.fetch_i8();
                let n = d.fetch8();
                Op::LdIndexDFromN(idx, disp, n)
            }
        },
        0x37 => Op::Scf,
        0x38 => { let e = d.fetch_i8(); Op::JrCond(Cond::C, e) }
        0x39 => Op::AddR16(idx, R16::SP),
        0x3A => { let nn = d.fetch16(); Op::LdAFromAddr(nn) }
        0x3B => Op::DecR16(R16::SP),
        0x3C => Op::IncR8(R8::A),
        0x3D => Op::DecR8(R8::A),
        0x3E => { let n = d.fetch8(); Op::LdR8N(R8::A, n) }
        0x3F => Op::Ccf,

        0x40..=0x7F => {
            let dst_code = (opcode >> 3) & 0x07;
            let src_code = opcode & 0x07;
            if dst_code == 6 && src_code == 6 {
                // 0x76: HALT (also DD 76 / FD 76)
                return Op::Halt;
            }
            match (dst_code != 6, src_code != 6) {
                (true, true) => Op::LdR8R8(map_r8(dst_code, index), map_r8(src_code, index)),
                (true, false) => match index {
                    None => Op::LdR8FromPtr(map_r8(dst_code, index), R16::HL),
                    Some(_) => Op::LdR8FromIndexD(map_r8(dst_code, index), idx, d.fetch_i8()),
                },
                (false, true) => match index {
                    None => Op::LdPtrFromR8(R16::HL, map_r8(src_code, index)),
                    Some(_) => Op::LdIndexDFromR8(idx, d.fetch_i8(), map_r8(src_code, index)),
                },
                (false, false) => unreachable!(),
            }
        }

        0x80..=0xBF => {
            let alu = AluOp::from_bits((opcode >> 3) & 0x07);
            let src_code = opcode & 0x07;
            match (src_code, index) {
                (6, None) => Op::AluPtr(alu, R16::HL),
                (6, Some(_)) => Op::AluIndexD(alu, idx, d.fetch_i8()),
                (_, ix) => Op::AluR8(alu, map_r8(src_code, ix)),
            }
        }

        0xC0..=0xFF => decode_base_high(d, opcode, index),

        _ => unreachable!(),
    }
}

fn decode_base_high(d: &mut Decoder, opcode: u8, index: Option<R16>) -> Op {
    let cond_bits = (opcode >> 3) & 0x07;
    let idx = index.unwrap_or(R16::HL);

    match opcode {
        0xC3 => { let nn = d.fetch16(); Op::Jp(nn) }
        0xCD => { let nn = d.fetch16(); Op::Call(nn) }
        0xC9 => Op::Ret,
        0xD3 => { let n = d.fetch8(); Op::OutAN(n) }
        0xDB => { let n = d.fetch8(); Op::InAN(n) }
        0xD9 => Op::Exx,
        0xE3 => Op::ExSpPtr(idx),
        0xE9 => Op::JpPtr(idx),
        0xF9 => Op::LdSpFromPtr(idx),
        0xEB => Op::ExDeHl,
        0xF3 => Op::Di,
        0xFB => Op::Ei,

        _ if opcode & 0xC7 == 0xC0 => Op::RetCond(Cond::from_bits(cond_bits)),
        _ if opcode & 0xC7 == 0xC2 => { let nn = d.fetch16(); Op::JpCond(Cond::from_bits(cond_bits), nn) }
        _ if opcode & 0xC7 == 0xC4 => { let nn = d.fetch16(); Op::CallCond(Cond::from_bits(cond_bits), nn) }
        _ if opcode & 0xC7 == 0xC5 => Op::Push(map_push_pop((opcode >> 4) & 3, index)),
        _ if opcode & 0xC7 == 0xC1 => Op::Pop(map_push_pop((opcode >> 4) & 3, index)),
        _ if opcode & 0xC7 == 0xC7 => Op::Rst(opcode & 0x38),
        _ if opcode & 0xC7 == 0xC6 => { let n = d.fetch8(); Op::AluN(AluOp::from_bits(cond_bits), n) }

        _ => Op::Unimplemented { prefix: None, opcode },
    }
}

fn map_push_pop(code: u8, index: Option<R16>) -> R16 {
    match code & 0x03 {
        0 => R16::BC,
        1 => R16::DE,
        2 => index.unwrap_or(R16::HL),
        _ => R16::AF,
    }
}

fn decode_cb(d: &mut Decoder, index: Option<R16>, disp: Option<i8>) -> Op {
    let opcode = d.fetch8();
    let code = opcode & 0x07;
    let target = match (disp, code) {
        (None, 6) => RotTarget::Ptr(R16::HL),
        (None, _) => RotTarget::R8(map_r8(code, None)),
        (Some(disp), _) => RotTarget::IndexD(index.expect("index required for DDCB/FDCB"), disp),
    };
    match opcode >> 6 {
        0 => Op::Rot(RotOp::from_bits((opcode >> 3) & 0x07), target),
        1 => Op::Bit((opcode >> 3) & 0x07, target),
        2 => Op::Res((opcode >> 3) & 0x07, target),
        _ => Op::Set((opcode >> 3) & 0x07, target),
    }
}

fn decode_ed(d: &mut Decoder) -> Op {
    let opcode = d.fetch8();

    match opcode {
        0x46 => Op::Im(0),
        0x56 => Op::Im(1),
        0x5E => Op::Im(2),
        0x44 => Op::Neg,
        0x4D => Op::Reti,
        0x45 | 0x55 | 0x5D | 0x65 | 0x6D | 0x75 | 0x7D => Op::Retn,
        0x57 => Op::LdAFromI,
        0x5F => Op::LdAFromR,
        0x47 => Op::LdIFromA,
        0x4F => Op::LdRFromA,
        0x67 => Op::Rrd,
        0x6F => Op::Rld,

        0x42 | 0x52 | 0x62 | 0x72 => Op::SbcR16(R16::HL, map_ed_pair((opcode >> 4) & 3)),
        0x4A | 0x5A | 0x6A | 0x7A => Op::AdcR16(R16::HL, map_ed_pair((opcode >> 4) & 3)),
        0x43 | 0x53 | 0x63 | 0x73 => { let nn = d.fetch16(); Op::LdAddrFromR16(nn, map_ed_pair((opcode >> 4) & 3)) }
        0x4B | 0x5B | 0x6B | 0x7B => { let nn = d.fetch16(); Op::LdR16FromAddr(map_ed_pair((opcode >> 4) & 3), nn) }

        0x40..=0x7F => {
            let code = (opcode >> 3) & 0x07;
            if code == 6 {
                return Op::Unimplemented { prefix: Some(0xED), opcode }; // ED 70/71
            }
            let r = map_r8(code, None);
            if opcode & 0x01 == 0 {
                Op::InR8C(r)
            } else {
                Op::OutCR8(r)
            }
        }

        0xA0..=0xBF => {
            // Layout: kind in bits 1-0 (00=LD 01=CP 10=IN 11=OUT), bit 3 = decrement, bit 4 = repeat
            let kind = opcode & 0x03;
            let decrement = opcode & 0x08 != 0;
            let repeat = opcode & 0x10 != 0;
            match kind {
                0 => match (decrement, repeat) {
                    (false, false) => Op::Ldi,
                    (false, true) => Op::Ldir,
                    (true, false) => Op::Ldd,
                    (true, true) => Op::Lddr,
                },
                1 => match (decrement, repeat) {
                    (false, false) => Op::Cpi,
                    (false, true) => Op::Cpir,
                    (true, false) => Op::Cpd,
                    (true, true) => Op::Cpdr,
                },
                _ => Op::Unimplemented { prefix: Some(0xED), opcode }, // block IO
            }
        }

        _ => Op::Unimplemented { prefix: Some(0xED), opcode },
    }
}

fn map_ed_pair(code: u8) -> R16 {
    match code & 0x03 {
        0 => R16::BC,
        1 => R16::DE,
        2 => R16::HL,
        _ => R16::SP,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::Memory;

    fn decode_bytes(bytes: &[u8]) -> Op {
        let mut mem = Memory::default();
        for (i, b) in bytes.iter().enumerate() {
            mem.write(i as u16, *b);
        }
        let mut pc = ProgramCounter::new();
        decode(&mem, &mut pc)
    }

    #[test]
    fn test_decode_ld_bc_nn() {
        assert_eq!(decode_bytes(&[0x01, 0x34, 0x12]), Op::LdR16N(R16::BC, 0x1234));
    }

    #[test]
    fn test_decode_ld_matrix() {
        assert_eq!(decode_bytes(&[0x41]), Op::LdR8R8(R8::B, R8::C));
        assert_eq!(decode_bytes(&[0x47]), Op::LdR8R8(R8::B, R8::A));
        assert_eq!(decode_bytes(&[0x4E]), Op::LdR8FromPtr(R8::C, R16::HL));
        assert_eq!(decode_bytes(&[0x70]), Op::LdPtrFromR8(R16::HL, R8::B));
        assert_eq!(decode_bytes(&[0x36, 0xAB]), Op::LdPtrFromN(R16::HL, 0xAB));
        assert_eq!(decode_bytes(&[0x77]), Op::LdPtrFromR8(R16::HL, R8::A));
        assert_eq!(decode_bytes(&[0x3E, 0x10]), Op::LdR8N(R8::A, 0x10));
        assert_eq!(decode_bytes(&[0x34]), Op::IncPtr(R16::HL));
        assert_eq!(decode_bytes(&[0x35]), Op::DecPtr(R16::HL));
    }

    #[test]
    fn test_decode_alu() {
        assert_eq!(decode_bytes(&[0x86]), Op::AluPtr(AluOp::Add, R16::HL));
        assert_eq!(decode_bytes(&[0xB9]), Op::AluR8(AluOp::Cp, R8::C));
        assert_eq!(decode_bytes(&[0xC6, 0x05]), Op::AluN(AluOp::Add, 0x05));
        assert_eq!(decode_bytes(&[0xFE, 0x20]), Op::AluN(AluOp::Cp, 0x20));
    }

    #[test]
    fn test_decode_index() {
        assert_eq!(decode_bytes(&[0xDD, 0x21, 0x00, 0x40]), Op::LdR16N(R16::IX, 0x4000));
        assert_eq!(decode_bytes(&[0xDD, 0x46, 0x03]), Op::LdR8FromIndexD(R8::B, R16::IX, 0x03));
        assert_eq!(decode_bytes(&[0xDD, 0x86, 0x7F]), Op::AluIndexD(AluOp::Add, R16::IX, 0x7F));
        assert_eq!(decode_bytes(&[0xFD, 0xE1]), Op::Pop(R16::IY));
        assert_eq!(decode_bytes(&[0xDD, 0x64]), Op::LdR8R8(R8::IXH, R8::IXH));
        assert_eq!(decode_bytes(&[0xDD, 0x7C]), Op::LdR8R8(R8::A, R8::IXH));
        assert_eq!(decode_bytes(&[0xDD, 0x34, 0x02]), Op::IncIndexD(R16::IX, 0x02));
        assert_eq!(decode_bytes(&[0xDD, 0x36, 0x02, 0x77]), Op::LdIndexDFromN(R16::IX, 0x02, 0x77));
    }

    #[test]
    fn test_decode_cb() {
        assert_eq!(decode_bytes(&[0xCB, 0x38]), Op::Rot(RotOp::Srl, RotTarget::R8(R8::B)));
        assert_eq!(decode_bytes(&[0xCB, 0x7E]), Op::Bit(7, RotTarget::Ptr(R16::HL)));
        assert_eq!(decode_bytes(&[0xCB, 0xC6]), Op::Set(0, RotTarget::Ptr(R16::HL)));
        assert_eq!(decode_bytes(&[0xDD, 0xCB, 0x01, 0x38]), Op::Rot(RotOp::Srl, RotTarget::IndexD(R16::IX, 0x01)));
        assert_eq!(decode_bytes(&[0xFD, 0xCB, 0x02, 0x7E]), Op::Bit(7, RotTarget::IndexD(R16::IY, 0x02)));
    }

    #[test]
    fn test_decode_ed() {
        assert_eq!(decode_bytes(&[0xED, 0x56]), Op::Im(1));
        assert_eq!(decode_bytes(&[0xED, 0x78]), Op::InR8C(R8::A));
        assert_eq!(decode_bytes(&[0xED, 0x79]), Op::OutCR8(R8::A));
        assert_eq!(decode_bytes(&[0xED, 0x49]), Op::OutCR8(R8::C));
        assert_eq!(decode_bytes(&[0xED, 0x5B, 0x00, 0x60]), Op::LdR16FromAddr(R16::DE, 0x6000));
        assert_eq!(decode_bytes(&[0xED, 0xB0]), Op::Ldir);
    }

    #[test]
    fn test_decode_control_flow() {
        assert_eq!(decode_bytes(&[0xC3, 0x00, 0x01]), Op::Jp(0x0100));
        assert_eq!(decode_bytes(&[0xC2, 0xAA, 0xFF]), Op::JpCond(Cond::NZ, 0xFFAA));
        assert_eq!(decode_bytes(&[0xF2, 0xAA, 0xFF]), Op::JpCond(Cond::P, 0xFFAA));
        assert_eq!(decode_bytes(&[0x18, 0xFE]), Op::Jr(-2));
        assert_eq!(decode_bytes(&[0xE9]), Op::JpPtr(R16::HL));
        assert_eq!(decode_bytes(&[0xC7]), Op::Rst(0x00));
        assert_eq!(decode_bytes(&[0xFF]), Op::Rst(0x38));
    }

    #[test]
    fn test_decode_never_panics_for_any_byte() {
        for byte in 0..=255u16 {
            let mut mem = Memory::default();
            mem.write(0, byte as u8);
            let mut pc = ProgramCounter::new();
            decode(&mem, &mut pc);
        }
        for byte in 0..=255u16 {
            let mut mem = Memory::default();
            mem.write(0, 0xED);
            mem.write(1, byte as u8);
            let mut pc = ProgramCounter::new();
            decode(&mem, &mut pc);
        }
        for byte in 0..=255u16 {
            let mut mem = Memory::default();
            mem.write(0, 0xCB);
            mem.write(1, byte as u8);
            let mut pc = ProgramCounter::new();
            decode(&mem, &mut pc);
        }
        for byte in 0..=255u16 {
            let mut mem = Memory::default();
            mem.write(0, 0xDD);
            mem.write(1, 0xCB);
            mem.write(2, 0x05);
            mem.write(3, byte as u8);
            let mut pc = ProgramCounter::new();
            decode(&mem, &mut pc);
        }
    }
}
