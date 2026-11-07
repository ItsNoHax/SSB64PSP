"""Headless VidExt override for the RMG core (n64_driver.py RMG profile).

ParaLLEl-RDP (mupen64plus-video-parallel) renders through Vulkan and asks the
front end for a surface; this hands it a VK_EXT_headless_surface one, so no
window opens. Passed to CoreOverrideVidExt before ROM_OPEN (RE-564).
Struct layout: m64p_video_extension_functions (m64p_types.h, 17 functions).
"""
import ctypes
import sys

E = ctypes.c_int
F_init = ctypes.CFUNCTYPE(E)
F_quit = ctypes.CFUNCTYPE(E)
F_listmodes = ctypes.CFUNCTYPE(E, ctypes.c_void_p, ctypes.c_void_p)
F_listrates = ctypes.CFUNCTYPE(E, ctypes.c_uint64, ctypes.c_void_p, ctypes.c_void_p)
F_setmode = ctypes.CFUNCTYPE(E, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_int)
F_setmoderate = ctypes.CFUNCTYPE(E, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_int)
F_getproc = ctypes.CFUNCTYPE(ctypes.c_void_p, ctypes.c_char_p)
F_setattr = ctypes.CFUNCTYPE(E, ctypes.c_int, ctypes.c_int)
F_getattr = ctypes.CFUNCTYPE(E, ctypes.c_int, ctypes.c_void_p)
F_void = ctypes.CFUNCTYPE(E)
F_caption = ctypes.CFUNCTYPE(E, ctypes.c_char_p)
F_resize = ctypes.CFUNCTYPE(E, ctypes.c_int, ctypes.c_int)
F_fb = ctypes.CFUNCTYPE(ctypes.c_uint32)
F_initmode = ctypes.CFUNCTYPE(E, ctypes.c_int)
F_vksurf = ctypes.CFUNCTYPE(E, ctypes.POINTER(ctypes.c_uint64), ctypes.c_void_p)
F_vkext = ctypes.CFUNCTYPE(E, ctypes.POINTER(ctypes.POINTER(ctypes.c_char_p)), ctypes.POINTER(ctypes.c_uint32))


class VidExt(ctypes.Structure):
    _fields_ = [("Functions", ctypes.c_uint), ("init", F_init), ("quit", F_quit), ("listmodes", F_listmodes),
                ("listrates", F_listrates), ("setmode", F_setmode), ("setmoderate", F_setmoderate),
                ("getproc", F_getproc), ("setattr", F_setattr), ("getattr", F_getattr), ("swap", F_void),
                ("caption", F_caption), ("togglefs", F_void), ("resize", F_resize), ("fb", F_fb),
                ("initmode", F_initmode), ("vksurf", F_vksurf), ("vkext", F_vkext)]


vk = ctypes.CDLL("libvulkan.so.1")
vk.vkGetInstanceProcAddr.restype = ctypes.c_void_p
vk.vkGetInstanceProcAddr.argtypes = [ctypes.c_void_p, ctypes.c_char_p]
EXTS = (ctypes.c_char_p * 2)(b"VK_KHR_surface", b"VK_EXT_headless_surface")
VK_STRUCTURE_TYPE_HEADLESS_SURFACE_CREATE_INFO_EXT = 1000256000


def log(m):
    sys.stderr.write("[vidext] %s\n" % m)
    sys.stderr.flush()


def vksurf(out, inst):
    p = vk.vkGetInstanceProcAddr(inst, b"vkCreateHeadlessSurfaceEXT")
    log("vkCreateHeadlessSurfaceEXT=%s" % p)
    if not p:
        return 1

    class CI(ctypes.Structure):
        _fields_ = [("sType", ctypes.c_int), ("pNext", ctypes.c_void_p), ("flags", ctypes.c_uint32)]
    ci = CI(VK_STRUCTURE_TYPE_HEADLESS_SURFACE_CREATE_INFO_EXT, None, 0)
    fn = ctypes.CFUNCTYPE(ctypes.c_int, ctypes.c_void_p, ctypes.c_void_p, ctypes.c_void_p,
                          ctypes.POINTER(ctypes.c_uint64))(p)
    r = fn(inst, ctypes.byref(ci), None, out)
    log("create surface -> %d %x" % (r, out[0]))
    return 0 if r == 0 else 1


def vkext(out, count):
    out[0] = ctypes.cast(EXTS, ctypes.POINTER(ctypes.c_char_p))
    count[0] = 2
    return 0


def initmode(m):
    log("InitWithRenderMode %d" % m)
    return 0


def make():
    """The struct to pass by reference to CoreOverrideVidExt; keep it alive while the core runs."""
    return VidExt(17, F_init(lambda: 0), F_quit(lambda: 0), F_listmodes(lambda a, b: 1),
                  F_listrates(lambda a, b, c: 1), F_setmode(lambda *a: (log("setmode %s" % (a,)), 0)[1]),
                  F_setmoderate(lambda *a: 0), F_getproc(lambda n: None), F_setattr(lambda a, b: 0),
                  F_getattr(lambda a, b: 1), F_void(lambda: 0), F_caption(lambda c: 0), F_void(lambda: 0),
                  F_resize(lambda a, b: 0), F_fb(lambda: 0), F_initmode(initmode), F_vksurf(vksurf),
                  F_vkext(vkext))
