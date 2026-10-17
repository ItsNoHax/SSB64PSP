//! The Reflector hexagon's packed tables (`effect::FOX_REFLECTOR_ANIM_SLOT`)
//! replayed as `efManagerFoxReflectorProcUpdate` plays them reproduce an
//! N64 RDRAM trace of a CPU Fox (port 2) holding the Reflector: the drawn
//! child's scale and the `GObj`'s `anim_frame` on every frame from the make
//! (el 31) to the eject (el 82), B released so End is requested on el 72.
//! `psp-game`'s `ReflectorFx::sync` is this replay.

use ssb_rom::pack::Pack;
use ssb_rom::skeleton::StageAnimator;

fn pack_bytes() -> Option<Vec<u8>> {
    std::env::var_os("SSB64_ROM")?;
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/generated/ssb64.pak");
    std::fs::read(path).ok()
}

#[test]
fn reflector_tables_replay_the_n64_trace() {
    let Some(bytes) = pack_bytes() else {
        eprintln!("skipping: SSB64_ROM unset or pack not built");
        return;
    };
    let p = Pack::open(&bytes).unwrap();
    let key = ssb_rom::effect::FOX_REFLECTOR_KEY;
    let own = ssb_rom::effect::MANAGER_EFFECT_KEYS
        .iter()
        .position(|&k| k == key)
        .unwrap() as u32;
    let object = (0..p.object_count())
        .filter_map(|i| p.object(i))
        .find(|o| (o.source_file, o.source_offset) == key)
        .unwrap();
    let slot = ssb_rom::effect::FOX_REFLECTOR_ANIM_SLOT;
    let anims = [own, slot, slot + 1, slot + 2].map(|s| p.effect_anim(s).unwrap());
    // (scale, anim_frame) of el 31..=81; el 82 has no effect.
    let start = [(0.4, 1.0), (0.6, 2.0), (0.8, 3.0)];
    let lp = [1.0, 0.4, 0.4, 0.8, 0.8, 0.2, 0.2, 0.6, 0.6, 1.0];
    let end = [0.2, 0.2, 0.6, 0.6, 1.0, 1.0, 0.8, 0.6, 0.4, 0.2];
    let mut want: Vec<(f32, f32)> = start.to_vec();
    for el in 34..=71 {
        let f = (el - 34) % 10;
        want.push((lp[f], f as f32));
    }
    want.extend(end.iter().enumerate().map(|(f, &s)| (s, f as f32)));
    assert_eq!(want.len(), 51);

    let mut a = StageAnimator::default();
    a.start(&p, &anims[0]);
    let mut index = 0;
    let set = |a: &mut StageAnimator, index: &mut usize, id: usize| {
        a.start_changed(&p, &anims[id]);
        let _ = a.tick(p.anim_script(&anims[id]).unwrap());
        *index = id;
    };
    let mut got = Vec::new();
    for el in 31..=82 {
        let _ = a.tick(p.anim_script(&anims[index]).unwrap());
        if a.gobj_frame() <= 0.0 {
            match index {
                0 | 2 => set(&mut a, &mut index, 1),
                3 => break,
                _ => {}
            }
        }
        if el == 72 {
            set(&mut a, &mut index, 3);
        }
        let scale = a.node_pose(object.first_node + 1).unwrap().scale;
        got.push((scale[0], a.gobj_frame()));
    }
    assert_eq!(got.len(), want.len(), "ejected on el 82");
    for (i, (g, w)) in got.iter().zip(&want).enumerate() {
        assert!(
            (g.0 - w.0).abs() < 1e-4 && (g.1 - w.1).abs() < 1e-4,
            "el {}: {g:?} != {w:?}",
            31 + i
        );
    }
}
