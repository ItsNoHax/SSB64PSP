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

/// Real content width of the LB "loading transition" framebuffer capture
/// (RE-099/RE-100): the real ROM's own snapshot buffer is 300 texels wide
/// (`sLBTransitionPhotoHeap`, `refs/ssb-decomp-re/src/lb/lbtransition.c:224`),
/// and every real display list's own baked UVs were authored against exactly
/// that width. Matches `TextureDesc::width` for every `ROLE_FRAMEBUFFER`
/// entry the pack builds (`PackWriter::add_framebuffer_texture`).
pub const TRANSITION_PHOTO_WIDTH: usize = 300;

/// Row stride in texels the GE actually addresses -- `TRANSITION_PHOTO_WIDTH`
/// padded to a power of two, matching every other `TextureDesc::stride`'s own
/// convention (`crate::meshdraw::bind_texture`'s general path already reads
/// `t.stride`, never `t.width`, for `sceGuTexImage`'s `bufferwidth`). Real
/// measured UV spans never exceed the real width (RE-100: U repeats maxed out
/// at 1.00 across all 13 files), so the padding columns are never sampled.
const TRANSITION_PHOTO_STRIDE: usize = 512; // TRANSITION_PHOTO_WIDTH.next_power_of_two()

/// Height of the capture buffer, padded to a power of two.
///
/// RE-100 measured the real ROM only ever samples the top 5 or 6 rows of the
/// N64's 220-row snapshot (both `G_SETTIMG`s bind offset 0, tiled/repeated
/// across much taller geometry by ordinary wrap addressing) -- not the whole
/// 220-row image RE-099 originally guessed a PSP port might need. 6 pads to
/// 8, which also covers the 5-row case, so one capture buffer serves both of
/// `TextureDesc::ROLE_FRAMEBUFFER`'s real shapes.
pub const TRANSITION_PHOTO_HEIGHT: usize = 8;

/// Real captured rows before the wrap-periodicity padding described on
/// [`Gpu::capture_transition_photo`].
const TRANSITION_PHOTO_REAL_ROWS: usize = 6;

/// Captured by [`Gpu::request_transition_capture`], read by
/// `meshdraw::bind_texture` whenever a primitive's `TextureDesc::role` is
/// `ROLE_FRAMEBUFFER`.
///
/// A plain module static, not threaded through the draw call chain the way
/// `MaterialAnimator` is: unlike a per-object material animator there is
/// exactly one of these for the whole process, the same shape `DISPLAY_LIST`
/// above already uses for the same reason.
///
/// Captured in the PSP's own native `Psm8888` rather than the N64's
/// RGBA5551 -- the GE already reads the real draw buffer in that format, so
/// this is a plain block copy with no conversion, and a screen-colour smear
/// has no need for the original's 16-bit precision. An accepted format
/// deviation, not a fidelity gap that matters here.
static mut TRANSITION_PHOTO: Align16<[u32; TRANSITION_PHOTO_STRIDE * TRANSITION_PHOTO_HEIGHT]> =
    Align16([0; TRANSITION_PHOTO_STRIDE * TRANSITION_PHOTO_HEIGHT]);

/// Bytes the GE should read for the transition photo capture.
///
/// # Safety
///
/// Aliases [`TRANSITION_PHOTO`]; the caller must not hold this across a call
/// to [`Gpu::request_transition_capture`]'s eventual capture (i.e. not across
/// a frame boundary), the same rule the pack's own texture data already
/// follows since the GE reads it by DMA.
pub unsafe fn transition_photo_data() -> &'static [u8] {
    let ptr = core::ptr::addr_of!(TRANSITION_PHOTO.0) as *const u8;
    core::slice::from_raw_parts(ptr, TRANSITION_PHOTO_STRIDE * TRANSITION_PHOTO_HEIGHT * 4)
}

/// Owns the GU context and the frame lifecycle.
pub struct Gpu {
    frame_open: bool,
    frames: u64,
    /// CPU-dereferenceable (not GE-relative) pointers to the two colour
    /// buffers, retained so a transition capture can read one back after it
    /// finishes rendering. `init()`'s local `VramMemChunk`s only live long
    /// enough to hand the GE their GE-relative addresses.
    fbp0_direct: *mut u8,
    fbp1_direct: *mut u8,
    /// Which physical buffer the GE is *currently* drawing into. Toggled once
    /// per `end_frame`, in lockstep with the one `sceGuSwapBuffers` call this
    /// code makes -- the PSP SDK swaps the draw/display roles internally on
    /// that call without needing `sceGuDrawBuffer` reissued, so nothing else
    /// changes which physical address is "the draw buffer" between calls.
    draw_is_fbp0: bool,
    /// Set by [`Gpu::request_transition_capture`]; consumed (and cleared) the
    /// next time `end_frame` finishes syncing the frame that was requested.
    capture_requested: bool,
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

