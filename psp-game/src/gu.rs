//! GU context: display setup, frame lifecycle, 2D UI drawing and the 3D
//! pipeline the training scene needs (depth test, culling, projection/view
//! matrices).
//!
//! `psp/`'s `gu.rs` is the fuller reference (adds LB-transition/1P-wallpaper
//! framebuffer capture, which `psp-game` has no use for). Kept as its own
//! copy rather than a shared module: `psp/` and `psp-game/` are separate
//! `cargo psp` crates outside the workspace (see both `Cargo.toml`s), and
//! `AGENTS.md`/`plans/gameplay/F1.md` require `psp/`'s own build and EBOOT to
//! stay unmodified by this work.
//!
//! Deliberately does **not** use `sceGuDebugPrint`/`sceGuDebugFlush`: RE-202
//! found that path reliably crashes real PSP hardware with an unaligned
//! instruction-fetch fault, which is why `psp/`'s equivalent text lives behind
//! an off-by-default `debug_overlay` feature. A front end that ships text
//! through that call every frame would carry the same fault unconditionally.
//! Real on-screen text for the menu is follow-up work through `sceFont`
//! (PGF glyph rasterisation into a GE texture) instead -- see `TODO.md`.

use core::ffi::c_void;

use psp::sys::{
    self, ClearBuffer, DepthFunc, DisplayPixelFormat, FrontFaceDirection, GuContextType,
    GuPrimitive, GuState, GuSyncBehavior, GuSyncMode, ShadingModel, TexturePixelFormat, VertexType,
};
use psp::vram_alloc::get_vram_allocator;
use psp::{Align16, BUF_WIDTH, SCREEN_HEIGHT, SCREEN_WIDTH};

use ssb_engine::renderer::Color;

/// Backing storage for [`transition_photo_data`], sized to match `psp/gu.rs`'s
/// own `TRANSITION_PHOTO_STRIDE` (512) x `TRANSITION_PHOTO_HEIGHT` (8), the
/// shape a `TextureDesc::ROLE_FRAMEBUFFER` entry in the shared pack declares.
static TRANSITION_PHOTO: Align16<[u32; 512 * 8]> = Align16([0; 512 * 8]);

/// Stand-in for `psp/gu.rs`'s own real LB-transition framebuffer capture:
/// always zero-filled, since `psp-game` has no LB-transition system yet to
/// call a `request_transition_capture` equivalent. Exists only so
/// `meshdraw::bind_texture`'s `ROLE_FRAMEBUFFER` branch -- copied verbatim,
/// see `meshdraw`'s own doc comment -- has something safe to bind; no packed
/// mesh this build currently draws references that role.
pub unsafe fn transition_photo_data() -> &'static [u8] {
    let ptr = core::ptr::addr_of!(TRANSITION_PHOTO.0) as *const u8;
    core::slice::from_raw_parts(ptr, core::mem::size_of_val(&TRANSITION_PHOTO.0))
}

