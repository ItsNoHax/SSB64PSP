//! Strict rendering checks: every reference a draw resolves, checked up
//! front instead of skipped at draw time.
//!
//! The PSP draw path (`psp-runtime/src/meshdraw.rs`) tolerates a missing
//! texture, palette, mesh or node by drawing untextured or skipping the
//! piece, so a converter bug shows up as a subtly wrong frame. [`check`]
//! walks the same references a draw follows and reports each one that would
//! not resolve. `romtool strict <pack.pak>` runs it over a built pack, and
//! the `strict_render` feature of `psp-runtime` refuses to start on a pack
//! that fails it.
//!
//! What counts as unresolved:
//!
//! * a primitive naming a texture or material animation past the table;
//! * a texture whose texels are outside the blob (framebuffer textures are
//!   filled at run time and have none by design), or a paletted texture with
//!   no CLUT;
//! * a node whose mesh is past its table, whose parent does not precede it
//!   inside the same object (`Skeleton` composes parents first and leaves an
//!   unresolvable parent out), or whose rest transform or world matrix is
//!   not finite;
//! * a mesh whose vertices, primitives or indices are outside the pack;
//! * a costume override naming a node or mesh past its table;
//! * a material animation whose script file is outside the blob, whose
//!   palette range runs past the palette table, or whose sprite list names a
//!   texture past its table (`TextureIDCurrent` selects from it);
//! * an animated palette whose CLUT is outside the blob;
//! * a two-tile blend record keyed by a material animation past its table,
//!   or naming a `TextureIDNext` texture past the texture table.

use crate::pack::{LodBlendDesc, MatAnimDesc, NodeDesc, Pack, PrimDesc, TextureDesc};
use crate::psp_texture::Psm;

/// One reference that would not resolve at draw time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Issue {
    PrimTexture { prim: u32, texture: u32 },
    PrimMatAnim { prim: u32, mat_anim: u32 },
    TextureData { texture: u32 },
    TexturePalette { texture: u32 },
    TextureMatAnim { texture: u32, mat_anim: u32 },
    NodeMesh { node: u32, mesh: u32 },
    NodeParent { node: u32, parent: u32 },
    NodeTransform { node: u32 },
    MeshVertices { mesh: u32 },
    MeshPrims { mesh: u32 },
    PrimIndices { prim: u32 },
    CostumeOverride { index: u32 },
    MatAnimScript { mat_anim: u32 },
    MatAnimPalettes { mat_anim: u32 },
    MatAnimTexture { mat_anim: u32, texture: u32 },
    MatAnimPaletteData { palette: u32 },
    LodBlendMatAnim { index: u32, mat_anim: u32 },
    LodBlendTexture { mat_anim: u32, texture: u32 },
}

impl core::fmt::Display for Issue {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match *self {
            Issue::PrimTexture { prim, texture } => {
                write!(
                    f,
                    "primitive {prim} names texture {texture}, past the table"
                )
            }
            Issue::PrimMatAnim { prim, mat_anim } => {
                write!(
                    f,
                    "primitive {prim} names material animation {mat_anim}, past the table"
                )
            }
            Issue::TextureData { texture } => write!(f, "texture {texture} has no texel data"),
            Issue::TexturePalette { texture } => {
                write!(f, "paletted texture {texture} has no palette")
            }
            Issue::TextureMatAnim { texture, mat_anim } => write!(
                f,
                "texture {texture} names material animation {mat_anim}, past the table"
            ),
            Issue::NodeMesh { node, mesh } => {
                write!(f, "node {node} names mesh {mesh}, past the table")
            }
            Issue::NodeParent { node, parent } => {
                write!(
                    f,
                    "node {node} names parent {parent}, which does not precede it"
                )
            }
            Issue::NodeTransform { node } => write!(f, "node {node} has a non-finite transform"),
            Issue::MeshVertices { mesh } => write!(f, "mesh {mesh} vertices are outside the pack"),
            Issue::MeshPrims { mesh } => write!(f, "mesh {mesh} primitives are past the table"),
            Issue::PrimIndices { prim } => {
                write!(f, "primitive {prim} indices are outside the pack")
            }
            Issue::MatAnimScript { mat_anim } => write!(
                f,
                "material animation {mat_anim} script is outside its file or the pack"
            ),
            Issue::MatAnimPalettes { mat_anim } => write!(
                f,
                "material animation {mat_anim} palette range runs past the table"
            ),
            Issue::MatAnimTexture { mat_anim, texture } => write!(
                f,
                "material animation {mat_anim} names texture {texture}, past the table"
            ),
            Issue::MatAnimPaletteData { palette } => {
                write!(f, "animated palette {palette} is outside the pack")
            }
            Issue::LodBlendMatAnim { index, mat_anim } => write!(
                f,
                "two-tile blend {index} names material animation {mat_anim}, past the table"
            ),
            Issue::LodBlendTexture { mat_anim, texture } => write!(
                f,
                "two-tile blend for material animation {mat_anim} names texture {texture}, past the table"
            ),
            Issue::CostumeOverride { index } => {
                write!(
                    f,
                    "costume override {index} names a node or mesh past its table"
                )
            }
        }
    }
}

