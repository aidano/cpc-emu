// AY-3-8912 PSG behind the PPI. The register file and I/O ports feed the
// keyboard matrix; the audio side models the chip the way Caprice32 does:
// a 1MHz clock, tone half-cycles of 8*T clocks (f = clock/(16*T)), a 17-bit
// noise LFSR stepped every 16*N clocks, and one 16-level envelope ramp per
// 256*EP clocks.

pub const PSG_REG_SELECT: u8 = 0xC0;
pub const PSG_REG_WRITE: u8 = 0x80;
pub const PSG_REG_READ: u8 = 0x40;

pub const PSG_PORT_A: usize = 14;
pub const PSG_PORT_B: usize = 15;

/// The gate array divides its 16MHz clock by 16 for the PSG.
pub const PSG_CLOCK_HZ: u32 = 1_000_000;

/// Output sample rate until the audio backend reports the device's rate.
pub const DEFAULT_SAMPLE_RATE: u32 = 44_100;

/// The AY's 4-bit amplitude curve (Hacker KAY's measured table, also used
/// by Caprice32): each step is roughly exponential, not linear.
const VOLUME: [f32; 16] = [
    0.0,
    836.0 / 65535.0,
    1212.0 / 65535.0,
    1773.0 / 65535.0,
    2619.0 / 65535.0,
    3875.0 / 65535.0,
    5397.0 / 65535.0,
    8823.0 / 65535.0,
    10392.0 / 65535.0,
    16706.0 / 65535.0,
    23339.0 / 65535.0,
    29292.0 / 65535.0,
    36969.0 / 65535.0,
    46421.0 / 65535.0,
    55195.0 / 65535.0,
    65535.0 / 65535.0,
];

/// Corner frequency of the output high-pass: the hardware AC-couples the
/// PSG signal before the amplifier, so the DC level must not reach samples.
const OUTPUT_HPF_HZ: f64 = 20.0;

#[derive(Debug, Clone)]
pub struct Psg {
    pub reg_select: u8,
    pub registers: [u8; 16],
    /// The PSG control byte, maintained from PPI port C upper-half writes.
    pub control: u8,
    /// Audio output sample rate (overwritten by the backend with the
    /// device's rate when the stream opens).
    pub sample_rate: u32,

    // Audio generator state.
    /// PSG clocks elapsed in the current tone half-cycle, per channel.
    tone_phase: [u32; 3],
    /// Current tone square-wave level, per channel.
    tone_level: [bool; 3],
    /// The 17-bit noise LFSR, seeded like Caprice32 (period 2^17-1).
    noise_lfsr: u32,
    noise_phase: u32,
    noise_level: bool,
    /// PSG clocks elapsed in the current envelope step.
    env_phase: u32,
    /// Envelope counter level 0-15 (the output level itself).
    env_pos: i8,
    env_dir: i8,
    /// True once the counter has reached a segment end and stays.
    env_holding: bool,
    /// Set after an alternating envelope's first segment end: with the hold
    /// bit the counter stops at the *second* end, like the hardware.
    env_flipped: bool,
    /// Fractional PSG clocks owed per output sample.
    clock_debt: u64,
    /// Output high-pass state (x[n-1], y[n-1]).
    hpf_prev_x: f32,
    hpf_state: f32,
}

impl Psg {
    pub fn default() -> Psg {
        Psg {
            reg_select: 0,
            registers: [0x00; 16],
            control: 0,
            sample_rate: DEFAULT_SAMPLE_RATE,
            tone_phase: [0; 3],
            tone_level: [false; 3],
            noise_lfsr: 0xFFFF,
            noise_phase: 0,
            noise_level: false,
            env_phase: 0,
            env_pos: 15,
            env_dir: -1,
            env_holding: false,
            env_flipped: false,
            clock_debt: 0,
            hpf_prev_x: 0.0,
            hpf_state: 0.0,
        }
    }

    /// Handles a PSG data write (triggered by PPI port A / port C sequences).
    pub fn write_data(&mut self, value: u8) {
        match self.control & 0xC0 {
            PSG_REG_SELECT => self.reg_select = value & 0x0F,
            PSG_REG_WRITE => {
                let reg = self.reg_select as usize;
                if reg < 16 {
                    self.registers[reg] = value;
                    // Writing the envelope shape restarts the generator.
                    if reg == 13 {
                        self.env_reset();
                    }
                }
            }
            _ => {}
        }
    }

