// µPD765 floppy disc controller as wired in the CPC disc interface:
//   &FA7E  motor flip-flop (write bit 0; global, all drives)
//   &FB7E  main status register (read only)
//   &FB7F  data register (read/write)
// The interface is selected when address bit 10 is reset. The CPC FDC has no
// interrupt line: software polls the main status register through the three
// command phases (command / execution / result). Drive 0 is fitted; data
// reads need the motor running. Sector delivery is instant (no rotational
// timing), which is indistinguishable for AMSDOS's polled transfers.

use std::collections::VecDeque;

use crate::dsk::Dsk;

/// Main status register bits.
const MSR_RQM: u8 = 0x80;
const MSR_DIO: u8 = 0x40;
const MSR_EXM: u8 = 0x20;
const MSR_CB: u8 = 0x10;

/// ST0 interrupt codes.
const IC_NORMAL: u8 = 0x00;
const IC_ABNORMAL: u8 = 0x40;
const IC_INVALID: u8 = 0x80;

/// ST1 "no data" (sector id not found on the cylinder).
const ST1_NO_DATA: u8 = 0x04;
/// ST0 "not ready".
const ST0_NOT_READY: u8 = 0x08;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// Accepting a new command byte.
    Idle,
    /// Collecting the command + parameter bytes.
    Command,
    /// Sector data being drained through the data register.
    Execution,
    /// Returning the result bytes.
    Result,
}

/// A located sector, copied out of the disc image.
struct FoundSector {
    data: Vec<u8>,
    c: u8,
    h: u8,
    r: u8,
    n: u8,
    st1: u8,
    st2: u8,
}

pub struct Fdc {
    pub motor: bool,
    phase: Phase,
    /// Bytes collected during the command phase.
    command: Vec<u8>,
    /// Total bytes the current command expects in the command phase.
    expected: Option<usize>,
    /// Sector bytes available during the execution phase.
    read_queue: VecDeque<u8>,
    /// Result bytes waiting for the execution phase to drain.
    pending_results: VecDeque<u8>,
    /// Result-phase bytes being returned.
    results: VecDeque<u8>,
    /// Current cylinder per drive (PCN).
    tracks: [u8; 4],
    /// Seek/recalibrate completions awaiting Sense Interrupt Status.
    seek_pending: [bool; 4],
    /// The inserted disc image (drive 0).
    pub disk: Option<Dsk>,
}

impl Fdc {
    pub fn default() -> Fdc {
        Fdc {
            motor: false,
            phase: Phase::Idle,
            command: Vec::new(),
            expected: None,
            read_queue: VecDeque::new(),
            pending_results: VecDeque::new(),
            results: VecDeque::new(),
            tracks: [0; 4],
            seek_pending: [false; 4],
            disk: None,
        }
    }

    /// The motor flip-flop at &FA7E: bit 0 drives all drive motors.
    pub fn set_motor(&mut self, on: bool) {
        self.motor = on;
    }

    /// Inserts (or removes) the disc in drive 0.
    pub fn set_disk(&mut self, disk: Option<Dsk>) {
        self.disk = disk;
    }

    fn drive_busy_bits(&self) -> u8 {
        let mut bits = 0;
        for (drive, pending) in self.seek_pending.iter().enumerate() {
            if *pending {
                bits |= 1 << drive;
            }
        }
        bits
    }

    /// &FB7E: the main status register.
    pub fn read_status(&self) -> u8 {
        let v = match self.phase {
            Phase::Idle => MSR_RQM | self.drive_busy_bits(),
            Phase::Command => MSR_RQM | MSR_CB | self.drive_busy_bits(),
            Phase::Execution => MSR_RQM | MSR_DIO | MSR_EXM | MSR_CB | self.drive_busy_bits(),
            Phase::Result => MSR_RQM | MSR_DIO | MSR_CB | self.drive_busy_bits(),
        };
        v
    }

    /// &FB7F read: execution-phase data or result bytes.
    pub fn read_data(&mut self) -> u8 {
        let value = match self.phase {
            Phase::Execution => {
                match self.read_queue.pop_front() {
                    Some(byte) => {
                        if self.read_queue.is_empty() {
                            // The sector drained completely: publish the
                            // result bytes and enter the result phase.
                            self.results = std::mem::take(&mut self.pending_results);
                            self.phase = Phase::Result;
                        }
                        byte
                    },
                    None => {
                        self.phase = Phase::Idle;
                        0
                    },
                }
            },
            Phase::Result => {
                let byte = self.results.pop_front().unwrap_or(0);
                if self.results.is_empty() {
                    self.phase = Phase::Idle;
                }
                byte
            },
            _ => 0xFF,
        };
        value
    }

