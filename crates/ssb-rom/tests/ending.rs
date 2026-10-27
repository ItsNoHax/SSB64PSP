//! The 1P Game's last scenes against the ROM: the staff roll's credits
//! tables in `ovl59`'s `.data`, its letter images, the names' path and
//! tilt, and the ending's operator camera (RE-459).
use ssb_game::spgame::staffroll::{self, Credits, Staffroll};
use ssb_rom::{ending, rom, Archive};

fn rom_bytes() -> Option<Vec<u8>> {
    std::fs::read(ssb_tablegen::test_rom(env!("CARGO_MANIFEST_DIR"))?).ok()
}

#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn the_credits_tables_parse_with_the_sources_counts() {
    let Some(bytes) = rom_bytes() else { return };
    let c = Credits::parse(ending::credits_bytes(&bytes).unwrap()).unwrap();
    // `dSCStaffrollUnused0x80136794`'s 0x4A5 characters, 84 names, roles
    // and company ids, 16 US jobs ending in "Presents".
    assert_eq!(c.name_chars.len(), 0x4A5);
    assert_eq!(
        (c.names.len(), c.roles.len(), c.company_ids.len()),
        (84, 84, 84)
    );
    assert_eq!(c.jobs.len(), 16);
    assert_eq!(c.jobs[0].staff_count, 1);
    assert_eq!(c.jobs[15].staff_count, -1);
    assert_eq!(c.companies.len(), 11);
    let first: Vec<i32> = b"Masahiro".iter().map(|&b| staffroll::letter(b)).collect();
    assert_eq!(&c.name_chars[..8], &first[..]);
}

#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn the_roll_reaches_its_last_name_with_the_rom_tables() {
    let Some(bytes) = rom_bytes() else { return };
    let info = rom::identify(&bytes).unwrap();
    let archive = Archive::open(&bytes, info.region).unwrap();
    let file = archive.load(ending::STAFFROLL_FILE).unwrap();
    struct Motion(Vec<u8>, ssb_rom::interp::Spline);
    impl staffroll::NameMotion for Motion {
        fn path(&self, t: f32) -> [f32; 3] {
            self.1.cubic(&self.0, t).unwrap()
        }
        fn rotate_z(&self, frame: f32) -> f32 {
            let mut pose = ssb_rom::figatree::JointPose::default();
            let mut j =
                ssb_rom::objanim::StageJoint::start_changed(ending::STAFFROLL_ANIM_JOINT, frame);
            j.tick(&self.0, 1.0, &mut pose).unwrap();
            pose.rotate[2]
        }
    }
    let spline =
        ssb_rom::interp::Spline::read(&file.data, ending::STAFFROLL_INTERPOLATION).unwrap();
    let motion = Motion(file.data.clone(), spline);
    // The tilt runs from -0.19 to 0.4189 over 99 frames.
    assert!((staffroll::NameMotion::rotate_z(&motion, 0.0) + 0.19).abs() < 0.01);
    let c = Credits::parse(ending::credits_bytes(&bytes).unwrap()).unwrap();
    let mut s = Staffroll::new(c, 0);
    let mut ticks = 0;
    while !s.tick(Default::default(), Default::default(), &motion) {
        ticks += 1;
        assert!(ticks < 20_000);
    }
    assert_eq!(s.name_id, 84);
    // N64 staff-roll warp (no input): the Startup scene is set on update
    // 5068 and the roll's last draw is update 5127 (`ROLL_END_WAIT` 60).
    assert_eq!(ticks + 1, 5127);
}

#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn the_letter_images_fit_before_the_next_and_the_camera_bakes() {
    let Some(bytes) = rom_bytes() else { return };
    let info = rom::identify(&bytes).unwrap();
    let archive = Archive::open(&bytes, info.region).unwrap();
    let file = archive.load(ending::STAFFROLL_FILE).unwrap();
    let mut starts: Vec<(u32, usize)> = ending::NAME_IMAGES.iter().copied().zip(0..).collect();
    starts.sort();
    for w in starts.windows(2) {
        let [gw, gh] = staffroll::NAME_GLYPHS[w[0].1];
        let len = u32::from(gw).next_multiple_of(16) * u32::from(gh) / 2;
        assert!(w[0].0 + len <= w[1].0, "letter {} overruns", w[0].1);
        ending::name_glyph(&file, w[0].0, u16::from(gw), u16::from(gh)).unwrap();
    }
    let cam = archive.load(ending::ENDING_FILE).unwrap();
    let frames = ssb_rom::campaign::camera_frames(&cam.data, ending::ENDING_CAMERA).unwrap();
    assert!(frames.len() >= 540, "{} plays", frames.len());
}

fn pack_bytes() -> Option<Vec<u8>> {
    ssb_tablegen::test_rom(env!("CARGO_MANIFEST_DIR"))?;
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/generated/ssb64.pak");
    std::fs::read(path).ok()
}

