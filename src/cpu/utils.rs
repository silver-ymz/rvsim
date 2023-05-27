pub fn is_fp(num: u8) -> bool {
    num & 0b00100000 != 0
}