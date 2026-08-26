//! Rendering abstraction.
//!
//! The game submits meshes and materials; the backend decides how to get them
//! onto a screen. Nothing here mentions `sceGu`, and nothing here knows what a
//! fighter is.

use crate::math::{Mat4, Vec3};

/// Handle to a texture the backend has uploaded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextureId(pub u32);

/// Handle to a vertex/index buffer pair the backend owns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MeshId(pub u32);

/// RGBA colour, 0-255 per channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const WHITE: Color = Color::rgba(255, 255, 255, 255);
    pub const BLACK: Color = Color::rgba(0, 0, 0, 255);
    pub const TRANSPARENT: Color = Color::rgba(0, 0, 0, 0);

    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Color {
        Color { r, g, b, a }
    }

    /// Packs to the 0xAABBGGRR word `sceGu` expects.
    pub const fn to_abgr(self) -> u32 {
        (self.a as u32) << 24 | (self.b as u32) << 16 | (self.g as u32) << 8 | self.r as u32
    }
}

/// How a surface blends against what is already in the framebuffer.
///
/// These are the modes the N64 render modes in Smash actually reduce to; the
/// full RDP blender is far more expressive than the game uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BlendMode {
    /// Write the source directly.
    #[default]
    Opaque,
    /// Standard `src_alpha, 1 - src_alpha` transparency.
    Alpha,
    /// Additive, for energy effects and flashes.
    Additive,
}

/// Face culling, mirroring `G_CULL_FRONT` / `G_CULL_BACK`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CullMode {
    None,
    #[default]
    Back,
    Front,
}

/// The state a draw call is issued under.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Material {
    pub texture: Option<TextureId>,
    pub blend: BlendMode,
    pub cull: CullMode,
    /// Multiplied into vertex colour. Carries the RDP primitive colour.
    pub prim_color: Color,
    /// The RDP environment colour, used by several of Smash's combiners.
    pub env_color: Color,
    pub depth_test: bool,
    pub depth_write: bool,
    /// Whether vertex colours carry lighting normals instead of colours.
    pub lit: bool,
}

/// A vertex in the backend's preferred layout.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Vertex {
    pub pos: Vec3,
    pub uv: [f32; 2],
    pub color: Color,
}

/// Per-frame statistics, for the on-screen profiler.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RenderStats {
    pub draw_calls: u32,
    pub triangles: u32,
    pub texture_binds: u32,
    pub state_changes: u32,
}

/// What the game needs from a rendering backend.
pub trait Renderer {
    /// Begins a frame, acquiring a display list and clearing as configured.
    fn begin_frame(&mut self, clear: Option<Color>);

    /// Finishes the frame and presents it.
    fn end_frame(&mut self);

    /// Sets the camera for subsequent draws.
    fn set_camera(&mut self, view: Mat4, projection: Mat4);

    /// Restricts drawing to a screen-space rectangle.
    fn set_scissor(&mut self, x: u32, y: u32, w: u32, h: u32);

    /// Draws an uploaded mesh with the given world transform and material.
    fn draw_mesh(&mut self, mesh: MeshId, transform: Mat4, material: &Material);

    /// Draws immediate-mode geometry. Used by debug visualisation and UI,
    /// where building a persistent mesh is not worth it.
    fn draw_immediate(&mut self, verts: &[Vertex], indices: &[u16], material: &Material);

    /// Draws a 2D screen-space quad, for HUD and menus.
    fn draw_sprite(&mut self, x: f32, y: f32, w: f32, h: f32, tex: TextureId, tint: Color);

    /// Statistics for the frame just submitted.
    fn stats(&self) -> RenderStats;
}

/// Debug overlays the development build can toggle. Kept in Layer B so the
/// game can ask for them without knowing how they are drawn.
pub trait DebugDraw {
    fn line(&mut self, from: Vec3, to: Vec3, color: Color);
    fn wire_box(&mut self, center: Vec3, half_extents: Vec3, color: Color);
    /// Smash's hitboxes and hurtboxes are spheres and capsules, so this is the
    /// primitive collision debugging actually needs.
    fn wire_sphere(&mut self, center: Vec3, radius: f32, color: Color);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_packs_to_abgr_for_gu() {
        let c = Color::rgba(0x11, 0x22, 0x33, 0x44);
        assert_eq!(c.to_abgr(), 0x4433_2211);
        assert_eq!(Color::WHITE.to_abgr(), 0xFFFF_FFFF);
        assert_eq!(Color::TRANSPARENT.to_abgr(), 0x0000_0000);
    }

    #[test]
    fn default_material_is_opaque_backface_culled() {
        let m = Material::default();
        assert_eq!(m.blend, BlendMode::Opaque);
        assert_eq!(m.cull, CullMode::Back);
        assert!(m.texture.is_none());
    }
}
