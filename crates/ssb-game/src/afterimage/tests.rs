use super::*;
use crate::fighter::{AfterImage, AfterImageDesc};

/// `tools/r4300/afterimage_hashes.py`'s input generator.
struct Lcg(u32);

impl Lcg {
    fn next(&mut self, n: u32) -> u32 {
        self.0 = (self.0.wrapping_mul(1_103_515_245).wrapping_add(12345)) & 0x7fff_ffff;
        (self.0 >> 8) % n
    }
    fn range(&mut self, lo: i32, hi: i32) -> i32 {
        lo + self.next((hi - lo + 1) as u32) as i32
    }
}

fn case_inputs(k: u32) -> AfterImage {
    let mut g = Lcg(k + 1);
    let mut desc = [AfterImageDesc::default(); 3];
    for d in &mut desc {
        let t = [0; 3].map(|_| g.range(-3000, 3000) as i16);
        let v = [0; 3].map(|_| g.range(-1000, 1000) as f32 / 1000.0);
        *d = AfterImageDesc {
            translate: t,
            vec: Vec3::new(v[0], v[1], v[2]),
        };
    }
    match k % 10 {
        0 => desc[1].vec = desc[0].vec,
        1 => desc[2].vec = Vec3::new(-desc[1].vec.x, -desc[1].vec.y, -desc[1].vec.z),
        2 => desc[1].vec = Vec3::new(desc[0].vec.x, desc[0].vec.y + 0.01, desc[0].vec.z),
        _ => {}
    }
    AfterImage {
        is_itemswing: k & 1 != 0,
        drawstatus: 2 + ((k >> 1) & 1) as i8,
        desc_id: ((k >> 2) % 3) as u8,
        desc,
    }
}

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x100_0000_01b3;

fn fnv(mut h: u64, word: u32) -> u64 {
    for b in word.to_le_bytes() {
        h = (h ^ u64::from(b)).wrapping_mul(FNV_PRIME);
    }
    h
}

/// The vertex output of the ROM's `ftDisplayMainDrawAfterImage` over 1,200
/// swept trails (both variants, drawstatus 2 and 3, every `desc_id`,
/// parallel, opposite and under-30-degree neighbours), as the r4300
/// interpreter runs it (`tools/r4300/afterimage_hashes.py`).
#[test]
fn the_trail_vertices_match_the_rom() {
    let mut h = FNV_OFFSET;
    let mut out = [TrailVtx::default(); TRAIL_VTX_MAX];
    let mut total = 0;
    for k in 0..1200 {
        let a = case_inputs(k);
        let n = trail_vertices(&a, &mut out);
        h = fnv(h, n as u32);
        for v in &out[..n] {
            let [x, y, z] = v.pos.map(|c| u32::from(c as u16));
            h = fnv(h, (x << 16) | y);
            h = fnv(h, z << 16);
            h = fnv(h, 0);
            h = fnv(h, u32::from_be_bytes(v.rgba));
        }
        total += n;
    }
    assert_eq!(total, 14238);
    assert_eq!(h, 0x67c0_84cf_3477_b6cf);
}

#[test]
fn the_rotate_constant_is_the_roms() {
    // `F_CLC_DTOR32(30.0F)` rounds to the f32 nearest pi/6.
    assert_eq!(TRAIL_ROTATE, core::f32::consts::FRAC_PI_6);
}

#[test]
fn the_strip_covers_each_pair_step_twice() {
    let t: Vec<_> = trail_triangles(6).collect();
    assert_eq!(t, vec![[0, 1, 2], [1, 3, 2], [2, 3, 4], [3, 5, 4]]);
}

/// RE-512 trace s1 (Link Jab1, status 190, set on frame 452 and again on
/// 503): `drawstatus` reaches 1 on the motion's fourth frame, 2 on its
/// fifth (the first frame the N64 draws the trail; a frame at 1 draws
/// nothing) and 3 on its sixth.
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn link_jab_trail_draws_from_the_frame_drawstatus_reaches_two() {
    use crate::collision::{flags, Segment};
    use crate::fighter::{Fighter, FighterKind};
    let floor = || {
        [(
            0u16,
            Segment {
                x1: -2318,
                y1: 0,
                x2: 2318,
                y2: 0,
                flags: flags::CLIFF,
            },
        )]
    };
    let mut f = Fighter::new(FighterKind::Link, 0, 3);
    f.pos = Vec3::new(0.0, 4.0, 0.0);
    assert!(f.place_on_stage(floor()));
    crate::status::set_wait(&mut f);
    crate::status::set_attack11(&mut f);
    let mut seen = std::vec::Vec::new();
    for frame in 1..=12 {
        f.tick(floor);
        crate::combat::proc_params(&mut f);
        seen.push((frame, f.afterimage.drawstatus, trail_visible(&f.afterimage)));
    }
    let first = |ds: i8| seen.iter().find(|s| s.1 == ds).map(|s| s.0);
    assert_eq!(
        (first(1), first(2), first(3)),
        (Some(4), Some(5), Some(6)),
        "{seen:?}"
    );
    assert!(seen.iter().all(|&(_, ds, visible)| visible == (ds >= 2)));
    // Drawn 457..467 around the hit's 5 hitlag frames (458..462): motion
    // frames 5 to 10.
    let drawn: std::vec::Vec<_> = seen.iter().filter(|s| s.2).map(|s| s.0).collect();
    assert_eq!(drawn, [5, 6, 7, 8, 9, 10], "{seen:?}");
}
