//! Shared formatting for the generated Rust.

use std::fmt::Write;

/// A Rust `f32` literal for `x`: the shortest decimal that reads back as the
/// same `f32`, always with a decimal point (`150.0`, `-0.5`).
pub fn f32_lit(x: f32) -> String {
    let s = format!("{x:?}");
    if s.contains(['.', 'e', 'i', 'N']) {
        s
    } else {
        format!("{s}.0")
    }
}

/// `words` as `0x%08X` rows of eight, each row indented four spaces and
/// ending in a comma.
pub fn hex_rows_u32(out: &mut String, words: &[u32]) {
    for row in words.chunks(8) {
        let cells: Vec<String> = row.iter().map(|w| format!("0x{w:08X}")).collect();
        let _ = writeln!(out, "    {},", cells.join(", "));
    }
}

/// `halves` as `0x%04X` rows of `per_row`.
pub fn hex_rows_u16(out: &mut String, halves: &[u16], per_row: usize) {
    for row in halves.chunks(per_row) {
        let cells: Vec<String> = row.iter().map(|h| format!("0x{h:04X}")).collect();
        let _ = writeln!(out, "    {},", cells.join(", "));
    }
}

/// The generated header every table file starts with.
pub fn header(what: &str) -> String {
    format!(
        "// Generated at build time by crates/ssb-tablegen from the user's ROM:\n\
         // {what}. Not committed; never edit.\n\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn float_literals_round_trip_and_keep_a_point() {
        assert_eq!(f32_lit(150.0), "150.0");
        assert_eq!(f32_lit(-0.5), "-0.5");
        assert_eq!(f32_lit(1.12), "1.12");
        assert_eq!(f32_lit(0.0), "0.0");
    }
}
