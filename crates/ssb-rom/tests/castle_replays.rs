//! Castle replays against N64 traces: Yoshi's Egg Lay and double jump
//! (warp ROM `warp_yoshi`, Training on Peach's Castle, Yoshi P1 at the
//! left spawn, the dummy Mario P2 at the right).
//!
//! The rows are the N64 game's TopN translation and `status_id`, printed to
//! two decimals by the Mupen64Plus harness (`tools/n64-headless`
//! `n64_driver.py`): N64 row `t` is replay tick `t`; a route step starting
//! at `s` is recorded at `t = s - 1` and acts on frame `s`
//! (`scene_inputs`'s Castle spit replay).

use ssb_capture::script::button;
use ssb_game::fighter::FighterKind;

#[path = "common/posed.rs"]
mod posed;
use posed::{tick_pair, tick_pair_pads, Pair, Posed};

#[path = "common/dream_land.rs"]
mod dream_land;
use dream_land::{dream_land_map, stage_floors, training_fighter};

/// Peach's Castle in the pack.
const CASTLE: u32 = 4;

fn open_pack() -> Option<Vec<u8>> {
    ssb_tablegen::test_rom(env!("CARGO_MANIFEST_DIR"))?;
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/generated/ssb64.pak"),
    )
    .ok()
}

/// The trace prints two decimals.
fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.0051
}

fn castle_pair(pack: &ssb_rom::pack::Pack<'_>) -> (Vec<ssb_game::weapon::MapSurface>, Vec<Posed>) {
    let (stage, segments) = stage_floors(pack, CASTLE);
    let map = dream_land_map(pack, &stage);
    let fighters = [(FighterKind::Yoshi, 0), (FighterKind::Mario, 1)]
        .into_iter()
        .map(|(kind, port)| Posed::new(pack, training_fighter(pack, &stage, kind, port, &segments)))
        .collect();
    (map, fighters)
}

/// `yoshiA`: route `200:1:b;320:1:b+y=80`, N64 frames 195-330. Per row:
/// tick; Yoshi's status, x, y; Mario's status, x, y, `vel_air` x, y, `ga`
/// and percent. Yoshi tongues Mario at frame 220 (`CaptureYoshi` 177),
/// lays him at 242 (`YoshiEgg` 178): the egg is placed 200 behind and 90
/// above Yoshi and moves by its lay velocity (-20, 60) on that frame. The
/// egg lands at frame 296 and stays at x -1213. The dummy is the Training
/// CPU: it starts to wiggle in the egg later (`ftComputer`'s trapped
/// statuses), so the rows stop before the escape.
const YOSHI_A: &str = "
195 10 -210.00 1399.25 10 765.00 1413.09 0.00 -0.00 0 0
196 10 -210.00 1399.25 10 765.00 1413.09 0.00 -0.00 0 0
197 10 -210.00 1399.25 10 765.00 1413.09 0.00 -0.00 0 0
198 10 -210.00 1399.25 10 765.00 1413.09 0.00 -0.00 0 0
199 10 -210.00 1399.25 10 765.00 1413.09 0.00 -0.00 0 0
200 228 -210.00 1399.25 10 765.00 1413.09 0.00 -0.00 0 0
201 228 -210.00 1399.25 10 765.00 1413.09 0.00 -0.00 0 0
202 228 -210.00 1399.25 10 765.00 1413.09 0.00 -0.00 0 0
203 228 -210.00 1399.25 10 765.00 1413.09 0.00 -0.00 0 0
204 228 -210.00 1399.25 10 765.00 1413.09 0.00 -0.00 0 0
205 228 -210.00 1399.25 10 765.00 1413.09 0.00 -0.00 0 0
206 228 -210.00 1399.25 10 765.00 1413.09 0.00 -0.00 0 0
207 228 -210.00 1399.25 10 765.00 1413.09 0.00 -0.00 0 0
208 228 -210.00 1399.25 10 765.00 1413.09 0.00 -0.00 0 0
209 228 -210.00 1399.25 10 765.00 1413.09 0.00 -0.00 0 0
210 228 -210.00 1399.25 10 765.00 1413.09 0.00 -0.00 0 0
211 228 -210.00 1399.25 10 765.00 1413.09 0.00 -0.00 0 0
212 228 -210.00 1399.25 10 765.00 1413.09 0.00 -0.00 0 0
213 228 -210.00 1399.25 10 765.00 1413.09 0.00 -0.00 0 0
214 228 -210.00 1399.25 10 765.00 1413.09 0.00 -0.00 0 0
215 228 -210.00 1399.25 10 765.00 1413.09 0.00 -0.00 0 0
216 228 -210.00 1399.25 10 765.00 1413.09 0.00 -0.00 0 0
217 228 -210.00 1399.25 10 765.00 1413.09 0.00 -0.00 0 0
218 228 -210.00 1399.25 10 765.00 1413.09 0.00 -0.00 0 0
219 228 -210.00 1399.25 10 765.00 1413.09 0.00 -0.00 0 0
220 229 -210.00 1399.25 177 699.85 1413.09 0.00 0.00 1 0
221 229 -210.00 1399.25 177 733.37 1413.09 0.00 0.00 1 0
222 229 -210.00 1399.25 177 665.00 1413.09 0.00 0.00 1 0
223 230 -210.00 1399.25 177 665.00 1413.09 0.00 0.00 1 0
224 230 -210.00 1399.25 177 531.06 1413.09 0.00 0.00 1 0
225 230 -210.00 1399.25 177 371.19 1413.09 0.00 0.00 1 0
226 230 -210.00 1399.25 177 237.25 1413.09 0.00 0.00 1 0
227 230 -210.00 1399.25 177 181.07 1413.09 0.00 0.00 1 0
228 230 -210.00 1399.25 177 180.07 1413.09 0.00 0.00 1 0
229 230 -210.00 1399.25 177 178.92 1413.09 0.00 0.00 1 0
230 230 -210.00 1399.25 177 177.65 1413.09 0.00 0.00 1 0
231 230 -210.00 1399.25 177 176.26 1413.09 0.00 0.00 1 0
232 230 -210.00 1399.25 177 174.78 1413.09 0.00 0.00 1 0
233 230 -210.00 1399.25 177 173.22 1413.09 0.00 0.00 1 0
234 230 -210.00 1399.25 177 171.59 1413.09 0.00 0.00 1 0
235 230 -210.00 1399.25 177 169.90 1413.09 0.00 0.00 1 0
236 230 -210.00 1399.25 177 168.19 1413.09 0.00 0.00 1 0
237 230 -210.00 1399.25 177 166.45 1413.09 0.00 0.00 1 0
238 230 -210.00 1399.25 177 164.70 1413.09 0.00 0.00 1 0
239 230 -210.00 1399.25 177 162.97 1413.09 0.00 0.00 1 0
240 230 -210.00 1399.25 177 161.26 1413.09 0.00 0.00 1 0
241 230 -210.00 1399.25 177 159.59 1413.09 0.00 0.00 1 0
242 230 -210.00 1399.25 178 -430.00 1549.25 -20.00 60.00 1 5
243 230 -210.00 1399.25 178 -449.80 1606.85 -19.80 57.60 1 5
244 230 -210.00 1399.25 178 -469.40 1662.05 -19.60 55.20 1 5
245 230 -210.00 1399.25 178 -488.80 1714.85 -19.40 52.80 1 5
246 230 -210.00 1399.25 178 -508.00 1765.25 -19.20 50.40 1 5
247 230 -210.00 1399.25 178 -527.00 1813.25 -19.00 48.00 1 5
248 230 -210.00 1399.25 178 -545.80 1858.85 -18.80 45.60 1 5
249 230 -210.00 1399.25 178 -564.40 1902.05 -18.60 43.20 1 5
250 230 -210.00 1399.25 178 -582.80 1942.85 -18.40 40.80 1 5
251 230 -210.00 1399.25 178 -601.00 1981.25 -18.20 38.40 1 5
252 230 -210.00 1399.25 178 -619.00 2017.25 -18.00 36.00 1 5
253 230 -210.00 1399.25 178 -636.80 2050.85 -17.80 33.60 1 5
254 230 -210.00 1399.25 178 -654.40 2082.05 -17.60 31.20 1 5
255 230 -210.00 1399.25 178 -671.80 2110.85 -17.40 28.80 1 5
256 230 -210.00 1399.25 178 -689.00 2137.25 -17.20 26.40 1 5
257 10 -210.00 1399.25 178 -706.00 2161.25 -17.00 24.00 1 5
258 10 -210.00 1399.25 178 -722.80 2182.85 -16.80 21.60 1 5
259 10 -210.00 1399.25 178 -739.40 2202.05 -16.60 19.20 1 5
260 10 -210.00 1399.25 178 -755.80 2218.85 -16.40 16.80 1 5
261 10 -210.00 1399.25 178 -772.00 2233.25 -16.20 14.40 1 5
262 10 -210.00 1399.25 178 -788.00 2245.25 -16.00 12.00 1 5
263 10 -210.00 1399.25 178 -803.80 2254.85 -15.80 9.60 1 5
264 10 -210.00 1399.25 178 -819.40 2262.05 -15.60 7.20 1 5
265 10 -210.00 1399.25 178 -834.80 2266.85 -15.40 4.80 1 5
266 10 -210.00 1399.25 178 -850.00 2269.25 -15.20 2.40 1 5
267 10 -210.00 1399.25 178 -865.00 2269.25 -15.00 -0.00 1 5
268 10 -210.00 1399.25 178 -879.80 2266.85 -14.80 -2.40 1 5
269 10 -210.00 1399.25 178 -894.40 2262.05 -14.60 -4.80 1 5
270 10 -210.00 1399.25 178 -908.80 2254.85 -14.40 -7.20 1 5
271 10 -210.00 1399.25 178 -923.00 2245.25 -14.20 -9.60 1 5
272 10 -210.00 1399.25 178 -937.00 2233.25 -14.00 -12.00 1 5
273 10 -210.00 1399.25 178 -950.80 2218.85 -13.80 -14.40 1 5
274 10 -210.00 1399.25 178 -964.40 2202.05 -13.60 -16.80 1 5
275 10 -210.00 1399.25 178 -977.80 2182.85 -13.40 -19.20 1 5
276 10 -210.00 1399.25 178 -991.00 2161.25 -13.20 -21.60 1 5
277 10 -210.00 1399.25 178 -1004.00 2137.25 -13.00 -24.00 1 5
278 10 -210.00 1399.25 178 -1016.80 2110.85 -12.80 -26.40 1 5
279 10 -210.00 1399.25 178 -1029.40 2082.05 -12.60 -28.80 1 5
280 10 -210.00 1399.25 178 -1041.80 2050.85 -12.40 -31.20 1 5
281 10 -210.00 1399.25 178 -1054.00 2017.25 -12.20 -33.60 1 5
282 10 -210.00 1399.25 178 -1066.00 1981.25 -12.00 -36.00 1 5
283 10 -210.00 1399.25 178 -1077.80 1942.85 -11.80 -38.40 1 5
284 10 -210.00 1399.25 178 -1089.40 1902.05 -11.60 -40.80 1 5
285 10 -210.00 1399.25 178 -1100.80 1858.85 -11.40 -43.20 1 5
286 10 -210.00 1399.25 178 -1112.00 1814.85 -11.20 -44.00 1 5
287 10 -210.00 1399.25 178 -1123.00 1770.85 -11.00 -44.00 1 5
288 10 -210.00 1399.25 178 -1133.80 1726.85 -10.80 -44.00 1 5
289 10 -210.00 1399.25 178 -1144.40 1682.85 -10.60 -44.00 1 5
290 10 -210.00 1399.25 178 -1154.80 1638.85 -10.40 -44.00 1 5
291 10 -210.00 1399.25 178 -1165.00 1594.85 -10.20 -44.00 1 5
292 10 -210.00 1399.25 178 -1175.00 1550.85 -10.00 -44.00 1 5
293 10 -210.00 1399.25 178 -1184.80 1506.85 -9.80 -44.00 1 5
294 10 -210.00 1399.25 178 -1194.40 1462.85 -9.60 -44.00 1 5
295 10 -210.00 1399.25 178 -1203.80 1418.85 -9.40 -44.00 1 5
296 10 -210.00 1399.25 178 -1213.00 1400.81 -9.20 -44.00 0 5
297 10 -210.00 1399.25 178 -1213.00 1400.81 0.00 0.00 0 5
298 10 -210.00 1399.25 178 -1213.00 1400.81 0.00 0.00 0 5
299 10 -210.00 1399.25 178 -1213.00 1400.81 0.00 0.00 0 5
300 10 -210.00 1399.25 178 -1213.00 1400.81 0.00 0.00 0 5
301 10 -210.00 1399.25 178 -1213.00 1400.81 0.00 0.00 0 5
302 10 -210.00 1399.25 178 -1213.00 1400.81 0.00 0.00 0 5
303 10 -210.00 1399.25 178 -1213.00 1400.81 0.00 0.00 0 5
304 10 -210.00 1399.25 178 -1213.00 1400.81 0.00 0.00 0 5
305 10 -210.00 1399.25 178 -1213.00 1400.81 0.00 0.00 0 5
306 10 -210.00 1399.25 178 -1213.00 1400.81 0.00 0.00 0 5
307 10 -210.00 1399.25 178 -1213.00 1400.81 0.00 0.00 0 5
308 10 -210.00 1399.25 178 -1213.00 1400.81 0.00 0.00 0 5
309 10 -210.00 1399.25 178 -1213.00 1400.81 0.00 0.00 0 5
310 10 -210.00 1399.25 178 -1213.00 1400.81 0.00 0.00 0 5
311 10 -210.00 1399.25 178 -1213.00 1400.81 0.00 0.00 0 5
312 10 -210.00 1399.25 178 -1213.00 1400.81 0.00 0.00 0 5
313 10 -210.00 1399.25 178 -1213.00 1400.81 0.00 0.00 0 5
314 10 -210.00 1399.25 178 -1213.00 1400.81 0.00 0.00 0 5
315 10 -210.00 1399.25 178 -1213.00 1400.81 0.00 0.00 0 5
316 10 -210.00 1399.25 178 -1213.00 1400.81 0.00 0.00 0 5
317 10 -210.00 1399.25 178 -1213.00 1400.81 0.00 0.00 0 5
318 10 -210.00 1399.25 178 -1213.00 1400.81 0.00 0.00 0 5
319 10 -210.00 1399.25 178 -1213.00 1400.81 0.00 0.00 0 5
320 222 -210.00 1399.25 178 -1213.00 1400.81 0.00 0.00 0 5
321 222 -210.00 1399.25 178 -1213.00 1400.81 0.00 0.00 0 5
322 222 -210.00 1399.25 178 -1213.00 1400.81 0.00 0.00 0 5
323 222 -210.00 1399.25 178 -1213.00 1400.81 0.00 0.00 0 5
324 222 -210.00 1399.25 178 -1213.00 1400.81 0.00 0.00 0 5
325 222 -210.00 1399.25 178 -1213.00 1400.81 0.00 0.00 0 5
326 222 -210.00 1399.25 178 -1213.00 1400.81 0.00 0.00 0 5
327 222 -210.00 1399.25 178 -1213.00 1400.81 0.00 0.00 0 5
328 222 -210.00 1399.25 178 -1213.00 1400.81 0.00 0.00 0 5
329 222 -210.00 1399.25 178 -1213.00 1400.81 0.00 0.00 0 5
330 222 -210.00 1399.25 178 -1213.00 1400.81 0.00 0.00 0 5
";

/// `dj`: route `10:1:y=80;40:1:y=80;41:1:x=0`, N64 frames 0-125. Per row:
/// tick, Yoshi's status, x, y, `ga`. `JumpAerialF` (24) runs frames 40-113:
/// its 70 animation frames plus 4 frames (55-58) of hitlag from the Castle
/// bumper, which the item-less replay has no counterpart of. Frames 55-58
/// are skipped and later rows read 4 ticks early; from frame 123 the
/// bumper's knockback (`DamageFlyN`) ends the comparison.
const YOSHI_DJ: &str = "
0 10 -210.00 1399.25 0
1 10 -210.00 1399.25 0
2 10 -210.00 1399.25 0
3 10 -210.00 1399.25 0
4 10 -210.00 1399.25 0
5 10 -210.00 1399.25 0
6 10 -210.00 1399.25 0
7 10 -210.00 1399.25 0
8 10 -210.00 1399.25 0
9 10 -210.00 1399.25 0
10 20 -210.00 1399.25 0
11 20 -210.00 1399.25 0
12 20 -210.00 1399.25 0
13 20 -210.00 1399.25 0
14 20 -210.00 1399.25 0
15 22 -210.00 1479.95 1
16 22 -210.00 1557.85 1
17 22 -210.00 1632.95 1
18 22 -210.00 1705.25 1
19 22 -210.00 1774.75 1
20 22 -210.00 1841.45 1
21 22 -210.00 1905.35 1
22 22 -210.00 1966.45 1
23 22 -210.00 2024.75 1
24 22 -210.00 2080.25 1
25 22 -210.00 2132.95 1
26 22 -210.00 2182.85 1
27 22 -210.00 2229.95 1
28 22 -210.00 2274.25 1
29 22 -210.00 2315.75 1
30 22 -210.00 2354.45 1
31 22 -210.00 2390.35 1
32 22 -210.00 2423.45 1
33 22 -210.00 2453.75 1
34 22 -210.00 2481.25 1
35 22 -210.00 2505.95 1
36 22 -210.00 2527.85 1
37 22 -210.00 2546.95 1
38 22 -210.00 2563.25 1
39 22 -210.00 2576.75 1
40 24 -210.00 2576.75 1
41 24 -210.00 2552.31 1
42 24 -210.00 2531.31 1
43 24 -210.00 2513.87 1
44 24 -210.00 2500.09 1
45 24 -210.00 2490.09 1
46 24 -210.00 2483.98 1
47 24 -210.00 2481.88 1
48 24 -210.00 2490.09 1
49 24 -210.00 2510.19 1
50 24 -210.00 2540.95 1
51 24 -210.00 2581.10 1
52 24 -210.00 2629.43 1
53 24 -210.00 2684.68 1
54 24 -210.00 2745.62 1
55 24 -210.00 2745.62 1
56 24 -210.00 2745.62 1
57 24 -210.00 2745.62 1
58 24 -210.00 2745.62 1
59 24 -210.00 2811.01 1
60 24 -210.00 2879.60 1
61 24 -210.00 2950.15 1
62 24 -210.00 3021.43 1
63 24 -210.00 3092.40 1
64 24 -210.00 3162.81 1
65 24 -210.00 3232.57 1
66 24 -210.00 3301.57 1
67 24 -210.00 3369.73 1
68 24 -210.00 3436.95 1
69 24 -210.00 3503.14 1
70 24 -210.00 3568.20 1
71 24 -210.00 3632.04 1
72 24 -210.00 3694.56 1
73 24 -210.00 3755.68 1
74 24 -210.00 3815.30 1
75 24 -210.00 3873.32 1
76 24 -210.00 3929.65 1
77 24 -210.00 3984.19 1
78 24 -210.00 4036.86 1
79 24 -210.00 4087.56 1
80 24 -210.00 4136.19 1
81 24 -210.00 4182.66 1
82 24 -210.00 4226.88 1
83 24 -210.00 4268.75 1
84 24 -210.00 4308.18 1
85 24 -210.00 4345.08 1
86 24 -210.00 4379.34 1
87 24 -210.00 4410.89 1
88 24 -210.00 4439.61 1
89 24 -210.00 4465.43 1
90 24 -210.00 4488.24 1
91 24 -210.00 4507.95 1
92 24 -210.00 4524.47 1
93 24 -210.00 4537.71 1
94 24 -210.00 4547.56 1
95 24 -210.00 4553.94 1
96 24 -210.00 4556.75 1
97 24 -210.00 4555.81 1
98 24 -210.00 4551.07 1
99 24 -210.00 4542.66 1
100 24 -210.00 4530.70 1
101 24 -210.00 4515.33 1
102 24 -210.00 4496.67 1
103 24 -210.00 4474.83 1
104 24 -210.00 4449.96 1
105 24 -210.00 4422.17 1
106 24 -210.00 4391.60 1
107 24 -210.00 4358.36 1
108 24 -210.00 4322.59 1
109 24 -210.00 4284.41 1
110 24 -210.00 4243.94 1
111 24 -210.00 4201.31 1
112 24 -210.00 4156.66 1
113 24 -210.00 4110.10 1
114 27 -210.00 4060.73 1
115 27 -210.00 4008.57 1
116 27 -210.00 3953.61 1
117 27 -210.00 3895.84 1
118 27 -210.00 3837.84 1
119 27 -210.00 3779.84 1
120 27 -210.00 3721.84 1
121 27 -210.00 3663.84 1
122 27 -210.00 3605.84 1
123 52 -210.00 3547.84 1
124 52 -210.00 3547.84 1
125 52 -210.00 3547.84 1
";

fn rows(table: &str) -> impl Iterator<Item = Vec<f32>> + '_ {
    table
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.split_whitespace().map(|v| v.parse().unwrap()).collect())
}

