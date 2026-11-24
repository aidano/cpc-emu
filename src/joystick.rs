// Host gamepad plumbing (gilrs). A poller thread keeps a shared, polled
// pressed-bits mask per player; the emulation thread applies it to the
// keyboard matrix at each frame boundary — as the joystick ports' matrix
// entries (joystick 0 = matrix line 9, joystick 1 = line 6) and, when
// mirroring is enabled, as the guest's cursor keys, Space and Return. The
// CPC wires its joystick ports into the same matrix as the keyboard, so
// both paths end in the same 10x8 switches. Without a gamepad the poller
// exits quietly and the masks stay zero (the audio backend's no-device
// pattern). Mask changes log at debug level, so RUST_LOG=debug traces
// exactly what the pad is feeding the guest.

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

pub use crate::devices::keyboard::{
    JOY_DOWN, JOY_FIRE1, JOY_FIRE2, JOY_LEFT, JOY_RIGHT, JOY_START, JOY_UP,
};

/// Per-player joystick state shared between the poller thread and the
/// emulation thread: one pressed-bits mask per player, zero when no pad
/// drives that player.
pub struct JoyInputs {
    masks: [AtomicU8; 2],
}

impl JoyInputs {
    pub fn new() -> JoyInputs {
        JoyInputs { masks: [AtomicU8::new(0), AtomicU8::new(0)] }
    }

    /// Publishes the pressed bits for player 0 or 1.
    pub fn set(&self, index: u8, mask: u8) {
        if (index as usize) < self.masks.len() {
            self.masks[index as usize].store(mask, Ordering::Relaxed);
        }
    }

    /// The player's pressed bits (0 for an unknown or absent player).
    pub fn mask(&self, index: u8) -> u8 {
        self.masks.get(index as usize).map_or(0, |m| m.load(Ordering::Relaxed))
    }
}

/// One gamepad's input snapshot, independent of the host's gamepad API.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct PadSnapshot {
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
    pub fire1: bool,
    pub fire2: bool,
    /// Start: menu/start screens through the guest keyboard (the joystick
    /// port has no such input, so Start only takes part in mirroring).
    pub start: bool,
}

/// Collapses a snapshot into the shared pressed-bits mask. Fire 2 is bit 4
/// and fire 1 bit 5 — the matrix layout, not the buttons' natural order —
/// and Start is bit 6, which only the guest-keyboard mirror consumes.
pub fn mask_from(pad: &PadSnapshot) -> u8 {
    let mut mask = 0;
    if pad.up {
        mask |= JOY_UP;
    }
    if pad.down {
        mask |= JOY_DOWN;
    }
    if pad.left {
        mask |= JOY_LEFT;
    }
    if pad.right {
        mask |= JOY_RIGHT;
    }
    if pad.fire1 {
        mask |= JOY_FIRE1;
    }
    if pad.fire2 {
        mask |= JOY_FIRE2;
    }
    if pad.start {
        mask |= JOY_START;
    }
    mask
}

#[cfg(test)]
mod tests {
    use super::{JoyInputs, PadSnapshot, mask_from};
    use crate::devices::keyboard::{JOY_DOWN, JOY_FIRE1, JOY_FIRE2, JOY_LEFT, JOY_RIGHT, JOY_START, JOY_UP};
    use std::sync::Arc;

    #[test]
    fn test_joy_inputs_per_player_slots() {
        let joy = JoyInputs::new();
        assert_eq!(joy.mask(0), 0);
        joy.set(0, JOY_UP | JOY_FIRE1);
        joy.set(1, JOY_LEFT);
        assert_eq!(joy.mask(0), JOY_UP | JOY_FIRE1);
        assert_eq!(joy.mask(1), JOY_LEFT);
        assert_eq!(joy.mask(2), 0, "unknown players read as no pad");
        joy.set(2, JOY_FIRE1);
        assert_eq!(joy.mask(2), 0);
    }

