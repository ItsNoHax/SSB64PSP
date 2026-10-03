//! RDP's 14-bit compressed Z in a 16-bit memory word; low bits store dZ.

/// Expand the exponent/mantissa into the RDP's 18-bit unsigned depth.
/// Hardware format reference: paraLLEl-RDP `shaders/z_encode.h` (RE-440).
pub fn decode(word: u16) -> u32 {
    let code = u32::from(word >> 2);
    let exponent = code >> 11;
    let mantissa = code & 0x7ff;
    let shift = 6_u32.saturating_sub(exponent);
    (0x40000 - (0x40000 >> exponent)) + (mantissa << shift)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn depth_endpoints_exponents_and_dz_bits() {
        for (word, expanded) in [
            (0x0000, 0),
            (0x1ffc, 0x1ffc0),
            (0x2000, 0x20000),
            (0x4000, 0x30000),
            (0xc000, 0x3f000),
            (0xe000, 0x3f800),
            (0xfffc, 0x3ffff),
        ] {
            for dz in 0..4 {
                assert_eq!(decode(word | dz), expanded);
            }
        }
    }
}