    /// &FB7F write: command/parameter bytes.
    pub fn write_data(&mut self, value: u8) {
        match self.phase {
            Phase::Idle | Phase::Command => {
                if self.phase == Phase::Idle {
                    self.command.clear();
                    self.phase = Phase::Command;
                    match self.command_length(value) {
                        Some(len) => self.expected = Some(len),
                        None => {
                            // Unknown opcode: the real chip rejects it with
                            // an invalid-code ST0 in the result phase.
                            self.enter_result_phase(&[IC_INVALID]);
                            return;
                        },
                    }
                }
                self.command.push(value);
                if Some(self.command.len()) == self.expected {
                    self.execute();
                }
            },
            // Writes during the execution/result phases are ignored: the
            // CPC interface does not wire the FDC's terminal-count line.
            _ => {},
        }
    }

    /// The number of command-phase bytes each command takes, or None for
    /// commands this emulation does not implement (write/format/scan).
    fn command_length(&self, opcode: u8) -> Option<usize> {
        match opcode & 0x1F {
            0x03 => Some(3), // Specify
            0x04 => Some(2), // Sense Drive Status
            0x06 => Some(9), // Read Data (+ MT/MF/SK variants)
            0x07 => Some(2), // Recalibrate
            0x08 => Some(1), // Sense Interrupt State
            0x0A => Some(2), // Read ID
            0x0F => Some(3), // Seek
            _ => None,
        }
    }

    fn enter_result_phase(&mut self, results: &[u8]) {
        self.results.extend(results.iter().copied());
        self.phase = Phase::Result;
    }

    fn execute(&mut self) {
        let cmd = std::mem::take(&mut self.command);
        self.expected = None;
        match cmd[0] & 0x1F {
            0x08 => self.sense_interrupt_state(),
            0x03 => {
                // Specify: sets timings only; the CPC interface is non-DMA.
                self.phase = Phase::Idle;
            },
            0x07 => self.recalibrate(&cmd),
            0x0F => self.seek(&cmd),
            0x04 => self.sense_drive_status(&cmd),
            0x0A => self.read_id(&cmd),
            0x06 => self.read_data_cmd(&cmd),
            _ => {
                // Invalid command: ST0 with IC = invalid.
                self.enter_result_phase(&[IC_INVALID]);
            },
        }
    }

    /// Terminates a seek/recalibrate. With none pending the real chip
    /// reports ST0 = 0x80 (IC = invalid, no interrupt occurred).
    fn sense_interrupt_state(&mut self) {
        for drive in 0..4 {
            if self.seek_pending[drive] {
                self.seek_pending[drive] = false;
                let st0 = IC_NORMAL | 0x20 | drive as u8; // SE: seek end
                self.enter_result_phase(&[st0, self.tracks[drive]]);
                return;
            }
        }
        self.enter_result_phase(&[IC_INVALID, self.tracks[0]]);
    }

    fn recalibrate(&mut self, cmd: &[u8]) {
        let drive = (cmd[1] & 3) as usize;
        self.tracks[drive] = 0;
        self.seek_pending[drive] = true;
        self.phase = Phase::Idle;
    }

    fn seek(&mut self, cmd: &[u8]) {
        let drive = (cmd[1] & 3) as usize;
        self.tracks[drive] = cmd[2];
        self.seek_pending[drive] = true;
        self.phase = Phase::Idle;
    }

    /// ST3: drive status. Only drive 0 is fitted; ready follows the motor.
    fn sense_drive_status(&mut self, cmd: &[u8]) {
        let drive = (cmd[1] & 3) as usize;
        let mut st3 = 0u8;
        if drive == 0 {
            if self.motor {
                st3 |= 0x20; // RY: ready
            }
            if self.tracks[0] == 0 {
                st3 |= 0x04; // T0: at track 0
            }
        } else {
            st3 = 0x08; // an unfitted drive never reports ready
        }
        self.enter_result_phase(&[st3]);
    }

    fn read_id(&mut self, cmd: &[u8]) {
        let (hd, us) = decode_unit(cmd[1]);
        if !self.motor || us != 0 || self.disk.is_none() {
            self.not_ready_result(hd, us);
            return;
        }
        let track = self.tracks[0];
        match self.locate(track, hd, None) {
            Some(sector) => {
                let st0 = IC_NORMAL | (hd << 2) | us;
                self.enter_result_phase(&[
                    st0, 0, 0, sector.c, sector.h, sector.r, sector.n,
                ]);
            },
            None => {
                let st0 = IC_ABNORMAL | (hd << 2) | us;
                self.enter_result_phase(&[st0, ST1_NO_DATA, 0, 0, hd, 1, 2]);
            },
        }
    }

