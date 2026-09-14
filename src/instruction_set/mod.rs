pub mod alu;
pub mod decode;
pub mod ops;

use crate::memory::{R8, R16, Cond};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AluOp { Add, Adc, Sub, Sbc, And, Xor, Or, Cp }

impl AluOp {
    pub fn from_bits(bits: u8) -> AluOp {
        match bits & 0x07 {
            0 => AluOp::Add,
            1 => AluOp::Adc,
            2 => AluOp::Sub,
            3 => AluOp::Sbc,
            4 => AluOp::And,
            5 => AluOp::Xor,
            6 => AluOp::Or,
            _ => AluOp::Cp,
        }
    }

    pub fn mnemonic(&self) -> &'static str {
        match self {
            AluOp::Add => "ADD A,",
            AluOp::Adc => "ADC A,",
            AluOp::Sub => "SUB ",
            AluOp::Sbc => "SBC A,",
            AluOp::And => "AND ",
            AluOp::Xor => "XOR ",
            AluOp::Or => "OR ",
            AluOp::Cp => "CP ",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RotOp { Rlc, Rrc, Rl, Rr, Sla, Sra, Sll, Srl }

impl RotOp {
    pub fn from_bits(bits: u8) -> RotOp {
        match bits & 0x07 {
            0 => RotOp::Rlc,
            1 => RotOp::Rrc,
            2 => RotOp::Rl,
            3 => RotOp::Rr,
            4 => RotOp::Sla,
            5 => RotOp::Sra,
            6 => RotOp::Sll,
            _ => RotOp::Srl,
        }
    }

    pub fn mnemonic(&self) -> &'static str {
        match self {
            RotOp::Rlc => "RLC",
            RotOp::Rrc => "RRC",
            RotOp::Rl => "RL",
            RotOp::Rr => "RR",
            RotOp::Sla => "SLA",
            RotOp::Sra => "SRA",
            RotOp::Sll => "SLL",
            RotOp::Srl => "SRL",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RotTarget {
    R8(R8),
    Ptr(R16),
    IndexD(R16, i8),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Op {
    Unimplemented { prefix: Option<u8>, opcode: u8 },
    Nop,
    Halt,
    Di,
    Ei,

    // 8-bit loads
    LdR8R8(R8, R8),
    LdR8N(R8, u8),
    LdR8FromPtr(R8, R16),
    LdPtrFromR8(R16, R8),
    LdPtrFromN(R16, u8),
    LdR8FromIndexD(R8, R16, i8),
    LdIndexDFromR8(R16, i8, R8),
    LdIndexDFromN(R16, i8, u8),
    LdAddrFromA(u16),
    LdAFromAddr(u16),

    // 16-bit loads
    LdR16N(R16, u16),
    LdR16FromAddr(R16, u16),
    LdAddrFromR16(u16, R16),
    LdSpFromPtr(R16),

    // stack + exchanges
    Push(R16),
    Pop(R16),
    ExDeHl,
    ExAfAlt,
    ExSpPtr(R16),
    Exx,

    // ALU
    AluR8(AluOp, R8),
    AluPtr(AluOp, R16),
    AluIndexD(AluOp, R16, i8),
    AluN(AluOp, u8),

    // inc / dec
    IncR8(R8),
    DecR8(R8),
    IncPtr(R16),
    DecPtr(R16),
    IncIndexD(R16, i8),
    DecIndexD(R16, i8),
    IncR16(R16),
    DecR16(R16),
    AddR16(R16, R16),
    AdcR16(R16, R16),
    SbcR16(R16, R16),

    // control flow
    Jp(u16),
    JpCond(Cond, u16),
    JpPtr(R16),
    Jr(i8),
    JrCond(Cond, i8),
    Djnz(i8),
    Call(u16),
    CallCond(Cond, u16),
    Ret,
    RetCond(Cond),
    Reti,
    Retn,
    Rst(u8),

    // accumulator / flag ops
    Daa,
    Cpl,
    Scf,
    Ccf,
    Rlca,
    Rrca,
    Rla,
    Rra,

    // CB set
    Rot(RotOp, RotTarget),
    Bit(u8, RotTarget),
    Res(u8, RotTarget),
    Set(u8, RotTarget),

    // ED set
    Im(u8),
    Neg,
    LdAFromI,
    LdAFromR,
    LdIFromA,
    LdRFromA,
    Rrd,
    Rld,
    InR8C(R8),
    OutCR8(R8),
    Ldi,
    Ldir,
    Ldd,
    Lddr,
    Cpi,
    Cpir,
    Cpd,
    Cpdr,

    // base IO
    InAN(u8),
    OutAN(u8),
}

pub fn assembly(op: &Op) -> String {
    fn r8_name(r: &R8) -> &'static str {
        match r {
            R8::A => "A", R8::F => "F", R8::B => "B", R8::C => "C",
            R8::D => "D", R8::E => "E", R8::H => "H", R8::L => "L",
            R8::IXH => "IXH", R8::IXL => "IXL", R8::IYH => "IYH", R8::IYL => "IYL",
            R8::I => "I", R8::R => "R",
        }
    }
    fn r16_name(r: &R16) -> &'static str {
        match r {
            R16::AF => "AF", R16::BC => "BC", R16::DE => "DE", R16::HL => "HL",
            R16::IX => "IX", R16::IY => "IY", R16::SP => "SP", R16::PC => "PC",
        }
    }
    fn cond_name(c: &Cond) -> &'static str {
        match c {
            Cond::NZ => "NZ", Cond::Z => "Z", Cond::NC => "NC", Cond::C => "C",
            Cond::PO => "PO", Cond::PE => "PE", Cond::P => "P", Cond::M => "M",
        }
    }
    fn alu_target(a: &AluOp, target: String) -> String {
        format!("{}{}", a.mnemonic(), target)
    }
    fn ptr_name(r: &R16) -> String {
        format!("({})", r16_name(r))
    }
    fn index_name(r: &R16, d: &i8) -> String {
        if *d < 0 {
            format!("({}-{:02X})", r16_name(r), -*d as u16)
        } else {
            format!("({}+{:02X})", r16_name(r), d)
        }
    }
    fn rot_target_name(t: &RotTarget) -> String {
        match t {
            RotTarget::R8(r) => r8_name(r).to_string(),
            RotTarget::Ptr(p) => ptr_name(p),
            RotTarget::IndexD(p, d) => index_name(p, d),
        }
    }
    fn nn_name(v: u16) -> String {
        format!("{:04X}", v)
    }
    fn n_name(v: u8) -> String {
        format!("{:02X}", v)
    }

    match op {
        Op::Unimplemented { prefix, opcode } => match prefix {
            Some(p) => format!("UNIMPLEMENTED {:02X} {:02X}", p, opcode),
            None => format!("UNIMPLEMENTED {:02X}", opcode),
        },
        Op::Nop => "NOP".to_string(),
        Op::Halt => "HALT".to_string(),
        Op::Di => "DI".to_string(),
        Op::Ei => "EI".to_string(),

        Op::LdR8R8(dst, src) => format!("LD {},{}", r8_name(dst), r8_name(src)),
        Op::LdR8N(r, n) => format!("LD {},{}", r8_name(r), n_name(*n)),
        Op::LdR8FromPtr(r, p) => format!("LD {},{}", r8_name(r), ptr_name(p)),
        Op::LdPtrFromR8(p, r) => format!("LD {},{}", ptr_name(p), r8_name(r)),
        Op::LdPtrFromN(p, n) => format!("LD {},{}", ptr_name(p), n_name(*n)),
        Op::LdR8FromIndexD(r, p, d) => format!("LD {},{}", r8_name(r), index_name(p, d)),
        Op::LdIndexDFromR8(p, d, r) => format!("LD {},{}", index_name(p, d), r8_name(r)),
        Op::LdIndexDFromN(p, d, n) => format!("LD {},{}", index_name(p, d), n_name(*n)),
        Op::LdAddrFromA(a) => format!("LD ({}),A", nn_name(*a)),
        Op::LdAFromAddr(a) => format!("LD A,({})", nn_name(*a)),

        Op::LdR16N(r, nn) => format!("LD {},{}", r16_name(r), nn_name(*nn)),
        Op::LdR16FromAddr(r, a) => format!("LD {},({})", r16_name(r), nn_name(*a)),
        Op::LdAddrFromR16(a, r) => format!("LD ({}),{}", nn_name(*a), r16_name(r)),
        Op::LdSpFromPtr(r) => format!("LD SP,{}", r16_name(r)),

        Op::Push(r) => format!("PUSH {}", r16_name(r)),
        Op::Pop(r) => format!("POP {}", r16_name(r)),
        Op::ExDeHl => "EX DE,HL".to_string(),
        Op::ExAfAlt => "EX AF,AF'".to_string(),
        Op::ExSpPtr(r) => format!("EX (SP),{}", r16_name(r)),
        Op::Exx => "EXX".to_string(),

        Op::AluR8(a, r) => alu_target(a, r8_name(r).to_string()),
        Op::AluPtr(a, p) => alu_target(a, ptr_name(p)),
        Op::AluIndexD(a, p, d) => alu_target(a, index_name(p, d)),
        Op::AluN(a, n) => alu_target(a, n_name(*n)),

        Op::IncR8(r) => format!("INC {}", r8_name(r)),
        Op::DecR8(r) => format!("DEC {}", r8_name(r)),
        Op::IncPtr(r) => format!("INC {}", ptr_name(r)),
        Op::DecPtr(r) => format!("DEC {}", ptr_name(r)),
        Op::IncIndexD(p, d) => format!("INC {}", index_name(p, d)),
        Op::DecIndexD(p, d) => format!("DEC {}", index_name(p, d)),
        Op::IncR16(r) => format!("INC {}", r16_name(r)),
        Op::DecR16(r) => format!("DEC {}", r16_name(r)),
        Op::AddR16(dst, src) => format!("ADD {},{}", r16_name(dst), r16_name(src)),
        Op::AdcR16(dst, src) => format!("ADC {},{}", r16_name(dst), r16_name(src)),
        Op::SbcR16(dst, src) => format!("SBC {},{}", r16_name(dst), r16_name(src)),

        Op::Jp(a) => format!("JP {}", nn_name(*a)),
        Op::JpCond(c, a) => format!("JP {},{}", cond_name(c), nn_name(*a)),
        Op::JpPtr(r) => format!("JP {}", ptr_name(r)),
        Op::Jr(d) => format!("JR {}", d),
        Op::JrCond(c, d) => format!("JR {},{}", cond_name(c), d),
        Op::Djnz(d) => format!("DJNZ {}", d),
        Op::Call(a) => format!("CALL {}", nn_name(*a)),
        Op::CallCond(c, a) => format!("CALL {},{}", cond_name(c), nn_name(*a)),
        Op::Ret => "RET".to_string(),
        Op::RetCond(c) => format!("RET {}", cond_name(c)),
        Op::Reti => "RETI".to_string(),
        Op::Retn => "RETN".to_string(),
        Op::Rst(v) => format!("RST {:02X}h", v),

        Op::Daa => "DAA".to_string(),
        Op::Cpl => "CPL".to_string(),
        Op::Scf => "SCF".to_string(),
        Op::Ccf => "CCF".to_string(),
        Op::Rlca => "RLCA".to_string(),
        Op::Rrca => "RRCA".to_string(),
        Op::Rla => "RLA".to_string(),
        Op::Rra => "RRA".to_string(),

        Op::Rot(o, t) => format!("{} {}", o.mnemonic(), rot_target_name(t)),
        Op::Bit(b, t) => format!("BIT {},{}", b, rot_target_name(t)),
        Op::Res(b, t) => format!("RES {},{}", b, rot_target_name(t)),
        Op::Set(b, t) => format!("SET {},{}", b, rot_target_name(t)),

        Op::Im(m) => format!("IM {}", m),
        Op::Neg => "NEG".to_string(),
        Op::LdAFromI => "LD A,I".to_string(),
        Op::LdAFromR => "LD A,R".to_string(),
        Op::LdIFromA => "LD I,A".to_string(),
        Op::LdRFromA => "LD R,A".to_string(),
        Op::Rrd => "RRD".to_string(),
        Op::Rld => "RLD".to_string(),
        Op::InR8C(r) => format!("IN {},(C)", r8_name(r)),
        Op::OutCR8(r) => format!("OUT (C),{}", r8_name(r)),
        Op::Ldi => "LDI".to_string(),
        Op::Ldir => "LDIR".to_string(),
        Op::Ldd => "LDD".to_string(),
        Op::Lddr => "LDDR".to_string(),
        Op::Cpi => "CPI".to_string(),
        Op::Cpir => "CPIR".to_string(),
        Op::Cpd => "CPD".to_string(),
        Op::Cpdr => "CPDR".to_string(),

        Op::InAN(n) => format!("IN A,({:02X})", n),
        Op::OutAN(n) => format!("OUT ({:02X}),A", n),
    }
}
