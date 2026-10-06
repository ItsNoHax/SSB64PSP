//! The 1P Game's last scenes against the ROM: the staff roll's credits
//! tables in `ovl59`'s `.data`, its letter images, the names' path and
//! tilt, and the ending's operator camera (RE-459).
use ssb_game::spgame::staffroll::{self, Credits, Staffroll};
use ssb_rom::{ending, rom, Archive};

fn rom_bytes() -> Option<Vec<u8>> {
    std::fs::read(std::env::var_os("SSB64_ROM")?).ok()
}

#[test]
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
}

#[test]
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
