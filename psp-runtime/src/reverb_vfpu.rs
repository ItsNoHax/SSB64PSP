//! The reverb's plain sections on the VFPU ([`section`], an
//! `ssb_engine::audio::reverb::SectionKernel`), bit-identical to the
//! integer path: `reverb::vfpu_model` is this kernel step for step in
//! `f32`, with the proof that every intermediate is exact and the host
//! tests against `mixq`; the `audio_selftest` hashes check it on the PSP.
//!
//! The step bodies are list-scheduled for the VFPU's latency (generated:
//! one body per set of nonzero coefficients, as `section` skips a zero
//! coefficient's mix). Registers: matrices 0-7, the constants loaded on
//! every call (no state kept between calls). The audio thread owns its
//! VFPU context; [`with_saved`] keeps the game thread's registers (gum's
//! matrices) around the selftest.
//!
//! Written by `tools/audio/gen_reverb_vfpu.py` (code only, no ROM data):
//! change the script and rerun it; never edit this file by hand.

/// Constants: `C000` = `[ff >> 8, ff & 255, fb >> 8, fb & 255]` and `C010`
/// = `[gain >> 8, gain & 255, -, -]` as `vfpu_model::split` scales them;
/// `C020` = 1/2, `C030` = 32767, `C100` = -32768.
#[repr(C, align(16))]
struct Consts([f32; 20]);

/// `y = mixq(y, x, ff)`, `x = mixq(x, y, fb)`, `out = mixq(out, y, gain)`
/// per sample (a zero coefficient's mix skipped), 8 samples per step as
/// two groups of 4.
///
/// # Safety
///
/// As `SectionKernel`: `x`, `y`, `out` 16-byte aligned, disjoint, `n`
/// samples each, `n` a multiple of 8; the calling thread has the VFPU.
pub unsafe fn section(x: *mut i16, y: *mut i16, out: *mut i16, n: usize, coef: [i32; 3]) {
    let split = |g: i32| ((g >> 8) as f32 / 128.0, (g & 255) as f32 / 32768.0);
    let ((ffh, ffl), (fbh, fbl), (gh, gl)) = (split(coef[0]), split(coef[1]), split(coef[2]));
    let h = 0.5;
    let c = Consts([
        ffh, ffl, fbh, fbl, gh, gl, 0.0, 0.0, h, h, h, h, 32767.0, 32767.0, 32767.0, 32767.0,
        -32768.0, -32768.0, -32768.0, -32768.0,
    ]);
    // SAFETY: `c` is a live, 16-byte-aligned 80-byte block.
    unsafe {
        psp::vfpu_asm!(
            "lv.q C000,  0({c})",
            "lv.q C010, 16({c})",
            "lv.q C020, 32({c})",
            "lv.q C030, 48({c})",
            "lv.q C100, 64({c})",
            c = in(reg) (&c),
            options(nostack),
        );
    }
    let mask = (coef[0] != 0) as u8 | ((coef[1] != 0) as u8) << 1 | ((coef[2] != 0) as u8) << 2;
    // One loop per mask, the step inlined (no call per 8 samples).
    macro_rules! run {
        ($step:ident) => {{
            let mut k = 0;
            while k < n {
                // SAFETY: the caller's contract: 8 aligned samples at `k`
                // in each window.
                unsafe { $step(x.add(k), y.add(k), out.add(k)) };
                k += 8;
            }
        }};
    }
    match mask {
        1 => run!(step1),
        2 => run!(step2),
        3 => run!(step3),
        4 => run!(step4),
        5 => run!(step5),
        6 => run!(step6),
        7 => run!(step7),
        _ => {}
    }
}

