//! [`super`] against the real ROM and the built pack
//! (`assets/generated/ssb64.pak`). Skipped without a ROM; nothing read
//! here is written anywhere (D-048).

extern crate std;

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;
use std::println;

use crate::archive::{Archive, File};
use crate::pack::Pack;

struct Real {
    rom: Vec<u8>,
    pack: Vec<u8>,
}

impl Real {
    fn load() -> Option<Self> {
        let path = ssb_tablegen::test_rom(env!("CARGO_MANIFEST_DIR"))?;
        let rom = std::fs::read(path).ok()?;
        let pack = std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../assets/generated/ssb64.pak"),
        )
        .ok()?;
        Some(Self { rom, pack })
    }

    fn archive(&self) -> Archive<'_> {
        let info = crate::rom::identify(&self.rom).unwrap();
        Archive::open(&self.rom, info.region).unwrap()
    }

    fn pack(&self) -> Pack<'_> {
        Pack::open(&self.pack).expect("the built pack matches this crate's format")
    }
}

/// Where `chunk` sits in `file`, if it is a window of the file's
/// [`Archive::load`] bytes: every word equal, except intern slots, which
/// may hold their target rebased to the window (`romtool`'s weapon
/// windows, RE-543).
fn window(file: &File, chunk: &[u8]) -> Option<usize> {
    let interns: BTreeMap<u32, u32> = file
        .intern_relocs
        .iter()
        .map(|r| (r.at, r.target))
        .collect();
    let d = &file.data;
    if chunk.len() > d.len() {
        return None;
    }
    let fits = |w: usize| {
        let mut k = 0;
        while k < chunk.len() {
            let at = w + k;
            if at.is_multiple_of(4) && k + 4 <= chunk.len() {
                if let Some(&target) = interns.get(&(at as u32)) {
                    let got = u32::from_be_bytes(chunk[k..k + 4].try_into().unwrap());
                    if got != target && got != target.wrapping_sub(w as u32) {
                        return false;
                    }
                    k += 4;
                    continue;
                }
            }
            if chunk[k] != d[at] {
                return false;
            }
            k += 1;
        }
        true
    };
    (0..=d.len() - chunk.len()).step_by(4).find(|&w| fits(w))
}

/// Every word an `AObjEvent32` interpreter can read from `roots`
/// (`objanim::StageJoint`, `matanim::MaterialJoint`, `sector`): each
/// command, value and pointer word on every path, and the
/// `SYInterpDesc` a `SetInterp` names (its three pointers at 8, 0x10,
/// 0x14). `Err((at, opcode))` for a command none of them models.
fn reached(data: &[u8], roots: &[u32]) -> Result<BTreeSet<u32>, (u32, u32)> {
    let word = |at: u32| {
        data.get(at as usize..at as usize + 4)
            .map(|b| u32::from_be_bytes(b.try_into().unwrap()))
            .ok_or((at, u32::MAX))
    };
    let mut words = BTreeSet::new();
    let mut visited = BTreeSet::new();
    let mut pending: Vec<u32> = roots.to_vec();
    while let Some(mut pc) = pending.pop() {
        while visited.insert(pc) {
            let w = word(pc)?;
            words.insert(pc);
            let (opcode, flags) = (w >> 25, (w >> 15) & 0x3FF);
            pc += 4;
            let per = match opcode {
                0 => break,
                // `Jump`, `SetAnim`: the next word is the script pointer.
                1 | 14 => {
                    words.insert(pc);
                    pending.push(word(pc)?);
                    break;
                }
                2 | 12 | 15 => 0,
                13 => {
                    words.insert(pc);
                    let desc = word(pc)?;
                    words.extend((0..6).map(|k| desc + 4 * k));
                    pc += 4;
                    0
                }
                22 => 0,
                5 | 6 => 2,
                3 | 4 | 7..=11 | 18..=21 => 1,
                _ => return Err((pc - 4, opcode)),
            };
            let values = if opcode == 22 {
                (flags & 0x1F).count_ones()
            } else {
                per * flags.count_ones()
            };
            for _ in 0..values {
                words.insert(pc);
                pc += 4;
            }
        }
    }
    Ok(words)
}

/// A chunk of `relocData` bytes in the pack and what reads it.
struct Chunk {
    what: alloc::string::String,
    file: u32,
    bytes: &'static [u8],
    /// Script starts, chunk-relative.
    roots: Vec<u32>,
    /// Words read other than through scripts, chunk-relative.
    words: Vec<u32>,
}

