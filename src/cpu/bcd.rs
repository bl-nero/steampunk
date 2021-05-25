/// Performs a BCD addition with carry, returning result and carry.
pub fn bcd_add(a: u8, b: u8, carry: bool) -> (u8, bool) {
    // Note that there is a fancy algorithm that doesn't use branches, but it
    // proved to be not much better in benchmarks (perhaps because we only add
    // two digits), so we go with a more readable and straightforward one.

    // Less significant digit
    let mut result: u16 = ((a as u16) & 0x0F) + ((b as u16) & 0x0F) + if carry { 1 } else { 0 };
    if result > 0x09 {
        result += 0x06;
    }
    // More significant digit
    result += ((a as u16) & 0xF0) + ((b as u16) & 0xF0);
    return if result > 0x99 {
        ((result + 0x60) as u8, true)
    } else {
        (result as u8, false)
    };
}

/// Performs a BCD subtraction with borrow, returning result and borrow.
pub fn bcd_sub(a: u8, b: u8, borrow: bool) -> (u8, bool) {
    // See comment in `bcd_add`.
    // Less significant digit
    let mut result: i16 =
        ((a as u16 as i16) & 0x0F) - ((b as u16 as i16) & 0x0F) - if borrow { 1 } else { 0 };
    if result < 0 {
        result -= 0x06;
    }
    // More significant digit
    result += ((a as u16 as i16) & 0xF0) - ((b as u16 as i16) & 0xF0);
    return if result < 0 {
        ((result - 0x60) as u8, true)
    } else {
        (result as u8, false)
    };
}

#[cfg(test)]
mod tests {
    extern crate test;

    use super::*;
    use test::Bencher;

    #[test]
    fn adding() {
        assert_eq!(bcd_add(0, 0, false), (0, false));
        assert_eq!(bcd_add(2, 2, false), (4, false));
        assert_eq!(bcd_add(3, 4, true), (8, false));
        assert_eq!(bcd_add(0x07, 0x09, false), (0x16, false));
        assert_eq!(bcd_add(0x07, 0x02, true), (0x10, false));
        assert_eq!(bcd_add(0x12, 0x46, false), (0x58, false));
        assert_eq!(bcd_add(0x54, 0x28, false), (0x82, false));
        assert_eq!(bcd_add(0x78, 0x61, false), (0x39, true));
        assert_eq!(bcd_add(0x67, 0x86, false), (0x53, true));
        assert_eq!(bcd_add(0x99, 0x99, true), (0x99, true));
    }

    #[test]
    fn subtracting() {
        assert_eq!(bcd_sub(0, 0, false), (0, false));
        assert_eq!(bcd_sub(5, 2, false), (3, false));
        assert_eq!(bcd_sub(9, 3, true), (5, false));
        assert_eq!(bcd_sub(0x57, 0x08, false), (0x49, false));
        assert_eq!(bcd_sub(0x12, 0x02, true), (0x09, false));
        assert_eq!(bcd_sub(0x75, 0x41, false), (0x34, false));
        assert_eq!(bcd_sub(0x54, 0x26, false), (0x28, false));
        assert_eq!(bcd_sub(0x27, 0x82, false), (0x45, true));
        assert_eq!(bcd_sub(0x13, 0x97, false), (0x16, true));
        assert_eq!(bcd_sub(0x42, 0x84, true), (0x57, true));
    }

    #[bench]
    fn benchmark(b: &mut Bencher) {
        b.iter(|| {
            let mut a = 0u8;
            for i in 0x00..=test::black_box(0xFF) {
                for j in 0x00..=test::black_box(0xFF) {
                    a |= bcd_add(i, j, false).0;
                    a |= bcd_add(i, j, true).0;
                    a |= bcd_sub(i, j, false).0;
                    a |= bcd_sub(i, j, true).0;
                }
            }
            return a;
        });
    }
}
