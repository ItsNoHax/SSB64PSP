/* PCM-dump audio plugin for Mupen64Plus (N64 audio reference captures).
 *
 * Replaces mupen64plus-audio-sdl when n64_driver.py is given --audio-dump.
 * Every buffer the game hands the AI (the core reports it through
 * AiLenChanged with AI_DRAM_ADDR_REG/AI_LEN_REG pointing at it) is appended
 * verbatim to a raw file as s16le stereo, L then R. Nothing is resampled,
 * scaled or paced in real time, so the dump is the exact PCM the game
 * produced, locked to the driver's frame stepping. It is still HLE audio
 * (mupen64plus-rsp-hle runs the audio microcode), not hardware truth.
 *
 * Byte order: RDRAM is stored as host-endian u32 words, so on a
 * little-endian host each word reads as (L << 16) | R. The left sample is
 * the high half of the word, not the first two bytes in memory.
 *
 * Sidecar (<path>.txt), one record per line:
 *   rate <hz> dacrate <AI_DACRATE> system <type> from_buffer <n>
 *   buf <n> frame <video frame> sample_offset <stereo frames before it>
 *       len <bytes> dram <AI_DRAM_ADDR>
 * `frame` is g_video_frame, which the driver stores from its frame
 * callback; it is the last video frame completed when the buffer arrived.
 * The core hands each game buffer over in two pieces (contiguous `dram`
 * ranges); joined, they are the 552- or 368-sample frames syAudio renders.
 * Use sample counts, not callback frames, as the clock: a callback frame is
 * not exactly one VI.
 *
 * The output path is set with n64_audio_dump_set_output() before
 * PluginStartup. Headers: see n64_input.c.
 */

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define M64P_PLUGIN_PROTOTYPES 1
#include "m64p_types.h"
#include "m64p_plugin.h"

EXPORT volatile uint32_t g_video_frame = 0;

static AUDIO_INFO l_info;
static int l_have_info = 0;
static char l_path[4096];
static FILE *l_raw = NULL;
static FILE *l_meta = NULL;
static uint64_t l_buffers = 0;
static uint64_t l_stereo_frames = 0;
static int16_t l_scratch[2 * 65536];

EXPORT int CALL n64_audio_dump_set_output(const char *path)
{
    if (path == NULL || strlen(path) + 5 > sizeof l_path)
        return 0;
    strcpy(l_path, path);
    return 1;
}

static void close_files(void)
{
    if (l_raw != NULL) fclose(l_raw);
    if (l_meta != NULL) fclose(l_meta);
    l_raw = NULL;
    l_meta = NULL;
}

static int open_files(void)
{
    char meta_path[sizeof l_path + 4];
    if (l_raw != NULL) return 1;
    if (l_path[0] == '\0') return 0;
    snprintf(meta_path, sizeof meta_path, "%s.txt", l_path);
    l_raw = fopen(l_path, "wb");
    l_meta = fopen(meta_path, "w");
    if (l_raw == NULL || l_meta == NULL) {
        close_files();
        return 0;
    }
    fprintf(l_meta, "# n64_audio_dump v1: raw is s16le stereo L,R\n");
    return 1;
}

EXPORT m64p_error CALL PluginStartup(m64p_dynlib_handle CoreLibHandle, void *Context,
                                      void (*DebugCallback)(void *, int, const char *))
{
    (void) CoreLibHandle;
    (void) Context;
    (void) DebugCallback;
    return M64ERR_SUCCESS;
}

EXPORT m64p_error CALL PluginShutdown(void)
{
    close_files();
    return M64ERR_SUCCESS;
}

EXPORT m64p_error CALL PluginGetVersion(m64p_plugin_type *PluginType, int *PluginVersion,
                                         int *APIVersion, const char **PluginNamePtr,
                                         int *Capabilities)
{
    if (PluginType != NULL) *PluginType = M64PLUGIN_AUDIO;
    if (PluginVersion != NULL) *PluginVersion = 0x000100;
    if (APIVersion != NULL) *APIVersion = 0x020000;
    if (PluginNamePtr != NULL) *PluginNamePtr = "SSB64PSP PCM dump";
    if (Capabilities != NULL) *Capabilities = 0;
    return M64ERR_SUCCESS;
}

EXPORT int CALL InitiateAudio(AUDIO_INFO Audio_Info)
{
    l_info = Audio_Info;
    l_have_info = 1;
    return 1;
}

EXPORT void CALL AiDacrateChanged(int SystemType)
{
    double clock;
    unsigned int dacrate;
    if (!l_have_info || !open_files()) return;
    switch (SystemType) {
    case SYSTEM_PAL: clock = 49656530.0; break;
    case SYSTEM_MPAL: clock = 48628316.0; break;
    default: clock = 48681812.0; break;
    }
    dacrate = *l_info.AI_DACRATE_REG;
    fprintf(l_meta, "rate %.3f dacrate %u system %d from_buffer %llu\n",
            clock / (double) (dacrate + 1), dacrate, SystemType,
            (unsigned long long) l_buffers);
    fflush(l_meta);
}

EXPORT void CALL AiLenChanged(void)
{
    uint32_t addr, len, i, words;
    const uint32_t *src;
    if (!l_have_info || !open_files()) return;
    addr = *l_info.AI_DRAM_ADDR_REG & 0xFFFFFF;
    len = *l_info.AI_LEN_REG & 0x3FFF8;
    if (len > sizeof l_scratch) len = sizeof l_scratch;
    words = len / 4;
    src = (const uint32_t *) (l_info.RDRAM + addr);
    for (i = 0; i < words; i++) {
        uint32_t w = src[i];
        l_scratch[2 * i] = (int16_t) (w >> 16);
        l_scratch[2 * i + 1] = (int16_t) (w & 0xFFFF);
    }
    fwrite(l_scratch, 4, words, l_raw);
    fprintf(l_meta, "buf %llu frame %u sample_offset %llu len %u dram 0x%06x\n",
            (unsigned long long) l_buffers, g_video_frame,
            (unsigned long long) l_stereo_frames, len, addr);
    fflush(l_raw);
    fflush(l_meta);
    l_buffers++;
    l_stereo_frames += words;
}

EXPORT void CALL ProcessAList(void) { }
EXPORT int CALL RomOpen(void) { return 1; }
EXPORT void CALL RomClosed(void) { close_files(); }
EXPORT void CALL SetSpeedFactor(int percent) { (void) percent; }
EXPORT void CALL VolumeUp(void) { }
EXPORT void CALL VolumeDown(void) { }
EXPORT int CALL VolumeGetLevel(void) { return 100; }
EXPORT void CALL VolumeSetLevel(int level) { (void) level; }
EXPORT void CALL VolumeMute(void) { }
EXPORT const char * CALL VolumeGetString(void) { return "100%"; }