#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn yoshi_egg_lay_matches_the_n64() {
    let Some(bytes) = open_pack() else {
        return;
    };
    let pack = ssb_rom::pack::Pack::open(&bytes).unwrap();
    let (map, mut fighters) = castle_pair(&pack);
    let pad = |t: u64| match t {
        200 => (button::B, 0, 0),
        320 => (button::B, 0, 80),
        _ => (0, 0, 0),
    };
    let want: Vec<Vec<f32>> = rows(YOSHI_A).collect();
    let mut pair = Pair::default();
    let mut log = Vec::new();
    let mut first_bad = None;
    for tick in 1..=330u64 {
        let _ = tick_pair(&pack, &map, &mut fighters, &pad, tick, &mut pair);
        let (y, m) = (&fighters[0].fighter, &fighters[1].fighter);
        let got = [
            f32::from(y.status.status.id()),
            y.pos.x,
            y.pos.y,
            f32::from(m.status.status.id()),
            m.pos.x,
            m.pos.y,
            m.physics.vel_air.x,
            m.physics.vel_air.y,
            if m.is_grounded() { 0.0 } else { 1.0 },
            m.damage as f32,
        ];
        let Some(row) = want.iter().find(|r| r[0] as u64 == tick) else {
            continue;
        };
        let ok = got.iter().zip(&row[1..]).all(|(g, w)| near(*g, *w));
        log.push(format!(
            "{tick} {} rust {got:?}\n{tick} {}  n64 {:?}",
            if ok { " " } else { "X" },
            if ok { " " } else { "X" },
            &row[1..]
        ));
        if !ok && first_bad.is_none() {
            first_bad = Some(tick);
        }
    }
    let log = log.join("\n");
    if std::env::var_os("SCENE_TRACE").is_some() {
        println!("yoshi egg lay on the Castle:\n{log}");
    }
    assert_eq!(first_bad, None, "{log}");
}

/// `gFTManagerMotionCount` and `gFTManagerStatUpdateCount` are the
/// battle's, reset per battle (`ftmanager.c:163-164`): two runs of one
/// replay draw the same counts tick for tick, and its fighters and pools
/// draw from one pair.
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn a_replay_draws_the_same_counts_every_run() {
    let Some(bytes) = open_pack() else {
        return;
    };
    let pack = ssb_rom::pack::Pack::open(&bytes).unwrap();
    let pad = |t: u64| match t {
        200 => (button::B, 0, 0),
        320 => (button::B, 0, 80),
        _ => (0, 0, 0),
    };
    let run = || {
        let (map, mut fighters) = castle_pair(&pack);
        let mut pair = Pair::default();
        let mut counts = Vec::new();
        for tick in 1..=330u64 {
            let _ = tick_pair(&pack, &map, &mut fighters, &pad, tick, &mut pair);
            for f in &fighters {
                assert!(f.fighter.stats.counts.same(&pair.items.counts));
            }
            assert!(pair.weapons.counts.same(&pair.items.counts));
            counts.push(pair.items.counts.peek());
        }
        counts
    };
    let first = run();
    assert!(first[0] < first[first.len() - 1], "{first:?}");
    assert_eq!(first, run());
}

#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn yoshi_double_jump_matches_the_n64() {
    let Some(bytes) = open_pack() else {
        return;
    };
    let pack = ssb_rom::pack::Pack::open(&bytes).unwrap();
    let (map, mut fighters) = castle_pair(&pack);
    let pad = |t: u64| match t {
        10 | 40 => (0, 0, 80),
        _ => (0, 0, 0),
    };
    let want: Vec<Vec<f32>> = rows(YOSHI_DJ).collect();
    let mut pair = Pair::default();
    let mut log = Vec::new();
    let mut first_bad = None;
    // The N64 frame a replay tick stands for, around the bumper hitlag.
    let n64_tick = |tick: u64| if tick <= 54 { tick } else { tick + 4 };
    for tick in 1..=118u64 {
        let _ = tick_pair(&pack, &map, &mut fighters, &pad, tick, &mut pair);
        let y = &fighters[0].fighter;
        let tick = n64_tick(tick);
        let got = [
            f32::from(y.status.status.id()),
            y.pos.x,
            y.pos.y,
            if y.is_grounded() { 0.0 } else { 1.0 },
        ];
        let Some(row) = want.iter().find(|r| r[0] as u64 == tick) else {
            continue;
        };
        let ok = got.iter().zip(&row[1..]).all(|(g, w)| near(*g, *w));
        log.push(format!(
            "{tick} {} rust {got:?} n64 {:?}",
            if ok { " " } else { "X" },
            &row[1..]
        ));
        if !ok && first_bad.is_none() {
            first_bad = Some(tick);
        }
    }
    let log = log.join("\n");
    if std::env::var_os("SCENE_TRACE").is_some() {
        println!("yoshi double jump on the Castle:\n{log}");
    }
    assert_eq!(first_bad, None, "{log}");
}

/// `kwiggle`: warp ROM `warp_kirbyvssamus` (Training Castle, Samus the
/// dummy), P1 route `9:8:x=127;49:40:b`: Kirby dashes to x 498 and inhales
/// Samus. At frame 100 the dummy's `pkind` is written to `Man` (RDRAM
/// `fp + 0x20`; Training points its `input.controller` at port 2), and port 2
/// holds the stick down (y -80) on frames 110-111. The Castle's upper floor
/// (line 0) is `PASS`: the captured Samus's down wiggle
/// (`ftCommonCaptureWaitKirbyUpdateBreakoutVars`) switches Kirby to
/// `SpecialAirNWait` (284) with `vel_air.y` -20 and `ignore_line_id` set to
/// that floor, restarts the figatree at frame 0, and Kirby drops through it
/// to line 1 (y 510), landing back in `SpecialNWait` (275) at frame 125.
/// Per row: tick; Kirby's status, `anim_frame`, x, y, `vel_air.y`, `ga`;
/// Samus's status, x, y.
const KIRBY_DOWN_WIGGLE: &str = "
66 269 18.00 498.00 1398.15 -0.00 0 10 765.00 1413.09
67 269 19.00 498.00 1398.15 -0.00 0 10 765.00 1413.09
68 272 2.00 498.00 1398.15 -0.00 0 173 737.00 1449.09
69 272 3.00 498.00 1398.15 -0.00 0 173 709.00 1485.09
70 272 4.00 498.00 1398.15 -0.00 0 173 681.00 1498.15
71 273 1.00 498.00 1398.15 -0.00 0 174 508.00 1398.15
72 273 2.00 498.00 1398.15 -0.00 0 174 508.00 1398.15
73 273 3.00 498.00 1398.15 -0.00 0 174 508.00 1398.15
74 273 4.00 498.00 1398.15 -0.00 0 174 508.00 1398.15
75 273 5.00 498.00 1398.15 -0.00 0 174 508.00 1398.15
76 273 6.00 498.00 1398.15 -0.00 0 174 508.00 1398.15
77 273 7.00 498.00 1398.15 -0.00 0 174 508.00 1398.15
78 273 8.00 498.00 1398.15 -0.00 0 174 508.00 1398.15
79 273 9.00 498.00 1398.15 -0.00 0 174 508.00 1398.15
80 273 10.00 498.00 1398.15 -0.00 0 174 508.00 1398.15
81 273 11.00 498.00 1398.15 -0.00 0 174 508.00 1398.15
82 273 12.00 498.00 1398.15 -0.00 0 174 508.00 1398.15
83 273 13.00 498.00 1398.15 -0.00 0 174 508.00 1398.15
84 273 14.00 498.00 1398.15 -0.00 0 174 508.00 1398.15
85 273 15.00 498.00 1398.15 -0.00 0 174 508.00 1398.15
86 273 16.00 498.00 1398.15 -0.00 0 174 508.00 1398.15
87 273 17.00 498.00 1398.15 -0.00 0 174 508.00 1398.15
88 273 18.00 498.00 1398.15 -0.00 0 174 508.00 1398.15
89 273 19.00 498.00 1398.15 -0.00 0 174 508.00 1398.15
90 275 1.00 498.00 1398.15 -0.00 0 174 508.00 1398.15
91 275 2.00 498.00 1398.15 -0.00 0 174 508.00 1398.15
92 275 3.00 498.00 1398.15 -0.00 0 174 508.00 1398.15
93 275 4.00 498.00 1398.15 -0.00 0 174 508.00 1398.15
94 275 5.00 498.00 1398.15 -0.00 0 174 508.00 1398.15
95 275 6.00 498.00 1398.15 -0.00 0 174 508.00 1398.15
96 275 7.00 498.00 1398.15 -0.00 0 174 508.00 1398.15
97 275 8.00 498.00 1398.15 -0.00 0 174 508.00 1398.15
98 275 9.00 498.00 1398.15 -0.00 0 174 508.00 1398.15
99 275 10.00 498.00 1398.15 -0.00 0 174 508.00 1398.15
100 275 11.00 498.00 1398.15 -0.00 0 174 508.00 1398.15
101 275 12.00 498.00 1398.15 -0.00 0 174 508.00 1398.15
102 275 13.00 498.00 1398.15 -0.00 0 174 508.00 1398.15
103 275 14.00 498.00 1398.15 -0.00 0 174 508.00 1398.15
104 275 15.00 498.00 1398.15 -0.00 0 174 508.00 1398.15
105 275 16.00 498.00 1398.15 -0.00 0 174 508.00 1398.15
106 275 17.00 498.00 1398.15 -0.00 0 174 508.00 1398.15
107 275 18.00 498.00 1398.15 -0.00 0 174 508.00 1398.15
108 275 19.00 498.00 1398.15 -0.00 0 174 508.00 1398.15
109 275 20.00 498.00 1398.15 -0.00 0 174 508.00 1398.15
110 284 0.00 498.00 1373.35 -24.80 1 174 508.00 1373.35
111 284 0.00 498.00 1343.75 -29.60 1 174 508.00 1343.75
112 284 1.00 498.00 1309.35 -34.40 1 174 508.00 1309.35
113 284 2.00 498.00 1270.15 -39.20 1 174 508.00 1270.15
114 284 3.00 498.00 1226.15 -44.00 1 174 508.00 1226.15
115 284 4.00 498.00 1177.35 -48.80 1 174 508.00 1177.35
116 284 5.00 498.00 1123.75 -53.60 1 174 508.00 1123.75
117 284 6.00 498.00 1065.35 -58.40 1 174 508.00 1065.35
118 284 7.00 498.00 1002.15 -63.20 1 174 508.00 1002.15
119 284 8.00 498.00 934.15 -68.00 1 174 508.00 934.15
120 284 9.00 498.00 861.35 -72.80 1 174 508.00 861.35
121 284 10.00 498.00 783.75 -77.60 1 174 508.00 783.75
122 284 11.00 498.00 701.35 -82.40 1 174 508.00 701.35
123 284 12.00 498.00 614.15 -87.20 1 174 508.00 614.15
124 284 13.00 498.00 522.15 -92.00 1 174 508.00 522.15
125 275 14.00 498.00 510.00 -96.00 0 174 508.00 510.00
126 275 15.00 498.00 510.00 -0.00 0 174 508.00 510.00
127 275 16.00 498.00 510.00 -0.00 0 174 508.00 510.00
128 275 17.00 498.00 510.00 -0.00 0 174 508.00 510.00
129 275 18.00 498.00 510.00 -0.00 0 174 508.00 510.00
130 275 19.00 498.00 510.00 -0.00 0 174 508.00 510.00
131 275 20.00 498.00 510.00 -0.00 0 174 508.00 510.00
132 275 21.00 498.00 510.00 -0.00 0 174 508.00 510.00
133 275 22.00 498.00 510.00 -0.00 0 174 508.00 510.00
134 275 23.00 498.00 510.00 -0.00 0 174 508.00 510.00
135 275 24.00 498.00 510.00 -0.00 0 174 508.00 510.00
136 275 25.00 498.00 510.00 -0.00 0 174 508.00 510.00
137 275 26.00 498.00 510.00 -0.00 0 174 508.00 510.00
138 275 27.00 498.00 510.00 -0.00 0 174 508.00 510.00
139 275 28.00 498.00 510.00 -0.00 0 174 508.00 510.00
140 275 29.00 498.00 510.00 -0.00 0 174 508.00 510.00
";

#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn kirby_down_wiggle_drops_through_the_castle_floor_as_on_the_n64() {
    let Some(bytes) = open_pack() else {
        return;
    };
    let pack = ssb_rom::pack::Pack::open(&bytes).unwrap();
    let (stage, segments) = stage_floors(&pack, CASTLE);
    let map = dream_land_map(&pack, &stage);
    let mut fighters: Vec<Posed> = [(FighterKind::Kirby, 0), (FighterKind::Samus, 1)]
        .into_iter()
        .map(|(kind, port)| {
            Posed::new(
                &pack,
                training_fighter(&pack, &stage, kind, port, &segments),
            )
        })
        .collect();
    let pad0 = |t: u64| match t {
        9..=16 => (0, 127, 0),
        49..=88 => (button::B, 0, 0),
        _ => (0, 0, 0),
    };
    let pad1 = |t: u64| match t {
        110..=111 => (0, 0, -80),
        _ => (0, 0, 0),
    };
    let want: Vec<Vec<f32>> = rows(KIRBY_DOWN_WIGGLE).collect();
    let mut pair = Pair::default();
    let mut log = Vec::new();
    let mut first_bad = None;
    let mut transitions = Vec::new();
    for tick in 1..=140u64 {
        let _ = tick_pair_pads(&pack, &map, &mut fighters, &pad0, &pad1, tick, &mut pair);
        let (k, s) = (&fighters[0].fighter, &fighters[1].fighter);
        let got = [
            f32::from(k.status.status.id()),
            k.status.anim_frame,
            k.pos.x,
            k.pos.y,
            k.physics.vel_air.y,
            if k.is_grounded() { 0.0 } else { 1.0 },
            f32::from(s.status.status.id()),
            s.pos.x,
            s.pos.y,
        ];
        let now = (k.status.status, s.status.status);
        if transitions.last().is_none_or(|(_, last)| *last != now) {
            transitions.push((tick, now));
            log.push(format!(
                "{tick} K {:?} {} x {:.2} / S {:?}",
                now.0, k.status.anim_frame, k.pos.x, now.1
            ));
        }
        let Some(row) = want.iter().find(|r| r[0] as u64 == tick) else {
            continue;
        };
        let ok = got.iter().zip(&row[1..]).all(|(g, w)| near(*g, *w));
        log.push(format!(
            "{tick} {} rust {got:?}\n{tick} {}  n64 {:?}",
            if ok { " " } else { "X" },
            if ok { " " } else { "X" },
            &row[1..]
        ));
        if !ok && first_bad.is_none() {
            first_bad = Some(tick);
        }
    }
    let log = log.join("\n");
    if std::env::var_os("SCENE_TRACE").is_some() {
        println!("kirby down wiggle on the Castle:\n{log}");
    }
    assert_eq!(first_bad, None, "{log}");
}

/// `kwj`: warp ROM `warp_kirby` (Training Castle, Mario the dummy), P1 route
/// `8:8:x=127;48:40:b`: Kirby dashes to x 498 and inhales Mario. At frame
/// 100 the dummy's `pkind` is written to `Man`; port 2 taps the stick right
/// (x 80) on frames 120 and 134 and left (x -80) on 126. Each wiggle
/// (`ftCommonCaptureWaitKirbyUpdateBreakoutVars`) pushes Kirby and restarts
/// its figatree (`lbCommonAddFighterPartsFigatree`) after Kirby's own play
/// that frame: the frame draws the clip run on, the next parse poses frame 0
/// without counting (`AOBJ_ANIM_CHANGED`), the one after frame 1.
/// Per row: tick; Kirby's status, `anim_frame`, x, y; Mario's x; then the
/// rotation words of Kirby's joints 6 and 10 (`fp->joints[]`), which the
/// inhale clips' skeleton holds at 2 and 6.
const KIRBY_SIDE_WIGGLE: &str = "
116 275 28.00 498.00 1398.15 508.00 3ca24dd4 00000000 00000000 bf9503ba bf5b0e39 4028c106
117 275 29.00 498.00 1398.15 508.00 3bf6c080 00000000 00000000 bf95bdc7 bf59df0f 402941a2
118 275 30.00 498.00 1398.15 508.00 bba00000 00000000 00000000 bf967996 bf58a72f 4029c23e
119 275 31.00 498.00 1398.15 508.00 bc8ce35b 00000000 00000000 bf9736ae bf57670a 402a4279
120 275 0.00 512.40 1398.13 522.40 bcef1aa1 00000000 00000000 bf97f495 bf561f0f 402ac1f3
121 275 0.00 521.20 1398.11 531.20 00000000 00000000 00000000 bf8b0000 bf678000 4020c000
122 275 1.00 524.40 1398.11 534.40 3b226e98 00000000 00000000 bf8ac7b2 bf67f567 4020a578
123 275 2.00 524.40 1398.11 534.40 3bc20c4a 00000000 00000000 bf8a9e76 bf6855e9 402095a5
124 275 3.00 524.40 1398.11 534.40 3c2722d1 00000000 00000000 bf8a83d5 bf68a1f5 40209020
125 275 4.00 524.40 1398.11 534.40 3c78d4fe 00000000 00000000 bf8a7751 bf68d9fc 40209489
126 275 0.00 510.00 1398.13 520.00 3caa0000 00000000 00000000 bf8a7873 bf68fe66 4020a27e
127 275 0.00 501.20 1398.14 511.20 00000000 00000000 00000000 bf8b0000 bf678000 4020c000
128 275 1.00 498.00 1398.15 508.00 3b226e98 00000000 00000000 bf8ac7b2 bf67f567 4020a578
129 275 2.00 498.00 1398.15 508.00 3bc20c4a 00000000 00000000 bf8a9e76 bf6855e9 402095a5
130 275 3.00 498.00 1398.15 508.00 3c2722d1 00000000 00000000 bf8a83d5 bf68a1f5 40209020
131 275 4.00 498.00 1398.15 508.00 3c78d4fe 00000000 00000000 bf8a7751 bf68d9fc 40209489
132 275 5.00 498.00 1398.15 508.00 3caa0000 00000000 00000000 bf8a7873 bf68fe66 4020a27e
133 275 6.00 498.00 1398.15 508.00 3cdb4396 00000000 00000000 bf8a86c2 bf690fad 4020b99f
134 275 0.00 512.40 1398.13 522.40 3d079376 00000000 00000000 bf8aa1c4 bf690e39 4020d989
135 275 0.00 521.20 1398.11 531.20 00000000 00000000 00000000 bf8b0000 bf678000 4020c000
136 275 1.00 524.40 1398.11 534.40 3b226e98 00000000 00000000 bf8ac7b2 bf67f567 4020a578
137 275 2.00 524.40 1398.11 534.40 3bc20c4a 00000000 00000000 bf8a9e76 bf6855e9 402095a5
138 275 3.00 524.40 1398.11 534.40 3c2722d1 00000000 00000000 bf8a83d5 bf68a1f5 40209020
139 275 4.00 524.40 1398.11 534.40 3c78d4fe 00000000 00000000 bf8a7751 bf68d9fc 40209489
140 275 5.00 524.40 1398.11 534.40 3caa0000 00000000 00000000 bf8a7873 bf68fe66 4020a27e
";

#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn kirby_side_wiggles_restart_the_drawn_clip_as_on_the_n64() {
    let Some(bytes) = open_pack() else {
        return;
    };
    let pack = ssb_rom::pack::Pack::open(&bytes).unwrap();
    let (stage, segments) = stage_floors(&pack, CASTLE);
    let map = dream_land_map(&pack, &stage);
    let mut fighters: Vec<Posed> = [(FighterKind::Kirby, 0), (FighterKind::Mario, 1)]
        .into_iter()
        .map(|(kind, port)| {
            Posed::new(
                &pack,
                training_fighter(&pack, &stage, kind, port, &segments),
            )
        })
        .collect();
    let pad0 = |t: u64| match t {
        8..=15 => (0, 127, 0),
        48..=87 => (button::B, 0, 0),
        _ => (0, 0, 0),
    };
    let pad1 = |t: u64| match t {
        120 | 134 => (0, 80, 0),
        126 => (0, -80, 0),
        _ => (0, 0, 0),
    };
    let want: Vec<Vec<String>> = KIRBY_SIDE_WIGGLE
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.split_whitespace().map(str::to_owned).collect())
        .collect();
    let mut pair = Pair::default();
    let mut log = Vec::new();
    let mut first_bad = None;
    for tick in 0..=140u64 {
        let _ = tick_pair_pads(&pack, &map, &mut fighters, &pad0, &pad1, tick, &mut pair);
        // `ftKirbySpecialNCatchEatSetStatusParam` and
        // `ftKirbySpecialNEatSetStatusParam` run `proc_slope` at once, but
        // keep the contour SpecialNStart/Loop cleared: no leg fit (the N64's
        // joints 19-30 equal the bare clip from 60 to 76, RE-510).
        let k = &fighters[0].fighter;
        if matches!(k.status.status.id(), 272 | 273) {
            assert_eq!(k.slope_contour, 0, "tick {tick}: {:?}", k.status.status);
        }
        let Some(row) = want.iter().find(|r| r[0].parse::<u64>() == Ok(tick)) else {
            continue;
        };
        let k = &fighters[0];
        let f = &k.fighter;
        let got = [
            f32::from(f.status.status.id()),
            f.status.anim_frame,
            f.pos.x,
            f.pos.y,
            fighters[1].fighter.pos.x,
        ];
        let want_f: Vec<f32> = row[1..6].iter().map(|v| v.parse().unwrap()).collect();
        let joints: Vec<u32> = [2, 6]
            .iter()
            .flat_map(|&i| {
                k.skeleton
                    .pose(i)
                    .map_or([0; 3], |p| p.rotate.map(f32::to_bits))
            })
            .collect();
        let want_j: Vec<u32> = row[6..12]
            .iter()
            .map(|v| u32::from_str_radix(v, 16).unwrap())
            .collect();
        let ok = got.iter().zip(&want_f).all(|(g, w)| near(*g, *w)) && joints == want_j;
        log.push(format!(
            "{tick} {} rust {got:?} {joints:08x?}\n{tick} {}  n64 {want_f:?} {want_j:08x?}",
            if ok { " " } else { "X" },
            if ok { " " } else { "X" },
        ));
        if !ok && first_bad.is_none() {
            first_bad = Some(tick);
        }
    }
    let log = log.join("\n");
    if std::env::var_os("SCENE_TRACE").is_some() {
        println!("kirby side wiggles on the Castle:\n{log}");
    }
    assert_eq!(first_bad, None, "{log}");
}

