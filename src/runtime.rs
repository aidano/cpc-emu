use std::ops::Add;
use std::thread::{self};
use std::time::{self, SystemTime};

///////////////////////
//
// Runtime components - memory, registers, instruction set
//
/////////////////////
use crate::memory::{Memory, Registers, AddressBus, DataBus};
use crate::instruction_set::{self, Op};
use crate::video::GateArray;
use crate::dsk::Dsk;
use crate::devices::{crtc::Crtc, fdc::Fdc, keyboard::Keyboard, ppi::Ppi, psg::Psg};

use log::{debug, error};

/// The gate array raises its interrupt every 52 scanlines; one scanline is
/// 64 characters of 4 T-cycles (256 T-cycles) on the CPC.
pub const LINE_T: u32 = 256;
pub const IRQ_PERIOD: u32 = 52 * LINE_T;

pub struct RuntimeComponents {
    pub mem: Memory,
    pub registers: Registers,
    pub address_bus: AddressBus,
    pub data_bus: DataBus,
    pub gate_array: GateArray,
    pub ppi: Ppi,
    pub psg: Psg,
    pub crtc: Crtc,
    pub keyboard: Keyboard,
    /// µPD765 floppy disc controller (the 6128's internal interface).
    pub fdc: Fdc,
    /// T-cycles since the gate array's 52-line interrupt counter last
    /// restarted. Also drives the block-op interrupt-yield projection.
    pub cycles_since_irq: u32,
}

impl RuntimeComponents {
    pub fn default() -> RuntimeComponents {
        let mem = Memory::default();
        let registers: Registers = Registers::default();
        let address_bus = AddressBus { value: 0 };
        let data_bus = DataBus { };
        let gate_array = GateArray::default();
        let ppi = Ppi::default();
        let psg = Psg::default();
        let crtc = Crtc::default();
        let keyboard = Keyboard::default();
        let fdc = Fdc::default();
        RuntimeComponents { mem, registers, address_bus, data_bus, gate_array, ppi, psg, crtc, keyboard, fdc, cycles_since_irq: 0 }
    }

    /// Routes an OUT to the right device. Decode follows the hardware (and
    /// Caprice32): chips select on high-byte bit ranges; the RAM configuration
    /// is identified by the written value and can overlap other decodes.
    pub fn port_out(&mut self, port: u16, value: u8) {
        let high = (port >> 8) as u8;

        // Gate array: selected when address bits 15-14 are 01 (0x4000-0x7FFF).
        if high & 0xC0 == 0x40 {
            self.gate_array.write(value);
            self.mem.lower_rom_enabled = self.gate_array.lower_rom_enabled;
            self.mem.upper_rom_enabled = self.gate_array.upper_rom_enabled;
            if self.gate_array.pending_interrupt_resync {
                self.gate_array.pending_interrupt_resync = false;
                // The RMR "delay Z80 interrupt" bit clears the gate array's
                // interrupt counter (and any pending INT) so the tick
                // cadence realigns with the firmware.
                self.cycles_since_irq = 0;
                self.gate_array.irq_pending = false;
            }
        }        // RAM configuration: ports 0x0000-0x7FFF with data bits 7-6 = 11.
        if high & 0x80 == 0 && value & 0xC0 == 0xC0 {
            debug!("RAM config -> {:02X}", value);
            self.mem.set_ram_config(value);
        }
        // CRTC: register select (0xBCxx) and data write (0xBDxx).
        if high & 0x40 == 0 {
            match high & 3 {
                0 => self.crtc.reg_select = value,
                1 => self.crtc.write_register(value),
                _ => {}
            }
        }
        // Upper ROM select (0xC0xx-0xDFxx): stored; only BASIC is fitted so
        // every selection maps to it.
        if high & 0x20 == 0 {
            self.mem.set_upper_rom_number(value);
        }
        // Floppy disc interface (selected when address bit 10 is reset):
        // &FA7E = motor flip-flop (write bit 0), &FB7F = FDC data register.
        if high == 0xFA {
            self.fdc.set_motor(value & 1 == 1);
        } else if high == 0xFB {
            if port & 1 == 1 {
                self.fdc.write_data(value);
            }
        }
        // Printer port (0xE0xx-0xFFxx with bit 4 clear): data ignored.
        // PPI (0xF0xx-0xF7xx).
        if high & 0x08 == 0 {
            self.ppi.write(high & 3, value, &mut self.psg);
        }
    }

