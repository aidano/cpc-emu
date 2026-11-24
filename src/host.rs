// Host plumbing: the emulation runs in a dedicated thread, fully decoupled
// from the UI. Display frames flow to the UI through a latest-wins mailbox,
// keyboard input flows back through a channel, and audio goes straight to
// the output ring from the emulation thread — so UI work (window events,
// presentation, image copies) can never stall sound production, and the
// emulation never waits on the UI.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use log::debug;

use crate::devices::keyboard::Keyboard;
use crate::joystick::JoyInputs;
use crate::runtime::{CYCLES_PER_FRAME, Runtime};
use crate::video::{Frame, renderer};

/// The machine's display frame period: 50Hz.
const FRAME_DURATION: Duration = Duration::from_millis(20);

/// Display footprint every mode is normalised to (see `Frame::scaled_into`).
const DISPLAY_WIDTH: u16 = 640;
const DISPLAY_HEIGHT: u16 = 400;

/// After a stall this long (debugger break, suspended process), restart the
/// pacing schedule instead of emitting a burst of catch-up frames.
const MAX_CATCHUP: Duration = Duration::from_millis(100);

struct MailboxInner {
    slots: [Frame; 2],
    /// Index of the newest published slot, if any frame was published yet.
    latest: Option<usize>,
    /// Slot the producer writes next. Both sides operate under the mutex,
    /// so a publish can never tear a concurrent read of the other slot.
    producer_slot: usize,
}

/// A latest-wins frame handoff from the emulation thread to the display.
/// Neither side ever blocks on the other for longer than one ~1MB copy: a
/// slow consumer just skips to the newest frame, and a slow producer simply
/// means the consumer re-presents the previous frame.
pub struct FrameMailbox {
    inner: Mutex<MailboxInner>,
}

impl FrameMailbox {
    pub fn new(width: u16, height: u16) -> FrameMailbox {
        FrameMailbox {
            inner: Mutex::new(MailboxInner {
                slots: [Frame::new(width, height), Frame::new(width, height)],
                latest: None,
                producer_slot: 0,
            }),
        }
    }

    /// Publishes `frame` as the newest display frame.
    pub fn publish(&self, frame: &Frame) {
        let mut inner = self.inner.lock().unwrap();
        let producer_slot = inner.producer_slot;
        let slot = &mut inner.slots[producer_slot];
        if slot.width != frame.width || slot.height != frame.height {
            *slot = Frame::new(frame.width, frame.height);
        }
        slot.rgba.copy_from_slice(&frame.rgba);
        inner.latest = Some(producer_slot);
        inner.producer_slot = 1 - producer_slot;
    }

    /// Copies the newest published frame into `out`; returns whether one
    /// existed. Frames published between polls are skipped: only the latest
    /// matters for display.
    pub fn read_latest(&self, out: &mut Frame) -> bool {
        let inner = self.inner.lock().unwrap();
        let Some(index) = inner.latest else { return false };
        let slot = &inner.slots[index];
        if out.width != slot.width || out.height != slot.height {
            *out = Frame::new(slot.width, slot.height);
        }
        out.rgba.copy_from_slice(&slot.rgba);
        true
    }
}

/// A message from the UI thread to the emulation thread.
pub enum Input {
    /// A guest keyboard event: the text of the key and whether it went
    /// down. Keys intercepted by the host (everything under Ctrl) never
    /// arrive here.
    Key { text: String, pressed: bool },
    /// Cold-reset the machine.
    Reset,
    /// Debug memory browser: read `len` bytes of guest-visible memory
    /// (through the CPU's ROM/RAM banking view) from `address` and return
    /// them over `reply`. Serviced at the emulation thread's next frame
    /// boundary, so the reply lags at most one frame.
    ReadMemory {
        address: u16,
        len: usize,
        reply: mpsc::Sender<Vec<u8>>,
    },
}

/// Copies the polled joystick masks into the keyboard matrix: the ports'
/// matrix entries always, and with mirroring enabled the guest's cursor
/// keys, Space and Return as well.
fn apply_joysticks(keyboard: &mut Keyboard, joy: &JoyInputs, joy_keys: bool) {
    keyboard.set_joystick(0, joy.mask(0));
    keyboard.set_joystick(1, joy.mask(1));
    if joy_keys {
        keyboard.set_mirror(0, joy.mask(0));
        keyboard.set_mirror(1, joy.mask(1));
    }
}