/// L0 of the runtime-loader batch: no byte the scene loader places is a
/// pointer that needs patching at load (D-011).
///
/// Per-file blob chunks are meshes, bindings, indices, texels, palettes
/// and figatree/object-animation scripts (`PackWriter::push_blob`);
/// material-animation files and particle bytecode are in the shared,
/// resident region. Only the scripts and material files carry `relocData`
/// bytes; everything else is built by the converter. `relayout` moves
/// every chunk without rewriting its bytes (it asserts each moved range
/// equal), so a reference inside a chunk can only be chunk-relative. What
/// remains is the extern slots `Archive::load` zeroed: for each chunk, its
/// window in the source file, the slots inside it, and whether any word
/// the chunk's readers can reach is one.
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn pack_readers_never_reach_an_extern_slot() {
    let Some(real) = Real::load() else { return };
    let real: &'static Real = alloc::boxed::Box::leak(alloc::boxed::Box::new(real));
    let archive = real.archive();
    let pack = real.pack();
    use crate::pack::{AnimDesc, AnimJoint};
    let mut chunks: BTreeMap<(u32, u32), Chunk> = BTreeMap::new();
    for i in 0..pack.anim_count() {
        let a = pack.anim(i).unwrap();
        let Some(bytes) = pack.anim_script(&a).filter(|b| !b.is_empty()) else {
            continue;
        };
        let c = chunks
            .entry((a.script_offset, a.script_len))
            .or_insert_with(|| Chunk {
                what: alloc::format!("anim {i} fighter {:#x} slot {}", a.fighter, a.slot),
                file: a.source_file,
                bytes,
                roots: Vec::new(),
                words: Vec::new(),
            });
        for k in 0..a.joint_count {
            let j = pack.anim_joint(a.first_joint + k).unwrap();
            if j.script != AnimJoint::NO_SCRIPT {
                c.roots.push(j.script);
            }
        }
        // Readers that start from the file's own tables.
        if a.fighter == AnimDesc::SECTOR {
            // `sector::Script::locate`.
            if a.slot == crate::sector::SLOT_FLIGHT {
                for desc in crate::sector::SECTOR_DESCS {
                    for (field, _) in crate::sector::DESC_FIELDS {
                        let at = desc + field;
                        c.words.push(at);
                        let ptr = u32::from_be_bytes(
                            bytes[at as usize..at as usize + 4].try_into().unwrap(),
                        );
                        if ptr != 0 {
                            c.roots.push(ptr);
                        }
                    }
                }
                c.roots
                    .extend(crate::sector::PILOT_ANIMS.iter().filter(|&&s| s != 0));
                c.roots.extend([0x1B84, 0x1B34]);
            } else {
                c.roots.extend([0x2EB4, 0x2E74]);
            }
        }
        if a.fighter == AnimDesc::TRAINING_LAYOUT {
            // `training_screen`: a positioned sprite's word before its
            // pointer slot; the slot itself only keys `Pack::sprite`.
            for (at, count, stride) in crate::sprite::TRAINING_TABLES {
                if stride == 8 {
                    c.words.extend((0..count as u32).map(|k| at + k * stride));
                }
            }
        }
    }
    for i in 0..pack.mat_anim_count() {
        let d = pack.mat_anim(i).unwrap();
        let Some(bytes) = pack.mat_anim_file(&d).filter(|b| !b.is_empty()) else {
            continue;
        };
        let c = chunks
            .entry((d.file_offset, d.file_len))
            .or_insert_with(|| Chunk {
                what: alloc::format!("mat anim {i} (resident)"),
                file: d.source_file,
                bytes,
                roots: Vec::new(),
                words: Vec::new(),
            });
        c.roots.push(d.script);
    }
    let mut files: BTreeMap<u32, File> = BTreeMap::new();
    let (mut matched, mut with_externs, mut slots, mut walk_errors) = (0, 0, 0, 0);
    let mut unmatched = 0;
    let mut hits: Vec<alloc::string::String> = Vec::new();
    for c in chunks.values() {
        let file = match files.entry(c.file) {
            alloc::collections::btree_map::Entry::Occupied(e) => e.into_mut(),
            alloc::collections::btree_map::Entry::Vacant(e) => match archive.load(c.file) {
                Ok(f) => e.insert(f),
                Err(_) => {
                    unmatched += 1;
                    continue;
                }
            },
        };
        let Some(w) = window(file, c.bytes) else {
            unmatched += 1;
            continue;
        };
        matched += 1;
        let range = w as u32..(w + c.bytes.len()) as u32;
        let ext: BTreeSet<u32> = file
            .extern_relocs
            .iter()
            .filter(|r| range.contains(&r.at))
            .map(|r| r.at - range.start)
            .collect();
        if ext.is_empty() {
            continue;
        }
        with_externs += 1;
        slots += ext.len();
        let mut words = match reached(c.bytes, &c.roots) {
            Ok(words) => words,
            Err((at, op)) => {
                walk_errors += 1;
                println!(
                    "  {}: file {} walk stops at {at:#x} (opcode {op})",
                    c.what, c.file
                );
                BTreeSet::new()
            }
        };
        words.extend(c.words.iter().copied());
        let read: Vec<u32> = ext.intersection(&words).copied().collect();
        println!(
            "  {}: file {} window {w:#x}+{:#x}, {} roots, {} words reached, {} extern slots, {} reached",
            c.what,
            c.file,
            c.bytes.len(),
            c.roots.len(),
            words.len(),
            ext.len(),
            read.len()
        );
        for at in read {
            hits.push(alloc::format!(
                "{}: file {} slot {:#x}",
                c.what,
                c.file,
                at + range.start
            ));
        }
    }
    println!(
        "{} chunks: {matched} matched to their source file, {unmatched} built by the converter; \
         {with_externs} hold {slots} extern slots; {} reached",
        chunks.len(),
        hits.len()
    );
    assert!(matched > 0);
    assert_eq!(walk_errors, 0);
    assert_eq!(
        hits,
        alloc::vec::Vec::<alloc::string::String>::new(),
        "a reader reaches an extern slot"
    );
}