    /// Handles a PSG data read (triggered by a PPI port A read while the PSG
    /// control byte is in read mode). `keyboard_row` is the currently selected
    /// keyboard matrix row, if the PPI wants the keyboard path.
    pub fn read_data(&self, keyboard_row: Option<u8>) -> u8 {
        let value = match self.reg_select as usize {
            PSG_PORT_A => match keyboard_row {
                Some(row) => {
                    if self.registers[7] & 0x40 == 0 {
                        row // port A configured as input: read the matrix
                    } else {
                        self.registers[PSG_PORT_A] & row
                    }
                }
                None => self.registers[PSG_PORT_A],
            },
            PSG_PORT_B => self.registers[PSG_PORT_B],
            reg => self.registers[reg],
        };
        value
    }

    /// Renders `out.len()` samples of the mixed PSG output at `sample_rate`,
    /// advancing the chip by the matching number of PSG clocks. Register
    /// writes take effect from the moment they occur; the granularity is the
    /// caller's chunk (one display frame in the hosts).
    pub fn render_samples(&mut self, out: &mut [f32]) {
        if out.is_empty() {
            return;
        }
        let hpf_r = (-2.0 * std::f64::consts::PI * OUTPUT_HPF_HZ / self.sample_rate as f64).exp() as f32;
        let mut prev_x = self.hpf_prev_x;
        let mut hpf = self.hpf_state;
        for slot in out.iter_mut() {
            let mut sum = 0.0f64;
            let mut clocks = 0u64;
            self.clock_debt += PSG_CLOCK_HZ as u64;
            while self.clock_debt >= self.sample_rate as u64 {
                self.clock_debt -= self.sample_rate as u64;
                sum += self.clock_and_mix();
                clocks += 1;
            }
            let x = (sum / (clocks as f64).max(1.0)) as f32 / 3.0;
            hpf = x - prev_x + hpf_r * hpf;
            prev_x = x;
            *slot = hpf;
        }
        self.hpf_prev_x = prev_x;
        self.hpf_state = hpf;
    }

    /// Advances the chip one PSG clock and returns the summed signed channel
    /// levels (in VOLUME units) for the mixer window.
    fn clock_and_mix(&mut self) -> f64 {
        // Tone: the counter toggles the channel output every T ticks of the
        // PSG/8 clock; a period of 0 behaves like 1.
        for channel in 0..3 {
            let half = self.tone_period(channel).max(1) as u32 * 8;
            self.tone_phase[channel] += 1;
            if self.tone_phase[channel] >= half {
                self.tone_phase[channel] = 0;
                self.tone_level[channel] = !self.tone_level[channel];
            }
        }
        // Noise: the 17-bit LFSR shifts every 16*N clocks.
        self.noise_phase += 1;
        if self.noise_phase >= self.noise_period().max(1) as u32 * 16 {
            self.noise_phase = 0;
            let feedback = ((self.noise_lfsr >> 13) ^ (self.noise_lfsr >> 16)) & 1;
            let bit = feedback ^ 1;
            self.noise_lfsr = ((self.noise_lfsr << 1) | bit) & 0x1FFFF;
            self.noise_level = bit != 0;
        }
        // Envelope: one 16-level ramp lasts 256*EP clocks.
        self.env_phase += 1;
        if self.env_phase >= self.envelope_period().max(1) as u32 * 16 {
            self.env_phase = 0;
            self.env_step();
        }
        // Mixer: register 7 bits 0-2 gate tone, 3-5 gate noise (set = off).
        // Caprice32 quirk worth keeping: in envelope mode with a tone period
        // of 4 or less the tone counts as always-high, so envelope-driven
        // sounds are not gated by an ultrasonic square wave.
        let mut sum = 0.0f64;
        for channel in 0..3 {
            let amp_reg = self.registers[8 + channel];
            let envelope_mode = amp_reg & 0x10 != 0;
            let level = VOLUME[if envelope_mode {
                self.env_pos as usize & 0x0F
            } else {
                (amp_reg & 0x0F) as usize
            }] as f64;
            let tone_bit = if self.registers[7] & (1 << channel) == 0 {
                if envelope_mode && self.tone_period(channel) <= 4 {
                    true
                } else {
                    self.tone_level[channel]
                }
            } else {
                true
            };
            let noise_bit = if self.registers[7] & (1 << (3 + channel)) == 0 {
                self.noise_level
            } else {
                true
            };
            sum += if tone_bit && noise_bit { level } else { -level };
        }
        sum
    }