    #[test]
    fn test_mask_from_snapshot() {
        let mut pad = PadSnapshot::default();
        assert_eq!(mask_from(&pad), 0);
        pad.up = true;
        pad.fire1 = true;
        assert_eq!(mask_from(&pad), JOY_UP | JOY_FIRE1, "up is bit 0, fire 1 bit 5");
        let all = PadSnapshot { up: true, down: true, left: true, right: true, fire1: true, fire2: true, start: true };
        assert_eq!(
            mask_from(&all),
            JOY_UP | JOY_DOWN | JOY_LEFT | JOY_RIGHT | JOY_FIRE1 | JOY_FIRE2 | JOY_START
        );
    }

    /// A dead poller must leave the machine's inputs idle: the guard
    /// zeroes the published masks on any exit path.
    #[cfg(feature = "joystick")]
    #[test]
    fn test_mask_guard_idles_everything_on_drop() {
        use super::MaskGuard;
        let joy = Arc::new(JoyInputs::new());
        joy.set(0, JOY_UP | JOY_FIRE1);
        joy.set(1, JOY_FIRE1);
        {
            let _guard = MaskGuard(Arc::clone(&joy));
        }
        assert_eq!(joy.mask(0), 0, "the guard must zero player 1's mask");
        assert_eq!(joy.mask(1), 0, "the guard must zero player 2's mask");
    }
}

/// Zeroes the published masks when the poller exits, whatever the reason —
/// a dead poller must leave the joystick ports and the mirror idle, never
/// holding a bit the guest can see.
#[cfg(feature = "joystick")]
struct MaskGuard(Arc<JoyInputs>);

#[cfg(feature = "joystick")]
impl Drop for MaskGuard {
    fn drop(&mut self) {
        self.0.set(0, 0);
        self.0.set(1, 0);
    }
}

/// Spawns the gamepad poller thread. It publishes each player's pressed
/// bits into the shared state; the emulation thread applies them to the
/// joystick ports and, with mirroring enabled there, the guest keyboard.
#[cfg(feature = "joystick")]
pub fn spawn_joy_thread(joy: Arc<JoyInputs>) -> std::thread::JoinHandle<()> {
    use std::thread;
    thread::Builder::new()
        .name("cpc-joy".into())
        .spawn(move || poll_gamepads(joy))
        .expect("failed to spawn the joystick poller thread")
}

/// The poller loop: drains gilrs events (hotplug, state updates), then
/// republishes the pressed bits each tick from a fresh enumeration of the
/// connected pads — the first two take the player slots, so a disconnect
/// re-slots survivors forward. No pad id is trusted across ticks: gilrs's
/// `gamepad(id)` panics on ids a backend has dropped, so only this tick's
/// own enumeration is ever dereferenced. Mask changes log at debug level:
/// with RUST_LOG=debug, a pad holding guest input names itself.
#[cfg(feature = "joystick")]
fn poll_gamepads(joy: Arc<JoyInputs>) {
    use std::thread;
    use std::time::Duration;

    use gilrs::GilrsBuilder;

    /// Stick deflection that counts as a direction (the CPC input is
    /// digital; a generous zone keeps a resting stick centred).
    const DEADZONE: f32 = 0.3;
    const POLL_INTERVAL: Duration = Duration::from_millis(4);

    let _guard = MaskGuard(Arc::clone(&joy));

    // No default event filters: gilrs's chain (axis_dpad_to_button,
    // jitter, deadzone) inserts and drops events in front of the cached
    // state we read, and its inserted ButtonChanged events never update
    // the pressed flag — an un-auditable mutation layer. Our own deadzone
    // below and value-based button reads cover everything it did.
    let mut gilrs = match GilrsBuilder::new().with_default_filters(false).build() {
        Ok(gilrs) => gilrs,
        Err(err) => {
            log::warn!("gamepad support unavailable: {err}");
            return;
        }
    };
    let connected = gilrs.gamepads().filter(|(_, pad)| pad.is_connected()).count();
    log::info!("gamepad support ready: {} pad(s) detected", connected);

    // Slot identity: the pad's UUID bytes, stable across reconnects.
    let mut occupants: [Option<[u8; 16]>; 2] = [None, None];
    let mut published: [u8; 2] = [0; 2];

    loop {
        // Drain the event queue so the cached pad state stays current.
        while let Some(_) = gilrs.next_event() {}

        // The first two connected pads take the player slots, in
        // enumeration order; further pads are ignored. Handles are not
        // stored — each slot keeps just its snapshot, identity and name.
        let mut snapshots: [PadSnapshot; 2] = [PadSnapshot::default(); 2];
        let mut now_occupants: [Option<[u8; 16]>; 2] = [None, None];
        let mut names: [Option<String>; 2] = [None, None];
        for (slot, (_id, pad)) in gilrs.gamepads().take(2).enumerate() {
            snapshots[slot] = pad_snapshot(&pad, DEADZONE);
            now_occupants[slot] = Some(pad.uuid());
            names[slot] = Some(pad.name().to_string());
        }

        for slot in 0..2usize {
            if occupants[slot] != now_occupants[slot] {
                match now_occupants[slot] {
                    Some(_) => log::info!(
                        "gamepad '{}' is player {}",
                        names[slot].as_deref().unwrap_or("?"),
                        slot + 1
                    ),
                    None => log::info!("player {} slot empty", slot + 1),
                }
                occupants[slot] = now_occupants[slot];
            }

            let mask = mask_from(&snapshots[slot]);
            if mask != published[slot] {
                log::debug!(
                    "player {} mask {:02X} -> {:02X} [{}]",
                    slot + 1,
                    published[slot],
                    mask,
                    mask_names(mask)
                );
                published[slot] = mask;
            }
            joy.set(slot as u8, mask);
        }

        thread::sleep(POLL_INTERVAL);
    }
}

