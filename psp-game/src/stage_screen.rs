//! The stage select's drawing (RE-419): `ssb_game::stage_select_layer`
//! lays out the sprites and the preview camera; this looks the sprites up
//! in the pack and draws the preview's stage model, played from its first
//! frame each time the preview is made (`mnMapsMakeLayer`'s
//! `gcAddAnimAll(..., 0.0F)` and `gcPlayAnimAll`).
//!
//! The preview's animators live on the heap: a `StageAnimator` is several
//! KB and the main thread's stack is 256 KB.

use alloc::boxed::Box;

use ssb_game::stage_select::StageSelect;
use ssb_game::stage_select_layer::{Draw, Hide, Layer, Piece, Source};
use ssb_psp_runtime::gu::Gpu;
use ssb_psp_runtime::meshdraw;
use ssb_rom::pack::Pack;
use ssb_rom::skeleton::{MaterialAnimator, StageAnimator};

/// The preview's stage and its clocks.
pub struct Preview {
    /// The layer's `preview_serial` the model was made for.
    serial: Option<u32>,
    stage: Option<u32>,
    anim: Option<ssb_rom::pack::AnimDesc>,
    animator: StageAnimator,
    mat_anim: MaterialAnimator,
    /// Global nodes `mnMapsMakeModel` hides.
    hide: [u32; 2],
    hide_len: usize,
}

/// Boxed so the session holds a pointer.
#[inline(never)]
pub fn start() -> Box<Preview> {
    Box::new(Preview {
        serial: None,
        stage: None,
        anim: None,
        animator: StageAnimator::new(),
        mat_anim: MaterialAnimator::new(),
        hide: [u32::MAX; 2],
        hide_len: 0,
    })
}

/// One tick after the layer's: a preview made again starts its model's
/// animations over, then every one plays a frame.
#[inline(never)]
pub fn tick(pack: Option<&Pack<'_>>, layer: &Layer, preview: &mut Preview) {
    let Some(p) = pack else {
        return;
    };
    if preview.serial != Some(layer.preview_serial) {
        preview.serial = Some(layer.preview_serial);
        make(p, layer, preview);
    }
    if let Some(script) = preview.anim.and_then(|a| p.anim_script(&a)) {
        let _ = preview.animator.tick(script);
    }
    preview.mat_anim.tick(p);
}

/// `mnMapsMakeModel`: the stage's layers, their animations from frame 0,
/// and the nodes it hides.
fn make(p: &Pack<'_>, layer: &Layer, preview: &mut Preview) {
    let model = layer.model();
    preview.stage = model.and_then(|m| ssb_psp_runtime::scene::vs_stage_index(p, m.gkind));
    preview.anim = preview.stage.and_then(|i| p.stage_anim(i));
    // In place: a stage without a controller clip starts no joints.
    preview.animator.start(p, &preview.anim.unwrap_or_default());
    preview.mat_anim.start(p);
    preview.hide_len = 0;
    let (Some(m), Some(stage)) = (model, preview.stage.and_then(|i| p.stage(i))) else {
        return;
    };
    let object = |slot: usize| {
        let o = stage.layers[slot];
        (o != ssb_rom::pack::StageDesc::NO_LAYER)
            .then(|| p.object(o))
            .flatten()
    };
    // The first child of `parent` in tree order: `DObjGetChild`.
    let child = |o: &ssb_rom::pack::ObjectDesc, parent: u32| {
        (o.first_node..o.first_node + o.node_count)
            .find(|&n| p.node(n).is_some_and(|d| d.parent == parent))
    };
    match m.hide {
        Hide::None => {}
        Hide::GrandchildOfLayer3 => {
            if let Some(o) = object(3) {
                if let Some(n) = child(&o, o.first_node).and_then(|c| child(&o, c)) {
                    preview.hide[0] = n;
                    preview.hide_len = 1;
                }
            }
        }
        Hide::Layer0TreeIndices(indices) => {
            if let Some(o) = object(0) {
                // The tree walk counts the root as 1; the pack's nodes are in
                // the same depth-first order.
                for (slot, i) in indices.into_iter().enumerate() {
                    preview.hide[slot] = o.first_node + i - 1;
                }
                preview.hide_len = indices.len();
            }
        }
    }
}