/// `dkc2` (`re474`, warp ROM `warp_donkey`): Donkey Kong walks to the
/// dummy Mario, grabs him (R, t=49), forward-throws into the cargo carry
/// (stick 80, t=59), button-jumps (C-up, t=99) and throws in the air (A,
/// t=111). Per row: tick; Donkey Kong's status, `anim_frame`, x, y. The
/// cargo jumpsquat (240) and the rise (241) pass an animation speed of 0
/// and do not play: `anim_frame` holds 0 (`ftDonkeyThrowFKneeBendSetStatus`,
/// `ftDonkeyThrowFJumpSetStatus`).
const DONKEY_CARGO_JUMP: &str = "
1 10 2.00 -210.00 1399.25
2 10 3.00 -210.00 1399.25
3 10 4.00 -210.00 1399.25
4 10 5.00 -210.00 1399.25
5 10 6.00 -210.00 1399.25
6 10 7.00 -210.00 1399.25
7 10 8.00 -210.00 1399.25
8 10 9.00 -210.00 1399.25
9 10 10.00 -210.00 1399.25
10 15 1.00 -150.00 1399.16
11 15 2.00 -90.00 1399.06
12 15 3.00 -30.00 1398.97
13 15 4.00 30.00 1398.88
14 15 5.00 90.00 1398.78
15 15 6.00 150.00 1398.69
16 15 7.00 206.50 1398.60
17 15 8.00 259.50 1398.52
18 15 9.00 309.00 1398.44
19 15 10.00 355.00 1398.37
20 15 11.00 397.50 1398.31
21 15 12.00 436.50 1398.25
22 15 13.00 472.00 1398.19
23 15 14.00 504.00 1398.14
24 15 15.00 525.75 1398.11
25 15 16.00 544.00 1398.08
26 15 17.00 558.75 1398.05
27 15 18.00 570.00 1398.04
28 15 19.00 577.75 1398.03
29 15 20.00 582.00 1398.02
30 15 21.00 582.75 1398.02
31 15 22.00 580.00 1398.02
32 15 23.00 573.75 1398.03
33 15 24.00 567.00 1398.04
34 15 25.00 560.25 1398.05
35 15 26.00 553.50 1398.06
36 15 27.00 553.50 1398.06
37 15 28.00 553.50 1398.06
38 15 29.00 553.50 1398.06
39 15 30.00 553.50 1398.06
40 10 0.00 553.50 1398.06
41 10 1.00 553.50 1398.06
42 10 2.00 553.50 1398.06
43 10 3.00 553.50 1398.06
44 10 4.00 553.50 1398.06
45 10 5.00 553.50 1398.06
46 10 6.00 553.50 1398.06
47 10 7.00 553.50 1398.06
48 10 8.00 553.50 1398.06
49 10 9.00 553.50 1398.06
50 166 1.00 553.50 1398.06
51 166 2.00 553.50 1398.06
52 166 3.00 553.50 1398.06
53 166 4.00 553.50 1398.06
54 166 5.00 553.50 1398.06
55 167 0.00 553.50 1398.06
56 167 1.00 553.50 1398.06
57 168 0.00 553.50 1398.06
58 168 0.00 553.50 1398.06
59 168 0.00 553.50 1398.06
60 169 1.00 553.50 1398.06
61 169 2.00 553.50 1398.06
62 169 3.00 553.50 1398.06
63 169 4.00 553.50 1398.06
64 169 5.00 553.50 1398.06
65 169 6.00 553.50 1398.06
66 169 7.00 553.50 1398.06
67 169 8.00 553.50 1398.06
68 169 9.00 553.50 1398.06
69 169 10.00 553.50 1398.06
70 169 11.00 553.50 1398.06
71 169 12.00 553.50 1398.06
72 169 13.00 553.50 1398.06
73 169 14.00 553.50 1398.06
74 169 15.00 553.50 1398.06
75 169 16.00 553.50 1398.06
76 169 17.00 553.50 1398.06
77 169 18.00 553.50 1398.06
78 169 19.00 553.50 1398.06
79 235 0.00 553.50 1398.06
80 235 1.00 553.50 1398.06
81 235 2.00 553.50 1398.06
82 235 3.00 553.50 1398.06
83 235 4.00 553.50 1398.06
84 235 5.00 553.50 1398.06
85 235 6.00 553.50 1398.06
86 235 7.00 553.50 1398.06
87 235 8.00 553.50 1398.06
88 235 9.00 553.50 1398.06
89 235 10.00 553.50 1398.06
90 235 11.00 553.50 1398.06
91 235 12.00 553.50 1398.06
92 235 13.00 553.50 1398.06
93 235 14.00 553.50 1398.06
94 235 15.00 553.50 1398.06
95 235 16.00 553.50 1398.06
96 235 17.00 553.50 1398.06
97 235 18.00 553.50 1398.06
98 235 19.00 553.50 1398.06
99 235 20.00 553.50 1398.06
100 240 0.00 553.50 1398.06
101 240 0.00 553.50 1398.06
102 240 0.00 553.50 1398.06
103 240 0.00 553.50 1398.06
104 240 0.00 553.50 1398.06
105 240 0.00 553.50 1398.06
106 241 0.00 553.50 1471.21
107 241 0.00 553.50 1541.36
108 241 0.00 553.50 1608.51
109 241 0.00 553.50 1672.66
110 241 0.00 553.50 1733.81
111 241 0.00 553.50 1791.96
112 245 1.00 553.50 1847.11
113 245 2.00 553.50 1899.26
114 245 3.00 553.50 1948.41
115 245 4.00 553.50 1994.56
116 245 5.00 553.50 2037.71
117 245 6.00 553.50 2077.86
118 245 7.00 553.50 2115.01
119 245 8.00 553.50 2149.16
120 245 9.00 553.50 2180.31
121 245 10.00 553.50 2208.46
122 245 11.00 553.50 2233.61
123 245 12.00 553.50 2255.76
124 245 13.00 553.50 2274.91
125 245 14.00 553.50 2291.06
126 245 15.00 553.50 2304.21
127 245 16.00 553.50 2314.36
128 245 17.00 553.50 2321.51
129 245 18.00 553.50 2325.66
130 245 19.00 553.50 2326.81
131 245 20.00 553.50 2324.96
132 245 21.00 553.50 2320.11
133 245 22.00 553.50 2312.26
134 245 23.00 553.50 2301.41
135 245 24.00 553.50 2287.56
136 245 25.00 553.50 2270.71
137 245 26.00 553.50 2250.86
138 245 27.00 553.50 2228.01
139 245 28.00 553.50 2202.16
140 245 29.00 553.50 2173.31
";

/// Replays `kind` as P1 against the Training dummy Mario on the Castle
/// with `pad` up to the last row of `table`, and compares P1's status id,
/// `anim_frame`, x and y with each `t st af x y` row from tick `from` on.
/// A row with two more values also holds the x and y of P1's weapon
/// (`weapon` reads it from the pool); a row without them, no weapon.
/// Returns the first bad tick, the log and the largest position error.
fn replay_status_rows(
    kind: FighterKind,
    pad: &dyn Fn(u64) -> (u16, i8, i8),
    weapon: &dyn Fn(&ssb_game::weapon::WeaponPool) -> Option<[f32; 2]>,
    table: &str,
    from: u64,
) -> Option<(Option<u64>, String, f32)> {
    let bytes = open_pack()?;
    let pack = ssb_rom::pack::Pack::open(&bytes).unwrap();
    let (stage, segments) = stage_floors(&pack, CASTLE);
    let map = dream_land_map(&pack, &stage);
    let mut fighters: Vec<Posed> = [(kind, 0), (FighterKind::Mario, 1)]
        .into_iter()
        .map(|(kind, port)| {
            Posed::new(
                &pack,
                training_fighter(&pack, &stage, kind, port, &segments),
            )
        })
        .collect();
    let want: Vec<Vec<f32>> = rows(table).collect();
    let last = want.last().unwrap()[0] as u64;
    let mut pair = Pair::default();
    let mut log = Vec::new();
    let mut first_bad = None;
    let mut max_err = 0.0f32;
    for tick in 1..=last {
        let _ = tick_pair(&pack, &map, &mut fighters, pad, tick, &mut pair);
        let f = &fighters[0].fighter;
        let mut got = vec![
            f32::from(f.status.status.id()),
            f.status.anim_frame,
            f.pos.x,
            f.pos.y,
        ];
        got.extend(weapon(&pair.weapons).into_iter().flatten());
        let Some(row) = want.iter().find(|r| r[0] as u64 == tick && tick >= from) else {
            continue;
        };
        let ok = got.len() + 1 == row.len() && got.iter().zip(&row[1..]).all(|(g, w)| near(*g, *w));
        max_err = got[2..]
            .iter()
            .zip(&row[3..])
            .fold(max_err, |m, (g, w)| m.max((g - w).abs()));
        log.push(format!(
            "{tick} {} rust {got:?}\n{tick} {}  n64 {:?}",
            if ok { " " } else { "X" },
            if ok { " " } else { "X" },
            &row[1..]
        ));
        if !ok && first_bad.is_none() {
            first_bad = Some(tick);
        }
    }
    Some((first_bad, log.join("\n"), max_err))
}

#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn donkey_cargo_jump_holds_anim_frame_as_on_the_n64() {
    // `R_TRIG`, the grab.
    const R: u16 = 0x0010;
    let pad = |t: u64| match t {
        10..=17 => (0, 127, 0),
        50..=51 => (R, 0, 0),
        60..=61 => (0, 80, 0),
        100 => (button::C_UP, 0, 0),
        112..=113 => (button::A, 0, 0),
        _ => (0, 0, 0),
    };
    // The opening Wait's `anim_frame` runs one ahead on the N64 (the
    // replay starts on the Training scene's first fighter frame, not its
    // entry); the rows compare from the walk on.
    let Some((first_bad, log, max_err)) =
        replay_status_rows(FighterKind::Donkey, &pad, &|_| None, DONKEY_CARGO_JUMP, 10)
    else {
        return;
    };
    if std::env::var_os("SCENE_TRACE").is_some() {
        println!("donkey cargo jump on the Castle (max position error {max_err}):\n{log}");
    }
    assert_eq!(first_bad, None, "{log}");
}

/// `b30set/linkG649` (`warp_link`): route `500:1:c_u;515:1:b;649:1:c_u`,
/// N64 frames 500-700. Link throws the Boomerang in the air (232), lands
/// into `SpecialN` (229) and jumps as it comes back: it reaches him on the
/// frame his jumpsquat leaves the ground, so `ftLinkSpecialNGetSetStatus`
/// takes the air branch, `SpecialAirNReturn` (233), at `anim_frame` 1
/// (the setter plays). The return keeps the jump's velocity and ends into
/// Fall after 19 frames. Per row: tick, Link's status, `anim_frame`, x, y and,
/// while it flies, the Boomerang's x and y.
const LINK_AIR_RETURN: &str = "
500 20 0.00 -210.00 1399.25
501 20 1.00 -210.00 1399.25
502 20 2.00 -210.00 1399.25
503 20 3.00 -210.00 1399.25
504 20 4.00 -210.00 1399.25
505 20 5.00 -210.00 1399.25
506 20 6.00 -210.00 1399.25
507 22 0.00 -210.00 1463.35
508 22 1.00 -210.00 1524.05
509 22 2.00 -210.00 1581.35
510 22 3.00 -210.00 1635.25
511 22 4.00 -210.00 1685.75
512 22 5.00 -210.00 1732.85
513 22 6.00 -210.00 1776.55
514 22 7.00 -210.00 1816.85
515 232 1.00 -210.00 1853.75
516 232 2.00 -210.00 1887.25
517 232 3.00 -210.00 1917.35
518 232 4.00 -210.00 1944.05
519 232 5.00 -210.00 1967.35
520 232 6.00 -210.00 1987.25
521 232 7.00 -210.00 2003.75
522 232 8.00 -210.00 2016.85
523 232 9.00 -210.00 2026.55
524 232 10.00 -210.00 2032.85
525 232 11.00 -210.00 2035.75
526 232 12.00 -210.00 2035.25
527 232 13.00 -210.00 2031.35
528 232 14.00 -210.00 2024.05
529 232 15.00 -210.00 2013.35
530 232 16.00 -210.00 1999.25
531 232 17.00 -210.00 1981.75
532 232 18.00 -210.00 1960.85
533 232 19.00 -210.00 1936.55
534 232 20.00 -210.00 1908.85
535 232 21.00 -210.00 1877.75
536 232 22.00 -210.00 1843.25
537 232 23.00 -210.00 1805.35
538 232 24.00 -210.00 1764.05
539 232 25.00 -210.00 1719.35
540 232 26.00 -210.00 1671.25 23.60 2009.35
541 232 27.00 -210.00 1619.75 105.80 2009.35
542 232 28.00 -210.00 1564.85 186.60 2009.35
543 232 29.00 -210.00 1506.55 266.00 2009.35
544 232 30.00 -210.00 1444.85 344.00 2009.35
545 229 31.00 -210.00 1399.25 420.60 2009.35
546 229 32.00 -210.00 1399.25 495.80 2009.35
547 229 33.00 -210.00 1399.25 569.60 2009.35
548 229 34.00 -210.00 1399.25 642.00 2009.35
549 229 35.00 -210.00 1399.25 713.00 2009.35
550 229 36.00 -210.00 1399.25 782.60 2009.35
551 229 37.00 -210.00 1399.25 850.80 2009.35
552 229 38.00 -210.00 1399.25 917.60 2009.35
553 229 39.00 -210.00 1399.25 983.00 2009.35
554 229 40.00 -210.00 1399.25 1047.00 2009.35
555 229 41.00 -210.00 1399.25 1109.60 2009.35
556 229 42.00 -210.00 1399.25 1170.80 2009.35
557 229 43.00 -210.00 1399.25 1230.60 2009.35
558 229 44.00 -210.00 1399.25 1289.00 2009.35
559 229 45.00 -210.00 1399.25 1346.00 2009.35
560 10 0.00 -210.00 1399.25 1401.60 2009.35
561 10 1.00 -210.00 1399.25 1455.80 2009.35
562 10 2.00 -210.00 1399.25 1508.60 2009.35
563 10 3.00 -210.00 1399.25 1560.00 2009.35
564 10 4.00 -210.00 1399.25 1610.00 2009.35
565 10 5.00 -210.00 1399.25 1658.60 2009.35
566 10 6.00 -210.00 1399.25 1705.80 2009.35
567 10 7.00 -210.00 1399.25 1751.60 2009.35
568 10 8.00 -210.00 1399.25 1796.00 2009.35
569 10 9.00 -210.00 1399.25 1839.00 2009.35
570 10 10.00 -210.00 1399.25 1880.60 2009.35
571 10 11.00 -210.00 1399.25 1920.80 2009.35
572 10 12.00 -210.00 1399.25 1959.60 2009.35
573 10 13.00 -210.00 1399.25 1997.00 2009.35
574 10 14.00 -210.00 1399.25 2033.00 2009.35
575 10 15.00 -210.00 1399.25 2067.60 2009.35
576 10 16.00 -210.00 1399.25 2100.80 2009.35
577 10 17.00 -210.00 1399.25 2132.60 2009.35
578 10 18.00 -210.00 1399.25 2163.00 2009.35
579 10 19.00 -210.00 1399.25 2192.00 2009.35
580 10 20.00 -210.00 1399.25 2219.60 2009.35
581 10 21.00 -210.00 1399.25 2245.80 2009.35
582 10 22.00 -210.00 1399.25 2270.60 2009.35
583 10 23.00 -210.00 1399.25 2294.00 2009.35
584 10 24.00 -210.00 1399.25 2316.00 2009.35
585 10 25.00 -210.00 1399.25 2336.60 2009.35
586 10 26.00 -210.00 1399.25 2355.80 2009.35
587 10 27.00 -210.00 1399.25 2373.60 2009.35
588 10 28.00 -210.00 1399.25 2390.00 2009.35
589 10 29.00 -210.00 1399.25 2405.00 2009.35
590 10 30.00 -210.00 1399.25 2418.60 2009.35
591 10 31.00 -210.00 1399.25 2430.80 2009.35
592 10 32.00 -210.00 1399.25 2441.60 2009.35
593 10 33.00 -210.00 1399.25 2451.60 2009.35
594 10 34.00 -210.00 1399.25 2440.60 2009.35
595 10 35.00 -210.00 1399.25 2428.60 2009.19
596 10 36.00 -210.00 1399.25 2415.60 2008.85
597 10 37.00 -210.00 1399.25 2401.61 2008.30
598 10 38.00 -210.00 1399.25 2386.63 2007.52
599 10 39.00 -210.00 1399.25 2370.67 2006.47
600 10 40.00 -210.00 1399.25 2353.72 2005.14
601 10 41.00 -210.00 1399.25 2335.80 2003.49
602 10 42.00 -210.00 1399.25 2316.90 2001.51
603 10 43.00 -210.00 1399.25 2297.04 1999.15
604 10 44.00 -210.00 1399.25 2276.20 1996.58
605 10 45.00 -210.00 1399.25 2254.36 1993.88
606 10 46.00 -210.00 1399.25 2231.54 1991.06
607 10 47.00 -210.00 1399.25 2207.72 1988.11
608 10 48.00 -210.00 1399.25 2182.91 1985.05
609 10 49.00 -210.00 1399.25 2157.10 1981.86
610 10 50.00 -210.00 1399.25 2130.31 1978.55
611 10 51.00 -210.00 1399.25 2102.52 1975.11
612 10 52.00 -210.00 1399.25 2073.74 1971.55
613 10 53.00 -210.00 1399.25 2043.97 1967.87
614 10 54.00 -210.00 1399.25 2013.20 1964.07
615 10 55.00 -210.00 1399.25 1981.44 1960.14
616 10 56.00 -210.00 1399.25 1948.69 1956.09
617 10 57.00 -210.00 1399.25 1914.95 1951.92
618 10 58.00 -210.00 1399.25 1880.21 1947.63
619 10 59.00 -210.00 1399.25 1844.48 1943.21
620 10 60.00 -210.00 1399.25 1807.76 1938.67
621 10 61.00 -210.00 1399.25 1770.05 1934.01
622 10 62.00 -210.00 1399.25 1731.35 1929.23
623 10 63.00 -210.00 1399.25 1691.65 1924.32
624 10 64.00 -210.00 1399.25 1650.96 1919.29
625 10 65.00 -210.00 1399.25 1609.27 1914.14
626 10 66.00 -210.00 1399.25 1566.60 1908.86
627 10 67.00 -210.00 1399.25 1522.93 1903.46
628 10 68.00 -210.00 1399.25 1478.27 1897.94
629 10 69.00 -210.00 1399.25 1432.62 1892.30
630 10 70.00 -210.00 1399.25 1385.97 1886.54
631 10 71.00 -210.00 1399.25 1338.34 1880.65
632 10 72.00 -210.00 1399.25 1289.71 1874.64
633 10 73.00 -210.00 1399.25 1240.08 1868.50
634 10 74.00 -210.00 1399.25 1189.47 1862.24
635 10 75.00 -210.00 1399.25 1137.86 1855.87
636 10 76.00 -210.00 1399.25 1085.26 1849.36
637 10 77.00 -210.00 1399.25 1031.67 1842.74
638 10 78.00 -210.00 1399.25 977.09 1835.99
639 10 79.00 -210.00 1399.25 921.51 1829.12
640 10 -0.00 -210.00 1399.25 864.94 1822.13
641 10 1.00 -210.00 1399.25 807.38 1815.01
642 10 2.00 -210.00 1399.25 748.82 1807.77
643 10 3.00 -210.00 1399.25 689.28 1800.41
644 10 4.00 -210.00 1399.25 628.74 1792.93
645 10 5.00 -210.00 1399.25 567.21 1785.32
646 10 6.00 -210.00 1399.25 504.68 1777.60
647 10 7.00 -210.00 1399.25 441.17 1769.74
648 10 8.00 -210.00 1399.25 376.66 1761.77
649 20 0.00 -210.00 1399.25 311.15 1753.67
650 20 1.00 -210.00 1399.25 244.66 1745.45
651 20 2.00 -210.00 1399.25 177.17 1737.11
652 20 3.00 -210.00 1399.25 108.70 1728.65
653 20 4.00 -210.00 1399.25 39.22 1720.06
654 20 5.00 -210.00 1399.25 -31.24 1711.35
655 20 6.00 -210.00 1399.25 -102.69 1702.52
656 233 1.00 -210.00 1463.35
657 233 2.00 -210.00 1524.05
658 233 3.00 -210.00 1581.35
659 233 4.00 -210.00 1635.25
660 233 5.00 -210.00 1685.75
661 233 6.00 -210.00 1732.85
662 233 7.00 -210.00 1776.55
663 233 8.00 -210.00 1816.85
664 233 9.00 -210.00 1853.75
665 233 10.00 -210.00 1887.25
666 233 11.00 -210.00 1917.35
667 233 12.00 -210.00 1944.05
668 233 13.00 -210.00 1967.35
669 233 14.00 -210.00 1987.25
670 233 15.00 -210.00 2003.75
671 233 16.00 -210.00 2016.85
672 233 17.00 -210.00 2026.55
673 233 18.00 -210.00 2032.85
674 233 19.00 -210.00 2035.75
675 26 0.00 -210.00 2035.25
676 26 1.00 -210.00 2031.35
677 26 2.00 -210.00 2024.05
678 26 3.00 -210.00 2013.35
679 26 4.00 -210.00 1999.25
680 26 5.00 -210.00 1981.75
681 26 -0.00 -210.00 1960.85
682 26 1.00 -210.00 1936.55
683 26 2.00 -210.00 1908.85
684 26 3.00 -210.00 1877.75
685 26 4.00 -210.00 1843.25
686 26 5.00 -210.00 1805.35
687 26 -0.00 -210.00 1764.05
688 26 1.00 -210.00 1719.35
689 26 2.00 -210.00 1671.25
690 26 3.00 -210.00 1619.75
691 26 4.00 -210.00 1564.85
692 26 5.00 -210.00 1506.55
693 26 -0.00 -210.00 1444.85
694 31 0.00 -210.00 1399.25
695 31 1.00 -210.00 1399.25
696 31 2.00 -210.00 1399.25
697 31 3.00 -210.00 1399.25
698 31 4.00 -210.00 1399.25
699 31 5.00 -210.00 1399.25
700 31 6.00 -210.00 1399.25
";