/// One step of [`section`] with ff nonzero (45 instructions).
///
/// # Safety
///
/// As [`section`], for 8 samples.
#[inline(always)]
unsafe fn step1(x: *mut i16, y: *mut i16, _o: *mut i16) {
    // SAFETY: the caller's contract.
    unsafe {
        psp::vfpu_asm!(
            "lv.q C110, 0({x})",
            "lv.q C120, 0({y})",
            "vs2i.p C630, C110",
            "vs2i.p C720, C112",
            "vs2i.p C700, C120",
            "vs2i.p C730, C122",
            "vi2f.q C200, C630, 16",
            "vi2f.q C410, C720, 16",
            "vi2f.q C230, C700, 31",
            "vi2f.q C500, C730, 31",
            "vi2f.q C220, C700, 16",
            "vscl.q C320, C200, S000",
            "vscl.q C530, C410, S000",
            "vsub.q C230, C020, C230",
            "vscl.q C400, C200, S001",
            "vsub.q C500, C020, C500",
            "vf2id.q C330, C320, 0",
            "vf2id.q C600, C530, 0",
            "vscl.q C610, C410, S001",
            "vadd.q C400, C400, C230",
            "vi2f.q C430, C730, 16",
            "vi2f.q C330, C330, 0",
            "vi2f.q C600, C600, 0",
            "vadd.q C610, C610, C500",
            "vsub.q C320, C320, C330",
            "vsub.q C530, C530, C600",
            "vadd.q C220, C220, C330",
            "vadd.q C430, C430, C600",
            x = in(reg) x,
            y = in(reg) y,
            options(nostack),
        );
        psp::vfpu_asm!(
            "vadd.q C320, C320, C400",
            "vadd.q C530, C530, C610",
            "vf2id.q C320, C320, 0",
            "vf2id.q C530, C530, 0",
            "vi2f.q C320, C320, 0",
            "vi2f.q C530, C530, 0",
            "vadd.q C220, C220, C320",
            "vadd.q C430, C430, C530",
            "vmin.q C220, C220, C030",
            "vmin.q C430, C430, C030",
            "vmax.q C220, C220, C100",
            "vmax.q C430, C430, C100",
            "vf2iz.q C230, C220, 16",
            "vf2iz.q C500, C430, 16",
            "vi2s.q C120, C230",
            "vi2s.q C122, C500",
            "sv.q C120, 0({y})",
            y = in(reg) y,
            options(nostack),
        );
    }
}

/// One step of [`section`] with fb nonzero (45 instructions).
///
/// # Safety
///
/// As [`section`], for 8 samples.
#[inline(always)]
unsafe fn step2(x: *mut i16, y: *mut i16, _o: *mut i16) {
    // SAFETY: the caller's contract.
    unsafe {
        psp::vfpu_asm!(
            "lv.q C120, 0({y})",
            "lv.q C110, 0({x})",
            "vs2i.p C700, C120",
            "vs2i.p C730, C122",
            "vs2i.p C630, C110",
            "vs2i.p C720, C112",
            "vi2f.q C220, C700, 16",
            "vi2f.q C430, C730, 16",
            "vi2f.q C210, C630, 31",
            "vi2f.q C420, C720, 31",
            "vi2f.q C200, C630, 16",
            "vscl.q C320, C220, S002",
            "vscl.q C530, C430, S002",
            "vsub.q C210, C020, C210",
            "vscl.q C400, C220, S003",
            "vsub.q C420, C020, C420",
            "vf2id.q C330, C320, 0",
            "vf2id.q C600, C530, 0",
            "vscl.q C610, C430, S003",
            "vadd.q C400, C400, C210",
            "vi2f.q C410, C720, 16",
            "vi2f.q C330, C330, 0",
            "vi2f.q C600, C600, 0",
            "vadd.q C610, C610, C420",
            "vsub.q C320, C320, C330",
            "vsub.q C530, C530, C600",
            "vadd.q C200, C200, C330",
            "vadd.q C410, C410, C600",
            x = in(reg) x,
            y = in(reg) y,
            options(nostack),
        );
        psp::vfpu_asm!(
            "vadd.q C320, C320, C400",
            "vadd.q C530, C530, C610",
            "vf2id.q C320, C320, 0",
            "vf2id.q C530, C530, 0",
            "vi2f.q C320, C320, 0",
            "vi2f.q C530, C530, 0",
            "vadd.q C200, C200, C320",
            "vadd.q C410, C410, C530",
            "vmin.q C200, C200, C030",
            "vmin.q C410, C410, C030",
            "vmax.q C200, C200, C100",
            "vmax.q C410, C410, C100",
            "vf2iz.q C210, C200, 16",
            "vf2iz.q C420, C410, 16",
            "vi2s.q C110, C210",
            "vi2s.q C112, C420",
            "sv.q C110, 0({x})",
            x = in(reg) x,
            options(nostack),
        );
    }
}

