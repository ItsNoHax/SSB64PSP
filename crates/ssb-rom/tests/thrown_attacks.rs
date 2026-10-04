//! Independent US ROM input for SetDamageThrown's table and body attacks.
use ssb_game::{
    fighter::{Fighter, FighterKind},
    motion,
    status::{self, Preserve, Status, StatusTiming},
    thrown::ThrowOwner,
};
use ssb_rom::archive::Archive;

fn word(data: &[u8], at: u32) -> u32 {
    u32::from_be_bytes(data[at as usize..at as usize + 4].try_into().unwrap())
}

#[test]
fn damage_thrown_table_pointers_and_attacks_match_the_rom() {
    let Some(path) = std::env::var_os("SSB64_ROM") else {
        return;
    };
    let rom = std::fs::read(path).unwrap();
    let archive = Archive::open(&rom, ssb_rom::rom::identify(&rom).unwrap().region).unwrap();
    let common = archive.load(201).unwrap();
    // The table follows the common hit scripts; all 54 slots are intern
    // pointers. Mario, Metal Mario and Polygon Mario use the back variant.
    for row in 0..2 {
        for kind in 0..27 {
            let at = 0x758 + (row * 27 + kind) * 4;
            let target = common
                .intern_relocs
                .iter()
                .find(|r| r.at == at)
                .unwrap()
                .target;
            assert_eq!(
                target,
                if row == 1 && matches!(kind, 0 | 13 | 14) {
                    0x740
                } else {
                    0x728
                }
            );
        }
    }
    let files = [202, 208, 212, 216, 220, 224, 246, 235, 228, 242, 232, 238];
    for (&victim, file_id) in FighterKind::PLAYABLE.iter().zip(files) {
        let file = archive.load(file_id).unwrap();
        let ptr = file
            .extern_relocs
            .iter()
            .find(|r| {
                r.target_file == 201
                    && r.target_offset == 0x758
                    && word(&file.data, r.at - 4) >> 26 == 13
            })
            .unwrap();
        let generated = motion::fighter_scripts(victim).unwrap().words;
        assert_eq!(
            generated[ptr.at as usize / 4],
            motion::COMMON_BIT | (ptr.target_offset / 4)
        );
        for &thrower in FighterKind::PLAYABLE {
            for row in 0..2 {
                let target = common
                    .intern_relocs
                    .iter()
                    .find(|r| r.at == 0x758 + (row * 27 + thrower as u32) * 4)
                    .unwrap()
                    .target;
                let mut f = Fighter::new(victim, 1, 3);
                f.thrown.owner = Some(ThrowOwner {
                    port: 0,
                    kind: thrower,
                    team: 0,
                });
                f.thrown.script_id = row as u8;
                status::set_any_status_preserve(
                    &mut f,
                    Status::DamageFlyN.into(),
                    0.0,
                    StatusTiming::unknown(),
                    Preserve {
                        throw_pointer: true,
                        ..Preserve::NONE
                    },
                );
                let coll = f.attack_colls[0];
                let w = (0..5)
                    .map(|i| word(&common.data, target + i * 4))
                    .collect::<Vec<_>>();
                assert_eq!(coll.damage, ((w[0] >> 5) & 0xff) as i32);
                assert_eq!(coll.joint, ((w[0] >> 13) & 0x7f) as u8);
                assert_eq!(coll.can_rebound, w[0] & 0x10 != 0);
                assert_eq!(coll.element as u32, w[0] & 0xf);
                assert_eq!(coll.size, (w[1] >> 16) as f32 * 0.5);
                assert_eq!(
                    coll.offset,
                    ssb_engine::math::Vec3::new(
                        w[1] as i16 as f32,
                        (w[2] >> 16) as i16 as f32,
                        w[2] as i16 as f32
                    )
                );
                assert_eq!(coll.angle, (w[3] as i32) >> 22);
                assert_eq!(coll.kb_scale, ((w[3] >> 12) & 0x3ff) as i32);
                assert_eq!(coll.kb_weight, ((w[3] >> 2) & 0x3ff) as i32);
                assert_eq!(coll.is_hit_air, w[3] & 1 != 0);
                assert_eq!(coll.is_hit_ground, w[3] & 2 != 0);
                assert_eq!(coll.kb_base, ((w[4] >> 7) & 0x3ff) as i32);
                assert_eq!(coll.fgm_level, ((w[4] >> 21) & 7) as u8);
            }
        }
    }
}
