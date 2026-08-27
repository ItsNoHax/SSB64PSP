//! GU context: display setup, frame lifecycle, VRAM layout.
//!
//! All the `unsafe` needed to talk to the GE lives here, behind a safe
//! `Gpu` type, per the project's rule that unsafe is isolated rather than
//! sprinkled through gameplay.

use core::ffi::c_void;

use psp::sys::{
    self, ClearBuffer, DepthFunc, DisplayPixelFormat, FrontFaceDirection, GuContextType,
    GuPrimitive, GuState, GuSyncBehavior, GuSyncMode, ShadingModel, TexturePixelFormat, VertexType,
};
use psp::vram_alloc::get_vram_allocator;
use psp::{Align16, BUF_WIDTH, SCREEN_HEIGHT, SCREEN_WIDTH};

use ssb_engine::renderer::Color;

/// Display list scratch buffer.
///
/// 256 KiB of command space. Smash submits a lot of small draws, and running
/// out mid-frame corrupts the display silently rather than failing loudly, so
/// this is sized generously until profiling says otherwise.
static mut DISPLAY_LIST: Align16<[u32; 0x40000]> = Align16([0; 0x40000]);

/// A vertex laid out the way the GE wants it.
///
/// Field order is dictated by hardware, not taste: the GE reads texture
/// coordinates, then colour, then position, and the `VertexType` flags must
/// describe exactly that order. Reordering these fields silently renders
/// garbage.
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

/// A stack-allocated, NUL-terminated string builder.
///
/// `sceGuDebugPrint` wants a C string, and formatting into the heap every frame
/// is exactly what the "no allocation in hot paths" rule forbids. Writes past
/// capacity are dropped rather than panicking.
struct FixedStr<const N: usize> {
    buf: [u8; N],
    len: usize,
}

impl<const N: usize> FixedStr<N> {
    fn new() -> Self {
        FixedStr {
            buf: [0; N],
            len: 0,
        }
    }

    /// Pointer to the NUL-terminated contents. The last byte is reserved for
    /// the terminator and is never written by `write_str`.
    fn as_c_str(&self) -> *const u8 {
        self.buf.as_ptr()
    }
}

impl<const N: usize> core::fmt::Write for FixedStr<N> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        // Interior NUL bytes are dropped, not copied. This builds a C string,
        // so a single embedded NUL silently truncates everything after it --
        // which is exactly what happened when a NUL-terminated path constant
        // was formatted into the overlay and swallowed nine of eleven lines.
        for &b in s.as_bytes() {
            if b == 0 {
                continue;
            }
            if self.len >= N - 1 {
                break; // leave room for the terminator
            }
            self.buf[self.len] = b;
            self.len += 1;
        }
        Ok(())
    }
}

/// Owns the GU context and the frame lifecycle.
pub struct Gpu {
    frame_open: bool,
    frames: u64,
}

