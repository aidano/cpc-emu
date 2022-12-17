pub fn nibbles(byte: u8) -> (u8, u8) {
    let high = (byte & 0xF0) >> 4;
    let low = byte & 0x0F;
    (high, low)
}

pub fn combine_to_double_byte(high_byte: u8, low_byte: u8) -> u16 {
    let high = (high_byte as u16) << 8;
    high | low_byte as u16
}

pub fn split_double_byte(value: u16) -> (u8, u8) {
    let high = ((value & 0xFF00) >> 16) as u8;
    let low = (value & 0x00FF) as u8;
    (high, low)
}