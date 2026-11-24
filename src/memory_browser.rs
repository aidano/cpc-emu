// Debug memory browser: the visible window of guest memory is a page of
// fixed rows — each an address, the eight bytes visible from it and their
// readable text. The browser pages forward (wrapping past the end of the
// address space) while the machine is paused, and reads memory through the
// emulation thread, so this module stays pure view logic: no machine state.

use crate::video::Frame;

/// Bytes shown per browser row.
pub const BYTES_PER_ROW: usize = 8;
/// Rows shown per browser page.
pub const ROWS_PER_PAGE: usize = 16;
/// Bytes paged through per step, and the length of each memory read.
pub const PAGE_BYTES: usize = BYTES_PER_ROW * ROWS_PER_PAGE;

/// The range of the guest address space the browser pages through.
pub const PAGE_COUNT: usize = 0x1_0000 / PAGE_BYTES;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PageView {
    /// Address of the first byte on the page.
    pub address: u16,
}

impl Default for PageView {
    fn default() -> PageView {
        PageView::new(0x0000)
    }
}

impl PageView {
    pub fn new(address: u16) -> PageView {
        PageView { address }
    }

    /// Pages forward by one page, wrapping past the end of the address
    /// space back to 0x0000.
    pub fn next_page(&mut self) {
        self.address = self.address.wrapping_add(PAGE_BYTES as u16);
    }

    /// Lays `bytes` (a page read from guest memory, possibly shorter than
    /// a full page if the read failed) into rows starting at the view
    /// address. Only complete rows are produced.
    pub fn rows(&self, bytes: &[u8]) -> Vec<Row> {
        bytes
            .as_chunks::<BYTES_PER_ROW>()
            .0
            .iter()
            .enumerate()
            .map(|(index, chunk)| Row {
                address: self.address.wrapping_add((index * BYTES_PER_ROW) as u16),
                bytes: *chunk,
            })
            .collect()
    }
}

/// One browser row: an address and the bytes visible from it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Row {
    pub address: u16,
    pub bytes: [u8; BYTES_PER_ROW],
}

impl Row {
    /// The row's text column: the readable character for each byte, blank
    /// for bytes that have none.
    pub fn ascii(&self) -> String {
        self.bytes
            .iter()
            .map(|&byte| ascii_char(byte).unwrap_or(' '))
            .collect()
    }
}

/// The readable character for a guest byte, if any: printable ASCII
/// (0x20-0x7E). Everything else — control codes, the CPC's extended
/// glyphs above 0x7F — renders blank.
pub fn ascii_char(byte: u8) -> Option<char> {
    if (0x20..=0x7E).contains(&byte) {
        Some(byte as char)
    } else {
        None
    }
}

/// The CPC OS ROM's character block: 8x8 glyph bitmaps, one byte per
/// scanline, MSB leftmost. The "AMSDOS font" is this block — AMSDOS and the
/// firmware both draw text through it — so it is extracted from the loaded
/// main ROM at runtime rather than shipped as a separate asset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CpcFont {
    /// Glyph bitmaps for the printable codes 0x20-0x7E (index 0 is space).
    glyphs: [[u8; 8]; 95],
}

/// ROM offset of the character block: the glyph for ASCII code `c` lives at
/// 0x3800 + c*8 (the firmware's space glyph, ASCII 32, at 0x3900 — the same
/// offset the boot test matches screen glyphs against).
const FONT_BASE: usize = 0x3800;

impl CpcFont {
    /// Extracts the printable glyphs from a main ROM image (16K OS or 32K
    /// OS+BASIC; the block sits within the first 16K either way). Returns
    /// None when the image is absent or too short to carry the block.
    pub fn from_os_rom(rom: &[u8]) -> Option<CpcFont> {
        let mut glyphs = [[0u8; 8]; 95];
        for (index, glyph) in glyphs.iter_mut().enumerate() {
            let offset = FONT_BASE + (0x20 + index) * 8;
            glyph.copy_from_slice(rom.get(offset..offset + 8)?);
        }
        Some(CpcFont { glyphs })
    }