/// The pack queries [`check`] needs. [`Pack`] implements it; tests use a
/// synthetic source to reach states a well-formed writer cannot produce.
pub trait Source {
    fn texture_count(&self) -> u32;
    fn mat_anim_count(&self) -> u32;
    fn mesh_count(&self) -> u32;
    fn node_count(&self) -> u32;
    fn prim_count(&self) -> u32;
    fn costume_override_count(&self) -> u32;
    fn object_count(&self) -> u32;
    /// `(first_node, node_count)`.
    fn object(&self, i: u32) -> Option<(u32, u32)>;
    fn texture(&self, i: u32) -> Option<TextureDesc>;
    fn has_texture_data(&self, t: &TextureDesc) -> bool;
    fn has_palette_data(&self, t: &TextureDesc) -> bool;
    fn node(&self, i: u32) -> Option<NodeDesc>;
    /// `(first_prim, prim_count, vertices resolve)`.
    fn mesh(&self, i: u32) -> Option<(u32, u32, bool)>;
    fn prim(&self, i: u32) -> Option<PrimDesc>;
    fn has_indices(&self, p: &PrimDesc) -> bool;
    /// `(node, mesh)`.
    fn costume_override(&self, i: u32) -> Option<(u32, u32)>;
    fn mat_anim(&self, i: u32) -> Option<MatAnimDesc>;
    /// Whether the script's file bytes resolve and contain `a.script`.
    fn has_mat_anim_script(&self, a: &MatAnimDesc) -> bool;
    fn mat_anim_palette_count(&self) -> u32;
    fn has_mat_anim_palette_data(&self, i: u32) -> bool;
    fn lod_blend_count(&self) -> u32;
    fn lod_blend(&self, i: u32) -> Option<LodBlendDesc>;
}

impl Source for Pack<'_> {
    fn texture_count(&self) -> u32 {
        Pack::texture_count(self)
    }
    fn mat_anim_count(&self) -> u32 {
        Pack::mat_anim_count(self)
    }
    fn mesh_count(&self) -> u32 {
        Pack::mesh_count(self)
    }
    fn node_count(&self) -> u32 {
        Pack::node_count(self)
    }
    fn prim_count(&self) -> u32 {
        Pack::prim_count(self)
    }
    fn costume_override_count(&self) -> u32 {
        Pack::costume_override_count(self)
    }
    fn object_count(&self) -> u32 {
        Pack::object_count(self)
    }
    fn object(&self, i: u32) -> Option<(u32, u32)> {
        Pack::object(self, i).map(|o| (o.first_node, o.node_count))
    }
    fn texture(&self, i: u32) -> Option<TextureDesc> {
        Pack::texture(self, i)
    }
    fn has_texture_data(&self, t: &TextureDesc) -> bool {
        self.texture_data(t).is_some_and(|d| !d.is_empty())
    }
    fn has_palette_data(&self, t: &TextureDesc) -> bool {
        t.palette_len > 0 && self.palette_data(t).is_some_and(|d| !d.is_empty())
    }
    fn node(&self, i: u32) -> Option<NodeDesc> {
        Pack::node(self, i)
    }
    fn mesh(&self, i: u32) -> Option<(u32, u32, bool)> {
        Pack::mesh(self, i).map(|m| {
            let verts = m.vertex_count == 0 || self.vertices(&m).is_some();
            (m.first_prim, m.prim_count, verts)
        })
    }
    fn prim(&self, i: u32) -> Option<PrimDesc> {
        Pack::prim(self, i)
    }
    fn has_indices(&self, p: &PrimDesc) -> bool {
        p.index_count == 0 || self.indices(p).is_some()
    }
    fn costume_override(&self, i: u32) -> Option<(u32, u32)> {
        Pack::costume_override(self, i).map(|o| (o.node, o.mesh))
    }
    fn mat_anim(&self, i: u32) -> Option<MatAnimDesc> {
        Pack::mat_anim(self, i)
    }
    fn has_mat_anim_script(&self, a: &MatAnimDesc) -> bool {
        self.mat_anim_file(a)
            .is_some_and(|d| (a.script as usize) < d.len())
    }
    fn mat_anim_palette_count(&self) -> u32 {
        Pack::mat_anim_palette_count(self)
    }
    fn has_mat_anim_palette_data(&self, i: u32) -> bool {
        self.mat_anim_palette(i)
            .and_then(|p| self.mat_anim_palette_data(&p))
            .is_some_and(|d| !d.is_empty())
    }
    fn lod_blend_count(&self) -> u32 {
        Pack::lod_blend_count(self)
    }
    fn lod_blend(&self, i: u32) -> Option<LodBlendDesc> {
        self.lod_blend_at(i)
    }
}

