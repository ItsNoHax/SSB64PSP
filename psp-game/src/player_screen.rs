//! Packed-asset adapter for the portable battle player interface.
use ssb_game::player_interface::{self as logic, Interface};
use ssb_psp_runtime::{
    gu::Gpu,
    meshdraw::{self, DrawState, SObjDraw},
    scene,
};
use ssb_rom::{
    pack::Pack,
    player_interface as asset,
};

pub fn frame_image(p: &Pack<'_>) -> Option<ssb_rom::texture::Rgba8> {
    let a = p.effect_anim(asset::ARROWS_SLOT + 1)?;
    Some(asset::frame(p.anim_script(&a)?)?.image)
}

pub fn tags(
    gpu: &mut Gpu,
    p: &Pack<'_>,
    st: &mut DrawState,
    fighters: [Option<&crate::play::FighterScene>; 4],
    camera: &ssb_game::camera::Camera,
    colors: [u8; 4],
    cp: [bool; 4],
) {
    gpu.set_viewport_n64([10.0, 10.0, 310.0, 230.0]);
    for f in fighters.into_iter().flatten() {
        let port = usize::from(f.fighter.port).min(3);
        let key = ssb_rom::sprite::PLAYER_TAGS.offsets[if cp[port] { 4 } else { port }];
        let Some(sprite) = p.sprite(38, key) else {
            continue;
        };
        let zoom = p
            .fighter(f.fighter.kind as u32)
            .map_or(0.0, |d| d.camera_zoom_base);
        let Some((x, y)) =
            logic::tag_position(&f.fighter, camera, zoom, [sprite.width, sprite.height])
        else {
            continue;
        };
        unsafe {
            meshdraw::draw_sprite(
                p,
                &sprite,
                &SObjDraw {
                    x,
                    y,
                    scale: 1.0,
                    prim: logic::TAG_COLORS[usize::from(colors[port]).min(4)],
                    env: [0; 3],
                    solid: false,
                    attr: ssb_rom::sprite::SP_TEXSHUF | ssb_rom::sprite::SP_TRANSPARENT,
                },
                st,
            );
        }
    }
    gpu.set_viewport_pillarboxed();
}

/// `ifCommonItemArrowProcDisplay` for every item with a pickup arrow.
pub fn item_arrows(
    gpu: &mut Gpu,
    p: &Pack<'_>,
    st: &mut DrawState,
    items: &ssb_game::item::ItemPool,
    camera: &ssb_game::camera::Camera,
) {
    let arrow = &ssb_rom::sprite::ITEM_ARROW;
    let Some(sprite) = p.sprite(arrow.file, arrow.offsets[0]) else {
        return;
    };
    gpu.set_viewport_n64([10.0, 10.0, 310.0, 230.0]);
    for item in items.items() {
        let Some((x, y)) = logic::item_arrow_position(item, camera, [sprite.width, sprite.height])
        else {
            continue;
        };
        unsafe {
            meshdraw::draw_sprite(
                p,
                &sprite,
                &SObjDraw {
                    x,
                    y,
                    scale: 1.0,
                    prim: logic::ITEM_ARROW_COLOR,
                    env: [0; 3],
                    solid: false,
                    attr: ssb_rom::sprite::SP_TEXSHUF | ssb_rom::sprite::SP_TRANSPARENT,
                },
                st,
            );
        }
    }
    gpu.set_viewport_pillarboxed();
}

pub unsafe fn arrows(
    gpu: &mut Gpu,
    p: &Pack<'_>,
    st: &mut DrawState,
    interface: &Interface,
    flags: u8,
) {
    let Some(obj) = scene::object_keyed(p, (asset::FILE, asset::ARROWS)) else {
        return;
    };
    let Some(anim) = p.effect_anim(asset::ARROWS_SLOT)
    else {
        return;
    };
    let Some(data) = p.anim_script(&anim) else {
        return;
    };
    gpu.set_viewport_n64([10.0, 10.0, 310.0, 230.0]);
    gpu.set_ortho([-150.0, 150.0, -110.0, 110.0], 100.0, 12800.0);
    gpu.set_view(&ssb_engine::math::Mat4::look_at(
        ssb_engine::math::Vec3::new(0.0, 0.0, 1000.0),
        ssb_engine::math::Vec3::ZERO,
        ssb_engine::math::Vec3::Y,
    ));
    for (side, arrow) in interface.arrows.iter().enumerate() {
        if flags & (1 << side) == 0 {
            continue;
        }
        let mut hidden = [false; 4];
        for j in (0..anim.joint_count).filter_map(|i| p.anim_joint(anim.first_joint + i)) {
            let mut joint = ssb_rom::objanim::StageJoint::start_changed(j.script, 0.0);
            let mut pose = ssb_rom::figatree::JointPose::default();
            let ticks = if arrow.ticks == 0 {
                0
            } else {
                (arrow.ticks - 1) % asset::ARROWS_PERIOD + 1
            };
            for _ in 0..ticks {
                if joint.tick(data, 1.0, &mut pose).is_err() {
                    break;
                }
            }
            if let Some(index) = j.node.checked_sub(obj.first_node).filter(|&n| n < 4) {
                hidden[index as usize] = joint.flags & 3 != 0;
            }
        }
        gpu.model_transform(
            [if side == 0 { -134.0 } else { 134.0 }, 0.0, 0.0],
            [0.0, 0.0, if side == 0 { 0.0 } else { 180.0 }],
            meshdraw::MODEL_SCALE,
        );
        st.color_override = Some(ssb_rom::skeleton::EffectColors {
            prim: Some([255, 0, 0, 128]),
            ..Default::default()
        });
        meshdraw::draw_object_posed_hiding(p, &obj, &gpu.model_matrix(), &[], st, None, &|node| {
            hidden
                .get(node.wrapping_sub(obj.first_node) as usize)
                .copied()
                .unwrap_or(false)
        });
        st.color_override = None;
    }
}

pub unsafe fn pointer(
    gpu: &mut Gpu,
    p: &Pack<'_>,
    st: &mut DrawState,
    xy: (f32, f32),
    direction: (f32, f32),
    scale: f32,
    color: [u8; 4],
) {
    let Some(obj) = scene::object_keyed(p, (asset::FILE, asset::POINTER)) else {
        return;
    };
    gpu.set_viewport_n64([10.0, 10.0, 310.0, 230.0]);
    gpu.set_ortho([-150.0, 150.0, -110.0, 110.0], 100.0, 12800.0);
    gpu.set_view(&ssb_engine::math::Mat4::look_at(
        ssb_engine::math::Vec3::new(0.0, 0.0, 1000.0),
        ssb_engine::math::Vec3::ZERO,
        ssb_engine::math::Vec3::Y,
    ));
    gpu.model_transform(
        [xy.0, xy.1, 0.0],
        [
            0.0,
            0.0,
            ssb_engine::math::atan2(direction.1, direction.0).to_degrees() - 90.0,
        ],
        meshdraw::MODEL_SCALE * scale * 0.5,
    );
    st.color_override = Some(ssb_rom::skeleton::EffectColors {
        prim: Some(color),
        ..Default::default()
    });
    meshdraw::draw_object_posed(p, &obj, &gpu.model_matrix(), &[], None, st, None, None, 0);
    st.color_override = None;
}
