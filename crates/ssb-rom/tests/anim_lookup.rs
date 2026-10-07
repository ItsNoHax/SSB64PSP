//! RE-469: `Pack::fighter_anim`'s binary search over the built pack agrees
//! with a scan of the animation table in order, for every fighter and slot.

use ssb_rom::pack::Pack;

fn pack_bytes() -> Option<Vec<u8>> {
    std::env::var_os("SSB64_ROM")?;
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/generated/ssb64.pak");
    std::fs::read(path).ok()
}

#[test]
fn fighter_anim_matches_a_table_scan() {
    let Some(bytes) = pack_bytes() else {
        eprintln!("skipping: SSB64_ROM unset or pack not built");
        return;
    };
    let pack = Pack::open(&bytes).unwrap();
    let rows: Vec<_> = (0..pack.anim_count())
        .filter_map(|i| pack.anim(i))
        .collect();
    let mut found = 0;
    for fighter in 0..32 {
        for slot in 0..ssb_rom::anim::SLOT_COUNT as u32 {
            let scan = rows
                .iter()
                .find(|a| a.fighter == fighter && a.slot == slot)
                .copied();
            assert_eq!(
                pack.fighter_anim(fighter, slot),
                scan,
                "({fighter}, {slot})"
            );
            found += usize::from(scan.is_some());
        }
    }
    assert!(found > 5000, "{found} fighter clips");
}

/// RE-470: the special rows' lookups (the fighters' translate scales, the
/// shield poses, the stage, ground, item, weapon, effect and transition
/// clips) agree with the scans they replaced: first match in table order,
/// or last for the ones that searched from the end.
#[test]
fn special_rows_match_a_table_scan() {
    use ssb_rom::pack::AnimDesc;
    let Some(bytes) = pack_bytes() else {
        eprintln!("skipping: SSB64_ROM unset or pack not built");
        return;
    };
    let pack = Pack::open(&bytes).unwrap();
    let rows: Vec<_> = (0..pack.anim_count())
        .filter_map(|i| pack.anim(i))
        .collect();
    let first = |tag: u32, slot: u32| {
        rows.iter()
            .find(|a| a.fighter == tag && a.slot == slot)
            .copied()
    };
    let last = |tag: u32, slot: u32| {
        rows.iter()
            .rev()
            .find(|a| a.fighter == tag && a.slot == slot)
            .copied()
    };
    let script = |a: Option<AnimDesc>| a.and_then(|a| pack.anim_script(&a));
    let mut checked = 0;
    for slot in 0..64 {
        assert_eq!(
            pack.stage_anim(slot),
            first(AnimDesc::STAGE, slot),
            "stage {slot}"
        );
        assert_eq!(
            pack.ground_anim(slot),
            first(AnimDesc::GROUND, slot),
            "ground {slot}"
        );
        assert_eq!(
            pack.transition_anim(slot),
            first(AnimDesc::TRANSITION, slot),
            "transition {slot}"
        );
        assert_eq!(
            pack.weapon_anim(slot),
            first(AnimDesc::WEAPON, slot),
            "weapon {slot}"
        );
        assert_eq!(
            pack.item_anim(slot),
            first(AnimDesc::ITEM, slot),
            "item {slot}"
        );
        assert_eq!(
            pack.effect_anim(slot),
            first(AnimDesc::EFFECT, slot),
            "effect {slot}"
        );
        assert_eq!(
            pack.fighter_translate_scales(slot),
            script(last(AnimDesc::TRANSLATE_SCALES, slot)),
            "translate scales {slot}"
        );
        for sector in 0..AnimDesc::SHIELD_SECTORS {
            assert_eq!(
                pack.shield_pose(slot, sector),
                last(
                    AnimDesc::SHIELD_POSE,
                    slot * AnimDesc::SHIELD_SECTORS + sector
                ),
                "shield {slot} {sector}"
            );
        }
        checked += 1;
    }
    assert_eq!(
        pack.training_layout(),
        script(last(AnimDesc::TRAINING_LAYOUT, 0))
    );
    let special = rows.iter().filter(|a| a.fighter > 1000).count();
    assert!(checked == 64 && special > 300, "{special} special rows");
}

