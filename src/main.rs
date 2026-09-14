#![allow(dead_code, unused)]

use cpc_emu::dsk::Dsk;
use cpc_emu::runtime::*;
use cpc_emu::video;
use cpc_emu::video::VideoSink;

use std::io;
use std::io::Read;
use std::io::BufReader;
use std::fs::File;
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

    /// Display backend: "slint" (window) or "terminal"
    #[clap(long, default_value = "slint")]
    display: String,

    /// Terminal scaling mode: "full" (1 pixel per cell), "half" (2x1) or "braille" (2x4)
    #[clap(long, default_value = "braille")]
    terminal_scale: String,
}

/// 50 frames per second at 4MHz: the cycles executed per display frame.
const CYCLES_PER_FRAME: u32 = 80_000;

fn run_terminal(mut runtime: Runtime, scale: video::terminal::TerminalScale) {
    let mut sink = video::terminal::TerminalSink::new(scale);
    let frame_duration = Duration::from_millis(20); // 50Hz
    loop {
        let frame_start = std::time::Instant::now();
        runtime.run_for_cycles(CYCLES_PER_FRAME);
        let frame = video::renderer::render(&runtime.components.mem, &runtime.components.gate_array, &runtime.components.crtc);
        sink.present(&frame);
        let elapsed = frame_start.elapsed();
        if elapsed < frame_duration {
            thread::sleep(frame_duration - elapsed);
        }
    }
}

#[cfg(feature = "gui")]
fn run_slint(runtime: Runtime) -> Result<(), slint::PlatformError> {
    video::slint_backend::run(runtime)
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

    match args.display.as_str() {
        "terminal" => {
            let scale = video::terminal::TerminalScale::from_str(&args.terminal_scale)
                .unwrap_or_else(|| {
                    error!("Unknown terminal scale '{}', using braille", args.terminal_scale);
                    video::terminal::TerminalScale::Braille
                });
            run_terminal(runtime, scale);
        }
        #[cfg(feature = "gui")]
        "slint" => {
            run_slint(runtime).map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
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