/// `mvEndingMakeRoomBackground` adds `llMVCommonRoomBackgroundMatAnimJoint`
/// (0x8788) over `llMVCommonRoomBackgroundMObjSub` (0x42F8): the packed
/// background's primitives carry one `mat_anim` per scripted `MObjSub`, as
/// many as the table names scripts (two readings: the pack's bindings and
/// the file's table).
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn the_room_background_carries_its_material_animation() {
    use ssb_rom::pack::{Pack, TextureDesc};
    let (Some(bytes), Some(pack)) = (rom_bytes(), pack_bytes()) else {
        return;
    };
    let pack = Pack::open(&pack).unwrap();
    let object = (0..pack.object_count())
        .filter_map(|i| pack.object(i))
        .find(|o| o.source_file == ending::ROOM_FILE && o.source_offset == ending::ROOM_BACKGROUND)
        .expect("the room background is packed");
    let mut bound = std::collections::BTreeSet::new();
    for n in 0..object.node_count {
        let node = pack.node(object.first_node + n).unwrap();
        let Some(mesh) = pack.mesh(node.mesh) else {
            continue;
        };
        for j in 0..mesh.prim_count {
            let prim = pack.prim(mesh.first_prim + j).unwrap();
            if prim.mat_anim != TextureDesc::NO_ANIM {
                bound.insert(prim.mat_anim);
            }
        }
    }
    let info = rom::identify(&bytes).unwrap();
    let archive = Archive::open(&bytes, info.region).unwrap();
    let file = archive.load(ending::ROOM_FILE).unwrap();
    let word = |at: usize| u32::from_be_bytes(file.data[at..at + 4].try_into().unwrap());
    let subs = ssb_rom::opening::room::BACKGROUND_MOBJSUB as usize;
    let mut scripted = 0;
    for node in 0..object.node_count as usize {
        let chain = word(subs + node * 4) as usize;
        if chain == 0 {
            continue;
        }
        let mut m = 0;
        while word(chain + m * 4) != 0 {
            let sub = word(chain + m * 4);
            scripted += usize::from(
                ssb_rom::matanim::mobj_script(
                    &file.data,
                    ending::ROOM_BACKGROUND_MAT_ANIM_JOINT,
                    subs as u32,
                    node,
                    sub,
                )
                .is_some(),
            );
            m += 1;
        }
    }
    assert!(scripted > 0, "the table names scripts");
    assert!(!bound.is_empty(), "the packed prims are bound");
    assert_eq!(bound.len(), scripted, "one binding per scripted MObj");
}

/// `gcDrawDObjTreeDLLinksForGObj` draws the background's task-1 links in
/// `DObjDesc` order, each DObj's links in its `DObjDLLink` array's order.
/// The pack holds DObj 20's and 37's second links as appended leaves; in
/// `object_draw_order` they draw where the file has them, so the window
/// jamb (DObj 20's list-1 link) no longer draws after the curtain (RE-459).
/// Two readings: the file's link arrays and the pack's meshes.
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn the_room_background_draws_its_links_in_dobj_order() {
    use ssb_rom::pack::{flags, Pack};
    let (Some(bytes), Some(pack)) = (rom_bytes(), pack_bytes()) else {
        return;
    };
    let info = rom::identify(&bytes).unwrap();
    let archive = Archive::open(&bytes, info.region).unwrap();
    let file = archive.load(ending::ROOM_FILE).unwrap();
    let word = |at: usize| u32::from_be_bytes(file.data[at..at + 4].try_into().unwrap());
    // `DObjDesc` is 0x2C bytes: id, the union's link array, then TRS.
    let mut file_links = Vec::new();
    let mut desc = ending::ROOM_BACKGROUND as usize;
    while word(desc) != 18 {
        let mut link = word(desc + 4) as usize;
        while link != 0 && word(link) != 4 {
            if word(link) == 1 {
                file_links.push(word(link + 4));
            }
            link += 8;
        }
        desc += 0x2C;
    }

    let pack = Pack::open(&pack).unwrap();
    let object = (0..pack.object_count())
        .filter_map(|i| pack.object(i))
        .find(|o| o.source_file == ending::ROOM_FILE && o.source_offset == ending::ROOM_BACKGROUND)
        .expect("the room background is packed");
    let mut pack_links = Vec::new();
    for n in pack.object_draw_order(&object) {
        let node = pack.node(object.first_node + n).unwrap();
        let Some(mesh) = pack.mesh(node.mesh) else {
            continue;
        };
        let head1 = (0..mesh.prim_count)
            .any(|j| pack.prim(mesh.first_prim + j).unwrap().flags & flags::HEAD1 != 0);
        if head1 {
            pack_links.push(mesh.source_offset);
        }
    }
    assert_eq!(file_links.len(), 10, "{file_links:x?}");
    assert_eq!(pack_links, file_links);
}
