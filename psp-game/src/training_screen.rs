//! Asset adapter for the portable Training interface layout.
use ssb_game::training::TrainingMenu;
use ssb_game::training_layer::{self, Draw, SpriteInfo};
use ssb_psp_runtime::meshdraw::{self, DrawState, SObjDraw};
use ssb_rom::pack::Pack;
use ssb_rom::sprite::{TRAINING_FILE, SP_TEXSHUF, SP_TRANSPARENT};

pub fn draw(p: &Pack<'_>, state: &mut DrawState, menu: &TrainingMenu, open: bool, held: u8) {
    let layout = p.training_layout();
    training_layer::visit(menu, &menu.stats, open, held, |slot| {
        let s = p.sprite(TRAINING_FILE, slot)?;
        let positioned = slot < 0x20 || (0xBC..0x10C).contains(&slot);
        let pos = if positioned {
            let at = slot.checked_sub(4)? as usize;
            let b = layout?.get(at..at + 4)?;
            [i16::from_be_bytes([b[0], b[1]]), i16::from_be_bytes([b[2], b[3]])]
        } else { [0; 2] };
        Some(SpriteInfo { size: [s.width, s.height], color: s.color, pos })
    }, |d| match d {
        Draw::Fill { rect, color } => unsafe { meshdraw::fill_rect_n64(rect, color, state) },
        Draw::Sprite { slot, pos, prim, env } => {
            if let Some(s) = p.sprite(TRAINING_FILE, slot) {
                let d = SObjDraw { x: pos[0], y: pos[1], scale: 1.0, prim, env,
                    solid: false, attr: SP_TEXSHUF | SP_TRANSPARENT };
                unsafe { meshdraw::draw_sprite(p, &s, &d, state); }
            }
        }
    });
}
