use std::fmt;

use crate::utils::{split_double_byte, combine_to_double_byte};

/// The gate array RAM configurations for a 128K machine: each entry maps the
/// four 16K address regions to a physical RAM block. Blocks 0-3 form the
/// standard 64K bank; blocks 4-7 are the expansion bank.
const RAM_CONFIGURATIONS: [[usize; 4]; 8] = [
    [0, 1, 2, 3],
    [0, 1, 2, 7],
    [4, 5, 6, 7],
    [0, 3, 2, 7],
    [0, 4, 2, 3],
    [0, 5, 2, 3],
    [0, 6, 2, 3],
    [0, 7, 2, 3],
];

pub struct Memory {
    /// Physical RAM: 8 x 16K blocks = 128K (6128).
    blocks: [[u8; 0x4000]; 8],
    /// Current RAM configuration: address region (0-3) -> physical block.
    ram_config: [usize; 4],
    rom_os: [u8; 0x4000],
    /// Upper ROM slots 0-15 (16K each). Slot 0 = BASIC; the 6128's soldered
    /// AMSDOS conventionally occupies slot 7. Unfitted sockets float high.
    rom_expansions: [Option<Box<[u8; 0x4000]>>; 16],

    // Gate array ROM configuration (updated via port writes)
    pub lower_rom_enabled: bool,
    pub upper_rom_enabled: bool,
    /// Selected upper ROM number.
    pub upper_rom_number: u8,
    /// Diagnostic: global instruction counter, updated by the runtime and
    /// shown in the per-instruction debug trace.
    pub diagnostics_step: u64,
}

impl Memory {
    pub fn default() -> Memory {
        Memory {
            blocks: [[0x01; 0x4000]; 8],
            ram_config: [0, 1, 2, 3],
            rom_os: [0x00; 0x4000],
            rom_expansions: Default::default(),
            lower_rom_enabled: false,
            upper_rom_enabled: false,
            upper_rom_number: 0,
            diagnostics_step: 0,
        }
    }

    pub fn read(&self, address: u16) -> u8 {
        if address < 0x4000 && self.lower_rom_enabled {
            return self.rom_os[address as usize];
        }
        if address >= 0xC000 && self.upper_rom_enabled {
            // Only fitted ROM slots answer; empty sockets float high. The
            // firmware's ROM scan depends on reading 0xFF from empty sockets
            // to skip them.
            if let Some(rom) = &self.rom_expansions[self.upper_rom_number as usize] {
                return rom[(address - 0xC000) as usize];
            }
            return 0xFF;
        }
        let region = (address >> 14) as usize;
        self.blocks[self.ram_config[region]][(address & 0x3FFF) as usize]
    }

    pub fn write(&mut self, address: u16, value: u8) {
        let region = (address >> 14) as usize;
        self.blocks[self.ram_config[region]][(address & 0x3FFF) as usize] = value;
    }

    /// Reads video RAM the way the gate array's video fetch does: always the
    /// base 64K RAM via the raw video address. The CRTC/gate-array fetch never
    /// sees the ROM configuration (ROMs overlay CPU reads only — this is why
    /// the firmware screen displays while the BASIC ROM is paged in at &C000)
    /// nor the RAM configuration ("VRAM is always located in the first 64K, in
    /// no way affected by this register" — CPCWiki Gate Array). Rendering
    /// through `read` instead draws ROM bytes or a paged expansion bank
    /// whenever the machine reconfigures memory with the screen static — the
    /// "random garbage" flashes.
    pub fn read_video_ram(&self, address: u16) -> u8 {
        self.blocks[(address >> 14) as usize][(address & 0x3FFF) as usize]
    }

    /// Selects one of the gate array RAM configurations (value written with
    /// bits 7-6 set to 11 on ports 0x0000-0x7FFF).
    pub fn set_ram_config(&mut self, config: u8) {
        self.ram_config = RAM_CONFIGURATIONS[(config & 0x07) as usize];
    }

    /// Records the upper ROM selection. With only BASIC fitted, every slot
    /// resolves to the BASIC image, so this is bookkeeping only.
    /// The hardware latch holds a 4-bit ROM number (16 slots, mirrored at the
    /// partial decode); guard reads against out-of-range values.
    pub fn set_upper_rom_number(&mut self, number: u8) {
        self.upper_rom_number = number & 0x0F;
    }


    pub fn read16(&self, address: u16) -> u16 {
        let low = self.read(address);
        let high = self.read(address.wrapping_add(1));
        combine_to_double_byte(high, low)
    }