/// A vertex laid out the way the GE wants it, for real 3D mesh drawing.
///
/// Field order is dictated by hardware: the GE reads texture coordinates,
/// then colour, then position, and the `VertexType` flags must describe
/// exactly that order. Verbatim copy of `psp/src/gu.rs`'s own type -- see this
/// module's doc comment for why it is a copy, not a shared one.
#[repr(C, align(4))]
#[derive(Clone, Copy)]
pub struct GuVertex {
    pub u: f32,
    pub v: f32,
    /// Packed ABGR, matching `Color::to_abgr`.
    pub color: u32,
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl GuVertex {
    /// The `VertexType` flags describing [`GuVertex`]'s layout.
    pub const FORMAT: VertexType = VertexType::from_bits_truncate(
        VertexType::TEXTURE_32BITF.bits()
            | VertexType::COLOR_8888.bits()
            | VertexType::VERTEX_32BITF.bits()
            | VertexType::TRANSFORM_3D.bits(),
    );

    pub const fn new(x: f32, y: f32, z: f32, u: f32, v: f32, color: u32) -> Self {
        GuVertex {
            u,
            v,
            color,
            x,
            y,
            z,
        }
    }
}

/// Display list scratch buffer. Sized like `psp/`'s own -- this build submits
/// far fewer, simpler primitives per frame, but there is no benefit to a
/// smaller buffer and a real cost to guessing too small.
static mut DISPLAY_LIST: Align16<[u32; 0x40000]> = Align16([0; 0x40000]);

/// A flat-shaded rectangle vertex in `GU_TRANSFORM_2D` screen space: raw pixel
/// coordinates, no MVP transform, no texture. Field order (colour, then
/// position) is dictated by hardware -- the GE reads whatever components
/// `VertexType` declares in a fixed order, texture first if present, then
/// colour, then position.
#[repr(C, align(4))]
#[derive(Clone, Copy)]
struct RectVertex {
    /// Packed ABGR, matching [`Color::to_abgr`].
    color: u32,
    x: i16,
    y: i16,
    z: i16,
    _pad: i16,
}

impl RectVertex {
    const FORMAT: VertexType = VertexType::from_bits_truncate(
        VertexType::COLOR_8888.bits()
            | VertexType::VERTEX_16BIT.bits()
            | VertexType::TRANSFORM_2D.bits(),
    );

    const fn new(x: i16, y: i16, color: u32) -> Self {
        RectVertex {
            color,
            x,
            y,
            z: 0,
            _pad: 0,
        }
    }
}

pub struct Gpu {
    frame_open: bool,
}

impl Gpu {
    /// Initialises the display and allocates the two colour buffers plus a
    /// depth buffer -- the training scene's real 3D drawing (`gpu.rs`'s
    /// `GuVertex` path, `meshdraw::draw_stage`/`draw_object_posed`) needs
    /// depth test on; the flat 2D menu/intro rectangles bracket it off around
    /// their own draw calls instead (see `draw_rect`) rather than the whole
    /// pipeline defaulting to 2D-only the way this build did before the
    /// scene-loading work in `plans/gameplay/F1.md`.
    ///
    /// # Safety
    ///
    /// Must be called exactly once, before any other GU use.
    pub unsafe fn init() -> Gpu {
        let allocator = get_vram_allocator().expect("VRAM allocator already taken");

        let fbp0 =
            allocator.alloc_texture_pixels(BUF_WIDTH, SCREEN_HEIGHT, TexturePixelFormat::Psm8888);
        let fbp1 =
            allocator.alloc_texture_pixels(BUF_WIDTH, SCREEN_HEIGHT, TexturePixelFormat::Psm8888);
        // 16-bit-per-pixel VRAM block for depth, matching `psp/gu.rs`'s own
        // `Psm4444` convention -- the GE reinterprets it as depth via
        // `sceGuDepthBuffer`.
        let zbp =
            allocator.alloc_texture_pixels(BUF_WIDTH, SCREEN_HEIGHT, TexturePixelFormat::Psm4444);

        sys::sceGuInit();
        sys::sceGuStart(GuContextType::Direct, Self::list_ptr());

        sys::sceGuDrawBuffer(
            DisplayPixelFormat::Psm8888,
            fbp0.as_mut_ptr_from_zero() as _,
            BUF_WIDTH as i32,
        );
        sys::sceGuDispBuffer(
            SCREEN_WIDTH as i32,
            SCREEN_HEIGHT as i32,
            fbp1.as_mut_ptr_from_zero() as _,
            BUF_WIDTH as i32,
        );
        sys::sceGuDepthBuffer(zbp.as_mut_ptr_from_zero() as _, BUF_WIDTH as i32);

        // The GE's screen space is centred on 2048; this offsets it so that
        // (0,0) is the top-left of the visible area -- matches `psp/`'s own
        // `sceGuOffset` call, which is what makes `RectVertex`'s raw pixel
        // coordinates land where they visually look like they should.
        sys::sceGuOffset(2048 - (SCREEN_WIDTH / 2), 2048 - (SCREEN_HEIGHT / 2));
        // Full-screen by default -- Intro/Menu keep exactly the viewport/
        // scissor RE-289/290 already pixel-confirmed. The training scene
        // switches to the N64-aspect pillarbox itself (`set_viewport_
        // pillarboxed`), matching `psp/gu.rs`'s own reasoning for why an
        // unpillarboxed 3D viewport measurably distorts the image.
        sys::sceGuViewport(2048, 2048, SCREEN_WIDTH as i32, SCREEN_HEIGHT as i32);
        sys::sceGuScissor(0, 0, SCREEN_WIDTH as i32, SCREEN_HEIGHT as i32);
        sys::sceGuEnable(GuState::ScissorTest);

        // The PSP's depth buffer is inverted relative to what you'd expect:
        // near maps to 65535, far to 0, so the depth test is GreaterOrEqual
        // (matches `psp/gu.rs`'s own measured convention).
        sys::sceGuDepthRange(65535, 0);
        sys::sceGuDepthFunc(DepthFunc::GreaterOrEqual);
        sys::sceGuEnable(GuState::DepthTest);

        sys::sceGuFrontFace(FrontFaceDirection::Clockwise);
        sys::sceGuShadeModel(ShadingModel::Smooth);
        sys::sceGuEnable(GuState::CullFace);
        sys::sceGuEnable(GuState::ClipPlanes);

        sys::sceGuDisable(GuState::Texture2D);
        sys::sceGuDisable(GuState::Lighting);

        sys::sceGuFinish();
        sys::sceGuSync(GuSyncMode::Finish, GuSyncBehavior::Wait);

        sys::sceDisplayWaitVblankStart();
        sys::sceGuDisplay(true);

        Gpu { frame_open: false }
    }

