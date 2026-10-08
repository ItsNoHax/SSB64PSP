//! RE-471: the magnifier's depth mask, sampled per texel, gives the spans
//! sampling every pixel gives, at every scale and position a battle can
//! show it.

use ssb_rom::depth_mask::{self, Placement, Span};
use ssb_rom::pack::Pack;
use ssb_rom::player_interface as asset;

fn pack_bytes() -> Option<Vec<u8>> {
    std::env::var_os("SSB64_ROM")?;
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/generated/ssb64.pak");
    std::fs::read(path).ok()
}

fn collect(f: impl FnOnce(&mut dyn FnMut(i32, &[Span]))) -> Vec<(i32, Span)> {
    let mut out = Vec::new();
    f(&mut |py, spans: &[Span]| out.extend(spans.iter().map(|&s| (py, s))));
    out
}

#[test]
fn magnifier_mask_spans_match_sampling_every_pixel() {
    let Some(bytes) = pack_bytes() else {
        eprintln!("skipping: SSB64_ROM unset or pack not built");
        return;
    };
    let pack = Pack::open(&bytes).unwrap();
    let a = pack.effect_anim(asset::ARROWS_SLOT + 1).unwrap();
    let image = asset::frame(pack.anim_script(&a).unwrap()).unwrap().image;
    let tables = depth_mask::Tables::new(&image);
    // `draw_magnifiers`: a 32-pixel mask times the magnify scale, centred
    // anywhere in the 10..310 x 10..230 viewport, through
    // `ssb_engine::coord`'s mapping onto the 480 x 272 screen.
    let k = ssb_engine::coord::SCREEN_SCALE;
    let (mut fast_t, mut slow_t, mut cases) = (0u128, 0u128, 0);
    for step in 0..=40 {
        let scale = 0.5 + step as f32 * 0.0937;
        for (cx, cy) in [
            (26.0f32, 26.0f32),
            (160.0, 120.0),
            (293.7, 213.3),
            (11.0, 229.0),
            (100.25, 40.75),
        ] {
            let (x, y) = (cx - 16.0 * scale, cy - 16.0 * scale);
            let (w, h) = (32.0 * scale, 32.0 * scale);
            let x0 = ssb_engine::coord::n64_to_psp_x(x);
            let y0 = ssb_engine::coord::n64_to_psp_y(y);
            let p = Placement {
                x0,
                y0,
                x1: x0 + w * k,
                y1: y0 + h * k,
                k,
                width: w,
                height: h,
            };
            let t = std::time::Instant::now();
            let fast = collect(|e| depth_mask::rows(&image, &tables, &p, e));
            fast_t += t.elapsed().as_nanos();
            let t = std::time::Instant::now();
            let slow = collect(|e| depth_mask::rows_per_pixel(&image, &p, e));
            slow_t += t.elapsed().as_nanos();
            assert!(!slow.is_empty());
            assert_eq!(fast, slow, "{p:?}");
            cases += 1;
        }
    }
    eprintln!(
        "{cases} placements: per texel {} us, per pixel {} us",
        fast_t / 1000,
        slow_t / 1000
    );
}
