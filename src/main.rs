#![allow(dead_code, unused)]

use cpc_emu::dsk::Dsk;
use cpc_emu::runtime::*;
use cpc_emu::video;
use cpc_emu::video::VideoSink;
use cpc_emu::audio;

use std::io;
use std::io::Read;
use std::io::BufReader;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::thread;
use std::time::Duration;

use clap::Parser;
use log::{debug, error};


#[derive(Parser,Debug)]
#[clap(name = "CPC Emu", version = "0.1.0", author = "aidano", about = "Amstrad CPC Emulator")]
struct Arguments {
    /// ROM file to use
    #[clap(short, long)]
    rom: String,

    /// DSK file to launch (optional)
    #[clap(short, long)]
    dsk: Option<String>,

    /// Additional expansion ROM(s) to fit: path, optionally with an upper
    /// ROM slot: "roms/amsdos.rom:7" (default slot 7, the 6128's internal
    /// disc interface convention). Repeatable.
    #[clap(short = 'e', long = "exprom")]
    exproms: Vec<String>,

    /// Boot without the automatic AMSDOS fit (a tape-only disc machine, or
    /// when fitting a disc ROM explicitly with -e)
    #[clap(long)]
    no_amsdos: bool,

    /// Display backend: "slint" (window) or "terminal"
    #[clap(long, default_value = "slint")]
    display: String,

    /// Terminal scaling mode: "full" (1 pixel per cell), "half" (2x1) or "braille" (2x4)
    #[clap(long, default_value = "braille")]
    terminal_scale: String,

    /// Disable audio output
    #[clap(long)]
    mute: bool,
}

/// The 664/6128 OS ROMs are (c) 1985; the 464's is (c) 1984 and that machine
/// has no internal disc interface. Used to decide whether a loaded OS+BASIC
/// pair is a disc-machine image that deserves the automatic AMSDOS fit.
const DISC_MACHINE_OS_MARKER: &[u8] = b"1985 Amstrad";

/// Returns the AMSDOS image to auto-fit when the main ROM is a 664/6128-class
/// OS+BASIC pair (32K whose OS half carries the (c) 1985 marker) and an
/// `amsdos.rom` image sits in the same directory: the real machines carry
/// AMSDOS as a third, separate ROM in upper ROM socket 7.
fn bundled_amsdos(main_rom: &[u8], main_rom_path: &str) -> Option<(Vec<u8>, PathBuf)> {
    if main_rom.len() != 0x8000 {
        return None;
    }
    if !main_rom[..0x4000]
        .windows(DISC_MACHINE_OS_MARKER.len())
        .any(|window| window == DISC_MACHINE_OS_MARKER)
    {
        return None;
    }
    let path = Path::new(main_rom_path)
        .parent()
        .unwrap_or_else(|| Path::new(""))
        .join("amsdos.rom");
    let bytes = std::fs::read(&path).ok()?;
    if bytes.len() != 0x4000 {
        error!(
            "{}: expected a 16K AMSDOS image, got {} bytes; not auto-fitting",
            path.display(),
            bytes.len()
        );
        return None;
    }
    Some((bytes, path))
}

fn run_terminal(
    runtime: Runtime,
    scale: video::terminal::TerminalScale,
    _audio: Option<audio::AudioOutput>,
) {
    // The emulation runs in its own thread; this loop only presents frames.
    let mailbox = Arc::new(cpc_emu::host::FrameMailbox::new(640, 400));
    // Terminal mode has no keyboard input path: the sender is dropped and
    // the emulation thread just sees a closed channel.
    let (_tx, rx) = mpsc::channel::<cpc_emu::host::Input>();
    let _emu = cpc_emu::host::spawn_emu_thread(
        runtime,
        Arc::clone(&mailbox),
        rx,
        Arc::new(AtomicBool::new(false)),
    );
    let mut sink = video::terminal::TerminalSink::new(scale);
    let mut display = video::Frame::new(640, 400);
    loop {
        if mailbox.read_latest(&mut display) {
            sink.present(&display);
        }
        thread::sleep(Duration::from_millis(20));
    }
}

#[cfg(feature = "gui")]
fn run_slint(
    runtime: Runtime,
    audio_out: Option<audio::AudioOutput>,
    mute: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
) -> Result<(), slint::PlatformError> {
    video::slint_backend::run(runtime, audio_out, mute, paused)
}

/// Starts the live audio output stream. The stream stays open even when
/// muted (so Ctrl+M can unmute into a live device); the callback emits
/// silence while the mute or pause flag is set. A missing device logs a
/// warning and the emulator runs silent.
fn setup_audio(
    runtime: &mut Runtime,
    mute: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
) -> Option<audio::AudioOutput> {
    let (producer, consumer) = audio::audio_ring();
    match audio::start(consumer, Arc::clone(&mute), Arc::clone(&paused)) {
        Some((output, rate)) => {
            runtime.components.psg.sample_rate = rate;
            runtime.audio = Some(producer);
            Some(output)
        }
        None => None,
    }
}

