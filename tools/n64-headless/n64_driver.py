#!/usr/bin/env python3
"""Headless, scripted-input Mupen64Plus driver for original-ROM reference captures.

Drives libmupen64plus.so.2 directly through its public Core API (CoreStartup /
CoreDoCommand / CoreAttachPlugin) -- not the M64Py Qt GUI. Must run inside the
M64Py flatpak sandbox:

    flatpak run --command=python3 net.sourceforge.m64py.M64Py \
        tools/n64-headless/n64_driver.py --rom "rom/Super Smash Bros. (USA).z64" \
        --input-so /path/to/n64_input.so --frames 120

so it picks up the bundled core/plugin .so files and their runtime
dependencies (SDL2, Qt/KDE runtime libs). Use tools/run-n64-headless.sh
instead of invoking this directly -- it builds n64_input.so and wraps the
flatpak invocation.

Input is supplied by a small custom plugin (n64_input.so, built from
n64_input.c in this directory) that exposes a 4-element uint32 g_buttons
array the driver pokes directly with ctypes before each M64CMD_ADVANCE_FRAME
call. This gives frame-exact scripted input without needing a real
controller or GUI automation.

Origin: built for RE-151/RE-216 (docs/evidence/re/) as a one-off, then
promoted to a shared tool since the pattern (navigate menus, settle a scene,
screenshot, read RDRAM) recurs across reverse-engineering tasks.
"""
import argparse
import ctypes
import os
import sys
import threading
import time

APP_LIB = "/app/lib"
PLUGIN_DIR = os.path.join(APP_LIB, "mupen64plus")
CORE_PATH = os.path.join(APP_LIB, "libmupen64plus.so.2")

# --- m64p_types.h enums we need -------------------------------------------------
M64ERR_SUCCESS = 0

M64PLUGIN_RSP = 1
M64PLUGIN_GFX = 2
M64PLUGIN_AUDIO = 3
M64PLUGIN_INPUT = 4

M64EMU_STOPPED = 1
M64EMU_RUNNING = 2
M64EMU_PAUSED = 3

M64CORE_EMU_STATE = 1
M64CORE_SPEED_LIMITER = 5
M64CORE_SCREENSHOT_CAPTURED = 12

M64CMD_ROM_OPEN = 1
M64CMD_ROM_CLOSE = 2
M64CMD_EXECUTE = 5
M64CMD_STOP = 6
M64CMD_PAUSE = 7
M64CMD_RESUME = 8
M64CMD_CORE_STATE_QUERY = 9
M64CMD_SET_FRAME_CALLBACK = 15
M64CMD_TAKE_NEXT_SCREENSHOT = 16
M64CMD_CORE_STATE_SET = 17
M64CMD_READ_SCREEN = 18
M64CMD_ADVANCE_FRAME = 20

M64P_DBG_PTR_RDRAM = 1

# BUTTONS bit layout (m64p_plugin.h)
BTN_R_DPAD = 1 << 0
BTN_L_DPAD = 1 << 1
BTN_D_DPAD = 1 << 2
BTN_U_DPAD = 1 << 3
BTN_START = 1 << 4
BTN_Z = 1 << 5
BTN_B = 1 << 6
BTN_A = 1 << 7
BTN_R_C = 1 << 8
BTN_L_C = 1 << 9
BTN_D_C = 1 << 10
BTN_U_C = 1 << 11
BTN_R = 1 << 12
BTN_L = 1 << 13


def encode_buttons(a=False, b=False, start=False, z=False, l=False, r=False,
                    dpad_u=False, dpad_d=False, dpad_l=False, dpad_r=False,
                    c_u=False, c_d=False, c_l=False, c_r=False, x=0, y=0):
    v = 0
    if a: v |= BTN_A
    if b: v |= BTN_B
    if start: v |= BTN_START
    if z: v |= BTN_Z
    if l: v |= BTN_L
    if r: v |= BTN_R
    if dpad_u: v |= BTN_U_DPAD
    if dpad_d: v |= BTN_D_DPAD
    if dpad_l: v |= BTN_L_DPAD
    if dpad_r: v |= BTN_R_DPAD
    if c_u: v |= BTN_U_C
    if c_d: v |= BTN_D_C
    if c_l: v |= BTN_L_C
    if c_r: v |= BTN_R_C
    v |= (x & 0xFF) << 16
    v |= (y & 0xFF) << 24
    return v

CORE_API_VERSION = 0x020001  # major.minor.patch packed; core checks front-end compatibility loosely