        // Retained past `init()` for `request_transition_capture`'s CPU-side
        // readback -- `as_mut_ptr_from_zero()` below is only meaningful as the
        // GE's own relative addressing, not a pointer the CPU can dereference.
        let fbp0_direct = fbp0.as_mut_ptr_direct_to_vram();
        let fbp1_direct = fbp1.as_mut_ptr_direct_to_vram();

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
            fbp0_direct,
            fbp1_direct,
            // `sceGuDrawBuffer(fbp0, ...)` above is the initial draw target.
            draw_is_fbp0: true,
            capture_requested: false,
        }
    }

    /// Requests that the frame currently in flight be copied into the
    /// transition photo buffer once it finishes rendering.
    ///
    /// This is the PSP-side equivalent of `lbTransitionSetupTransition`'s
    /// one-time framebuffer photocopy (RE-099/RE-100): a plain block copy,
    /// not a render pass, taken once and reused by every primitive whose
    /// `TextureDesc::role` is `ROLE_FRAMEBUFFER` until requested again.
    pub fn request_transition_capture(&mut self) {
        self.capture_requested = true;
    }

    /// Copies the top-left corner of whichever buffer just finished
    /// rendering into [`TRANSITION_PHOTO`].
    ///
    /// # Safety
    ///
    /// Must only be called between `sceGuSync(Finish, Wait)` completing (so
    /// the buffer's contents are final) and the next `sceGuSwapBuffers` (so
    /// `draw_is_fbp0` still names the buffer that was just drawn into).
    ///
    /// Rows beyond [`TRANSITION_PHOTO_REAL_ROWS`] are not left stale: the GE
    /// wraps a `ROLE_FRAMEBUFFER` texture at `TRANSITION_PHOTO_HEIGHT` (8),
    /// not at the real 6-row content, because `TextureDesc::height`'s padded
    /// power-of-two is what `sceGuTexImage`/`sceGuTexScale` actually use
    /// (same convention every other non-power-of-two-height texture in the
    /// pack already follows). Filling them with a copy of rows 0-1 makes the
    /// 8-row buffer repeat the real 6-row pattern seamlessly, matching the
    /// real ROM's own period for the one primitive shape that measurably
    /// wraps its V axis (RE-100: the 300x6 tile, up to 35.83 repeats) rather
    /// than introducing two extra, unintended rows into that repeat.
    ///
    /// `TRANSITION_PHOTO_WIDTH`'s columns are read starting at the
    /// pillarbox's own left edge (`pillarboxed_viewport().0`), not absolute
    /// column 0 of the raw 480-wide buffer (RE-111). Every real draw --
    /// including this project's own game content -- is scoped to the
    /// pillarboxed 4:3 viewport by the permanently-enabled scissor
    /// (`Gpu::new`), so columns left of it are never drawn to at all and
    /// stay at their power-on value (zero, i.e. solid black) for the whole
    /// program's life. `TRANSITION_PHOTO_WIDTH` (300) already fits entirely
    /// inside the pillarboxed width (362) starting from that edge, so this
    /// is a pure offset correction, not a re-tuned capture size.
    unsafe fn capture_transition_photo(&self) {
        let src = if self.draw_is_fbp0 {
            self.fbp0_direct
        } else {
            self.fbp1_direct
        } as *const u32;
        let (vx, _, _, _) = ssb_engine::coord::pillarboxed_viewport();
        let dst = core::ptr::addr_of_mut!(TRANSITION_PHOTO.0) as *mut u32;
        for y in 0..TRANSITION_PHOTO_REAL_ROWS {
            let src_row = src.add(y * BUF_WIDTH as usize + vx as usize);
            let dst_row = dst.add(y * TRANSITION_PHOTO_STRIDE);
            core::ptr::copy_nonoverlapping(src_row, dst_row, TRANSITION_PHOTO_WIDTH);
        }
        for y in TRANSITION_PHOTO_REAL_ROWS..TRANSITION_PHOTO_HEIGHT {
            let wrap_from = (y - TRANSITION_PHOTO_REAL_ROWS) * TRANSITION_PHOTO_STRIDE;
            let dst_row = dst.add(y * TRANSITION_PHOTO_STRIDE);
            core::ptr::copy_nonoverlapping(dst.add(wrap_from), dst_row, TRANSITION_PHOTO_WIDTH);
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
            if self.capture_requested {
                self.capture_requested = false;
                self.capture_transition_photo();
            }
            sys::sceDisplayWaitVblankStart();
            sys::sceGuSwapBuffers();
        }
        // Mirrors the swap `sceGuSwapBuffers` just performed internally: the
        // buffer that was the draw target for the frame just finished
        // becomes the display buffer, and the GE will draw the next frame
        // into whichever buffer was previously being displayed.
        self.draw_is_fbp0 = !self.draw_is_fbp0;
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

    /// Loads a real view matrix (RE-131) -- an `eye`/`at`/`up` camera, not
    /// just a translation. `m` is expected column-major, matching
    /// [`ssb_engine::math::Mat4::as_array`]'s own documented output.
    ///
    /// The model matrix is untouched: callers still set per-object placement
    /// with [`Gpu::model_transform`] as before, now composed under a real
    /// view transform instead of an implicit identity one.
    pub fn set_view(&mut self, m: &ssb_engine::math::Mat4) {
        let a = m.as_array();
        let fm = sys::ScePspFMatrix4 {
            x: sys::ScePspFVector4 { x: a[0], y: a[1], z: a[2], w: a[3] },
            y: sys::ScePspFVector4 { x: a[4], y: a[5], z: a[6], w: a[7] },
            z: sys::ScePspFVector4 { x: a[8], y: a[9], z: a[10], w: a[11] },
            w: sys::ScePspFVector4 { x: a[12], y: a[13], z: a[14], w: a[15] },
        };
        unsafe {
            sys::sceGumMatrixMode(sys::MatrixMode::View);
            sys::sceGumLoadMatrix(&fm);
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