/// Human-readable mask bits for the debug log.
#[cfg(feature = "joystick")]
fn mask_names(mask: u8) -> String {
    let mut names: Vec<&str> = Vec::new();
    if mask & JOY_UP != 0 {
        names.push("up");
    }
    if mask & JOY_DOWN != 0 {
        names.push("down");
    }
    if mask & JOY_LEFT != 0 {
        names.push("left");
    }
    if mask & JOY_RIGHT != 0 {
        names.push("right");
    }
    if mask & JOY_FIRE2 != 0 {
        names.push("fire2");
    }
    if mask & JOY_FIRE1 != 0 {
        names.push("fire1");
    }
    if mask & JOY_START != 0 {
        names.push("start");
    }
    if names.is_empty() { "none".into() } else { names.join("+") }
}

/// Reads one pad's current state into the host-independent snapshot. The
/// D-pad is taken from its buttons or hat axes (backends vary in which
/// they provide) and OR-combined with the left stick beyond the dead zone;
/// in gilrs, stick and hat Y is negative upwards.
#[cfg(feature = "joystick")]
fn pad_snapshot(pad: &gilrs::Gamepad, deadzone: f32) -> PadSnapshot {
    use gilrs::{Axis, Button};

    let value = |axis: Axis| pad.value(axis);
    let stick_x = value(Axis::LeftStickX);
    let stick_y = value(Axis::LeftStickY);
    let pad_x = value(Axis::DPadX);
    let pad_y = value(Axis::DPadY);
    // Buttons read by VALUE, not by gilrs's is_pressed flag: the flag is a
    // separate field that ButtonChanged events never update, so a release
    // delivered through a value-only path would leave the button latched
    // down at the guest. The value is kept current by every event kind.
    let pressed = |button: Button| {
        pad.button_data(button).map_or(false, |data| data.value() > 0.5)
    };

    PadSnapshot {
        up: pressed(Button::DPadUp) || pad_y < -deadzone || stick_y < -deadzone,
        down: pressed(Button::DPadDown) || pad_y > deadzone || stick_y > deadzone,
        left: pressed(Button::DPadLeft) || pad_x < -deadzone || stick_x < -deadzone,
        right: pressed(Button::DPadRight) || pad_x > deadzone || stick_x > deadzone,
        fire1: pressed(Button::South),
        fire2: pressed(Button::East),
        start: pressed(Button::Start),
    }
}