    /// Switches to the N64-aspect pillarboxed viewport/scissor the training
    /// scene's real 3D content needs -- `psp/gu.rs`'s own `Gpu::init` found
    /// (measured, not guessed) that a full-width 480px viewport stretches
    /// every character about a third too wide relative to the N64's 4:3
    /// projection. Call once when entering the training screen.
    pub fn set_viewport_pillarboxed(&mut self) {
        let (vx, _, vw, vh) = ssb_engine::coord::pillarboxed_viewport();
        unsafe {
            sys::sceGuViewport(2048, 2048, vw as i32, vh as i32);
            sys::sceGuScissor(vx as i32, 0, (vx + vw) as i32, vh as i32);
        }
    }

    /// Restores the full-screen viewport/scissor `init` set up, for the flat
    /// 2D intro/menu screens (RE-289/290's pixel-confirmed evidence assumes
    /// this shape). Call when leaving the training screen.
    pub fn set_viewport_fullscreen(&mut self) {
        unsafe {
            sys::sceGuViewport(2048, 2048, SCREEN_WIDTH as i32, SCREEN_HEIGHT as i32);
            sys::sceGuScissor(0, 0, SCREEN_WIDTH as i32, SCREEN_HEIGHT as i32);
        }
    }

    /// Sets the projection matrix from a field of view in degrees. Verbatim
    /// copy of `psp/gu.rs`'s own method.
    pub fn set_perspective(&mut self, fovy_degrees: f32, aspect: f32, near: f32, far: f32) {
        unsafe {
            sys::sceGumMatrixMode(sys::MatrixMode::Projection);
            sys::sceGumLoadIdentity();
            sys::sceGumPerspective(fovy_degrees, aspect, near, far);
        }
    }

    /// Resets view and model matrices to identity. Verbatim copy of
    /// `psp/gu.rs`'s own method.
    pub fn reset_modelview(&mut self) {
        unsafe {
            sys::sceGumMatrixMode(sys::MatrixMode::View);
            sys::sceGumLoadIdentity();
            sys::sceGumMatrixMode(sys::MatrixMode::Model);
            sys::sceGumLoadIdentity();
        }
    }