/// One step of [`section`] with ff, fb nonzero (80 instructions).
///
/// # Safety
///
/// As [`section`], for 8 samples.
#[inline(always)]
unsafe fn step3(x: *mut i16, y: *mut i16, _o: *mut i16) {
    // SAFETY: the caller's contract.
    unsafe {
        psp::vfpu_asm!(
            "lv.q C110, 0({x})",
            "lv.q C120, 0({y})",
            "vs2i.p C630, C110",
            "vs2i.p C720, C112",
            "vs2i.p C700, C120",
            "vs2i.p C730, C122",
            "vi2f.q C200, C630, 16",
            "vi2f.q C410, C720, 16",
            "vi2f.q C230, C700, 31",
            "vi2f.q C500, C730, 31",
            "vi2f.q C220, C700, 16",
            "vscl.q C320, C200, S000",
            "vscl.q C530, C410, S000",
            "vsub.q C230, C020, C230",
            "vscl.q C400, C200, S001",
            "vsub.q C500, C020, C500",
            "vf2id.q C330, C320, 0",
            "vf2id.q C600, C530, 0",
            "vscl.q C610, C410, S001",
            "vadd.q C400, C400, C230",
            "vi2f.q C430, C730, 16",
            "vi2f.q C330, C330, 0",
            "vi2f.q C600, C600, 0",
            "vadd.q C610, C610, C500",
            "vi2f.q C210, C630, 31",
            "vi2f.q C420, C720, 31",
            "vsub.q C320, C320, C330",
            "vsub.q C530, C530, C600",
            x = in(reg) x,
            y = in(reg) y,
            options(nostack),
        );
        psp::vfpu_asm!(
            "vadd.q C220, C220, C330",
            "vadd.q C430, C430, C600",
            "vsub.q C210, C020, C210",
            "vadd.q C320, C320, C400",
            "vadd.q C530, C530, C610",
            "vsub.q C420, C020, C420",
            "vf2id.q C320, C320, 0",
            "vf2id.q C530, C530, 0",
            "vi2f.q C320, C320, 0",
            "vi2f.q C530, C530, 0",
            "vadd.q C220, C220, C320",
            "vadd.q C430, C430, C530",
            "vmin.q C220, C220, C030",
            "vmin.q C430, C430, C030",
            "vmax.q C220, C220, C100",
            "vmax.q C430, C430, C100",
            "vscl.q C320, C220, S002",
            "vscl.q C530, C430, S002",
            "vscl.q C400, C220, S003",
            "vscl.q C610, C430, S003",
            "vf2iz.q C230, C220, 16",
            "vf2id.q C330, C320, 0",
            "vf2id.q C600, C530, 0",
            "vadd.q C400, C400, C210",
            "vadd.q C610, C610, C420",
            "vi2s.q C120, C230",
            "vi2f.q C330, C330, 0",
            "vi2f.q C600, C600, 0",
            options(nostack),
        );
        psp::vfpu_asm!(
            "vf2iz.q C500, C430, 16",
            "vsub.q C320, C320, C330",
            "vsub.q C530, C530, C600",
            "vadd.q C200, C200, C330",
            "vadd.q C410, C410, C600",
            "vi2s.q C122, C500",
            "vadd.q C320, C320, C400",
            "vadd.q C530, C530, C610",
            "sv.q C120, 0({y})",
            "vf2id.q C320, C320, 0",
            "vf2id.q C530, C530, 0",
            "vi2f.q C320, C320, 0",
            "vi2f.q C530, C530, 0",
            "vadd.q C200, C200, C320",
            "vadd.q C410, C410, C530",
            "vmin.q C200, C200, C030",
            "vmin.q C410, C410, C030",
            "vmax.q C200, C200, C100",
            "vmax.q C410, C410, C100",
            "vf2iz.q C210, C200, 16",
            "vf2iz.q C420, C410, 16",
            "vi2s.q C110, C210",
            "vi2s.q C112, C420",
            "sv.q C110, 0({x})",
            x = in(reg) x,
            y = in(reg) y,
            options(nostack),
        );
    }
}