    pub fn write16(&mut self, address: u16, value: u16) {
        let (high, low) = split_double_byte(value);
        self.write(address, low);
        self.write(address.wrapping_add(1), high);
    }

    /// Loads the OS ROM image (lower ROM, 0x0000-0x3FFF).
    pub fn load_os_rom(&mut self, bytes: &[u8; 0x4000]) {
        self.rom_os.copy_from_slice(bytes);
        self.lower_rom_enabled = true;
    }

    /// Loads the expansion ROM image (upper ROM, 0xC000-0xFFFF, e.g. BASIC)
    /// into the given slot, and enables upper ROM banking, as a real machine
    /// does when a ROM is fitted.
    pub fn load_expansion_rom_into_slot(&mut self, bytes: &[u8; 0x4000], slot: u8) {
        let mut rom = Box::new([0x00; 0x4000]);
        rom.copy_from_slice(bytes);
        self.rom_expansions[slot as usize] = Some(rom);
        self.upper_rom_enabled = true;
    }

    /// The BASIC ROM conventionally occupies upper ROM slot 0.
    pub fn load_expansion_rom(&mut self, bytes: &[u8; 0x4000]) {
        self.load_expansion_rom_into_slot(bytes, 0);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum R8 { A, F, B, C, D, E, H, L, IXH, IXL, IYH, IYL, I, R }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum R16 { AF, BC, DE, HL, IX, IY, SP, PC }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cond { NZ, Z, NC, C, PO, PE, P, M }

impl Cond {
    pub fn from_bits(bits: u8) -> Cond {
        match bits & 0x07 {
            0 => Cond::NZ,
            1 => Cond::Z,
            2 => Cond::NC,
            3 => Cond::C,
            4 => Cond::PO,
            5 => Cond::PE,
            6 => Cond::P,
            _ => Cond::M,
        }
    }

    pub fn evaluate(&self, flags: &FlagsRegister) -> bool {
        match self {
            Cond::NZ => !flags.zero(),
            Cond::Z => flags.zero(),
            Cond::NC => !flags.carry(),
            Cond::C => flags.carry(),
            Cond::PO => !flags.parity_overflow(),
            Cond::PE => flags.parity_overflow(),
            Cond::P => !flags.sign(),
            Cond::M => flags.sign(),
        }
    }
}

pub struct DefaultRegister {
    name: String,
    value: u8
}

impl DefaultRegister {
    pub fn new(name: &str) -> DefaultRegister {
        DefaultRegister { name: name.to_string(), value: 0 }
    }

    pub fn set(&mut self, value: u8) {
        self.value = value;
    }

    pub fn get(&self) -> u8 {
        self.value
    }
}

impl fmt::Debug for DefaultRegister {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{:02X}", self.name, self.value)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct FlagsRegister {
    value: u8
}

impl FlagsRegister {
    pub fn new() -> FlagsRegister {
        FlagsRegister { value: 0 }
    }

    // Bit       7 6 5 4 3 2 1 0
    // Position  S Z / H / P/V N C
    pub const CARRY: u8 = 0x01;
    pub const ADD_SUBTRACT: u8 = 0x02;
    pub const PARITY_OVERFLOW: u8 = 0x04;
    pub const FLAG_X1: u8 = 0x08;
    pub const HALF_CARRY: u8 = 0x10;
    pub const FLAG_X2: u8 = 0x20;
    pub const ZERO: u8 = 0x40;
    pub const SIGN: u8 = 0x80;

    pub fn get(&self) -> u8 {
        self.value
    }

    pub fn set(&mut self, value: u8) {
        self.value = value;
    }

    fn set_flag(&mut self, mask: u8, set: bool) {
        if set {
            self.value |= mask;
        } else {
            self.value &= !mask;
        }
    }

    pub fn carry(&self) -> bool { self.value & Self::CARRY != 0 }
    pub fn set_carry(&mut self, set: bool) { self.set_flag(Self::CARRY, set); }

    pub fn add_subtract(&self) -> bool { self.value & Self::ADD_SUBTRACT != 0 }
    pub fn set_add_subtract(&mut self, set: bool) { self.set_flag(Self::ADD_SUBTRACT, set); }

    pub fn parity_overflow(&self) -> bool { self.value & Self::PARITY_OVERFLOW != 0 }
    pub fn set_parity_overflow(&mut self, set: bool) { self.set_flag(Self::PARITY_OVERFLOW, set); }

    pub fn half_carry(&self) -> bool { self.value & Self::HALF_CARRY != 0 }
    pub fn set_half_carry(&mut self, set: bool) { self.set_flag(Self::HALF_CARRY, set); }

    pub fn zero(&self) -> bool { self.value & Self::ZERO != 0 }
    pub fn set_zero(&mut self, set: bool) { self.set_flag(Self::ZERO, set); }

    pub fn sign(&self) -> bool { self.value & Self::SIGN != 0 }
    pub fn set_sign(&mut self, set: bool) { self.set_flag(Self::SIGN, set); }
}

pub struct ProgramCounter {
    value: u16
}

impl ProgramCounter {
    pub fn new() -> ProgramCounter {
        ProgramCounter { value: 0 }
    }

    pub fn set(&mut self, value: u16) {
        self.value = value;
    }

    pub fn get(&self) -> u16 {
        self.value
    }

    pub fn inc(&mut self) {
        self.value = self.value.wrapping_add(1);
    }
}

pub struct StackPointer {
    location: u16
}

impl StackPointer {
    pub fn get(&self) -> u16 {
        self.location
    }

    pub fn set(&mut self, value: u16) {
        self.location = value;
    }

    pub fn push(&mut self, memory: &mut Memory, value: u16) {
        let (high, low) = split_double_byte(value);
        self.location = self.location.wrapping_sub(1);
        memory.write(self.location, high);
        self.location = self.location.wrapping_sub(1);
        memory.write(self.location, low);
    }

    pub fn pop(&mut self, memory: &Memory) -> u16 {
        let low = memory.read(self.location);
        self.location = self.location.wrapping_add(1);
        let high = memory.read(self.location);
        self.location = self.location.wrapping_add(1);
        combine_to_double_byte(high, low)
    }
}

pub struct AddressBus {
    pub value: u16 // TODO: simple impl for now.
}

// TODO: This struct might actually represent both the address and the data bus, in which case the above struct can go away.
pub struct DataBus {}
impl DataBus {
    pub fn write(&self, port: u16, value: u8) {
        // stub for now
    }

    pub fn read(&self, port: u16) -> u8 {
        // Idle-high default: unimplemented devices read as inactive. The
        // keyboard matrix is active-low, so 0xFF means "no keys pressed";
        // a lower value would look like stuck keys to the firmware.
        0xFF
    }
}

pub struct Registers {
    pub a: DefaultRegister,
    pub f: FlagsRegister,
    pub b: DefaultRegister,
    pub c: DefaultRegister,
    pub d: DefaultRegister,
    pub e: DefaultRegister,
    pub h: DefaultRegister,
    pub l: DefaultRegister,
    pub a_: DefaultRegister,
    pub f_: FlagsRegister,
    pub b_: DefaultRegister,
    pub c_: DefaultRegister,
    pub d_: DefaultRegister,
    pub e_: DefaultRegister,
    pub h_: DefaultRegister,
    pub l_: DefaultRegister,
    pub ixh: DefaultRegister,
    pub ixl: DefaultRegister,
    pub iyh: DefaultRegister,
    pub iyl: DefaultRegister,
    pub i: DefaultRegister,
    pub r: DefaultRegister,

    pub pc: ProgramCounter,
    pub sp: StackPointer,
    pub iff1: bool,
    pub iff2: bool,
    pub interrupt_mode: u8,
    pub halted: bool,
    pub ei_delay: bool
}

impl Registers {
    pub fn get8(&self, r: R8) -> u8 {
        match r {
            R8::A => self.a.get(),
            R8::F => self.f.get(),
            R8::B => self.b.get(),
            R8::C => self.c.get(),
            R8::D => self.d.get(),
            R8::E => self.e.get(),
            R8::H => self.h.get(),
            R8::L => self.l.get(),
            R8::IXH => self.ixh.get(),
            R8::IXL => self.ixl.get(),
            R8::IYH => self.iyh.get(),
            R8::IYL => self.iyl.get(),
            R8::I => self.i.get(),
            R8::R => self.r.get(),
        }
    }

    pub fn set8(&mut self, r: R8, value: u8) {
        match r {
            R8::A => self.a.set(value),
            R8::F => self.f.set(value),
            R8::B => self.b.set(value),
            R8::C => self.c.set(value),
            R8::D => self.d.set(value),
            R8::E => self.e.set(value),
            R8::H => self.h.set(value),
            R8::L => self.l.set(value),
            R8::IXH => self.ixh.set(value),
            R8::IXL => self.ixl.set(value),
            R8::IYH => self.iyh.set(value),
            R8::IYL => self.iyl.set(value),
            R8::I => self.i.set(value),
            R8::R => self.r.set(value),
        }
    }

    pub fn get16(&self, r: R16) -> u16 {
        match r {
            R16::AF => combine_to_double_byte(self.a.get(), self.f.get()),
            R16::BC => combine_to_double_byte(self.b.get(), self.c.get()),
            R16::DE => combine_to_double_byte(self.d.get(), self.e.get()),
            R16::HL => combine_to_double_byte(self.h.get(), self.l.get()),
            R16::IX => combine_to_double_byte(self.ixh.get(), self.ixl.get()),
            R16::IY => combine_to_double_byte(self.iyh.get(), self.iyl.get()),
            R16::SP => self.sp.get(),
            R16::PC => self.pc.get(),
        }
    }

    pub fn set16(&mut self, r: R16, value: u16) {
        let (high, low) = split_double_byte(value);
        match r {
            R16::AF => { self.a.set(high); self.f.set(low); },
            R16::BC => { self.b.set(high); self.c.set(low); },
            R16::DE => { self.d.set(high); self.e.set(low); },
            R16::HL => { self.h.set(high); self.l.set(low); },
            R16::IX => { self.ixh.set(high); self.ixl.set(low); },
            R16::IY => { self.iyh.set(high); self.iyl.set(low); },
            R16::SP => self.sp.set(value),
            R16::PC => self.pc.set(value),
        }
    }

    pub fn default() -> Registers {
        Registers {
            a: DefaultRegister::new("a"),
            f: FlagsRegister::new(),
            b: DefaultRegister::new("b"),
            c: DefaultRegister::new("c"),
            d: DefaultRegister::new("d"),
            e: DefaultRegister::new("e"),
            h: DefaultRegister::new("h"),
            l: DefaultRegister::new("l"),
            a_: DefaultRegister::new("a'"),
            f_: FlagsRegister::new(),
            b_: DefaultRegister::new("b'"),
            c_: DefaultRegister::new("c'"),
            d_: DefaultRegister::new("d'"),
            e_: DefaultRegister::new("e'"),
            h_: DefaultRegister::new("h'"),
            l_: DefaultRegister::new("l'"),
            ixh: DefaultRegister::new("ixh"),
            ixl: DefaultRegister::new("ixl"),
            iyh: DefaultRegister::new("iyh"),
            iyl: DefaultRegister::new("iyl"),
            i: DefaultRegister::new("i"),
            r: DefaultRegister::new("r"),
            pc: ProgramCounter::new(), // PC normally begins at start of memory
            sp: StackPointer { location: 0xFFFF }, // SP normally begins at the end of memory and moves down.
            iff1: false,
            iff2: false,
            interrupt_mode: 0,
            halted: false,
            ei_delay: false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Memory, StackPointer, Registers, R16};

    #[test]
    fn test_stack_pointer() {
        let mut sp = StackPointer { location: 0x100 };
        let mut mem = Memory::default();

        sp.push(&mut mem, 0xABEF);
        assert!(sp.location == 0x100 - 0x2);

        sp.push(&mut mem, 0xCD89);
        assert!(sp.location == 0x100 - 0x4);

        let val = sp.pop(&mut mem);
        assert!(val == 0xCD89);
        assert!(sp.location == 0x100 - 0x2);

        let val = sp.pop(&mut mem);
        assert!(val == 0xABEF);
        assert!(sp.location == 0x100);
    }

    #[test]
    fn test_read16_write16() {
        let mut mem = Memory::default();
        mem.write16(0x8000, 0x1234);
        assert!(mem.read(0x8000) == 0x34);
        assert!(mem.read(0x8001) == 0x12);
        assert!(mem.read16(0x8000) == 0x1234);
    }

    #[test]
    fn test_memory_wraps_at_ffff() {
        let mut mem = Memory::default();
        mem.write(0xFFFF, 0x42);
        assert!(mem.read(0xFFFF) == 0x42);
        mem.write16(0xFFFF, 0xABCD);
        assert!(mem.read(0xFFFF) == 0xCD);
        assert!(mem.read(0x0000) == 0xAB);
    }

    #[test]
    fn test_get_set16() {
        let mut regs = Registers::default();
        regs.set16(R16::IX, 0xBEEF);
        assert!(regs.get16(R16::IX) == 0xBEEF);
        regs.set16(R16::AF, 0x0142);
        assert!(regs.a.get() == 0x01);
        assert!(regs.f.get() == 0x42);
        assert!(regs.get16(R16::AF) == 0x0142);
    }

    #[test]
    fn test_ram_configuration_mapping() {
        let mut mem = Memory::default();
        // Config 0: write distinct markers into all four regions of bank 0
        mem.write(0x0000, 0xA1);
        mem.write(0x4000, 0xA2);
        mem.write(0x8000, 0xA3);
        mem.write(0xC000, 0xA4);

        // Config 2 maps all four regions to blocks 4-7 (expansion bank)
        mem.set_ram_config(2);
        assert_eq!(mem.read(0x0000), 0x01, "expansion bank starts at default fill");
        mem.write(0x4000, 0xB2);
        mem.write(0xC000, 0xB4);

        // Back to config 0: original data intact, expansion writes invisible
        mem.set_ram_config(0);
        assert_eq!(mem.read(0x0000), 0xA1);
        assert_eq!(mem.read(0x4000), 0xA2);
        assert_eq!(mem.read(0x8000), 0xA3);
        assert_eq!(mem.read(0xC000), 0xA4);

        // Config 1: region 3 -> block 7 (the shadow-screen mapping)
        mem.set_ram_config(1);
        assert_eq!(mem.read(0x0000), 0xA1, "regions 0-2 unchanged in config 1");
        assert_eq!(mem.read(0x4000), 0xA2);
        assert_eq!(mem.read(0x8000), 0xA3);
        assert_eq!(mem.read(0xC000), 0xB4, "region 3 maps to block 7 (written under config 2)");
    }

    #[test]
    fn test_rom_banking() {
        let mut mem = Memory::default();
        let os_rom = [0xAA; 0x4000];
        let expansion_rom = [0xBB; 0x4000];
        mem.load_os_rom(&os_rom);
        mem.load_expansion_rom(&expansion_rom);

        // After load_os_rom the lower ROM is enabled: reads hit ROM even over written RAM
        mem.write(0x0000, 0x42);
        assert!(mem.read(0x0000) == 0xAA, "lower ROM should shadow RAM");
        // Upper ROM present: reads at 0xC000+ hit the expansion ROM
        mem.write(0xC000, 0x42);
        assert!(mem.read(0xC000) == 0xBB, "upper ROM should shadow RAM");

        // Writes always go to RAM; disabling banking reveals it
        mem.lower_rom_enabled = false;
        assert!(mem.read(0x0000) == 0x42, "RAM write should survive under ROM");
        mem.upper_rom_enabled = false;
        assert!(mem.read(0xC000) == 0x42, "RAM write should survive under ROM");

        // With no expansion ROM loaded, upper reads always hit RAM
        let mut mem16 = Memory::default();
        mem16.load_os_rom(&os_rom);
        mem16.write(0xC000, 0x77);
        assert!(mem16.read(0xC000) == 0x77);
    }

    #[test]
    fn test_read_video_ram_bypasses_rom_and_paging() {
        let mut mem = Memory::default();
        let os_rom = [0xAA; 0x4000];
        let expansion_rom = [0xBB; 0x4000];
        mem.load_os_rom(&os_rom);
        mem.load_expansion_rom(&expansion_rom);
        mem.write(0xC000, 0x42);

        // The gate array's video fetch never sees the ROM overlay...
        mem.upper_rom_enabled = true;
        assert_eq!(mem.read(0xC000), 0xBB, "CPU read is ROM-overlaid");
        assert_eq!(mem.read_video_ram(0xC000), 0x42, "video fetch reads the RAM underneath");

        // ...nor the RAM configuration: VRAM always lives in the base 64K.
        mem.set_ram_config(0xC2);
        mem.upper_rom_enabled = false; // drop the ROM overlay to see the paging alone
        assert_eq!(mem.read(0xC000), 0x01, "config 2 exposes the expansion bank");
        assert_eq!(mem.read_video_ram(0xC000), 0x42, "video fetch still reads base-bank RAM");
        // A video page below 0x4000 likewise ignores the lower ROM overlay.
        mem.set_ram_config(0xC0);
        mem.lower_rom_enabled = true;
        mem.write(0x0100, 0x77);
        assert_eq!(mem.read(0x0100), 0xAA, "CPU read is lower-ROM-overlaid");
        assert_eq!(mem.read_video_ram(0x0100), 0x77);
    }
}