    /// Loads a real view matrix (RE-131) -- an `eye`/`at`/`up` camera, not
    /// just a translation. `m` is expected column-major, matching
    /// `ssb_engine::math::Mat4::as_array`'s own documented output. Verbatim
    /// copy of `psp/gu.rs`'s own method.
    pub fn set_view(&mut self, m: &ssb_engine::math::Mat4) {
        let a = m.as_array();
        let fm = sys::ScePspFMatrix4 {
            x: sys::ScePspFVector4 {
                x: a[0],
                y: a[1],
                z: a[2],
                w: a[3],
            },
            y: sys::ScePspFVector4 {
                x: a[4],
                y: a[5],
                z: a[6],
                w: a[7],
            },
            z: sys::ScePspFVector4 {
                x: a[8],
                y: a[9],
                z: a[10],
                w: a[11],
            },
            w: sys::ScePspFVector4 {
                x: a[12],
                y: a[13],
                z: a[14],
                w: a[15],
            },
        };
        unsafe {
            sys::sceGumMatrixMode(sys::MatrixMode::View);
            sys::sceGumLoadMatrix(&fm);
        }
    }

    /// Translates, rotates and uniformly scales the model matrix. Calls
    /// post-multiply, so the effective transform is `T * R * S`: vertices are
    /// scaled first, then rotated, then translated. Verbatim copy of
    /// `psp/gu.rs`'s own method.
    pub fn model_transform(&mut self, pos: [f32; 3], rot_radians: [f32; 3], scale: f32) {
        unsafe {
            sys::sceGumMatrixMode(sys::MatrixMode::Model);
            sys::sceGumLoadIdentity();
            sys::sceGumTranslate(&sys::ScePspFVector3 {
                x: pos[0],
                y: pos[1],
                z: pos[2],
            });
            sys::sceGumRotateXYZ(&sys::ScePspFVector3 {
                x: rot_radians[0],
                y: rot_radians[1],
                z: rot_radians[2],
            });
            sys::sceGumScale(&sys::ScePspFVector3 {
                x: scale,
                y: scale,
                z: scale,
            });
        }
    }

    /// Reads back the current model matrix. Object drawing needs the camera
    /// transform as a *base* to compose each node's baked world matrix onto.
    /// Verbatim copy of `psp/gu.rs`'s own method.
    pub fn model_matrix(&self) -> sys::ScePspFMatrix4 {
        let zero = sys::ScePspFVector4 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            w: 0.0,
        };
        let mut m = sys::ScePspFMatrix4 {
            x: zero,
            y: zero,
            z: zero,
            w: zero,
        };
        unsafe {
            sys::sceGumMatrixMode(sys::MatrixMode::Model);
            sys::sceGumStoreMatrix(&mut m);
        }
        m
    }

    /// Draws untextured, vertex-coloured triangles. Verbatim copy of
    /// `psp/gu.rs`'s own method (used by `meshdraw`'s collision/fighter-
    /// marker overlays, currently unused by any `psp-game` caller but needed
    /// for that module to compile as a verbatim copy).
    ///
    /// # Safety
    ///
    /// `verts` must be 16-byte aligned.
    pub unsafe fn draw_triangles(&mut self, verts: &[GuVertex]) {
        sys::sceGuDisable(GuState::Texture2D);
        let bytes = verts.len() * core::mem::size_of::<GuVertex>();
        let dynamic = sys::sceGuGetMemory(bytes as i32) as *mut GuVertex;
        core::ptr::copy_nonoverlapping(verts.as_ptr(), dynamic, verts.len());
        sys::sceGumDrawArray(
            GuPrimitive::Triangles,
            GuVertex::FORMAT,
            verts.len() as i32,
            core::ptr::null(),
            dynamic as *const c_void,
        );
    }

