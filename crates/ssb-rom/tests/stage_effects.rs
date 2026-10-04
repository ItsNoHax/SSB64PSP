//! The stage effects the controllers make, against the US ROM and pack:
//! Yoshi's Island's cloud vapor is generator 0 of its own bank, which the
//! pack holds where the runtime looks for it.
use ssb_rom::{pack::Pack, particle};

#[test]
fn the_cloud_vapor_is_the_yoster_banks_first_generator() {
    let Some(path) = std::env::var_os("SSB64_ROM") else {
        return;
    };
    let rom = std::fs::read(path).unwrap();
    let index = particle::BANKS
        .iter()
        .position(|b| b.name == "gryoster")
        .unwrap();
    let (scripts, textures) = particle::decode_bank(&rom, particle::BANKS[index]).unwrap();
    assert!(!scripts.is_empty());
    assert!(usize::from(scripts[0].texture_id) < textures.len());

    let bytes = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/generated/ssb64.pak"),
    )
    .unwrap();
    let pack = Pack::open(&bytes).unwrap();
    let bank = pack.particle_bank(index as u32).unwrap();
    assert_eq!(bank.script_count as usize, scripts.len());
    assert_eq!(bank.texture_count as usize, textures.len());
    let packed = pack.particle_script(bank.first_script).unwrap();
    assert_eq!(packed.kind, scripts[0].kind);
    assert_eq!(packed.generator_lifetime, scripts[0].generator_lifetime);
}