    /// Routes an IN from the right device.
    pub fn port_in(&mut self, port: u16) -> u8 {
        let high = (port >> 8) as u8;

        // CRTC status port (0xBFxx): registers 12-17 readable.
        if high & 0x40 == 0 && high & 3 == 3 {
            return self.crtc.read_register();
        }
        // Floppy disc interface: &FA7E motor latch is write-only;
        // &FB7E = main status register, &FB7F = data register.
        if high == 0xFA {
            return 0xFF;
        }
        if high == 0xFB {
            return match port & 1 {
                0 => self.fdc.read_status(),
                _ => self.fdc.read_data(),
            };
        }
        // PPI (0xF0xx-0xF7xx).
        if high & 0x08 == 0 {
            let vsync = self.crtc.vsync_active;
            let row = self.keyboard.row(self.ppi.keyboard_line & 0x0F);
            return self.ppi.read(high & 3, Some(row), vsync, &self.psg);
        }
        0xFF
    }
}

pub struct Runtime {
    pub components: RuntimeComponents,
}

impl Runtime {

    pub fn default() -> Runtime {
        Runtime { components: RuntimeComponents::default() }
    }

    pub fn load_rom_from_bytes(&mut self, bytes: &[u8]) {
        match bytes.len() {
            0x4000 => {
                let mut rom = [0u8; 0x4000];
                rom.copy_from_slice(bytes);
                self.components.mem.load_os_rom(&rom);
            },
            0x8000 => {
                let (os, expansion) = bytes.split_at(0x4000);
                let mut os_rom = [0u8; 0x4000];
                os_rom.copy_from_slice(os);
                let mut expansion_rom = [0u8; 0x4000];
                expansion_rom.copy_from_slice(expansion);
                self.components.mem.load_os_rom(&os_rom);
                self.components.mem.load_expansion_rom(&expansion_rom);
            },
            _ => {
                error!("Unexpected ROM size: {}", bytes.len());
                assert!(false);
            }
        }
    }

    /// Fits an expansion ROM into the given upper ROM slot (e.g. AMSDOS
    /// into slot 7, the 6128's internal disc interface convention).
    pub fn load_expansion_rom_into_slot(&mut self, bytes: &[u8; 0x4000], slot: u8) {
        self.components.mem.load_expansion_rom_into_slot(bytes, slot);
    }

    /// Inserts (or removes, with None) the disc image in drive 0.
    pub fn insert_disk(&mut self, disk: Option<Dsk>) {
        self.components.fdc.set_disk(disk);
    }

    /// Fetches, decodes and executes a single instruction.
    /// Returns the number of T-cycles consumed.
    pub fn step(&mut self) -> u32 {
        self.components.mem.diagnostics_step += 1;
        let pc_before = self.components.registers.pc.get();

        let op = instruction_set::decode::decode(&self.components.mem, &mut self.components.registers.pc);
        let pc_after = self.components.registers.pc.get();

        let machine_code: String = (pc_before..pc_after)
            .map(|addr| format!("{:02X}", self.components.mem.read(addr)))
            .collect::<Vec<String>>()
            .join(" ");

        let assembly = instruction_set::assembly(&op);
        let cycles = instruction_set::ops::execute(&op, &mut self.components);

        // EI takes effect after the instruction following it: the delay flag
        // survives this boundary only when EI itself just executed.
        if !matches!(op, Op::Ei) {
            self.components.registers.ei_delay = false;
        }

        // The R register is incremented on every opcode fetch (M1 cycle).
        let r = self.components.registers.r.get();
        self.components.registers.r.set((r & 0x80) | ((r.wrapping_add(1)) & 0x7F));

        let hl = self.components.registers.get16(crate::memory::R16::HL);
        let bc = self.components.registers.get16(crate::memory::R16::BC);
        let de = self.components.registers.get16(crate::memory::R16::DE);
        let iy = self.components.registers.get16(crate::memory::R16::IY);
        let af = self.components.registers.get16(crate::memory::R16::AF);
        debug!("{:0>8X} {:0>4X}\t{: <14}\t{: <20}\t({} cycles)\tAF:{:04X} HL:{:04X} DE:{:04X} BC:{:04X} IY:{:04X} SP:{:04X}", self.components.mem.diagnostics_step, pc_before, machine_code, assembly, cycles, af, hl, de, bc, iy, self.components.registers.sp.get());
        cycles
    }