    /// Draws one open polyline, joining each vertex to the next. Verbatim
    /// copy of `psp/gu.rs`'s own method; see `draw_triangles`'s doc comment
    /// for why this is kept even with no current `psp-game` caller.
    ///
    /// # Safety
    ///
    /// `verts` must be 16-byte aligned.
    pub unsafe fn draw_line_strip(&mut self, verts: &[GuVertex]) {
        if verts.len() < 2 {
            return;
        }
        sys::sceGuDisable(GuState::Texture2D);
        let bytes = verts.len() * core::mem::size_of::<GuVertex>();
        let dynamic = sys::sceGuGetMemory(bytes as i32) as *mut GuVertex;
        core::ptr::copy_nonoverlapping(verts.as_ptr(), dynamic, verts.len());
        sys::sceGumDrawArray(
            GuPrimitive::LineStrip,
            GuVertex::FORMAT,
            verts.len() as i32,
            core::ptr::null(),
            dynamic as *const c_void,
        );
    }

    pub fn begin_frame(&mut self, clear: Color) {
        debug_assert!(!self.frame_open, "begin_frame called twice");
        self.frame_open = true;
        unsafe {
            sys::sceGuStart(GuContextType::Direct, Self::list_ptr());
            sys::sceGuClearColor(clear.to_abgr());
            // Depth is cleared every frame regardless of screen: harmless for
            // the flat 2D intro/menu (which draw with depth test off, see
            // `draw_rect`) and required for the training scene's real 3D
            // draws, which would otherwise test against stale depth data from
            // whichever buffer this frame reuses.
            sys::sceGuClearDepth(0);
            sys::sceGuClear(ClearBuffer::COLOR_BUFFER_BIT | ClearBuffer::DEPTH_BUFFER_BIT);
        }
    }

    /// Submits the frame and swaps buffers on vblank.
    pub fn end_frame(&mut self) {
        debug_assert!(self.frame_open, "end_frame without begin_frame");
        self.frame_open = false;
        unsafe {
            sys::sceGuFinish();
            sys::sceGuSync(GuSyncMode::Finish, GuSyncBehavior::Wait);
            sys::sceDisplayWaitVblankStart();
            sys::sceGuSwapBuffers();
        }
    }

    /// Draws one filled, axis-aligned rectangle in screen-pixel coordinates.
    /// `GuPrimitive::Sprites` fills the box between two opposite corners, the
    /// same primitive `psp/`'s own `SpriteVertex` 2D drawing uses.
    ///
    /// Brackets depth test and culling off around the draw and restores them
    /// after, the same pattern `psp/gu.rs`'s own `draw_wallpaper_sprite`
    /// uses for its 2D sprite: `Gpu::init` now defaults both on for the
    /// training scene's 3D content, and a `TRANSFORM_2D` primitive must
    /// neither be depth-tested against stale buffer contents nor culled by a
    /// winding order that has no meaning in screen space.
    pub fn draw_rect(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, color: Color) {
        let verts = [
            RectVertex::new(x0 as i16, y0 as i16, color.to_abgr()),
            RectVertex::new(x1 as i16, y1 as i16, color.to_abgr()),
        ];
        unsafe {
            sys::sceGuDisable(GuState::DepthTest);
            sys::sceGuDisable(GuState::CullFace);
            sys::sceGuDrawArray(
                GuPrimitive::Sprites,
                RectVertex::FORMAT,
                2,
                core::ptr::null(),
                verts.as_ptr() as *const c_void,
            );
            sys::sceGuEnable(GuState::DepthTest);
            sys::sceGuEnable(GuState::CullFace);
        }
    }

    /// # Safety
    ///
    /// The returned buffer is handed to the GE, which writes to it
    /// asynchronously. Only the thread that owns the `Gpu` may call this, and
    /// only between `sceGuStart` and `sceGuSync`.
    unsafe fn list_ptr() -> *mut c_void {
        core::ptr::addr_of_mut!(DISPLAY_LIST.0) as *mut c_void
    }
}
