use super::*;

/// The levels are angrylion's `((k << 6) | 0x20)` through
/// `special_9bit_exttable` (`(i & 0x180) == 0x180` sign-extends).
#[test]
fn levels_are_the_rdp_9bit_noise_values() {
    for (k, &level) in LEVELS.iter().enumerate() {
        let raw = ((k as i32) << 6) | 0x20;
        let ext = if raw & 0x180 == 0x180 {
            raw - 0x200
        } else {
            raw
        };
        assert_eq!(level, ext);
    }
}

/// The shadows' near-black texels under `PRIM` 0x30: six grey levels and
/// two clamped to black, mean 27.
#[test]
fn a_black_texel_under_share_0x30() {
    let out: Vec<u8> = LEVELS.iter().map(|&n| combine(0, n, 0x30)).collect();
    assert_eq!(out, [6, 18, 30, 42, 54, 66, 0, 0]);
    assert_eq!(out.iter().map(|&v| u32::from(v)).sum::<u32>() / 8, 27);
}

/// Share 0 keeps the texel; a white texel clamps at 255 for high levels.
#[test]
fn share_zero_keeps_the_texel_and_white_clamps() {
    for &n in &LEVELS {
        assert_eq!(combine(0x7B, n, 0), 0x7B);
    }
    assert_eq!(combine(255, 352, 0x30), 255);
    assert_eq!(combine(255, -96, 0x30), 189);
}

/// Alpha passes through; R, G and B share one level; every level shows up.
#[test]
fn texel_keeps_alpha_and_greys_the_noise() {
    let mut rng = NoiseRng::new(1);
    let mut seen = [false; 6];
    for _ in 0..256 {
        let [r, g, b, a] =
            texel_rgba8888(u32::from_le_bytes([0, 0, 0, 0xC8]), 0x30, &mut rng).to_le_bytes();
        assert_eq!((r, a), (g, 0xC8));
        assert_eq!(g, b);
        if let Some(i) = [6, 18, 30, 42, 54, 66].iter().position(|&v| v == r) {
            seen[i] = true;
        }
    }
    assert!(seen.iter().all(|&s| s));
}

/// The table gives what the equation gives, draw for draw.
#[test]
fn lut_matches_the_equation() {
    let lut = Lut::new(0x30);
    assert_eq!(lut.share(), 0x30);
    let (mut a, mut b) = (NoiseRng::new(7), NoiseRng::new(7));
    for i in 0..4096u32 {
        let word = i.wrapping_mul(0x9E37_79B9);
        assert_eq!(
            lut.texel_rgba8888(word, &mut a),
            texel_rgba8888(word, 0x30, &mut b)
        );
    }
}
