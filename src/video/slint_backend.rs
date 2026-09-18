// Slint window backend: presents frames produced by the dedicated emulation
// thread through a latest-wins mailbox; keyboard events are forwarded to the
// emulation thread over a channel, with host hotkeys (Ctrl-modified keys)
// intercepted here so they never reach the guest. The UI thread never
// touches the machine, so window and presentation work cannot interfere
// with emulation pacing or audio production.

use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;

use i_slint_backend_winit::winit::window::Fullscreen;
use i_slint_backend_winit::WinitWindowAccessor;
use slint::{ComponentHandle, Image, SharedPixelBuffer, Timer, TimerMode, Weak};

use crate::audio;
use crate::host::{self, FrameMailbox, Input};
use crate::runtime::Runtime;
use crate::video::Frame;

slint::include_modules!();

/// Presentation cadence: 50Hz, matching the machine's frame rate.
const PRESENT_INTERVAL: Duration = Duration::from_millis(20);

/// Host commands carried by the Ctrl super key.
enum Hotkey {
    Pause,
    Reset,
    Mute,
    Fullscreen,
    Quit,
}

/// The Slint key codes for the modifier keys themselves (Shift, Ctrl, Alt,
/// AltGr, Meta and their right-hand twins): they arrive as ordinary key
/// events whenever held alongside Ctrl (e.g. Shift during Ctrl+Shift+P),
/// and are never commands.
fn is_modifier_key(ch: char) -> bool {
    matches!(ch, '\u{10}'..='\u{13}' | '\u{15}'..='\u{18}')
}

/// Maps Ctrl-modified key text to a host command; the text for Ctrl+P is
/// the plain letter "p" (winit's logical key is unaffected by modifiers).
fn hotkey(text: &str) -> Option<Hotkey> {
    let ch = text.chars().next()?;
    if is_modifier_key(ch) {
        return None;
    }
    match ch.to_ascii_lowercase() {
        'p' => Some(Hotkey::Pause),
        'r' => Some(Hotkey::Reset),
        'm' => Some(Hotkey::Mute),
        'f' => Some(Hotkey::Fullscreen),
        'q' => Some(Hotkey::Quit),
        _ => None,
    }
}

pub fn run(
    runtime: Runtime,
    _audio: Option<audio::AudioOutput>,
    mute: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
) -> Result<(), slint::PlatformError> {
    let mailbox = Arc::new(FrameMailbox::new(640, 400));
    let (input_tx, input_rx) = mpsc::channel::<Input>();
    // The emulation thread owns the machine from here on; the UI thread
    // only presents frames and forwards input.
    let _emu = host::spawn_emu_thread(runtime, Arc::clone(&mailbox), input_rx, Arc::clone(&paused));

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

    // Host keys: every Ctrl-modified event is swallowed so the guest never
    // sees the super key, and the four hotkey letters act as commands.
    //
    // `control`/`meta` come straight from Slint's KeyEvent, which follows
    // Qt's macOS convention of swapping them (the physical Ctrl key is
    // reported as `meta`, Cmd as `control`): pick the flag that means the
    // physical Ctrl key on this platform.
    //
    // `held` tracks which keys the guest is actually holding: a release is
    // forwarded only for those, which both drops the trailing release of a
    // swallowed press and un-sticks a guest key whose release lands with
    // Ctrl held (e.g. type "p", hold Ctrl, release "p").
    let super_is_meta = cfg!(target_os = "macos");
    let fullscreen_state = Arc::new(AtomicBool::new(false));
    let held: Rc<RefCell<HashSet<String>>> = Rc::new(RefCell::new(HashSet::new()));
    let command_tx = input_tx.clone();
    let hotkey_mute = Arc::clone(&mute);
    let hotkey_paused = Arc::clone(&paused);
    let hotkey_fullscreen = Arc::clone(&fullscreen_state);
    let hotkey_window = window.as_weak();
    window.on_key(move |text, pressed, control, meta, repeat| {
        let text = text.to_string();
        let super_key = if super_is_meta { meta } else { control };
        if pressed != 0 {
            if super_key {
                held.borrow_mut().remove(&text);
                if !repeat {
                    match hotkey(&text) {
                        Some(Hotkey::Pause) => hotkey_paused
                            .store(!hotkey_paused.load(Ordering::Relaxed), Ordering::Relaxed),
                        Some(Hotkey::Reset) => {
                            let _ = command_tx.send(Input::Reset);
                        }
                        Some(Hotkey::Mute) => hotkey_mute
                            .store(!hotkey_mute.load(Ordering::Relaxed), Ordering::Relaxed),
                        Some(Hotkey::Fullscreen) => {
                            let desired = !hotkey_fullscreen.load(Ordering::Relaxed);
                            hotkey_fullscreen.store(desired, Ordering::Relaxed);
                            if let Some(window) = hotkey_window.upgrade() {
                                // Slint 1.7's winit backend ignores the exit
                                // request from Window::set_fullscreen(false)
                                // (its internal fullscreen marker is never
                                // updated after entering), so drive winit
                                // directly, with our own desired-state flag.
                                window.window().with_winit_window(|winit| {
                                    winit.set_fullscreen(if desired {
                                        Some(Fullscreen::Borderless(None))
                                    } else {
                                        None
                                    });
                                });
                            }
                        }
                        Some(Hotkey::Quit) => {
                            // Hiding the last window ends the event loop,
                            // so run() returns and the process unwinds
                            // cleanly (emu thread joined, audio closed).
                            if let Some(window) = hotkey_window.upgrade() {
                                let _ = window.hide();
                            }
                        }
                        None => {}
                    }
                }
            } else {
                held.borrow_mut().insert(text.clone());
                let _ = command_tx.send(Input::Key { text, pressed: true });
            }
        } else if held.borrow_mut().remove(&text) {
            let _ = command_tx.send(Input::Key { text, pressed: false });
        }
    });

    window.run()
}

#[cfg(test)]
mod tests {
    use super::{Hotkey, hotkey, is_modifier_key};

    fn hotkey_char(text: &str) -> Option<&'static str> {
        hotkey(text).map(|h| match h {
            Hotkey::Pause => "pause",
            Hotkey::Reset => "reset",
            Hotkey::Mute => "mute",
            Hotkey::Fullscreen => "fullscreen",
            Hotkey::Quit => "quit",
        })
    }

    #[test]
    fn test_hotkey_letters() {
        assert_eq!(hotkey_char("p"), Some("pause"));
        assert_eq!(hotkey_char("P"), Some("pause"), "capitals must still bind");
        assert_eq!(hotkey_char("r"), Some("reset"));
        assert_eq!(hotkey_char("m"), Some("mute"));
        assert_eq!(hotkey_char("f"), Some("fullscreen"));
        assert_eq!(hotkey_char("q"), Some("quit"));
    }

    #[test]
    fn test_hotkey_ignores_other_keys() {
        assert_eq!(hotkey_char("x"), None);
        assert_eq!(hotkey_char("a"), None);
        assert_eq!(hotkey_char(""), None);
        assert_eq!(hotkey_char("Enter"), None);
    }

    #[test]
    fn test_modifier_keys_are_not_commands() {
        for text in ["\u{10}", "\u{11}", "\u{12}", "\u{13}", "\u{15}", "\u{16}", "\u{17}", "\u{18}"] {
            assert!(is_modifier_key(text.chars().next().unwrap()), "{}", text.escape_debug());
            assert_eq!(hotkey_char(text), None, "{}", text.escape_debug());
        }
    }
}
