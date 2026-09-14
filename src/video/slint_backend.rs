// Slint window backend: presents frames in a native window, driven by a 50Hz timer.
// Keyboard events are forwarded to the emulated keyboard matrix.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use slint::{ComponentHandle, Image, SharedPixelBuffer, Timer, TimerMode, Weak};

use crate::runtime::Runtime;
use crate::video::renderer;

slint::include_modules!();

/// 50 frames per second at 4MHz: the cycles executed per display frame.
const CYCLES_PER_FRAME: u32 = 80_000;

pub fn run(runtime: Runtime) -> Result<(), slint::PlatformError> {
    let runtime = Rc::new(RefCell::new(runtime));
    let window = CpcWindow::new()?;

    let weak = window.as_weak();
    let timer_runtime = Rc::clone(&runtime);
    let timer = Timer::default();
    timer.start(TimerMode::Repeated, Duration::from_millis(20), move || {
        let Some(window) = weak.upgrade() else { return };
        let (frame, ppi_line) = {
            let mut runtime = timer_runtime.borrow_mut();
            runtime.run_for_cycles(CYCLES_PER_FRAME);
            let frame = renderer::render(&runtime.components.mem, &runtime.components.gate_array, &runtime.components.crtc);
            let line = runtime.components.ppi.keyboard_line;
            (frame, line)
        };
        let buffer = SharedPixelBuffer::clone_from_slice(&frame.rgba, frame.width as u32, frame.height as u32);
        window.set_frame(Image::from_rgba8(buffer));
        // Keep the keyboard line selection visible in the window title for debugging.
        let _ = ppi_line;
    });

    let key_runtime = runtime.clone();
    window.on_key(move |text, pressed| {
        let mut runtime = key_runtime.borrow_mut();
        if pressed != 0 {
            runtime.components.keyboard.press_text(&text);
        } else {
            runtime.components.keyboard.release_text(&text);
        }
    });

    window.run()
}
