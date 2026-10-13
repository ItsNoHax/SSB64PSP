//! How many direct-colour textures would fit a CLUT losslessly.
//! cargo run -p ssb-rom --example texture_palette_fit -- assets/generated/ssb64.pak [offset...]
use ssb_rom::pack::Pack;
use std::collections::HashSet;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let path = args.next().ok_or("expected pack path")?;
    let want: Vec<u32> = args.map(|a| u32::from_str_radix(&a, 16).unwrap()).collect();
    let bytes = std::fs::read(path)?;
    let pack = Pack::open(&bytes).map_err(|e| format!("{e:?}"))?;
    // psm, bucket -> (count, bytes)
    let mut sum = std::collections::BTreeMap::<(u8, &str), (u32, u64)>::new();
    let mut seen = HashSet::new();
    for i in 0..pack.texture_count() {
        let Some(t) = pack.texture(i) else { continue };
        if t.psm > 3 || !seen.insert(t.data_offset) {
            continue;
        }
        let Some(data) = pack.texture_data(&t) else {
            continue;
        };
        let colours: HashSet<u32> = if t.psm == 3 {
            data.as_chunks::<4>()
                .0
                .iter()
                .map(|&c| u32::from_le_bytes(c))
                .collect()
        } else {
            data.as_chunks::<2>()
                .0
                .iter()
                .map(|&c| u32::from(u16::from_le_bytes(c)))
                .collect()
        };
        let n = colours.len();
        let bucket = if n <= 16 {
            "<=16"
        } else if n <= 256 {
            "<=256"
        } else {
            ">256"
        };
        let e = sum.entry((t.psm, bucket)).or_default();
        e.0 += 1;
        e.1 += u64::from(t.data_len);
        if want.contains(&t.data_offset) {
            println!(
                "{:08x} {}x{} psm {} levels {} bytes {} colours {}",
                t.data_offset, t.width, t.height, t.psm, t.levels, t.data_len, n
            );
        }
    }
    for ((psm, b), (c, by)) in sum {
        println!("psm {psm} {b:>6}: {c:5} textures {by:9} bytes");
    }
    Ok(())
}
