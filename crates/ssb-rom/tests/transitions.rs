//! RE-464: the results wipes (`dLBTransitionDescs`) sample the whole
//! 300 x 220 transition photo, and each plays until `lbTransitionProcUpdate`
//! sees its `GObj::anim_frame` at or below 0.

use ssb_rom::mobj::LB_TRANSITION_PHOTO;
use ssb_rom::pack::{Pack, TextureDesc, VERTEX_SIZE};
use ssb_rom::skeleton::StageAnimator;
use ssb_rom::transition::ASSETS;

fn pack_bytes() -> Option<Vec<u8>> {
    std::env::var_os("SSB64_ROM")?;
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/generated/ssb64.pak");
    std::fs::read(path).ok()
}

/// Every photo-sampling vertex's `(s, t)` in texels, for wipe `id`.
fn photo_texels(pack: &Pack<'_>, id: u32) -> Vec<(f32, f32)> {
    let anim = pack.transition_anim(id).expect("wipe packed");
    let object = pack.transition_object(&anim).expect("wipe object");
    let mut out = Vec::new();
    for n in 0..object.node_count {
        let node = pack.node(object.first_node + n).unwrap();
        let Some(mesh) = pack.mesh(node.mesh) else {
            continue;
        };
        let vertices = pack.vertices(&mesh).unwrap();
        for j in 0..mesh.prim_count {
            let prim = pack.prim(mesh.first_prim + j).unwrap();
            let Some(t) = pack.texture(prim.texture) else {
                continue;
            };
            if t.role != TextureDesc::ROLE_FRAMEBUFFER {
                continue;
            }
            for i in pack.indices(&prim).unwrap().as_chunks::<2>().0 {
                let at = usize::from(u16::from_le_bytes(*i)) * VERTEX_SIZE;
                let s = i16::from_le_bytes([vertices[at], vertices[at + 1]]);
                let t = i16::from_le_bytes([vertices[at + 2], vertices[at + 3]]);
                out.push((f32::from(s) / 32.0, f32::from(t) / 32.0));
            }
        }
    }
    out
}

#[test]
fn the_photo_is_one_full_picture_texture() {
    let Some(bytes) = pack_bytes() else {
        return;
    };
    let pack = Pack::open(&bytes).unwrap();
    let photos: Vec<TextureDesc> = (0..pack.texture_count())
        .filter_map(|i| pack.texture(i))
        .filter(|t| t.role == TextureDesc::ROLE_FRAMEBUFFER)
        .collect();
    assert_eq!(photos.len(), 1, "one photo, whichever strip loads it");
    let t = photos[0];
    assert_eq!((t.width, t.height), LB_TRANSITION_PHOTO);
    assert_eq!(t.stride, 512);
}

#[test]
fn every_wipe_addresses_the_whole_photo() {
    let Some(bytes) = pack_bytes() else {
        return;
    };
    let pack = Pack::open(&bytes).unwrap();
    let (w, h) = (
        f32::from(LB_TRANSITION_PHOTO.0),
        f32::from(LB_TRANSITION_PHOTO.1),
    );
    for (id, asset) in ASSETS.iter().enumerate() {
        let texels = photo_texels(&pack, id as u32);
        assert!(!texels.is_empty(), "{}", asset.name);
        let (min_s, max_s, min_t, max_t) = texels.iter().fold(
            (f32::MAX, f32::MIN, f32::MAX, f32::MIN),
            |(a, b, c, d), &(s, t)| (a.min(s), b.max(s), c.min(t), d.max(t)),
        );
        // The strips run from row 0 to the last, 219 or 220 at the edge.
        assert!(
            min_s >= 0.0 && max_s <= w,
            "{} s {min_s}..{max_s}",
            asset.name
        );
        assert!(
            min_t >= 0.0 && max_t <= h,
            "{} t {min_t}..{max_t}",
            asset.name
        );
        assert!(
            min_t < 1.0 && max_t > h - 6.0,
            "{} t {min_t}..{max_t}",
            asset.name
        );
    }
}

/// `lbTransitionMakeTransition`'s `gcAddAnimJointAll(..., 0.0F)` and
/// `gcPlayAnimAll`, then one `lbTransitionProcUpdate` a frame until its
/// `anim_frame <= 0.0F` ejects the wipe: the updates it takes, the last of
/// which ejects it before it draws.
fn updates(pack: &Pack<'_>, id: u32) -> u32 {
    let anim = pack.transition_anim(id).unwrap();
    let script = pack.anim_script(&anim).unwrap();
    let mut a = StageAnimator::new();
    a.start_changed(pack, &anim);
    a.tick(script).unwrap();
    let mut n = 0;
    loop {
        n += 1;
        a.tick(script).unwrap();
        if a.gobj_frame() <= 0.0 {
            return n;
        }
        assert!(n < 1000, "wipe {id} never ends");
    }
}

#[test]
fn every_wipe_ends_on_its_clock() {
    let Some(bytes) = pack_bytes() else {
        return;
    };
    let pack = Pack::open(&bytes).unwrap();
    let measured: Vec<u32> = (0..ASSETS.len() as u32)
        .map(|id| updates(&pack, id))
        .collect();
    // RE-146's lengths, 64 frames (72 for the curtain): the wipe draws on
    // the scene's first 63 (71) frames.
    let expected: Vec<u32> = ASSETS.iter().map(|a| a.frames).collect();
    assert_eq!(measured, expected);
}