fn main() -> io::Result<()> {
    env_logger::init();
    let args = Arguments::parse();

    let mut disc: Option<Dsk> = None;
    if let Some(dsk_file_name) = &args.dsk {
        let file_name: &str = dsk_file_name.trim();

        debug!("loading dsk: {} ...", file_name);

        let f = File::open(file_name)?;
        let mut reader = BufReader::new(f);
        let mut buffer = Vec::new();

        match (reader.read_to_end(&mut buffer)) {
            Ok(bytes) => {
                debug!("File: read {} bytes\n", bytes);
                match Dsk::init_from_bytes(buffer.as_slice()) {
                    Ok(dsk) => disc = Some(dsk),
                    Err(msg) => error!("Invalid dsk: {}", msg),
                }
            },
            Err(code) =>  {
                error!("Error reading dsk: {:?}", code);
            }
        }
    } else {
        debug!("No DSK provided - booting from ROM only");
    }


    // Try out the runtime

    let mut runtime = Runtime::default();
    runtime.insert_disk(disc);

    // Load the rom
    let rom_file_name: &str = args.rom.trim();

    debug!("loading rom: {} ...", rom_file_name);

    let f = File::open(rom_file_name)?;
    let mut reader = BufReader::new(f);
    let mut buffer = Vec::new();

    match (reader.read_to_end(&mut buffer)) {
        Ok(bytes) => {
            debug!("Read {} bytes\n", bytes);
            let _ = runtime.load_rom_from_bytes(buffer.as_slice());
        },
        Err(code) =>  {
            error!("Error reading rom: {:?}", code);
        }
    }

    // The 6128's AMSDOS is not part of the OS+BASIC pair in the main ROM
    // image: the real machine carries it as a third, separate ROM in upper
    // ROM socket 7. Fit it automatically so a disc machine boots
    // disc-enabled like the hardware; an explicit -e ROM into slot 7 below
    // still overrides, and --no-amsdos skips this entirely.
    if !args.no_amsdos {
        match bundled_amsdos(&buffer, rom_file_name) {
            Some((bytes, path)) => {
                let mut rom = [0u8; 0x4000];
                rom.copy_from_slice(&bytes);
                runtime.load_expansion_rom_into_slot(&rom, 7);
                debug!(
                    "auto-fitted {} into upper ROM slot 7 (the 6128's internal disc interface)",
                    path.display()
                );
            },
            None => debug!("no amsdos.rom found beside {}; booting without AMSDOS", rom_file_name),
        }
    }

    // Fit extra expansion ROMs (e.g. AMSDOS into upper ROM slot 7).
    for exprom in &args.exproms {
        let (path, slot) = match exprom.rsplit_once(':') {
            Some((path, slot_text)) => match slot_text.parse::<u8>() {
                Ok(slot @ 0..=15) => (path, slot),
                _ => (exprom.as_str(), 7),
            },
            None => (exprom.as_str(), 7),
        };
        match File::open(path) {
            Ok(mut rom_file) => {
                let mut rom_bytes = Vec::new();
                rom_file.read_to_end(&mut rom_bytes)?;
                match rom_bytes.len() {
                    0x4000 => {
                        let mut rom = [0u8; 0x4000];
                        rom.copy_from_slice(&rom_bytes);
                        runtime.load_expansion_rom_into_slot(&rom, slot);
                        debug!("fitted expansion rom {} into slot {}", path, slot);
                    },
                    other => error!("{}: expected a 16K ROM image, got {} bytes", path, other),
                }
            },
            Err(code) => error!("Error reading expansion rom {}: {:?}", path, code),
        }
    }

    // Live audio: the output handle must outlive the run loop, so it stays
    // with the caller of the selected backend. The mute and pause flags are
    // shared with the audio callback (silence) and, in GUI mode, the window
    // backend (the Ctrl hotkeys flip them).
    let mute_flag = Arc::new(AtomicBool::new(args.mute));
    let paused_flag = Arc::new(AtomicBool::new(false));
    let audio_out = setup_audio(
        &mut runtime,
        Arc::clone(&mute_flag),
        Arc::clone(&paused_flag),
    );

    match args.display.as_str() {
        "terminal" => {
            let scale = video::terminal::TerminalScale::from_str(&args.terminal_scale)
                .unwrap_or_else(|| {
                    error!("Unknown terminal scale '{}', using braille", args.terminal_scale);
                    video::terminal::TerminalScale::Braille
                });
            run_terminal(runtime, scale, audio_out);
        }
        #[cfg(feature = "gui")]
        "slint" => {
            run_slint(runtime, audio_out, mute_flag, paused_flag)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
        }
        _ => {
            if args.display == "slint" {
                error!("Slint display requires the 'gui' feature: cargo run --features gui");
            } else {
                error!("Unknown display backend '{}'", args.display);
            }
            std::process::exit(2);
        }
    }

    Ok(())
}