/// Root lists the scene loader uses (`psp-game/src/scene_load.rs` builds
/// its scenes from these), plus every fighter main file alone.
fn scene_root_lists() -> Vec<Vec<u32>> {
    use crate::scene_roots as r;
    let mut lists: Vec<Vec<u32>> = alloc::vec![
        r::GM_COMMON.to_vec(),
        r::EF_COMMON.to_vec(),
        r::IT_COMMON.to_vec(),
        r::FT_MANAGER.to_vec(),
        r::TRANSITIONS.to_vec(),
        r::TRAINING.to_vec(),
        r::BONUS1.to_vec(),
        r::BONUS2.to_vec(),
        r::EXPLAIN.to_vec(),
        r::AUTO_DEMO.to_vec(),
        r::STARTUP.to_vec(),
        r::TITLE.to_vec(),
        r::MODE_SELECT.to_vec(),
        r::ONE_P_MODE.to_vec(),
        r::VS_MODE.to_vec(),
        r::VS_OPTIONS.to_vec(),
        r::OPTION.to_vec(),
        r::DATA.to_vec(),
        r::CHARACTERS.to_vec(),
        r::VS_RECORD.to_vec(),
        r::SOUND_TEST.to_vec(),
        r::PLAYERS_VS.to_vec(),
        r::PLAYERS_1P.to_vec(),
        r::PLAYERS_TRAINING.to_vec(),
        r::PLAYERS_BONUS.to_vec(),
        r::MAPS.to_vec(),
        r::STAGE_PREVIEW_MODELS.to_vec(),
        r::VS_RESULTS.to_vec(),
        r::ONE_P_INTRO.to_vec(),
        r::ONE_P_CONTINUE.to_vec(),
        r::ONE_P_STAGE_CLEAR.to_vec(),
        r::ENDING.to_vec(),
        r::OPENING_ROOM.to_vec(),
        r::OPENING_RUN.to_vec(),
        r::OPENING_CLASH.to_vec(),
    ];
    lists.extend(r::FIGHTER_MAIN.iter().map(|&f| alloc::vec![f]));
    let mut battle = r::GM_COMMON.to_vec();
    battle.extend(
        r::EF_COMMON
            .iter()
            .chain(&r::IT_COMMON)
            .chain(&r::FT_MANAGER),
    );
    battle.extend(&r::FIGHTER_MAIN[..4]);
    lists.push(battle);
    lists
}

