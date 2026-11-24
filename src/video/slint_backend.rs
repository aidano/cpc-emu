// Slint window backend: presents frames produced by the dedicated emulation
// thread through a latest-wins mailbox; keyboard events are forwarded to the
// emulation thread over a channel, with host hotkeys (Ctrl-modified keys)
// intercepted here so they never reach the guest. The UI thread never
// touches the machine, so window and presentation work cannot interfere
// with emulation pacing or audio production. While the machine is paused
// (Ctrl+P) a second window — the debug memory browser — shows pages of
// guest memory read on demand through the input channel.

use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;

use i_slint_backend_winit::winit::window::Fullscreen;
use i_slint_backend_winit::WinitWindowAccessor;
use slint::{ComponentHandle, Image, ModelRc, SharedPixelBuffer, Timer, TimerMode, VecModel, Weak};

use crate::audio;
use crate::host::{self, FrameMailbox, Input};
use crate::joystick::JoyInputs;
use crate::memory_browser::{page_image, CpcFont, PageView, PAGE_BYTES};
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

/// Requests one page of guest memory through the emulation thread and loads
/// it into the browser's row model. The emulation thread services input at
/// its next frame boundary (at most ~20ms away, paused or not); a timeout
/// or lost reply leaves the previous page in place.
///
/// With a font (extracted from the OS ROM) the page is additionally drawn
/// as a pixel image using the firmware's own glyphs, and the browser window
/// is sized to it; without one the browser falls back to its text rows.
fn refresh_memory_browser(
    browser: &MemoryBrowser,
    command_tx: &mpsc::Sender<Input>,
    view_addr: u16,
    font: Option<&CpcFont>,
) {
    let (reply_tx, reply_rx) = mpsc::channel::<Vec<u8>>();
    let _ = command_tx.send(Input::ReadMemory {
        address: view_addr,
        len: PAGE_BYTES,
        reply: reply_tx,
    });
    let bytes = reply_rx
        .recv_timeout(Duration::from_millis(500))
        .unwrap_or_default();
    let page_rows = PageView::new(view_addr).rows(&bytes);
    let rows: Vec<MemoryRow> = page_rows
        .iter()
        .map(|row| MemoryRow {
            address: format!("{:04X}", row.address).into(),
            b0: format!("{:02X}", row.bytes[0]).into(),
            b1: format!("{:02X}", row.bytes[1]).into(),
            b2: format!("{:02X}", row.bytes[2]).into(),
            b3: format!("{:02X}", row.bytes[3]).into(),
            b4: format!("{:02X}", row.bytes[4]).into(),
            b5: format!("{:02X}", row.bytes[5]).into(),
            b6: format!("{:02X}", row.bytes[6]).into(),
            b7: format!("{:02X}", row.bytes[7]).into(),
            ascii: row.ascii().into(),
        })
        .collect();
    browser.set_rows(ModelRc::new(VecModel::from(rows)));
    match font {
        Some(font) => {
            // Draw at 1x with the ROM glyphs, then double for display: the
            // same presentation scale as the main window's frame.
            let native = page_image(font, &page_rows);
            let mut scaled = Frame::new(1, 1);
            native.scaled_into(2, 2, &mut scaled);
            let buffer = SharedPixelBuffer::clone_from_slice(
                &scaled.rgba,
                scaled.width as u32,
                scaled.height as u32,
            );
            browser.set_page_image(Image::from_rgba8(buffer));
            browser.set_has_page_image(true);
            // The window keeps the .slint layout's 8px padding around the
            // image; only the font branch resizes, and always to the same
            // footprint, so repeated paging is a no-op here.
            browser.window().set_size(slint::LogicalSize::new(
                f32::from(scaled.width) + 16.0,
                f32::from(scaled.height) + 16.0,
            ));
        }
        None => browser.set_has_page_image(false),
    }
}

/// A host command dispatcher: maps Ctrl-modified key text to a command.
/// Shared by the main window and the memory browser, whichever is focused.
type HostCommandDispatch = Rc<dyn Fn(&str, bool)>;