    /// Runs for approximately `budget` T-cycles, servicing the gate array's
    /// 52-scanline interrupt as boundaries are crossed. Returns the cycles
    /// actually consumed.
    pub fn run_for_cycles(&mut self, budget: u32) -> u32 {
        let mut consumed: u32 = 0;
        while consumed < budget {
            if self.components.registers.halted {
                // A halted CPU only wakes via interrupts. Burn in line-sized
                // chunks so the CRTC/gate-array counters keep pace; if
                // interrupts are off, the halt is permanent for this budget
                // (matches the real lock-up).
                let burn = (budget - consumed).min(self.components.crtc.t_to_line_end());
                consumed += burn;
                self.advance_time(burn);
                continue;
            }

            let cycles = self.step();
            consumed += cycles;
            self.advance_time(cycles);
        }
        consumed
    }

    /// Advances the CRTC and gate array timing by `cycles` T-cycles and
    /// services a held interrupt if one is pending.
    ///
    /// Interrupt semantics follow the gate array: a 6-bit counter increments
    /// on every HSYNC falling edge (one per scanline); at 52 it asserts INT
    /// and restarts. INT stays asserted until acknowledged, so crossings
    /// under DI latch at most one pending interrupt. Two HSYNCs after a
    /// VSYNC the counter is reset, issuing an interrupt only when the
    /// counter is below 32 (the frame-fly alignment the firmware relies on).
    /// On acknowledgement bit 5 of the counter clears, so the next interrupt
    /// cannot occur fewer than 32 scanlines later.
    fn advance_time(&mut self, cycles: u32) {
        self.components.cycles_since_irq += cycles;
        let lines = self.components.crtc.advance(cycles);
        if self.components.crtc.take_vsync_start() {
            self.components.gate_array.vsync_wait = 2;
        }
        // The 52-line counter first: a hit asserts INT and restarts the
        // counter (so a coinciding post-VSYNC check sees the wrapped value).
        while self.components.cycles_since_irq >= IRQ_PERIOD {
            self.components.cycles_since_irq -= IRQ_PERIOD;
            self.components.gate_array.irq_pending = true;
        }
        for _ in 0..lines {
            if self.components.gate_array.vsync_wait > 0 {
                self.components.gate_array.vsync_wait -= 1;
                if self.components.gate_array.vsync_wait == 0 {
                    let below_32 = self.components.cycles_since_irq < 32 * LINE_T;
                    self.components.cycles_since_irq = 0;
                    if below_32 {
                        self.components.gate_array.irq_pending = true;
                    }
                }
            }
        }
        if self.components.gate_array.irq_pending {
            self.service_interrupt();
        }
    }

    /// Services a maskable interrupt if the gate array INT line is asserted,
    /// IFF1 is set and the EI delay has elapsed.
    fn service_interrupt(&mut self) {
        let registers = &mut self.components.registers;
        if !self.components.gate_array.irq_pending
            || !registers.iff1
            || registers.ei_delay
        {
            return;
        }
        self.components.gate_array.irq_pending = false;
        // Acknowledging the interrupt clears bit 5 of the gate array's
        // counter: the next interrupt cannot occur fewer than 32 scanlines
        // later. In T-cycles that is a modulo by 32 * LINE_T.
        self.components.cycles_since_irq %= 32 * LINE_T;
        let return_address = registers.pc.get();
        registers.sp.push(&mut self.components.mem, return_address);
        match registers.interrupt_mode {
            0 => {
                // Data bus acknowledges with 0xFF on a CPC, which decodes as RST 38h.
                registers.pc.set(0x0038);
            }
            2 => {
                let vector = ((registers.i.get() as u16) << 8) | 0xFF;
                let target = self.components.mem.read16(vector);
                registers.pc.set(target);
            }
            _ => {
                registers.pc.set(0x0038);
            }
        }
        registers.iff1 = false;
        registers.iff2 = false;
        registers.halted = false;
        registers.ei_delay = false;
    }

