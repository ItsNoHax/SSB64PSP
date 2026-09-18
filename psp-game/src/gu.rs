//! GU context: display setup, frame lifecycle, flat 2D UI drawing.
//!
//! `psp/`'s `gu.rs` is the fuller reference (mesh drawing, texturing, view
//! matrices, debug overlay). This is a deliberately smaller subset: F1's
//! current increment is a menu made of flat coloured rectangles, not textured
//! 3D content, so the 3D pipeline (depth test, culling, projection) stays off
//! until a later increment turns it on for the training scene. Kept as its
//! own copy rather than a shared module: `psp/` and `psp-game/` are separate
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
    self, ClearBuffer, DisplayPixelFormat, GuContextType, GuPrimitive, GuState, GuSyncBehavior,
    GuSyncMode, TexturePixelFormat, VertexType,
};
use psp::vram_alloc::get_vram_allocator;
use psp::{Align16, BUF_WIDTH, SCREEN_HEIGHT, SCREEN_WIDTH};

use ssb_engine::renderer::Color;

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
        VertexType::COLOR_8888.bits() | VertexType::VERTEX_16BIT.bits() | VertexType::TRANSFORM_2D.bits(),
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
    /// Initialises the display and allocates the two colour buffers this
    /// build needs. No depth buffer yet: nothing drawn so far needs one, and
    /// allocating it unused would just be VRAM the training scene's real 3D
    /// setup can claim later instead.
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

        // The GE's screen space is centred on 2048; this offsets it so that
        // (0,0) is the top-left of the visible area -- matches `psp/`'s own
        // `sceGuOffset` call, which is what makes `RectVertex`'s raw pixel
        // coordinates land where they visually look like they should.
        sys::sceGuOffset(2048 - (SCREEN_WIDTH / 2), 2048 - (SCREEN_HEIGHT / 2));
        sys::sceGuViewport(2048, 2048, SCREEN_WIDTH as i32, SCREEN_HEIGHT as i32);
        sys::sceGuScissor(0, 0, SCREEN_WIDTH as i32, SCREEN_HEIGHT as i32);
        sys::sceGuEnable(GuState::ScissorTest);

        sys::sceGuDisable(GuState::DepthTest);
        sys::sceGuDisable(GuState::Texture2D);
        sys::sceGuDisable(GuState::Lighting);
        sys::sceGuDisable(GuState::CullFace);

        sys::sceGuFinish();
        sys::sceGuSync(GuSyncMode::Finish, GuSyncBehavior::Wait);

        sys::sceDisplayWaitVblankStart();
        sys::sceGuDisplay(true);

        Gpu { frame_open: false }
    }

    pub fn begin_frame(&mut self, clear: Color) {
        debug_assert!(!self.frame_open, "begin_frame called twice");
        self.frame_open = true;
        unsafe {
            sys::sceGuStart(GuContextType::Direct, Self::list_ptr());
            sys::sceGuClearColor(clear.to_abgr());
            sys::sceGuClear(ClearBuffer::COLOR_BUFFER_BIT);
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
    pub fn draw_rect(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, color: Color) {
        let verts = [
            RectVertex::new(x0 as i16, y0 as i16, color.to_abgr()),
            RectVertex::new(x1 as i16, y1 as i16, color.to_abgr()),
        ];
        unsafe {
            sys::sceGuDrawArray(
                GuPrimitive::Sprites,
                RectVertex::FORMAT,
                2,
                core::ptr::null(),
                verts.as_ptr() as *const c_void,
            );
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