/// One step of [`section`] with gain nonzero (45 instructions).
///
/// # Safety
///
/// As [`section`], for 8 samples.
#[inline(always)]
unsafe fn step4(_x: *mut i16, y: *mut i16, o: *mut i16) {
    // SAFETY: the caller's contract.
    unsafe {
        psp::vfpu_asm!(
            "lv.q C120, 0({y})",
            "lv.q C130, 0({o})",
            "vs2i.p C700, C120",
            "vs2i.p C620, C130",
            "vs2i.p C730, C122",
            "vs2i.p C710, C132",
            "vi2f.q C220, C700, 16",
            "vi2f.q C300, C620, 16",
            "vi2f.q C310, C620, 31",
            "vi2f.q C430, C730, 16",
            "vi2f.q C510, C710, 16",
            "vscl.q C620, C220, S010",
            "vi2f.q C520, C710, 31",
            "vsub.q C310, C020, C310",
            "vscl.q C710, C430, S010",
            "vscl.q C700, C220, S011",
            "vf2id.q C630, C620, 0",
            "vsub.q C520, C020, C520",
            "vscl.q C730, C430, S011",
            "vf2id.q C720, C710, 0",
            "vadd.q C700, C700, C310",
            "vi2f.q C630, C630, 0",
            "vadd.q C730, C730, C520",
            "vi2f.q C720, C720, 0",
            "vsub.q C620, C620, C630",
            "vadd.q C300, C300, C630",
            "vsub.q C710, C710, C720",
            "vadd.q C510, C510, C720",
            y = in(reg) y,
            o = in(reg) o,
            options(nostack),
        );
        psp::vfpu_asm!(
            "vadd.q C620, C620, C700",
            "vadd.q C710, C710, C730",
            "vf2id.q C620, C620, 0",
            "vf2id.q C710, C710, 0",
            "vi2f.q C620, C620, 0",
            "vi2f.q C710, C710, 0",
            "vadd.q C300, C300, C620",
            "vadd.q C510, C510, C710",
            "vmin.q C300, C300, C030",
            "vmin.q C510, C510, C030",
            "vmax.q C300, C300, C100",
            "vmax.q C510, C510, C100",
            "vf2iz.q C310, C300, 16",
            "vf2iz.q C520, C510, 16",
            "vi2s.q C130, C310",
            "vi2s.q C132, C520",
            "sv.q C130, 0({o})",
            o = in(reg) o,
            options(nostack),
        );
    }
}