    /// Headless run with real-time pacing. Display hosts use run_for_cycles instead.
    pub fn run(&mut self, start_address: u16) {
        self.components.registers.pc.set(start_address);
        loop {
            if self.components.registers.halted && !self.components.registers.iff1 {
                debug!("HALT with interrupts disabled at #{:04X}; stopping", self.components.registers.pc.get());
                return;
            }
            let cycles = self.run_for_cycles(IRQ_PERIOD);

            let start_time = SystemTime::now();
            let target_elapsed = cycles as u128 * 250u128; // 1 cycle is 250 nanoseconds on a 4Mhz chip.
            let mut elapsed = start_time.elapsed().unwrap().as_nanos();
            while elapsed < target_elapsed {
                thread::sleep(time::Duration::from_nanos(1));
                elapsed = start_time.elapsed().unwrap().as_nanos();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Runtime, RuntimeComponents, IRQ_PERIOD};
    use crate::instruction_set::{decode, ops, assembly, Op};
    use crate::memory::{AddressBus, DataBus, Memory, Registers};

    pub fn runtime() -> Runtime {
        Runtime::default()
    }

    /// Writes the given bytes at 0x0000 and steps the runtime, returning cycles.
    pub fn step_bytes(runtime: &mut Runtime, bytes: &[u8]) -> u32 {
        for (i, b) in bytes.iter().enumerate() {
            runtime.components.mem.write(i as u16, *b);
        }
        runtime.components.registers.pc.set(0);
        runtime.step()
    }

    #[test]
    fn test_call_ret_round_trip() {
        let mut rt = runtime();

        // CALL 0x0100: pushes the address of the *next* instruction (0x0003)
        let cycles = step_bytes(&mut rt, &[0xCD, 0x00, 0x01]);
        assert_eq!(cycles, 17);
        assert_eq!(rt.components.registers.pc.get(), 0x0100);
        assert_eq!(rt.components.registers.sp.get(), 0xFFFD);
        assert_eq!(rt.components.mem.read(0xFFFE), 0x00); // return address, high byte
        assert_eq!(rt.components.mem.read(0xFFFD), 0x03); // return address, low byte = next instruction

        // RET at the target resumes at the instruction after the CALL
        rt.components.mem.write(0x0100, 0xC9);
        let cycles = rt.step();
        assert_eq!(cycles, 10);
        assert_eq!(rt.components.registers.pc.get(), 0x0003);
        assert_eq!(rt.components.registers.sp.get(), 0xFFFF);
    }

    #[test]
    fn test_jr_cond_taken_and_not_taken() {
        let mut rt = runtime();

        // Taken: JR NZ,+4 with Z unset. PC after fetch is 2, target = 2 + 4 = 6.
        rt.components.registers.f.set_zero(false);
        let cycles = step_bytes(&mut rt, &[0x20, 0x04]);
        assert_eq!(cycles, 12);
        assert_eq!(rt.components.registers.pc.get(), 6);

        // Not taken: JR NZ,+4 with Z set. PC still advances past the instruction.
        rt.components.registers.f.set_zero(true);
        let cycles = step_bytes(&mut rt, &[0x20, 0x04]);
        assert_eq!(cycles, 7);
        assert_eq!(rt.components.registers.pc.get(), 2);
    }

    #[test]
    fn test_jp_hl_via_pointer() {
        let mut rt = runtime();
        rt.components.registers.h.set(0x12);
        rt.components.registers.l.set(0x34);
        let cycles = step_bytes(&mut rt, &[0xE9]);
        assert_eq!(cycles, 4);
        assert_eq!(rt.components.registers.pc.get(), 0x1234);
    }

    #[test]
    fn test_step_ld_bc_nn() {
        let mut rt = runtime();
        let cycles = step_bytes(&mut rt, &[0x01, 0x34, 0x12]);
        assert_eq!(cycles, 10);
        assert_eq!(rt.components.registers.b.get(), 0x12);
        assert_eq!(rt.components.registers.c.get(), 0x34);
        assert_eq!(rt.components.registers.pc.get(), 3);
    }

    #[test]
    fn test_step_jr_negative() {
        let mut rt = runtime();
        // JR -2 at 0x0000: PC after fetch is 2; 2 - 2 = 0
        let cycles = step_bytes(&mut rt, &[0x18, 0xFE]);
        assert_eq!(cycles, 12);
        assert_eq!(rt.components.registers.pc.get(), 0);
    }

    #[test]
    fn test_step_xor_a() {
        let mut rt = runtime();
        rt.components.registers.a.set(0xAA);
        step_bytes(&mut rt, &[0xAF]);
        assert_eq!(rt.components.registers.a.get(), 0);
        assert!(rt.components.registers.f.zero());
        assert!(!rt.components.registers.f.carry());
    }

    #[test]
    fn test_step_ld_addr_hl_writes_both_bytes() {
        let mut rt = runtime();
        rt.components.registers.h.set(0x40);
        rt.components.registers.l.set(0x00);
        rt.components.registers.b.set(0x12);
        rt.components.registers.c.set(0x34);
        // LD (0x8000),HL then LD (0x8002),BC
        step_bytes(&mut rt, &[0x22, 0x00, 0x80]);
        step_bytes(&mut rt, &[0xED, 0x43, 0x02, 0x80]);
        assert_eq!(rt.components.mem.read(0x8000), 0x00);
        assert_eq!(rt.components.mem.read(0x8001), 0x40);
        assert_eq!(rt.components.mem.read(0x8002), 0x34);
        assert_eq!(rt.components.mem.read(0x8003), 0x12);
    }

    #[test]
    fn test_step_push_pop_ix() {
        let mut rt = runtime();
        rt.components.registers.ixh.set(0xAB);
        rt.components.registers.ixl.set(0xCD);
        step_bytes(&mut rt, &[0xDD, 0xE5]); // PUSH IX
        assert_eq!(rt.components.registers.sp.get(), 0xFFFD);
        rt.components.registers.ixh.set(0);
        rt.components.registers.ixl.set(0);
        step_bytes(&mut rt, &[0xDD, 0xE1]); // POP IX
        assert_eq!(rt.components.registers.get16(crate::memory::R16::IX), 0xABCD);
    }

    #[test]
    fn test_step_ld_ix_plus_d() {
        let mut rt = runtime();
        rt.components.registers.ixh.set(0x80);
        rt.components.registers.ixl.set(0x00);
        // DD 46 05 = LD B,(IX+5)
        let cycles = step_bytes(&mut rt, &[0xDD, 0x46, 0x05]);
        assert_eq!(cycles, 19);
        assert_eq!(rt.components.registers.b.get(), rt.components.mem.read(0x8005));
    }

    #[test]
    fn test_step_rlca() {
        let mut rt = runtime();
        rt.components.registers.a.set(1);
        step_bytes(&mut rt, &[0x07]);
        assert_eq!(rt.components.registers.a.get(), 2);
        rt.components.registers.a.set(255);
        step_bytes(&mut rt, &[0x07]);
        assert_eq!(rt.components.registers.a.get(), 255);
        assert!(rt.components.registers.f.carry());
    }

    #[test]
    fn test_step_djnz() {
        let mut rt = runtime();
        rt.components.registers.b.set(2);
        // DJNZ 0x04: B=2 -> 1, jump taken, target = 2 + 4 = 6
        let cycles = step_bytes(&mut rt, &[0x10, 0x04]);
        assert_eq!(cycles, 13);
        assert_eq!(rt.components.registers.pc.get(), 6);
        assert_eq!(rt.components.registers.b.get(), 1);
    }

    #[test]
    fn test_step_sbc_a_n() {
        let mut rt = runtime();
        rt.components.registers.a.set(0x11);
        rt.components.registers.f.set(0x01); // carry set
        let cycles = step_bytes(&mut rt, &[0xDE, 0x01]);
        assert_eq!(cycles, 7);
        assert_eq!(rt.components.registers.a.get(), 0x0F);
    }

    #[test]
    fn test_step_sub_n_cycles() {
        let mut rt = runtime();
        rt.components.registers.a.set(5);
        let cycles = step_bytes(&mut rt, &[0xD6, 0x03]);
        assert_eq!(cycles, 7);
        assert_eq!(rt.components.registers.a.get(), 2);
    }

    #[test]
    fn test_step_ldir_copies_block() {
        let mut rt = runtime();
        rt.components.registers.h.set(0x10);
        rt.components.registers.l.set(0x00);
        rt.components.registers.d.set(0x20);
        rt.components.registers.e.set(0x00);
        rt.components.registers.b.set(0x00);
        rt.components.registers.c.set(0x03);
        for i in 0..3 {
            rt.components.mem.write(0x1000 + i, 0xA0 + i as u8);
        }
        let cycles = step_bytes(&mut rt, &[0xED, 0xB0]);
        assert_eq!(cycles, 16 + 21 * 2);
        assert_eq!(rt.components.mem.read(0x2000), 0xA0);
        assert_eq!(rt.components.mem.read(0x2001), 0xA1);
        assert_eq!(rt.components.mem.read(0x2002), 0xA2);
        assert_eq!(rt.components.registers.get16(crate::memory::R16::BC), 0);
        assert_eq!(rt.components.registers.get16(crate::memory::R16::HL), 0x1003);
        assert_eq!(rt.components.registers.get16(crate::memory::R16::DE), 0x2003);
    }

    #[test]
    fn test_step_rst() {
        let mut rt = runtime();
        let cycles = step_bytes(&mut rt, &[0xFF]); // RST 38h
        assert_eq!(cycles, 11);
        assert_eq!(rt.components.registers.pc.get(), 0x38);
        assert_eq!(rt.components.registers.sp.get(), 0xFFFD);
        assert_eq!(rt.components.mem.read(0xFFFE), 0x00); // pushed PC (high byte)
        assert_eq!(rt.components.mem.read(0xFFFD), 0x01); // pushed PC (low byte)
    }

    #[test]
    fn test_step_halt_sets_flag() {
        let mut rt = runtime();
        step_bytes(&mut rt, &[0x76]);
        assert!(rt.components.registers.halted);
    }

    #[test]
    fn test_interrupt_pushes_return_address_and_returns() {
        let mut rt = runtime();
        // EI at 0x0000, then JR -2 self-loop at 0x0001
        rt.components.mem.write(0x0000, 0xFB);
        rt.components.mem.write(0x0001, 0x18);
        rt.components.mem.write(0x0002, 0xFE);
        // IRQ handler at 0x0038: INC A; RET
        rt.components.mem.write(0x0038, 0x3C);
        rt.components.mem.write(0x0039, 0xC9);
        rt.components.registers.pc.set(0);
        rt.step(); // EI (delay active)
        rt.step(); // JR self-loop

        rt.run_for_cycles(IRQ_PERIOD * 3);

        assert!(rt.components.registers.a.get() >= 1, "handler should have run at least once");
        assert_eq!(rt.components.registers.sp.get(), 0xFFFF, "handler RET must restore SP");
    }

    #[test]
    fn test_interrupt_wakes_halt() {
        let mut rt = runtime();
        // EI; HALT
        rt.components.mem.write(0x0000, 0xFB);
        rt.components.mem.write(0x0001, 0x76);
        // Handler: INC A; JR -3 (loop back to 0x38)
        rt.components.mem.write(0x0038, 0x3C);
        rt.components.mem.write(0x0039, 0x18);
        rt.components.mem.write(0x003A, 0xFD);
        rt.components.registers.pc.set(0);
        rt.step(); // EI
        rt.step(); // HALT
        assert!(rt.components.registers.halted);

        rt.run_for_cycles(IRQ_PERIOD * 2);
        assert!(!rt.components.registers.halted, "interrupt should wake the CPU from HALT");
        assert!(rt.components.registers.a.get() >= 1);
    }

    #[test]
    fn test_ei_di_blocks_interrupt() {
        let mut rt = runtime();
        // EI; DI; JR -2 self-loop
        rt.components.mem.write(0x0000, 0xFB);
        rt.components.mem.write(0x0001, 0xF3);
        rt.components.mem.write(0x0002, 0x18);
        rt.components.mem.write(0x0003, 0xFE);
        rt.components.mem.write(0x0038, 0x3C); // handler: INC A
        rt.components.registers.pc.set(0);
        rt.step(); // EI
        rt.step(); // DI
        rt.step(); // JR

        rt.run_for_cycles(IRQ_PERIOD * 3);

        assert_eq!(rt.components.registers.pc.get(), 2, "no interrupt may fire after EI;DI");
        assert_eq!(rt.components.registers.a.get(), 0);
    }

    #[test]
    fn test_di_blocks_interrupt() {
        let mut rt = runtime();
        // DI; JR -2 self-loop
        rt.components.mem.write(0x0000, 0xF3);
        rt.components.mem.write(0x0001, 0x18);
        rt.components.mem.write(0x0002, 0xFE);
        rt.components.mem.write(0x0038, 0x3C);
        rt.components.registers.pc.set(0);
        rt.step(); // DI
        rt.step(); // JR

        rt.run_for_cycles(IRQ_PERIOD * 2);
        assert_eq!(rt.components.registers.a.get(), 0);
    }

    #[test]
    fn test_im2_vectors_through_i_register() {
        let mut rt = runtime();
        rt.components.registers.i.set(0x40);
        // IM2; EI; JR -2 self-loop
        rt.components.mem.write(0x0000, 0xED);
        rt.components.mem.write(0x0001, 0x5E);
        rt.components.mem.write(0x0002, 0xFB);
        rt.components.mem.write(0x0003, 0x18);
        rt.components.mem.write(0x0004, 0xFE); // JR -2 back to 0x0003
        // Vector table at 0x40FF points to handler at 0x1234: INC A; RET
        rt.components.mem.write16(0x40FF, 0x1234);
        rt.components.mem.write(0x1234, 0x3C);
        rt.components.mem.write(0x1235, 0xC9);
        rt.components.registers.pc.set(0);
        rt.step(); // IM2
        rt.step(); // EI
        rt.step(); // JR

        rt.run_for_cycles(IRQ_PERIOD * 2);
        assert!(rt.components.registers.a.get() >= 1, "IM2 should vector to the handler at 0x1234");
        assert_eq!(rt.components.registers.sp.get(), 0xFFFF);
    }

    #[test]
    fn test_gate_array_port_write_changes_mode() {
        let mut rt = runtime();
        // OUT (0x7Fxx) via BC: LD BC,0x7F00; LD A,0x8E (mode 2, ROMs off); OUT (C),A
        step_bytes(&mut rt, &[0x01, 0x00, 0x7F]);
        step_bytes(&mut rt, &[0x3E, 0x8E]);
        step_bytes(&mut rt, &[0xED, 0x79]);
        assert_eq!(rt.components.gate_array.mode, 2);
        assert!(!rt.components.mem.lower_rom_enabled);
        assert!(!rt.components.mem.upper_rom_enabled);
    }

    #[test]
    fn test_fdc_ports_leave_the_ppi_untouched() {
        let mut rt = runtime();
        // The PPI latches as the firmware leaves them after init: control
        // 0x92 (port A input, port C output), a keyboard line selected in
        // port C.
        rt.components.ppi.control = 0x92;
        rt.components.ppi.port_c = 0x44;
        rt.components.ppi.keyboard_line = 0x44;
        // The disc interface ports (0xFAxx/0xFBxx) must decode apart from
        // the PPI (0xF4xx-0xF7xx): motor, data-register and status accesses
        // during AMSDOS's init must not write the PPI or the PSG behind it.
        rt.components.port_out(0xFA7E, 0x01);
        rt.components.port_out(0xFB7F, 0xFF);
        rt.components.port_out(0xFB7F, 0x46);
        let _status = rt.components.port_in(0xFB7E);
        assert_eq!(rt.components.ppi.control, 0x92);
        assert_eq!(rt.components.ppi.port_c, 0x44);
        assert_eq!(rt.components.ppi.keyboard_line, 0x44);
        assert!(rt.components.fdc.motor, "the motor latch write must reach the FDC");
        // The real PPI range still works normally.
        rt.components.port_out(0xF610, 0x40);
        assert_eq!(rt.components.ppi.port_c, 0x40);
        assert_eq!(rt.components.ppi.keyboard_line, 0x40);
    }

    #[test]
    fn test_step_ddcb_srl_ix_d() {
        let mut rt = runtime();
        rt.components.registers.ixh.set(0x80);
        rt.components.registers.ixl.set(0x00);
        rt.components.mem.write(0x8001, 0x05);
        // DD CB 01 38 = SRL (IX+1)
        let cycles = step_bytes(&mut rt, &[0xDD, 0xCB, 0x01, 0x38]);
        assert_eq!(cycles, 23);
        assert_eq!(rt.components.mem.read(0x8001), 0x02);
        assert!(rt.components.registers.f.carry());
    }

    #[test]
    fn test_assembly_output() {
        let op = Op::LdR16N(crate::memory::R16::HL, 0x1234);
        assert_eq!(assembly(&op), "LD HL,1234");
    }

    #[test]
    fn test_add16_flags_preserved() {
        let mut rt = runtime();
        rt.components.registers.f.set(0x40); // Z set - must be preserved by ADD HL
        rt.components.registers.h.set(0x00);
        rt.components.registers.l.set(0xFF);
        rt.components.registers.b.set(0x00);
        rt.components.registers.c.set(0x01);
        step_bytes(&mut rt, &[0x09]); // ADD HL,BC
        assert_eq!(rt.components.registers.get16(crate::memory::R16::HL), 0x0100);
        assert!(rt.components.registers.f.zero()); // preserved
        assert!(!rt.components.registers.f.carry());
    }
}
