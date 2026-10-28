//! RE-572 option C: fighter figatrees streamed through
//! [`ssb_rom::stream_pool`] slabs behind the residency registry, as
//! `psp-runtime`'s `scene_files` streams them, give every fighter status
//! the same script bytes and the same sampled joint poses as the whole
//! pack. The registry is process-global: this binary holds one test.

use std::collections::HashMap;
use std::sync::Mutex;

use ssb_rom::pack::Pack;
use ssb_rom::residency::{self, Span};
use ssb_rom::skeleton::Skeleton;
use ssb_rom::stream_pool::{Entry, StreamPool};

/// `scene_files::GROUP_BYTES`, `SECTOR`.
const GROUP_BYTES: u32 = 64 * 1024;
const SECTOR: usize = 512;
/// `nFTKindEnumCount`.
const FIGHTER_KINDS: u32 = 27;

struct Shell {
    full: &'static [u8],
    /// File-table index → group.
    group_of: HashMap<u32, u8>,
    groups: HashMap<u8, (StreamPool, Box<[u8]>)>,
    misses: u32,
    evicted: u32,
    frame_misses: u32,
    max_frame: u32,
}

static SHELL: Mutex<Option<Shell>> = Mutex::new(None);

fn pack_bytes() -> Option<Vec<u8>> {
    ssb_tablegen::test_rom(env!("CARGO_MANIFEST_DIR"))?;
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/generated/ssb64.pak");
    std::fs::read(path).ok()
}

/// `scene_files::stream_miss`, reading from the whole pack's bytes.
fn on_miss(offset: u32, _len: u32) -> bool {
    let mut guard = SHELL.lock().unwrap();
    let s = guard.as_mut().unwrap();
    let pack = Pack::open(s.full).unwrap();
    let Some(i) = pack.file_at_offset(offset) else {
        return false;
    };
    let Some(&group) = s.group_of.get(&i) else {
        return false;
    };
    let file = pack.file_at(i).unwrap();
    let at = pack.blob_start() + file.blob_start as usize;
    let pad = at % SECTOR;
    let now = residency::epoch();
    let (pool, slab) = s.groups.entry(group).or_insert_with(|| {
        (
            StreamPool::new(GROUP_BYTES),
            vec![0u8; GROUP_BYTES as usize].into(),
        )
    });
    let entry = match pool.get(i) {
        Some(e) => e,
        None => {
            let mut evicted: Vec<Entry> = Vec::new();
            let e = pool
                .place(
                    i,
                    (pad + file.len as usize) as u32,
                    now,
                    |e| {
                        let start = pack.file_at(e.key).unwrap().blob_start;
                        residency::last_used(start).unwrap_or(now.wrapping_sub(1))
                    },
                    &mut evicted,
                )
                .expect("a figatree fits its group's slab");
            for x in &evicted {
                unsafe { residency::remove(pack.file_at(x.key).unwrap().blob_start) };
            }
            s.evicted += evicted.len() as u32;
            // Evicted bytes are poisoned, so a stale slice would show.
            for x in &evicted {
                slab[x.at as usize..(x.at + x.len) as usize].fill(0xA5);
            }
            let dst = &mut slab[e.at as usize..(e.at + e.len) as usize];
            dst.copy_from_slice(&s.full[at - pad..at + file.len as usize]);
            s.misses += 1;
            s.frame_misses += 1;
            e
        }
    };
    unsafe {
        residency::insert(Span {
            start: file.blob_start,
            len: file.len,
            ptr: slab.as_ptr().add(entry.at as usize + pad),
        })
    };
    true
}

/// The frame's end: `scene_files::stream_safe_point`.
fn end_frame() {
    let mut guard = SHELL.lock().unwrap();
    let s = guard.as_mut().unwrap();
    s.max_frame = s.max_frame.max(s.frame_misses);
    s.frame_misses = 0;
    drop(guard);
    residency::advance();
}

