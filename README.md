# Amstrad CPC Emulator (Rust)

Work in progress!

## Running

Boot a CPC 6128:

```
cargo run -- --rom roms/cpc6128.rom
```

`cpc6128.rom` is the OS + BASIC 1.1 pair. AMSDOS — the disc filing system —
is a *third, separate* ROM on the real machine (upper ROM socket 7, the
internal disc interface), so the emulator auto-fits an `amsdos.rom` image
found next to the main ROM into slot 7 when the main ROM is a 664/6128-class
image ((c) 1985 OS). The machine then boots disc-enabled like the hardware:
`CAT` reads drive A instead of the tape catalogue.

Options:

- `--no-amsdos` — boot without the automatic fit (a tape-only machine).
- `-e/--exprom <path>[:slot]` — fit another expansion ROM (default slot 7);
  this replaces the auto-fitted AMSDOS.
- `--dsk <file>.dsk` — insert a disc image into drive 0.

- `--display slint` (window, default) or `--display terminal`
  (with `--terminal-scale full|half|braille`).
- `--mute` — disable audio output.
- `--no-joystick` — disable USB gamepad support entirely.
- `--no-joy-keys` — keep the joystick ports but stop the gamepad from
  driving the guest keyboard.

### Sound

The AY-3-8912 PSG is emulated — three tone channels, the noise generator and
the envelope shapes — using the chip's timing model (1MHz PSG
clock, tone = clock/(16·period)). Samples are mixed to the default audio
output via `cpal`: WASAPI on Windows, CoreAudio on macOS, ALSA on Linux, at
the device's sample rate. On machines without an audio device the emulator
logs a warning and runs silent. The `audio` feature (on by default) can be
dropped with `--no-default-features` for a soundless build.

Audio production is tied to executed machine time, rendered in ~5ms slices
inside the emulation loop: the audio timeline always matches the machine's,
and PSG register writes land in the audio within one slice. The emulation
itself runs in a dedicated thread — paced by a drift-correcting 50Hz
schedule, feeding the display through a latest-wins mailbox and the audio
stream through a lock-free ring — so window/UI work can never stall sound
production. If the host genuinely can't keep 50Hz the emulated machine —
and with it music tempo — slows coherently; build with `--release`.
`RUST_LOG=debug` metering shows the render, machine and stream rates.

### Joysticks

USB gamepads drive the CPC's joystick ports through the `joystick` feature
(on by default), polled cross-platform with `gilrs`: XInput/DirectInput on
Windows, evdev/libudev on Linux, IOKit HID on macOS. The
CPC wires its joystick ports into the keyboard matrix — joystick 0 owns
matrix line 9, joystick 1 shares line 6 with keys — so the gamepad state is
applied to the same matrix the PSG scan reads, and games see it like real
hardware. With no gamepad the emulator logs the detection result and runs
normally; on Linux the `libudev` system library must be present.

Mapping (per pad): D-pad or left stick are the directions, the bottom face
button (A/✕) is fire 1, the right face button (B/○) is fire 2, and Start is
the CPC's Return. The first two pads connected map to players 1 and 2; a
disconnect re-slots the rest. With keyboard mirroring on (default), the
directions also press the guest's cursor keys, the face buttons press
Space, and Start presses Return — so keyboard-only games work from the pad
too; `--no-joy-keys` restricts the pad to the joystick ports alone.

Mirroring is applied from the polled pad state on every matrix read
(alongside the joystick ports), so it cannot leave a stuck key behind,
and a reset re-applies it within one frame. The terminal display accepts
pad input the same way.

### Logging

`RUST_LOG=debug` is safe to run games with: the emulator's debug output is
event-driven — guest key events (`key '\n' pressed=true`), joystick mask
changes (`player 1 mask 00 -> 21 [fire1]`), gamepad connect/disconnect and
once-per-second audio metering — enough to pinpoint a pad whose idle state
drifts into held input or a host key whose event text never maps. Scope it
to the emulator's own input modules to silence third-party noise:

```
RUST_LOG=cpc_emu::joystick=debug,cpc_emu::host=debug
```

A once-per-second `keyboard reads/s` line shows what the guest polled from
the matrix — which lines, and any output-mode reg 14 reads — so a game whose
input code stops polling (or never polls line 7 while you press `3`) is
visible directly, independent of whether keys are pressed.

The per-instruction Z80 trace (machine code, assembly, registers — millions
of lines a second, throttling the machine) lives at `trace` level and is
opt-in only:

```
RUST_LOG=cpc_emu::runtime=trace
```

## Tests

```
cargo test                                              # unit tests
cargo test --test boot -- --ignored --nocapture         # full boot to READY (needs roms/)
```

## Known issues

- Register writes take effect at the next ~5ms audio slice boundary: music
  and firmware beeps are correct, but digidrum-style PCM tricks that
  rewrite registers mid-frame are not cycle-accurate yet.
- `CAT` reaches the drive-A catalog (the disc-redirect install, the FDC
  dialogue and the sector transfer all work), but the catalog truncates after
  the first entry: AMSDOS's motor spin-up wait keys off a frame-fly event
  (`C9D6`) that isn't dispatched yet, because the RAM jumpblock/RST-08h event
  table is still half-initialised when the ROM walk installs AMSDOS. The port
  of the frame-fly event machinery is the next investigation.