/// One step of [`section`] with ff, gain nonzero (85 instructions).
///
/// # Safety
///
/// As [`section`], for 8 samples.
#[inline(always)]
unsafe fn step5(x: *mut i16, y: *mut i16, o: *mut i16) {
    // SAFETY: the caller's contract.
    unsafe {
        psp::vfpu_asm!(
            "lv.q C110, 0({x})",
            "lv.q C120, 0({y})",
            "lv.q C130, 0({o})",
            "vs2i.p C630, C110",
            "vs2i.p C720, C112",
            "vs2i.p C700, C120",
            "vs2i.p C730, C122",
            "vs2i.p C620, C130",
            "vi2f.q C200, C630, 16",
            "vi2f.q C410, C720, 16",
            "vi2f.q C230, C700, 31",
            "vi2f.q C500, C730, 31",
            "vi2f.q C220, C700, 16",
            "vscl.q C320, C200, S000",
            "vscl.q C530, C410, S000",
            "vsub.q C230, C020, C230",
            "vscl.q C400, C200, S001",
            "vsub.q C500, C020, C500",
            "vf2id.q C330, C320, 0",
            "vf2id.q C600, C530, 0",
            "vscl.q C610, C410, S001",
            "vadd.q C400, C400, C230",
            "vi2f.q C430, C730, 16",
            "vi2f.q C330, C330, 0",
            "vi2f.q C600, C600, 0",
            "vadd.q C610, C610, C500",
            "vi2f.q C300, C620, 16",
            "vi2f.q C310, C620, 31",
            x = in(reg) x,
            y = in(reg) y,
            o = in(reg) o,
            options(nostack),
        );
        psp::vfpu_asm!(
            "vsub.q C320, C320, C330",
            "vsub.q C530, C530, C600",
            "vadd.q C220, C220, C330",
            "vadd.q C430, C430, C600",
            "vs2i.p C710, C132",
            "vadd.q C320, C320, C400",
            "vadd.q C530, C530, C610",
            "vsub.q C310, C020, C310",
            "vi2f.q C510, C710, 16",
            "vf2id.q C320, C320, 0",
            "vf2id.q C530, C530, 0",
            "vi2f.q C520, C710, 31",
            "vi2f.q C320, C320, 0",
            "vi2f.q C530, C530, 0",
            "vsub.q C520, C020, C520",
            "vadd.q C220, C220, C320",
            "vadd.q C430, C430, C530",
            "vmin.q C220, C220, C030",
            "vmin.q C430, C430, C030",
            "vmax.q C220, C220, C100",
            "vmax.q C430, C430, C100",
            "vscl.q C620, C220, S010",
            "vscl.q C710, C430, S010",
            "vscl.q C700, C220, S011",
            "vscl.q C730, C430, S011",
            "vf2iz.q C230, C220, 16",
            "vf2id.q C630, C620, 0",
            "vf2id.q C720, C710, 0",
            options(nostack),
        );
        psp::vfpu_asm!(
            "vadd.q C700, C700, C310",
            "vadd.q C730, C730, C520",
            "vi2s.q C120, C230",
            "vi2f.q C630, C630, 0",
            "vi2f.q C720, C720, 0",
            "vf2iz.q C500, C430, 16",
            "vsub.q C620, C620, C630",
            "vsub.q C710, C710, C720",
            "vadd.q C300, C300, C630",
            "vadd.q C510, C510, C720",
            "vi2s.q C122, C500",
            "vadd.q C620, C620, C700",
            "vadd.q C710, C710, C730",
            "sv.q C120, 0({y})",
            "vf2id.q C620, C620, 0",
            "vf2id.q C710, C710, 0",
            "vi2f.q C620, C620, 0",
            "vi2f.q C710, C710, 0",
            "vadd.q C300, C300, C620",
            "vadd.q C510, C510, C710",
            "vmin.q C300, C300, C030",
            "vmin.q C510, C510, C030",
            "vmax.q C300, C300, C100",
            "vmax.q C510, C510, C100",
            "vf2iz.q C310, C300, 16",
            "vf2iz.q C520, C510, 16",
            "vi2s.q C130, C310",
            "vi2s.q C132, C520",
            y = in(reg) y,
            options(nostack),
        );
        psp::vfpu_asm!(
            "sv.q C130, 0({o})",
            o = in(reg) o,
            options(nostack),
        );
    }
}