fn is_paletted(psm: u8) -> bool {
    psm == Psm::PsmT4 as u8 || psm == Psm::PsmT8 as u8
}

fn finite(values: &[f32]) -> bool {
    values.iter().all(|v| v.is_finite())
}

/// Reports every unresolved reference to `report`, in table order, and
/// returns how many there were.
pub fn check<S: Source + ?Sized>(pack: &S, mut report: impl FnMut(Issue)) -> usize {
    let mut n = 0;
    let mut issue = |i: Issue| {
        n += 1;
        report(i);
    };
    let textures = pack.texture_count();
    let mat_anims = pack.mat_anim_count();
    let meshes = pack.mesh_count();
    let nodes = pack.node_count();
    let prims = pack.prim_count();

    for i in 0..textures {
        let Some(t) = pack.texture(i) else {
            issue(Issue::TextureData { texture: i });
            continue;
        };
        if t.role != TextureDesc::ROLE_FRAMEBUFFER && !pack.has_texture_data(&t) {
            issue(Issue::TextureData { texture: i });
        }
        if t.role != TextureDesc::ROLE_FRAMEBUFFER
            && is_paletted(t.psm)
            && !pack.has_palette_data(&t)
        {
            issue(Issue::TexturePalette { texture: i });
        }
        if t.mat_anim != TextureDesc::NO_ANIM && t.mat_anim >= mat_anims {
            issue(Issue::TextureMatAnim {
                texture: i,
                mat_anim: t.mat_anim,
            });
        }
    }

    for i in 0..prims {
        let Some(p) = pack.prim(i) else {
            issue(Issue::PrimIndices { prim: i });
            continue;
        };
        if p.texture != PrimDesc::NO_TEXTURE && p.texture >= textures {
            issue(Issue::PrimTexture {
                prim: i,
                texture: p.texture,
            });
        }
        if p.mat_anim != TextureDesc::NO_ANIM && p.mat_anim >= mat_anims {
            issue(Issue::PrimMatAnim {
                prim: i,
                mat_anim: p.mat_anim,
            });
        }
        if !pack.has_indices(&p) {
            issue(Issue::PrimIndices { prim: i });
        }
    }

    for i in 0..meshes {
        let Some((first, count, verts)) = pack.mesh(i) else {
            issue(Issue::MeshVertices { mesh: i });
            continue;
        };
        if !verts {
            issue(Issue::MeshVertices { mesh: i });
        }
        if u64::from(first) + u64::from(count) > u64::from(prims) {
            issue(Issue::MeshPrims { mesh: i });
        }
    }

    for i in 0..nodes {
        let Some(node) = pack.node(i) else {
            issue(Issue::NodeTransform { node: i });
            continue;
        };
        if node.mesh != NodeDesc::NO_MESH && node.mesh >= meshes {
            issue(Issue::NodeMesh {
                node: i,
                mesh: node.mesh,
            });
        }
        if node.parent != NodeDesc::NO_PARENT && node.parent >= i {
            issue(Issue::NodeParent {
                node: i,
                parent: node.parent,
            });
        }
        if !(finite(&node.world)
            && finite(&node.rest_translate)
            && finite(&node.rest_rotate)
            && finite(&node.rest_scale))
        {
            issue(Issue::NodeTransform { node: i });
        }
    }

    for o in 0..pack.object_count() {
        let Some((first, count)) = pack.object(o) else {
            continue;
        };
        for i in first..first.saturating_add(count).min(nodes) {
            if let Some(node) = pack.node(i) {
                if node.parent != NodeDesc::NO_PARENT && node.parent < i && node.parent < first {
                    issue(Issue::NodeParent {
                        node: i,
                        parent: node.parent,
                    });
                }
            }
        }
    }

    for i in 0..pack.costume_override_count() {
        match pack.costume_override(i) {
            Some((node, mesh)) if node < nodes && mesh < meshes => {}
            _ => issue(Issue::CostumeOverride { index: i }),
        }
    }

    let palettes = pack.mat_anim_palette_count();
    for i in 0..mat_anims {
        let Some(a) = pack.mat_anim(i) else {
            issue(Issue::MatAnimScript { mat_anim: i });
            continue;
        };
        if !pack.has_mat_anim_script(&a) {
            issue(Issue::MatAnimScript { mat_anim: i });
        }
        if u64::from(a.first_palette) + u64::from(a.palette_count) > u64::from(palettes) {
            issue(Issue::MatAnimPalettes { mat_anim: i });
        }
        // `resolve_texture` clamps into `textures[..texture_count]` and
        // treats `NO_ANIM` as "keep the primitive's own image".
        let count = (a.texture_count as usize).min(MatAnimDesc::MAX_TEXTURES);
        for &t in &a.textures[..count] {
            if t != TextureDesc::NO_ANIM && t >= textures {
                issue(Issue::MatAnimTexture {
                    mat_anim: i,
                    texture: t,
                });
            }
        }
        if a.texture_count as usize > MatAnimDesc::MAX_TEXTURES {
            issue(Issue::MatAnimTexture {
                mat_anim: i,
                texture: a.texture_count,
            });
        }
    }
    for i in 0..palettes {
        if !pack.has_mat_anim_palette_data(i) {
            issue(Issue::MatAnimPaletteData { palette: i });
        }
    }

    for i in 0..pack.lod_blend_count() {
        let Some(lod) = pack.lod_blend(i) else {
            issue(Issue::LodBlendMatAnim {
                index: i,
                mat_anim: TextureDesc::NO_ANIM,
            });
            continue;
        };
        if lod.mat_anim >= mat_anims {
            issue(Issue::LodBlendMatAnim {
                index: i,
                mat_anim: lod.mat_anim,
            });
        }
        let count = (lod.next_count as usize).min(MatAnimDesc::MAX_TEXTURES);
        for &t in &lod.next_textures[..count] {
            if t >= textures {
                issue(Issue::LodBlendTexture {
                    mat_anim: lod.mat_anim,
                    texture: t,
                });
            }
        }
    }
    n
}