DebugCallback_t = ctypes.CFUNCTYPE(None, ctypes.c_void_p, ctypes.c_int, ctypes.c_char_p)
StateCallback_t = ctypes.CFUNCTYPE(None, ctypes.c_void_p, ctypes.c_int, ctypes.c_int)
FrameCallback_t = ctypes.CFUNCTYPE(None, ctypes.c_uint)


def _debug_cb(context, level, message):
    text = message.decode("utf-8", "replace") if message else ""
    sys.stderr.write(f"[core:{level}] {text}\n")


def _state_cb(context, param, value):
    if param == M64CORE_EMU_STATE:
        sys.stderr.write(f"[state] emu_state={value}\n")


class Mupen64PlusHarness:
    def __init__(self, rom_path, input_so_path, config_dir, data_dir):
        self.rom_path = rom_path
        self.input_so_path = input_so_path
        self.config_dir = config_dir
        self.data_dir = data_dir

        self.core = None
        self.gfx = None
        self.audio = None
        self.rsp = None
        self.inp = None

        self._debug_cb_ref = DebugCallback_t(_debug_cb)
        self._state_cb_ref = StateCallback_t(_state_cb)
        self._frame_cb_ref = FrameCallback_t(self._on_frame)

        self._frame_event = threading.Event()
        self.current_frame = 0
        self._exec_thread = None
        self.buttons = None  # (c_uint32 * 4) view into the input plugin

    # -- plugin lifecycle ---------------------------------------------------
    def _load(self, path):
        return ctypes.CDLL(path, mode=ctypes.RTLD_GLOBAL)

    def start(self):
        self.core = self._load(CORE_PATH)

        # ctypes truncates plain Python ints to 32-bit C int without explicit
        # argtypes; every dynlib handle/pointer here is 64-bit, so this is not
        # optional -- omitting it corrupts handles and segfaults inside the
        # plugin (observed: PluginStartup(gfx) SIGSEGV with default argtypes).
        self.core.CoreStartup.restype = ctypes.c_int
        self.core.CoreStartup.argtypes = [
            ctypes.c_int, ctypes.c_char_p, ctypes.c_char_p, ctypes.c_void_p,
            DebugCallback_t, ctypes.c_void_p, StateCallback_t,
        ]
        self.core.CoreDoCommand.restype = ctypes.c_int
        self.core.CoreDoCommand.argtypes = [ctypes.c_int, ctypes.c_int, ctypes.c_void_p]
        self.core.CoreAttachPlugin.restype = ctypes.c_int
        self.core.CoreAttachPlugin.argtypes = [ctypes.c_int, ctypes.c_void_p]
        self.core.CoreShutdown.restype = ctypes.c_int

        rval = self.core.CoreStartup(
            CORE_API_VERSION,
            self.config_dir.encode(),
            self.data_dir.encode(),
            b"Core",
            self._debug_cb_ref,
            None,
            self._state_cb_ref,
        )
        if rval != M64ERR_SUCCESS:
            raise RuntimeError(f"CoreStartup failed: {rval}")

        with open(self.rom_path, "rb") as f:
            rom_bytes = f.read()
        rom_buf = ctypes.create_string_buffer(rom_bytes, len(rom_bytes))
        rval = self.core.CoreDoCommand(M64CMD_ROM_OPEN, len(rom_bytes), rom_buf)
        if rval != M64ERR_SUCCESS:
            raise RuntimeError(f"ROM_OPEN failed: {rval}")

        self.gfx = self._load(os.path.join(PLUGIN_DIR, "mupen64plus-video-rice.so"))
        self.audio = self._load(os.path.join(PLUGIN_DIR, "mupen64plus-audio-sdl.so"))
        self.rsp = self._load(os.path.join(PLUGIN_DIR, "mupen64plus-rsp-hle.so"))
        self.inp = self._load(self.input_so_path)

        for name, lib in (("gfx", self.gfx), ("audio", self.audio), ("input", self.inp), ("rsp", self.rsp)):
            sys.stderr.write(f"[driver] PluginStartup({name})\n"); sys.stderr.flush()
            lib.PluginStartup.restype = ctypes.c_int
            lib.PluginStartup.argtypes = [ctypes.c_void_p, ctypes.c_void_p, DebugCallback_t]
            lib.PluginShutdown.restype = ctypes.c_int
            rval = lib.PluginStartup(self.core._handle, name.encode(), self._debug_cb_ref)
            sys.stderr.write(f"[driver] PluginStartup({name}) -> {rval}\n"); sys.stderr.flush()
            if rval != M64ERR_SUCCESS:
                raise RuntimeError(f"PluginStartup({name}) failed: {rval}")

        for ptype, lib, name in (
            (M64PLUGIN_GFX, self.gfx, "gfx"),
            (M64PLUGIN_AUDIO, self.audio, "audio"),
            (M64PLUGIN_INPUT, self.inp, "input"),
            (M64PLUGIN_RSP, self.rsp, "rsp"),
        ):
            sys.stderr.write(f"[driver] CoreAttachPlugin({name})\n"); sys.stderr.flush()
            rval = self.core.CoreAttachPlugin(ptype, lib._handle)
            sys.stderr.write(f"[driver] CoreAttachPlugin({name}) -> {rval}\n"); sys.stderr.flush()
            if rval != M64ERR_SUCCESS:
                raise RuntimeError(f"CoreAttachPlugin({name}) failed: {rval}")

        self.buttons = (ctypes.c_uint32 * 4).in_dll(self.inp, "g_buttons")

        rval = self.core.CoreDoCommand(
            M64CMD_SET_FRAME_CALLBACK, 0, ctypes.cast(self._frame_cb_ref, ctypes.c_void_p)
        )
        if rval != M64ERR_SUCCESS:
            raise RuntimeError(f"SET_FRAME_CALLBACK failed: {rval}")

        sys.stderr.write("[driver] starting execute thread\n"); sys.stderr.flush()
        self._exec_thread = threading.Thread(target=self._run_execute, daemon=True)
        self._exec_thread.start()
        self._wait_for_state(M64EMU_RUNNING, timeout=15.0)
        sys.stderr.write("[driver] running; pausing\n"); sys.stderr.flush()
        # Pause immediately; from here on we drive frame-by-frame.
        self.core.CoreDoCommand(M64CMD_PAUSE, 0, None)
        self._wait_for_state(M64EMU_PAUSED, timeout=15.0)
        sys.stderr.write("[driver] paused\n"); sys.stderr.flush()

    def _run_execute(self):
        self.core.CoreDoCommand(M64CMD_EXECUTE, 0, None)

    def _query_state(self):
        out = ctypes.c_int(0)
        self.core.CoreDoCommand(M64CMD_CORE_STATE_QUERY, M64CORE_EMU_STATE, ctypes.byref(out))
        return out.value

    def _wait_for_state(self, target, timeout=10.0):
        deadline = time.time() + timeout
        while time.time() < deadline:
            if self._query_state() == target:
                return
            time.sleep(0.02)
        raise TimeoutError(f"timed out waiting for emu_state={target}, last={self._query_state()}")

    def _on_frame(self, frame_index):
        self.current_frame = frame_index
        self._frame_event.set()

    # -- stepping -------------------------------------------------------------
    def set_input(self, controller, value):
        self.buttons[controller] = value & 0xFFFFFFFF

    def advance_frame(self, timeout=5.0):
        self._frame_event.clear()
        rval = self.core.CoreDoCommand(M64CMD_ADVANCE_FRAME, 0, None)
        if rval != M64ERR_SUCCESS:
            raise RuntimeError(f"ADVANCE_FRAME failed: {rval}")
        if not self._frame_event.wait(timeout=timeout):
            raise TimeoutError(f"ADVANCE_FRAME did not signal at frame {self.current_frame}")

    def run_script(self, frames, on_frame=None):
        """frames: iterable of (p1_buttons, p2_buttons) per frame, in order."""
        for i, (p1, p2) in enumerate(frames):
            self.set_input(0, p1)
            self.set_input(1, p2)
            self.advance_frame()
            if on_frame is not None:
                on_frame(self.current_frame)

    def screenshot(self):
        rval = self.core.CoreDoCommand(M64CMD_TAKE_NEXT_SCREENSHOT, 0, None)
        if rval != M64ERR_SUCCESS:
            raise RuntimeError(f"TAKE_NEXT_SCREENSHOT failed: {rval}")
        self.advance_frame()

    def latest_screenshot(self):
        d = os.path.join(self.data_dir, "screenshot")
        entries = [os.path.join(d, f) for f in os.listdir(d) if f.endswith(".png")]
        if not entries:
            return None
        return max(entries, key=os.path.getmtime)

    def run_steps(self, steps, screenshot_every=None, on_frame=None):
        """steps: list of (hold_frames, p1_buttons, p2_buttons)."""
        since_shot = 0
        for hold, p1, p2 in steps:
            for _ in range(hold):
                self.set_input(0, p1)
                self.set_input(1, p2)
                self.advance_frame()
                since_shot += 1
                if on_frame is not None:
                    on_frame(self.current_frame)
                if screenshot_every and since_shot >= screenshot_every:
                    since_shot = 0
                    self.screenshot()
                    sys.stderr.write(f"[driver] screenshot at frame {self.current_frame}: {self.latest_screenshot()}\n")
                    sys.stderr.flush()

    def _rdram_base(self):
        if not hasattr(self, "_rdram_ptr") or self._rdram_ptr is None:
            self.core.DebugMemGetPointer.restype = ctypes.c_void_p
            self.core.DebugMemGetPointer.argtypes = [ctypes.c_int]
            ptr = self.core.DebugMemGetPointer(M64P_DBG_PTR_RDRAM)
            if not ptr:
                raise RuntimeError("DebugMemGetPointer(RDRAM) returned NULL")
            self._rdram_ptr = ptr
        return self._rdram_ptr

    def read_rdram(self, vaddr, length):
        """vaddr: a 0x80xxxxxx/0xA0xxxxxx N64 virtual address. Returns big-endian bytes."""
        base = self._rdram_base()
        offset = vaddr & 0x7FFFFF  # 8 MB RDRAM
        buf = (ctypes.c_ubyte * length).from_address(base + offset)
        return bytes(buf)

    def write_rdram(self, vaddr, data):
        base = self._rdram_base()
        offset = vaddr & 0x7FFFFF
        buf = (ctypes.c_ubyte * len(data)).from_address(base + offset)
        buf[:] = data

    def read_u32(self, vaddr):
        return int.from_bytes(self.read_rdram(vaddr, 4), "big")

    def read_f32(self, vaddr):
        import struct
        return struct.unpack(">f", self.read_rdram(vaddr, 4))[0]

    def write_u8(self, vaddr, value):
        self.write_rdram(vaddr, bytes([value & 0xFF]))

    def stop(self):
        if self.core is None:
            return
        try:
            self.core.CoreDoCommand(M64CMD_STOP, 0, None)
        except Exception:
            pass
        if self._exec_thread is not None:
            self._exec_thread.join(timeout=10.0)
        self.core.CoreDoCommand(M64CMD_ROM_CLOSE, 0, None)
        for lib in (self.gfx, self.audio, self.inp, self.rsp):
            try:
                lib.PluginShutdown()
            except Exception:
                pass
        self.core.CoreShutdown()