    /// The channel's 12-bit tone period (registers 0/1, 2/3, 4/5; the coarse
    /// register is 4 bits wide on the hardware).
    fn tone_period(&self, channel: usize) -> u16 {
        let reg = channel * 2;
        self.registers[reg] as u16 | ((self.registers[reg + 1] as u16 & 0x0F) << 8)
    }

    fn noise_period(&self) -> u16 {
        (self.registers[6] & 0x1F) as u16
    }

    /// The 16-bit envelope period (registers 11/12).
    fn envelope_period(&self) -> u16 {
        self.registers[11] as u16 | ((self.registers[12] as u16) << 8)
    }

    /// Restarts the envelope generator after a shape (register 13) write:
    /// attack ramps up from 0, decay ramps down from 15.
    fn env_reset(&mut self) {
        let attack = self.registers[13] & 0x04 != 0;
        self.env_dir = if attack { 1 } else { -1 };
        self.env_pos = if attack { 0 } else { 15 };
        self.env_phase = 0;
        self.env_holding = false;
        self.env_flipped = false;
    }

    /// Moves the envelope counter one level along the shape's
    /// continue/attack/alternate/hold bits (register 13, shape 0-15).
    fn env_step(&mut self) {
        if self.env_holding {
            return;
        }
        let shape = self.registers[13] & 0x0F;
        let alternate = shape & 0x02 != 0;
        let hold = shape & 0x01 != 0;
        let continue_ = shape & 0x08 != 0;
        let mut pos = self.env_pos + self.env_dir;
        if pos < 0 || pos > 15 {
            let ended_at_top = self.env_dir > 0;
            if alternate {
                if hold && self.env_flipped {
                    self.env_holding = true;
                    pos = if ended_at_top { 15 } else { 0 };
                } else {
                    self.env_dir = -self.env_dir;
                    self.env_flipped = true;
                    pos = self.env_pos + self.env_dir;
                }
            } else if hold {
                pos = if ended_at_top { 15 } else { 0 };
                self.env_holding = true;
            } else if continue_ {
                // Restart the same ramp: a repeating sawtooth.
                pos = if ended_at_top { 0 } else { 15 };
            } else {
                // Without continue the counter falls to zero and stays.
                pos = 0;
                self.env_holding = true;
            }
        }
        self.env_pos = pos;
    }
}

#[cfg(test)]
mod tests {
    use super::{Psg, PSG_CLOCK_HZ, PSG_REG_READ, PSG_REG_SELECT, PSG_REG_WRITE};

    /// Programs a register through the real protocol: latch the address,
    /// then write the data.
    fn write_reg(psg: &mut Psg, reg: u8, value: u8) {
        psg.control = PSG_REG_SELECT;
        psg.write_data(reg);
        psg.control = PSG_REG_WRITE;
        psg.write_data(value);
    }

    /// Enables tone channel A only, at fixed volume 15.
    fn enable_tone_a(psg: &mut Psg, period: u16) {
        write_reg(psg, 0, (period & 0xFF) as u8);
        write_reg(psg, 1, (period >> 8) as u8);
        write_reg(psg, 7, 0x3E); // tone A on; tone B/C and all noise off
        write_reg(psg, 8, 15);
    }

    #[test]
    fn test_select_then_write() {
        let mut psg = Psg::default();
        psg.control = PSG_REG_SELECT;
        psg.write_data(7); // latch register 7
        psg.control = PSG_REG_WRITE;
        psg.write_data(0x3F); // all ports output
        assert_eq!(psg.registers[7], 0x3F);
        assert_eq!(psg.reg_select, 7);
    }

    #[test]
    fn test_read_keyboard_row() {
        let mut psg = Psg::default();
        // Register 7 bit 6 clear: port A is input, so reads pass the matrix through
        psg.control = PSG_REG_SELECT;
        psg.write_data(14); // select register 14 (keyboard port A)
        psg.control = PSG_REG_READ;
        assert_eq!(psg.read_data(Some(0xFB)), 0xFB);
        // Register 7 bit 6 set: port A is output; stored value is ANDed with the matrix
        psg.control = PSG_REG_SELECT;
        psg.write_data(7);
        psg.control = PSG_REG_WRITE;
        psg.write_data(0x40);
        psg.control = PSG_REG_SELECT;
        psg.write_data(14);
        psg.control = PSG_REG_READ;
        assert_eq!(psg.read_data(Some(0xFB)), 0x00);
    }