/// A running emulation thread; dropping it signals stop and joins.
pub struct EmuThread {
    stop: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl Drop for EmuThread {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// Spawns the thread that owns the emulated machine. Each 20ms deadline it
/// applies queued keyboard events and commands, refreshes the polled
/// joystick state, runs one machine frame (audio renders in lockstep
/// inside `run_for_cycles`), publishes the display frame to the mailbox
/// and paces itself with a drift-correcting schedule, so host scheduling
/// jitter never accumulates. While `paused` is set the machine stops
/// advancing but the thread keeps draining input and pacing, so a later
/// unpause resumes cleanly. `joystick` carries the shared gamepad state
/// (None when gamepad support is off); `joy_keys` enables the guest
/// keyboard mirror for it.
pub fn spawn_emu_thread(
    mut runtime: Runtime,
    mailbox: Arc<FrameMailbox>,
    input: Receiver<Input>,
    paused: Arc<AtomicBool>,
    joystick: Option<Arc<JoyInputs>>,
    joy_keys: bool,
) -> EmuThread {
    let stop = Arc::new(AtomicBool::new(false));
    let stop_flag = Arc::clone(&stop);
    let handle = thread::Builder::new()
        .name("cpc-emu".into())
        .spawn(move || {
            let mut machine_frame = Frame::new(1, 1);
            let mut scaled = Frame::new(DISPLAY_WIDTH, DISPLAY_HEIGHT);
            let mut next_deadline = Instant::now() + FRAME_DURATION;
            let mut diag_frame: u32 = 0;
            loop {
                if stop_flag.load(Ordering::Relaxed) {
                    break;
                }
                // Apply everything queued on the input channel; a closed
                // channel just means nobody is sending keys.
                while let Ok(input) = input.try_recv() {
                    match input {
                        Input::Key { text, pressed } => {
                            debug!("key {text:?} pressed={pressed}");
                            if pressed {
                                runtime.components.keyboard.press_text(&text);
                            } else {
                                runtime.components.keyboard.release_text(&text);
                            }
                        }
                        Input::Reset => runtime.reset(),
                        Input::ReadMemory { address, len, reply } => {
                            let bytes = (0..len)
                                .map(|i| {
                                    runtime
                                        .components
                                        .mem
                                        .read(address.wrapping_add(i as u16))
                                })
                                .collect();
                            let _ = reply.send(bytes);
                        }
                    }
                }

                if let Some(joy) = &joystick {
                    apply_joysticks(&mut runtime.components.keyboard, joy, joy_keys);
                }

                // Once per second: what the guest polled from the keyboard
                // matrix — shows whether (and which lines) a game's input
                // code reads, independent of any keys being pressed. A zero
                // count while input is expected means the game's reader
                // stopped polling (for example a music driver that hijacked
                // the frame-fly interrupt). The reg-14 side categories flag
                // protocol divergence: reads gated by PSG port A output
                // mode, or with the PSG control lines out of read mode (the
                // port floats high and always sees no keys). The interrupt
                // stats show who owns the frame-fly regime — a game's own
                // IM 2 handler or the firmware's RST 38h path.
                diag_frame += 1;
                if diag_frame >= 50 {
                    diag_frame = 0;
                    let ppi = &mut runtime.components.ppi;
                    let total: u32 = ppi.keyboard_reads.iter().sum();
                    let lines: String = ppi
                        .keyboard_reads
                        .iter()
                        .enumerate()
                        .filter(|(_, &reads)| reads > 0)
                        .map(|(line, reads)| format!("L{}×{}", line, reads))
                        .collect::<Vec<_>>()
                        .join(" ");
                    // The guest's modifier bits: a stuck Shift (the old
                    // uppercasing bug) or a latched key shows here.
                    let keyboard = &runtime.components.keyboard;
                    let (shift, ctrl, caps) =
                        (keyboard.row(2) & 0x20 == 0, keyboard.row(2) & 0x80 == 0, keyboard.row(8) & 0x40 == 0);
                    debug!(
                        "keyboard reads/s: {} [{}] reg14-out-mode: {} reg14-stale: {} mods: shift={} ctrl={} caps={}",
                        total,
                        lines,
                        ppi.reg14_output_reads,
                        ppi.reg14_stale_reads,
                        shift as u8,
                        ctrl as u8,
                        caps as u8
                    );
                    // Who reads the keyboard: top reading PCs (ROM = the
                    // firmware's scan routine, high RAM = game code).
                    let mut readers: Vec<(u16, u32)> =
                        ppi.keyboard_read_pcs.iter().map(|(&pc, &n)| (pc, n)).collect();
                    readers.sort_by_key(|&(_, n)| std::cmp::Reverse(n));
                    let readers: String = readers
                        .iter()
                        .take(4)
                        .map(|(pc, n)| format!("{:04X}×{}", pc, n))
                        .collect::<Vec<_>>()
                        .join(" ");
                    debug!("keyboard readers: {}", readers);
                    ppi.keyboard_reads = [0; 16];
                    ppi.reg14_output_reads = 0;
                    ppi.reg14_stale_reads = 0;
                    ppi.keyboard_read_pcs.clear();
                    debug!(
                        "interrupts: {} ack/s, {} frame-fly/s, im={}, im2→{:04X}",
                        runtime.components.interrupts_ack,
                        runtime.components.frame_fly_fires,
                        runtime.components.registers.interrupt_mode,
                        runtime.components.last_im2_target
                    );
                    runtime.components.interrupts_ack = 0;
                    runtime.components.frame_fly_fires = 0;
                }

                if !paused.load(Ordering::Relaxed) {
                    // One machine frame; audio renders inside in ~5ms slices.
                    runtime.run_for_cycles(CYCLES_PER_FRAME);

                    renderer::render_into(
                        &runtime.components.mem,
                        &runtime.components.gate_array,
                        &runtime.components.crtc,
                        &mut machine_frame,
                    );
                    // The CRTC's window (R1/R6) sizes the machine frame; scale
                    // by integer factors, never below 1x: a window larger than
                    // the display footprint crops instead of dividing by zero.
                    machine_frame.scaled_into(
                        (DISPLAY_WIDTH / machine_frame.width).max(1),
                        (DISPLAY_HEIGHT / machine_frame.height).max(1),
                        &mut scaled,
                    );
                    mailbox.publish(&scaled);
                }

                // Pace to the next frame deadline. A long stall restarts
                // the schedule rather than bursting catch-up frames.
                let now = Instant::now();
                if let Some(late) = now.checked_duration_since(next_deadline) {
                    if late > MAX_CATCHUP {
                        next_deadline = now + FRAME_DURATION;
                    }
                }
                sleep_to(next_deadline);
                next_deadline += FRAME_DURATION;
            }
            runtime.flush_audio();
        })
        .expect("failed to spawn the emulation thread");
    EmuThread {
        stop,
        handle: Some(handle),
    }
}

/// Sleeps until `deadline`: coarse sleeps while far away, short sleeps near
/// it, and yields for the final microseconds — host sleep granularity is
/// coarser than the tail of the 20ms frame budget, and the deadline
/// accumulator would otherwise drift by the oversleep every frame.
fn sleep_to(deadline: Instant) {
    loop {
        let now = Instant::now();
        if now >= deadline {
            return;
        }
        let remaining = deadline - now;
        if remaining > Duration::from_millis(1) {
            thread::sleep(remaining - Duration::from_millis(1));
        } else if remaining > Duration::from_micros(200) {
            thread::sleep(remaining / 2);
        } else {
            thread::yield_now();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{FrameMailbox, Input, apply_joysticks, spawn_emu_thread};
    use crate::audio::audio_ring;
    use crate::devices::keyboard::{JOY_FIRE1, JOY_START, JOY_UP};
    use crate::joystick::JoyInputs;
    use crate::runtime::Runtime;
    use crate::video::Frame;
    use std::sync::mpsc;
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;
    use std::thread;
    use std::time::{Duration, Instant};

    #[test]
    fn test_apply_joysticks_updates_the_matrix() {
        let joy = JoyInputs::new();
        joy.set(0, JOY_UP | JOY_FIRE1);
        joy.set(1, 0);
        let mut keyboard = crate::devices::keyboard::Keyboard::default();
        apply_joysticks(&mut keyboard, &joy, false);
        assert_eq!(keyboard.row(9), 0xDE, "player 1's up + fire 1 on line 9");
        assert_eq!(keyboard.row(6), 0xFF, "player 2's pad is idle");
        assert_eq!(keyboard.row(0), 0xFF, "mirroring off leaves the guest keys alone");
        joy.set(1, JOY_FIRE1);
        apply_joysticks(&mut keyboard, &joy, false);
        assert_eq!(keyboard.row(6), 0xDF, "player 2's fire 1 on line 6");
    }

    #[test]
    fn test_apply_joysticks_mirrors_guest_keys_when_enabled() {
        let joy = JoyInputs::new();
        joy.set(0, JOY_UP | JOY_START);
        let mut keyboard = crate::devices::keyboard::Keyboard::default();
        apply_joysticks(&mut keyboard, &joy, true);
        // Up mirrors onto cursor up (line 0, bit 0) and Start onto Return
        // (line 0, bit 6); the port only sees up — Start never reaches it.
        assert_eq!(keyboard.row(0), 0xBE, "cursor up + return mirrored");
        assert_eq!(keyboard.row(9), 0xFE);
    }

    fn frame_with(width: u16, height: u16, value: u8) -> Frame {
        let mut frame = Frame::new(width, height);
        frame.rgba.iter_mut().for_each(|b| *b = value);
        frame
    }

    #[test]
    fn test_mailbox_latest_wins() {
        let mailbox = FrameMailbox::new(2, 2);
        let mut out = Frame::new(1, 1);
        assert!(!mailbox.read_latest(&mut out), "no frame published yet");

        mailbox.publish(&frame_with(2, 2, 1));
        mailbox.publish(&frame_with(2, 2, 2));
        assert!(mailbox.read_latest(&mut out));
        assert_eq!((out.width, out.height), (2, 2));
        assert!(out.rgba.iter().all(|&b| b == 2), "the newest frame wins");
    }

    #[test]
    fn test_emu_thread_paces_frames_and_audio() {
        let (producer, consumer) = audio_ring();
        let mut runtime = Runtime::default();
        runtime.audio = Some(producer);
        runtime.components.psg.sample_rate = 44_100;

        let mailbox = Arc::new(FrameMailbox::new(640, 400));
        let (_tx, rx) = mpsc::channel::<Input>();
        let _emu = spawn_emu_thread(
            runtime,
            Arc::clone(&mailbox),
            rx,
            Arc::new(AtomicBool::new(false)),
            None,
            false,
        );

        // ~300ms of real time: at 50Hz the thread presents a frame roughly
        // every 20ms and owes ~13,230 audio samples. The bounds stay
        // generous so a loaded host cannot flake the test, but a thread
        // that runs unpaced (or not at all) falls far outside them.
        let deadline = Instant::now() + Duration::from_millis(300);
        let mut display = Frame::new(640, 400);
        let mut got_frame = false;
        let mut total_samples = 0usize;
        let mut buf = [0f32; 4096];
        while Instant::now() < deadline {
            got_frame |= mailbox.read_latest(&mut display);
            loop {
                let got = consumer.pop(&mut buf);
                if got == 0 {
                    break;
                }
                total_samples += got;
            }
            thread::sleep(Duration::from_millis(5));
        }
        assert!(got_frame, "the emulation thread must publish display frames");
        assert!(
            (5_000..=20_000).contains(&total_samples),
            "audio production must track the paced machine time, got {}",
            total_samples
        );
    }

    #[test]
    fn test_emu_thread_serves_memory_reads() {
        let mut runtime = Runtime::default();
        // Two markers a page apart-spanning read must pick up in order.
        runtime.components.mem.write(0x1234, 0xAB);
        runtime.components.mem.write(0x1239, 0xCD);

        let mailbox = Arc::new(FrameMailbox::new(2, 2));
        let (tx, rx) = mpsc::channel::<Input>();
        // Paused: the thread only drains input and paces, but must still
        // service the read request.
        let paused = Arc::new(AtomicBool::new(true));
        let _emu = spawn_emu_thread(runtime, mailbox, rx, paused, None, false);

        let (reply_tx, reply_rx) = mpsc::channel();
        tx.send(Input::ReadMemory {
            address: 0x1230,
            len: 16,
            reply: reply_tx,
        })
        .unwrap();
        let bytes = reply_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("the emulation thread must reply to memory reads");
        assert_eq!(bytes.len(), 16);
        assert_eq!(bytes[0x1234 - 0x1230], 0xAB);
        assert_eq!(bytes[0x1239 - 0x1230], 0xCD);
        // Default RAM reads as 0x01 elsewhere.
        assert_eq!(bytes[0x1230 - 0x1230], 0x01);
    }
}
