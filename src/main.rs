#![allow(dead_code, unused)]

mod memory;
mod screen;
mod instruction_set;
mod runtime;
mod dsk;
mod utils;

use dsk::Dsk;
use runtime::*;

use std::io;
use std::io::Read;
use std::io::BufReader;
use std::fs::File;

use clap::{Parser, Arg, App};
use log::{debug, error, log_enabled, info, Level};


#[derive(Parser,Default,Debug)]
struct Arguments {
    file_name: String
}

fn main() -> io::Result<()> {
    env_logger::init();
    
    let matches = App::new("CPC Emu")
        .version("0.1.0")
        .author("aidano")
        .about("Amstrad CPC Emulator")
        .arg(Arg::with_name("dsk")
                 .short('d')
                 .long("dsk")
                 .takes_value(true)
                 .required(true)
                 .help("DSK file to launch"))
        .arg(Arg::with_name("rom")
        .short('r')
        .long("rom")
        .takes_value(true)
        .required(true)
        .help("ROM file to use"))
        .get_matches();
    
    let file_name: &str = matches.get_one::<String>("dsk").unwrap().trim();

    debug!("loading file: {} ...", file_name);

    let f = File::open(file_name)?;
    let mut reader = BufReader::new(f);
    let mut buffer = Vec::new();
    
    match (reader.read_to_end(&mut buffer)) {
        Ok(bytes) => {
            debug!("File: read {} bytes\n", bytes);
            let _ = dbg!(Dsk::init_from_bytes(buffer.as_slice()));
        },
        Err(code) =>  {
            error!("Error reading dsk: {:?}", code);
        }
    }


    // Try out the runtime

    let mut runtime = Runtime::default();

    // Load the rom
    let rom_file_name: &str = matches.get_one::<String>("rom").unwrap().trim();

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
            error!("Error reading dsk: {:?}", code);
        }
    }

    debug!("Running from #0000...");
    runtime.run(0x0);

    Ok(())

}