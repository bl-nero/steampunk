pub fn as_single_hex_digit(n: u8) -> char {
    if n <= 0x0f {
        format!("{:X}", n)
            .chars()
            .last()
            .expect("Hex formatting error")
    } else {
        '?'
    }
}