/// One step of [`section`] with fb, gain nonzero (85 instructions).
///
/// # Safety
///
/// As [`section`], for 8 samples.
#[inline(always)]
unsafe fn step6(x: *mut i16, y: *mut i16, o: *mut i16) {
    // SAFETY: the caller's contract.
    unsafe {
        psp::vfpu_asm!(
            "lv.q C120, 0({y})",
            "lv.q C130, 0({o})",
            "lv.q C110, 0({x})",
            "vs2i.p C700, C120",
            "vs2i.p C620, C130",
            "vs2i.p C730, C122",
            "vs2i.p C630, C110",
            "vs2i.p C710, C132",
            "vi2f.q C220, C700, 16",
            "vi2f.q C300, C620, 16",
            "vi2f.q C310, C620, 31",
            "vi2f.q C430, C730, 16",
            "vi2f.q C200, C630, 16",
            "vscl.q C320, C220, S002",
            "vscl.q C620, C220, S010",
            "vi2f.q C210, C630, 31",
            "vs2i.p C720, C112",
            "vi2f.q C510, C710, 16",
            "vi2f.q C520, C710, 31",
            "vf2id.q C330, C320, 0",
            "vf2id.q C630, C620, 0",
            "vscl.q C530, C430, S002",
            "vscl.q C710, C430, S010",
            "vi2f.q C410, C720, 16",
            "vi2f.q C420, C720, 31",
            "vsub.q C210, C020, C210",
            "vsub.q C310, C020, C310",
            "vscl.q C400, C220, S003",
            x = in(reg) x,
            y = in(reg) y,
            o = in(reg) o,
            options(nostack),
        );
        psp::vfpu_asm!(
            "vi2f.q C330, C330, 0",
            "vscl.q C700, C220, S011",
            "vi2f.q C630, C630, 0",
            "vf2id.q C600, C530, 0",
            "vf2id.q C720, C710, 0",
            "vsub.q C420, C020, C420",
            "vsub.q C520, C020, C520",
            "vadd.q C400, C400, C210",
            "vsub.q C320, C320, C330",
            "vadd.q C700, C700, C310",
            "vsub.q C620, C620, C630",
            "vscl.q C610, C430, S003",
            "vi2f.q C600, C600, 0",
            "vscl.q C730, C430, S011",
            "vi2f.q C720, C720, 0",
            "vadd.q C320, C320, C400",
            "vadd.q C620, C620, C700",
            "vadd.q C610, C610, C420",
            "vsub.q C530, C530, C600",
            "vadd.q C730, C730, C520",
            "vsub.q C710, C710, C720",
            "vf2id.q C320, C320, 0",
            "vf2id.q C620, C620, 0",
            "vadd.q C530, C530, C610",
            "vadd.q C200, C200, C330",
            "vadd.q C710, C710, C730",
            "vi2f.q C320, C320, 0",
            "vadd.q C300, C300, C630",
            options(nostack),
        );
        psp::vfpu_asm!(
            "vi2f.q C620, C620, 0",
            "vf2id.q C530, C530, 0",
            "vf2id.q C710, C710, 0",
            "vadd.q C200, C200, C320",
            "vadd.q C410, C410, C600",
            "vadd.q C300, C300, C620",
            "vi2f.q C530, C530, 0",
            "vadd.q C510, C510, C720",
            "vi2f.q C710, C710, 0",
            "vmin.q C200, C200, C030",
            "vmin.q C300, C300, C030",
            "vadd.q C410, C410, C530",
            "vadd.q C510, C510, C710",
            "vmax.q C200, C200, C100",
            "vmax.q C300, C300, C100",
            "vmin.q C410, C410, C030",
            "vmin.q C510, C510, C030",
            "vf2iz.q C210, C200, 16",
            "vf2iz.q C310, C300, 16",
            "vmax.q C410, C410, C100",
            "vmax.q C510, C510, C100",
            "vi2s.q C110, C210",
            "vi2s.q C130, C310",
            "vf2iz.q C420, C410, 16",
            "vf2iz.q C520, C510, 16",
            "vi2s.q C112, C420",
            "vi2s.q C132, C520",
            "sv.q C110, 0({x})",
            x = in(reg) x,
            options(nostack),
        );
        psp::vfpu_asm!(
            "sv.q C130, 0({o})",
            o = in(reg) o,
            options(nostack),
        );
    }
}