    fn read_data_cmd(&mut self, cmd: &[u8]) {
        let (hd, us) = decode_unit(cmd[1]);
        let c = cmd[2];
        let h = cmd[3];
        let r = cmd[4];
        let n = cmd[5];
        if !self.motor || us != 0 || self.disk.is_none() {
            self.not_ready_result(hd, us);
            return;
        }
        if self.tracks[0] != c {
            // The head sits on a different cylinder: the id cannot be found.
            let st0 = IC_ABNORMAL | (hd << 2) | us;
            self.enter_result_phase(&[st0, ST1_NO_DATA, 0, c, h, r, n]);
            return;
        }
        match self.locate(c, h, Some(r)) {
            Some(sector) => {
                let st0 = IC_NORMAL | (hd << 2) | us;
                self.read_queue.extend(sector.data.iter().copied());
                self.pending_results.extend([
                    st0, sector.st1, sector.st2, sector.c, sector.h, sector.r, sector.n,
                ]);
                self.phase = Phase::Execution;
            },
            None => {
                let st0 = IC_ABNORMAL | (hd << 2) | us;
                self.enter_result_phase(&[st0, ST1_NO_DATA, 0, c, h, r, n]);
            },
        }
    }

    fn not_ready_result(&mut self, hd: u8, us: u8) {
        self.enter_result_phase(&[
            IC_ABNORMAL | ST0_NOT_READY | (hd << 2) | us,
            0, 0, 0, 0, 0, 0,
        ]);
    }

    /// Locates a sector on the disc: by id when Some(r), otherwise the first
    /// sector of the cylinder (Read ID).
    fn locate(&self, c: u8, h: u8, r: Option<u8>) -> Option<FoundSector> {
        let disk = self.disk.as_ref()?;
        let view = match r {
            Some(r) => disk.read_sector(c, h, r)?,
            None => disk.first_sector(c, h)?,
        };
        Some(FoundSector {
            data: view.data.to_vec(),
            c: view.c,
            h: view.h,
            r: view.r,
            n: view.n,
            st1: view.st1,
            st2: view.st2,
        })
    }
}

fn decode_unit(byte: u8) -> (u8, u8) {
    let hd = (byte >> 2) & 1;
    let us = byte & 3;
    (hd, us)
}

#[cfg(test)]
mod tests {
    use super::Fdc;

    /// Builds a minimal NORMAL-format DSK with one 9-sector track holding
    /// sector 0xC1 at track 0, side 0.
    fn synthetic_dsk() -> Vec<u8> {
        let mut dsk = vec![0u8; 0x100 + 0x100 + 9 * 512];
        dsk[..11].copy_from_slice(b"MV - CPCEMU");
        dsk[0x30] = 1; // track count
        dsk[0x31] = 1; // side count
        dsk[0x32] = 0x00; // track size low
        dsk[0x33] = 0x13; // track size high: 0x1300 = 4864
        let tib = 0x100;
        dsk[tib + 0x10] = 0; // track number
        dsk[tib + 0x11] = 0; // side number
        dsk[tib + 0x14] = 2; // sector size code
        dsk[tib + 0x15] = 1; // sector count
        dsk[tib + 0x18] = 0; // sector C
        dsk[tib + 0x19] = 0; // sector H
        dsk[tib + 0x1A] = 0xC1; // sector R
        dsk[tib + 0x1B] = 2; // sector N
        let data_start = tib + 0x100;
        for (i, b) in dsk[data_start..data_start + 512].iter_mut().enumerate() {
            *b = (i & 0xFF) as u8;
        }
        dsk
    }

    fn fdc_with_disk() -> Fdc {
        let mut fdc = Fdc::default();
        fdc.set_disk(Some(crate::dsk::Dsk::init_from_bytes(&synthetic_dsk()).unwrap()));
        fdc.set_motor(true);
        fdc
    }

    fn write_command(fdc: &mut Fdc, bytes: &[u8]) {
        for b in bytes {
            fdc.write_data(*b);
        }
    }

    #[test]
    fn test_motor_latch_and_status() {
        let mut fdc = Fdc::default();
        fdc.set_motor(true);
        // Sense Drive Status: drive 0 ready at track 0.
        write_command(&mut fdc, &[0x04, 0x00]);
        assert_eq!(fdc.read_data(), 0x24); // RY | T0
        fdc.set_motor(false);
        write_command(&mut fdc, &[0x04, 0x00]);
        assert_eq!(fdc.read_data(), 0x04); // T0 only
        // Unfitted drive: never ready.
        write_command(&mut fdc, &[0x04, 0x01]);
        assert_eq!(fdc.read_data(), 0x08);
    }

