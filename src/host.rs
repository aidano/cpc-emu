// Host plumbing: the emulation runs in a dedicated thread, fully decoupled
// from the UI. Display frames flow to the UI through a latest-wins mailbox,
// keyboard input flows back through a channel, and audio goes straight to
// the output ring from the emulation thread — so UI work (window events,
// presentation, image copies) can never stall sound production, and the
// emulation never waits on the UI.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

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
/// applies queued keyboard events, runs one machine frame (audio renders in
/// lockstep inside `run_for_cycles`), publishes the display frame to the
/// mailbox and paces itself with a drift-correcting schedule, so host
/// scheduling jitter never accumulates.
pub fn spawn_emu_thread(
    mut runtime: Runtime,
    mailbox: Arc<FrameMailbox>,
    input: Receiver<(String, i32)>,
) -> EmuThread {
    let stop = Arc::new(AtomicBool::new(false));
    let stop_flag = Arc::clone(&stop);
    let handle = thread::Builder::new()
        .name("cpc-emu".into())
        .spawn(move || {
            let mut machine_frame = Frame::new(1, 1);
            let mut scaled = Frame::new(DISPLAY_WIDTH, DISPLAY_HEIGHT);
            let mut next_deadline = Instant::now() + FRAME_DURATION;
            loop {
                if stop_flag.load(Ordering::Relaxed) {
                    break;
                }
                // Apply everything queued on the input channel; a closed
                // channel just means nobody is sending keys.
                while let Ok((text, pressed)) = input.try_recv() {
                    if pressed != 0 {
                        runtime.components.keyboard.press_text(&text);
                    } else {
                        runtime.components.keyboard.release_text(&text);
                    }
                }

                // One machine frame; audio renders inside in ~5ms slices.
                runtime.run_for_cycles(CYCLES_PER_FRAME);

                renderer::render_into(
                    &runtime.components.mem,
                    &runtime.components.gate_array,
                    &runtime.components.crtc,
                    &mut machine_frame,
                );
                machine_frame.scaled_into(
                    DISPLAY_WIDTH / machine_frame.width,
                    DISPLAY_HEIGHT / machine_frame.height,
                    &mut scaled,
                );
                mailbox.publish(&scaled);

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
    use super::{FrameMailbox, spawn_emu_thread};
    use crate::audio::audio_ring;
    use crate::runtime::Runtime;
    use crate::video::Frame;
    use std::sync::mpsc;
    use std::sync::Arc;
    use std::thread;
    use std::time::{Duration, Instant};

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
        let (_tx, rx) = mpsc::channel::<(String, i32)>();
        let _emu = spawn_emu_thread(runtime, Arc::clone(&mailbox), rx);

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
}
