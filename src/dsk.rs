/*
 The "Disc Information block" is always at offset 0 in the disk image file. 
 If track data exists, then this will immediatly follow the Disc Information Block and 
 will start at offset &100 in the disc image file.
*/

use std::fmt;
use log::{debug};

#[derive(Debug, Copy, Clone)]
enum DskType {
    NORMAL,
    EXTENDED
}

#[derive(Debug)]
pub struct Dsk {
    dsk_type: DskType,
    dsk_info: DiscInformationBlock,
    tracks: Vec<Track>
}

#[derive(Debug)]
struct DiscInformationBlock {
    dsk_type: DskType,
    creator: String, // 22-2f	name of creator	14
    track_count: u8, // 30	number of tracks	1
    side_count: u8, // 31	number of sides	1
    track_size: u32 // 32-33	size of a track (little endian; low byte followed by high byte)	2. Includes the &100 byte Track Information Block.
}

//#[derive(Debug)]
struct Track {
    track_info: TrackInformationBlock,
    sector_infos: Vec<SectorInfo>,
    sector_data: Vec<u8>
}

#[derive(Debug)]
struct TrackInformationBlock {
    track_number: u8, // 10	track number	1
    side_number: u8, // 11	side number	1
    sector_size: u8, // 14	sector size	1 (note: this is not the value in bytes..has to be looked up). 
    sector_count: u8, // 15	number of sectors	1
    gap_3_length: u8,  // 16	GAP#3 length	1
}

#[derive(Debug)]
struct SectorInfo {
    track_number: u8, // 00	track (equivalent to C parameter in NEC765 commands)	1
    side_number: u8, // 01	side (equivalent to H parameter in NEC765 commands)	1
    sector_id: u8, // 02	sector ID (equivalent to R parameter in NEC765 commands)	1
    sector_size: u8, // 03	sector size (equivalent to N parameter in NEC765 commands)	1. Same value for all sectors in a given track.
    fdc_status_register_1: u8, // 04	FDC status register 1 (equivalent to NEC765 ST1 status register)	1
    fdc_status_register_2: u8, // 05	FDC status register 2 (equivalent to NEC765 ST2 status register)	1
}

impl Dsk {
    pub fn init_from_bytes(bytes: &[u8]) -> Result<Dsk, &str> {
        let res = match DiscInformationBlock::from_bytes(bytes)  {
            Ok(dib) => {
                let mut dsk = Dsk { dsk_type: dib.dsk_type, dsk_info: dib, tracks: Vec::new() };
                // The first Track Block is located at offset &100 in the disk image file. 
                dsk.tracks = dsk.dsk_info.load_tracks(&bytes[0x100..]);
                Ok(dsk)
            },
            Err(msg) => Err(msg)
        };
        res
    }
}


const TYPE_NORMAL_PREAMBLE: &str =   &"MV - CPCEMU"; // 00-21	"MV - CPCEMU Disk-File\r\nDisk-Info\r\n"	34
const TYPE_EXTENDED_PREAMBLE: &str = &"EXTENDED CP"; //  00-21 "EXTENDED CPC DSK File\r\nDisk-Info\r\n"   34

impl DiscInformationBlock {
    fn from_bytes(bytes: &[u8]) -> Result<DiscInformationBlock, &str> {

        // Check the header preamble and ensure it matches one of the two expected headers
        //   "MV - CPCEMU Disk-File\r\nDisk-Info\r\n"
        //   "EXTENDED CPC DSK File\r\nDisk-Info\r\n"
        let dsk_type: Option<DskType> = match std::str::from_utf8(&bytes[0..0xB]).unwrap() {
            TYPE_NORMAL_PREAMBLE => Some(DskType::NORMAL),
            TYPE_EXTENDED_PREAMBLE => Some(DskType::EXTENDED),
            _ => None
        };

        let creator = std::str::from_utf8(&bytes[0x22..0x2f]).unwrap();
        let track_count = bytes[0x30];
        let side_count = bytes[0x31];
        let track_size = match u32::from_le_bytes([bytes[0x32], bytes[0x33], 0, 0]) {
            0 => 4864, // Sometimes DSKs have this set to 0, so let's try defaulting in that case.
            anything_but_zero => anything_but_zero
        };

        match dsk_type {
            Some(dsk_type) => Ok(DiscInformationBlock { dsk_type: dsk_type, creator: creator.to_string(), track_count: track_count, side_count: side_count, track_size: track_size }),
            None => Err("Invalid Dsk format")
        }
    }

    fn load_tracks(&mut self, bytes: &[u8]) -> Vec<Track> {
        let mut tracks: Vec<Track> = Vec::new();
        for x in 0..self.track_count {
            let track_start: u32 = x as u32 * self.track_size;
            let track_end = track_start + self.track_size - 1;
            match Track::init_from_bytes(&bytes[track_start as usize..track_end as usize], self.track_size) {
                Ok(track) => tracks.push(track),
                Err(msg) => { dbg!(msg);() }
            }
        }
        tracks
    }

}



impl Track {
    fn init_from_bytes(bytes: &[u8], track_size: u32) -> Result<Track, &str> {
        
        let track_info = TrackInformationBlock::init_from_bytes(bytes);
        let mut sector_infos: Vec<SectorInfo> = Vec::new();
        for x in 0..track_info.sector_count {
            let sector_info_size = 8;
            let start_index = 0x18 + (x * sector_info_size) as usize;
            let end_index = start_index + sector_info_size as usize;
            sector_infos.push(SectorInfo::init_from_bytes(&track_info, &bytes[start_index..end_index]));
        } 
        let sector_data = bytes[0x100..].to_vec(); 
        
        Ok(
            Track {
                track_info: track_info, 
                sector_infos: sector_infos,
                sector_data
            }
        )
    }
}

impl fmt::Debug for Track {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Track {} sector data {} bytes", self.track_info.track_number, self.sector_data.len() )
    }
}



impl TrackInformationBlock {
    fn init_from_bytes(bytes: &[u8]) -> TrackInformationBlock {
        //debugPrintBytes(bytes, 0x200);        
        TrackInformationBlock {
            track_number: bytes[0x10],
            side_number: bytes[0x11],
            sector_size: bytes[0x14],
            sector_count: bytes[0x15],
            gap_3_length: bytes[0x16]
        }
    }
}

fn debug_print_bytes(bytes: &[u8], max: u32 ) {
    let mut idx: u32 = 0;
    for b in bytes {
        debug!("{:02X?} {} {:?}", idx, b, char::from_u32(*b as u32).unwrap().to_string());
        idx += 1;
        if idx > max { break; }
    }
}


impl SectorInfo {
    fn init_from_bytes(track_info_block: &TrackInformationBlock, bytes: &[u8]) -> SectorInfo {
        SectorInfo { 
            track_number: bytes[0x0], 
            side_number: bytes[0x1], 
            sector_id: bytes[0x2],
            sector_size: bytes[0x3],
            fdc_status_register_1: bytes[0x4],
            fdc_status_register_2: bytes[0x5]
        }
    }  
}