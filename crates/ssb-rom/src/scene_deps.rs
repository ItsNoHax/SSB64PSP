//! Scene dependency graph: `scene → objects → nodes → meshes → primitives →
//! materials → textures → palettes`, resolved from a built pack.
//!
//! A scene is the set of objects it draws (a stage's layers, each fighter in
//! its costume, effects). [`SceneDeps`] follows the same references the draw
//! path follows:
//!
//! * an object owns `first_node..first_node + node_count`;
//! * a node draws its costume's override mesh when one exists, else its own
//!   mesh (`Pack::costume_mesh`, as `meshdraw::draw_object_posed` does);
//! * a mesh owns `first_prim..first_prim + prim_count`;
//! * a primitive names a texture and may name a material animation;
//! * a texture may name the material animation that cycles its palette;
//! * a material animation names its runtime-selectable sprites, its palette
//!   variants and, for a fractional image blend, the tile-1 textures in its
//!   `LodBlendDesc`.
//!
//! The result is the exact set of textures and CLUTs a scene can sample,
//! which per-scene texture residency (RE-076/RE-077) needs to budget VRAM.

use alloc::collections::BTreeSet;

use crate::pack::{NodeDesc, Pack, PrimDesc, TextureDesc};

/// Everything one scene can draw.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SceneDeps {
    pub objects: BTreeSet<u32>,
    pub nodes: BTreeSet<u32>,
    pub meshes: BTreeSet<u32>,
    pub prims: BTreeSet<u32>,
    pub textures: BTreeSet<u32>,
    pub mat_anims: BTreeSet<u32>,
    /// `MatAnimPalette` entries (palette variants of animated textures).
    pub mat_anim_palettes: BTreeSet<u32>,
    /// References the pack could not resolve; see `crate::strict`.
    pub unresolved: usize,
}

/// Byte totals for a [`SceneDeps`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Footprint {
    /// Texel bytes, all mip levels.
    pub texel_bytes: u64,
    /// Each texture's own CLUT, in bytes.
    pub palette_bytes: u64,
    /// Animated palette variants, in bytes.
    pub mat_anim_palette_bytes: u64,
}

impl Footprint {
    pub fn total(&self) -> u64 {
        self.texel_bytes + self.palette_bytes + self.mat_anim_palette_bytes
    }
}

impl SceneDeps {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds `object` drawn in `costume` (0 for the base model).
    pub fn add_object(&mut self, pack: &Pack<'_>, object: u32, costume: u32) {
        let Some(o) = pack.object(object) else {
            self.unresolved += 1;
            return;
        };
        self.objects.insert(object);
        for node in o.first_node..o.first_node.saturating_add(o.node_count) {
            let Some(n) = pack.node(node) else {
                self.unresolved += 1;
                continue;
            };
            self.nodes.insert(node);
            let mesh = pack.costume_mesh(node, costume).unwrap_or(n.mesh);
            if mesh != NodeDesc::NO_MESH {
                self.add_mesh(pack, mesh);
            }
        }
    }

    /// Adds a stage's layer objects (`StageDesc::layers`, `u32::MAX` for an
    /// empty layer).
    pub fn add_stage(&mut self, pack: &Pack<'_>, stage: u32) {
        let Some(s) = pack.stage(stage) else {
            self.unresolved += 1;
            return;
        };
        for layer in s.layers {
            if layer != u32::MAX {
                self.add_object(pack, layer, 0);
            }
        }
    }

    fn add_mesh(&mut self, pack: &Pack<'_>, mesh: u32) {
        let Some(m) = pack.mesh(mesh) else {
            self.unresolved += 1;
            return;
        };
        if !self.meshes.insert(mesh) {
            return;
        }
        for prim in m.first_prim..m.first_prim.saturating_add(m.prim_count) {
            let Some(p) = pack.prim(prim) else {
                self.unresolved += 1;
                continue;
            };
            self.prims.insert(prim);
            if p.texture != PrimDesc::NO_TEXTURE {
                self.add_texture(pack, p.texture);
            }
            if p.mat_anim != TextureDesc::NO_ANIM {
                self.add_mat_anim(pack, p.mat_anim);
            }
        }
    }

    fn add_texture(&mut self, pack: &Pack<'_>, texture: u32) {
        let Some(t) = pack.texture(texture) else {
            self.unresolved += 1;
            return;
        };
        if !self.textures.insert(texture) {
            return;
        }
        if t.mat_anim != TextureDesc::NO_ANIM {
            self.add_mat_anim(pack, t.mat_anim);
        }
    }

    fn add_mat_anim(&mut self, pack: &Pack<'_>, mat_anim: u32) {
        let Some(a) = pack.mat_anim(mat_anim) else {
            self.unresolved += 1;
            return;
        };
        if !self.mat_anims.insert(mat_anim) {
            return;
        }
        let sprites = (a.texture_count as usize).min(a.textures.len());
        for &t in &a.textures[..sprites] {
            if t != TextureDesc::NO_ANIM {
                self.add_texture(pack, t);
            }
        }
        for i in a.first_palette..a.first_palette.saturating_add(a.palette_count) {
            self.mat_anim_palettes.insert(i);
        }
        if let Some(lod) = pack.lod_blend(mat_anim) {
            let next = (lod.next_count as usize).min(lod.next_textures.len());
            for &t in &lod.next_textures[..next] {
                if t != TextureDesc::NO_ANIM {
                    self.add_texture(pack, t);
                }
            }
        }
    }

    /// Texel and palette bytes the scene's textures occupy.
    pub fn footprint(&self, pack: &Pack<'_>) -> Footprint {
        let mut f = Footprint::default();
        for &i in &self.textures {
            if let Some(t) = pack.texture(i) {
                f.texel_bytes += u64::from(t.data_len);
                f.palette_bytes += u64::from(t.palette_len) * 4;
            }
        }
        for &i in &self.mat_anim_palettes {
            if let Some(p) = pack.mat_anim_palette(i) {
                f.mat_anim_palette_bytes += u64::from(p.palette_len) * 4;
            }
        }
        f
    }
}