/// The first unresolved reference, for callers that stop at one.
pub fn first_issue<S: Source + ?Sized>(pack: &S) -> Option<Issue> {
    let mut first = None;
    check(pack, |i| {
        if first.is_none() {
            first = Some(i);
        }
    });
    first
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pack::PackWriter;
    use crate::psp_texture::PspTexture;
    use alloc::vec::Vec;

    #[test]
    fn a_well_formed_pack_is_clean() {
        let mut w = PackWriter::new();
        w.add_texture(
            &PspTexture {
                width: 32,
                height: 8,
                stride: 32,
                format: Psm::PsmT4,
                data: alloc::vec![0xABu8; 128],
                swizzled: true,
                palette: alloc::vec![0xFF00_00FFu32; 16],
                levels: 1,
            },
            false,
            false,
        );
        w.add_framebuffer_texture(64, 32);
        let bytes = w.finish();
        let pack = Pack::open(&bytes).unwrap();
        assert_eq!(first_issue(&pack), None);
    }

    /// A synthetic pack with one of everything, which each test breaks in
    /// one way.
    struct Fake {
        textures: Vec<TextureDesc>,
        texel_data: bool,
        palette_data: bool,
        nodes: Vec<NodeDesc>,
        meshes: Vec<(u32, u32, bool)>,
        prims: Vec<PrimDesc>,
        overrides: Vec<(u32, u32)>,
        objects: Vec<(u32, u32)>,
        mat_anims: Vec<MatAnimDesc>,
        script_data: bool,
        palettes: Vec<bool>,
        lod_blends: Vec<LodBlendDesc>,
    }

    impl Fake {
        fn new() -> Self {
            let mut texture = Pack::open(&{
                let mut w = PackWriter::new();
                w.add_texture(
                    &PspTexture {
                        width: 16,
                        height: 16,
                        stride: 16,
                        format: Psm::PsmT8,
                        data: alloc::vec![0; 256],
                        swizzled: false,
                        palette: alloc::vec![0; 256],
                        levels: 1,
                    },
                    false,
                    false,
                );
                w.finish()
            })
            .unwrap()
            .texture(0)
            .unwrap();
            texture.mat_anim = TextureDesc::NO_ANIM;
            let node = NodeDesc {
                mesh: 0,
                parent: NodeDesc::NO_PARENT,
                world: [0.0; 16],
                rest_translate: [0.0; 3],
                rest_rotate: [0.0; 3],
                rest_scale: [1.0; 3],
                flags: 0,
            };
            // SAFETY: `PrimDesc` is `repr(C)` and every field is a plain
            // integer, so the all-zero bit pattern is a valid value.
            let mut prim: PrimDesc = unsafe { core::mem::zeroed() };
            prim.texture = 0;
            prim.mat_anim = TextureDesc::NO_ANIM;
            Fake {
                textures: alloc::vec![texture],
                texel_data: true,
                palette_data: true,
                nodes: alloc::vec![node],
                meshes: alloc::vec![(0, 1, true)],
                prims: alloc::vec![prim],
                overrides: alloc::vec![(0, 0)],
                objects: alloc::vec![(0, 1)],
                mat_anims: alloc::vec![MatAnimDesc {
                    palette_count: 1,
                    texture_count: 2,
                    textures: [0, TextureDesc::NO_ANIM, 0, 0, 0, 0, 0, 0],
                    ..MatAnimDesc::default()
                }],
                script_data: true,
                palettes: alloc::vec![true],
                lod_blends: alloc::vec![LodBlendDesc {
                    mat_anim: 0,
                    next_count: 1,
                    ..LodBlendDesc::default()
                }],
            }
        }
    }

    impl Source for Fake {
        fn texture_count(&self) -> u32 {
            self.textures.len() as u32
        }
        fn mat_anim_count(&self) -> u32 {
            self.mat_anims.len() as u32
        }
        fn mesh_count(&self) -> u32 {
            self.meshes.len() as u32
        }
        fn node_count(&self) -> u32 {
            self.nodes.len() as u32
        }
        fn prim_count(&self) -> u32 {
            self.prims.len() as u32
        }
        fn costume_override_count(&self) -> u32 {
            self.overrides.len() as u32
        }
        fn object_count(&self) -> u32 {
            self.objects.len() as u32
        }
        fn object(&self, i: u32) -> Option<(u32, u32)> {
            self.objects.get(i as usize).copied()
        }
        fn texture(&self, i: u32) -> Option<TextureDesc> {
            self.textures.get(i as usize).copied()
        }
        fn has_texture_data(&self, _: &TextureDesc) -> bool {
            self.texel_data
        }
        fn has_palette_data(&self, _: &TextureDesc) -> bool {
            self.palette_data
        }
        fn node(&self, i: u32) -> Option<NodeDesc> {
            self.nodes.get(i as usize).copied()
        }
        fn mesh(&self, i: u32) -> Option<(u32, u32, bool)> {
            self.meshes.get(i as usize).copied()
        }
        fn prim(&self, i: u32) -> Option<PrimDesc> {
            self.prims.get(i as usize).copied()
        }
        fn has_indices(&self, _: &PrimDesc) -> bool {
            true
        }
        fn costume_override(&self, i: u32) -> Option<(u32, u32)> {
            self.overrides.get(i as usize).copied()
        }
        fn mat_anim(&self, i: u32) -> Option<MatAnimDesc> {
            self.mat_anims.get(i as usize).copied()
        }
        fn has_mat_anim_script(&self, _: &MatAnimDesc) -> bool {
            self.script_data
        }
        fn mat_anim_palette_count(&self) -> u32 {
            self.palettes.len() as u32
        }
        fn has_mat_anim_palette_data(&self, i: u32) -> bool {
            self.palettes[i as usize]
        }
        fn lod_blend_count(&self) -> u32 {
            self.lod_blends.len() as u32
        }
        fn lod_blend(&self, i: u32) -> Option<LodBlendDesc> {
            self.lod_blends.get(i as usize).copied()
        }
    }

    fn issues(f: &Fake) -> Vec<Issue> {
        let mut out = Vec::new();
        check(f, |i| out.push(i));
        out
    }

    #[test]
    fn the_fake_starts_clean() {
        assert_eq!(issues(&Fake::new()), []);
    }

    #[test]
    fn unresolved_textures_and_palettes_are_reported() {
        let mut f = Fake::new();
        f.prims[0].texture = 7;
        f.palette_data = false;
        assert_eq!(
            issues(&f),
            [
                Issue::TexturePalette { texture: 0 },
                Issue::PrimTexture {
                    prim: 0,
                    texture: 7
                },
            ]
        );
        let mut f = Fake::new();
        f.texel_data = false;
        f.textures[0].mat_anim = 3;
        assert_eq!(
            issues(&f),
            [
                Issue::TextureData { texture: 0 },
                Issue::TextureMatAnim {
                    texture: 0,
                    mat_anim: 3
                },
            ]
        );
    }

    #[test]
    fn unresolved_transforms_and_meshes_are_reported() {
        let mut f = Fake::new();
        f.nodes[0].world[5] = f32::NAN;
        f.nodes[0].mesh = 4;
        f.meshes[0] = (0, 2, false);
        f.overrides[0] = (9, 0);
        assert_eq!(
            issues(&f),
            [
                Issue::MeshVertices { mesh: 0 },
                Issue::MeshPrims { mesh: 0 },
                Issue::NodeMesh { node: 0, mesh: 4 },
                Issue::NodeTransform { node: 0 },
                Issue::CostumeOverride { index: 0 },
            ]
        );
        let mut f = Fake::new();
        f.nodes.push(f.nodes[0]);
        f.nodes[0].parent = 1;
        assert_eq!(issues(&f), [Issue::NodeParent { node: 0, parent: 1 }]);
        // Node 1 starts a second object but names node 0 of the first.
        let mut f = Fake::new();
        f.nodes.push(f.nodes[0]);
        f.nodes[1].parent = 0;
        f.objects.push((1, 1));
        assert_eq!(issues(&f), [Issue::NodeParent { node: 1, parent: 0 }]);
    }

    #[test]
    fn unresolved_material_animations_are_reported() {
        let mut f = Fake::new();
        f.script_data = false;
        f.mat_anims[0].first_palette = 1;
        f.mat_anims[0].textures[0] = 5;
        f.palettes[0] = false;
        assert_eq!(
            issues(&f),
            [
                Issue::MatAnimScript { mat_anim: 0 },
                Issue::MatAnimPalettes { mat_anim: 0 },
                Issue::MatAnimTexture {
                    mat_anim: 0,
                    texture: 5
                },
                Issue::MatAnimPaletteData { palette: 0 },
            ]
        );
        // A sprite past `texture_count` is never selected.
        let mut f = Fake::new();
        f.mat_anims[0].textures[2] = 9;
        assert_eq!(issues(&f), []);
        let mut f = Fake::new();
        f.lod_blends[0].mat_anim = 3;
        f.lod_blends[0].next_textures[0] = 4;
        assert_eq!(
            issues(&f),
            [
                Issue::LodBlendMatAnim {
                    index: 0,
                    mat_anim: 3
                },
                Issue::LodBlendTexture {
                    mat_anim: 3,
                    texture: 4
                },
            ]
        );
    }
}