#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn link_catches_the_boomerang_in_the_air_as_on_the_n64() {
    let pad = |t: u64| match t {
        500 | 649 => (button::C_UP, 0, 0),
        515 => (button::B, 0, 0),
        _ => (0, 0, 0),
    };
    let Some((first_bad, log, max_err)) = replay_status_rows(
        FighterKind::Link,
        &pad,
        &|w| w.boomerangs().next().map(|b| [b.position.x, b.position.y]),
        LINK_AIR_RETURN,
        500,
    ) else {
        return;
    };
    if std::env::var_os("SCENE_TRACE").is_some() {
        println!("link air boomerang return on the Castle (max position error {max_err}):\n{log}");
    }
    assert_eq!(first_bad, None, "{log}");
}

/// `b30set/yoshiF3_242` (`warp_yoshi`): route `200:18:x=127;219:1:c_u;
/// 242:1:b`, N64 frames 200-300. Yoshi dashes at the dummy, jumps and
/// tongues it on the way down: `SpecialAirN` (231), `SpecialAirNCatch`
/// (232), then `ftYoshiSpecialAirNReleaseSetStatus` plays, so
/// `SpecialAirNRelease` (233) starts at `anim_frame` 1. He lands in the
/// release: `ftYoshiSpecialAirNSwitchStatusGround` sets `SpecialNRelease`
/// (230) at the figatree's frame without a play (8 -> 9 is the frame's own
/// play). Per row: tick, Yoshi's status, `anim_frame`, x, y.
const YOSHI_AIR_RELEASE: &str = "
200 15 1.00 -170.00 1399.19
201 15 2.00 -130.00 1399.13
202 15 3.00 -90.00 1399.06
203 15 4.00 -50.00 1399.00
204 15 5.00 -10.00 1398.94
205 15 6.00 30.00 1398.88
206 15 7.00 68.50 1398.82
207 15 8.00 105.50 1398.76
208 15 9.00 141.00 1398.71
209 15 10.00 175.00 1398.65
210 15 11.00 207.50 1398.60
211 15 12.00 238.50 1398.55
212 15 13.00 268.00 1398.51
213 15 14.00 296.00 1398.46
214 15 15.00 322.50 1398.42
215 16 0.00 372.50 1398.34
216 16 1.00 422.50 1398.27
217 16 2.00 465.75 1398.20
218 17 0.00 507.00 1398.14
219 20 0.00 541.85 1398.08
220 20 1.00 570.30 1398.04
221 20 2.00 592.35 1398.00
222 20 3.00 608.00 1399.24
223 20 4.00 617.19 1400.05
224 22 0.00 617.19 1456.25
225 22 1.00 617.19 1509.65
226 22 2.00 617.19 1560.25
227 22 3.00 617.19 1608.05
228 22 4.00 617.19 1653.05
229 22 5.00 617.19 1695.25
230 22 6.00 617.19 1734.65
231 22 7.00 617.19 1771.25
232 22 8.00 617.19 1805.05
233 22 9.00 617.19 1836.05
234 22 10.00 617.19 1864.25
235 22 11.00 617.19 1889.65
236 22 12.00 617.19 1912.25
237 22 13.00 617.19 1932.05
238 22 14.00 617.19 1949.05
239 22 15.00 617.19 1963.25
240 22 16.00 617.19 1974.65
241 22 17.00 617.19 1983.25
242 231 1.00 617.19 1989.05
243 231 2.00 617.19 1992.05
244 231 3.00 617.19 1992.25
245 231 4.00 617.19 1989.65
246 231 5.00 617.19 1984.25
247 231 6.00 617.19 1976.05
248 231 7.00 617.19 1965.05
249 231 8.00 617.19 1951.25
250 231 9.00 617.19 1934.65
251 231 10.00 617.19 1915.25
252 231 11.00 617.19 1893.05
253 231 12.00 617.19 1868.05
254 231 13.00 617.19 1840.25
255 231 14.00 617.19 1809.65
256 231 15.00 617.19 1776.25
257 231 16.00 617.19 1740.05
258 231 17.00 617.19 1701.05
259 232 19.00 617.19 1659.25
260 232 20.00 617.19 1656.45
261 232 21.00 617.19 1650.85
262 232 22.00 617.19 1642.45
263 232 23.00 617.19 1631.25
264 232 24.00 617.19 1617.25
265 233 1.00 617.19 1600.45
266 233 2.00 617.19 1580.85
267 233 3.00 617.19 1558.45
268 233 4.00 617.19 1533.25
269 233 5.00 617.19 1505.25
270 233 6.00 617.19 1474.45
271 233 7.00 617.19 1440.85
272 233 8.00 617.19 1404.45
273 230 9.00 617.19 1400.05
274 230 10.00 617.19 1400.05
275 230 11.00 617.19 1400.05
276 230 12.00 617.19 1400.05
277 230 13.00 617.19 1400.05
278 230 14.00 617.19 1400.05
279 230 15.00 617.19 1400.05
280 230 16.00 617.19 1400.05
281 230 17.00 617.19 1400.05
282 230 18.00 617.19 1400.05
283 230 19.00 617.19 1400.05
284 230 20.00 617.19 1400.05
285 230 21.00 617.19 1400.05
286 230 22.00 617.19 1400.05
287 230 23.00 617.19 1400.05
288 230 24.00 617.19 1400.05
289 230 25.00 617.19 1400.05
290 230 26.00 617.19 1400.05
291 230 27.00 617.19 1400.05
292 230 28.00 617.19 1400.05
293 230 29.00 617.19 1400.05
294 230 30.00 617.19 1400.05
295 230 31.00 617.19 1400.05
296 230 32.00 617.19 1400.05
297 230 33.00 617.19 1400.05
298 230 34.00 617.19 1400.05
299 10 0.00 617.19 1400.05
300 10 1.00 617.19 1400.05
";

#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn yoshi_releases_in_the_air_and_lands_as_on_the_n64() {
    let pad = |t: u64| match t {
        200..=217 => (0, 127, 0),
        219 => (button::C_UP, 0, 0),
        242 => (button::B, 0, 0),
        _ => (0, 0, 0),
    };
    let Some((first_bad, log, max_err)) =
        replay_status_rows(FighterKind::Yoshi, &pad, &|_| None, YOSHI_AIR_RELEASE, 200)
    else {
        return;
    };
    if std::env::var_os("SCENE_TRACE").is_some() {
        println!("yoshi air release on the Castle (max position error {max_err}):\n{log}");
    }
    assert_eq!(first_bad, None, "{log}");
}

/// `b30set/kirbyE1` (`warp_kirby`): route `200:2:y=-128;300:1:b+y=80`,
/// N64 frames 195-390. Kirby drops through the castle roof (Pass, 33) to
/// the floor below at y 510, then Final Cutters up through the roof: he
/// meets it still in `SpecialHi` (256) with `vel_air.y` < 0, so
/// `ftKirbySpecialHiProcMap` lands him into `SpecialHiLanding` (257),
/// whose setter plays: `anim_frame` 1 (RE-473 traced the landing from
/// `SpecialAirHiFall`, which starts at 0); its flag spawns the wave two
/// frames later. Per row: tick, Kirby's status, `anim_frame`, x, y and the
/// wave's x and y while it flies; the opening Wait's `anim_frame` runs one ahead
/// on the N64, so the rows compare from the drop on.
const KIRBY_RISE_LANDING: &str = "
195 10 36.00 -210.00 1399.25
196 10 37.00 -210.00 1399.25
197 10 38.00 -210.00 1399.25
198 10 39.00 -210.00 1399.25
199 10 40.00 -210.00 1399.25
200 28 1.00 -210.00 1399.25
201 28 2.00 -210.00 1399.25
202 28 3.00 -210.00 1399.25
203 33 1.00 -210.00 1396.85
204 33 2.00 -210.00 1392.05
205 33 3.00 -210.00 1384.85
206 33 4.00 -210.00 1375.25
207 33 5.00 -210.00 1363.25
208 33 6.00 -210.00 1348.85
209 33 7.00 -210.00 1332.05
210 33 8.00 -210.00 1312.85
211 33 9.00 -210.00 1291.25
212 33 10.00 -210.00 1267.25
213 33 11.00 -210.00 1240.85
214 33 12.00 -210.00 1212.05
215 33 13.00 -210.00 1180.85
216 33 14.00 -210.00 1147.25
217 33 15.00 -210.00 1111.25
218 33 16.00 -210.00 1072.85
219 33 17.00 -210.00 1032.05
220 33 18.00 -210.00 988.85
221 33 19.00 -210.00 943.25
222 33 20.00 -210.00 895.25
223 33 21.00 -210.00 847.25
224 33 22.00 -210.00 799.25
225 33 23.00 -210.00 751.25
226 33 24.00 -210.00 703.25
227 26 0.00 -210.00 655.25
228 26 1.00 -210.00 607.25
229 26 2.00 -210.00 559.25
230 26 3.00 -210.00 511.25
231 31 0.00 -210.00 510.00
232 31 1.00 -210.00 510.00
233 31 2.00 -210.00 510.00
234 31 3.00 -210.00 510.00
235 31 4.00 -210.00 510.00
236 31 5.00 -210.00 510.00
237 31 6.00 -210.00 510.00
238 31 7.00 -210.00 510.00
239 10 0.00 -210.00 510.00
240 10 1.00 -210.00 510.00
241 10 2.00 -210.00 510.00
242 10 3.00 -210.00 510.00
243 10 4.00 -210.00 510.00
244 10 5.00 -210.00 510.00
245 10 6.00 -210.00 510.00
246 10 7.00 -210.00 510.00
247 10 8.00 -210.00 510.00
248 10 9.00 -210.00 510.00
249 10 10.00 -210.00 510.00
250 10 11.00 -210.00 510.00
251 10 12.00 -210.00 510.00
252 10 13.00 -210.00 510.00
253 10 14.00 -210.00 510.00
254 10 15.00 -210.00 510.00
255 10 16.00 -210.00 510.00
256 10 17.00 -210.00 510.00
257 10 18.00 -210.00 510.00
258 10 19.00 -210.00 510.00
259 10 20.00 -210.00 510.00
260 10 21.00 -210.00 510.00
261 10 22.00 -210.00 510.00
262 10 23.00 -210.00 510.00
263 10 24.00 -210.00 510.00
264 10 25.00 -210.00 510.00
265 10 26.00 -210.00 510.00
266 10 27.00 -210.00 510.00
267 10 28.00 -210.00 510.00
268 10 29.00 -210.00 510.00
269 10 30.00 -210.00 510.00
270 10 31.00 -210.00 510.00
271 10 32.00 -210.00 510.00
272 10 33.00 -210.00 510.00
273 10 34.00 -210.00 510.00
274 10 35.00 -210.00 510.00
275 10 36.00 -210.00 510.00
276 10 37.00 -210.00 510.00
277 10 38.00 -210.00 510.00
278 10 39.00 -210.00 510.00
279 10 40.00 -210.00 510.00
280 10 41.00 -210.00 510.00
281 10 42.00 -210.00 510.00
282 10 43.00 -210.00 510.00
283 10 44.00 -210.00 510.00
284 10 45.00 -210.00 510.00
285 10 46.00 -210.00 510.00
286 10 47.00 -210.00 510.00
287 10 48.00 -210.00 510.00
288 10 49.00 -210.00 510.00
289 10 50.00 -210.00 510.00
290 10 51.00 -210.00 510.00
291 10 52.00 -210.00 510.00
292 10 53.00 -210.00 510.00
293 10 54.00 -210.00 510.00
294 10 55.00 -210.00 510.00
295 10 56.00 -210.00 510.00
296 10 57.00 -210.00 510.00
297 10 58.00 -210.00 510.00
298 10 59.00 -210.00 510.00
299 10 60.00 -210.00 510.00
300 256 1.00 -210.00 510.00
301 256 2.00 -210.00 510.00
302 256 3.00 -210.00 510.00
303 256 4.00 -210.00 510.00
304 256 5.00 -210.00 510.00
305 256 6.00 -210.00 510.00
306 256 7.00 -210.00 510.00
307 256 8.00 -210.00 510.00
308 256 9.00 -210.00 510.00
309 256 10.00 -210.00 510.00
310 256 11.00 -210.00 510.00
311 256 12.00 -210.00 510.00
312 256 13.00 -210.00 510.00
313 256 14.00 -210.00 510.00
314 256 15.00 -210.00 510.00
315 256 16.00 -210.00 510.00
316 256 17.00 -210.00 510.00
317 256 18.00 -210.00 510.00
318 256 19.00 -210.00 510.00
319 256 20.00 -210.00 510.00
320 256 21.00 -210.00 510.00
321 256 22.00 -210.00 510.00
322 256 23.00 -210.00 544.12
323 256 24.00 -210.00 646.50
324 256 25.00 -210.00 789.71
325 256 26.00 -210.00 943.98
326 256 27.00 -210.00 1105.73
327 256 28.00 -210.00 1271.40
328 256 29.00 -210.00 1437.42
329 256 30.00 -210.00 1600.23
330 256 31.00 -210.00 1756.25
331 256 32.00 -210.00 1901.94
332 256 33.00 -210.00 2033.71
333 256 34.00 -210.00 2148.00
334 256 35.00 -210.00 2239.80
335 256 36.00 -210.00 2308.62
336 256 37.00 -210.00 2357.69
337 256 38.00 -210.00 2390.23
338 256 39.00 -210.00 2409.48
339 256 40.00 -210.00 2418.66
340 256 41.00 -210.00 2421.00
341 256 42.00 -210.00 2420.27
342 256 43.00 -210.00 2416.05
343 256 44.00 -210.00 2405.32
344 256 45.00 -210.00 2385.06
345 256 46.00 -210.00 2352.22
346 256 47.00 -210.00 2303.78
347 256 48.00 -210.00 2236.72
348 256 49.00 -210.00 2148.00
349 256 50.00 -210.00 2041.04
350 256 51.00 -210.00 1922.06
351 256 52.00 -210.00 1792.28
352 256 53.00 -210.00 1652.90
353 256 54.00 -210.00 1505.13
354 257 1.00 -210.00 1399.25
355 257 2.00 -210.00 1399.25
356 257 3.00 -210.00 1399.25 90.00 1618.78
357 257 4.00 -210.00 1399.25 190.00 1618.63
358 257 5.00 -210.00 1399.25 290.00 1618.47
359 257 6.00 -210.00 1399.25 390.00 1618.32
360 257 7.00 -210.00 1399.25 490.00 1618.16
361 257 8.00 -210.00 1399.25 590.00 1618.01
362 257 9.00 -210.00 1399.25 690.00 1626.47
363 257 10.00 -210.00 1399.25 790.00 1635.29
364 257 11.00 -210.00 1399.25 890.00 1644.12
365 257 12.00 -210.00 1399.25 990.00 1652.94
366 257 13.00 -210.00 1399.25 1090.00 1661.76
367 257 14.00 -210.00 1399.25 1190.00 1659.88
368 257 15.00 -210.00 1399.25 1290.00 1648.12
369 257 16.00 -210.00 1399.25 1390.00 1621.79
370 257 17.00 -210.00 1399.25 1490.00 1579.70
371 257 18.00 -210.00 1399.25 1590.00 1537.61
372 257 19.00 -210.00 1399.25 1690.00 1495.51
373 257 20.00 -210.00 1399.25 1790.00 1453.42
374 257 21.00 -210.00 1399.25 1890.00 1445.98
375 257 22.00 -210.00 1399.25
376 257 23.00 -210.00 1399.25
377 257 24.00 -210.00 1399.25
378 257 25.00 -210.00 1399.25
379 257 26.00 -210.00 1399.25
380 257 27.00 -210.00 1399.25
381 257 28.00 -210.00 1399.25
382 257 29.00 -210.00 1399.25
383 257 30.00 -210.00 1399.25
384 257 31.00 -210.00 1399.25
385 257 32.00 -210.00 1399.25
386 257 33.00 -210.00 1399.25
387 257 34.00 -210.00 1399.25
388 10 0.00 -210.00 1399.25
389 10 1.00 -210.00 1399.25
390 10 2.00 -210.00 1399.25
";

#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn kirby_lands_final_cutter_from_the_rise_as_on_the_n64() {
    let pad = |t: u64| match t {
        200..=201 => (0, 0, -128),
        300 => (button::B, 0, 80),
        _ => (0, 0, 0),
    };
    let Some((first_bad, log, max_err)) = replay_status_rows(
        FighterKind::Kirby,
        &pad,
        &|w| w.cutters().next().map(|c| [c.position.x, c.position.y]),
        KIRBY_RISE_LANDING,
        200,
    ) else {
        return;
    };
    if std::env::var_os("SCENE_TRACE").is_some() {
        println!("kirby rise landing on the Castle (max position error {max_err}):\n{log}");
    }
    assert_eq!(first_bad, None, "{log}");
}