/// The ROM's extern closure from the `u16` extern-ID arrays alone.
fn rom_closure(archive: &Archive<'_>, roots: &[u32]) -> BTreeSet<u32> {
    let mut seen = BTreeSet::new();
    let mut stack = roots.to_vec();
    while let Some(id) = stack.pop() {
        if seen.insert(id) {
            stack.extend(archive.extern_ids(id).unwrap().into_iter().map(u32::from));
        }
    }
    seen
}

/// `scene_files::Loader::closure` (`psp-runtime/src/scene_files.rs`)
/// as written there, for comparison: a LIFO walk over pack deps that skips
/// files the table lacks.
fn scene_files_closure(pack: &Pack<'_>, roots: &[u32]) -> Vec<bool> {
    let mut want = alloc::vec![false; pack.file_count() as usize];
    let mut queue: Vec<u32> = roots.to_vec();
    while let Some(file) = queue.pop() {
        let Some(i) = pack.file_index(file) else {
            continue;
        };
        if core::mem::replace(&mut want[i as usize], true) {
            continue;
        }
        if let Some(d) = pack.file_at(i) {
            queue.extend((0..d.dep_count).filter_map(|k| pack.file_dep(&d, k)));
        }
    }
    want
}

/// L0/L1: the pack's dependency lists are the ROM's extern-ID arrays, so
/// the closure `reloc_link` computes over the pack equals the loader's and
/// the ROM's (restricted to files the pack keeps), and `Original` placement
/// over the pack visits the ROM placement order minus the files it omits.
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn closure_equals_scene_files_closure() {
    use super::{closure, place_into, FileSource, Order, Placement};
    let Some(real) = Real::load() else { return };
    let (archive, pack) = (real.archive(), real.pack());
    let (mut want, mut stack, mut p) = (Vec::new(), Vec::new(), Placement::default());
    for roots in scene_root_lists() {
        closure(&pack, &roots, &mut want, &mut stack).unwrap();
        assert_eq!(want, scene_files_closure(&pack, &roots), "{roots:x?}");
        let ids: BTreeSet<u32> = (0..pack.count())
            .filter(|&i| want[i as usize])
            .map(|i| pack.id(i))
            .collect();
        let rom: BTreeSet<u32> = rom_closure(&archive, &roots)
            .into_iter()
            .filter(|&id| pack.index(id).is_some())
            .collect();
        assert_eq!(ids, rom, "{roots:x?}");
        place_into(&pack, &roots, |_| false, Order::Original, &mut p).unwrap();
        assert_eq!(p.want, want);
        let placed: BTreeSet<u32> = p.order.iter().map(|&i| pack.id(i)).collect();
        assert_eq!(placed, ids);
        // Same depth-first order as the ROM's, minus files the pack omits.
        let files = closure_files(&archive, &roots);
        let rom_order: Vec<u32> = super::layout(&files, &roots)
            .unwrap()
            .order
            .into_iter()
            .filter(|&id| pack.index(id).is_some())
            .collect();
        let pack_order: Vec<u32> = p.order.iter().map(|&i| pack.id(i)).collect();
        assert_eq!(pack_order, rom_order, "{roots:x?}");
    }
}

fn closure_files(archive: &Archive<'_>, roots: &[u32]) -> BTreeMap<u32, File> {
    let mut files = BTreeMap::new();
    for &root in roots {
        if !files.contains_key(&root) {
            files.extend(archive.load_closure(root).unwrap());
        }
    }
    files
}

/// L1: the generic `Original` placement equals [`super::layout`] and,
/// aligned, `lbRelocGetAllocSize` for RE-341's 13 closures.
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn original_order_matches_alloc_size() {
    use super::{place_into, Files, Order, Placement};
    let Some(real) = Real::load() else { return };
    let archive = real.archive();
    let mut sets: Vec<Vec<u32>> = [203, 213, 217, 225, 236, 247, 296, 104, 106, 113, 152]
        .iter()
        .map(|&r| alloc::vec![r])
        .collect();
    sets.push(alloc::vec![203, 104, 296]);
    let mut battle = crate::scene_roots::GM_COMMON.to_vec();
    battle.extend(crate::scene_roots::FT_MANAGER);
    battle.extend([213, 247]);
    sets.push(battle);
    let mut p = Placement::default();
    for roots in &sets {
        let files = closure_files(&archive, roots);
        let src = Files::new(&files);
        place_into(&src, roots, |_| false, Order::Original, &mut p).unwrap();
        let old = super::layout(&files, roots).unwrap();
        assert_eq!(p.to_layout(&src), old, "{roots:?}");
        assert_eq!(
            super::align(p.size),
            super::alloc_size(&archive, roots).unwrap(),
            "{roots:?}"
        );
    }
}