    /// The glyph bitmap for an ASCII code, or None outside the printable
    /// range (those render blank, like `ascii_char`).
    fn glyph(&self, code: u8) -> Option<[u8; 8]> {
        if (0x20..=0x7E).contains(&code) {
            Some(self.glyphs[(code - 0x20) as usize])
        } else {
            None
        }
    }
}

// Browser page layout in 8x8 glyph cells, mirroring the text view's
// columns: a four-digit address, a two-digit column per byte and an
// eight-character ASCII column, each followed by a one-cell gutter.
const ADDRESS_CELLS: usize = 5;
const BYTE_CELLS: usize = 3;
const ASCII_CELLS: usize = 8;
const PAGE_CELLS_WIDE: usize = ADDRESS_CELLS + BYTES_PER_ROW * BYTE_CELLS + ASCII_CELLS;

/// The text view's palette, carried over to the drawn page.
const PAPER: (u8, u8, u8) = (0x16, 0x18, 0x1D);
const HEADER_INK: (u8, u8, u8) = (0x6A, 0x73, 0x7D);
const ADDRESS_INK: (u8, u8, u8) = (0x79, 0xC0, 0xFF);
const HEX_INK: (u8, u8, u8) = (0xE6, 0xED, 0xF3);
const ASCII_INK: (u8, u8, u8) = (0x7E, 0xE7, 0x87);

fn draw_glyph(
    frame: &mut Frame,
    font: &CpcFont,
    cell_x: usize,
    cell_y: usize,
    ch: char,
    ink: (u8, u8, u8),
) {
    if !ch.is_ascii() {
        return;
    }
    let Some(glyph) = font.glyph(ch as u8) else {
        return;
    };
    for (line, &bits) in glyph.iter().enumerate() {
        for pixel in 0..8u16 {
            if bits & (0x80 >> pixel) != 0 {
                frame.set_pixel(
                    cell_x as u16 * 8 + pixel,
                    cell_y as u16 * 8 + line as u16,
                    ink,
                );
            }
        }
    }
}

fn draw_text(
    frame: &mut Frame,
    font: &CpcFont,
    cell_x: usize,
    cell_y: usize,
    text: &str,
    ink: (u8, u8, u8),
) {
    for (index, ch) in text.chars().enumerate() {
        draw_glyph(frame, font, cell_x + index, cell_y, ch, ink);
    }
}

/// Renders a browser page (a header row plus data rows) as a 1x image: one
/// 8x8 cell per character, the glyphs drawn exactly as the firmware draws
/// mode-2 text. Bytes with no readable character leave their ASCII cell
/// blank, matching `ascii_char`. Scale up with `Frame::scaled_into` before
/// display.
pub fn page_image(font: &CpcFont, rows: &[Row]) -> Frame {
    let mut frame = Frame::new((PAGE_CELLS_WIDE * 8) as u16, ((rows.len() + 1) * 8) as u16);
    for y in 0..frame.height {
        for x in 0..frame.width {
            frame.set_pixel(x, y, PAPER);
        }
    }

    draw_text(&mut frame, font, 0, 0, "ADDR", HEADER_INK);
    for (index, label) in ["+0", "+1", "+2", "+3", "+4", "+5", "+6", "+7"]
        .iter()
        .enumerate()
    {
        draw_text(
            &mut frame,
            font,
            ADDRESS_CELLS + index * BYTE_CELLS,
            0,
            label,
            HEADER_INK,
        );
    }
    draw_text(
        &mut frame,
        font,
        ADDRESS_CELLS + BYTES_PER_ROW * BYTE_CELLS,
        0,
        "ASCII",
        HEADER_INK,
    );

    for (row_index, row) in rows.iter().enumerate() {
        let cell_y = row_index + 1;
        draw_text(
            &mut frame,
            font,
            0,
            cell_y,
            &format!("{:04X}", row.address),
            ADDRESS_INK,
        );
        for (index, &byte) in row.bytes.iter().enumerate() {
            draw_text(
                &mut frame,
                font,
                ADDRESS_CELLS + index * BYTE_CELLS,
                cell_y,
                &format!("{:02X}", byte),
                HEX_INK,
            );
            if let Some(ch) = ascii_char(byte) {
                draw_glyph(
                    &mut frame,
                    font,
                    ADDRESS_CELLS + BYTES_PER_ROW * BYTE_CELLS + index,
                    cell_y,
                    ch,
                    ASCII_INK,
                );
            }
        }
    }
    frame
}