/// `b30set/hyr3` (`warp_hyrule_mario`, Training on Hyrule Castle, Mario P1
/// at x -2400, the dummy Mario at -1110): route `1690:200:x=-127`, N64
/// frames 1599-2000. The Twister wakes on frame 0 (`syUtilsRandIntRange`
/// from the program's seed 1: wait 1600), is made at frame 1600 at the
/// fourth Twister point (-3499, 423.93; line 5) and moves left from 1680.
/// Mario dashes left off the roof, lands on line 5 and walks into it: at
/// frame 1742 the Twister (-4119) is 291 away, so `ftCommonTwisterSetStatus`
/// takes him (60) at `anim_frame` 1, airborne, with his walk's position;
/// he spirals round the Twister (`ftCommonTwisterProcPhysics`), it turns at
/// the left edge (frame 1776), and he is shot at frame 1802 (`DamageFlyHi`,
/// 54), then tumbles down as `DamageFall` (57) from frame 1874, drifting
/// left under the held stick. Per row: tick, then Mario's status, `anim_frame`, x and y, then
/// the Twister's x and y while it exists; the rows before the walk hold the
/// Twister only (the opening Wait's `anim_frame` runs one ahead on the
/// N64).
const HYRULE_TWISTER: &str = "
1599
1600 -3499.00 423.93
1601 -3499.00 423.93
1679 -3499.00 423.93
1680 -3499.00 423.93
1681 -3509.00 421.51
1682 -3519.00 419.09
1731 13 6.00 -3571.47 406.40 -4009.00 300.57
1732 13 7.00 -3594.80 400.76 -4019.00 298.15
1733 13 8.00 -3618.12 395.12 -4029.00 295.73
1734 13 9.00 -3641.45 389.48 -4039.00 293.32
1735 13 10.00 -3664.78 383.83 -4049.00 290.90
1736 13 11.00 -3688.11 378.19 -4059.00 288.48
1737 13 12.00 -3711.43 372.55 -4069.00 286.06
1738 13 13.00 -3734.76 366.91 -4079.00 283.64
1739 13 14.00 -3758.09 361.26 -4089.00 281.22
1740 13 15.00 -3781.42 355.62 -4099.00 278.80
1741 13 16.00 -3804.74 349.98 -4109.00 276.38
1742 60 1.00 -3828.07 344.34 -4119.00 273.97
1743 60 2.00 -3876.30 332.67 -4129.00 271.55
1744 60 3.00 -3924.51 323.03 -4139.00 269.13
1745 60 4.00 -3973.05 316.26 -4149.00 266.71
1746 60 5.00 -4022.38 312.03 -4159.00 264.29
1747 60 6.00 -4072.33 309.96 -4169.00 261.87
1748 60 7.00 -4121.55 309.82 -4179.00 259.45
1749 60 8.00 -4167.05 311.74 -4189.00 257.03
1750 60 9.00 -4202.69 316.58 -4199.00 254.61
1751 60 10.00 -4209.00 327.20 -4209.00 252.20
1752 60 11.00 -4177.37 333.11 -4219.00 249.78
1753 60 12.00 -4153.99 339.03 -4229.00 247.36
1754 60 13.00 -4149.00 344.94 -4239.00 244.94
1755 60 14.00 -4167.92 350.76 -4249.00 242.52
1756 60 15.00 -4205.21 356.00 -4259.00 240.10
1757 60 16.00 -4252.43 360.95 -4269.00 237.68
1758 60 17.00 -4302.09 365.81 -4279.00 235.26
1759 60 18.00 -4346.95 370.74 -4289.00 232.85
1760 60 19.00 -4380.00 375.90 -4299.00 230.43
1761 60 20.00 -4394.44 381.46 -4309.00 228.01
1762 60 21.00 -4384.73 387.57 -4319.00 225.59
1763 60 22.00 -4349.90 394.20 -4329.00 223.17
1764 60 23.00 -4300.64 400.91 -4339.00 220.75
1765 60 24.00 -4259.38 407.03 -4349.00 218.33
1766 60 25.00 -4241.06 412.39 -4359.00 215.91
1767 60 26.00 -4247.44 417.19 -4369.00 213.49
1768 60 27.00 -4274.19 421.65 -4379.00 211.08
1769 60 28.00 -4315.22 425.94 -4389.00 208.66
1770 60 -0.00 -4363.83 430.21 -4399.00 206.24
1771 60 1.00 -4413.04 434.57 -4409.00 203.82
1772 60 2.00 -4455.87 439.20 -4419.00 201.40
1773 60 3.00 -4485.38 444.25 -4429.00 198.98
1774 60 4.00 -4494.94 449.94 -4439.00 196.56
1775 60 5.00 -4479.31 456.47 -4449.00 194.14
1776 60 6.00 -4439.02 463.82 -4459.00 191.73
1777 60 7.00 -4389.95 471.77 -4452.00 193.42
1778 60 8.00 -4352.36 479.31 -4452.00 193.42
1779 60 9.00 -4336.28 486.47 -4452.00 193.42
1780 60 10.00 -4344.35 493.41 -4452.00 193.42
1781 60 11.00 -4373.95 500.29 -4452.00 193.42
1782 60 12.00 -4418.39 507.22 -4452.00 193.42
1783 60 13.00 -4467.88 514.29 -4452.00 193.42
1784 60 14.00 -4511.19 521.53 -4452.00 193.42
1785 60 15.00 -4537.86 528.98 -4452.00 193.42
1786 60 16.00 -4540.90 536.62 -4452.00 193.42
1787 60 17.00 -4519.15 544.40 -4452.00 193.42
1788 60 18.00 -4478.12 552.27 -4452.00 193.42
1789 60 19.00 -4428.84 560.16 -4452.00 193.42
1790 60 20.00 -4384.69 568.02 -4452.00 193.42
1791 60 21.00 -4357.51 575.83 -4452.00 193.42
1792 60 22.00 -4354.40 583.59 -4452.00 193.42
1793 60 23.00 -4375.99 591.29 -4452.00 193.42
1794 60 24.00 -4416.61 598.97 -4452.00 193.42
1795 60 25.00 -4465.81 606.64 -4452.00 193.42
1796 60 26.00 -4510.95 614.32 -4452.00 193.42
1797 60 27.00 -4540.39 622.03 -4452.00 193.42
1798 60 28.00 -4546.45 629.76 -4452.00 193.42
1799 60 -0.00 -4527.51 637.51 -4452.00 193.42
1800 60 1.00 -4488.44 645.29 -4452.00 193.42
1801 60 2.00 -4439.52 653.08 -4452.00 193.42
1802 54 1.00 -4439.52 784.19 -4452.00 193.42
1803 54 2.00 -4439.52 911.20 -4452.00 193.42
1804 54 3.00 -4439.52 1034.11 -4452.00 193.42
1805 54 4.00 -4439.52 1152.91 -4452.00 193.42
1806 54 5.00 -4439.52 1267.62 -4452.00 193.42
1807 54 6.00 -4439.52 1378.23 -4452.00 193.42
1808 54 7.00 -4439.52 1484.74 -4452.00 193.42
1809 54 8.00 -4439.52 1587.15 -4452.00 193.42
1810 54 9.00 -4439.52 1685.45 -4452.00 193.42
1811 54 10.00 -4439.52 1779.66 -4452.00 193.42
1812 54 11.00 -4439.52 1869.77 -4452.00 193.42
1813 54 12.00 -4439.52 1955.78 -4452.00 193.42
1814 54 13.00 -4439.52 2037.69 -4452.00 193.42
1815 54 14.00 -4439.52 2115.49 -4452.00 193.42
1816 54 15.00 -4439.52 2189.20 -4452.00 193.42
1817 54 16.00 -4439.52 2258.81 -4452.00 193.42
1818 54 17.00 -4439.52 2324.32 -4452.00 193.42
1819 54 18.00 -4439.52 2385.73 -4452.00 193.42
1820 54 19.00 -4439.52 2444.63 -4452.00 193.42
1821 54 20.00 -4439.52 2501.84 -4452.00 193.42
1822 54 21.00 -4439.52 2557.35 -4452.00 193.42
1823 54 22.00 -4439.52 2611.16 -4452.00 193.42
1824 54 23.00 -4439.52 2663.27 -4452.00 193.42
1825 54 24.00 -4439.52 2713.67 -4452.00 193.42
1826 54 25.00 -4439.52 2762.38 -4452.00 193.42
1827 54 26.00 -4439.52 2809.39 -4452.00 193.42
1828 54 27.00 -4439.52 2854.70 -4452.00 193.42
1829 54 28.00 -4439.52 2898.31 -4452.00 193.42
1830 54 29.00 -4439.52 2940.21 -4452.00 193.42
1831 54 30.00 -4439.52 2980.42 -4452.00 193.42
1832 54 31.00 -4439.52 3018.93 -4452.00 193.42
1833 54 32.00 -4439.52 3055.74 -4452.00 193.42
1834 54 33.00 -4439.52 3090.85 -4452.00 193.42
1835 54 34.00 -4439.52 3124.26 -4452.00 193.42
1836 54 35.00 -4439.52 3155.96 -4452.00 193.42
1837 54 36.00 -4439.52 3185.97 -4452.00 193.42
1838 54 37.00 -4439.52 3214.28 -4452.00 193.42
1839 54 38.00 -4439.52 3240.89 -4452.00 193.42
1840 54 39.00 -4439.52 3265.80 -4452.00 193.42
1841 54 40.00 -4439.52 3289.00 -4452.00 193.42
1842 54 41.00 -4439.52 3310.51 -4452.00 193.42
1843 54 42.00 -4439.52 3330.32 -4452.00 193.42
1844 54 43.00 -4439.52 3348.43 -4452.00 193.42
1845 54 44.00 -4439.52 3364.84 -4452.00 193.42
1846 54 45.00 -4439.52 3379.54 -4452.00 193.42
1847 54 46.00 -4439.52 3392.55 -4452.00 193.42
1848 54 47.00 -4439.52 3403.86 -4452.00 193.42
1849 54 48.00 -4439.52 3413.47 -4452.00 193.42
1850 54 49.00 -4439.52 3421.38 -4452.00 193.42
1851 54 50.00 -4439.52 3427.58 -4452.00 193.42
1852 54 51.00 -4439.52 3432.09 -4452.00 193.42
1853 54 52.00 -4439.52 3434.90 -4452.00 193.42
1854 54 53.00 -4439.52 3436.01 -4452.00 193.42
1855 54 54.00 -4439.52 3435.42 -4452.00 193.42
1856 54 55.00 -4439.52 3433.13 -4452.00 193.42
1857 54 56.00 -4439.52 3429.13 -4452.00 193.42
1858 54 57.00 -4439.52 3423.44 -4452.00 193.42
1859 54 58.00 -4439.52 3416.05 -4452.00 193.42
1860 54 59.00 -4439.52 3406.96 -4452.00 193.42
1861 54 0.00 -4439.52 3396.17 -4452.00 193.42
1862 54 0.00 -4439.52 3383.67 -4452.00 193.42
1863 54 0.00 -4439.52 3369.48 -4452.00 193.42
1864 54 0.00 -4439.52 3353.59 -4452.00 193.42
1865 54 0.00 -4439.52 3336.00 -4452.00 193.42
1866 54 0.00 -4439.52 3316.71 -4452.00 193.42
1867 54 0.00 -4439.52 3295.71 -4452.00 193.42
1868 54 0.00 -4439.52 3273.02 -4452.00 193.42
1869 54 0.00 -4439.52 3248.63 -4452.00 193.42
1870 54 0.00 -4439.52 3222.54 -4452.00 193.42
1871 54 0.00 -4439.52 3194.75 -4452.00 193.42
1872 54 0.00 -4439.52 3165.26 -4452.00 193.42
1873 54 0.00 -4439.52 3134.06 -4452.00 193.42
1874 57 0.00 -4441.32 3101.17 -4452.00 193.42
1875 57 1.00 -4444.92 3066.58 -4452.00 193.42
1876 57 2.00 -4450.32 3030.29 -4452.00 193.42
1877 57 3.00 -4457.52 2992.30 -4452.00 193.42
1878 57 4.00 -4466.52 2952.60 -4452.00 193.42
1879 57 5.00 -4477.32 2911.21 -4452.00 193.42
1880 57 6.00 -4489.92 2868.12 -4452.00 193.42
1881 57 7.00 -4504.32 2824.12 -4452.00 193.42
1882 57 8.00 -4520.52 2780.12 -4452.00 193.42
1883 57 9.00 -4538.52 2736.12 -4452.00 193.42
1884 57 10.00 -4558.32 2692.12 -4452.00 193.42
1885 57 11.00 -4579.92 2648.12 -4452.00 193.42
1886 57 12.00 -4603.32 2604.12 -4452.00 193.42
1887 57 13.00 -4628.52 2560.12 -4452.00 193.42
1888 57 14.00 -4655.52 2516.12 -4452.00 193.42
1889 57 15.00 -4684.32 2472.12 -4452.00 193.42
1890 57 16.00 -4712.92 2428.12 -4452.00 193.42
1891 57 17.00 -4741.32 2384.12 -4452.00 193.42
1892 57 18.00 -4769.52 2340.12 -4452.00 193.42
1893 57 19.00 -4797.52 2296.12 -4452.00 193.42
1894 57 20.00 -4825.32 2252.12 -4452.00 193.42
1895 57 21.00 -4852.92 2208.12 -4452.00 193.42
1896 57 22.00 -4880.32 2164.12 -4452.00 193.42
1897 57 23.00 -4907.52 2120.12 -4452.00 193.42
1898 57 24.00 -4934.52 2076.12 -4442.00 195.84
1899 57 25.00 -4961.32 2032.12 -4432.00 198.26
1900 57 26.00 -4987.92 1988.12 -4422.00 200.68
1901 57 27.00 -5014.32 1944.12 -4412.00 203.09
1902 57 28.00 -5040.52 1900.12 -4402.00 205.51
1903 57 0.00 -5066.52 1856.12 -4392.00 207.93
1904 57 1.00 -5092.32 1812.12 -4382.00 210.35
1905 57 2.00 -5117.92 1768.12 -4372.00 212.77
1906 57 3.00 -5143.32 1724.12 -4362.00 215.19
1907 57 4.00 -5168.52 1680.12 -4352.00 217.61
1908 57 5.00 -5193.52 1636.12 -4342.00 220.03
1909 57 6.00 -5218.32 1592.12 -4332.00 222.44
1910 57 7.00 -5242.92 1548.12 -4322.00 224.86
1911 57 8.00 -5267.32 1504.12 -4312.00 227.28
1912 57 9.00 -5291.52 1460.12 -4302.00 229.70
1913 57 10.00 -5315.52 1416.12 -4292.00 232.12
1914 57 11.00 -5339.32 1372.12 -4282.00 234.54
1915 57 12.00 -5362.92 1328.12 -4272.00 236.96
1916 57 13.00 -5386.32 1284.12 -4262.00 239.38
1917 57 14.00 -5409.52 1240.12 -4252.00 241.80
1918 57 15.00 -5432.52 1196.12 -4242.00 244.21
1919 57 16.00 -5455.32 1152.12 -4232.00 246.63
1920 57 17.00 -5477.92 1108.12 -4222.00 249.05
1921 57 18.00 -5500.32 1064.12 -4212.00 251.47
1922 57 19.00 -5522.52 1020.12 -4202.00 253.89
1923 57 20.00 -5544.52 976.12 -4192.00 256.31
1924 57 21.00 -5566.32 932.12 -4182.00 258.73
1925 57 22.00 -5587.92 888.12 -4172.00 261.15
1926 57 23.00 -5609.32 844.12 -4162.00 263.56
1927 57 24.00 -5630.52 800.12 -4152.00 265.98
1928 57 25.00 -5651.52 756.12 -4142.00 268.40
1929 57 26.00 -5672.32 712.12 -4132.00 270.82
1930 57 27.00 -5692.92 668.12 -4122.00 273.24
1931 57 28.00 -5713.32 624.12 -4112.00 275.66
1932 57 0.00 -5733.52 580.12 -4102.00 278.08
1933 57 1.00 -5753.52 536.12 -4092.00 280.50
1934 57 2.00 -5773.32 492.12 -4082.00 282.91
1935 57 3.00 -5792.92 448.12 -4072.00 285.33
1936 57 4.00 -5812.32 404.12 -4062.00 287.75
1937 57 5.00 -5831.52 360.12 -4052.00 290.17
1938 57 6.00 -5850.52 316.12 -4042.00 292.59
1939 57 7.00 -5869.32 272.12 -4032.00 295.01
1940 57 8.00 -5887.92 228.12 -4022.00 297.43
1941 57 9.00 -5906.32 184.12 -4012.00 299.85
1942 57 10.00 -5924.52 140.12 -4002.00 302.27
1943 57 11.00 -5942.52 96.12 -3992.00 304.68
1944 57 12.00 -5960.32 52.12 -3982.00 307.10
1945 57 13.00 -5977.92 8.12 -3972.00 309.52
1946 57 14.00 -5995.32 -35.88 -3962.00 311.94
1947 57 15.00 -6012.52 -79.88 -3952.00 314.36
1948 57 16.00 -6029.52 -123.88 -3942.00 316.78
1949 57 17.00 -6046.32 -167.88 -3932.00 319.20
1950 57 18.00 -6062.92 -211.88 -3922.00 321.62
1951 57 19.00 -6079.32 -255.88 -3912.00 324.03
1952 57 20.00 -6095.52 -299.88 -3902.00 326.45
1953 57 21.00 -6111.52 -343.88 -3892.00 328.87
1954 57 22.00 -6127.32 -387.88 -3882.00 331.29
1955 57 23.00 -6142.92 -431.88 -3872.00 333.71
1956 57 24.00 -6158.32 -475.88 -3862.00 336.13
1957 57 25.00 -6173.52 -519.88 -3852.00 338.55
1958 57 26.00 -6188.52 -563.88 -3842.00 340.97
1959 57 27.00 -6203.32 -607.88 -3832.00 343.39
1960 57 28.00 -6217.92 -651.88 -3822.00 345.80
1961 57 0.00 -6232.32 -695.88 -3812.00 348.22
1962 57 1.00 -6246.52 -739.88 -3802.00 350.64
1963 57 2.00 -6260.52 -783.88 -3792.00 353.06
1964 57 3.00 -6274.32 -827.88 -3782.00 355.48
1965 57 4.00 -6287.92 -871.88 -3772.00 357.90
1966 57 5.00 -6301.32 -915.88 -3762.00 360.32
1967 57 6.00 -6314.52 -959.88 -3752.00 362.74
1968 57 7.00 -6327.52 -1003.88 -3742.00 365.15
1969 57 8.00 -6340.32 -1047.88 -3732.00 367.57
1970 57 9.00 -6352.92 -1091.88 -3722.00 369.99
1971 57 10.00 -6365.32 -1135.88 -3712.00 372.41
1972 57 11.00 -6377.52 -1179.88 -3702.00 374.83
1973 57 12.00 -6389.52 -1223.88 -3692.00 377.25
1974 57 13.00 -6401.32 -1267.88 -3682.00 379.67
1975 57 14.00 -6412.92 -1311.88 -3672.00 382.09
1976 57 15.00 -6424.32 -1355.88 -3662.00 384.51
1977 57 16.00 -6435.52 -1399.88 -3652.00 386.92
1978 57 17.00 -6446.52 -1443.88 -3642.00 389.34
1979 57 18.00 -6457.32 -1487.88 -3632.00 391.76
1980 57 19.00 -6467.92 -1531.88 -3622.00 394.18
1981 57 20.00 -6478.32 -1575.88 -3612.00 396.60
1982 57 21.00 -6488.52 -1619.88 -3602.00 399.02
1983 57 22.00 -6498.52 -1663.88 -3592.00 401.44
1984 57 23.00 -6508.32 -1707.88 -3582.00 403.86
1985 57 24.00 -6517.92 -1751.88 -3572.00 406.27
1986 57 25.00 -6527.32 -1795.88 -3562.00 408.69
1987 57 26.00 -6536.52 -1839.88 -3552.00 411.11
1988 57 27.00 -6545.52 -1883.88 -3542.00 413.53
1989 57 28.00 -6554.32 -1927.88 -3532.00 415.95
1990 57 0.00 -6562.92 -1971.88 -3522.00 418.37
1991 57 1.00 -6571.32 -2015.88 -3512.00 420.79
1992 57 2.00 -6579.52 -2059.88 -3502.00 423.21
1993 57 3.00 -6587.52 -2103.88 -3492.00 425.62
1994 57 4.00 -6595.32 -2147.88 -3482.00 428.04
1995 57 5.00 -6602.92 -2191.88 -3472.00 430.46
1996 57 6.00 -6610.32 -2235.88 -3462.00 432.88
1997 57 7.00 -6617.52 -2279.88 -3452.00 435.30
1998 57 8.00 -6624.52 -2323.88 -3442.00 437.72
1999 57 9.00 -6631.32 -2367.88 -3432.00 440.14
2000 57 10.00 -6637.92 -2411.88 -3422.00 442.56
";

/// The seed the N64's frame 1680 left for the Twister's Move draws: its
/// particle scripts drew ten values first (`0xFF619947`, frame 1679's
/// seed, stepped ten times). The host runs no particle scripts, so the
/// replay sets it; the three draws then leave `0xB492E61A`, frame 1680's.
const HYRULE_SUMMON_END_SEED: u32 = 0xA51F_B561;

/// Training on Hyrule (Mario P1, the dummy Mario) with its Twister, after
/// the N64's frame 0: the map, both fighters and the pair.
fn hyrule_pair(
    pack: &ssb_rom::pack::Pack<'_>,
) -> (Vec<ssb_game::weapon::MapSurface>, Vec<Posed>, Pair) {
    hazard_pair(pack, 4)
}

/// Training on VS stage `gkind` (Mario P1, the dummy Mario) with its
/// controller, after the N64's frame 0: the map, both fighters and the
/// pair.
fn hazard_pair(
    pack: &ssb_rom::pack::Pack<'_>,
    gkind: usize,
) -> (Vec<ssb_game::weapon::MapSurface>, Vec<Posed>, Pair) {
    use ssb_game::stage::{MapObject, NoItems, NoObjects, Stage, StageInit, StageKind};
    let index = pack
        .stage_of_file(ssb_rom::stage::VS_GROUND_FILES[gkind])
        .unwrap();
    let (stage, segments) = stage_floors(pack, index);
    let map = dream_land_map(pack, &stage);
    let mut fighters: Vec<Posed> = [0, 1]
        .into_iter()
        .map(|port| {
            Posed::new(
                pack,
                training_fighter(pack, &stage, FighterKind::Mario, port, &segments),
            )
        })
        .collect();
    let s = pack.stage(index).unwrap();
    let kind = StageKind::from_gkind(gkind as u8).unwrap();
    let map_objects: Vec<MapObject> = pack
        .stage_points(&s)
        .map(|p| MapObject {
            kind: p.kind,
            pos: ssb_engine::math::Vec3::new(f32::from(p.x), f32::from(p.y), 0.0),
        })
        .collect();
    let (hazard_attack, hazard_throw) = kind.hazard_descs(s.hazard);
    let init = StageInit {
        kind,
        map_objects: &map_objects,
        bound_bottom: f32::from(s.bounds.bottom),
        hazard_attack,
        hazard_throw,
        acid_surface_y: s.hazard_surface_y,
        bonus3_bumpers: &[],
        player: 0,
    };
    // The program's seed 1 reaches the stage's make: Jungle's Barrel draws
    // its first wait there; Hyrule's Twister draws nothing until frame 0.
    ssb_game::rng::set_seed(1);
    let mut pair = Pair {
        stage: Stage::new(&init, &mut [], &mut NoObjects, &mut NoItems),
        ..Pair::default()
    };
    // N64 frame 0 already ran the Ground link's processes (the Twister's
    // Sleep -> Wait); the replay's tick 1 is the N64's frame 1.
    {
        let mut refs: Vec<&mut ssb_game::fighter::Fighter> =
            fighters.iter_mut().map(|p| &mut p.fighter).collect();
        posed::tick_stage(&mut pair.stage, &map, &mut refs, &mut NoItems, true);
    }
    (map, fighters, pair)
}

/// Replays Mario against the dummy on Hyrule with its Twister. Returns the
/// first bad tick, the log and the largest position error.
fn hyrule_twister_replay(table: &str) -> Option<(Option<u64>, String, f32)> {
    use ssb_game::stage::Controller;
    let bytes = open_pack()?;
    let pack = ssb_rom::pack::Pack::open(&bytes).unwrap();
    let (map, mut fighters, mut pair) = hyrule_pair(&pack);
    let pad = |t: u64| match t {
        1690..=1889 => (0, -127, 0),
        _ => (0, 0, 0),
    };
    let want: Vec<Vec<f32>> = rows(table).collect();
    let last = want.last().unwrap()[0] as u64;
    let mut log = Vec::new();
    let mut first_bad = None;
    let mut max_err = 0.0f32;
    for tick in 1..=last {
        if tick == 1680 {
            ssb_game::rng::set_seed(HYRULE_SUMMON_END_SEED as i32);
        }
        let _ = tick_pair(&pack, &map, &mut fighters, &pad, tick, &mut pair);
        let Some(row) = want.iter().find(|r| r[0] as u64 == tick) else {
            continue;
        };
        let f = &fighters[0].fighter;
        let mut got = Vec::new();
        if row.len() >= 5 {
            got.extend([
                f32::from(f.status.status.id()),
                f.status.anim_frame,
                f.pos.x,
                f.pos.y,
            ]);
        }
        if let Controller::Hyrule(h) = &pair.stage.controller {
            use ssb_game::stage::hyrule::TwisterStatus as T;
            if matches!(h.status, T::Summon | T::Move | T::Turn | T::Stop) {
                got.extend([h.twister_pos.x, h.twister_pos.y]);
            }
        }
        let ok = got.len() + 1 == row.len() && got.iter().zip(&row[1..]).all(|(g, w)| near(*g, *w));
        let skip = if row.len() >= 5 { 2 } else { 0 };
        max_err = got
            .iter()
            .zip(&row[1..])
            .skip(skip)
            .fold(max_err, |m, (g, w)| m.max((g - w).abs()));
        log.push(format!(
            "{tick} {} rust {got:?}\n{tick} {}  n64 {:?}",
            if ok { " " } else { "X" },
            if ok { " " } else { "X" },
            &row[1..]
        ));
        if !ok && first_bad.is_none() {
            first_bad = Some(tick);
        }
    }
    Some((first_bad, log.join("\n"), max_err))
}

#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn the_twister_takes_mario_as_on_the_n64() {
    let Some((first_bad, log, max_err)) = hyrule_twister_replay(HYRULE_TWISTER) else {
        return;
    };
    if std::env::var_os("SCENE_TRACE").is_some() {
        println!("Mario in Hyrule's Twister (max position error {max_err}):\n{log}");
    }
    assert_eq!(first_bad, None, "{log}");
}

