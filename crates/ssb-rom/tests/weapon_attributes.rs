//! The clash search's inputs (RE-416): each ported weapon's `WPAttributes`
//! record (`ll*WeaponAttributes`, `include/reloc_data.us.h`), decoded as
//! `wpManagerMakeWeapon` reads it. The bitfields are big-endian and packed
//! in 32-bit units from offset 0x24: `size:16 angle:10`, then
//! `knockback_scale:10 damage:8 element:4 knockback_weight:10`, then
//! `shield_damage:8 attack_count:2 can_setoff:1 sfx:10 priority:3
//! can_rehit_item:1 can_rehit_fighter:1 can_hop:1 can_reflect:1
//! can_absorb:1 can_shield:1`, then `knockback_base:10`.

use ssb_rom::Archive;

fn word(d: &[u8], at: usize) -> u32 {
    u32::from_be_bytes(d[at..at + 4].try_into().unwrap())
}

#[derive(Debug, PartialEq, Eq)]
struct Attr {
    size: u32,
    damage: u32,
    attack_count: u32,
    can_setoff: bool,
    priority: i32,
    can_hop: bool,
    can_reflect: bool,
    can_absorb: bool,
}

fn decode(d: &[u8], off: usize) -> Attr {
    let a = off + 0x24;
    let (w0, w1, w2) = (word(d, a), word(d, a + 4), word(d, a + 8));
    Attr {
        size: w0 >> 16,
        damage: (w1 >> 14) & 0xFF,
        attack_count: (w2 >> 22) & 3,
        can_setoff: (w2 >> 21) & 1 != 0,
        priority: ((w2 >> 8) & 7) as i32,
        can_hop: (w2 >> 5) & 1 != 0,
        can_reflect: (w2 >> 4) & 1 != 0,
        can_absorb: (w2 >> 3) & 1 != 0,
    }
}

#[test]
fn every_ported_weapon_has_the_clash_priority_and_flags_the_port_uses() {
    let Some(path) = std::env::var_os("SSB64_ROM") else {
        return;
    };
    let rom = std::fs::read(path).unwrap();
    let info = ssb_rom::rom::identify(&rom).unwrap();
    let archive = Archive::open(&rom, info.region).unwrap();
    // (name, file id, record offset, damage, can_setoff, can_hop,
    // can_reflect, can_absorb): the file ids are `ll*FileID`.
    for (name, file, off, damage, setoff, hop, reflect, absorb) in [
        ("Mario Fireball", 0xCC, 0x00, 7, true, true, true, true),
        ("Luigi Fireball", 0xDE, 0x00, 6, true, true, true, true),
        ("Blaster", 0xD2, 0x00, 6, false, true, true, true),
        ("Charge Shot", 0xDA, 0x00, 0, true, true, true, true),
        ("Bomb", 0xD9, 0x0C, 9, false, true, false, false),
        ("Boomerang", 0xE2, 0x00, 9, true, true, true, false),
        ("Final Cutter", 0xE5, 0x08, 6, true, false, true, true),
        ("PK Thunder", 0xEF, 0x0C, 6, true, false, true, true),
        (
            "PK Thunder trail",
            0xEF,
            0x40,
            3,
            false,
            false,
            false,
            false,
        ),
        ("PK Fire", 0xF0, 0x00, 4, true, true, true, true),
        ("Thunder head", 0xF3, 0x0C, 12, false, false, false, false),
        ("Thunder trail", 0xF3, 0x40, 12, false, false, false, false),
        ("Thunder Jolt (air)", 0xF4, 0x00, 10, true, true, true, true),
        (
            "Thunder Jolt (ground)",
            0xF4,
            0x34,
            7,
            true,
            false,
            true,
            true,
        ),
        ("Egg", 0xF7, 0x0C, 14, true, true, true, false),
        ("Star", 0xF7, 0x40, 4, true, true, true, true),
    ] {
        let f = archive.load(file).unwrap();
        let a = decode(&f.data, off);
        eprintln!("{name}: {a:?}");
        assert_eq!(a.priority, ssb_game::weapon::WEAPON_PRIORITY, "{name}");
        assert_eq!(a.damage, damage, "{name}");
        assert_eq!(a.can_setoff, setoff, "{name}");
        assert_eq!(
            (a.can_hop, a.can_reflect, a.can_absorb),
            (hop, reflect, absorb),
            "{name}"
        );
        // One attack box, or the Boomerang's and the Thunder trail's two.
        let two = matches!(name, "Boomerang" | "Thunder trail");
        assert_eq!(a.attack_count, if two { 2 } else { 1 }, "{name}");
        assert!(a.size > 0 || name == "Charge Shot", "{name}");
    }
}