#[cfg(test)]
mod tests {
    use super::{
        ascii_char, page_image, CpcFont, PageView, ADDRESS_CELLS, ADDRESS_INK, ASCII_INK,
        BYTES_PER_ROW, BYTE_CELLS, HEADER_INK, HEX_INK, PAGE_BYTES, PAGE_CELLS_WIDE, PAGE_COUNT,
        PAPER, ROWS_PER_PAGE,
    };

    #[test]
    fn test_page_metrics() {
        assert_eq!(BYTES_PER_ROW, 8);
        assert_eq!(ROWS_PER_PAGE, 16);
        assert_eq!(PAGE_BYTES, 128);
        // 512 pages cover the whole 64K space with no slack.
        assert_eq!(PAGE_COUNT * PAGE_BYTES, 0x1_0000);
    }

    #[test]
    fn test_next_page_advances_by_one_page() {
        let mut view = PageView::new(0x0000);
        view.next_page();
        assert_eq!(view.address, 0x0080);
        view.next_page();
        assert_eq!(view.address, 0x0100);
    }

    #[test]
    fn test_next_page_wraps_past_address_space_end() {
        let mut view = PageView::new(0xFF80);
        view.next_page();
        assert_eq!(view.address, 0x0000);
    }

    #[test]
    fn test_rows_lay_out_addresses_and_bytes() {
        let bytes: Vec<u8> = (0..PAGE_BYTES as u8).collect();
        let rows = PageView::new(0x1234).rows(&bytes);
        assert_eq!(rows.len(), ROWS_PER_PAGE);
        assert_eq!(rows[0].address, 0x1234);
        assert_eq!(rows[1].address, 0x123C);
        assert_eq!(
            rows[0].bytes,
            [0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07]
        );
        assert_eq!(rows[15].address, 0x12AC);
        assert_eq!(rows[15].bytes[7], 0x7F);
    }

    #[test]
    fn test_rows_wrap_addresses_at_the_top_of_memory() {
        let bytes: Vec<u8> = vec![0xFF; PAGE_BYTES];
        let rows = PageView::new(0xFF80).rows(&bytes);
        assert_eq!(rows[0].address, 0xFF80);
        assert_eq!(rows[1].address, 0xFF88);
        // The last row's final byte lands exactly on 0xFFFF.
        assert_eq!(rows[15].address, 0xFFF8);
        assert_eq!(rows[15].bytes[7], 0xFF);
    }

    #[test]
    fn test_rows_skip_incomplete_final_chunk() {
        let rows = PageView::new(0x0000).rows(&[0x41; 20]);
        assert_eq!(rows.len(), 2);
        let rows = PageView::new(0x0000).rows(&[]);
        assert!(rows.is_empty());
    }

    #[test]
    fn test_ascii_printable_and_blank() {
        assert_eq!(ascii_char(0x41), Some('A'));
        assert_eq!(ascii_char(0x7E), Some('~'));
        assert_eq!(ascii_char(0x20), Some(' '));
        assert_eq!(ascii_char(0x1F), None);
        assert_eq!(ascii_char(0x7F), None);
        assert_eq!(ascii_char(0x80), None);
        assert_eq!(ascii_char(0x0D), None);

        // A row mixes readable characters and blanks, positionally.
        let row = super::Row {
            address: 0x1000,
            bytes: [0x41, 0x0D, 0x42, 0x00, 0x43, 0xFF, 0x44, 0x7F],
        };
        assert_eq!(row.ascii(), "A B C D ");
    }

