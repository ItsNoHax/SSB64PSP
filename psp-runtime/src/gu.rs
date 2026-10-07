//! GU context: display setup, frame lifecycle, VRAM layout.
//!
//! All the `unsafe` needed to talk to the GE lives here, behind a safe
//! `Gpu` type, per the project's rule that unsafe is isolated rather than
//! sprinkled through gameplay.
//!
//! `Gpu::init` brings up a full-screen viewport/scissor. Callers that need
//! the N64-aspect pillarbox (`ssb_engine::coord::pillarboxed_viewport`) for
//! real 3D content call [`Gpu::set_viewport_pillarboxed`] explicitly and
//! [`Gpu::set_viewport_fullscreen`] to go back -- e.g. for a flat 2D
//! intro/menu around a pillarboxed training scene. A caller that only ever
//! draws 3D content (no flat 2D screens) can call
//! [`Gpu::set_viewport_pillarboxed`] once, immediately after `init`, before
//! the first `begin_frame`.

use core::ffi::c_void;

use psp::sys::{
    self, ClearBuffer, DepthFunc, DisplayPixelFormat, FrontFaceDirection, GuContextType,
    GuPrimitive, GuState, GuSyncBehavior, GuSyncMode, ShadingModel, TexturePixelFormat, VertexType,
};
use psp::vram_alloc::get_vram_allocator;
use psp::{BUF_WIDTH, SCREEN_HEIGHT, SCREEN_WIDTH};

use ssb_engine::renderer::Color;

/// Display list scratch buffer.
///
/// 256 KiB of command space. Smash submits a lot of small draws, and running
/// out mid-frame corrupts the display silently rather than failing loudly, so
/// this is sized generously until profiling says otherwise.
///
/// `sceGuStart` writes commands through the uncached alias, so no cached
/// variable may share a 64-byte D-cache line with this buffer. With only
/// 16-byte alignment, `.bss` neighbours written through the cache
/// (`CURRENT_MATRIX_UPDATE`, `VRAM_ALLOCATOR`) shared the first line. Its
/// writeback replaced the init list's `FramebufPixFormat` command, and the
/// PSP rendered every frame as RGB565 into the RGBA8888 buffer (RE-360).
static mut DISPLAY_LIST: CacheLineAligned<[u32; 0x40000]> = CacheLineAligned([0; 0x40000]);

/// Aligns a GE buffer to the Allegrex's 64-byte D-cache line.
#[repr(C, align(64))]
struct CacheLineAligned<T>(T);

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

/// Screen-space 2D vertex (`GU_TRANSFORM_2D`): raw pixel coordinates, no
/// MVP transform. This is the PSP GE's native equivalent of the RDP's
/// `gSPTextureRectangle`, the primitive a real `SObj` sprite draw bottoms
/// out in (RE-193) -- unlike every other draw in this renderer, `SObj`
/// rendering bypasses the 3D transform pipeline entirely.
#[repr(C, align(4))]
#[derive(Clone, Copy)]
struct SpriteVertex {
    pub u: f32,
    pub v: f32,
    /// Packed ABGR, matching [`GuVertex::color`]'s own convention.
    pub color: u32,
    pub x: i16,
    pub y: i16,
    pub z: i16,
    _pad: i16,
}

impl SpriteVertex {
    /// Field order (texture, then colour, then position) is dictated by
    /// hardware, the same rule [`GuVertex::FORMAT`] documents.
    const FORMAT: VertexType = VertexType::from_bits_truncate(
        VertexType::TEXTURE_32BITF.bits()
            | VertexType::COLOR_8888.bits()
            | VertexType::VERTEX_16BIT.bits()
            | VertexType::TRANSFORM_2D.bits(),
    );
}

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

/// The LB transition photo's size, `sLBTransitionPhotoHeap`'s 300 x 220
/// texels (`ssb_rom::mobj::LB_TRANSITION_PHOTO`, RE-464). Matches
/// `TextureDesc::width`/`height` of the pack's one `ROLE_FRAMEBUFFER`
/// entry (`PackWriter::add_framebuffer_texture`).
pub const TRANSITION_PHOTO_WIDTH: usize = ssb_rom::mobj::LB_TRANSITION_PHOTO.0 as usize;
/// See [`TRANSITION_PHOTO_WIDTH`].
pub const TRANSITION_PHOTO_HEIGHT: usize = ssb_rom::mobj::LB_TRANSITION_PHOTO.1 as usize;

/// The row stride the GE addresses: the width padded to a power of two,
/// every `TextureDesc::stride`'s convention.
const TRANSITION_PHOTO_STRIDE: usize = 512;

/// The rows the buffer holds: the height padded to the power of two
/// `sceGuTexImage` is given (`ge_texture_dims`).
const TRANSITION_PHOTO_ROWS: usize = 256;

/// Captured by [`Gpu::request_transition_capture`], read by
/// `meshdraw::bind_texture` whenever a primitive's `TextureDesc::role` is
/// `ROLE_FRAMEBUFFER`.
///
/// A plain module static, not threaded through the draw call chain the way
/// `MaterialAnimator` is: unlike a per-object material animator there is
/// exactly one of these for the whole process, the same shape `DISPLAY_LIST`
/// above already uses for the same reason.
///
/// RGB565 (`Psm5650`): the N64's photo is RGBA5551, and its wipes' lists
/// set no render mode (no `G_SETOTHERMODE_L` in files 40-51), so they draw
/// opaque under the one `lbTransitionMakeCamera`'s pass leaves and the
/// photo's alpha bit is never read. 16 bits keep its precision at half
/// `Psm8888`'s 512 KiB.
///
/// The CPU fills this through the D-cache and the GE samples it by DMA, so
/// the capture writes it back after each copy. The 64-byte alignment keeps
/// that writeback from spilling into, or being spilled into by, a `.bss`
/// neighbour's cache line, the RE-360 failure shape. The size (256 KiB) is
/// a whole number of lines.
static mut TRANSITION_PHOTO: CacheLineAligned<[u16; TRANSITION_PHOTO_STRIDE * TRANSITION_PHOTO_ROWS]> =
    CacheLineAligned([0; TRANSITION_PHOTO_STRIDE * TRANSITION_PHOTO_ROWS]);

