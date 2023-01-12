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
    let high = ((value & 0xFF00).wrapping_shr(8)) as u8;
    let low = (value & 0x00FF) as u8;
    (high, low)
}


#[cfg(test)]
mod tests {
    use super::split_double_byte;
    
    #[test]
    fn test_split_double_byte() {
        let (high, low) = split_double_byte(0xABEF);
        assert!(high == 0xAB);
        assert!(low == 0xEF);
    }

}