/// RE-470: the sprite lookups' index agrees with a scan of the sprite table
/// in order, for every row's key and every fighter, role, costume and stage.
#[test]
fn sprite_lookups_match_a_table_scan() {
    use ssb_rom::pack::SpriteDesc;
    let Some(bytes) = pack_bytes() else {
        eprintln!("skipping: SSB64_ROM unset or pack not built");
        return;
    };
    let pack = Pack::open(&bytes).unwrap();
    let rows: Vec<SpriteDesc> = (0..pack.sprite_count())
        .filter_map(|i| pack.sprite_at(i))
        .collect();
    assert!(rows.len() > 500, "{} sprites", rows.len());
    for s in &rows {
        let (file, offset) = (s.source_file, s.source_offset);
        let scan = rows
            .iter()
            .find(|r| {
                r.source_file == file && r.source_offset == offset && r.role != SpriteDesc::ROLE_LUT
            })
            .copied();
        assert_eq!(pack.sprite(file, offset), scan, "sprite {file} {offset:#x}");
        for lut in 0..8 {
            let scan = rows
                .iter()
                .find(|r| {
                    r.source_file == file
                        && r.source_offset == offset
                        && r.role == SpriteDesc::ROLE_LUT
                        && r.costume == lut
                })
                .copied();
            assert_eq!(
                pack.sprite_lut(file, offset, lut),
                scan,
                "lut {file} {offset:#x} {lut}"
            );
        }
    }
    assert_eq!(pack.sprite(u32::MAX, 0), None);
    for fighter in 0..32u8 {
        for role in 0..5u8 {
            for costume in 0..8u8 {
                let scan = rows
                    .iter()
                    .find(|s| {
                        s.fighter == fighter
                            && s.role == role
                            && (role != SpriteDesc::ROLE_STOCK || s.costume == costume)
                    })
                    .copied();
                assert_eq!(
                    pack.fighter_sprite(fighter, role, costume),
                    scan,
                    "{fighter} {role} {costume}"
                );
            }
        }
        let scan = rows
            .iter()
            .find(|s| s.role == SpriteDesc::ROLE_WALLPAPER && s.costume == fighter)
            .copied();
        assert_eq!(pack.stage_wallpaper(fighter), scan, "wallpaper {fighter}");
    }
}

/// RE-470: a costume lookup within a node's own rows agrees with the
/// search of the whole table, for every override row and its neighbours.
#[test]
fn costume_rows_hold_every_override_of_their_node() {
    let Some(bytes) = pack_bytes() else {
        eprintln!("skipping: SSB64_ROM unset or pack not built");
        return;
    };
    let pack = Pack::open(&bytes).unwrap();
    let rows: Vec<_> = (0..pack.costume_override_count())
        .filter_map(|i| pack.costume_override(i))
        .collect();
    assert!(rows.len() > 100, "{} overrides", rows.len());
    for o in &rows {
        for node in [o.node.saturating_sub(1), o.node, o.node + 1] {
            let run = pack.costume_rows(node);
            assert!(rows[run.start as usize..run.end as usize]
                .iter()
                .all(|r| r.node == node));
            assert_eq!(
                run.len(),
                rows.iter().filter(|r| r.node == node).count(),
                "rows of node {node}"
            );
            for costume in [0, o.costume.saturating_sub(1), o.costume, o.costume + 1] {
                let expect = rows
                    .iter()
                    .find(|r| r.node == node && r.costume == costume && costume != 0)
                    .map(|r| r.mesh);
                assert_eq!(pack.costume_mesh(node, costume), expect, "{node} {costume}");
                assert_eq!(
                    pack.costume_mesh_in(run.clone(), node, costume),
                    expect,
                    "{node} {costume} in rows"
                );
            }
        }
    }
}
