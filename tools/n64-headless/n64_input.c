/* Scripted-input plugin for Mupen64Plus (RE-151/RE-216/RE-274).
 *
 * Minimal M64+ controller plugin. Button state for up to 4 controllers is
 * exported as a plain array a driving process can poke directly with ctypes
 * before each M64CMD_ADVANCE_FRAME call -- there is no timing logic here,
 * the frame-stepping driver (n64_driver.py) owns the schedule.
 *
 * Vendored headers: include/m64p_types.h and include/m64p_plugin.h are
 * mupen64plus-core's public src/api/ interface headers (GPLv2), pulled in
 * only for the plugin ABI declarations -- no core/plugin implementation
 * code is copied here.
 */

#include <string.h>
#include <stdint.h>

#define M64P_PLUGIN_PROTOTYPES 1
#include "m64p_types.h"
#include "m64p_plugin.h"

/* Controllers this plugin claims. Only P1/P2 are used by the driver. */
#define NUM_CONTROLLERS 4

EXPORT volatile uint32_t g_buttons[NUM_CONTROLLERS] = {0, 0, 0, 0};
EXPORT volatile int g_present[NUM_CONTROLLERS] = {1, 1, 0, 0};

static int l_PluginInit = 0;

EXPORT m64p_error CALL PluginStartup(m64p_dynlib_handle CoreLibHandle, void *Context,
                                      void (*DebugCallback)(void *, int, const char *))
{
    (void) CoreLibHandle;
    (void) Context;
    (void) DebugCallback;
    if (l_PluginInit)
        return M64ERR_ALREADY_INIT;
    l_PluginInit = 1;
    return M64ERR_SUCCESS;
}

EXPORT m64p_error CALL PluginShutdown(void)
{
    l_PluginInit = 0;
    return M64ERR_SUCCESS;
}

EXPORT m64p_error CALL PluginGetVersion(m64p_plugin_type *PluginType, int *PluginVersion,
                                         int *APIVersion, const char **PluginNamePtr,
                                         int *Capabilities)
{
    if (PluginType != NULL) *PluginType = M64PLUGIN_INPUT;
    if (PluginVersion != NULL) *PluginVersion = 0x000100;
    if (APIVersion != NULL) *APIVersion = 0x020100;
    if (PluginNamePtr != NULL) *PluginNamePtr = "SSB64PSP scripted input";
    if (Capabilities != NULL) *Capabilities = 0;
    return M64ERR_SUCCESS;
}

EXPORT void CALL InitiateControllers(CONTROL_INFO ControlInfo)
{
    int i;
    for (i = 0; i < NUM_CONTROLLERS; i++) {
        ControlInfo.Controls[i].Present = g_present[i];
        ControlInfo.Controls[i].RawData = 0;
        ControlInfo.Controls[i].Plugin = PLUGIN_NONE;
        ControlInfo.Controls[i].Type = CONT_TYPE_STANDARD;
    }
}

EXPORT void CALL GetKeys(int Control, BUTTONS *Keys)
{
    if (Control < 0 || Control >= NUM_CONTROLLERS || Keys == NULL)
        return;
    Keys->Value = g_buttons[Control];
}

EXPORT void CALL ControllerCommand(int Control, unsigned char *Command)
{
    (void) Control;
    (void) Command;
}

EXPORT void CALL ReadController(int Control, unsigned char *Command)
{
    (void) Control;
    (void) Command;
}

EXPORT int CALL RomOpen(void) { return 1; }
EXPORT void CALL RomClosed(void) { }

EXPORT void CALL SDL_KeyDown(int keymod, int keysym) { (void) keymod; (void) keysym; }
EXPORT void CALL SDL_KeyUp(int keymod, int keysym) { (void) keymod; (void) keysym; }