/// One step of [`section`] with ff, fb, gain nonzero (120 instructions).
///
/// # Safety
///
/// As [`section`], for 8 samples.
#[inline(always)]
unsafe fn step7(x: *mut i16, y: *mut i16, o: *mut i16) {
    // SAFETY: the caller's contract.
    unsafe {
        psp::vfpu_asm!(
            "lv.q C110, 0({x})",
            "lv.q C120, 0({y})",
            "lv.q C130, 0({o})",
            "vs2i.p C630, C110",
            "vs2i.p C720, C112",
            "vs2i.p C700, C120",
            "vs2i.p C730, C122",
            "vs2i.p C620, C130",
            "vi2f.q C200, C630, 16",
            "vi2f.q C410, C720, 16",
            "vi2f.q C230, C700, 31",
            "vi2f.q C500, C730, 31",
            "vi2f.q C220, C700, 16",
            "vscl.q C320, C200, S000",
            "vscl.q C530, C410, S000",
            "vsub.q C230, C020, C230",
            "vscl.q C400, C200, S001",
            "vsub.q C500, C020, C500",
            "vf2id.q C330, C320, 0",
            "vf2id.q C600, C530, 0",
            "vscl.q C610, C410, S001",
            "vadd.q C400, C400, C230",
            "vi2f.q C430, C730, 16",
            "vi2f.q C330, C330, 0",
            "vi2f.q C600, C600, 0",
            "vadd.q C610, C610, C500",
            "vi2f.q C300, C620, 16",
            "vi2f.q C310, C620, 31",
            x = in(reg) x,
            y = in(reg) y,
            o = in(reg) o,
            options(nostack),
        );
        psp::vfpu_asm!(
            "vsub.q C320, C320, C330",
            "vsub.q C530, C530, C600",
            "vadd.q C220, C220, C330",
            "vadd.q C430, C430, C600",
            "vs2i.p C710, C132",
            "vadd.q C320, C320, C400",
            "vadd.q C530, C530, C610",
            "vi2f.q C210, C630, 31",
            "vi2f.q C420, C720, 31",
            "vi2f.q C510, C710, 16",
            "vf2id.q C320, C320, 0",
            "vf2id.q C530, C530, 0",
            "vi2f.q C520, C710, 31",
            "vsub.q C210, C020, C210",
            "vsub.q C310, C020, C310",
            "vi2f.q C320, C320, 0",
            "vi2f.q C530, C530, 0",
            "vsub.q C420, C020, C420",
            "vsub.q C520, C020, C520",
            "vadd.q C220, C220, C320",
            "vadd.q C430, C430, C530",
            "vmin.q C220, C220, C030",
            "vmin.q C430, C430, C030",
            "vmax.q C220, C220, C100",
            "vmax.q C430, C430, C100",
            "vscl.q C320, C220, S002",
            "vscl.q C620, C220, S010",
            "vscl.q C530, C430, S002",
            options(nostack),
        );
        psp::vfpu_asm!(
            "vscl.q C710, C430, S010",
            "vscl.q C400, C220, S003",
            "vf2id.q C330, C320, 0",
            "vf2id.q C630, C620, 0",
            "vscl.q C700, C220, S011",
            "vf2id.q C600, C530, 0",
            "vf2id.q C720, C710, 0",
            "vi2f.q C330, C330, 0",
            "vi2f.q C630, C630, 0",
            "vadd.q C400, C400, C210",
            "vadd.q C700, C700, C310",
            "vscl.q C610, C430, S003",
            "vsub.q C320, C320, C330",
            "vsub.q C620, C620, C630",
            "vi2f.q C600, C600, 0",
            "vscl.q C730, C430, S011",
            "vi2f.q C720, C720, 0",
            "vadd.q C320, C320, C400",
            "vadd.q C620, C620, C700",
            "vadd.q C610, C610, C420",
            "vsub.q C530, C530, C600",
            "vadd.q C730, C730, C520",
            "vsub.q C710, C710, C720",
            "vf2id.q C320, C320, 0",
            "vf2id.q C620, C620, 0",
            "vadd.q C530, C530, C610",
            "vadd.q C200, C200, C330",
            "vadd.q C710, C710, C730",
            options(nostack),
        );
        psp::vfpu_asm!(
            "vi2f.q C320, C320, 0",
            "vadd.q C300, C300, C630",
            "vi2f.q C620, C620, 0",
            "vf2id.q C530, C530, 0",
            "vf2id.q C710, C710, 0",
            "vadd.q C200, C200, C320",
            "vadd.q C410, C410, C600",
            "vadd.q C300, C300, C620",
            "vi2f.q C530, C530, 0",
            "vadd.q C510, C510, C720",
            "vi2f.q C710, C710, 0",
            "vmin.q C200, C200, C030",
            "vmin.q C300, C300, C030",
            "vadd.q C410, C410, C530",
            "vf2iz.q C230, C220, 16",
            "vadd.q C510, C510, C710",
            "vmax.q C200, C200, C100",
            "vmax.q C300, C300, C100",
            "vmin.q C410, C410, C030",
            "vi2s.q C120, C230",
            "vmin.q C510, C510, C030",
            "vf2iz.q C210, C200, 16",
            "vf2iz.q C310, C300, 16",
            "vmax.q C410, C410, C100",
            "vf2iz.q C500, C430, 16",
            "vmax.q C510, C510, C100",
            "vi2s.q C110, C210",
            "vi2s.q C130, C310",
            options(nostack),
        );
        psp::vfpu_asm!(
            "vf2iz.q C420, C410, 16",
            "vi2s.q C122, C500",
            "vf2iz.q C520, C510, 16",
            "vi2s.q C112, C420",
            "sv.q C120, 0({y})",
            "vi2s.q C132, C520",
            "sv.q C110, 0({x})",
            "sv.q C130, 0({o})",
            x = in(reg) x,
            y = in(reg) y,
            o = in(reg) o,
            options(nostack),
        );
    }
}