/// Bytes the GE should read for the transition photo capture.
///
/// # Safety
///
/// Aliases [`TRANSITION_PHOTO`]; the caller must not hold this across a
/// capture (i.e. not across a frame boundary), the same rule the pack's own
/// texture data already follows since the GE reads it by DMA.
pub unsafe fn transition_photo_data() -> &'static [u8] {
    let ptr = core::ptr::addr_of!(TRANSITION_PHOTO.0) as *const u8;
    core::slice::from_raw_parts(ptr, TRANSITION_PHOTO_STRIDE * TRANSITION_PHOTO_ROWS * 2)
}

/// `lbTransitionSetupTransition`'s copy from an 8888 buffer holding the
/// pillarboxed 4:3 picture: photo texel `(x, y)` is N64 screen pixel
/// `(10 + x, 230 - y)`, the copy running from row 230 upwards, sampled at
/// that pixel's centre on the PSP. The texel past the last column and the
/// row past the last row repeat their neighbours, so the GE's clamped
/// bilinear filter at the picture's right and bottom edges reads the
/// picture rather than the padding.
///
/// # Safety
///
/// `src` must point at a complete `BUF_WIDTH`-stride 8888 frame.
unsafe fn copy_transition_photo(src: *const u32) {
    let (vx, _, vw, vh) = ssb_engine::coord::pillarboxed_viewport();
    let dst = core::ptr::addr_of_mut!(TRANSITION_PHOTO.0) as *mut u16;
    let rgb565 = |c: u32| -> u16 {
        let (r, g, b) = (c & 0xFF, (c >> 8) & 0xFF, (c >> 16) & 0xFF);
        ((r >> 3) | ((g >> 2) << 5) | ((b >> 3) << 11)) as u16
    };
    for y in 0..TRANSITION_PHOTO_HEIGHT {
        let n64_y = 230 - y as i32;
        let sy = (((n64_y as f32 + 0.5) * vh as f32 / 240.0) as usize).min(SCREEN_HEIGHT as usize - 1);
        let row = dst.add(y * TRANSITION_PHOTO_STRIDE);
        for x in 0..TRANSITION_PHOTO_WIDTH {
            let sx = vx as usize + ((x as f32 + 10.5) * vw as f32 / 320.0) as usize;
            row.add(x).write(rgb565(src.add(sy * BUF_WIDTH as usize + sx).read()));
        }
        row.add(TRANSITION_PHOTO_WIDTH).write(row.add(TRANSITION_PHOTO_WIDTH - 1).read());
    }
    core::ptr::copy_nonoverlapping(
        dst.add((TRANSITION_PHOTO_HEIGHT - 1) * TRANSITION_PHOTO_STRIDE),
        dst.add(TRANSITION_PHOTO_HEIGHT * TRANSITION_PHOTO_STRIDE),
        TRANSITION_PHOTO_STRIDE,
    );
    // The GE reads this buffer by DMA and does not see the D-cache.
    let bytes = transition_photo_data();
    sys::sceKernelDcacheWritebackRange(bytes.as_ptr() as *const c_void, bytes.len() as u32);
}


/// Real content width of the 1P Stage Clear wallpaper-capture snapshot
/// (`sc1PStageClearCopyFramebufToWallpaper`, RE-190/191): the same 300-texel
/// active-picture rectangle `TRANSITION_PHOTO_WIDTH` already captures, since
/// both mechanisms read the same pillarboxed draw area.
pub const WALLPAPER_PHOTO_WIDTH: usize = 300;

/// Real content height: the whole 220-row active picture, not a padded
/// tile strip. Unlike [`TRANSITION_PHOTO_HEIGHT`], RE-191 read the ROM's own
/// destination `Sprite` header and `spDraw` source and found the N64's tile
/// padding and odd/even row swizzle both exist only to satisfy `LoadBlock`'s
/// TMEM addressing -- a PSP-side capture needs neither, so this is one plain
/// rectangular copy with no periodic wrap-fill.
pub const WALLPAPER_PHOTO_HEIGHT: usize = 220;

/// Row stride in texels, matching [`TRANSITION_PHOTO_STRIDE`]'s own
/// `WALLPAPER_PHOTO_WIDTH.next_power_of_two()` convention.
const WALLPAPER_PHOTO_STRIDE: usize = 512;

/// Rows the buffer actually allocates: the PSP GE's texture-height register
/// stores an exponent, so `sceGuTexImage` requires a power-of-two height the
/// same way it requires a power-of-two `bufferwidth` (RE-193,
/// [`draw_wallpaper_sprite`][Gpu::draw_wallpaper_sprite]). `WALLPAPER_PHOTO_HEIGHT`
/// (220) is real content, not a GE-legal texture height, so the backing
/// array is padded here rather than at the type used by the CPU-only
/// capture/blit paths, which never cared about this constraint before this
/// session added the first real GE bind of this buffer.
const WALLPAPER_PHOTO_PADDED_HEIGHT: usize = 256;

/// Captured by [`Gpu::request_wallpaper_capture`], read by
/// [`wallpaper_photo_data`] and drawn by
/// [`Gpu::draw_wallpaper_sprite`][Gpu::draw_wallpaper_sprite] (RE-193).
/// Rows [`WALLPAPER_PHOTO_HEIGHT`]..[`WALLPAPER_PHOTO_PADDED_HEIGHT`] are
/// never written and stay zero for the process's whole life -- harmless,
/// since [`Gpu::draw_wallpaper_sprite`] never samples past the real content.
/// Cache-line aligned and written back after each capture for the same
/// reason as [`TRANSITION_PHOTO`]; the size (512 KiB) is a whole number of
/// lines.
static mut WALLPAPER_PHOTO: CacheLineAligned<
    [u32; WALLPAPER_PHOTO_STRIDE * WALLPAPER_PHOTO_PADDED_HEIGHT],
> = CacheLineAligned([0; WALLPAPER_PHOTO_STRIDE * WALLPAPER_PHOTO_PADDED_HEIGHT]);

/// Bytes captured for the wallpaper snapshot, padded height included (the
/// shape [`Gpu::draw_wallpaper_sprite`]'s `sceGuTexImage` call needs). Same
/// aliasing/safety contract as [`transition_photo_data`].
pub unsafe fn wallpaper_photo_data() -> &'static [u8] {
    let ptr = core::ptr::addr_of!(WALLPAPER_PHOTO.0) as *const u8;
    core::slice::from_raw_parts(
        ptr,
        WALLPAPER_PHOTO_STRIDE * WALLPAPER_PHOTO_PADDED_HEIGHT * 4,
    )
}

