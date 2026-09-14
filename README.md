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
- `--dsk <file>.dsk` — insert a disc image into drive 0:

  ```
  cargo run -- --rom roms/cpc6128.rom --dsk 'dsk/Barbarian (1987)(Palace Software)[a2].dsk'
  ```

- `--display slint` (window, default) or `--display terminal`
  (with `--terminal-scale full|half|braille`).

## Tests

```
cargo test                                              # unit tests
cargo test --test boot -- --ignored --nocapture         # full boot to READY (needs roms/)
cargo test --test disc -- --ignored --nocapture         # AMSDOS + CAT a disc (needs roms/ and dsk/)
```

## Known issues

- `CAT` reaches the drive-A catalog (the disc-redirect install, the FDC
  dialogue and the sector transfer all work), but the catalog truncates after
  the first entry: AMSDOS's motor spin-up wait keys off a frame-fly event
  (`C9D6`) that isn't dispatched yet, because the RAM jumpblock/RST-08h event
  table is still half-initialised when the ROM walk installs AMSDOS. The port
  of the frame-fly event machinery is the next investigation; until then the
  disc test asserts the working part (boot to READY with AMSDOS, and CAT
  reaching drive A) and documents the truncation.