/// Runs `f` with the calling thread's 128 VFPU registers saved and
/// restored around it: for the selftest, which runs [`section`] on the
/// game thread.
pub fn with_saved<R>(f: impl FnOnce() -> R) -> R {
    #[repr(C, align(16))]
    struct Regs([[f32; 4]; 32]);
    let mut r = Regs([[0.0; 4]; 32]);
    let p = r.0.as_mut_ptr();
    // SAFETY: `r` is a live, 16-byte-aligned 512-byte block.
    unsafe { save(p) };
    let v = f();
    // SAFETY: as above.
    unsafe { restore(p) };
    v
}

/// Stores all 32 VFPU columns to `p` (512 bytes, 16-byte aligned).
unsafe fn save(p: *mut [f32; 4]) {
    // SAFETY: the caller's contract.
    unsafe {
        psp::vfpu_asm!(
            "sv.q C000, 0({p})",
            "sv.q C010, 16({p})",
            "sv.q C020, 32({p})",
            "sv.q C030, 48({p})",
            "sv.q C100, 64({p})",
            "sv.q C110, 80({p})",
            "sv.q C120, 96({p})",
            "sv.q C130, 112({p})",
            "sv.q C200, 128({p})",
            "sv.q C210, 144({p})",
            "sv.q C220, 160({p})",
            "sv.q C230, 176({p})",
            "sv.q C300, 192({p})",
            "sv.q C310, 208({p})",
            "sv.q C320, 224({p})",
            "sv.q C330, 240({p})",
            p = in(reg) p,
            options(nostack),
        );
        psp::vfpu_asm!(
            "sv.q C400, 0({p})",
            "sv.q C410, 16({p})",
            "sv.q C420, 32({p})",
            "sv.q C430, 48({p})",
            "sv.q C500, 64({p})",
            "sv.q C510, 80({p})",
            "sv.q C520, 96({p})",
            "sv.q C530, 112({p})",
            "sv.q C600, 128({p})",
            "sv.q C610, 144({p})",
            "sv.q C620, 160({p})",
            "sv.q C630, 176({p})",
            "sv.q C700, 192({p})",
            "sv.q C710, 208({p})",
            "sv.q C720, 224({p})",
            "sv.q C730, 240({p})",
            p = in(reg) (p.add(16)),
            options(nostack),
        );
    }
}

/// Loads all 32 VFPU columns from `p` (512 bytes, 16-byte aligned).
unsafe fn restore(p: *mut [f32; 4]) {
    // SAFETY: the caller's contract.
    unsafe {
        psp::vfpu_asm!(
            "lv.q C000, 0({p})",
            "lv.q C010, 16({p})",
            "lv.q C020, 32({p})",
            "lv.q C030, 48({p})",
            "lv.q C100, 64({p})",
            "lv.q C110, 80({p})",
            "lv.q C120, 96({p})",
            "lv.q C130, 112({p})",
            "lv.q C200, 128({p})",
            "lv.q C210, 144({p})",
            "lv.q C220, 160({p})",
            "lv.q C230, 176({p})",
            "lv.q C300, 192({p})",
            "lv.q C310, 208({p})",
            "lv.q C320, 224({p})",
            "lv.q C330, 240({p})",
            p = in(reg) p,
            options(nostack),
        );
        psp::vfpu_asm!(
            "lv.q C400, 0({p})",
            "lv.q C410, 16({p})",
            "lv.q C420, 32({p})",
            "lv.q C430, 48({p})",
            "lv.q C500, 64({p})",
            "lv.q C510, 80({p})",
            "lv.q C520, 96({p})",
            "lv.q C530, 112({p})",
            "lv.q C600, 128({p})",
            "lv.q C610, 144({p})",
            "lv.q C620, 160({p})",
            "lv.q C630, 176({p})",
            "lv.q C700, 192({p})",
            "lv.q C710, 208({p})",
            "lv.q C720, 224({p})",
            "lv.q C730, 240({p})",
            p = in(reg) (p.add(16)),
            options(nostack),
        );
    }
}
