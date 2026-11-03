//! [`super`]'s passes for a run of display effects of one object, each at
//! its own root: the small shocks of an electric hit
//! (`efManagerShockSmallMakeEffect`), which the game draws one at a time
//! through [`super::super::draw_object_posed`] with its own material player.
//!
//! [`DisplayBatch::new`] poses and expands every display of the run;
//! the caller then replays each display's material player and
//! [`DisplayBatch::submit`]s it, in the run's order. The pose and the
//! expansion read no material state: the expansion's colours are
//! `color_override`'s, which [`DisplayBatch::new`] requires unset, so the
//! per-object path's `material_colors` are the player's, which must set no
//! PRIM or ENV colour (the caller's object). [`plan_display`] copies
//! [`super::plan_object`], whose only caller inlines it (RE-494).

use psp::sys::ScePspFMatrix4;
use ssb_rom::pack::{NodeDesc, ObjectDesc, Pack, PackedVertex};
use ssb_rom::skeleton::{EffectColors, EffectMaterialAnimator};

use super::super::DrawState;
use super::{
    batch_expand, batch_pose, batch_ready, batch_submit, Batch, BatchSize, UNBATCHED_FLAGS,
};

/// [`super::plan_object`] for one display; `false` when it does not take
/// these passes.
#[inline(never)]
fn plan_display(pack: &Pack<'_>, object: &ObjectDesc, size: &mut BatchSize) -> bool {
    for i in 0..object.node_count {
        let Some(node) = pack.node(object.first_node + i) else {
            continue;
        };
        // Costume 0 without parts draws the node's own mesh, no accessory.
        if node.mesh == NodeDesc::NO_MESH {
            continue;
        }
        if node.flags & NodeDesc::FLAG_BILLBOARD != 0 {
            return false;
        }
        let Some(mesh) = pack.mesh(node.mesh) else {
            return false;
        };
        let Some(verts) = pack.vertices(&mesh) else {
            return false;
        };
        for j in 0..mesh.prim_count {
            if pack
                .prim(mesh.first_prim + j)
                .is_some_and(|p| p.flags & UNBATCHED_FLAGS != 0)
            {
                return false;
            }
        }
        size.meshes += 1;
        size.prims += mesh.prim_count as usize;
        size.verts += verts.len() / core::mem::size_of::<PackedVertex>();
    }
    true
}

/// A run of displays of one object, posed and expanded, submitted one
/// display at a time.
pub struct DisplayBatch {
    batch: Batch,
    /// Meshes per display.
    per: usize,
    /// The next display's first mesh.
    next: usize,
}

impl DisplayBatch {
    /// The run of `object` drawn at each of `bases` as
    /// [`super::super::draw_object_posed`] draws it with `posed`, no
    /// billboard scales, costume 0 and no stage material player; `None`
    /// (nothing drawn or allocated) for fewer than two displays, a state or
    /// object the passes do not take, a set `color_override`, or a run past
    /// their scratch.
    ///
    /// # Safety
    ///
    /// Between `sceGuStart` and `sceGuFinish` of the direct list.
    #[inline(never)]
    pub unsafe fn new(
        pack: &Pack<'_>,
        st: &DrawState,
        object: &ObjectDesc,
        bases: &[ScePspFMatrix4],
        posed: &[ssb_rom::scene::Mat4],
    ) -> Option<Self> {
        if bases.len() < 2 || !batch_ready(st) || st.color_override.is_some() {
            return None;
        }
        let mut one = BatchSize::default();
        if !plan_display(pack, object, &mut one) {
            return None;
        }
        let n = bases.len();
        let size = BatchSize {
            meshes: one.meshes * n,
            prims: one.prims * n,
            verts: one.verts * n,
        };
        if !size.fits() {
            return None;
        }
        let mut batch = Batch::new(&size);
        let mut per = 0;
        for base in bases {
            per = batch_pose(
                pack,
                object,
                base,
                posed,
                st,
                EffectColors::default(),
                &mut batch,
            );
        }
        batch_expand(pack, st, &mut batch);
        Some(DisplayBatch {
            batch,
            per,
            next: 0,
        })
    }

    /// The next display's draw, under its material player `mats`.
    ///
    /// # Safety
    ///
    /// As [`Self::new`], in the same list; once per display, in order.
    #[inline(never)]
    pub unsafe fn submit(
        &mut self,
        pack: &Pack<'_>,
        st: &mut DrawState,
        mats: Option<&EffectMaterialAnimator>,
    ) {
        batch_submit(pack, st, &self.batch, self.next, self.per, mats);
        self.next += self.per;
    }

    /// Ends the run ([`Batch::finish`]).
    ///
    /// # Safety
    ///
    /// As [`Self::new`], in the same list, after the last
    /// [`Self::submit`].
    #[inline(never)]
    pub unsafe fn finish(self) {
        self.batch.finish();
    }
}