/// `b30set/hyr2` (`warp_hyrule_mario`, no input): `hyr3`'s Twister to the
/// end of its life, nobody on its line. Made at frame 1600, it moves left
/// from 1680, turns at each edge (120 frames at 1777, 2045, 2313), its
/// 888-frame life runs out at 2568 (`Stop` 5), and it subsides for 32
/// frames from 2569. Per row (every 25th frame and the frames round each
/// change): tick, `gGRCommonStruct.hyrule` status, wait, `speed_wait`,
/// `turn_wait`, line, vel, left and right edge x, then the Twister's x
/// and y while it exists.
const HYRULE_TWISTER_LIFE: &str = "
25 1 1575 0 0 0 0.00 0.00 0.00
50 1 1550 0 0 0 0.00 0.00 0.00
75 1 1525 0 0 0 0.00 0.00 0.00
100 1 1500 0 0 0 0.00 0.00 0.00
125 1 1475 0 0 0 0.00 0.00 0.00
150 1 1450 0 0 0 0.00 0.00 0.00
175 1 1425 0 0 0 0.00 0.00 0.00
200 1 1400 0 0 0 0.00 0.00 0.00
225 1 1375 0 0 0 0.00 0.00 0.00
250 1 1350 0 0 0 0.00 0.00 0.00
275 1 1325 0 0 0 0.00 0.00 0.00
300 1 1300 0 0 0 0.00 0.00 0.00
325 1 1275 0 0 0 0.00 0.00 0.00
350 1 1250 0 0 0 0.00 0.00 0.00
375 1 1225 0 0 0 0.00 0.00 0.00
400 1 1200 0 0 0 0.00 0.00 0.00
425 1 1175 0 0 0 0.00 0.00 0.00
450 1 1150 0 0 0 0.00 0.00 0.00
475 1 1125 0 0 0 0.00 0.00 0.00
500 1 1100 0 0 0 0.00 0.00 0.00
525 1 1075 0 0 0 0.00 0.00 0.00
550 1 1050 0 0 0 0.00 0.00 0.00
575 1 1025 0 0 0 0.00 0.00 0.00
600 1 1000 0 0 0 0.00 0.00 0.00
625 1 975 0 0 0 0.00 0.00 0.00
650 1 950 0 0 0 0.00 0.00 0.00
675 1 925 0 0 0 0.00 0.00 0.00
700 1 900 0 0 0 0.00 0.00 0.00
725 1 875 0 0 0 0.00 0.00 0.00
750 1 850 0 0 0 0.00 0.00 0.00
775 1 825 0 0 0 0.00 0.00 0.00
800 1 800 0 0 0 0.00 0.00 0.00
825 1 775 0 0 0 0.00 0.00 0.00
850 1 750 0 0 0 0.00 0.00 0.00
875 1 725 0 0 0 0.00 0.00 0.00
900 1 700 0 0 0 0.00 0.00 0.00
925 1 675 0 0 0 0.00 0.00 0.00
950 1 650 0 0 0 0.00 0.00 0.00
975 1 625 0 0 0 0.00 0.00 0.00
1000 1 600 0 0 0 0.00 0.00 0.00
1025 1 575 0 0 0 0.00 0.00 0.00
1050 1 550 0 0 0 0.00 0.00 0.00
1075 1 525 0 0 0 0.00 0.00 0.00
1100 1 500 0 0 0 0.00 0.00 0.00
1125 1 475 0 0 0 0.00 0.00 0.00
1150 1 450 0 0 0 0.00 0.00 0.00
1175 1 425 0 0 0 0.00 0.00 0.00
1200 1 400 0 0 0 0.00 0.00 0.00
1225 1 375 0 0 0 0.00 0.00 0.00
1250 1 350 0 0 0 0.00 0.00 0.00
1275 1 325 0 0 0 0.00 0.00 0.00
1300 1 300 0 0 0 0.00 0.00 0.00
1325 1 275 0 0 0 0.00 0.00 0.00
1350 1 250 0 0 0 0.00 0.00 0.00
1375 1 225 0 0 0 0.00 0.00 0.00
1400 1 200 0 0 0 0.00 0.00 0.00
1425 1 175 0 0 0 0.00 0.00 0.00
1450 1 150 0 0 0 0.00 0.00 0.00
1475 1 125 0 0 0 0.00 0.00 0.00
1500 1 100 0 0 0 0.00 0.00 0.00
1525 1 75 0 0 0 0.00 0.00 0.00
1550 1 50 0 0 0 0.00 0.00 0.00
1575 1 25 0 0 0 0.00 0.00 0.00
1598 1 2 0 0 0 0.00 0.00 0.00
1599 1 1 0 0 0 0.00 0.00 0.00
1600 2 80 0 0 5 0.00 -4462.00 -2976.00 -3499.00 423.93
1601 2 79 0 0 5 0.00 -4462.00 -2976.00 -3499.00 423.93
1602 2 78 0 0 5 0.00 -4462.00 -2976.00 -3499.00 423.93
1625 2 55 0 0 5 0.00 -4462.00 -2976.00 -3499.00 423.93
1650 2 30 0 0 5 0.00 -4462.00 -2976.00 -3499.00 423.93
1675 2 5 0 0 5 0.00 -4462.00 -2976.00 -3499.00 423.93
1678 2 2 0 0 5 0.00 -4462.00 -2976.00 -3499.00 423.93
1679 2 1 0 0 5 0.00 -4462.00 -2976.00 -3499.00 423.93
1680 3 888 264 0 5 -10.00 -4462.00 -2976.00 -3499.00 423.93
1681 3 887 263 0 5 -10.00 -4462.00 -2976.00 -3509.00 421.51
1682 3 886 262 0 5 -10.00 -4462.00 -2976.00 -3519.00 419.09
1700 3 868 244 0 5 -10.00 -4462.00 -2976.00 -3699.00 375.56
1725 3 843 219 0 5 -10.00 -4462.00 -2976.00 -3949.00 315.09
1750 3 818 194 0 5 -10.00 -4462.00 -2976.00 -4199.00 254.61
1775 3 793 169 0 5 -10.00 -4462.00 -2976.00 -4449.00 194.14
1776 3 792 168 0 5 -10.00 -4462.00 -2976.00 -4459.00 191.73
1777 4 791 167 120 5 -10.00 -4462.00 -2976.00 -4452.00 193.42
1778 4 790 167 119 5 -10.00 -4462.00 -2976.00 -4452.00 193.42
1779 4 789 167 118 5 -10.00 -4462.00 -2976.00 -4452.00 193.42
1800 4 768 167 97 5 -10.00 -4462.00 -2976.00 -4452.00 193.42
1825 4 743 167 72 5 -10.00 -4462.00 -2976.00 -4452.00 193.42
1850 4 718 167 47 5 -10.00 -4462.00 -2976.00 -4452.00 193.42
1875 4 693 167 22 5 -10.00 -4462.00 -2976.00 -4452.00 193.42
1895 4 673 167 2 5 -10.00 -4462.00 -2976.00 -4452.00 193.42
1896 4 672 167 1 5 -10.00 -4462.00 -2976.00 -4452.00 193.42
1897 3 671 167 0 5 10.00 -4462.00 -2976.00 -4452.00 193.42
1898 3 670 166 0 5 10.00 -4462.00 -2976.00 -4442.00 195.84
1899 3 669 165 0 5 10.00 -4462.00 -2976.00 -4432.00 198.26
1900 3 668 164 0 5 10.00 -4462.00 -2976.00 -4422.00 200.68
1925 3 643 139 0 5 10.00 -4462.00 -2976.00 -4172.00 261.15
1950 3 618 114 0 5 10.00 -4462.00 -2976.00 -3922.00 321.62
1975 3 593 89 0 5 10.00 -4462.00 -2976.00 -3672.00 382.09
2000 3 568 64 0 5 10.00 -4462.00 -2976.00 -3422.00 442.56
2025 3 543 39 0 5 10.00 -4462.00 -2976.00 -3172.00 503.03
2043 3 525 21 0 5 10.00 -4462.00 -2976.00 -2992.00 546.57
2044 3 524 20 0 5 10.00 -4462.00 -2976.00 -2982.00 548.98
2045 4 523 19 120 5 10.00 -4462.00 -2976.00 -2986.00 548.02
2046 4 522 19 119 5 10.00 -4462.00 -2976.00 -2986.00 548.02
2047 4 521 19 118 5 10.00 -4462.00 -2976.00 -2986.00 548.02
2050 4 518 19 115 5 10.00 -4462.00 -2976.00 -2986.00 548.02
2075 4 493 19 90 5 10.00 -4462.00 -2976.00 -2986.00 548.02
2100 4 468 19 65 5 10.00 -4462.00 -2976.00 -2986.00 548.02
2125 4 443 19 40 5 10.00 -4462.00 -2976.00 -2986.00 548.02
2150 4 418 19 15 5 10.00 -4462.00 -2976.00 -2986.00 548.02
2163 4 405 19 2 5 10.00 -4462.00 -2976.00 -2986.00 548.02
2164 4 404 19 1 5 10.00 -4462.00 -2976.00 -2986.00 548.02
2165 3 403 19 0 5 -10.00 -4462.00 -2976.00 -2986.00 548.02
2166 3 402 18 0 5 -10.00 -4462.00 -2976.00 -2996.00 545.60
2167 3 401 17 0 5 -10.00 -4462.00 -2976.00 -3006.00 543.18
2175 3 393 9 0 5 -10.00 -4462.00 -2976.00 -3086.00 523.83
2200 3 368 65520 0 5 -10.00 -4462.00 -2976.00 -3336.00 463.36
2225 3 343 65495 0 5 -10.00 -4462.00 -2976.00 -3586.00 402.89
2250 3 318 65470 0 5 -10.00 -4462.00 -2976.00 -3836.00 342.42
2275 3 293 65445 0 5 -10.00 -4462.00 -2976.00 -4086.00 281.95
2300 3 268 65420 0 5 -10.00 -4462.00 -2976.00 -4336.00 221.48
2311 3 257 65409 0 5 -10.00 -4462.00 -2976.00 -4446.00 194.87
2312 3 256 65408 0 5 -10.00 -4462.00 -2976.00 -4456.00 192.45
2313 4 255 65407 120 5 -10.00 -4462.00 -2976.00 -4452.00 193.42
2314 4 254 65407 119 5 -10.00 -4462.00 -2976.00 -4452.00 193.42
2315 4 253 65407 118 5 -10.00 -4462.00 -2976.00 -4452.00 193.42
2325 4 243 65407 108 5 -10.00 -4462.00 -2976.00 -4452.00 193.42
2350 4 218 65407 83 5 -10.00 -4462.00 -2976.00 -4452.00 193.42
2375 4 193 65407 58 5 -10.00 -4462.00 -2976.00 -4452.00 193.42
2400 4 168 65407 33 5 -10.00 -4462.00 -2976.00 -4452.00 193.42
2425 4 143 65407 8 5 -10.00 -4462.00 -2976.00 -4452.00 193.42
2431 4 137 65407 2 5 -10.00 -4462.00 -2976.00 -4452.00 193.42
2432 4 136 65407 1 5 -10.00 -4462.00 -2976.00 -4452.00 193.42
2433 3 135 65407 0 5 10.00 -4462.00 -2976.00 -4452.00 193.42
2434 3 134 65406 0 5 10.00 -4462.00 -2976.00 -4442.00 195.84
2435 3 133 65405 0 5 10.00 -4462.00 -2976.00 -4432.00 198.26
2450 3 118 65390 0 5 10.00 -4462.00 -2976.00 -4282.00 234.54
2475 3 93 65365 0 5 10.00 -4462.00 -2976.00 -4032.00 295.01
2500 3 68 65340 0 5 10.00 -4462.00 -2976.00 -3782.00 355.48
2525 3 43 65315 0 5 10.00 -4462.00 -2976.00 -3532.00 415.95
2550 3 18 65290 0 5 10.00 -4462.00 -2976.00 -3282.00 476.42
2566 3 2 65274 0 5 10.00 -4462.00 -2976.00 -3122.00 515.12
2567 3 1 65273 0 5 10.00 -4462.00 -2976.00 -3112.00 517.54
2568 5 0 65273 0 5 10.00 -4462.00 -2976.00 -3112.00 517.54
2569 6 32 65273 0 5 10.00 -4462.00 -2976.00
2570 6 31 65273 0 5 10.00 -4462.00 -2976.00
2571 6 30 65273 0 5 10.00 -4462.00 -2976.00
2575 6 26 65273 0 5 10.00 -4462.00 -2976.00
2600 6 1 65273 0 5 10.00 -4462.00 -2976.00
";

#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn the_twister_lives_and_subsides_as_on_the_n64() {
    use ssb_game::stage::Controller;
    let Some(bytes) = open_pack() else {
        return;
    };
    let pack = ssb_rom::pack::Pack::open(&bytes).unwrap();
    let (map, mut fighters, mut pair) = hyrule_pair(&pack);
    let want: Vec<Vec<f32>> = rows(HYRULE_TWISTER_LIFE).collect();
    let last = want.last().unwrap()[0] as u64;
    let mut log = Vec::new();
    let mut first_bad = None;
    let mut max_err = 0.0f32;
    for tick in 1..=last {
        if tick == 1680 {
            ssb_game::rng::set_seed(HYRULE_SUMMON_END_SEED as i32);
        }
        let _ = tick_pair(&pack, &map, &mut fighters, &|_| (0, 0, 0), tick, &mut pair);
        let Some(row) = want.iter().find(|r| r[0] as u64 == tick) else {
            continue;
        };
        let Controller::Hyrule(h) = &pair.stage.controller else {
            unreachable!()
        };
        let mut got = vec![
            h.status as u8 as f32,
            f32::from(h.wait),
            f32::from(h.speed_wait),
            f32::from(h.turn_wait),
            f32::from(h.line),
            h.vel,
            h.left_edge_x,
            h.right_edge_x,
        ];
        if row.len() == 11 {
            got.extend([h.twister_pos.x, h.twister_pos.y]);
            max_err = max_err
                .max((h.twister_pos.x - row[9]).abs())
                .max((h.twister_pos.y - row[10]).abs());
        }
        let ok = got.len() + 1 == row.len() && got.iter().zip(&row[1..]).all(|(g, w)| near(*g, *w));
        let mark = if ok { " " } else { "X" };
        log.push(format!(
            "{tick} {mark} rust {got:?}\n{tick} {mark}  n64 {:?}",
            &row[1..]
        ));
        if !ok && first_bad.is_none() {
            first_bad = Some(tick);
        }
    }
    let log = log.join("\n");
    if std::env::var_os("SCENE_TRACE").is_some() {
        println!("The Twister's life (max position error {max_err}):\n{log}");
    }
    assert_eq!(first_bad, None, "{log}");
}

/// `b30set/itm2` (`warp_link`): Training's menu makes a Beam Sword above
/// Link, he picks it up and smash-throws it into the dummy. Route
/// `30:1:start;40:1:y=-80;50:1:x=80;60:1:x=80;70:1:x=80;80:1:x=80;90:1:a;
/// 100:1:start;150:1:a;200:1:z+a+x=127`, N64 frames 89-250. The menu
/// (START at 30, Item row, four steps right to the Beam Sword, A at 90)
/// locks both fighters' control without stopping the game:
/// `sc1PTrainingModeUpdateItemOption` makes the sword (`ITKind` 7) at frame
/// 90, 200 above Link at (-210, 1599.25) rising at 30, and it moves on the
/// same frame. It comes to rest at (-209.68, 1821.25). A at 150 picks it up
/// (`LightGet` 100, `anim_frame` 1); Z+A with a smash right at 200 throws it
/// (`LightThrowF4` 110, `anim_frame` 1), let go at frame 207 (`anim_frame`
/// 8) at (171.03, 1833.66) moving (99.172, 12.845). It hits the dummy at
/// frame 211 (`Damage` 51) and bounces back at (-5.95, 22.586). Per row:
/// tick, then Link's status, `anim_frame`, x, y and the dummy's status
/// (from 150, after the opening Wait), then the sword's x, y and velocity
/// while it is not held.
const LINK_SWORD_THROW: &str = "
89
90 -210.00 1628.05 0.000 28.800
91 -210.00 1655.65 0.000 27.600
92 -210.00 1682.05 0.000 26.400
93 -210.00 1707.25 0.000 25.200
94 -210.00 1731.25 0.000 24.000
95 -210.00 1754.05 0.000 22.800
96 -210.00 1775.65 0.000 21.600
97 -210.00 1796.05 0.000 20.400
98 -210.00 1815.25 0.000 19.200
99 -210.00 1833.25 0.000 18.000
100 -210.00 1850.05 0.000 16.800
101 -210.00 1865.65 0.000 15.600
102 -210.00 1880.05 0.000 14.400
103 -210.00 1893.25 0.000 13.200
104 -210.00 1905.25 0.000 12.000
105 -210.00 1916.05 0.000 10.800
106 -210.00 1925.65 0.000 9.600
107 -210.00 1934.05 0.000 8.400
108 -210.00 1941.25 0.000 7.200
109 -210.00 1947.25 0.000 6.000
110 -210.00 1952.05 0.000 4.800
111 -210.00 1955.65 0.000 3.600
112 -210.00 1958.05 0.000 2.400
113 -210.00 1959.25 0.000 1.200
114 -210.00 1959.25 0.000 -0.000
115 -210.00 1958.05 0.000 -1.200
116 -210.00 1955.65 0.000 -2.400
117 -210.00 1952.05 0.000 -3.600
118 -210.00 1947.25 0.000 -4.800
119 -210.00 1941.25 0.000 -6.000
120 -210.00 1934.05 0.000 -7.200
121 -210.00 1925.65 0.000 -8.400
122 -210.00 1916.05 0.000 -9.600
123 -210.00 1905.25 0.000 -10.800
124 -210.00 1893.25 0.000 -12.000
125 -210.00 1880.05 0.000 -13.200
126 -210.00 1865.65 0.000 -14.400
127 -210.00 1850.05 0.000 -15.600
128 -210.00 1833.25 0.000 -16.800
129 -210.00 1821.25 0.021 9.000
130 -209.98 1829.05 0.021 7.800
131 -209.96 1835.65 0.021 6.600
132 -209.94 1841.05 0.021 5.400
133 -209.92 1845.25 0.021 4.200
134 -209.89 1848.25 0.021 3.000
135 -209.87 1850.05 0.021 1.800
136 -209.85 1850.65 0.021 0.600
137 -209.83 1850.05 0.021 -0.600
138 -209.81 1848.25 0.021 -1.800
139 -209.79 1845.25 0.021 -3.000
140 -209.77 1841.05 0.021 -4.200
141 -209.75 1835.65 0.021 -5.400
142 -209.73 1829.05 0.021 -6.600
143 -209.71 1821.25 0.021 -7.800
144 -209.68 1821.25 0.000 0.000
145 -209.68 1821.25 0.000 0.000
146 -209.68 1821.25 0.000 0.000
147 -209.68 1821.25 0.000 0.000
148 -209.68 1821.25 0.000 0.000
149 -209.68 1821.25 0.000 0.000
150 100 1.00 -210.00 1399.25 10 -209.68 1821.25 0.000 0.000
151 100 2.00 -210.00 1399.25 10 -209.68 1821.25 0.000 0.000
152 100 3.00 -210.00 1399.25 10 -209.68 1821.25 0.000 0.000
153 100 4.00 -210.00 1399.25 10
154 100 5.00 -210.00 1399.25 10
155 100 6.00 -210.00 1399.25 10
156 100 7.00 -210.00 1399.25 10
157 10 0.00 -210.00 1399.25 10
158 10 1.00 -210.00 1399.25 10
159 10 2.00 -210.00 1399.25 10
160 10 3.00 -210.00 1399.25 10
161 10 4.00 -210.00 1399.25 10
162 10 5.00 -210.00 1399.25 10
163 10 6.00 -210.00 1399.25 10
164 10 7.00 -210.00 1399.25 10
165 10 8.00 -210.00 1399.25 10
166 10 9.00 -210.00 1399.25 10
167 10 10.00 -210.00 1399.25 10
168 10 11.00 -210.00 1399.25 10
169 10 12.00 -210.00 1399.25 10
170 10 13.00 -210.00 1399.25 10
171 10 14.00 -210.00 1399.25 10
172 10 15.00 -210.00 1399.25 10
173 10 16.00 -210.00 1399.25 10
174 10 17.00 -210.00 1399.25 10
175 10 18.00 -210.00 1399.25 10
176 10 19.00 -210.00 1399.25 10
177 10 20.00 -210.00 1399.25 10
178 10 21.00 -210.00 1399.25 10
179 10 22.00 -210.00 1399.25 10
180 10 23.00 -210.00 1399.25 10
181 10 24.00 -210.00 1399.25 10
182 10 25.00 -210.00 1399.25 10
183 10 26.00 -210.00 1399.25 10
184 10 27.00 -210.00 1399.25 10
185 10 28.00 -210.00 1399.25 10
186 10 29.00 -210.00 1399.25 10
187 10 30.00 -210.00 1399.25 10
188 10 31.00 -210.00 1399.25 10
189 10 32.00 -210.00 1399.25 10
190 10 33.00 -210.00 1399.25 10
191 10 34.00 -210.00 1399.25 10
192 10 35.00 -210.00 1399.25 10
193 10 36.00 -210.00 1399.25 10
194 10 37.00 -210.00 1399.25 10
195 10 38.00 -210.00 1399.25 10
196 10 39.00 -210.00 1399.25 10
197 10 40.00 -210.00 1399.25 10
198 10 41.00 -210.00 1399.25 10
199 10 42.00 -210.00 1399.25 10
200 110 1.00 -210.00 1399.25 10
201 110 2.00 -210.00 1399.25 10
202 110 3.00 -210.00 1399.25 10
203 110 4.00 -210.00 1399.25 10
204 110 5.00 -210.00 1399.25 10
205 110 6.00 -210.00 1399.25 10
206 110 7.00 -210.00 1399.25 10
207 110 8.00 -210.00 1399.25 10 171.03 1833.66 99.172 12.845
208 110 9.00 -210.00 1399.25 10 270.20 1845.30 99.172 11.645
209 110 10.00 -210.00 1399.25 10 369.37 1855.75 99.172 10.445
210 110 11.00 -210.00 1399.25 10 468.55 1864.99 99.172 9.245
211 110 12.00 -210.00 1399.25 51 567.72 1873.04 -5.950 22.586
212 110 13.00 -210.00 1399.25 51 561.77 1894.43 -5.950 21.386
213 110 14.00 -210.00 1399.25 51 555.82 1914.61 -5.950 20.186
214 110 15.00 -210.00 1399.25 51 549.87 1933.60 -5.950 18.986
215 110 16.00 -210.00 1399.25 51 543.92 1951.38 -5.950 17.786
216 110 17.00 -210.00 1399.25 51 537.97 1967.97 -5.950 16.586
217 110 18.00 -210.00 1399.25 51 532.02 1983.36 -5.950 15.386
218 110 19.00 -210.00 1399.25 51 526.06 1997.54 -5.950 14.186
219 110 20.00 -210.00 1399.25 51 520.11 2010.53 -5.950 12.986
220 110 21.00 -210.00 1399.25 51 514.16 2022.32 -5.950 11.786
221 110 22.00 -210.00 1399.25 51 508.21 2032.90 -5.950 10.586
222 110 23.00 -210.00 1399.25 51 502.26 2042.29 -5.950 9.386
223 10 0.00 -210.00 1399.25 51 496.31 2050.48 -5.950 8.186
224 10 1.00 -210.00 1399.25 51 490.36 2057.46 -5.950 6.986
225 10 2.00 -210.00 1399.25 51 484.41 2063.25 -5.950 5.786
226 10 3.00 -210.00 1399.25 51 478.46 2067.84 -5.950 4.586
227 10 4.00 -210.00 1399.25 51 472.51 2071.22 -5.950 3.386
228 10 5.00 -210.00 1399.25 51 466.56 2073.41 -5.950 2.186
229 10 6.00 -210.00 1399.25 51 460.61 2074.40 -5.950 0.986
230 10 7.00 -210.00 1399.25 51 454.66 2074.18 -5.950 -0.214
231 10 8.00 -210.00 1399.25 51 448.71 2072.77 -5.950 -1.414
232 10 9.00 -210.00 1399.25 51 442.76 2070.15 -5.950 -2.614
233 10 10.00 -210.00 1399.25 51 436.81 2066.34 -5.950 -3.814
234 10 11.00 -210.00 1399.25 51 430.86 2061.33 -5.950 -5.014
235 10 12.00 -210.00 1399.25 51 424.91 2055.11 -5.950 -6.214
236 10 13.00 -210.00 1399.25 51 418.96 2047.70 -5.950 -7.414
237 10 14.00 -210.00 1399.25 51 413.01 2039.09 -5.950 -8.614
238 10 15.00 -210.00 1399.25 51 407.06 2029.27 -5.950 -9.814
239 10 16.00 -210.00 1399.25 51 401.11 2018.26 -5.950 -11.014
240 10 17.00 -210.00 1399.25 51 395.16 2006.05 -5.950 -12.214
241 10 18.00 -210.00 1399.25 51 389.21 1992.63 -5.950 -13.414
242 10 19.00 -210.00 1399.25 51 383.26 1978.02 -5.950 -14.614
243 10 20.00 -210.00 1399.25 51 377.31 1962.21 -5.950 -15.814
244 10 21.00 -210.00 1399.25 51 371.36 1945.19 -5.950 -17.014
245 10 22.00 -210.00 1399.25 51 365.41 1926.98 -5.950 -18.214
246 10 23.00 -210.00 1399.25 51 359.46 1907.56 -5.950 -19.414
247 10 24.00 -210.00 1399.25 51 353.51 1886.95 -5.950 -20.614
248 10 25.00 -210.00 1399.25 51 347.56 1865.14 -5.950 -21.814
249 10 26.00 -210.00 1399.25 51 341.61 1842.12 -5.950 -23.014
250 10 27.00 -210.00 1399.25 51 335.66 1820.40 -1.459 12.291
";