/// Owns the GU context and the frame lifecycle.
/// Whether a frame's display list is open, so the GE may be reading
/// memory the CPU frees (`scene_files`, RE-475). False between
/// [`Gpu::end_frame`]'s sync and the next [`Gpu::begin_frame`].
pub static GE_LIST_OPEN: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

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
    /// Set by [`Gpu::request_wallpaper_capture`]; consumed (and cleared) the
    /// next time `end_frame` finishes syncing the frame that was requested.
    wallpaper_capture_requested: bool,
    /// GE-relative addresses of the two colour buffers, for
    /// `sceGuDrawBuffer` and `sceDisplaySetFrameBuf`.
    fbp0_rel: *mut c_void,
    fbp1_rel: *mut c_void,
    /// `sceDisplayGetVcount` just after the last frame was handed to the
    /// display. That frame is latched at the next vblank, and the buffer it
    /// replaces may not be drawn into before then (RE-470).
    presented_vcount: Option<u32>,
}

impl Gpu {
    /// Initialises the display, allocates framebuffers in VRAM, and sets the
    /// pipeline state the game runs under.
    ///
    /// Brings up a full-screen viewport/scissor; a caller that needs the N64
    /// pillarbox for real 3D content calls [`Gpu::set_viewport_pillarboxed`]
    /// explicitly (see the module doc comment).
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
        let fbp0_rel = fbp0.as_mut_ptr_from_zero() as *mut c_void;
        let fbp1_rel = fbp1.as_mut_ptr_from_zero() as *mut c_void;

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
        // Full-screen by default -- see this method's doc comment and
        // `set_viewport_pillarboxed`.
        sys::sceGuViewport(2048, 2048, SCREEN_WIDTH as i32, SCREEN_HEIGHT as i32);
        sys::sceGuScissor(0, 0, SCREEN_WIDTH as i32, SCREEN_HEIGHT as i32);
        sys::sceGuEnable(GuState::ScissorTest);

        // The PSP's depth buffer is inverted relative to what you'd expect:
        // near maps to 65535, far to 0, so the depth test is GreaterOrEqual.
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

