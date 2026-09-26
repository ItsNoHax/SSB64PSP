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
//! * a costume override naming a node or mesh past its table.

use crate::pack::{NodeDesc, Pack, PrimDesc, TextureDesc};
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
            }
        }
    }

    impl Source for Fake {
        fn texture_count(&self) -> u32 {
            self.textures.len() as u32
        }
        fn mat_anim_count(&self) -> u32 {
            0
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
}
