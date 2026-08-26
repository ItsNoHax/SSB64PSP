# Super Smash Bros. 64 → PSP — Rust Native Port Plan

## Objective

Create a native Rust port of the original Nintendo 64 Super Smash Bros. to the Sony PSP.

The target is a genuine native PSP application, NOT an N64 emulator.

The project should:

- Be written primarily in Rust.
- Target the PSP-1000/2000/3000 hardware.
- Build using `rust-psp` / `cargo-psp`.
- Use the original Super Smash Bros. N64 decompilation as the primary source for game logic.
- Extract copyrighted game assets at build time from a legally owned user-supplied ROM.
- Never commit Nintendo ROM data, extracted copyrighted assets, or copyrighted game code/data that cannot legally be redistributed.
- Preserve original gameplay behavior as closely as practical.
- Replace N64-specific rendering, audio, input, memory, timing and OS functionality with PSP-native implementations.
- Exploit the PSP's VFPU and GPU where useful.
- Eventually run the complete original game on real PSP hardware.

Primary references:

1. https://github.com/VetriTheRetri/ssb-decomp-re
2. https://github.com/TheMrIron2/sf64-psp
3. https://github.com/TheMrIron2/n64psp
4. https://github.com/overdrivenpotato/rust-psp

Use these as architectural/reference projects, NOT as code to blindly copy.

---

# 1. First: Repository Reconnaissance

Before writing significant code, inspect all four reference repositories.

## 1.1 Study ssb-decomp-re

Determine:

- ROM regions supported.
- Exact supported ROM revision.
- Build process.
- Splat configuration.
- Segment layout.
- Asset extraction process.
- Generated source.
- Existing function names.
- Existing structure/type definitions.
- N64 OS dependencies.
- Rendering architecture.
- Audio architecture.
- Input architecture.
- Memory allocation.
- Thread/task architecture.
- DMA usage.
- RSP/RDP dependencies.
- Compression formats.
- Texture formats.
- Display-list formats.
- Audio sequence/sample formats.

Generate:

`docs/ssb-architecture.md`

This document should describe the original game's architecture in enough detail that another developer can understand where each N64 subsystem maps to PSP.

Do NOT start porting until this investigation is complete.

---

# 2. Establish the Exact ROM Baseline

The canonical US version of the SSB decompilation is:

- Internal game code: `NALE`
- SHA-1:
  `e2929e10fccc0aa84e5776227e798abc07cedabf`
- MD5:
  `f7c52568a31aadf26e14dc2b6416b2ed`

The user owns the game and has a `.z64` dump.

Implement a build-time ROM validation/extraction pipeline.

Expected workflow:

`baserom.us.z64`
        |
        v
ROM validation
        |
        v
asset extraction
        |
        +--> textures
        +--> models
        +--> animations
        +--> audio
        +--> stage data
        +--> game data
        |
        v
PSP build

The ROM itself must never be copied into the repository.

---

# 3. Decide the Port Architecture

Do NOT make the PSP project a giant translation of the original C code.

Use a layered architecture:

```text
                 Smash 64 Game Logic
                         |
             +-----------+-----------+
             |                       |
       Portable Rust              PSP Backend
             |                       |
      game systems              PSP hardware
      gameplay                  sceGu
      physics                   VFPU
      menus                    audio
      entities                 input
      animation                timing
      stage logic              memory

The goal is:

Layer A — Game

Contains:

fighters
attacks
collision
physics
damage
stocks
knockback
hitlag
hitstun
grabbing
throwing
shields
stage logic
AI
menus
game modes
match state
camera
animation state
Layer B — Engine abstraction

Create Rust traits/interfaces for:

Renderer
Audio
Input
Timer
Memory
File/resource loading
DMA-like transfers
Controller rumble
Video/display
Task scheduling
Layer C — PSP backend

Implement those abstractions using:

rust-psp
PSP kernel/user APIs
sceGu
PSP framebuffer
PSP GE
VFPU
Media Engine where worthwhile
4. Rust Strategy

The project should be Rust-first.

Do not convert every original C function mechanically into Rust.

Instead classify original code into:

A. Gameplay logic

Port to idiomatic Rust.

B. Mathematical/vector code

Rewrite using Rust types and aggressively investigate VFPU acceleration.

C. N64 rendering

Replace with a PSP renderer.

D. N64 audio

Replace the N64 audio pipeline with a PSP-compatible audio backend.

E. N64 OS calls

Replace with PSP abstractions.

F. N64 hardware-specific code

Rewrite rather than emulate.

G. Data/assets

Keep compatible with extracted original assets, but convert them into PSP-friendly representations during the build.

5. Project Structure

Create something approximately like:

ssb64-psp/
├── Cargo.toml
├── Cargo.lock
├── build.rs
├── README.md
│
├── docs/
│   ├── architecture.md
│   ├── porting-status.md
│   ├── rendering.md
│   ├── audio.md
│   ├── memory.md
│   └── reverse-engineering.md
│
├── src/
│   ├── main.rs
│   │
│   ├── game/
│   │   ├── mod.rs
│   │   ├── match.rs
│   │   ├── fighter.rs
│   │   ├── attack.rs
│   │   ├── collision.rs
│   │   ├── physics.rs
│   │   ├── damage.rs
│   │   ├── animation.rs
│   │   ├── stage.rs
│   │   ├── camera.rs
│   │   ├── ai.rs
│   │   └── menus.rs
│   │
│   ├── engine/
│   │   ├── renderer.rs
│   │   ├── audio.rs
│   │   ├── input.rs
│   │   ├── memory.rs
│   │   ├── timing.rs
│   │   └── resources.rs
│   │
│   ├── psp/
│   │   ├── mod.rs
│   │   ├── renderer.rs
│   │   ├── audio.rs
│   │   ├── input.rs
│   │   ├── memory.rs
│   │   ├── timing.rs
│   │   ├── vfpu.rs
│   │   └── system.rs
│   │
│   ├── n64/
│   │   ├── mod.rs
│   │   ├── types.rs
│   │   ├── formats.rs
│   │   └── constants.rs
│   │
│   └── debug/
│       ├── mod.rs
│       ├── profiler.rs
│       └── diagnostics.rs
│
├── tools/
│   ├── extract/
│   ├── convert/
│   ├── inspect/
│   └── verify/
│
└── assets/
    └── generated/

Adjust this structure after inspecting the actual SSB decompilation.

Do not artificially force this structure if the source architecture suggests something better.

6. Build a PSP Hello World FIRST

Before touching Smash:

Create a minimal Rust PSP executable.

It must:

compile with cargo psp
generate an EBOOT.PBP
boot on PPSSPP
boot on real PSP hardware if available
initialize the display
initialize GU
clear the screen
render a triangle
read a controller
print debug information

This becomes the platform baseline.

Do not proceed until this works.

7. Build a PSP Rendering Testbed

Before porting Smash rendering, create a standalone renderer test.

Test:

triangle
indexed triangle
textured quad
alpha blending
depth buffer
fog
perspective projection
orthographic projection
multiple textures
animated texture
vertex colors
lighting
clipping
scissor rectangles
framebuffer screenshots

Implement:

trait Renderer {
    fn begin_frame(&mut self);
    fn end_frame(&mut self);

    fn draw_mesh(&mut self, mesh: &Mesh, transform: Mat4);
    fn draw_ui(&mut self, ...);
}

The final implementation should use sceGu rather than an expensive software renderer.

8. N64 → PSP Rendering Translation

This is one of the biggest parts of the project.

Do NOT emulate the N64 RDP.

Instead translate the game's rendering data into PSP-native structures.

Create a conversion pipeline:

N64 display list
      |
      v
parse commands
      |
      v
intermediate representation
      |
      v
PSP mesh/material
      |
      v
sceGu

Implement support for the actual rendering primitives used by Smash.

At minimum investigate:

triangles
triangle strips
vertex colors
texture coordinates
textures
TLUT/paletted textures
alpha
depth
culling
fog
material state
matrix transforms
environment colors
primitive colors

Do not assume all N64 rendering features are needed.

Measure actual Smash usage from the decompilation.

9. Texture Conversion Pipeline

The N64 uses texture formats that do not map directly to PSP texture formats.

Create a build-time converter.

Support the formats actually used by SSB, likely including:

RGBA16
IA16
IA8
I8
CI4
CI8

Convert them into PSP-friendly formats.

Prefer preconversion over runtime conversion.

Generated assets should look conceptually like:

generated/
  textures/
  models/
  animations/
  stages/
  ui/
  audio/

The source ROM remains external.

10. Coordinate System Conversion

Document:

N64 coordinate handedness
PSP coordinate system
matrix conventions
fixed-point/vector representations
depth range
projection conventions

Create explicit conversion functions.

Do NOT scatter coordinate conversion throughout gameplay code.

For example:

fn n64_to_psp_position(v: Vec3) -> Vec3
fn n64_to_psp_matrix(m: N64Matrix) -> Mat4

Ideally gameplay continues operating in the original game's coordinate system while the renderer performs conversion.

11. Memory Architecture

The N64 and PSP memory models are substantially different.

Study the SSB decompilation and identify:

static allocations
heaps
arenas
temporary allocations
object pools
DMA buffers
display-list buffers
audio buffers
task buffers

Then map them onto PSP memory.

The PSP has:

main RAM
VRAM
scratchpad
Media Engine memory

Do NOT attempt to perfectly emulate N64 memory.

Instead preserve the semantics required by game code.

Create explicit allocators/pools where useful.

For example:

GameArena
RenderArena
AudioArena
FrameArena
ObjectPool<T>

Avoid heap allocation in per-frame hot paths.

12. N64 OS Replacement

This should be treated as its own subsystem.

Identify every dependency on:

threads
message queues
events
timers
interrupts
DMA
file I/O
VI
SI
PI
AI
SP
DP
controller APIs

Create PSP equivalents.

For example:

N64 scheduler
       |
       v
PSP task scheduler

N64 message queue
       |
       v
PSP semaphore/event/queue

N64 timer
       |
       v
PSP high-resolution timer

N64 controller
       |
       v
sceCtrl

Do not emulate the N64 hardware unless the game code fundamentally depends on the behavior.

13. Game Timing

SSB is timing-sensitive.

Determine:

original frame rate
simulation tick
animation timing
hitlag timing
physics update timing
menu timing
RNG timing
AI timing

Create one authoritative game clock.

Do not tie simulation directly to rendering.

Target:

Game simulation: fixed timestep
Rendering: PSP display cadence

If the original game runs at a fixed 60 Hz simulation, preserve that behavior as closely as possible.

14. Controller Mapping

Map N64 controls to PSP.

Recommended default:

N64 Control        PSP

Analog stick        Analog nub
A                  Cross
B                  Circle
Z                  Left trigger
L                  L trigger
R                  R trigger
Start              Start
D-pad              D-pad
C-buttons          Face/secondary mapping

Because PSP lacks the N64's four C buttons, design a sensible mapping.

Potentially:

C-Up    Triangle
C-Down  Square
C-Left  D-pad Left
C-Right D-pad Right

But verify which C-button functions are actually important in SSB.

Make the mapping configurable later.

15. Audio

Do NOT initially attempt to reproduce the N64 audio architecture exactly.

First identify:

music sequences
sound effects
samples
banks
streaming audio
voice samples
timing

Build an intermediate asset representation.

Create:

trait AudioBackend {
    fn play_sfx(...);
    fn stop_sfx(...);
    fn play_music(...);
    fn stop_music(...);
    fn set_volume(...);
}

Initially implement simple PSP audio playback.

Once gameplay works, optimize.

Investigate Media Engine usage after the CPU implementation is stable.

16. VFPU Strategy

This is critical.

The PSP's VFPU can provide significant acceleration for:

vector math
matrix multiplication
transforms
collision calculations
animation
camera calculations

But do NOT immediately rewrite everything in assembly.

First:

Implement correct scalar Rust version.
Benchmark it.
Identify hot functions.
Implement VFPU version.
Compare output.
Benchmark again.

Create:

mod vfpu {
    pub fn vec3_add(...);
    pub fn vec3_dot(...);
    pub fn vec3_cross(...);
    pub fn mat4_mul(...);
    pub fn transform_vec3(...);
}

Use the VFPU assembler facilities available through the Rust PSP ecosystem where appropriate.

Keep scalar fallbacks for debugging.

17. First Major Milestone: Bootable Smash Engine

The first real milestone is NOT the title screen.

It is:

PSP boot
   ↓
engine initialization
   ↓
resource loading
   ↓
game initialization
   ↓
stage initialization
   ↓
one fighter spawned
   ↓
camera active
   ↓
render frame

Use a development/test scene if necessary.

Get one fighter rendered and controllable before implementing the whole game.

18. Second Major Milestone: One Complete Match

Implement enough systems for:

Battlefield
+
Mario
+
CPU opponent
+
damage
+
attacks
+
knockback
+
stocks
+
blast zones
+
KO
+
match end

Do NOT port every character first.

Use one simple fighter as the vertical slice.

The vertical slice should prove:

rendering
animation
input
physics
collision
attacks
damage
audio
camera
UI
game timing
19. Third Major Milestone: Complete Character Set

After one character works:

Port the remaining original fighters using the data and logic from the decompilation.

Do not duplicate engine code.

Character-specific behavior should be data/state driven wherever the original architecture permits.

Track each fighter separately:

Mario
Luigi
DK
Link
Samus
Captain Falcon
Ness
Yoshi
Kirby
Fox
Pikachu
Jigglypuff

Include unlockable characters.

Verify each fighter individually.

20. Collision System

This deserves special attention.

Determine how SSB represents:

hurtboxes
hitboxes
grabs
throws
ledges
platforms
stage boundaries
blast zones
collision surfaces

Build debugging visualization.

Development mode should be capable of displaying:

hitboxes
hurtboxes
collision polygons
fighter origins
velocity vectors
blast zones
ledge points

This will make reverse-engineering and port validation dramatically easier.

21. Physics Verification

Build deterministic tests.

Examples:

jump velocity
fall acceleration
horizontal acceleration
air control
traction
friction
knockback
DI
hitlag
launch angle
terminal velocity
ledge grab
tech
shield
roll
air dodge

Where possible, compare PSP results against the original N64 implementation.

The objective is not merely "looks similar".

The objective is:

same input
+
same initial state
=
same gameplay state

within expected floating-point differences.

22. Deterministic Replay/Test System

This is strongly recommended.

Create a debug replay format:

frame
controller state
RNG state

Then allow:

N64 reference
       |
       v
input replay
       |
       v
PSP implementation
       |
       v
state comparison

This gives the project an objective way to identify gameplay regressions.

23. Automated Regression Tests

Build tests for:

Math
vectors
matrices
angles
interpolation
Physics
movement
gravity
knockback
collision
Animation
animation frame progression
transitions
Rendering
texture conversion
vertex conversion
matrix conversion
Resource system
extraction
decompression
parsing
Gameplay
damage
stocks
KO
match transitions
24. Performance Budget

Do not wait until the end to profile.

Establish a target budget early.

Target:

60 FPS
16.67 ms/frame

Measure separately:

game simulation
collision
animation
AI
render preparation
GPU submission
audio
resource streaming

Create an on-screen debug profiler:

FPS: 60
CPU: 11.2 ms
GPU:  7.4 ms

Game:       4.1 ms
Collision:  1.8 ms
Animation:  0.9 ms
Render:     3.5 ms
Audio:      0.4 ms
Other:      0.5 ms

Use the PSP's hardware timers rather than relying on rough measurements.

25. PSP Memory/VRAM Optimization

Once the game works:

Optimize asset placement.

Potential model:

Main RAM
    game state
    physics
    CPU-side meshes
    audio metadata

VRAM
    framebuffers
    depth buffer
    textures
    GPU-friendly vertex buffers

Avoid repeatedly copying textures and meshes.

Cache converted assets.

Use double/triple buffering appropriately.

26. Asset Pipeline

Build tools so the user only needs to supply their ROM.

Example:

cargo psp

should eventually perform:

check ROM
extract assets
convert textures
convert models
convert audio
generate PSP resource package
compile Rust
link EBOOT

Prefer deterministic generated output.

Never require the user to manually copy hundreds of extracted assets.

27. Development ROM / Asset Separation

The repository should contain:

src/
tools/
tests/
docs/

but NOT:

Nintendo ROM
Nintendo textures
Nintendo audio
Nintendo models
Nintendo binary data

The build system should generate them locally from:

baserom.us.z64

This follows the model already used by the SSB decomp and other N64 ports. The SSB decomp specifically expects the user to provide their own ROM for extraction.

28. Use the Existing SSB Decomp Aggressively

Do not independently reverse-engineer things that the decompilation already solved.

Use the decompilation to determine:

function behavior
structures
constants
state machines
fighter logic
stage logic
rendering inputs
resource formats
audio formats
game states

The decompilation is the reference implementation.

The PSP port is a new backend/platform implementation.

29. Do NOT Make These Mistakes
Do not build an N64 emulator

This is unnecessary and wastes PSP CPU resources.

Do not blindly port N64 C to Rust

The result will reproduce abstractions that only existed because of N64 hardware.

Do not emulate the RSP/RDP

Translate rendering operations into PSP-native operations.

Do not optimize before profiling

Especially VFPU code.

Do not convert every asset at runtime

Preconvert assets during the build.

Do not use floating point everywhere without investigating the original representation

Determine where the original game uses fixed-point/integer representations.

Do not assume PSP and N64 memory layouts are interchangeable

They are not.

Do not make rendering logic depend on gameplay

Keep the renderer behind an abstraction.

30. Reference n64psp Architecture

Study n64psp carefully.

It explicitly aims to provide:

VFPU implementations
a bridge between N64 OS concepts and PSP calls
optimized reusable functionality across N64 ports

Use this as architectural inspiration rather than copying its C API into Rust.

The long-term goal should be a Rust equivalent:

ssb64-psp
     |
     +---- n64-psp-core
     |
     +---- psp-renderer
     |
     +---- psp-audio
     |
     +---- psp-input
     |
     +---- ssb-game

Potentially extract reusable portions into a future Rust n64-psp library after the Smash port is working.

31. Reference sf64-psp Architecture

Study how sf64-psp approaches:

PSP rendering
N64 rendering translation
PSPGL/sceGu
VFPU
audio
N64 platform compatibility
build system
ROM extraction
debugging

Do not copy its C implementation wholesale.

Translate the architectural ideas into idiomatic Rust.

One especially useful concept is maintaining a reusable N64→PSP layer instead of implementing every hardware translation directly inside the game.

32. Rendering Backend Evolution

Implement in this order:

Renderer 0

Simple PSP test renderer.

Renderer 1

Basic SSB geometry.

Renderer 2

Complete materials/textures.

Renderer 3

Effects:

transparency
fog
particles
shadows
UI
framebuffer effects
Renderer 4

Optimization:

batching
state sorting
texture caching
vertex caching
VRAM management

Do not attempt Renderer 4 before the game is visibly running.

33. Audio Backend Evolution

Implement:

Audio 0

Basic sound effects.

Audio 1

All SFX.

Audio 2

Music.

Audio 3

Correct sequencing/timing.

Audio 4

Media Engine acceleration if needed.

34. Menu/System Port

After gameplay works:

Port:

title screen
character select
mode select
options
VS mode
1P modes
training
records
unlock screens
pause menu
results
credits

Do not spend weeks on menus before the match engine works.

35. Save Data

Investigate the original save data structure.

Implement a PSP-native save system.

Do not emulate the N64 Controller Pak unless required.

Map game progress into a PSP save directory.

Preserve:

unlocked characters
unlocked stages
records
settings
progression
36. Debug Build

Create a debug build with:

F1  toggle profiler
F2  collision visualization
F3  hitbox visualization
F4  wireframe
F5  texture debug
F6  AI debug
F7  memory statistics
F8  renderer statistics

Exact controls can change later.

Add logging to ms0:/ or an appropriate PSP debug output mechanism where practical.

37. PPSSPP First, Real PSP Second

Development order:

Build on PC.
Run under PPSSPP.
Fix correctness.
Profile.
Test on real PSP.
Fix hardware-specific behavior.

PPSSPP is useful for rapid iteration but must NOT be treated as proof of real-hardware performance.

Real PSP testing is mandatory before declaring performance work complete.

38. CI

Create CI that:

checks Rust formatting
runs Rust tests
builds host tools
builds PSP target
verifies generated resources
produces EBOOT.PBP as an artifact

Do not require the copyrighted ROM in public CI.

Use synthetic/test assets for CI.

39. Milestone Plan
M0 — Research

Deliverables:

architecture document
dependency map
N64 subsystem map
asset format inventory
rendering command inventory
audio inventory
M1 — Rust PSP bootstrap

Deliverables:

Cargo project
EBOOT.PBP
controller input
GU initialization
triangle renderer
M2 — Resource pipeline

Deliverables:

ROM validation
extraction
decompression
texture conversion
basic model conversion
M3 — Rendering

Deliverables:

SSB stage geometry
textures
camera
depth
transparency
lighting/materials
M4 — Gameplay vertical slice

Deliverables:

Battlefield
Mario
one CPU
movement
attacks
collision
damage
knockback
KO
stock system
M5 — Audio

Deliverables:

SFX
music
correct timing
volume/mixing
M6 — Full gameplay

Deliverables:

all original characters
all original stages
CPU AI
items
game modes
M7 — Menus/save system

Deliverables:

title
character select
mode select
pause
results
progression
save data
M8 — Optimization

Deliverables:

60 FPS target
VFPU acceleration
GPU batching
VRAM optimization
memory optimization
audio optimization
M9 — Hardware validation

Deliverables:

PSP-1000 test
PSP-2000/3000 test
PPSSPP test
long-duration stability test
40. Definition of Done

The project is complete when:

 User supplies their own supported SSB64 .z64
 Build system validates ROM
 Assets are extracted automatically
 Assets are converted automatically
 No Nintendo assets are committed
 Rust PSP application builds
 EBOOT.PBP boots
 Title screen works
 Character select works
 All original characters work
 All original stages work
 Original gameplay mechanics work
 CPU players work
 Items work
 Audio works
 Music works
 Save data works
 Pause/results/menus work
 Gameplay runs at a stable target frame rate
 Real PSP hardware tested
 PPSSPP tested
 Debug/profiling tools exist
 Build is reproducible
 No copyrighted ROM/assets are distributed by the repository
41. AI Agent Operating Rules

The AI implementing this project must follow these rules.

Rule 1

Always inspect the existing SSB decompilation before implementing functionality that already exists there.

Rule 2

Always inspect sf64-psp and n64psp before designing an N64→PSP subsystem.

Rule 3

Never invent N64 behavior when the decompilation can answer the question.

Rule 4

Do not perform giant speculative rewrites.

Work subsystem-by-subsystem.

Rule 5

Every major subsystem must compile and have a test/debug path before moving on.

Rule 6

Keep platform code separate from game logic.

Rule 7

Prefer Rust abstractions over C-style global state where practical.

Rule 8

Do not introduce unsafe without a concrete reason.

When unsafe is required for PSP APIs/VFPU/GPU memory, isolate it behind small safe abstractions.

Rule 9

Profile before optimizing.

Rule 10

When uncertain about original behavior, document the uncertainty rather than guessing.

Create:

docs/reverse-engineering.md

with:

Question
Evidence
Hypothesis
Implementation
Confidence
Rule 11

Maintain a porting-status document.

Example:

Subsystem             Status       Notes

ROM extraction        COMPLETE
Texture conversion    COMPLETE
Renderer              40%
Audio                 10%
Input                 COMPLETE
Physics               20%
Mario                 60%
Other fighters        0%
Menus                 0%
Save system           0%
VFPU optimization     5%
Rule 12

Do not move to the next milestone merely because the code compiles.

Each milestone requires functional validation.

42. Immediate Tasks for the AI

Start NOW with these tasks, in this exact order:

Clone/inspect ssb-decomp-re.
Inspect its complete source tree and build system.
Identify the exact supported US ROM revision.
Verify how the user's .z64 will be detected.
Map the original game's major subsystems.
Clone/inspect sf64-psp.
Clone/inspect n64psp.
Produce a written architecture comparison between:
original SSB N64
sf64-psp
n64psp
proposed Rust PSP port
Identify which parts of n64psp should become reusable Rust abstractions.
Bootstrap a minimal Rust PSP application using rust-psp.
Verify it builds an EBOOT.PBP.
Verify it runs in PPSSPP.
Implement the PSP rendering testbed.
Implement ROM validation/extraction without committing the ROM.
Only then begin porting SSB itself.

Do not skip the reconnaissance phase.

The first objective is to produce an architecture and a working PSP/Rust skeleton, NOT to immediately start translating Smash source code.
