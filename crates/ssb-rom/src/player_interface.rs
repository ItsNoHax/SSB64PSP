//! `IFCommonPlayer` file 166, `reloc_data.us.h` identities.
pub const FILE: u32 = 166;
pub const ARROWS: u32 = 0x188;
pub const ARROWS_ANIM: u32 = 0x270;
pub const ARROWS_SLOT: u32 = 0x120;
/// All three source visibility scripts loop after nine process ticks.
pub const ARROWS_PERIOD: u32 = 9;
pub const POINTER: u32 = 0x30;
pub const FRAME: u32 = 0x2C8;

/// The directly loaded IA8 quadrant, mirrored on both axes by the RDP.
/// `gDPLoadBlock` interleaves the linear source while loading TMEM.
pub fn frame(file: &[u8]) -> Option<crate::sprite::Sprite> {
    use crate::texture::{self, BitSize, Format, Rgba8};
    let bytes = file.get(FRAME as usize..FRAME as usize + 256)?;
    let mut image = Rgba8::new(16, 16);
    for y in 0..16 {
        for x in 0..16 {
            let texel = bytes[y * 16 + x];
            let i = (texel >> 4) * 17;
            image.put(y * 16 + x, [i, i, i, (texel & 15) * 17]);
        }
    }
    let image = texture::mirror_extend(&image, true, true, false, false, 16, 16);
    Some(crate::sprite::Sprite {
        width: 32,
        height: 32,
        attr: crate::sprite::SP_TRANSPARENT,
        color: [255; 4],
        format: Format::Ia,
        size: BitSize::Bits8,
        image,
    })
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;
    #[test]
    fn rom_frame_and_arrow_cycles_keep_original_samples() {
        let Some(path) = std::env::var_os("SSB64_ROM") else {
            return;
        };
        let rom = std::fs::read(path).unwrap();
        let info = crate::rom::identify(&rom).unwrap();
        let archive = crate::archive::Archive::open(&rom, info.region).unwrap();
        let file = archive.load(FILE).unwrap();
        let frame = frame(&file.data).unwrap();
        assert_eq!(frame.image.get(15 * 32 + 15), [255; 4]);
        assert_eq!(frame.image.get(0), [0; 4]);
        for y in 0..32 {
            for x in 0..32 {
                assert_eq!(
                    frame.image.get(y * 32 + x),
                    frame.image.get(y * 32 + 31 - x)
                );
                assert_eq!(
                    frame.image.get(y * 32 + x),
                    frame.image.get((31 - y) * 32 + x)
                );
            }
        }
        let scripts = crate::objanim::joint_scripts(&file.data, ARROWS_ANIM, 4);
        assert!(scripts[0].is_none());
        for script in scripts.into_iter().flatten() {
            let mut joint = crate::objanim::StageJoint::start_changed(script, 0.0);
            let mut pose = crate::figatree::JointPose::default();
            let mut flags = [0; 27];
            for flag in &mut flags {
                joint.tick(&file.data, 1.0, &mut pose).unwrap();
                *flag = joint.flags;
            }
            assert_eq!(&flags[..9], &flags[9..18]);
            assert_eq!(&flags[..9], &flags[18..]);
        }
    }
}