/// Replays `table`'s Training item route: Link P1 against the dummy on the
/// Castle, the menu's item `kind` made above Link before each `make_at` tick.
/// Returns the first bad tick, the log and the largest position error.
fn castle_item_replay(
    pad: &dyn Fn(u64) -> (u16, i8, i8),
    kind: u8,
    make_at: &[u64],
    table: &str,
) -> Option<(Option<u64>, String, f32)> {
    let bytes = open_pack()?;
    let pack = ssb_rom::pack::Pack::open(&bytes).unwrap();
    let (stage, segments) = stage_floors(&pack, CASTLE);
    let map = dream_land_map(&pack, &stage);
    let mut fighters: Vec<Posed> = [(FighterKind::Link, 0), (FighterKind::Mario, 1)]
        .into_iter()
        .map(|(kind, port)| {
            Posed::new(
                &pack,
                training_fighter(&pack, &stage, kind, port, &segments),
            )
        })
        .collect();
    let want: Vec<Vec<f32>> = rows(table).collect();
    let last = want.last().unwrap()[0] as u64;
    let mut pair = Pair::default();
    let mut log = Vec::new();
    let mut first_bad = None;
    let mut max_err = 0.0f32;
    let mut tracked = None;
    for tick in 1..=last {
        if make_at.contains(&tick) {
            // `sc1PTrainingModeUpdateItemOption`, before the frame's item
            // processes.
            let mut pos = fighters[0].fighter.pos;
            pos.y += 200.0;
            pos.z = 0.0;
            let vel = ssb_engine::math::Vec3::new(0.0, 30.0, 0.0);
            pair.items
                .make_setup_common(kind, None, pos, vel, &core::iter::empty);
        }
        let _ = tick_pair(&pack, &map, &mut fighters, pad, tick, &mut pair);
        let Some(row) = want.iter().find(|r| r[0] as u64 == tick) else {
            continue;
        };
        let mut got = Vec::new();
        if row.len() == 6 || row.len() == 10 {
            let f = &fighters[0].fighter;
            got.extend([
                f32::from(f.status.status.id()),
                f.status.anim_frame,
                f.pos.x,
                f.pos.y,
                f32::from(fighters[1].fighter.status.status.id()),
            ]);
        }
        // The item columns follow the item Link last held, the first live
        // item until he picks one up.
        let held = fighters[0].fighter.items.held.map(|h| h.slot);
        tracked = held.or(tracked);
        let free = match tracked {
            Some(slot) => pair.items.get(slot),
            None => pair.items.items().next(),
        };
        if let Some(it) = free.filter(|_| held.is_none()) {
            got.extend([it.pos.x, it.pos.y, it.vel_air.x, it.vel_air.y]);
        }
        let ok = got.len() + 1 == row.len() && got.iter().zip(&row[1..]).all(|(g, w)| near(*g, *w));
        let at = if row.len() >= 6 { [2, 3] } else { [9, 9] };
        max_err = got
            .iter()
            .zip(&row[1..])
            .enumerate()
            .filter(|(i, _)| at.contains(i) || *i + 4 >= row.len() - 1)
            .fold(max_err, |m, (_, (g, w))| m.max((g - w).abs()));
        log.push(format!(
            "{tick} {} rust {got:?}\n{tick} {}  n64 {:?}",
            if ok { " " } else { "X" },
            if ok { " " } else { "X" },
            &row[1..]
        ));
        if !ok && first_bad.is_none() {
            first_bad = Some(tick);
        }
    }
    Some((first_bad, log.join("\n"), max_err))
}

#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn link_smash_throws_a_beam_sword_as_on_the_n64() {
    const Z: u16 = 0x2000;
    // The menu frames (30-100) are neutral to the locked fighters.
    let pad = |t: u64| match t {
        150 => (button::A, 0, 0),
        200 => (button::A | Z, 127, 0),
        _ => (0, 0, 0),
    };
    let Some((first_bad, log, max_err)) = castle_item_replay(&pad, 7, &[90], LINK_SWORD_THROW)
    else {
        return;
    };
    if std::env::var_os("SCENE_TRACE").is_some() {
        println!("Link's Beam Sword smash throw (max position error {max_err}):\n{log}");
    }
    assert_eq!(first_bad, None, "{log}");
}

/// N64 rows of `b30set/itm3`: Training on the Castle, two Beam Swords made
/// from the menu (route `30:1:start;40:1:y=-80;50:1:x=80;60:1:x=80;70:1:x=80;
/// 80:1:x=80;90:1:a;100:1:a;110:1:start;160:1:a;200:1:z+a+x=-128;240:1:a;
/// 280:1:z+a+y=127`). Link takes the newer sword, smash throws it back,
/// takes the other and smash throws it up. Columns: `t st af x y p1st`, then
/// the item Link last took (`ix iy ivx ivy`) while he does not hold it.
const LINK_SWORD_THROWS_B_HI: &str = "
160 100 1.00 -210.00 1399.25 10 -209.68 1821.25 0.000 0.000
161 100 2.00 -210.00 1399.25 10 -209.68 1821.25 0.000 0.000
162 100 3.00 -210.00 1399.25 10 -209.68 1821.25 0.000 0.000
163 100 4.00 -210.00 1399.25 10
164 100 5.00 -210.00 1399.25 10
165 100 6.00 -210.00 1399.25 10
166 100 7.00 -210.00 1399.25 10
167 10 0.00 -210.00 1399.25 10
168 10 1.00 -210.00 1399.25 10
169 10 2.00 -210.00 1399.25 10
170 10 3.00 -210.00 1399.25 10
171 10 4.00 -210.00 1399.25 10
172 10 5.00 -210.00 1399.25 10
173 10 6.00 -210.00 1399.25 10
174 10 7.00 -210.00 1399.25 10
175 10 8.00 -210.00 1399.25 10
176 10 9.00 -210.00 1399.25 10
177 10 10.00 -210.00 1399.25 10
178 10 11.00 -210.00 1399.25 10
179 10 12.00 -210.00 1399.25 10
180 10 13.00 -210.00 1399.25 10
181 10 14.00 -210.00 1399.25 10
182 10 15.00 -210.00 1399.25 10
183 10 16.00 -210.00 1399.25 10
184 10 17.00 -210.00 1399.25 10
185 10 18.00 -210.00 1399.25 10
186 10 19.00 -210.00 1399.25 10
187 10 20.00 -210.00 1399.25 10
188 10 21.00 -210.00 1399.25 10
189 10 22.00 -210.00 1399.25 10
190 10 23.00 -210.00 1399.25 10
191 10 24.00 -210.00 1399.25 10
192 10 25.00 -210.00 1399.25 10
193 10 26.00 -210.00 1399.25 10
194 10 27.00 -210.00 1399.25 10
195 10 28.00 -210.00 1399.25 10
196 10 29.00 -210.00 1399.25 10
197 10 30.00 -210.00 1399.25 10
198 10 31.00 -210.00 1399.25 10
199 10 32.00 -210.00 1399.25 10
200 111 1.00 -210.00 1399.25 10
201 111 2.00 -210.00 1399.25 10
202 111 3.00 -210.00 1399.25 10
203 111 4.00 -210.00 1399.25 10
204 111 5.00 -210.00 1399.25 10
205 111 6.00 -210.00 1399.25 10
206 111 7.00 -210.00 1399.25 10
207 111 8.00 -210.00 1399.25 10 -591.03 1834.54 -99.172 12.845
208 111 9.00 -210.00 1399.25 10 -690.20 1846.18 -99.172 11.645
209 111 10.00 -210.00 1399.25 10 -789.37 1856.63 -99.172 10.445
210 111 11.00 -210.00 1399.25 10 -888.55 1865.87 -99.172 9.245
211 111 12.00 -210.00 1399.25 10 -987.72 1873.92 -99.172 8.045
212 111 13.00 -210.00 1399.25 10 -1086.89 1880.76 -99.172 6.845
213 111 14.00 -210.00 1399.25 10 -1186.06 1886.41 -99.172 5.645
214 111 15.00 -210.00 1399.25 10 -1285.23 1890.85 -99.172 4.445
215 111 16.00 -210.00 1399.25 10 -1384.40 1894.10 -99.172 3.245
216 111 17.00 -210.00 1399.25 10 -1483.57 1896.14 -99.172 2.045
217 111 18.00 -210.00 1399.25 10 -1582.75 1896.99 -99.172 0.845
218 111 19.00 -210.00 1399.25 10 -1681.92 1914.68 -15.247 36.244
219 111 20.00 -210.00 1399.25 10 -1697.16 1949.72 -15.247 35.044
220 111 21.00 -210.00 1399.25 10 -1712.41 1983.57 -15.247 33.844
221 111 22.00 -210.00 1399.25 10 -1727.66 2016.21 -15.247 32.644
222 111 23.00 -210.00 1399.25 10 -1742.90 2047.65 -15.247 31.444
223 10 0.00 -210.00 1399.25 10 -1758.15 2077.90 -15.247 30.244
224 10 1.00 -210.00 1399.25 10 -1773.40 2106.94 -15.247 29.044
225 10 2.00 -210.00 1399.25 10 -1788.64 2134.78 -15.247 27.844
226 10 3.00 -210.00 1399.25 10 -1803.89 2161.43 -15.247 26.644
227 10 4.00 -210.00 1399.25 10 -1819.14 2186.87 -15.247 25.444
228 10 5.00 -210.00 1399.25 10 -1834.38 2211.12 -15.247 24.244
229 10 6.00 -210.00 1399.25 10 -1849.63 2234.16 -15.247 23.044
230 10 7.00 -210.00 1399.25 10 -1864.88 2256.00 -15.247 21.844
231 10 8.00 -210.00 1399.25 10 -1880.12 2276.65 -15.247 20.644
232 10 9.00 -210.00 1399.25 10 -1895.37 2296.09 -15.247 19.444
233 10 10.00 -210.00 1399.25 10 -1910.62 2314.33 -15.247 18.244
234 10 11.00 -210.00 1399.25 10 -1925.86 2331.38 -15.247 17.044
235 10 12.00 -210.00 1399.25 10 -1941.11 2347.22 -15.247 15.844
236 10 13.00 -210.00 1399.25 10 -1956.35 2361.87 -15.247 14.644
237 10 14.00 -210.00 1399.25 10 -1971.60 2375.31 -15.247 13.444
238 10 15.00 -210.00 1399.25 10 -1986.85 2387.55 -15.247 12.244
239 10 16.00 -210.00 1399.25 10 -2002.09 2398.60 -15.247 11.044
240 100 1.00 -210.00 1399.25 10 -2017.34 2408.44 -15.247 9.844
241 100 2.00 -210.00 1399.25 10 -2032.59 2417.08 -15.247 8.644
242 100 3.00 -210.00 1399.25 10 -2047.83 2424.53 -15.247 7.444
243 100 4.00 -210.00 1399.25 10
244 100 5.00 -210.00 1399.25 10
245 100 6.00 -210.00 1399.25 10
246 100 7.00 -210.00 1399.25 10
247 10 0.00 -210.00 1399.25 10
248 10 1.00 -210.00 1399.25 10
249 10 2.00 -210.00 1399.25 10
250 10 3.00 -210.00 1399.25 10
251 10 4.00 -210.00 1399.25 10
252 10 5.00 -210.00 1399.25 10
253 10 6.00 -210.00 1399.25 10
254 10 7.00 -210.00 1399.25 10
255 10 8.00 -210.00 1399.25 10
256 10 9.00 -210.00 1399.25 10
257 10 10.00 -210.00 1399.25 10
258 10 11.00 -210.00 1399.25 10
259 10 12.00 -210.00 1399.25 10
260 10 13.00 -210.00 1399.25 10
261 10 14.00 -210.00 1399.25 10
262 10 15.00 -210.00 1399.25 10
263 10 16.00 -210.00 1399.25 10
264 10 17.00 -210.00 1399.25 10
265 10 18.00 -210.00 1399.25 10
266 10 19.00 -210.00 1399.25 10
267 10 20.00 -210.00 1399.25 10
268 10 21.00 -210.00 1399.25 10
269 10 22.00 -210.00 1399.25 10
270 10 23.00 -210.00 1399.25 10
271 10 24.00 -210.00 1399.25 10
272 10 25.00 -210.00 1399.25 10
273 10 26.00 -210.00 1399.25 10
274 10 27.00 -210.00 1399.25 10
275 10 28.00 -210.00 1399.25 10
276 10 29.00 -210.00 1399.25 10
277 10 30.00 -210.00 1399.25 10
278 10 31.00 -210.00 1399.25 10
279 10 32.00 -210.00 1399.25 10
280 112 1.00 -210.00 1399.25 10
281 112 2.00 -210.00 1399.25 10
282 112 3.00 -210.00 1399.25 10
283 112 4.00 -210.00 1399.25 10
284 112 5.00 -210.00 1399.25 10
285 112 6.00 -210.00 1399.25 10
286 112 7.00 -210.00 1399.25 10
287 112 8.00 -210.00 1399.25 10
288 112 9.00 -210.00 1399.25 10 -223.13 1997.89 0.000 100.000
289 112 10.00 -210.00 1399.25 10 -223.13 2096.69 0.000 98.800
290 112 11.00 -210.00 1399.25 10 -223.13 2194.29 0.000 97.600
291 112 12.00 -210.00 1399.25 10 -223.13 2290.69 0.000 96.400
292 112 13.00 -210.00 1399.25 10 -223.13 2385.89 0.000 95.200
293 112 14.00 -210.00 1399.25 10 -223.13 2479.89 0.000 94.000
294 112 15.00 -210.00 1399.25 10 -223.13 2572.69 0.000 92.800
295 112 16.00 -210.00 1399.25 10 -223.13 2664.29 0.000 91.600
296 112 17.00 -210.00 1399.25 10 -223.13 2754.69 0.000 90.400
297 112 18.00 -210.00 1399.25 10 -223.13 2843.89 0.000 89.200
298 112 19.00 -210.00 1399.25 10 -223.13 2931.89 0.000 88.000
299 112 20.00 -210.00 1399.25 10 -223.13 3018.69 0.000 86.800
300 112 21.00 -210.00 1399.25 10 -223.13 3104.29 0.000 85.600
301 112 22.00 -210.00 1399.25 10 -223.13 3188.69 0.000 84.400
302 112 23.00 -210.00 1399.25 10 -223.13 3271.89 0.000 83.200
303 10 0.00 -210.00 1399.25 10 -223.13 3353.89 0.000 82.000
304 10 1.00 -210.00 1399.25 10 -223.13 3434.69 0.000 80.800
305 10 2.00 -210.00 1399.25 10 -223.13 3514.29 0.000 79.600
306 10 3.00 -210.00 1399.25 10 -223.13 3592.69 0.000 78.400
307 10 4.00 -210.00 1399.25 10 -223.13 3669.89 0.000 77.200
308 10 5.00 -210.00 1399.25 10 -223.13 3745.89 0.000 76.000
309 10 6.00 -210.00 1399.25 10 -223.13 3820.69 0.000 74.800
310 10 7.00 -210.00 1399.25 10 -223.13 3894.29 0.000 73.600
311 10 8.00 -210.00 1399.25 10 -223.13 3966.69 0.000 72.400
312 10 9.00 -210.00 1399.25 10 -223.13 4037.89 0.000 71.200
313 10 10.00 -210.00 1399.25 10 -223.13 4107.89 0.000 70.000
314 10 11.00 -210.00 1399.25 10 -223.13 4176.69 0.000 68.800
315 10 12.00 -210.00 1399.25 10 -223.13 4244.29 0.000 67.600
316 10 13.00 -210.00 1399.25 10 -223.13 4310.69 0.000 66.400
317 10 14.00 -210.00 1399.25 10 -223.13 4375.89 0.000 65.200
318 10 15.00 -210.00 1399.25 10 -223.13 4439.89 0.000 64.000
319 10 16.00 -210.00 1399.25 10 -223.13 4502.69 0.000 62.800
320 10 17.00 -210.00 1399.25 10 -223.13 4564.29 0.000 61.600
321 10 18.00 -210.00 1399.25 10 -223.13 4624.69 0.000 60.400
322 10 19.00 -210.00 1399.25 10 -223.13 4683.90 0.000 59.200
323 10 20.00 -210.00 1399.25 10 -223.13 4741.90 0.000 58.000
324 10 21.00 -210.00 1399.25 10 -223.13 4798.70 0.000 56.800
325 10 22.00 -210.00 1399.25 10 -223.13 4854.30 0.000 55.600
326 10 23.00 -210.00 1399.25 10 -223.13 4908.70 0.000 54.400
327 10 24.00 -210.00 1399.25 10 -223.13 4961.90 0.000 53.200
328 10 25.00 -210.00 1399.25 10 -223.13 5013.90 0.000 52.000
329 10 26.00 -210.00 1399.25 10 -223.13 5064.70 0.000 50.800
330 10 27.00 -210.00 1399.25 10 -223.13 5114.30 0.000 49.600
331 10 28.00 -210.00 1399.25 10 -223.13 5162.70 0.000 48.400
332 10 29.00 -210.00 1399.25 10 -223.13 5209.90 0.000 47.200
333 10 30.00 -210.00 1399.25 10 -223.13 5255.90 0.000 46.000
334 10 31.00 -210.00 1399.25 10 -223.13 5300.70 0.000 44.800
335 10 32.00 -210.00 1399.25 10 -223.13 5344.30 0.000 43.600
336 10 33.00 -210.00 1399.25 10 -223.13 5386.70 0.000 42.400
337 10 34.00 -210.00 1399.25 10 -223.13 5427.90 0.000 41.200
338 10 35.00 -210.00 1399.25 10 -223.13 5467.90 0.000 40.000
339 10 36.00 -210.00 1399.25 10 -223.13 5506.70 0.000 38.800
340 10 37.00 -210.00 1399.25 10 -223.13 5544.30 0.000 37.600
";