    #[test]
    fn test_tone_frequency() {
        let mut psg = Psg::default();
        psg.sample_rate = 44_100;
        // SOUND 1,142 on a CPC is concert A: f = 1MHz / (16 * 142) = 440.1Hz.
        enable_tone_a(&mut psg, 142);
        let mut samples = [0f32; 44_100];
        psg.render_samples(&mut samples);
        // Skip the high-pass settling time, then count rising edges over 0.9s.
        let crossings = samples[4_410..]
            .windows(2)
            .filter(|w| w[0] <= 0.0 && w[1] > 0.0)
            .count();
        assert!(
            (388..=404).contains(&crossings),
            "expected ~396 rising edges (440Hz over 0.9s), got {}",
            crossings
        );
    }

    #[test]
    fn test_silent_when_tone_disabled() {
        let mut psg = Psg::default();
        psg.sample_rate = 44_100;
        enable_tone_a(&mut psg, 142);
        write_reg(&mut psg, 7, 0x3F); // everything off
        let mut samples = [0f32; 22_050];
        psg.render_samples(&mut samples);
        // The always-high gate is DC, so the AC-coupled output is silent
        // after the high-pass settles (the first few ms hold the transient).
        let peak = samples[4_410..].iter().fold(0.0f32, |m, s| m.max(s.abs()));
        assert!(peak < 1e-3, "mixer-disabled channel must be silent, peak {}", peak);
    }

    #[test]
    fn test_silent_at_zero_amplitude() {
        let mut psg = Psg::default();
        psg.sample_rate = 44_100;
        enable_tone_a(&mut psg, 142);
        write_reg(&mut psg, 8, 0);
        let mut samples = [0f32; 22_050];
        psg.render_samples(&mut samples);
        let peak = samples.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        assert!(peak < 1e-3, "zero amplitude must be silent, peak {}", peak);
    }

    #[test]
    fn test_noise_lfsr_matches_caprice32() {
        // Golden sequence computed from the Caprice32/AY Emulator formula:
        // feedback from bits 13 and 16, inverted, shifted into a 17-bit seed.
        let golden: [(u32, bool); 8] = [
            (0x1FFFE, false),
            (0x1FFFD, true),
            (0x1FFFB, true),
            (0x1FFF7, true),
            (0x1FFEF, true),
            (0x1FFDF, true),
            (0x1FFBF, true),
            (0x1FF7F, true),
        ];
        let mut psg = Psg::default();
        write_reg(&mut psg, 6, 1); // step every 16 clocks
        for (seed, level) in golden {
            for _ in 0..16 {
                psg.clock_and_mix();
            }
            assert_eq!(psg.noise_lfsr, seed, "LFSR diverged from the reference");
            assert_eq!(psg.noise_level, level);
        }
        // The taps are maximal: the seed returns after 2^17-1 steps.
        for _ in 0..(131_071 - 8) * 16 {
            psg.clock_and_mix();
        }
        assert_eq!(psg.noise_lfsr, 0xFFFF, "LFSR period must be 2^17-1");
    }

    #[test]
    fn test_tone_toggles_every_half_period() {
        let mut psg = Psg::default();
        write_reg(&mut psg, 0, 3); // period 3 -> half-cycle of 24 clocks
        for _ in 0..24 {
            psg.clock_and_mix();
        }
        assert!(psg.tone_level[0], "tone must flip after 8*T clocks");
        for _ in 0..24 {
            psg.clock_and_mix();
        }
        assert!(!psg.tone_level[0]);
    }

    #[test]
    fn test_envelope_period_register_pair() {
        let mut psg = Psg::default();
        write_reg(&mut psg, 11, 100);
        write_reg(&mut psg, 12, 2);
        assert_eq!(psg.envelope_period(), 0x0264);
    }

    #[test]
    fn test_envelope_step_cadence() {
        let mut psg = Psg::default();
        write_reg(&mut psg, 11, 100); // EP = 100
        write_reg(&mut psg, 13, 0x08); // continuous decay
        // One 16-level ramp per 256*EP clocks: one level per 16*EP clocks.
        for _ in 0..16 * 100 {
            psg.clock_and_mix();
        }
        assert_eq!(psg.env_pos, 14, "exactly one envelope step must elapse");
    }