/// The screen back to front (`Layer::visit`).
#[inline(never)]
pub unsafe fn draw_all(
    gpu: &mut Gpu,
    p: &Pack<'_>,
    draw_state: &mut meshdraw::DrawState,
    select: &StageSelect,
    layer: &Layer,
    preview: &Preview,
) {
    layer.visit(select, |d| match d {
        Draw::Scissor(rect) => gpu.set_viewport_n64(rect),
        Draw::Tiled { piece, size } => draw_piece(p, draw_state, &piece, Some(size)),
        Draw::Fill { rect, rgba } => meshdraw::fill_rect_n64(rect, rgba, draw_state),
        Draw::Sprite(piece) => draw_piece(p, draw_state, &piece, None),
        Draw::Model => draw_model(gpu, p, draw_state, layer, preview),
    });
    gpu.set_viewport_fullscreen();
}

/// One `SObj` through `lbCommonDrawSObjAttr`.
unsafe fn draw_piece(
    p: &Pack<'_>,
    draw_state: &mut meshdraw::DrawState,
    piece: &Piece,
    size: Option<[f32; 2]>,
) {
    const SP_FASTCOPY: u16 = 0x0020;
    let s = match piece.source {
        Source::File { file, offset } => p.sprite(file, offset),
        Source::StageWallpaper(gkind) => p.stage_wallpaper(gkind),
    };
    let Some(s) = s else {
        return;
    };
    let [r, g, b, a] = s.color;
    let prim = piece.prim.map_or([r, g, b, a], |c| [c[0], c[1], c[2], a]);
    let attr = if piece.transparent {
        (s.attr & !SP_FASTCOPY) | ssb_rom::sprite::SP_TRANSPARENT
    } else {
        s.attr & !ssb_rom::sprite::SP_TRANSPARENT
    };
    let d = meshdraw::SObjDraw {
        x: piece.x,
        y: piece.y,
        scale: piece.scale,
        prim,
        env: piece.env,
        solid: false,
        attr,
    };
    match size {
        Some(size) => meshdraw::draw_sprite_tiled(p, &s, &d, size, draw_state),
        None => meshdraw::draw_sprite(p, &s, &d, draw_state),
    }
}

/// The model under `mnMapsMakePreviewCamera`'s camera, its root scaled by
/// `mnMapsMakeLayer`'s `scales`.
#[inline(never)]
unsafe fn draw_model(
    gpu: &mut Gpu,
    p: &Pack<'_>,
    draw_state: &mut meshdraw::DrawState,
    layer: &Layer,
    preview: &Preview,
) {
    let (Some(model), Some(stage)) = (layer.model(), preview.stage.and_then(|i| p.stage(i))) else {
        return;
    };
    let cam = layer.camera();
    gpu.set_viewport_n64(cam.viewport);
    gpu.set_perspective(cam.fovy, cam.aspect, cam.near, cam.far);
    gpu.reset_modelview();
    draw_state.begin_frame();
    gpu.set_view(&ssb_engine::math::Mat4::look_at(cam.eye, cam.at, cam.up));
    gpu.model_transform([0.0; 3], [0.0; 3], meshdraw::MODEL_SCALE * model.scale);
    let base = gpu.model_matrix();
    draw_state.set_stage_light(stage.light_angle_xy);
    meshdraw::draw_stage_preview(
        p,
        &stage,
        &base,
        &preview.animator,
        &preview.hide[..preview.hide_len],
        draw_state,
        Some(&preview.mat_anim),
    );
}