def parse_route(spec):
    """spec: ';'-separated steps 'hold:p1button+button;hold:...'. Empty buttons = idle."""
    steps = []
    for chunk in spec.split(";"):
        chunk = chunk.strip()
        if not chunk:
            continue
        hold_str, _, btn_str = chunk.partition(":")
        hold = int(hold_str)
        kwargs = {}
        if btn_str:
            for name in btn_str.split("+"):
                name = name.strip()
                if not name:
                    continue
                if "=" in name:
                    key, _, val = name.partition("=")
                    kwargs[key] = int(val)
                else:
                    kwargs[name] = True
        p1 = encode_buttons(**kwargs)
        steps.append((hold, p1, 0))
    return steps


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--rom", required=True)
    ap.add_argument("--input-so", required=True)
    ap.add_argument("--config-dir", default=os.path.expanduser("~/.var/app/net.sourceforge.m64py.M64Py/config/mupen64plus"))
    ap.add_argument("--data-dir", default=os.path.expanduser("~/.var/app/net.sourceforge.m64py.M64Py/data/mupen64plus"))
    ap.add_argument("--frames", type=int, default=120, help="idle frames to smoke-test frame stepping")
    ap.add_argument("--route", default=None,
                     help="';'-separated steps 'hold:btn+btn;hold:btn' (btn in a,b,start,z,l,r,dpad_u,dpad_d,dpad_l,dpad_r,c_u,c_d,c_l,c_r), applied to P1 only")
    ap.add_argument("--screenshot-every", type=int, default=0)
    ap.add_argument("--final-screenshot", action="store_true")
    args = ap.parse_args()

    h = Mupen64PlusHarness(args.rom, args.input_so, args.config_dir, args.data_dir)
    h.start()
    print("started; state=", h._query_state(), file=sys.stderr)
    t0 = time.time()
    if args.route:
        steps = parse_route(args.route)
        h.run_steps(steps, screenshot_every=args.screenshot_every or None)
    else:
        h.run_script([(0, 0)] * args.frames)
    dt = time.time() - t0
    print(f"advanced to frame {h.current_frame} in {dt:.2f}s", file=sys.stderr)
    if args.final_screenshot:
        h.screenshot()
        print(f"final screenshot: {h.latest_screenshot()}", file=sys.stderr)
    h.stop()
    print("stopped cleanly", file=sys.stderr)


if __name__ == "__main__":
    main()
