---
name: visual-regression
description: Golden screenshots, PPSSPPHeadless, deterministic captures, screenshot comparisons, or regression scenes. Activates when adding, refreshing, or debugging a deterministic rendering-regression scene.
---

# Visual regression

Use this skill (PPSSPPHeadless) for visual/rendering testing — deterministic
screenshots and golden comparisons. For real-hardware validation and crash
capture use [psp-hardware](../psp-hardware/SKILL.md) (PSPLink) instead; for
live original-N64 behavior use [n64-emulator](../n64-emulator/SKILL.md)
(headless Mupen64Plus) instead.

1. Read [docs/visual-regression/README.md](../../docs/visual-regression/README.md)
   — setup, capture commands, rules, scene table (feature → golden →
   evidence), known failing goldens, physical-PSP staging. Load a scene's
   `RE-XXX` record only if you need its history.
2. If a scene's golden changes, explain the semantic delta (differing-pixel
   count, what changed and why) before accepting the refresh — an
   unexplained golden change is a red flag, not a pass.
3. A camera photograph of a physical PSP screen is qualitative evidence only
   and must not be passed to the exact PPSSPP pixel comparator.
4. PPSSPP software rendering is the deterministic golden source; PPSSPP
   hardware-backend and physical-PSP captures are separate, non-deterministic
   evidence tiers — do not conflate a PPSSPP-software pass with physical
   confirmation.