    #[test]
    fn test_envelope_shape_0_decay_holds_at_zero() {
        let mut psg = Psg::default();
        write_reg(&mut psg, 13, 0x00);
        assert_eq!(psg.env_pos, 15, "decay starts at the top");
        for _ in 0..15 {
            psg.env_step();
        }
        assert_eq!(psg.env_pos, 0);
        for _ in 0..10 {
            psg.env_step();
        }
        assert_eq!(psg.env_pos, 0, "shape 0 holds at zero");
    }

    #[test]
    fn test_envelope_shape_4_attack_drops_to_zero() {
        let mut psg = Psg::default();
        write_reg(&mut psg, 13, 0x04);
        assert_eq!(psg.env_pos, 0, "attack starts at the bottom");
        for _ in 0..15 {
            psg.env_step();
        }
        assert_eq!(psg.env_pos, 15);
        psg.env_step();
        assert_eq!(psg.env_pos, 0, "shape 4 drops to zero after the attack");
        psg.env_step();
        assert_eq!(psg.env_pos, 0);
    }

    #[test]
    fn test_envelope_shape_8_repeats_decay() {
        let mut psg = Psg::default();
        write_reg(&mut psg, 13, 0x08);
        for _ in 0..15 {
            psg.env_step();
        }
        assert_eq!(psg.env_pos, 0);
        psg.env_step();
        assert_eq!(psg.env_pos, 15, "shape 8 restarts the decay from the top");
    }

    #[test]
    fn test_envelope_shape_10_triangle() {
        let mut psg = Psg::default();
        write_reg(&mut psg, 13, 0x0A);
        for _ in 0..15 {
            psg.env_step();
        }
        assert_eq!(psg.env_pos, 0);
        psg.env_step();
        assert_eq!(psg.env_pos, 1, "shape 10 bounces into an attack ramp");
        for _ in 0..14 {
            psg.env_step();
        }
        assert_eq!(psg.env_pos, 15);
        psg.env_step();
        assert_eq!(psg.env_pos, 14, "then decays again: a repeating triangle");
    }

    #[test]
    fn test_envelope_shape_11_holds_at_top_after_cycle() {
        let mut psg = Psg::default();
        write_reg(&mut psg, 13, 0x0B);
        for _ in 0..15 {
            psg.env_step();
        }
        assert_eq!(psg.env_pos, 0, "decay completes");
        for _ in 0..15 {
            psg.env_step();
        }
        assert_eq!(psg.env_pos, 15, "attack completes");
        psg.env_step();
        assert_eq!(psg.env_pos, 15, "shape 11 holds at the top after one cycle");
    }

    #[test]
    fn test_envelope_shape_13_holds_at_top() {
        let mut psg = Psg::default();
        write_reg(&mut psg, 13, 0x0D);
        for _ in 0..15 {
            psg.env_step();
        }
        assert_eq!(psg.env_pos, 15);
        psg.env_step();
        assert_eq!(psg.env_pos, 15, "shape 13 holds at the top");
    }

    #[test]
    fn test_envelope_shape_15_attack_decay_holds_at_zero() {
        let mut psg = Psg::default();
        write_reg(&mut psg, 13, 0x0F);
        for _ in 0..15 {
            psg.env_step();
        }
        assert_eq!(psg.env_pos, 15, "attack completes");
        for _ in 0..15 {
            psg.env_step();
        }
        assert_eq!(psg.env_pos, 0, "decay completes");
        psg.env_step();
        assert_eq!(psg.env_pos, 0, "shape 15 holds at zero after one cycle");
    }

    #[test]
    fn test_envelope_resets_on_shape_write() {
        let mut psg = Psg::default();
        write_reg(&mut psg, 13, 0x08);
        for _ in 0..10 {
            psg.env_step();
        }
        assert_eq!(psg.env_pos, 5);
        write_reg(&mut psg, 13, 0x08); // rewrite: counter restarts
        assert_eq!(psg.env_pos, 15);
        assert_eq!(psg.env_phase, 0);
    }

    #[test]
    fn test_volume_curve_matches_hacker_kay_table() {
        // Spot-check the measured amplitude table used by Caprice32.
        assert_eq!(super::VOLUME[0], 0.0);
        assert_eq!(super::VOLUME[1], 836.0 / 65535.0);
        assert_eq!(super::VOLUME[15], 1.0);
        assert_eq!(PSG_CLOCK_HZ, 1_000_000);
    }
}