    #[test]
    fn test_seek_then_sense_interrupt_state() {
        let mut fdc = Fdc::default();
        // Seek drive 0 to cylinder 5.
        write_command(&mut fdc, &[0x0F, 0x00, 0x05]);
        assert_eq!(fdc.phase, super::Phase::Idle);
        assert!(fdc.read_status() & 0x01 != 0, "drive 0 busy until SIS");
        // Sense Interrupt State terminates the seek.
        write_command(&mut fdc, &[0x08]);
        assert_eq!(fdc.read_data(), 0x20); // ST0: SE, drive 0
        assert_eq!(fdc.read_data(), 0x05); // PCN
        assert_eq!(fdc.phase, super::Phase::Idle);
        // A further SIS with nothing pending reports invalid.
        write_command(&mut fdc, &[0x08]);
        assert_eq!(fdc.read_data(), 0x80);
    }

    #[test]
    fn test_recalibrate_restores_track_zero() {
        let mut fdc = Fdc::default();
        write_command(&mut fdc, &[0x0F, 0x00, 0x09]); // seek to 9
        write_command(&mut fdc, &[0x08]);
        assert_eq!(fdc.read_data(), 0x20); // SE
        assert_eq!(fdc.read_data(), 0x09); // PCN
        write_command(&mut fdc, &[0x07, 0x00]); // recalibrate
        write_command(&mut fdc, &[0x08]);
        assert_eq!(fdc.read_data(), 0x20); // SE
        assert_eq!(fdc.read_data(), 0x00); // PCN back to 0
    }

    #[test]
    fn test_read_data_delivers_sector() {
        let mut fdc = fdc_with_disk();
        // Read Data, drive 0, C=0 H=0 R=0xC1 N=2, EOT=9.
        write_command(&mut fdc, &[0x46, 0x00, 0x00, 0x00, 0xC1, 0x02, 0x09, 0x2A, 0xFF]);
        assert_eq!(fdc.read_status() & super::MSR_EXM, super::MSR_EXM);
        let first = fdc.read_data();
        assert_eq!(first, 0x00);
        let mut count = 1;
        while fdc.read_status() & super::MSR_CB != 0 {
            let _ = fdc.read_data();
            count += 1;
        }
        assert_eq!(count, 512 + 7, "512 sector bytes + 7 result bytes");
        // The last result byte is N = 2.
        // (read_data returned 512 data bytes then ST0..N)
    }

    #[test]
    fn test_read_data_missing_sector_reports_no_data() {
        let mut fdc = fdc_with_disk();
        write_command(&mut fdc, &[0x46, 0x00, 0x00, 0x00, 0xC9, 0x02, 0x09, 0x2A, 0xFF]);
        // Result phase straight away: ST0 abnormal, ST1 = no data.
        assert_eq!(fdc.read_data(), 0x40);
        assert_eq!(fdc.read_data(), 0x04);
    }

    #[test]
    fn test_read_requires_motor() {
        let mut fdc = fdc_with_disk();
        fdc.set_motor(false);
        write_command(&mut fdc, &[0x46, 0x00, 0x00, 0x00, 0xC1, 0x02, 0x09, 0x2A, 0xFF]);
        // ST0: abnormal + not ready.
        assert_eq!(fdc.read_data(), 0x48);
    }

    #[test]
    fn test_read_id_and_invalid_command() {
        let mut fdc = fdc_with_disk();
        write_command(&mut fdc, &[0x4A, 0x00]); // Read ID
        let st0 = fdc.read_data();
        assert_eq!(st0 & 0xC0, 0x00, "normal termination");
        assert_eq!(fdc.read_data(), 0x00); // ST1
        assert_eq!(fdc.read_data(), 0x00); // ST2
        assert_eq!(fdc.read_data(), 0x00); // C
        assert_eq!(fdc.read_data(), 0x00); // H
        assert_eq!(fdc.read_data(), 0xC1); // R
        assert_eq!(fdc.read_data(), 0x02); // N
        assert_eq!(fdc.phase, super::Phase::Idle);
        // Invalid opcode.
        write_command(&mut fdc, &[0x02]);
        assert_eq!(fdc.read_data(), 0x80);
    }

    #[test]
    fn test_specify_is_silent() {
        let mut fdc = Fdc::default();
        write_command(&mut fdc, &[0x03, 0xDF, 0x02]);
        assert_eq!(fdc.phase, super::Phase::Idle);
        assert_eq!(fdc.read_status(), super::MSR_RQM);
    }
}