    /// A ROM whose character block is filled with a solid glyph.
    fn solid_font() -> CpcFont {
        CpcFont::from_os_rom(&vec![0xFF; 0x3C00]).unwrap()
    }

    #[test]
    fn test_font_extracts_glyphs_from_os_rom() {
        // A synthetic OS ROM: the block for 'A' carries a distinctive 'A'
        // bitmap, everything else is blank.
        let mut rom = vec![0x00u8; 0x3C00];
        let a = [0x18, 0x3C, 0x66, 0x66, 0x7E, 0x66, 0x66, 0x00];
        rom[0x3800 + 0x41 * 8..0x3800 + 0x41 * 8 + 8].copy_from_slice(&a);
        let font = CpcFont::from_os_rom(&rom).unwrap();
        assert_eq!(font.glyph(b'A'), Some(a));
        assert_eq!(font.glyph(b' '), Some([0; 8]));
        // Control codes and DEL have no glyph: they render blank.
        assert_eq!(font.glyph(0x1F), None);
        assert_eq!(font.glyph(0x7F), None);
    }

    #[test]
    fn test_font_requires_the_full_character_block() {
        assert_eq!(CpcFont::from_os_rom(&[]), None);
        // The block runs from 0x3800 to just past the '}' glyph's last byte.
        assert_eq!(CpcFont::from_os_rom(&vec![0u8; 0x3BF7]), None);
        assert!(CpcFont::from_os_rom(&vec![0u8; 0x3BF8]).is_some());
    }

    #[test]
    fn test_page_image_dimensions_and_inks() {
        let font = solid_font();
        let bytes: Vec<u8> = (0..PAGE_BYTES as u8).collect();
        let rows = PageView::new(0x1234).rows(&bytes);
        let image = page_image(&font, &rows);
        assert_eq!(
            (image.width, image.height),
            (PAGE_CELLS_WIDE as u16 * 8, 17 * 8)
        );
        // Header glyphs in header ink, gutter cells in paper.
        assert_eq!(image.pixel(0, 0), HEADER_INK);
        assert_eq!(image.pixel((ADDRESS_CELLS as u16 - 1) * 8 + 7, 0), PAPER);
        // Row 0: the address in address ink...
        assert_eq!(image.pixel(0, 8), ADDRESS_INK);
        // ...the first hex column in hex ink...
        assert_eq!(image.pixel((ADDRESS_CELLS as u16) * 8, 8), HEX_INK);
        // ...and a blank ASCII column: bytes 0x00-0x07 are not printable.
        assert_eq!(
            image.pixel((ADDRESS_CELLS + BYTES_PER_ROW * BYTE_CELLS) as u16 * 8, 8),
            PAPER
        );
    }

    #[test]
    fn test_page_image_draws_ascii_and_respects_glyph_bits() {
        let mut rom = vec![0x00u8; 0x3C00];
        let a = [0x18, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        rom[0x3800 + 0x41 * 8..0x3800 + 0x41 * 8 + 8].copy_from_slice(&a);
        let font = CpcFont::from_os_rom(&rom).unwrap();

        let rows = PageView::new(0x0000).rows(&vec![0x41; PAGE_BYTES]);
        let image = page_image(&font, &rows);
        let ascii_x = (ADDRESS_CELLS + BYTES_PER_ROW * BYTE_CELLS) as u16 * 8;
        // Glyph bit 0x18 sets pixels 3 and 4 of the glyph's top line (and
        // every other line is blank), in the first ASCII cell of row 0.
        assert_eq!(image.pixel(ascii_x, 8), PAPER);
        assert_eq!(image.pixel(ascii_x + 3, 8), ASCII_INK);
        assert_eq!(image.pixel(ascii_x + 4, 8), ASCII_INK);
        assert_eq!(image.pixel(ascii_x + 5, 8), PAPER);
        assert_eq!(image.pixel(ascii_x, 9), PAPER);
    }
}
