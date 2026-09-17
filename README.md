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

### Sound

The AY-3-8912 PSG is emulated — three tone channels, the noise generator and
the envelope shapes — using the same timing model as Caprice32 (1MHz PSG
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