        Gpu {
            frame_open: false,
            frames: 0,
            fbp0_direct,
            fbp1_direct,
            // `sceGuDrawBuffer(fbp0, ...)` above is the initial draw target.
            draw_is_fbp0: true,
            capture_requested: false,
            wallpaper_capture_requested: false,
            fbp0_rel,
            fbp1_rel,
            presented_vcount: None,
        }
    }

    /// Switches to the N64-aspect pillarboxed viewport/scissor real 3D
    /// content needs -- a full-width 480px viewport stretches every
    /// character about a third too wide relative to the N64's 4:3
    /// projection (measured, not guessed: see `set_viewport_fullscreen`'s
    /// sibling call sites). Call once when entering a real 3D scene.
    pub fn set_viewport_pillarboxed(&mut self) {
        let (vx, _, vw, vh) = ssb_engine::coord::pillarboxed_viewport();
        unsafe {
            sys::sceGuViewport(2048, 2048, vw as i32, vh as i32);
            sys::sceGuScissor(vx as i32, 0, (vx + vw) as i32, vh as i32);
        }
    }

    /// A camera's own N64 viewport (`syRdpSetViewport`'s `ulx, uly, lrx,
    /// lry` on the 320 x 240 screen), placed inside the pillarboxed area
    /// and scissored to it.
    pub fn set_viewport_n64(&mut self, [ulx, uly, lrx, lry]: [f32; 4]) {
        let (vx, _, vw, vh) = ssb_engine::coord::pillarboxed_viewport();
        let (nw, nh) = ssb_engine::coord::N64_SCREEN;
        let kx = vw as f32 / nw as f32;
        let ky = vh as f32 / nh as f32;
        let (x0, x1) = (vx as f32 + ulx * kx, vx as f32 + lrx * kx);
        let (y0, y1) = (uly * ky, lry * ky);
        // `init`'s offset puts the screen's centre at 2048.
        let cx = 2048 - (SCREEN_WIDTH / 2) as i32 + ((x0 + x1) / 2.0) as i32;
        let cy = 2048 - (SCREEN_HEIGHT / 2) as i32 + ((y0 + y1) / 2.0) as i32;
        unsafe {
            sys::sceGuViewport(cx, cy, (x1 - x0) as i32, (y1 - y0) as i32);
            sys::sceGuScissor(x0 as i32, y0 as i32, x1 as i32, y1 as i32);
        }
    }

    /// `COBJ_FLAG_ZBUFFER`'s fill of a camera's depth with `G_MAXFBZ`: the
    /// far depth over the current scissor, colour untouched.
    pub fn clear_depth(&mut self) {
        unsafe {
            sys::sceGuClearDepth(0);
            sys::sceGuClear(ClearBuffer::DEPTH_BUFFER_BIT);
        }
    }

    /// Restores the full-screen viewport/scissor `init` set up, for flat 2D
    /// content (RE-289/290's pixel-confirmed evidence assumes this shape).
    /// Call when leaving a real 3D scene.
    pub fn set_viewport_fullscreen(&mut self) {
        unsafe {
            sys::sceGuViewport(2048, 2048, SCREEN_WIDTH as i32, SCREEN_HEIGHT as i32);
            sys::sceGuScissor(0, 0, SCREEN_WIDTH as i32, SCREEN_HEIGHT as i32);
        }
    }

    /// Requests that the frame currently in flight be copied into the
    /// transition photo buffer once it finishes rendering.
    ///
    /// This is the PSP-side equivalent of `lbTransitionSetupTransition`'s
    /// one-time framebuffer photocopy (RE-099, RE-464): a copy, not a render
    /// pass, taken once and reused by every primitive whose
    /// `TextureDesc::role` is `ROLE_FRAMEBUFFER` until requested again.
    pub fn request_transition_capture(&mut self) {
        self.capture_requested = true;
    }

    /// Copies the frame that just finished rendering into
    /// [`TRANSITION_PHOTO`] ([`copy_transition_photo`]).
    ///
    /// # Safety
    ///
    /// Must only be called between `sceGuSync(Finish, Wait)` completing (so
    /// the buffer's contents are final) and the next `sceGuSwapBuffers` (so
    /// `draw_is_fbp0` still names the buffer that was just drawn into).
    unsafe fn capture_transition_photo(&self) {
        let src = if self.draw_is_fbp0 {
            self.fbp0_direct
        } else {
            self.fbp1_direct
        } as *const u32;
        copy_transition_photo(src);
    }

    /// `lbTransitionSetupTransition` at a scene's start: copies the frame
    /// on display, the previous scene's last, into [`TRANSITION_PHOTO`]
    /// before this frame opens.
    pub fn capture_transition_photo_now(&self) {
        debug_assert!(!self.frame_open);
        unsafe {
            let src = if self.draw_is_fbp0 { self.fbp1_direct } else { self.fbp0_direct } as *const u32;
            copy_transition_photo(src);
        }
    }

    /// Requests that the frame currently in flight be copied into the
    /// wallpaper-capture buffer once it finishes rendering: the PSP-side
    /// equivalent of `sc1PStageClearCopyFramebufToWallpaper` (RE-191).
    pub fn request_wallpaper_capture(&mut self) {
        self.wallpaper_capture_requested = true;
    }

    /// sc1PStageClearCopyFramebufToWallpaper: sample the last completed
    /// display's N64 active picture before opening the result frame.
    /// Store 300x220 native texels; drawing maps them back to the same crop.
    pub fn capture_campaign_wallpaper(&self) {
        debug_assert!(!self.frame_open);
        unsafe {
            let src = if self.draw_is_fbp0 { self.fbp1_direct } else { self.fbp0_direct } as *const u32;
            let (vx, _, vw, vh) = ssb_engine::coord::pillarboxed_viewport();
            let dst = core::ptr::addr_of_mut!(WALLPAPER_PHOTO.0) as *mut u32;
            // Each column's source, computed once rather than per pixel
            // (RE-476): the same expression, so the same pixels.
            let mut cols = [0u16; WALLPAPER_PHOTO_WIDTH];
            for (x, c) in cols.iter_mut().enumerate() {
                *c = (vx as usize + ((x as f32 + 10.5) * vw as f32 / 320.0) as usize) as u16;
            }
            for y in 0..WALLPAPER_PHOTO_HEIGHT {
                let sy = ((y as f32 + 10.5) * vh as f32 / 240.0) as usize;
                let row = src.add(sy * BUF_WIDTH as usize);
                let out = dst.add(y * WALLPAPER_PHOTO_STRIDE);
                for (x, &sx) in cols.iter().enumerate() {
                    out.add(x).write(row.add(sx as usize).read());
                }
            }
            let bytes = wallpaper_photo_data();
            sys::sceKernelDcacheWritebackRange(bytes.as_ptr() as *const c_void, bytes.len() as u32);
        }
    }

    /// Copies the top-left 300x220 corner of whichever buffer just finished
    /// rendering into [`WALLPAPER_PHOTO`]. Same safety contract, pillarbox
    /// offset reasoning and draw-buffer selection as
    /// [`Gpu::capture_transition_photo`]; unlike that capture, RE-191 found
    /// no periodic wrap-fill is needed here, so this is a single plain copy.
    ///
    /// # Safety
    ///
    /// Same as [`Gpu::capture_transition_photo`]: only between
    /// `sceGuSync(Finish, Wait)` completing and the next `sceGuSwapBuffers`.
    unsafe fn capture_wallpaper_photo(&self) {
        let src = if self.draw_is_fbp0 {
            self.fbp0_direct
        } else {
            self.fbp1_direct
        } as *const u32;
        let (vx, _, _, _) = ssb_engine::coord::pillarboxed_viewport();
        let dst = core::ptr::addr_of_mut!(WALLPAPER_PHOTO.0) as *mut u32;
        for y in 0..WALLPAPER_PHOTO_HEIGHT {
            let src_row = src.add(y * BUF_WIDTH as usize + vx as usize);
            let dst_row = dst.add(y * WALLPAPER_PHOTO_STRIDE);
            core::ptr::copy_nonoverlapping(src_row, dst_row, WALLPAPER_PHOTO_WIDTH);
        }
        // Same GE DMA coherency requirement as the transition capture.
        let bytes = wallpaper_photo_data();
        sys::sceKernelDcacheWritebackRange(bytes.as_ptr() as *const c_void, bytes.len() as u32);
    }

    /// Debug-only proof that [`WALLPAPER_PHOTO`] holds real pixel data:
    /// overwrites the absolute top-left corner of the buffer currently being
    /// drawn into (inside the pillarbox bar and slightly beyond it, not
    /// scissored, so it is never touched by ordinary scene rendering) with
    /// the last capture, via a plain CPU block copy -- the same direct-VRAM-
    /// write approach `sceGuDebugFlush`'s own glyph overlay already uses.
    /// Only called by the `wallpaper_audit_capture` build (`main.rs`);
    /// RE-191's "Remaining scope" still blocks a real render-path caller.
    ///
    /// # Safety
    ///
    /// Must be called while `self` owns the buffer currently being drawn
    /// into, before that frame's `sceGuSwapBuffers` (i.e. between
    /// `begin_frame` and `end_frame`), the same window ordinary draw calls
    /// use.
    pub unsafe fn blit_wallpaper_debug(&self) {
        let dst = if self.draw_is_fbp0 {
            self.fbp0_direct
        } else {
            self.fbp1_direct
        } as *mut u32;
        let src = core::ptr::addr_of!(WALLPAPER_PHOTO.0) as *const u32;
        for y in 0..WALLPAPER_PHOTO_HEIGHT {
            let dst_row = dst.add(y * BUF_WIDTH as usize);
            let src_row = src.add(y * WALLPAPER_PHOTO_STRIDE);
            core::ptr::copy_nonoverlapping(src_row, dst_row, WALLPAPER_PHOTO_WIDTH);
        }
    }

    /// Draws the last wallpaper capture back onto the screen as a real GE
    /// sprite: the PSP-side equivalent of
    /// `sc1PStageClearWallpaperProcDisplay`/`sc1PStageClearMakeWallpaper`
    /// (`sc/sc1pmode/sc1pstageclear.c:1542-1579`, RE-193). Unlike every other
    /// draw in this renderer, a real `SObj` sprite draw bypasses the 3D
    /// transform pipeline entirely -- the RDP's `gSPTextureRectangle` reads
    /// literal screen pixels, not world/view/projection-transformed
    /// vertices. [`GuPrimitive::Sprites`] plus [`VertexType::TRANSFORM_2D`]
    /// is the PSP GE's own native equivalent of that same shape, so this
    /// deliberately does not reuse `crate::meshdraw`'s 3D quad-through-a-
    /// camera technique the way `ResultsTransition`'s LB-transition quad
    /// does -- that mechanism has a real ROM display list binding a
    /// `ROLE_FRAMEBUFFER` `MObj` texture, but the wallpaper has no such
    /// display list at all; the real ROM draws it as a 2D sprite, never
    /// through `MObj`/`DObj`.
    ///
    /// Reproduces exactly the one real draw call's own fixed parameters,
    /// not a general `SObj`/`Sprite` decoder (RE-190 already found no
    /// `romtool` `Sprite` decoder exists or is needed here): single
    /// bitmap, `scalex`/`scaley` == 1.0 (`sobj->pos.x/y = 10.0`, no scale
    /// field ever set), opaque (`G_RM_OPA_SURF`), and colour-modulated by
    /// the real draw's own `gDPSetPrimColor(0, 0, 0x80, 0x80, 0x80, 0xFF)`
    /// -- a 50% grey dim, not full brightness (`sc1pstageclear.c:1546`).
    ///
    /// Draws back at the identical pixel rectangle
    /// [`Gpu::capture_wallpaper_photo`] read from (the pillarbox's own left
    /// edge, row 0), not a separately re-derived N64-to-PSP scale: the
    /// capture already holds pixels at the PSP's own display scale, so
    /// redrawing them at their own source coordinates reproduces the
    /// original same-position photocopy with no additional conversion.
    ///
    /// # Safety
    ///
    /// Must be called between `begin_frame` and `end_frame`, the same window
    /// every other GE draw call uses.
    pub unsafe fn draw_wallpaper_sprite(&self) {
        let (vx, _, _, _) = ssb_engine::coord::pillarboxed_viewport();
        self.draw_wallpaper_rect([vx as i16, 0, (vx as usize + WALLPAPER_PHOTO_WIDTH) as i16, WALLPAPER_PHOTO_HEIGHT as i16]);
    }

    /// Draw the campaign snapshot at sc1PStageClearMakeWallpaper's (10,10).
    pub unsafe fn draw_campaign_wallpaper(&self) {
        let (vx, _, vw, vh) = ssb_engine::coord::pillarboxed_viewport();
        self.draw_wallpaper_rect([
            (vx as f32 + 10.0 * vw as f32 / 320.0) as i16,
            (10.0 * vh as f32 / 240.0) as i16,
            (vx as f32 + 310.0 * vw as f32 / 320.0) as i16,
            (230.0 * vh as f32 / 240.0) as i16,
        ]);
    }

    /// The opening room's last picture, held under its transition
    /// (RE-467): the snapshot [`Gpu::capture_campaign_wallpaper`] took,
    /// unshaded, at the picture's own place.
    pub unsafe fn draw_frozen_picture(&self) {
        let (vx, _, vw, vh) = ssb_engine::coord::pillarboxed_viewport();
        self.draw_photo_rect(
            [
                (vx as f32 + 10.0 * vw as f32 / 320.0) as i16,
                (10.0 * vh as f32 / 240.0) as i16,
                (vx as f32 + 310.0 * vw as f32 / 320.0) as i16,
                (230.0 * vh as f32 / 240.0) as i16,
            ],
            0xFFFF_FFFF,
        );
    }

    /// The depth over the current scissor set to the nearest value, which
    /// nothing drawn later passes: `mvOpeningRoomTransitionOutlineProcDisplay`'s
    /// black fill of the depth image (RE-467).
    pub fn fill_depth_near(&mut self) {
        unsafe {
            sys::sceGuClearDepth(65535);
            sys::sceGuClear(ClearBuffer::DEPTH_BUFFER_BIT);
        }
    }

    /// Draws what follows into the depth buffer alone, at the far depth:
    /// colour writes masked, every fragment passing (`G_RM_PASS` into the
    /// depth image with white primitive colour). `false` restores the
    /// renderer's state.
    pub fn depth_far_only(&mut self, on: bool) {
        unsafe {
            if on {
                sys::sceGuPixelMask(0xFFFF_FFFF);
                sys::sceGuDepthRange(0, 0);
                sys::sceGuDepthFunc(DepthFunc::Always);
            } else {
                sys::sceGuPixelMask(0);
                sys::sceGuDepthRange(65535, 0);
                sys::sceGuDepthFunc(DepthFunc::GreaterOrEqual);
            }
        }
    }

    unsafe fn draw_wallpaper_rect(&self, rect: [i16; 4]) {
        // ABGR-packed 0x80 red/green/blue, 0xFF alpha -- matches the real
        // draw's own `gDPSetPrimColor(0, 0, 0x80, 0x80, 0x80, 0xFF)`.
        self.draw_photo_rect(rect, 0xFF80_8080);
    }

    unsafe fn draw_photo_rect(&self, [x0, y0, x1, y1]: [i16; 4], prim_color: u32) {
        let data = wallpaper_photo_data();

        sys::sceGuEnable(GuState::Texture2D);
        sys::sceGuDisable(GuState::Lighting);
        sys::sceGuDisable(GuState::DepthTest);
        sys::sceGuDisable(GuState::Blend);

        sys::sceGuTexMode(TexturePixelFormat::Psm8888, 0, 0, 0);
        sys::sceGuTexImage(
            sys::MipmapLevel::None,
            WALLPAPER_PHOTO_STRIDE as i32,
            WALLPAPER_PHOTO_PADDED_HEIGHT as i32,
            WALLPAPER_PHOTO_STRIDE as i32,
            data.as_ptr() as *const c_void,
        );
        sys::sceGuTexFilter(sys::TextureFilter::Linear, sys::TextureFilter::Linear);
        sys::sceGuTexWrap(sys::GuTexWrapMode::Clamp, sys::GuTexWrapMode::Clamp);
        // With an identity scale/offset, `TEXTURE_32BITF` UV is a raw texel
        // address, not a 0..1 fraction (confirmed on device: a fractional
        // corner here samples almost only the top-left texel, smearing one
        // dark background pixel across the whole quad). These corners stop
        // at the real 300x220 content, not the padded 512x256 buffer.
        sys::sceGuTexScale(1.0, 1.0);
        sys::sceGuTexOffset(0.0, 0.0);
        sys::sceGuTexFunc(
            sys::TextureEffect::Modulate,
            sys::TextureColorComponent::Rgba,
        );

        let u1 = WALLPAPER_PHOTO_WIDTH as f32;
        let v1 = WALLPAPER_PHOTO_HEIGHT as f32;

        let verts = [
            SpriteVertex {
                u: 0.0,
                v: 0.0,
                color: prim_color,
                x: x0,
                y: y0,
                z: 0,
                _pad: 0,
            },
            SpriteVertex {
                u: u1,
                v: v1,
                color: prim_color,
                x: x1,
                y: y1,
                z: 0,
                _pad: 0,
            },
        ];
        let dynamic = sys::sceGuGetMemory(core::mem::size_of_val(&verts) as i32) as *mut SpriteVertex;
        core::ptr::copy_nonoverlapping(verts.as_ptr(), dynamic, verts.len());
        sys::sceGuDrawArray(
            GuPrimitive::Sprites,
            SpriteVertex::FORMAT,
            2,
            core::ptr::null(),
            dynamic as *const c_void,
        );

        sys::sceGuEnable(GuState::DepthTest);
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
    ///
    /// The clear always covers the full screen, regardless of whichever
    /// scissor a *previous* frame's [`Gpu::set_viewport_pillarboxed`] left
    /// active: the GE's scissor register is not reset by `sceGuStart`, so a
    /// pillarboxed caller's own narrowing call (made *after* `begin_frame`
    /// returns, once per frame) would otherwise leave this frame's clear
    /// scissored to last frame's narrow rectangle too. With two swap-chain
    /// buffers, only the very first presented frame ever ran under the
    /// full-screen scissor `init` sets up -- every later frame narrows the
    /// scissor before the *next* frame's clear runs, so the buffer that
    /// first frame did *not* land on never received a full clear at all,
    /// and its pillarbox border kept whatever was in that VRAM region at
    /// allocation time for the rest of the program's life. Resetting the
    /// scissor here first makes every clear -- on both buffers, every frame
    /// -- actually cover the screen; the caller's own narrowing call right
    /// after this one still applies to that frame's subsequent 3D draws.
    pub fn begin_frame(&mut self, clear: Option<Color>) {
        debug_assert!(!self.frame_open, "begin_frame called twice");
        self.frame_open = true;
        GE_LIST_OPEN.store(true, core::sync::atomic::Ordering::Relaxed);
        unsafe {
            self.wait_presented();
            sys::sceGuStart(GuContextType::Direct, Self::list_ptr());
            // The swap is this module's, not `sceGuSwapBuffers`' (see
            // `end_frame`), so the target is named every frame. Through
            // `sceGuDrawBuffer` rather than a bare list command, so
            // `sceGuDebugFlush` writes into the same buffer.
            sys::sceGuDrawBuffer(
                DisplayPixelFormat::Psm8888,
                if self.draw_is_fbp0 { self.fbp0_rel } else { self.fbp1_rel },
                BUF_WIDTH as i32,
            );
            if let Some(c) = clear {
                sys::sceGuScissor(0, 0, SCREEN_WIDTH as i32, SCREEN_HEIGHT as i32);
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
            let t = crate::profile::start();
            sys::sceGuSync(GuSyncMode::Finish, GuSyncBehavior::Wait);
            crate::profile::stop(crate::profile::Span::GeSync, t);
            GE_LIST_OPEN.store(false, core::sync::atomic::Ordering::Relaxed);
            // Debug text must be painted *here*, not earlier. sceGuDebugFlush
            // writes glyphs straight into the draw buffer rather than queueing
            // a GE command, so flushing before the sync would just get erased
            // by the sceGuClear that is still sitting in the display list.
            //
            // Called unconditionally, with no glyphs queued unless a caller
            // used `debug_text` this frame: RE-202 found the real hardware
            // crash lives in `sceGuDebugPrint`/`sceGuDebugFlush`'s own state
            // once glyphs are actually written, not in an unconditional
            // `sceGuDebugFlush` call against an empty buffer -- the fix
            // gates the `debug_text` call (`debug_overlay` feature), not
            // this one.
            sys::sceGuDebugFlush();
            if self.capture_requested {
                self.capture_requested = false;
                self.capture_transition_photo();
            }
            if self.wallpaper_capture_requested {
                self.wallpaper_capture_requested = false;
                self.capture_wallpaper_photo();
            }
            // RE-470: the finished frame is shown from the next vblank on
            // (`NextFrame`), without waiting for it here. The game is
            // CPU-bound, so the CPU starts the next frame's update at once,
            // and only the next `begin_frame` waits, if no vblank has
            // latched this frame yet. Waiting here instead (and swapping
            // with `sceGuSwapBuffers`' `Immediate`) rounded every frame up
            // to whole vblanks: a 20 ms frame took 33. A frame still never
            // takes less than one vblank, and the GE never draws into the
            // buffer on screen.
            let drawn = if self.draw_is_fbp0 { self.fbp0_rel } else { self.fbp1_rel };
            sys::sceDisplaySetFrameBuf(
                sys::sceGeEdramGetAddr().add(drawn as usize) as *const u8,
                BUF_WIDTH as usize,
                DisplayPixelFormat::Psm8888,
                sys::DisplaySetBufSync::NextFrame,
            );
            self.presented_vcount = Some(sys::sceDisplayGetVcount());
        }
        // The buffer that was the draw target for the frame just finished
        // becomes the display buffer, and the GE draws the next frame into
        // whichever buffer was previously being displayed.
        self.draw_is_fbp0 = !self.draw_is_fbp0;
    }

    /// An FNV-1a hash of the visible 480x272 pixels of the frame
    /// [`Gpu::end_frame`] last handed over, for frame-by-frame A/B runs
    /// (`frame_hash`, RE-476).
    pub fn shown_frame_hash(&self) -> u32 {
        let src = if self.draw_is_fbp0 { self.fbp1_direct } else { self.fbp0_direct } as *const u32;
        let mut h: u32 = 0x811C_9DC5;
        for y in 0..272usize {
            for x in 0..480usize {
                // SAFETY: inside the 512-wide buffer's visible rows.
                let p = unsafe { src.add(y * BUF_WIDTH as usize + x).read_volatile() };
                h = (h ^ p).wrapping_mul(0x0100_0193);
            }
        }
        h
    }

    /// Waits until the last frame [`Gpu::end_frame`] handed over is on
    /// screen: until a vblank after it.
    pub fn wait_presented(&mut self) {
        let Some(vcount) = self.presented_vcount else {
            return;
        };
        let t = crate::profile::start();
        // SAFETY: display queries and waits have no preconditions.
        unsafe {
            while sys::sceDisplayGetVcount() == vcount {
                sys::sceDisplayWaitVblankStart();
            }
        }
        crate::profile::stop(crate::profile::Span::Vblank, t);
        self.presented_vcount = None;
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

    /// Replaces the projection with an orthographic camera.
    pub fn set_ortho(&mut self, bounds: [f32; 4], near: f32, far: f32) {
        let [left, right, bottom, top] = bounds;
        unsafe {
            sys::sceGumMatrixMode(sys::MatrixMode::Projection);
            sys::sceGumLoadIdentity();
            sys::sceGumOrtho(left, right, bottom, top, near, far);
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

    /// Translates, rotates and uniformly scales the model matrix.
    ///
    /// Calls post-multiply, so the effective transform is `T * R * S`:
    /// vertices are scaled first, then rotated, then translated.
    pub fn model_transform(&mut self, pos: [f32; 3], rot_radians: [f32; 3], scale: f32) {
        self.model_transform_xyz(pos, rot_radians, [scale; 3]);
    }

    /// Loads a screen-aligned model transform: `gcPrepDObjMatrix` kind 46
    /// (`objdisplay.c`), which keeps the DObj's translated position, replaces
    /// its rotation with the camera's own axes and spins it by `rotate.z` in
    /// the screen plane. `eye`/`at` must be the ones given to the `look_at`
    /// view (world up is Y), so the object's X/Y axes are the camera's
    /// right/up. `scale` applies to X, Y and, as in the source, Z.
    pub fn model_transform_billboard(
        &mut self,
        pos: ssb_engine::math::Vec3,
        eye: ssb_engine::math::Vec3,
        at: ssb_engine::math::Vec3,
        spin: f32,
        scale: [f32; 2],
    ) {
        let forward = (at - eye).normalized();
        let right = forward.cross(ssb_engine::math::Vec3::Y).normalized();
        let up = right.cross(forward);
        let (s, c) = ssb_engine::math::sin_cos(spin);
        // Column 0 is the object's X axis after the in-plane spin, column 1
        // its Y axis, column 2 points back at the camera.
        let x = right * c + up * s;
        let y = up * c - right * s;
        let z = -forward;
        let axis = |v: ssb_engine::math::Vec3, k: f32| sys::ScePspFVector4 {
            x: v.x * k,
            y: v.y * k,
            z: v.z * k,
            w: 0.0,
        };
        let matrix = sys::ScePspFMatrix4 {
            x: axis(x, scale[0]),
            y: axis(y, scale[1]),
            z: axis(z, scale[0]),
            w: sys::ScePspFVector4 {
                x: pos.x,
                y: pos.y,
                z: pos.z,
                w: 1.0,
            },
        };
        unsafe {
            sys::sceGumMatrixMode(sys::MatrixMode::Model);
            sys::sceGumLoadMatrix(&matrix);
        }
    }

    /// Loads battle matrix function 0x47 (`func_ovl0_800CA5C8`, RE-418): the
    /// DObj keeps its translated position, and its rotation, relative to the
    /// camera rather than the world, is `rotate.x` then `rotate.y`. The
    /// source writes the rows `R · P` over the MVP's rotation, with row-vector
    /// `R` = `Rx(rotate.x) · Ry(rotate.y)`: the object's X, Y and Z axes are
    /// `(cos y, 0, -sin y)`, `(sin x sin y, cos x, sin x cos y)` and
    /// `(cos x sin y, -sin x, cos x cos y)` in the camera's right/up/back
    /// basis. With `rotate.y` at +/-90 degrees (`wpMainVelSetModelPitch`),
    /// `rotate.x` spins the object in the screen plane. No DObj scale is
    /// applied; `scale` is the port's model scale. `eye`/`at` are the
    /// `look_at` view's, as for [`Self::model_transform_billboard`].
    pub fn model_transform_camera_rotated(
        &mut self,
        pos: ssb_engine::math::Vec3,
        eye: ssb_engine::math::Vec3,
        at: ssb_engine::math::Vec3,
        rotate: [f32; 2],
        scale: f32,
    ) {
        self.model_transform_camera_rotated_scaled(pos, eye, at, rotate, [scale; 2]);
    }

    /// Battle matrix function 0x48 (`func_ovl0_800CAB48`): function 0x47
    /// with the `DObj` scale, X on the object's X and Z axes and Y on its Y
    /// axis (`scale` includes the port's model scale).
    pub fn model_transform_camera_rotated_scaled(
        &mut self,
        pos: ssb_engine::math::Vec3,
        eye: ssb_engine::math::Vec3,
        at: ssb_engine::math::Vec3,
        rotate: [f32; 2],
        scale: [f32; 2],
    ) {
        let forward = (at - eye).normalized();
        let right = forward.cross(ssb_engine::math::Vec3::Y).normalized();
        let up = right.cross(forward);
        let back = -forward;
        let (sx, cx) = ssb_engine::math::sin_cos(rotate[0]);
        let (sy, cy) = ssb_engine::math::sin_cos(rotate[1]);
        let camera = |r: f32, u: f32, b: f32, k: f32| {
            let v = (right * r + up * u + back * b) * k;
            sys::ScePspFVector4 {
                x: v.x,
                y: v.y,
                z: v.z,
                w: 0.0,
            }
        };
        let matrix = sys::ScePspFMatrix4 {
            x: camera(cy, 0.0, -sy, scale[0]),
            y: camera(sx * sy, cx, sx * cy, scale[1]),
            z: camera(cx * sy, -sx, cx * cy, scale[0]),
            w: sys::ScePspFVector4 {
                x: pos.x,
                y: pos.y,
                z: pos.z,
                w: 1.0,
            },
        };
        unsafe {
            sys::sceGumMatrixMode(sys::MatrixMode::Model);
            sys::sceGumLoadMatrix(&matrix);
        }
    }

    /// Loads a held fighter's TopN transform from its catcher's sampled joint.
    /// The source extracts rotation from the joint matrix after removing
    /// scale and writes it over TopN's facing yaw, so no facing rotation
    /// follows (RE-371); the held fighter's own model scale remains `scale`.
    pub fn model_transform_joint(
        &mut self,
        pos: ssb_engine::math::Vec3,
        joint: ssb_game::fighter::JointTransform,
        scale: f32,
    ) {
        let axes = ssb_game::grab::held_root_axes(joint);
        let axis = |i: usize| {
            let v = axes[i] * scale;
            sys::ScePspFVector4 {
                x: v.x,
                y: v.y,
                z: v.z,
                w: 0.0,
            }
        };
        let matrix = sys::ScePspFMatrix4 {
            x: axis(0),
            y: axis(1),
            z: axis(2),
            w: sys::ScePspFVector4 {
                x: pos.x,
                y: pos.y,
                z: pos.z,
                w: 1.0,
            },
        };
        unsafe {
            sys::sceGumMatrixMode(sys::MatrixMode::Model);
            sys::sceGumLoadMatrix(&matrix);
        }
    }

    /// Sets a model transform with independent axis scales, used by source
    /// weapons whose `DObj` animates only its X scale.
    pub fn model_transform_xyz(&mut self, pos: [f32; 3], rot_radians: [f32; 3], scale: [f32; 3]) {
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
                x: scale[0],
                y: scale[1],
                z: scale[2],
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
    ///
    /// RE-202: sustained per-frame use of this method reliably crashes real
    /// PSP hardware, content-independent. Callers must gate it behind an
    /// off-by-default feature the way `psp-asset-viewer`'s `debug_overlay`
    /// does; a front end shipping this unconditionally would carry the same
    /// fault.
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
    /// Copies `verts` into memory allocated from the current display-list
    /// arena before submitting, the same `sceGuGetMemory` pattern
    /// [`Gpu::draw_line_strip`] uses and for the same reason: the GE reads
    /// submitted vertex data asynchronously, and a caller that reuses one
    /// scratch buffer across several draws in the same frame (`depth_diag.rs`
    /// does exactly this, one static buffer for three sequential quads) can
    /// have real hardware's GE still reading an earlier draw's vertices out
    /// of that buffer after the CPU has already overwritten it with the next
    /// one -- silently substituting the last draw's geometry into the
    /// earlier, still-pending draw calls. PPSSPP's GE emulation keeps pace
    /// with the CPU closely enough that this never shows up there.
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

    /// Draws one open polyline, joining each vertex to the next.
    ///
    /// Used for the collision overlay, where the data really is a polyline —
    /// drawing it as anything else would misrepresent what the game stores.
    /// Depth testing is left on so a line behind stage geometry is occluded by
    /// it, which is what makes the overlay readable as "in the scene" rather
    /// than painted on top.
    ///
    /// Copies `verts` into memory allocated from the current display-list
    /// arena (the same `sceGuGetMemory` pattern `draw_object_posed` uses for
    /// its dynamic vertices) before submitting. The GE reads submitted vertex
    /// data asynchronously, and both callers reuse one small static scratch
    /// buffer for every segment drawn in a frame; without this copy, real
    /// hardware's GE can still be reading an earlier segment's vertices out of
    /// that buffer after the CPU has already overwritten it with the next
    /// one, corrupting whichever draw the GE was behind on. PPSSPP's GE
    /// emulation keeps pace with the CPU closely enough that this never shows
    /// up there, which is why it only reproduces on physical hardware.
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

    /// Draws one filled, axis-aligned rectangle in screen-pixel coordinates.
    /// `GuPrimitive::Sprites` fills the box between two opposite corners, the
    /// same primitive [`Gpu::draw_wallpaper_sprite`]'s 2D drawing uses.
    ///
    /// Brackets depth test and culling off around the draw and restores them
    /// after: `Gpu::init` defaults both on for real 3D content, and a
    /// `TRANSFORM_2D` primitive must neither be depth-tested against stale
    /// buffer contents nor culled by a winding order that has no meaning in
    /// screen space.
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
}

impl Gpu {
    /// A `G_CYC_FILL` rectangle: flat `color`, untextured, unblended, with
    /// no alpha or depth test, whatever the previous draw left enabled. The
    /// caller invalidates its cached draw state afterwards.
    pub fn draw_rect_fill(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, color: Color) {
        unsafe {
            sys::sceGuDisable(GuState::Texture2D);
            sys::sceGuDisable(GuState::AlphaTest);
            sys::sceGuDisable(GuState::Blend);
        }
        self.draw_rect(x0, y0, x1, y1, color);
    }

    /// `ifScreenFlashProcDisplay`'s fill: a flat `G_CC_PRIMITIVE` rectangle
    /// blended over the frame (`G_RM_AA_XLU_SURF`), untextured, with no
    /// depth test. The caller invalidates its cached draw state afterwards.
    pub fn draw_rect_translucent(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, color: Color) {
        unsafe {
            sys::sceGuDisable(GuState::Texture2D);
            sys::sceGuDisable(GuState::AlphaTest);
            sys::sceGuEnable(GuState::Blend);
            sys::sceGuBlendFunc(
                sys::BlendOp::Add,
                sys::BlendFactor::SrcAlpha,
                sys::BlendFactor::OneMinusSrcAlpha,
                0,
                0,
            );
        }
        self.draw_rect(x0, y0, x1, y1, color);
    }

    /// A fog whose factor is the same for everything near view depth
    /// `depth`: `G_RM_FOG_PRIM_A`'s blend of each pixel towards `rgba`'s
    /// colour by its alpha (`ftDisplayMainSetFogColor`). The GE fogs by
    /// `(far - depth) / (far - near)`, so a window `SPAN` wide placed to
    /// give `1 - alpha` at `depth` varies by under 1/255 across a fighter.
    pub fn set_constant_fog(&mut self, depth: f32, rgba: [u8; 4]) {
        const SPAN: f32 = 1.0e5;
        let keep = 1.0 - f32::from(rgba[3]) / 255.0;
        let far = depth + keep * SPAN;
        let color = u32::from_le_bytes([rgba[0], rgba[1], rgba[2], 0]);
        unsafe {
            sys::sceGuFog(far - SPAN, far, color);
            sys::sceGuEnable(GuState::Fog);
        }
    }

    /// Ends [`Self::set_constant_fog`].
    pub fn clear_fog(&mut self) {
        unsafe {
            sys::sceGuDisable(GuState::Fog);
        }
    }
}

/// Ask PPSSPPHeadless to save the current display framebuffer. Real PSPs do
/// not implement the emulator-only devctl, so the same build remains safe to
/// load on hardware (where the call simply returns an error). Callers gate
/// the call site behind their own `headless_capture` feature; this function
/// itself is unconditional so both applications share one implementation.
pub fn emit_headless_screenshot() {
    const EMULATOR_DEVCTL_EMIT_SCREENSHOT: u32 = 0x20;

    unsafe {
        sys::sceIoDevctl(
            b"emulator:\0".as_ptr(),
            EMULATOR_DEVCTL_EMIT_SCREENSHOT,
            core::ptr::null_mut(),
            0,
            core::ptr::null_mut(),
            0,
        );
    }
}
