// Slint window backend: presents frames produced by the dedicated emulation
// thread through a latest-wins mailbox; keyboard events are forwarded to the
// emulation thread over a channel. The UI thread never touches the machine,
// so window and presentation work cannot interfere with emulation pacing or
// audio production.

use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;

use slint::{ComponentHandle, Image, SharedPixelBuffer, Timer, TimerMode, Weak};

use crate::audio;
use crate::host::{self, FrameMailbox};
use crate::runtime::Runtime;
use crate::video::Frame;

slint::include_modules!();

/// Presentation cadence: 50Hz, matching the machine's frame rate.
const PRESENT_INTERVAL: Duration = Duration::from_millis(20);

pub fn run(
    runtime: Runtime,
    _audio: Option<audio::AudioOutput>,
) -> Result<(), slint::PlatformError> {
    let mailbox = Arc::new(FrameMailbox::new(640, 400));
    let (key_tx, key_rx) = mpsc::channel::<(String, i32)>();
    // The emulation thread owns the machine from here on; the UI thread
    // only presents frames and forwards input.
    let _emu = host::spawn_emu_thread(runtime, Arc::clone(&mailbox), key_rx);

    let window = CpcWindow::new()?;

    let weak = window.as_weak();
    let timer = Timer::default();
    let timer_mailbox = Arc::clone(&mailbox);
    let mut display = Frame::new(640, 400);
    timer.start(TimerMode::Repeated, PRESENT_INTERVAL, move || {
        let Some(window) = weak.upgrade() else { return };
        if !timer_mailbox.read_latest(&mut display) {
            return;
        }
        let buffer = SharedPixelBuffer::clone_from_slice(
            &display.rgba,
            display.width as u32,
            display.height as u32,
        );
        window.set_frame(Image::from_rgba8(buffer));
    });

    window.on_key(move |text, pressed| {
        let _ = key_tx.send((text.to_string(), pressed));
    });

    window.run()
}