pub fn run(
    runtime: Runtime,
    _audio: Option<audio::AudioOutput>,
    mute: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
    font: Option<CpcFont>,
    joy: Option<Arc<JoyInputs>>,
    joy_keys: bool,
) -> Result<(), slint::PlatformError> {
    let mailbox = Arc::new(FrameMailbox::new(640, 400));
    let (input_tx, input_rx) = mpsc::channel::<Input>();
    // The emulation thread owns the machine from here on; the UI thread
    // only presents frames and forwards input.
    let _emu = host::spawn_emu_thread(
        runtime,
        Arc::clone(&mailbox),
        input_rx,
        Arc::clone(&paused),
        joy.as_ref().map(Arc::clone),
        joy_keys,
    );

    // The gamepad poller publishes the shared joystick masks; the
    // emulation thread applies them to the joystick ports and, when
    // mirroring is enabled, the guest keyboard.
    #[cfg(feature = "joystick")]
    let _joy_thread =
        joy.as_ref().map(|joy| crate::joystick::spawn_joy_thread(Arc::clone(joy)));

    let window = CpcWindow::new()?;

    // The debug memory browser: a second window, hidden until a Ctrl+P
    // pause shows it. Its visible page address survives pause/unpause so
    // re-pausing reopens on the same page. When the main ROM carried the
    // firmware's character block, its pages draw with those glyphs.
    let browser = MemoryBrowser::new()?;
    let browser_weak = browser.as_weak();
    let browser_view: Rc<RefCell<PageView>> = Rc::new(RefCell::new(PageView::default()));
    let font = font.map(Rc::new);

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
    //
    // The dispatch is shared with the memory browser window, so host
    // commands keep working while it is focused (Ctrl+P to resume, Ctrl+Q
    // to quit, ...): the browser reports Ctrl-modified keys through its
    // `command` callback and never forwards anything to the guest.
    let super_is_meta = cfg!(target_os = "macos");
    let fullscreen_state = Arc::new(AtomicBool::new(false));
    let held: Rc<RefCell<HashSet<String>>> = Rc::new(RefCell::new(HashSet::new()));
    let command_tx = input_tx.clone();
    let hotkey_mute = Arc::clone(&mute);
    let hotkey_paused = Arc::clone(&paused);
    let hotkey_fullscreen = Arc::clone(&fullscreen_state);
    let hotkey_window = window.as_weak();
    let dispatch_tx = command_tx.clone();
    let dispatch_browser_weak = browser_weak.clone();
    let dispatch_browser_view = Rc::clone(&browser_view);
    let dispatch_browser_tx = command_tx.clone();
    let dispatch_font = font.clone();
    let quit_browser_weak = browser_weak.clone();
    let quit_window = window.as_weak();
    let dispatch_hotkey: HostCommandDispatch = Rc::new(move |text, super_key| {
        if !super_key {
            return;
        }
        let Some(command) = hotkey(text) else { return };
        match command {
            Hotkey::Pause => {
                let now_paused = !hotkey_paused.load(Ordering::Relaxed);
                hotkey_paused.store(now_paused, Ordering::Relaxed);
                if let Some(browser) = dispatch_browser_weak.upgrade() {
                    if now_paused {
                        // The browser opens on the paused machine's state,
                        // read through the emulation thread.
                        let view_addr = dispatch_browser_view.borrow().address;
                        refresh_memory_browser(
                            &browser,
                            &dispatch_browser_tx,
                            view_addr,
                            dispatch_font.as_deref(),
                        );
                        let _ = browser.show();
                    } else {
                        let _ = browser.hide();
                    }
                }
            }
            Hotkey::Reset => {
                let _ = dispatch_tx.send(Input::Reset);
            }
            Hotkey::Mute => hotkey_mute
                .store(!hotkey_mute.load(Ordering::Relaxed), Ordering::Relaxed),
            Hotkey::Fullscreen => {
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
            Hotkey::Quit => {
                // Hiding the last visible window ends the event loop, so
                // run() returns and the process unwinds cleanly (emu thread
                // joined, audio closed). The browser window is open exactly
                // while paused, so it must go first.
                if let Some(browser) = quit_browser_weak.upgrade() {
                    let _ = browser.hide();
                }
                if let Some(window) = quit_window.upgrade() {
                    let _ = window.hide();
                }
            }
        }
    });
    let onkey_tx = command_tx.clone();
    let onkey_dispatch = Rc::clone(&dispatch_hotkey);
    window.on_key(move |text, pressed, control, meta, repeat| {
        let text = text.to_string();
        let super_key = if super_is_meta { meta } else { control };
        if pressed != 0 {
            if super_key {
                held.borrow_mut().remove(&text);
                if !repeat {
                    onkey_dispatch(&text, super_key);
                }
            } else {
                held.borrow_mut().insert(text.clone());
                let _ = onkey_tx.send(Input::Key { text, pressed: true });
            }
        } else if held.borrow_mut().remove(&text) {
            let _ = onkey_tx.send(Input::Key { text, pressed: false });
        }
    });

    // Space in the browser pages forward through guest memory.
    let page_weak = browser_weak.clone();
    let page_view = Rc::clone(&browser_view);
    let page_tx = command_tx.clone();
    let page_font = font.clone();
    browser.on_page_next(move || {
        let Some(browser) = page_weak.upgrade() else { return };
        let view_addr = {
            let mut view = page_view.borrow_mut();
            view.next_page();
            view.address
        };
        refresh_memory_browser(&browser, &page_tx, view_addr, page_font.as_deref());
    });

    browser.on_command({
        let dispatch = Rc::clone(&dispatch_hotkey);
        move |text, control, meta, repeat| {
            if repeat {
                return;
            }
            let super_key = if super_is_meta { meta } else { control };
            dispatch(text.as_str(), super_key);
        }
    });

    window.run()
}

#[cfg(test)]
mod tests {
    use super::{CpcWindow, MemoryBrowser, Hotkey, hotkey, is_modifier_key};
    use slint::ComponentHandle;
    use std::cell::RefCell;
    use std::rc::Rc;

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

    /// Guest keys must reach the window's `key` callback: the root window
    /// forwards focus to the key-capturing FocusScope, whose key-pressed
    /// handler invokes the callback. Without the focus forward, Slint drops
    /// every key event before it reaches any item (nothing is focused), so
    /// the guest would never receive input. Space is the start key in most
    /// games, hence its explicit case.
    #[test]
    fn test_guest_keys_reach_the_key_callback() {
        i_slint_backend_testing::init_no_event_loop();
        let window = CpcWindow::new().unwrap();
        let events: Rc<RefCell<Vec<(String, i32)>>> = Rc::new(RefCell::new(Vec::new()));
        let sink = events.clone();
        window.on_key(move |text, pressed, _control, _meta, _repeat| {
            sink.borrow_mut().push((text.to_string(), pressed));
        });

        let dispatch = |text: &str, pressed: bool| {
            let event = if pressed {
                slint::platform::WindowEvent::KeyPressed { text: text.into() }
            } else {
                slint::platform::WindowEvent::KeyReleased { text: text.into() }
            };
            window.window().dispatch_event(event);
        };
        dispatch("a", true);
        dispatch("a", false);
        dispatch(" ", true);

        let got = events.borrow();
        assert!(
            got.contains(&("a".to_string(), 1)),
            "pressed 'a' must reach the key callback, got {got:?}"
        );
        assert!(
            got.contains(&("a".to_string(), 0)),
            "released 'a' must reach the key callback, got {got:?}"
        );
        assert!(
            got.contains(&(" ".to_string(), 1)),
            "space must reach the key callback (game start key), got {got:?}"
        );
        drop(got);

        // The memory browser window needs its own focus forward: Space must
        // page and Ctrl-modified keys must reach its command callback.
        let browser = MemoryBrowser::new().unwrap();
        let pages: Rc<RefCell<u32>> = Rc::new(RefCell::new(0));
        let commands: Rc<RefCell<Vec<(String, bool, bool)>>> = Rc::new(RefCell::new(Vec::new()));
        let page_sink = pages.clone();
        browser.on_page_next(move || *page_sink.borrow_mut() += 1);
        let command_sink = commands.clone();
        browser.on_command(move |text, control, meta, _repeat| {
            command_sink.borrow_mut().push((text.to_string(), control, meta));
        });

        let dispatch_browser = |text: &str, pressed: bool| {
            let event = if pressed {
                slint::platform::WindowEvent::KeyPressed { text: text.into() }
            } else {
                slint::platform::WindowEvent::KeyReleased { text: text.into() }
            };
            browser.window().dispatch_event(event);
        };
        dispatch_browser(" ", true);
        dispatch_browser("p", true);

        assert_eq!(*pages.borrow(), 1, "space in the browser must page forward");
        assert_eq!(
            commands.borrow().len(),
            0,
            "a plain key must not be reported as a host command"
        );
        // Hold Ctrl, then press 'p': the browser must report both the
        // modifier and 'p' as host commands (so Ctrl+P resumes and Ctrl+Q
        // quits from this window). The dispatcher filters the modifier
        // characters out later, like `hotkey()` does for the main window.
        let ctrl_text: slint::SharedString = slint::platform::Key::Control.into();
        dispatch_browser(ctrl_text.as_str(), true);
        dispatch_browser("p", true);
        dispatch_browser("p", false);
        dispatch_browser(ctrl_text.as_str(), false);
        assert_eq!(
            commands.borrow().as_slice(),
            [(ctrl_text.to_string(), true, false), ("p".to_string(), true, false)],
            "Ctrl-modified 'p' must reach the browser's command callback"
        );
    }

    /// The Return key's event text, as Slint's winit backend delivers it
    /// (its key table maps winit's NamedKey::Enter to "\n"; backends that
    /// fall back to the raw event text deliver "\r"), must reach the key
    /// callback — several games wait for Return on their start screens.
    #[test]
    fn test_return_key_texts_reach_the_key_callback() {
        i_slint_backend_testing::init_no_event_loop();
        let window = CpcWindow::new().unwrap();
        let events: Rc<RefCell<Vec<(String, i32)>>> = Rc::new(RefCell::new(Vec::new()));
        let sink = events.clone();
        window.on_key(move |text, pressed, _control, _meta, _repeat| {
            sink.borrow_mut().push((text.to_string(), pressed));
        });

        let dispatch = |text: &str, pressed: bool| {
            let event = if pressed {
                slint::platform::WindowEvent::KeyPressed { text: text.into() }
            } else {
                slint::platform::WindowEvent::KeyReleased { text: text.into() }
            };
            window.window().dispatch_event(event);
        };
        dispatch("\n", true);
        dispatch("\n", false);
        dispatch("\r", true);

        let got = events.borrow();
        assert!(
            got.contains(&("\n".to_string(), 1)),
            "return press must reach the key callback, got {got:?}"
        );
        assert!(
            got.contains(&("\n".to_string(), 0)),
            "return release must reach the key callback, got {got:?}"
        );
        assert!(
            got.contains(&("\r".to_string(), 1)),
            "the \\r return variant must reach the key callback too, got {got:?}"
        );
    }
}