#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn link_smash_throws_beam_swords_back_and_up_as_on_the_n64() {
    const Z: u16 = 0x2000;
    let pad = |t: u64| match t {
        160 | 240 => (button::A, 0, 0),
        200 => (button::A | Z, -128, 0),
        280 => (button::A | Z, 0, 127),
        _ => (0, 0, 0),
    };
    let Some((first_bad, log, max_err)) =
        castle_item_replay(&pad, 7, &[90, 100], LINK_SWORD_THROWS_B_HI)
    else {
        return;
    };
    if std::env::var_os("SCENE_TRACE").is_some() {
        println!("Link's Beam Sword back and up throws (max position error {max_err}):\n{log}");
    }
    assert_eq!(first_bad, None, "{log}");
}

/// N64 rows of `b30set/fb1` (`warp_mario`, the `warp_link` copy with
/// Mario as P1): route `100:2:x=-128;130:1:b`. Mario turns left and fires;
/// the Fireball bounces off the Castle's left end and falls. From t=231 its
/// speed passes the terminal 55 and `wpMainApplyGravityClampTVel` scales
/// both axes (x -30.343 then -28.388, -26.337, ...). Columns: `t st af x y`,
/// then the Fireball's `wx wy` while it lives (t=145-283).
const MARIO_FIREBALL_FALL: &str = "
100 18 1.00 -210.00 1399.25
101 18 2.00 -210.00 1399.25
102 18 3.00 -210.00 1399.25
103 18 4.00 -210.00 1399.25
104 18 5.00 -210.00 1399.25
105 18 6.00 -210.00 1399.25
106 18 7.00 -210.00 1399.25
107 18 8.00 -210.00 1399.25
108 18 9.00 -210.00 1399.25
109 18 10.00 -210.00 1399.25
110 18 11.00 -210.00 1399.25
111 10 0.00 -210.00 1399.25
112 10 1.00 -210.00 1399.25
113 10 2.00 -210.00 1399.25
114 10 3.00 -210.00 1399.25
115 10 4.00 -210.00 1399.25
116 10 5.00 -210.00 1399.25
117 10 6.00 -210.00 1399.25
118 10 7.00 -210.00 1399.25
119 10 8.00 -210.00 1399.25
120 10 9.00 -210.00 1399.25
121 10 10.00 -210.00 1399.25
122 10 11.00 -210.00 1399.25
123 10 12.00 -210.00 1399.25
124 10 13.00 -210.00 1399.25
125 10 14.00 -210.00 1399.25
126 10 15.00 -210.00 1399.25
127 10 16.00 -210.00 1399.25
128 10 17.00 -210.00 1399.25
129 10 18.00 -210.00 1399.25
130 223 1.00 -210.00 1399.25
131 223 2.00 -210.00 1399.25
132 223 3.00 -210.00 1399.25
133 223 4.00 -210.00 1399.25
134 223 5.00 -210.00 1399.25
135 223 6.00 -210.00 1399.25
136 223 7.00 -210.00 1399.25
137 223 8.00 -210.00 1399.25
138 223 9.00 -210.00 1399.25
139 223 10.00 -210.00 1399.25
140 223 11.00 -210.00 1399.25
141 223 12.00 -210.00 1399.25
142 223 13.00 -210.00 1399.25
143 223 14.00 -210.00 1399.25
144 223 15.00 -210.00 1399.25
145 223 16.00 -210.00 1399.25 -442.14 1611.90
146 223 17.00 -210.00 1399.25 -491.95 1605.14
147 223 18.00 -210.00 1399.25 -541.76 1597.18
148 223 19.00 -210.00 1399.25 -591.57 1588.03
149 223 20.00 -210.00 1399.25 -641.38 1577.67
150 223 21.00 -210.00 1399.25 -691.19 1566.11
151 223 22.00 -210.00 1399.25 -741.00 1553.35
152 223 23.00 -210.00 1399.25 -790.81 1539.39
153 223 24.00 -210.00 1399.25 -840.62 1524.24
154 223 25.00 -210.00 1399.25 -890.43 1507.88
155 223 26.00 -210.00 1399.25 -940.24 1490.32
156 223 27.00 -210.00 1399.25 -990.05 1471.56
157 223 28.00 -210.00 1399.25 -1039.86 1451.61
158 223 29.00 -210.00 1399.25 -1089.67 1450.62
159 223 30.00 -210.00 1399.25 -1131.95 1467.54
160 223 31.00 -210.00 1399.25 -1174.23 1483.25
161 223 32.00 -210.00 1399.25 -1216.51 1497.77
162 223 33.00 -210.00 1399.25 -1258.79 1511.08
163 223 34.00 -210.00 1399.25 -1301.08 1523.20
164 223 35.00 -210.00 1399.25 -1343.36 1534.12
165 223 36.00 -210.00 1399.25 -1385.64 1543.83
166 223 37.00 -210.00 1399.25 -1427.92 1552.35
167 223 38.00 -210.00 1399.25 -1470.21 1559.66
168 223 39.00 -210.00 1399.25 -1512.49 1565.78
169 223 40.00 -210.00 1399.25 -1554.77 1570.70
170 223 41.00 -210.00 1399.25 -1597.05 1574.41
171 223 42.00 -210.00 1399.25 -1639.33 1576.93
172 223 43.00 -210.00 1399.25 -1681.62 1578.24
173 223 44.00 -210.00 1399.25 -1723.90 1578.36
174 223 45.00 -210.00 1399.25 -1766.18 1577.28
175 10 0.00 -210.00 1399.25 -1808.46 1575.93
176 10 1.00 -210.00 1399.25 -1838.81 1594.09
177 10 2.00 -210.00 1399.25 -1869.15 1611.04
178 10 3.00 -210.00 1399.25 -1899.49 1626.80
179 10 4.00 -210.00 1399.25 -1929.84 1641.36
180 10 5.00 -210.00 1399.25 -1960.18 1654.72
181 10 6.00 -210.00 1399.25 -1990.52 1666.87
182 10 7.00 -210.00 1399.25 -2020.87 1677.83
183 10 8.00 -210.00 1399.25 -2051.21 1687.59
184 10 9.00 -210.00 1399.25 -2081.55 1696.15
185 10 10.00 -210.00 1399.25 -2111.90 1703.50
186 10 11.00 -210.00 1399.25 -2142.24 1709.66
187 10 12.00 -210.00 1399.25 -2172.58 1714.62
188 10 13.00 -210.00 1399.25 -2202.93 1718.38
189 10 14.00 -210.00 1399.25 -2233.27 1720.93
190 10 15.00 -210.00 1399.25 -2263.61 1722.29
191 10 16.00 -210.00 1399.25 -2293.96 1722.45
192 10 17.00 -210.00 1399.25 -2324.30 1721.41
193 10 18.00 -210.00 1399.25 -2354.64 1719.16
194 10 19.00 -210.00 1399.25 -2384.99 1715.72
195 10 20.00 -210.00 1399.25 -2415.33 1711.08
196 10 21.00 -210.00 1399.25 -2445.68 1705.24
197 10 22.00 -210.00 1399.25 -2476.02 1698.19
198 10 23.00 -210.00 1399.25 -2506.36 1689.95
199 10 24.00 -210.00 1399.25 -2536.71 1680.51
200 10 25.00 -210.00 1399.25 -2567.05 1669.87
201 10 26.00 -210.00 1399.25 -2597.39 1658.02
202 10 27.00 -210.00 1399.25 -2627.74 1644.98
203 10 28.00 -210.00 1399.25 -2658.08 1630.74
204 10 29.00 -210.00 1399.25 -2688.42 1615.30
205 10 30.00 -210.00 1399.25 -2718.77 1598.65
206 10 31.00 -210.00 1399.25 -2749.11 1580.81
207 10 32.00 -210.00 1399.25 -2779.45 1561.77
208 10 33.00 -210.00 1399.25 -2809.80 1541.53
209 10 34.00 -210.00 1399.25 -2840.14 1520.08
210 10 35.00 -210.00 1399.25 -2870.48 1497.44
211 10 36.00 -210.00 1399.25 -2900.83 1473.60
212 10 37.00 -210.00 1399.25 -2931.17 1448.56
213 10 38.00 -210.00 1399.25 -2961.51 1422.31
214 10 39.00 -210.00 1399.25 -2991.86 1394.87
215 10 40.00 -210.00 1399.25 -3022.20 1366.23
216 10 41.00 -210.00 1399.25 -3052.55 1336.39
217 10 42.00 -210.00 1399.25 -3082.89 1305.34
218 10 43.00 -210.00 1399.25 -3113.23 1273.10
219 10 44.00 -210.00 1399.25 -3143.58 1239.66
220 10 45.00 -210.00 1399.25 -3173.92 1205.02
221 10 46.00 -210.00 1399.25 -3204.26 1169.17
222 10 47.00 -210.00 1399.25 -3234.61 1132.13
223 10 48.00 -210.00 1399.25 -3264.95 1093.89
224 10 49.00 -210.00 1399.25 -3295.29 1054.45
225 10 0.00 -210.00 1399.25 -3325.64 1013.80
226 10 1.00 -210.00 1399.25 -3355.98 971.96
227 10 2.00 -210.00 1399.25 -3386.32 928.92
228 10 3.00 -210.00 1399.25 -3416.67 884.68
229 10 4.00 -210.00 1399.25 -3447.01 839.23
230 10 5.00 -210.00 1399.25 -3477.00 793.13
231 10 6.00 -210.00 1399.25 -3506.45 746.68
232 10 7.00 -210.00 1399.25 -3535.37 699.90
233 10 8.00 -210.00 1399.25 -3563.76 652.79
234 10 9.00 -210.00 1399.25 -3591.63 605.37
235 10 10.00 -210.00 1399.25 -3618.97 557.65
236 10 11.00 -210.00 1399.25 -3645.81 509.65
237 10 12.00 -210.00 1399.25 -3672.15 461.36
238 10 13.00 -210.00 1399.25 -3697.99 412.81
239 10 14.00 -210.00 1399.25 -3723.34 364.00
240 10 15.00 -210.00 1399.25 -3748.21 314.94
241 10 16.00 -210.00 1399.25 -3772.60 265.65
242 10 17.00 -210.00 1399.25 -3796.53 216.12
243 10 18.00 -210.00 1399.25 -3819.99 166.38
244 10 19.00 -210.00 1399.25 -3843.00 116.42
245 10 20.00 -210.00 1399.25 -3865.55 66.26
246 10 21.00 -210.00 1399.25 -3887.67 15.90
247 10 22.00 -210.00 1399.25 -3909.36 -34.64
248 10 23.00 -210.00 1399.25 -3930.61 -85.37
249 10 24.00 -210.00 1399.25 -3951.45 -136.27
250 10 25.00 -210.00 1399.25 -3971.87 -187.33
251 10 26.00 -210.00 1399.25 -3991.89 -238.56
252 10 27.00 -210.00 1399.25 -4011.51 -289.95
253 10 28.00 -210.00 1399.25 -4030.73 -341.48
254 10 29.00 -210.00 1399.25 -4049.57 -393.15
255 10 30.00 -210.00 1399.25 -4068.03 -444.96
256 10 31.00 -210.00 1399.25 -4086.12 -496.90
257 10 32.00 -210.00 1399.25 -4103.85 -548.96
258 10 33.00 -210.00 1399.25 -4121.21 -601.15
259 10 34.00 -210.00 1399.25 -4138.22 -653.45
260 10 35.00 -210.00 1399.25 -4154.89 -705.87
261 10 36.00 -210.00 1399.25 -4171.21 -758.39
262 10 37.00 -210.00 1399.25 -4187.21 -811.01
263 10 38.00 -210.00 1399.25 -4202.87 -863.74
264 10 39.00 -210.00 1399.25 -4218.21 -916.55
265 10 40.00 -210.00 1399.25 -4233.24 -969.46
266 10 41.00 -210.00 1399.25 -4247.96 -1022.45
267 10 42.00 -210.00 1399.25 -4262.38 -1075.53
268 10 43.00 -210.00 1399.25 -4276.50 -1128.69
269 10 44.00 -210.00 1399.25 -4290.32 -1181.92
270 10 45.00 -210.00 1399.25 -4303.86 -1235.23
271 10 46.00 -210.00 1399.25 -4317.12 -1288.60
272 10 47.00 -210.00 1399.25 -4330.11 -1342.05
273 10 48.00 -210.00 1399.25 -4342.82 -1395.56
274 10 49.00 -210.00 1399.25 -4355.27 -1449.13
275 10 0.00 -210.00 1399.25 -4367.47 -1502.76
276 10 1.00 -210.00 1399.25 -4379.40 -1556.45
277 10 2.00 -210.00 1399.25 -4391.09 -1610.20
278 10 3.00 -210.00 1399.25 -4402.54 -1663.99
279 10 4.00 -210.00 1399.25 -4413.74 -1717.84
280 10 5.00 -210.00 1399.25 -4424.71 -1771.73
281 10 6.00 -210.00 1399.25 -4435.45 -1825.68
282 10 7.00 -210.00 1399.25 -4445.97 -1879.66
283 10 8.00 -210.00 1399.25 -4456.26 -1933.69
284 10 9.00 -210.00 1399.25
285 10 10.00 -210.00 1399.25
286 10 11.00 -210.00 1399.25
287 10 12.00 -210.00 1399.25
288 10 13.00 -210.00 1399.25
289 10 14.00 -210.00 1399.25
290 10 15.00 -210.00 1399.25
291 10 16.00 -210.00 1399.25
292 10 17.00 -210.00 1399.25
293 10 18.00 -210.00 1399.25
294 10 19.00 -210.00 1399.25
295 10 20.00 -210.00 1399.25
296 10 21.00 -210.00 1399.25
297 10 22.00 -210.00 1399.25
298 10 23.00 -210.00 1399.25
299 10 24.00 -210.00 1399.25
300 10 25.00 -210.00 1399.25
";

#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn a_falling_fireball_clamps_its_speed_as_on_the_n64() {
    let pad = |t: u64| match t {
        100 | 101 => (0, -128, 0),
        130 => (button::B, 0, 0),
        _ => (0, 0, 0),
    };
    let Some((first_bad, log, max_err)) = replay_status_rows(
        FighterKind::Mario,
        &pad,
        &|w| w.first_fireball().map(|f| [f.position.x, f.position.y]),
        MARIO_FIREBALL_FALL,
        100,
    ) else {
        return;
    };
    if std::env::var_os("SCENE_TRACE").is_some() {
        println!("Mario's falling Fireball (max position error {max_err}):\n{log}");
    }
    assert_eq!(first_bad, None, "{log}");
}

/// `b30set/jg3` (`warp_jungle_mario`, Training on Kongo Jungle, Mario P1
/// at x -1739, the dummy Mario far right): route `175:200:x=-128`, N64
/// frames 1-520. The Barrel (`grJungleMakeTaruCann`: from seed 1, wait
/// 180) sweeps its default path at y -1597.5; it turns (rotate step 0.07)
/// from frame 179 to 269. Mario dashes left off the ledge and falls; at
/// frame 226 his TopN is within 280 of the Barrel, so
/// `ftCommonTaruCannSetStatus` takes him (61). The status has no motion
/// (Script ID -1): the Fall clip plays on, and the setter's
/// `ftMainPlayAnimEventsAll` advances it once more (8 -> 9 -> 0, its
/// ten-frame loop). He rides the Barrel (`ftCommonTaruCannProcPhysics`)
/// until `release_wait` reaches 180 (frame 406) and `shoot_wait` 10 runs
/// out: the shot (`DamageFlyRoll`, 55) at frame 416, straight up (rotate
/// 0). Per row: tick, Mario's status,
/// `anim_frame`, x and y, then the Barrel's status, wait, `rotate.z` and
/// x; the rows start at the walk (the opening Wait's `anim_frame` runs one
/// ahead on the N64).
const JUNGLE_BARREL: &str = "
175 18 1.00 -1739.00 0.00 0 4 0.00000 -2071.224
176 18 2.00 -1739.00 0.00 0 3 0.00000 -2106.435
177 18 3.00 -1739.00 0.00 0 2 0.00000 -2141.356
180 15 1.00 -1793.00 0.00 1 89 0.07000 -2244.331
187 26 0.00 -2162.60 0.00 1 82 0.56000 -2473.466
188 26 1.00 -2192.40 -2.40 1 81 0.63000 -2504.845
197 26 -0.00 -2460.60 -132.00 1 72 1.26000 -2770.631
200 26 3.00 -2550.00 -218.40 1 69 1.47000 -2852.142
225 26 8.00 -3295.00 -1290.40 1 44 3.22000 -3369.938
226 61 -0.00 -3324.80 -1334.40 1 43 3.29000 -3383.961
227 61 1.00 -3397.42 -1597.50 1 42 3.36000 -3397.417
228 61 2.00 -3410.31 -1597.50 1 41 3.43000 -3410.307
236 61 -0.00 -3492.25 -1597.50 1 33 3.99000 -3492.254
268 61 2.00 -3411.31 -1597.50 1 1 6.23000 -3411.314
269 61 3.00 -3398.52 -1597.50 0 254 0.00000 -3398.523
270 61 4.00 -3385.17 -1597.50 0 253 0.00000 -3385.167
300 61 4.00 -2748.50 -1597.50 0 223 0.00000 -2748.497
414 61 8.00 1679.22 -1597.50 0 109 0.00000 1679.221
415 61 9.00 1717.02 -1597.50 0 108 0.00000 1717.019
416 55 1.00 1717.02 -1421.60 0 107 0.00000 1754.588
417 55 2.00 1717.02 -1249.80 0 106 0.00000 1791.923
418 55 3.00 1717.02 -1082.10 0 105 0.00000 1829.019
420 55 5.00 1717.02 -759.00 0 103 0.00000 1902.472
428 55 13.00 1717.02 369.40 0 95 0.00000 2185.625
458 55 11.00 1717.02 3023.90 0 65 0.00000 3054.257
498 55 3.00 1717.02 4145.91 0 25 0.00000 3540.000
520 55 9.00 1717.02 3603.62 0 3 0.00000 3383.960
";

/// The seeds the N64's frames 180 and 184 left: Mario's dash dust
/// (effect 11, its scatter) draws three values each frame, which the
/// effect manager makes on the PSP and this harness does not. The idle
/// trace `b30set/jg0` steps only for the Barrel. The Barrel's next draw
/// (frame 269, wait 74 + 180) reads frame 184's seed.
const JUNGLE_DUST_SEEDS: [(u64, u32); 2] = [(180, 0xCAE1_DF84), (184, 0x72AE_7869)];

/// Replays Mario into Jungle's Barrel. The Barrel's root plays its
/// default path from the pack ([`ssb_rom::ground_obj::GroundObjects`],
/// `gcPlayAnimAll` before the fighters); the stage reads it through
/// [`Pair::barrel`]. Returns the first bad tick and the log.
fn jungle_barrel_replay(table: &str) -> Option<(Option<u64>, String)> {
    use ssb_game::stage::Controller;
    use ssb_rom::ground_obj as g;
    let bytes = open_pack()?;
    let pack = ssb_rom::pack::Pack::open(&bytes).unwrap();
    let gkind = 2;
    let index = pack
        .stage_of_file(ssb_rom::stage::VS_GROUND_FILES[gkind])
        .unwrap();
    let source = pack.stage(index).unwrap().source_file;
    let mut objects = Box::new(g::GroundObjects::new(&pack, source));
    objects.play(&pack, g::TARUCANN_DEFAULT).unwrap();
    let barrel = |o: &g::GroundObjects| {
        let [x, y, z] = o.instance(g::TARUCANN, 0).unwrap().translate();
        ssb_engine::math::Vec3::new(x, y, z)
    };
    if std::env::var_os("SCENE_TRACE").is_some() {
        println!("barrel at make: {:?}", barrel(&objects));
    }
    let (map, mut fighters, mut pair) = hazard_pair(&pack, gkind);
    let pad = |t: u64| match t {
        175..=374 => (0, -128, 0),
        _ => (0, 0, 0),
    };
    let want: Vec<Vec<f32>> = rows(table).collect();
    let last = want.last().unwrap()[0] as u64;
    let mut log = Vec::new();
    let mut first_bad = None;
    // N64 frame 0 already played the path once (the pair's pre-tick).
    objects.advance(&pack).unwrap();
    for tick in 1..=last {
        objects.advance(&pack).unwrap();
        pair.barrel = Some(barrel(&objects));
        let _ = tick_pair(&pack, &map, &mut fighters, &pad, tick, &mut pair);
        if let Some(&(_, seed)) = JUNGLE_DUST_SEEDS.iter().find(|(t, _)| *t == tick) {
            ssb_game::rng::set_seed(seed as i32);
        }
        let Some(row) = want.iter().find(|r| r[0] as u64 == tick) else {
            continue;
        };
        let f = &fighters[0].fighter;
        let mut got = vec![
            f32::from(f.status.status.id()),
            f.status.anim_frame,
            f.pos.x,
            f.pos.y,
        ];
        if let Controller::Jungle(j) = &pair.stage.controller {
            use ssb_game::stage::jungle::BarrelStatus as B;
            got.extend([
                f32::from(u8::from(j.status == B::Rotate)),
                f32::from(j.wait),
                j.rotate,
                pair.barrel.unwrap().x,
            ]);
        }
        let ok = got.len() + 1 == row.len() && got.iter().zip(&row[1..]).all(|(g, w)| near(*g, *w));
        log.push(format!(
            "{tick} {} rust {got:?} seed {:08x}\n{tick} {}  n64 {:?}",
            if ok { " " } else { "X" },
            ssb_game::rng::seed(),
            if ok { " " } else { "X" },
            &row[1..]
        ));
        if !ok && first_bad.is_none() {
            first_bad = Some(tick);
        }
    }
    Some((first_bad, log.join("\n")))
}

#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn the_barrel_takes_and_shoots_mario_as_on_the_n64() {
    let Some((first_bad, log)) = jungle_barrel_replay(JUNGLE_BARREL) else {
        return;
    };
    if std::env::var_os("SCENE_TRACE").is_some() {
        println!("Mario in Jungle's Barrel:\n{log}");
    }
    assert_eq!(first_bad, None, "{log}");
}