/// `scene_files::Loader::load_missing`'s runs for a cold load of `want`:
/// `(first, last, start, end)` by file index.
fn scene_files_runs(pack: &Pack<'_>, want: &[bool], max_run: u32) -> Vec<(u32, u32, u32, u32)> {
    let n = pack.file_count();
    let missing = |i: u32| want[i as usize] && pack.file_at(i).is_some_and(|d| d.len > 0);
    let mut runs = Vec::new();
    let mut i = 0;
    while i < n {
        if !missing(i) {
            i += 1;
            continue;
        }
        let first = pack.file_at(i).unwrap();
        let (mut end, mut last, mut j) = (first.blob_start + first.len, i, i + 1);
        while j < n {
            let d = pack.file_at(j).unwrap();
            if d.len == 0 {
                j += 1;
                continue;
            }
            if !missing(j)
                || d.blob_start != end.next_multiple_of(16)
                || d.blob_start + d.len - first.blob_start > max_run
            {
                break;
            }
            end = d.blob_start + d.len;
            last = j;
            j += 1;
        }
        runs.push((i, last, first.blob_start, end));
        i = last + 1;
    }
    runs
}

/// L1: `Ascending` reads exactly `load_missing`'s runs, each at a line, and
/// every file sits at its run offset plus its pack spacing.
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn ascending_runs_are_64_aligned_and_cover_pack_runs() {
    use super::{place_into, FileSource, Order, Placement, LINE};
    let Some(real) = Real::load() else { return };
    let pack = real.pack();
    let mut p = Placement::default();
    for max_run in [4 << 20, 1 << 20] {
        for roots in scene_root_lists() {
            place_into(
                &pack,
                &roots,
                |_| false,
                Order::Ascending { max_run },
                &mut p,
            )
            .unwrap();
            let runs: Vec<_> = p
                .runs
                .iter()
                .map(|r| (r.first, r.last, r.start, r.start + r.len))
                .collect();
            assert_eq!(
                runs,
                scene_files_runs(&pack, &p.want, max_run),
                "{roots:x?}"
            );
            let mut end = 0;
            for r in &p.runs {
                assert_eq!(r.offset % LINE, 0);
                assert!(r.offset >= end);
                end = (r.offset + r.len).next_multiple_of(LINE);
                for i in r.first..=r.last {
                    if pack.len(i) > 0 {
                        assert_eq!(
                            p.offset[i as usize],
                            r.offset + pack.start(i).unwrap() - r.start
                        );
                    }
                }
            }
            assert_eq!(p.size, end);
        }
    }
}

/// The L0 walker reaches pointer words through `Jump`, `SetInterp` and
/// value tracks, so a script that read an extern slot would be reported.
#[test]
fn reached_follows_jumps_values_and_interp_descs() {
    let cmd = |op: u32, flags: u32| (op << 25) | (flags << 15);
    let words: [u32; 16] = [
        cmd(5, 0b11), // 0x00: two tracks of two values
        0,
        0,
        0,
        0,
        cmd(13, 0), // 0x14: SetInterp, desc at 0x28
        0x28,
        cmd(1, 0), // 0x1C: Jump to 0x24
        0x24,
        0xDEAD, // 0x24: the jump target, rewritten to End below
        0,
        0,
        0,
        0,
        0,
        0,
    ];
    let mut data: Vec<u8> = words.iter().flat_map(|w| w.to_be_bytes()).collect();
    // 0x24: End.
    data[0x24..0x28].copy_from_slice(&0u32.to_be_bytes());
    let got = reached(&data, &[0]).unwrap();
    let want: BTreeSet<u32> = [0x00, 0x04, 0x08, 0x0C, 0x10, 0x14, 0x18, 0x1C, 0x20, 0x24]
        .into_iter()
        .chain((0..6).map(|k| 0x28 + 4 * k))
        .collect();
    assert_eq!(got, want);
    assert_eq!(
        reached(&data, &[0x40]).unwrap_err().1,
        u32::MAX,
        "runs off the end"
    );
    data[0..4].copy_from_slice(&cmd(40, 0).to_be_bytes());
    assert_eq!(reached(&data, &[0]), Err((0, 40)));
}