impl Gpu {
    /// Initialises the display, allocates framebuffers in VRAM, and sets the
    /// pipeline state the game runs under.
    ///
    /// # Safety
    ///
    /// Must be called exactly once, before any other GU use.
    pub unsafe fn init() -> Gpu {
        let allocator = get_vram_allocator().expect("VRAM allocator already taken");

        // Two 32-bit colour buffers plus a 16-bit depth buffer. Psm4444 is the
        // conventional way to request a 16-bit-per-pixel VRAM block for depth;
        // the GE reinterprets it as depth via sceGuDepthBuffer.
        let fbp0 =
            allocator.alloc_texture_pixels(BUF_WIDTH, SCREEN_HEIGHT, TexturePixelFormat::Psm8888);
        let fbp1 =
            allocator.alloc_texture_pixels(BUF_WIDTH, SCREEN_HEIGHT, TexturePixelFormat::Psm8888);
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
        // (0,0) is the top-left of the visible area.
        sys::sceGuOffset(2048 - (SCREEN_WIDTH / 2), 2048 - (SCREEN_HEIGHT / 2));

        // Pillarbox to 4:3. The projection is built with the *N64's* aspect
        // (`coord::pillarboxed_viewport`, 362x272), so the GE viewport has to
        // be that same 362 wide or the image is stretched across the full 480
        // -- a horizontal exaggeration of 480/362 = 1.33x that makes every
        // character a third too wide.
        //
        // This was measured, not guessed: Mario's collision diamond is 300
        // units across and 320 tall, so it should render very slightly taller
        // than wide. On device it came out 27 px wide against 22 px tall, a
        // width/height of 1.23 where 0.94 was expected. The ratio between
        // those, 1.31, is 480/362.
        let (vx, _, vw, vh) = ssb_engine::coord::pillarboxed_viewport();
        sys::sceGuViewport(2048, 2048, vw as i32, vh as i32);

        // The PSP's depth buffer is inverted relative to what you'd expect:
        // near maps to 65535, far to 0, so the depth test is GreaterOrEqual.
        sys::sceGuDepthRange(65535, 0);
        sys::sceGuDepthFunc(DepthFunc::GreaterOrEqual);
        sys::sceGuEnable(GuState::DepthTest);

        // Scissor to the same region, so nothing bleeds into the black bars.
        sys::sceGuScissor(vx as i32, 0, (vx + vw) as i32, vh as i32);
        sys::sceGuEnable(GuState::ScissorTest);

        sys::sceGuFrontFace(FrontFaceDirection::Clockwise);
        sys::sceGuShadeModel(ShadingModel::Smooth);
        sys::sceGuEnable(GuState::CullFace);
        sys::sceGuEnable(GuState::ClipPlanes);

        sys::sceGuFinish();
        sys::sceGuSync(GuSyncMode::Finish, GuSyncBehavior::Wait);

        sys::sceDisplayWaitVblankStart();
        sys::sceGuDisplay(true);

        Gpu {
            frame_open: false,
            frames: 0,
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

    /// Opens a frame and optionally clears.
    pub fn begin_frame(&mut self, clear: Option<Color>) {
        debug_assert!(!self.frame_open, "begin_frame called twice");
        self.frame_open = true;
        unsafe {
            sys::sceGuStart(GuContextType::Direct, Self::list_ptr());
            if let Some(c) = clear {
                sys::sceGuClearColor(c.to_abgr());
                sys::sceGuClearDepth(0);
                sys::sceGuClear(ClearBuffer::COLOR_BUFFER_BIT | ClearBuffer::DEPTH_BUFFER_BIT);
            }
        }
    }

    /// Submits the frame and swaps buffers on vblank.
    pub fn end_frame(&mut self) {
        debug_assert!(self.frame_open, "end_frame without begin_frame");
        self.frame_open = false;
        self.frames += 1;
        unsafe {
            sys::sceGuFinish();
            sys::sceGuSync(GuSyncMode::Finish, GuSyncBehavior::Wait);
            // Debug text must be painted *here*, not earlier. sceGuDebugFlush
            // writes glyphs straight into the draw buffer rather than queueing
            // a GE command, so flushing before the sync would just get erased
            // by the sceGuClear that is still sitting in the display list.
            sys::sceGuDebugFlush();
            sys::sceDisplayWaitVblankStart();
            sys::sceGuSwapBuffers();
        }
    }

    pub fn frame_count(&self) -> u64 {
        self.frames
    }

    /// Sets the projection matrix from a field of view in degrees.
    pub fn set_perspective(&mut self, fovy_degrees: f32, aspect: f32, near: f32, far: f32) {
        unsafe {
            sys::sceGumMatrixMode(sys::MatrixMode::Projection);
            sys::sceGumLoadIdentity();
            sys::sceGumPerspective(fovy_degrees, aspect, near, far);
        }
    }

    /// Resets view and model matrices to identity.
    pub fn reset_modelview(&mut self) {
        unsafe {
            sys::sceGumMatrixMode(sys::MatrixMode::View);
            sys::sceGumLoadIdentity();
            sys::sceGumMatrixMode(sys::MatrixMode::Model);
            sys::sceGumLoadIdentity();
        }
    }

    /// Translates, rotates and uniformly scales the model matrix.
    ///
    /// Calls post-multiply, so the effective transform is `T * R * S`:
    /// vertices are scaled first, then rotated, then translated.
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

    /// Reads back the current model matrix.
    ///
    /// Object drawing needs the camera transform as a *base* to compose each
    /// node's baked world matrix onto. Storing it here rather than rebuilding
    /// it keeps one definition of how the camera is placed.
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

    /// Queues a block of debug text for this frame. Embedded `\n` starts a new
    /// line. Call **once** per frame.
    ///
    /// Two hard-won constraints are encoded here:
    ///
    /// * Do not use `psp::dprintln!` per-frame. It re-runs `sceDisplaySetMode`
    ///   on every call; measured under PPSSPP it took a 4-triangle scene from
    ///   60 FPS to 2 FPS.
    /// * Call this once with newlines rather than once per line. rust-psp's
    ///   `sceGuDebugPrint` always writes from the start of its internal
    ///   character buffer while still advancing the "used" counter, so
    ///   successive calls overwrite each other and render garbage. A single
    ///   call sidesteps that entirely.
    ///
    /// The text is copied immediately, so the caller's buffer need not outlive
    /// the call. It is painted onto the draw buffer by [`Gpu::end_frame`],
    /// after the GE has finished — see there for why.
    pub fn debug_text(&mut self, x: i32, y: i32, color: u32, args: core::fmt::Arguments<'_>) {
        let mut text: FixedStr<512> = FixedStr::new();
        // Truncated diagnostics beat a panic in a no_std frame loop.
        let _ = core::fmt::Write::write_fmt(&mut text, args);
        unsafe { sys::sceGuDebugPrint(x, y, color, text.as_c_str()) }
    }

    /// Draws untextured, vertex-coloured triangles.
    ///
    /// `verts` must be 16-byte aligned for the GE to DMA it, which is why the
    /// caller passes an `Align16` buffer.
    ///
    /// # Safety
    ///
    /// `verts` must live until the frame is submitted.
    pub unsafe fn draw_triangles(&mut self, verts: &[GuVertex]) {
        sys::sceGuDisable(GuState::Texture2D);
        sys::sceGumDrawArray(
            GuPrimitive::Triangles,
            GuVertex::FORMAT,
            verts.len() as i32,
            core::ptr::null(),
            verts.as_ptr() as *const c_void,
        );
    }

    /// Draws one open polyline, joining each vertex to the next.
    ///
    /// Used for the collision overlay, where the data really is a polyline —
    /// drawing it as anything else would misrepresent what the game stores.
    /// Depth testing is left on so a line behind stage geometry is occluded by
    /// it, which is what makes the overlay readable as "in the scene" rather
    /// than painted on top.
    ///
    /// # Safety
    ///
    /// `verts` must be 16-byte aligned and live until the frame is submitted.
    pub unsafe fn draw_line_strip(&mut self, verts: &[GuVertex]) {
        if verts.len() < 2 {
            return;
        }
        sys::sceGuDisable(GuState::Texture2D);
        sys::sceGumDrawArray(
            GuPrimitive::LineStrip,
            GuVertex::FORMAT,
            verts.len() as i32,
            core::ptr::null(),
            verts.as_ptr() as *const c_void,
        );
    }
}