#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn streamed_figatrees_play_like_resident_ones() {
    let Some(bytes) = pack_bytes() else {
        eprintln!("skipping: SSB64_ROM unset or pack not built");
        return;
    };
    let full: &'static [u8] = Box::leak(bytes.into_boxed_slice());
    let whole = Pack::open(full).unwrap();
    let resident_bytes: &'static [u8] = &full[..whole.resident_len()];
    let streamed = Pack::open(resident_bytes).unwrap();

    // Every fighter's figatrees, grouped as `scene_load::stream_scene`
    // groups them (by the kind whose set it is; the variants' rows name
    // their model's files).
    let mut group_of = HashMap::new();
    let mut anims = Vec::new();
    for i in 0..whole.anim_count() {
        let a = whole.anim(i).unwrap();
        if a.fighter >= FIGHTER_KINDS || a.script_len == 0 {
            continue;
        }
        let f = whole.file_index(a.source_file).unwrap();
        let group = ssb_rom::scene_roots::anim_kind(a.fighter) as u8;
        group_of.entry(f).or_insert(group);
        anims.push(a);
    }
    *SHELL.lock().unwrap() = Some(Shell {
        full,
        group_of,
        groups: HashMap::new(),
        misses: 0,
        evicted: 0,
        frame_misses: 0,
        max_frame: 0,
    });
    unsafe { residency::install(resident_bytes.as_ptr(), Some(on_miss)) };

    // Bytes: every animation's script, read through the slabs, is the
    // whole pack's.
    for a in &anims {
        assert_eq!(
            streamed.anim_script(a),
            whole.anim_script(a),
            "fighter {} slot {}",
            a.fighter,
            a.slot
        );
        end_frame();
    }

    // Poses: every status's clip played 12 frames through the slabs, with
    // the kind's last clip before it played beside it (two clips in use a
    // frame), matches the whole pack's, bit for bit.
    let ticks = 12;
    let mut compared = 0usize;
    for (n, a) in anims.iter().enumerate() {
        let other = anims[..n].iter().rev().find(|b| b.fighter == a.fighter);
        let mut s_main = Skeleton::new();
        let mut w_main = Skeleton::new();
        s_main.start(&streamed, a, 0.0, 1.0);
        w_main.start(&whole, a, 0.0, 1.0);
        let mut s_other = Skeleton::new();
        let mut w_other = Skeleton::new();
        if let Some(b) = other {
            s_other.start(&streamed, b, 0.0, 1.0);
            w_other.start(&whole, b, 0.0, 1.0);
        }
        for _ in 0..ticks {
            // `a`'s bytes are held while `b`'s lookup may miss and evict:
            // an entry used this frame stays.
            let script = streamed.anim_script(a).unwrap();
            if let Some(b) = other {
                let _ = s_other.tick(streamed.anim_script(b).unwrap());
                let _ = w_other.tick(whole.anim_script(b).unwrap());
            }
            let s_res = s_main.tick(script);
            let w_res = w_main.tick(whole.anim_script(a).unwrap());
            assert_eq!(s_res.is_ok(), w_res.is_ok());
            for j in 0..w_main.joint_count() {
                let (s, w) = (s_main.pose(j).unwrap(), w_main.pose(j).unwrap());
                let bits = |p: &ssb_rom::figatree::JointPose| {
                    [p.rotate, p.translate, p.scale].map(|v| v.map(f32::to_bits))
                };
                assert_eq!(
                    bits(s),
                    bits(w),
                    "fighter {} slot {} joint {j}",
                    a.fighter,
                    a.slot
                );
                compared += 1;
            }
            end_frame();
        }
    }

    let shell = SHELL.lock().unwrap();
    let s = shell.as_ref().unwrap();
    eprintln!(
        "{} clips, {compared} joint poses; {} misses, {} evicted, max {} a frame",
        anims.len(),
        s.misses,
        s.evicted,
        s.max_frame
    );
    // The test streams: files were read and evicted on the way.
    assert!(s.misses > 1000 && s.evicted > 1000);
    assert!(compared > 100_000);
    drop(shell);
    unsafe { residency::install(core::ptr::null(), None) };
}
