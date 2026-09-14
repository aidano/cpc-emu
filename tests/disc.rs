// End-to-end disc boot test: AMSDOS fitted in upper ROM slot 7, the test
// disc in drive 0, then CAT typed at the READY prompt. Requires
// roms/cpc6128.rom, roms/amsdos.rom and dsk/Barbarian (1987)(Palace
// Software)[a2].dsk (all gitignored).
// Run with: cargo test --test disc -- --ignored --nocapture

use cpc_emu::dsk::Dsk;
use cpc_emu::runtime::{Runtime, IRQ_PERIOD};

fn decode_screen_text(mem: &cpc_emu::memory::Memory, os_rom: &[u8]) -> Vec<String> {
    let mut lines = Vec::new();
    for row in 0..25u16 {
        let mut line = String::new();
        for col in 0..40usize {
            let mut glyph = [0u8; 8];
            for sl in 0..8usize {
                let b0 = mem.read(0xC000 + row * 0x50 + (sl as u16) * 0x800 + (col as u16) * 2);
                let b1 = mem.read(0xC000 + row * 0x50 + (sl as u16) * 0x800 + (col as u16) * 2 + 1);
                let mut bits = 0u8;
                for n in 0..8usize {
                    let byte = if n < 4 { b0 } else { b1 };
                    let in_pixel = n % 4;
                    let pen = ((byte >> (7 - in_pixel)) & 1) << 1 | ((byte >> (3 - in_pixel)) & 1);
                    if pen != 0 {
                        bits |= 0x80 >> n;
                    }
                }
                glyph[sl] = bits;
            }
            let mut ch = '?';
            for ascii in 32..127u16 {
                let base = 0x3900 + (ascii - 32) * 8;
                if usize::from(base + 8) <= os_rom.len() && os_rom[base as usize..base as usize + 8] == glyph {
                    ch = ascii as u8 as char;
                    break;
                }
            }
            if glyph == [0u8; 8] {
                ch = ' ';
            }
            line.push(ch);
        }
        lines.push(line.trim_end().to_string());
    }
    lines
}

#[test]
#[ignore]
fn disc_boot_and_cat() {
    let mut runtime = Runtime::default();
    let os_rom = std::fs::read("roms/cpc6128.rom").expect("roms/cpc6128.rom missing");
    runtime.load_rom_from_bytes(&os_rom);
    let amsdos = std::fs::read("roms/amsdos.rom").expect("roms/amsdos.rom missing");
    assert_eq!(amsdos.len(), 0x4000);
    let mut amsdos_rom = [0u8; 0x4000];
    amsdos_rom.copy_from_slice(&amsdos);
    runtime.load_expansion_rom_into_slot(&amsdos_rom, 7);
    let dsk_bytes = std::fs::read("dsk/Barbarian (1987)(Palace Software)[a2].dsk")
        .expect("dsk/Barbarian (1987)(Palace Software)[a2].dsk missing");
    let dsk = Dsk::init_from_bytes(&dsk_bytes).expect("valid DSK");
    runtime.insert_disk(Some(dsk));

    // Boot to the READY prompt.
    for _ in 0..600 {
        runtime.run_for_cycles(IRQ_PERIOD * 6);
    }
    let screen: Vec<String> = decode_screen_text(&runtime.components.mem, &os_rom);
    println!("=== screen after boot ===");
    for line in &screen {
        println!("|{}", line);
    }
    assert!(
        screen.iter().any(|l| l.contains("Ready")),
        "boot with AMSDOS did not reach READY:\n{}",
        screen.join("\n")
    );

    // Type CAT and Enter at the prompt. Keys must be tapped one at a time:
    // holding several keys at once makes the firmware's matrix scan report
    // them in scan-line order, not the press order (real CPC behaviour).
    for ch in ["c", "a", "t"] {
        runtime.components.keyboard.press_text(ch);
        for _ in 0..6 {
            runtime.run_for_cycles(IRQ_PERIOD * 6);
        }
        runtime.components.keyboard.release_text(ch);
        for _ in 0..4 {
            runtime.run_for_cycles(IRQ_PERIOD * 6);
        }
    }
    runtime.components.keyboard.press_text("Enter");
    for _ in 0..10 {
        runtime.run_for_cycles(IRQ_PERIOD * 6);
    }
    runtime.components.keyboard.release_text("Enter");
    // Give AMSDOS time to spin the motor up and read the directory.
    for _ in 0..100 {
        runtime.run_for_cycles(IRQ_PERIOD * 6);
    }
    let screen: Vec<String> = decode_screen_text(&runtime.components.mem, &os_rom);
    println!("=== screen after CAT ===");
    for line in &screen {
        println!("|{}", line);
    }
    println!(
        "final PC: {:04X} iff1: {} halted: {}",
        runtime.components.registers.pc.get(),
        runtime.components.registers.iff1,
        runtime.components.registers.halted
    );
    let joined = screen.join("\n");
    // AMSDOS's CAS redirection makes CAT read drive A. It lists the first
    // directory entry; the rest of the catalog awaits the frame-fly event
    // machinery (the [BE5F] motor-wait spins because the registered event is
    // not yet dispatched - see README known issues).
    assert!(
        joined.contains("Drive A"),
        "CAT did not reach the drive A catalog:\n{}",
        joined
    );
}
