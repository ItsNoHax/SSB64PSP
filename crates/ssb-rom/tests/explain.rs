//! How to Play's ROM data (`ssb_rom::explain`) against the decomp's
//! `relocData/252_SCExplainMain.c` and `198_SCExplainGraphics.c`, read as
//! the game reads it (`ssb_game::explain`, `ssb_game::key`).

use ssb_game::explain::{graphics, Phase, KEY_EVENTS, PHASE_COUNT};
use ssb_rom::explain as ex;
use ssb_rom::matanim::{MaterialJoint, TRACK_TEXTURE_ID_CURRENT};

fn files() -> Option<(ssb_rom::archive::File, ssb_rom::archive::File)> {
    let path = std::env::var_os("SSB64_ROM")?;
    let data = std::fs::read(path).unwrap();
    let info = ssb_rom::rom::identify(&data).unwrap();
    let archive = ssb_rom::archive::Archive::open(&data, info.region).unwrap();
    Some((
        archive.load(ex::FILE_MAIN).unwrap(),
        archive.load(ex::FILE_GRAPHICS).unwrap(),
    ))
}

#[test]
fn the_phase_table_names_packed_captions() {
    let Some((main, _)) = files() else {
        return;
    };
    let table = ex::phase_table(&main).unwrap();
    let phases = Phase::read_all(&table).unwrap();
    assert_eq!(phases.len(), PHASE_COUNT);
    // `dSCExplainMain_ExplainPhase_0x1404` (US): the banner for 180, then
    // "Tap the stick" with the tap-forward stick at (257, 26).
    assert_eq!((phases[0].time, phases[0].sprite), (180, 0x10260));
    assert_eq!(phases[1].sprite, 0x11F60);
    assert_eq!((phases[1].textbox_x, phases[1].textbox_y), (20, 11));
    assert_eq!(
        phases[1].stick,
        ssb_game::explain::Args {
            x: 257,
            y: 26,
            status: 6
        }
    );
    // Special moves: the overlay over the neutral stick.
    assert_eq!(phases[13].time, 360);
    assert_eq!(phases[13].rgb.status, 1);
    assert_eq!(
        phases[19].args[5],
        ssb_game::explain::Args {
            x: 253,
            y: 33,
            status: 1
        }
    );
    for p in &phases {
        assert!(
            ex::SPRITES.offsets.contains(&p.sprite),
            "caption {:#x} is not packed",
            p.sprite
        );
    }
    let total: u32 = phases.iter().map(|p| u32::from(p.time)).sum();
    assert_eq!(total, 4450);
}

#[test]
fn the_input_scripts_parse() {
    let Some((main, _)) = files() else {
        return;
    };
    let keys = ex::key_scripts(&main).unwrap();
    let lens: Vec<usize> = KEY_EVENTS
        .iter()
        .map(|&at| ssb_game::key::parse(&keys, at as usize).unwrap().len())
        .collect();
    // Each script ends where the next begins (less its alignment
    // halfword); players 3 and 4 only end.
    assert_eq!(lens[2], 1);
    assert_eq!(lens[3], 1);
    assert!(lens[0] * 2 <= KEY_EVENTS[1] as usize);
    assert!(lens[1] * 2 <= (KEY_EVENTS[2] - KEY_EVENTS[1]) as usize);
    assert!(lens[0] > 100 && lens[1] > 100);
}

/// `*(AObjEvent32***)joint`: DObj 0's MObj 0 script.
fn script(data: &[u8], joint: u32) -> u32 {
    let word = |at: u32| u32::from_be_bytes(data[at as usize..at as usize + 4].try_into().unwrap());
    word(word(joint))
}

#[test]
fn the_stick_taps_loop_and_fire_on_frame_15() {
    let Some((_, graphics)) = files() else {
        return;
    };
    let data = ex::anim_bytes(&graphics).unwrap();
    for (status, period) in [(4u8, 24u32), (6, 25)] {
        let joint = graphics::STICK_MAT_ANIM_JOINTS[usize::from(status)];
        let mut j = MaterialJoint::start(script(&data, joint), 0.0);
        j.tick(&data, 1.0).unwrap();
        let mut fires = Vec::new();
        for frame in 1..=3 * period {
            j.tick(&data, 1.0).unwrap();
            if j.anim_frame() == 15.0 {
                fires.push(frame);
            }
            let id = j.track_value(TRACK_TEXTURE_ID_CURRENT).unwrap() as usize;
            assert!(id < graphics::STICK_TEXTURES.len());
        }
        assert_eq!(fires, [15, 15 + period, 15 + 2 * period], "status {status}");
    }
}

#[test]
fn the_spark_plays_out() {
    let Some((_, graphics)) = files() else {
        return;
    };
    let data = ex::anim_bytes(&graphics).unwrap();
    let mut j = MaterialJoint::start(script(&data, graphics::TAP_SPARK_MAT_ANIM_JOINT), 0.0);
    j.tick(&data, 1.0).unwrap();
    let mut frames = 0;
    while !j.ended() {
        j.tick(&data, 1.0).unwrap();
        frames += 1;
        let id = j.track_value(TRACK_TEXTURE_ID_CURRENT).unwrap() as usize;
        assert!(id < graphics::SPARK_TEXTURES.len());
        assert!(frames < 30);
    }
    assert_eq!(frames, 8);
}

#[test]
fn the_textures_decode() {
    let Some((_, graphics)) = files() else {
        return;
    };
    for &at in &ex::STICK_TEXTURES {
        assert!(ex::stick_texture(&graphics, at).is_some());
    }
    for &at in &ex::SPARK_TEXTURES {
        assert!(ex::spark_texture(&graphics, at).is_some());
    }
    let rgb = ex::rgb_texture(&graphics).unwrap();
    assert_eq!((rgb.width, rgb.height), (60, 48));
    assert_eq!(graphics::STICK_TEXTURES, ex::STICK_TEXTURES);
    assert_eq!(graphics::SPARK_TEXTURES, ex::SPARK_TEXTURES);
}
