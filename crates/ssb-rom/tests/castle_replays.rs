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
use posed::{tick_pair, tick_pair_pads, HostCpu, Pair, Posed};

#[path = "common/dream_land.rs"]
mod dream_land;
use dream_land::{
    dream_land_map, map_groups, stage_bounds, stage_floors, training_fighter, vs_fighter,
};

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
/// `anim_frame`, x and y with each `t st af x y` row from tick `from` on;
/// the drawn clip's frame (`GObj::anim_frame`, one value on the N64) must
/// match `af` too. A row with two more values also holds the x and y of P1's weapon
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
        let clip = fighters[0].skeleton.frame();
        let ok = got.len() + 1 == row.len()
            && got.iter().zip(&row[1..]).all(|(g, w)| near(*g, *w))
            && near(clip, row[2]);
        max_err = got[2..]
            .iter()
            .zip(&row[3..])
            .fold(max_err, |m, (g, w)| m.max((g - w).abs()));
        log.push(format!(
            "{tick} {} rust {got:?} clip {clip:?}\n{tick} {}  n64 {:?}",
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

/// `b30set/kair1` (`warp_kirby`): route `9:8:x=127;50:1:c_u;56:60:b`, N64
/// frames 49-140. Kirby dashes to x 498, jumps, inhales in the air
/// (`SpecialAirNStart` 278, `SpecialAirNLoop` 279) and catches the dummy
/// Mario on the way down at frame 91: `SpecialAirNCatch` (281) has no
/// figatree, so the loop's runs on, and the setter's
/// `ftMainPlayAnimEventsAll` steps it once more (4 to the loop's end, then
/// -0). He lands holding Mario at frame 99 and
/// `ftKirbySpecialAirNCatchSwitchStatusGround` passes `anim_frame` (272 at
/// 2; motion -1, so the map step parses nothing and the drawn clip stays
/// on 2 too), then eats (273) and waits (275). Per row: tick, Kirby's status,
/// `anim_frame`, x, y; the opening Wait's `anim_frame` runs one ahead on the
/// N64, so the rows compare from the jump on.
const KIRBY_AIR_CATCH: &str = "
49 10 17.00 498.00 1398.15
50 20 0.00 498.00 1398.15
51 20 1.00 498.00 1398.15
52 20 2.00 498.00 1398.15
53 22 0.00 498.00 1452.75
54 22 1.00 498.00 1504.95
55 22 2.00 498.00 1554.75
56 278 1.00 498.00 1602.15
57 278 2.00 498.00 1647.15
58 278 3.00 498.00 1689.75
59 278 4.00 498.00 1729.95
60 278 5.00 498.00 1767.75
61 278 6.00 498.00 1803.15
62 278 7.00 498.00 1836.15
63 278 8.00 498.00 1866.75
64 278 9.00 498.00 1894.95
65 278 10.00 498.00 1920.75
66 278 11.00 498.00 1944.15
67 278 12.00 498.00 1965.15
68 278 13.00 498.00 1983.75
69 278 14.00 498.00 1999.95
70 278 15.00 498.00 2013.75
71 278 16.00 498.00 2025.15
72 278 17.00 498.00 2034.15
73 278 18.00 498.00 2040.75
74 278 19.00 498.00 2044.95
75 279 1.00 498.00 2046.75
76 279 2.00 498.00 2046.15
77 279 3.00 498.00 2043.15
78 279 4.00 498.00 2037.75
79 279 5.00 498.00 2029.95
80 279 -0.00 498.00 2019.75
81 279 1.00 498.00 2007.15
82 279 2.00 498.00 1992.15
83 279 3.00 498.00 1974.75
84 279 4.00 498.00 1954.95
85 279 5.00 498.00 1932.75
86 279 -0.00 498.00 1908.15
87 279 1.00 498.00 1881.15
88 279 2.00 498.00 1851.75
89 279 3.00 498.00 1819.95
90 279 4.00 498.00 1785.75
91 281 -0.00 498.00 1749.15
92 281 1.00 498.00 1710.15
93 281 2.00 498.00 1668.75
94 281 3.00 498.00 1624.95
95 281 4.00 498.00 1578.75
96 281 5.00 498.00 1530.75
97 281 -0.00 498.00 1482.75
98 281 1.00 498.00 1434.75
99 272 2.00 498.00 1398.15
100 272 3.00 498.00 1398.15
101 272 4.00 498.00 1398.15
102 272 5.00 498.00 1398.15
103 273 1.00 498.00 1398.15
104 273 2.00 498.00 1398.15
105 273 3.00 498.00 1398.15
106 273 4.00 498.00 1398.15
107 273 5.00 498.00 1398.15
108 273 6.00 498.00 1398.15
109 273 7.00 498.00 1398.15
110 273 8.00 498.00 1398.15
111 273 9.00 498.00 1398.15
112 273 10.00 498.00 1398.15
113 273 11.00 498.00 1398.15
114 273 12.00 498.00 1398.15
115 273 13.00 498.00 1398.15
116 273 14.00 498.00 1398.15
117 273 15.00 498.00 1398.15
118 273 16.00 498.00 1398.15
119 273 17.00 498.00 1398.15
120 273 18.00 498.00 1398.15
121 273 19.00 498.00 1398.15
122 275 1.00 498.00 1398.15
123 275 2.00 498.00 1398.15
124 275 3.00 498.00 1398.15
125 275 4.00 498.00 1398.15
126 275 5.00 498.00 1398.15
127 275 6.00 498.00 1398.15
128 275 7.00 498.00 1398.15
129 275 8.00 498.00 1398.15
130 275 9.00 498.00 1398.15
131 275 10.00 498.00 1398.15
132 275 11.00 498.00 1398.15
133 275 12.00 498.00 1398.15
134 275 13.00 498.00 1398.15
135 275 14.00 498.00 1398.15
136 275 15.00 498.00 1398.15
137 275 16.00 498.00 1398.15
138 275 17.00 498.00 1398.15
139 275 18.00 498.00 1398.15
140 275 19.00 498.00 1398.15
";

#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn kirby_air_inhale_catch_anim_frame_matches_the_n64() {
    let pad = |t: u64| match t {
        9..=16 => (0, 127, 0),
        50 => (button::C_UP, 0, 0),
        56..=115 => (button::B, 0, 0),
        _ => (0, 0, 0),
    };
    let Some((first_bad, log, max_err)) =
        replay_status_rows(FighterKind::Kirby, &pad, &|_| None, KIRBY_AIR_CATCH, 50)
    else {
        return;
    };
    if std::env::var_os("SCENE_TRACE").is_some() {
        println!("kirby air catch on the Castle (max position error {max_err}):\n{log}");
    }
    assert_eq!(first_bad, None, "{log}");
}

/// `sSYUtilsRandomSeed` fed from an N64 trace: the host plays no effect
/// makers, so their draws (dust, sparks) are missing; at `tick` the seed is
/// set to the N64's value `before` it, and the tick's own draws must leave
/// the N64's `after`.
#[derive(Clone, Copy)]
struct N64Seed {
    tick: u64,
    before: u32,
    after: u32,
}

/// Replays `kind` (port 0) against the dummy Mario (port 1) on the Castle
/// and compares both fighters' status, `anim_frame`, x and y per row
/// (tick, then the two quads) from `from` on, skipping a fighter's
/// `anim_frame` until it leaves the opening Wait. Returns the first differing
/// tick, the log and the largest position error.
fn replay_pair_rows(
    kind: FighterKind,
    pad: &dyn Fn(u64) -> (u16, i8, i8),
    table: &str,
    from: u64,
    seed: Option<N64Seed>,
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
    let mut pair = Pair {
        groups: Some((
            ssb_pose::ground::StageGroups::new(&pack, CASTLE, &stage),
            map_groups(&pack, &stage),
        )),
        ..Pair::default()
    };
    pair.cpu = Some(HostCpu::training_dummy(
        &fighters[1].fighter,
        &stage,
        stage_bounds(&pack, &stage),
        &map,
        pair.groups
            .as_ref()
            .map(|(g, y)| (g.groups.as_slice(), y.as_slice())),
    ));
    // The program's seed 1; the effect makers' draws are not played (see
    // [`N64Seed`]).
    ssb_game::rng::set_seed(1);
    // N64 frame 0 already played the Ground link's `gcPlayAnimAll`: the
    // castle's moving groups (the side lenses 3 and 4, whose walls a
    // thrown fighter strikes) stand one step on at the replay's tick 1
    // (the trace's group 2 x reads 1796.3964 at frame 0: the creation
    // pose's 1800 less one step).
    if let Some((groups, _)) = pair.groups.as_mut() {
        groups.tick(&pack).unwrap();
    }
    let mut log = Vec::new();
    let mut first_bad = None;
    let mut max_err = 0.0f32;
    let mut left_opening = [false; 2];
    for tick in 1..=last {
        let fed = seed.filter(|s| s.tick == tick);
        if let Some(s) = fed {
            ssb_game::rng::set_seed(s.before as i32);
        }
        let _ = tick_pair(&pack, &map, &mut fighters, pad, tick, &mut pair);
        if let Some(s) = fed {
            assert_eq!(
                ssb_game::rng::seed() as u32,
                s.after,
                "tick {tick}: the frame's draws leave the N64's seed"
            );
        }
        let got: Vec<f32> = fighters
            .iter()
            .flat_map(|p| {
                let f = &p.fighter;
                [
                    f32::from(f.status.status.id()),
                    f.status.anim_frame,
                    f.pos.x,
                    f.pos.y,
                ]
            })
            .collect();
        let Some(row) = want.iter().find(|r| r[0] as u64 == tick && tick >= from) else {
            continue;
        };
        // The opening Wait's `anim_frame` runs one ahead on the N64: skip it
        // until the fighter first leaves it.
        let mut skip = [false; 8];
        for (i, p) in fighters.iter().enumerate() {
            left_opening[i] |= p.fighter.status.status != ssb_game::status::Status::Wait;
            skip[i * 4 + 1] = !left_opening[i];
        }
        let ok = got.len() + 1 == row.len()
            && got
                .iter()
                .zip(&row[1..])
                .zip(skip)
                .all(|((g, w), skip)| skip || near(*g, *w));
        for i in [2, 3, 6, 7] {
            max_err = max_err.max((got[i] - row[i + 1]).abs());
        }
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
    Some((first_bad, log.join("\n"), max_err))
}

/// `b30set/ygrab1` (`warp_yoshi`): route `9:8:x=127;60:3:z;61:1:z+a;90:1:a`,
/// N64 frames 58-160. Yoshi dashes to x 448.6, shields (`GuardOn` 152) and
/// grabs out of it (`ftCommonCatchCheckInterruptGuard`, `Catch` 166); his
/// tongue reaches the dummy Mario 384 away at frame 75 (`CatchPull` 167,
/// Mario `CapturePulled` 171), holds him (`CatchWait` 168, `CaptureWait`
/// 172; Mario's TopN stays at the pull's place) and the A tap throws forward
/// at frame 90 (`ThrowF` 169, Mario `Thrown` 186), releasing him into
/// `DamageFlyN` (52) at frame 107. At frame 144 Mario strikes the lower
/// left side (line 12) of the right lens (moving group 3) head-on
/// (`mpCommonProcFighterDamage`: `WallDamage`, 56, at x 2701.22), bounces
/// back, falls as `DamageFall` (57) from 157, slides down that side and
/// drops off its tip at 163. Per row:
/// tick, then Yoshi's and Mario's status, `anim_frame`, x and y; the
/// opening Wait's `anim_frame` runs one ahead on the N64, so the rows
/// compare from the shield on.
const YOSHI_GRAB_THROW_F: &str = "
58 10 23.00 448.60 1398.23 10 9.00 832.50 1419.04
59 10 24.00 448.60 1398.23 10 10.00 832.50 1419.04
60 152 1.00 448.60 1398.23 10 11.00 832.50 1419.04
61 166 1.00 448.60 1398.23 10 12.00 832.50 1419.04
62 166 2.00 448.60 1398.23 10 13.00 832.50 1419.04
63 166 3.00 448.60 1398.23 10 14.00 832.50 1419.04
64 166 4.00 448.60 1398.23 10 15.00 832.50 1419.04
65 166 5.00 448.60 1398.23 10 16.00 832.50 1419.04
66 166 6.00 448.60 1398.23 10 17.00 832.50 1419.04
67 166 7.00 448.60 1398.23 10 18.00 832.50 1419.04
68 166 8.00 448.60 1398.23 10 19.00 832.50 1419.04
69 166 9.00 448.60 1398.23 10 20.00 832.50 1419.04
70 166 10.00 448.60 1398.23 10 21.00 832.50 1419.04
71 166 11.00 448.60 1398.23 10 22.00 832.50 1419.04
72 166 12.00 448.60 1398.23 10 23.00 832.50 1419.04
73 166 13.00 448.60 1398.23 10 24.00 832.50 1419.04
74 166 14.00 448.60 1398.23 10 25.00 832.50 1419.04
75 167 2.00 448.60 1398.23 171 1.00 1008.87 1434.61
76 168 0.00 448.60 1398.23 172 2.00 865.12 1421.92
77 168 0.00 448.60 1398.23 172 2.00 865.12 1421.92
78 168 0.00 448.60 1398.23 172 2.00 865.12 1421.92
79 168 0.00 448.60 1398.23 172 2.00 865.12 1421.92
80 168 0.00 448.60 1398.23 172 2.00 865.12 1421.92
81 168 0.00 448.60 1398.23 172 2.00 865.12 1421.92
82 168 0.00 448.60 1398.23 172 2.00 865.12 1421.92
83 168 0.00 448.60 1398.23 172 2.00 865.12 1421.92
84 168 0.00 448.60 1398.23 172 2.00 865.12 1421.92
85 168 0.00 448.60 1398.23 172 2.00 865.12 1421.92
86 168 0.00 448.60 1398.23 172 2.00 865.12 1421.92
87 168 0.00 448.60 1398.23 172 2.00 865.12 1421.92
88 168 0.00 448.60 1398.23 172 2.00 865.12 1421.92
89 168 0.00 448.60 1398.23 172 2.00 865.12 1421.92
90 169 1.00 448.60 1398.23 186 2.00 774.82 1575.87
91 169 2.00 448.60 1398.23 186 3.00 701.17 1592.19
92 169 3.00 448.60 1398.23 186 4.00 698.70 1610.00
93 169 4.00 448.60 1398.23 186 5.00 700.82 1629.15
94 169 5.00 448.60 1398.23 186 6.00 708.16 1648.05
95 169 6.00 448.60 1398.23 186 7.00 719.10 1667.72
96 169 7.00 448.60 1398.23 186 8.00 733.03 1687.84
97 169 8.00 448.60 1398.23 186 9.00 749.34 1708.10
98 169 9.00 448.60 1398.23 186 10.00 767.42 1728.20
99 169 10.00 448.60 1398.23 186 11.00 786.64 1747.82
100 169 11.00 448.60 1398.23 186 12.00 806.41 1766.64
101 169 12.00 448.60 1398.23 186 13.00 826.10 1784.36
102 169 13.00 448.60 1398.23 186 14.00 845.11 1800.66
103 169 14.00 448.60 1398.23 186 15.00 862.82 1815.23
104 169 15.00 448.60 1398.23 186 16.00 878.61 1827.76
105 169 16.00 448.60 1398.23 186 17.00 891.88 1839.29
106 169 17.00 448.60 1398.23 186 18.00 902.02 1847.94
107 169 18.00 448.60 1398.23 186 19.00 908.40 1853.39
108 169 19.00 448.60 1398.23 52 2.00 995.67 1642.81
109 169 20.00 448.60 1398.23 52 3.00 1064.56 1706.91
110 169 21.00 448.60 1398.23 52 4.00 1132.26 1767.40
111 169 22.00 448.60 1398.23 52 5.00 1198.75 1824.29
112 169 23.00 448.60 1398.23 52 6.00 1264.04 1877.58
113 169 24.00 448.60 1398.23 52 7.00 1328.12 1927.26
114 169 25.00 448.60 1398.23 52 8.00 1391.01 1973.35
115 169 26.00 448.60 1398.23 52 9.00 1452.69 2015.83
116 169 27.00 448.60 1398.23 52 10.00 1513.17 2054.71
117 169 28.00 448.60 1398.23 52 11.00 1572.45 2089.99
118 169 29.00 448.60 1398.23 52 12.00 1630.52 2121.67
119 169 30.00 448.60 1398.23 52 13.00 1687.40 2149.74
120 169 31.00 448.60 1398.23 52 14.00 1743.07 2174.21
121 169 32.00 448.60 1398.23 52 15.00 1797.54 2195.08
122 169 33.00 448.60 1398.23 52 16.00 1850.81 2212.35
123 169 34.00 448.60 1398.23 52 17.00 1902.87 2226.01
124 169 35.00 448.60 1398.23 52 18.00 1953.74 2236.08
125 169 36.00 448.60 1398.23 52 19.00 2003.40 2242.54
126 169 37.00 448.60 1398.23 52 20.00 2051.86 2247.00
127 169 38.00 448.60 1398.23 52 21.00 2099.11 2250.26
128 10 0.00 448.60 1398.23 52 22.00 2145.17 2252.31
129 10 1.00 448.60 1398.23 52 23.00 2190.02 2253.17
130 10 2.00 448.60 1398.23 52 24.00 2233.67 2252.82
131 10 3.00 448.60 1398.23 52 25.00 2276.12 2251.27
132 10 4.00 448.60 1398.23 52 26.00 2317.37 2248.51
133 10 5.00 448.60 1398.23 52 27.00 2357.42 2244.56
134 10 6.00 448.60 1398.23 52 28.00 2396.26 2239.40
135 10 7.00 448.60 1398.23 52 0.00 2433.90 2233.04
136 10 8.00 448.60 1398.23 52 0.00 2470.34 2225.48
137 10 9.00 448.60 1398.23 52 0.00 2505.57 2216.72
138 10 10.00 448.60 1398.23 52 0.00 2539.61 2206.75
139 10 11.00 448.60 1398.23 52 0.00 2572.44 2195.58
140 10 12.00 448.60 1398.23 52 0.00 2604.07 2183.21
141 10 13.00 448.60 1398.23 52 0.00 2634.50 2169.64
142 10 14.00 448.60 1398.23 52 0.00 2663.73 2154.87
143 10 15.00 448.60 1398.23 52 0.00 2691.75 2138.89
144 10 16.00 448.60 1398.23 56 0.00 2701.22 2121.71
145 10 17.00 448.60 1398.23 56 2.00 2685.48 2137.14
146 10 18.00 448.60 1398.23 56 4.00 2670.87 2148.90
147 10 19.00 448.60 1398.23 56 6.00 2657.38 2156.98
148 10 20.00 448.60 1398.23 56 8.00 2645.02 2161.38
149 10 21.00 448.60 1398.23 56 10.00 2633.78 2162.11
150 10 22.00 448.60 1398.23 56 12.00 2623.67 2159.17
151 10 23.00 448.60 1398.23 56 14.00 2614.68 2152.55
152 10 24.00 448.60 1398.23 56 16.00 2606.82 2142.26
153 10 25.00 448.60 1398.23 56 18.00 2600.08 2128.29
154 10 26.00 448.60 1398.23 56 20.00 2594.47 2110.65
155 10 27.00 448.60 1398.23 56 22.00 2589.98 2089.33
156 10 28.00 448.60 1398.23 56 24.00 2586.62 2064.34
157 10 29.00 448.60 1398.23 57 0.00 2584.38 2035.68
158 10 30.00 448.60 1398.23 57 1.00 2583.27 2003.34
159 10 31.00 448.60 1398.23 57 2.00 2570.62 1967.34
160 10 32.00 448.60 1398.23 57 3.00 2540.31 1928.94
161 10 33.00 448.60 1398.23 57 4.00 2508.11 1888.14
162 10 34.00 448.60 1398.23 57 5.00 2474.01 1844.94
163 10 35.00 448.60 1398.23 57 6.00 2445.75 1800.94
164 10 36.00 448.60 1398.23 57 7.00 2445.75 1756.94
165 10 37.00 448.60 1398.23 57 8.00 2445.75 1712.94
166 10 38.00 448.60 1398.23 57 9.00 2445.75 1668.94
167 10 39.00 448.60 1398.23 57 10.00 2445.75 1624.94
168 10 40.00 448.60 1398.23 57 11.00 2445.75 1580.94
169 10 41.00 448.60 1398.23 57 12.00 2445.75 1536.94
170 10 42.00 448.60 1398.23 57 13.00 2445.75 1492.94
171 10 43.00 448.60 1398.23 57 14.00 2445.75 1448.94
172 10 44.00 448.60 1398.23 57 15.00 2445.75 1404.94
173 10 45.00 448.60 1398.23 57 16.00 2445.75 1360.94
174 10 46.00 448.60 1398.23 57 17.00 2445.75 1316.94
175 10 47.00 448.60 1398.23 57 18.00 2445.75 1272.94
176 10 48.00 448.60 1398.23 57 19.00 2445.75 1228.94
177 10 49.00 448.60 1398.23 57 20.00 2445.75 1184.94
178 10 50.00 448.60 1398.23 57 21.00 2445.75 1140.94
179 10 51.00 448.60 1398.23 57 22.00 2445.75 1096.94
180 10 52.00 448.60 1398.23 57 23.00 2445.75 1052.94
";

/// `b30set/ygrab2`: as `ygrab1`, with a back tap (`90:2:x=-127`) for the
/// throw: `ThrowB` (170) from frame 90, Mario thrown behind Yoshi. The
/// dummy is Training's Stand CPU (level 3): falling as `DamageFall` within
/// five frames' fall of the floor at frame 159, `ftComputerGetObjectiveStatus`
/// draws 0.175 (* 250 < 70) and counterattacks, its Z tap
/// (`nFTComputerInputButtonZ2`) arms the tech, and it lands at 160 into
/// `Passive` (81) instead of `DownBounceU` (68).
const YOSHI_GRAB_THROW_B: &str = "
58 10 23.00 448.60 1398.23 10 9.00 832.50 1419.04
59 10 24.00 448.60 1398.23 10 10.00 832.50 1419.04
60 152 1.00 448.60 1398.23 10 11.00 832.50 1419.04
61 166 1.00 448.60 1398.23 10 12.00 832.50 1419.04
62 166 2.00 448.60 1398.23 10 13.00 832.50 1419.04
63 166 3.00 448.60 1398.23 10 14.00 832.50 1419.04
64 166 4.00 448.60 1398.23 10 15.00 832.50 1419.04
65 166 5.00 448.60 1398.23 10 16.00 832.50 1419.04
66 166 6.00 448.60 1398.23 10 17.00 832.50 1419.04
67 166 7.00 448.60 1398.23 10 18.00 832.50 1419.04
68 166 8.00 448.60 1398.23 10 19.00 832.50 1419.04
69 166 9.00 448.60 1398.23 10 20.00 832.50 1419.04
70 166 10.00 448.60 1398.23 10 21.00 832.50 1419.04
71 166 11.00 448.60 1398.23 10 22.00 832.50 1419.04
72 166 12.00 448.60 1398.23 10 23.00 832.50 1419.04
73 166 13.00 448.60 1398.23 10 24.00 832.50 1419.04
74 166 14.00 448.60 1398.23 10 25.00 832.50 1419.04
75 167 2.00 448.60 1398.23 171 1.00 1008.87 1434.61
76 168 0.00 448.60 1398.23 172 2.00 865.12 1421.92
77 168 0.00 448.60 1398.23 172 2.00 865.12 1421.92
78 168 0.00 448.60 1398.23 172 2.00 865.12 1421.92
79 168 0.00 448.60 1398.23 172 2.00 865.12 1421.92
80 168 0.00 448.60 1398.23 172 2.00 865.12 1421.92
81 168 0.00 448.60 1398.23 172 2.00 865.12 1421.92
82 168 0.00 448.60 1398.23 172 2.00 865.12 1421.92
83 168 0.00 448.60 1398.23 172 2.00 865.12 1421.92
84 168 0.00 448.60 1398.23 172 2.00 865.12 1421.92
85 168 0.00 448.60 1398.23 172 2.00 865.12 1421.92
86 168 0.00 448.60 1398.23 172 2.00 865.12 1421.92
87 168 0.00 448.60 1398.23 172 2.00 865.12 1421.92
88 168 0.00 448.60 1398.23 172 2.00 865.12 1421.92
89 168 0.00 448.60 1398.23 172 2.00 865.12 1421.92
90 170 1.00 448.60 1398.23 186 2.00 758.07 1575.46
91 170 2.00 448.60 1398.23 186 3.00 665.47 1591.29
92 170 3.00 448.60 1398.23 186 4.00 642.47 1608.57
93 170 4.00 448.60 1398.23 186 5.00 619.20 1627.14
94 170 5.00 448.60 1398.23 186 6.00 622.45 1642.73
95 170 6.00 448.60 1398.23 186 7.00 627.23 1658.16
96 170 7.00 448.60 1398.23 186 8.00 621.87 1673.37
97 170 8.00 448.60 1398.23 186 9.00 580.91 1688.29
98 170 9.00 448.60 1398.23 186 10.00 536.70 1702.85
99 170 10.00 448.60 1398.23 186 11.00 490.00 1716.98
100 170 11.00 448.60 1398.23 186 12.00 442.67 1730.61
101 170 12.00 448.60 1398.23 186 13.00 396.13 1743.68
102 170 13.00 448.60 1398.23 186 14.00 352.24 1756.12
103 170 14.00 448.60 1398.23 186 15.00 312.49 1767.85
104 170 15.00 448.60 1398.23 186 16.00 278.52 1778.82
105 170 16.00 448.60 1398.23 186 17.00 251.27 1788.95
106 170 17.00 448.60 1398.23 186 18.00 232.24 1798.17
107 170 18.00 448.60 1398.23 186 19.00 221.32 1806.42
108 170 19.00 448.60 1398.23 52 2.00 59.09 1626.01
109 170 20.00 448.60 1398.23 52 3.00 -6.89 1687.19
110 170 21.00 448.60 1398.23 52 4.00 -71.67 1744.77
111 170 22.00 448.60 1398.23 52 5.00 -135.25 1798.75
112 170 23.00 448.60 1398.23 52 6.00 -197.62 1849.12
113 170 24.00 448.60 1398.23 52 7.00 -258.79 1895.90
114 170 25.00 448.60 1398.23 52 8.00 -318.77 1939.07
115 170 26.00 448.60 1398.23 52 9.00 -377.53 1978.64
116 170 27.00 448.60 1398.23 52 10.00 -435.10 2014.60
117 170 28.00 448.60 1398.23 52 11.00 -491.47 2046.97
118 170 29.00 448.60 1398.23 52 12.00 -546.63 2075.73
119 170 30.00 448.60 1398.23 52 13.00 -600.59 2100.89
120 170 31.00 448.60 1398.23 52 14.00 -653.35 2122.45
121 170 32.00 448.60 1398.23 52 15.00 -704.91 2140.41
122 170 33.00 448.60 1398.23 52 16.00 -755.26 2154.76
123 170 34.00 448.60 1398.23 52 17.00 -804.41 2165.51
124 170 35.00 448.60 1398.23 52 18.00 -852.36 2172.67
125 170 36.00 448.60 1398.23 52 19.00 -899.11 2176.21
126 170 37.00 448.60 1398.23 52 20.00 -944.66 2177.76
127 170 38.00 448.60 1398.23 52 21.00 -989.00 2178.10
128 170 39.00 448.60 1398.23 52 22.00 -1032.14 2177.25
129 170 40.00 448.60 1398.23 52 23.00 -1074.08 2175.19
130 170 41.00 448.60 1398.23 52 24.00 -1114.82 2171.92
131 170 42.00 448.60 1398.23 52 25.00 -1154.36 2167.46
132 170 43.00 448.60 1398.23 52 26.00 -1192.69 2161.79
133 170 44.00 448.60 1398.23 52 27.00 -1229.82 2154.93
134 170 45.00 448.60 1398.23 52 28.00 -1265.75 2146.85
135 170 46.00 448.60 1398.23 52 0.00 -1300.48 2137.58
136 170 47.00 448.60 1398.23 52 0.00 -1334.00 2127.11
137 170 48.00 448.60 1398.23 52 0.00 -1366.33 2115.43
138 10 0.00 448.60 1398.23 52 0.00 -1397.45 2102.55
139 10 1.00 448.60 1398.23 52 0.00 -1427.37 2088.47
140 10 2.00 448.60 1398.23 52 0.00 -1456.09 2073.19
141 10 3.00 448.60 1398.23 52 0.00 -1483.60 2056.70
142 10 4.00 448.60 1398.23 52 0.00 -1509.91 2039.02
143 10 5.00 448.60 1398.23 52 0.00 -1535.02 2020.13
144 10 6.00 448.60 1398.23 52 0.00 -1558.93 2000.04
145 10 7.00 448.60 1398.23 52 0.00 -1581.64 1978.74
146 10 8.00 448.60 1398.23 52 0.00 -1603.14 1956.25
147 10 9.00 448.60 1398.23 52 0.00 -1623.45 1932.55
148 10 10.00 448.60 1398.23 52 0.00 -1642.55 1907.65
149 10 11.00 448.60 1398.23 52 0.00 -1660.45 1881.55
150 10 12.00 448.60 1398.23 52 0.00 -1677.14 1854.24
151 10 13.00 448.60 1398.23 52 0.00 -1692.64 1825.74
152 10 14.00 448.60 1398.23 52 0.00 -1706.93 1796.03
153 10 15.00 448.60 1398.23 52 0.00 -1720.02 1765.12
154 10 16.00 448.60 1398.23 52 0.00 -1731.91 1733.01
155 10 17.00 448.60 1398.23 52 0.00 -1742.59 1699.69
156 10 18.00 448.60 1398.23 52 0.00 -1752.08 1665.18
157 10 19.00 448.60 1398.23 52 0.00 -1760.36 1629.46
158 10 20.00 448.60 1398.23 57 0.00 -1767.44 1592.54
159 10 21.00 448.60 1398.23 57 1.00 -1773.31 1554.42
160 10 22.00 448.60 1398.23 81 0.00 -1777.99 1517.92
161 10 23.00 448.60 1398.23 81 1.00 -1781.06 1518.73
162 10 24.00 448.60 1398.23 81 2.00 -1782.68 1519.15
163 10 25.00 448.60 1398.23 81 3.00 -1782.85 1519.20
164 10 26.00 448.60 1398.23 81 4.00 -1782.85 1519.20
165 10 27.00 448.60 1398.23 81 5.00 -1782.85 1519.20
166 10 28.00 448.60 1398.23 81 6.00 -1782.85 1519.20
167 10 29.00 448.60 1398.23 81 7.00 -1782.85 1519.20
168 10 30.00 448.60 1398.23 81 8.00 -1782.85 1519.20
169 10 31.00 448.60 1398.23 81 9.00 -1782.85 1519.20
170 10 32.00 448.60 1398.23 81 10.00 -1782.85 1519.20
171 10 33.00 448.60 1398.23 81 11.00 -1782.85 1519.20
172 10 34.00 448.60 1398.23 81 12.00 -1782.85 1519.20
173 10 35.00 448.60 1398.23 81 13.00 -1782.85 1519.20
174 10 36.00 448.60 1398.23 81 14.00 -1782.85 1519.20
175 10 37.00 448.60 1398.23 81 15.00 -1782.85 1519.20
176 10 38.00 448.60 1398.23 81 16.00 -1782.85 1519.20
177 10 39.00 448.60 1398.23 81 17.00 -1782.85 1519.20
178 10 40.00 448.60 1398.23 81 18.00 -1782.85 1519.20
179 10 41.00 448.60 1398.23 81 19.00 -1782.85 1519.20
180 10 42.00 448.60 1398.23 81 20.00 -1782.85 1519.20
";

fn yoshi_grab_pad(throw: (u16, i8)) -> impl Fn(u64) -> (u16, i8, i8) {
    move |t: u64| match t {
        9..=16 => (0, 127, 0),
        60 | 62 => (button::Z, 0, 0),
        61 => (button::Z | button::A, 0, 0),
        90 => (throw.0, throw.1, 0),
        91 if throw.1 != 0 => (0, throw.1, 0),
        _ => (0, 0, 0),
    }
}

#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn yoshi_grab_and_throw_match_the_n64() {
    // `ygrab2s`: Yoshi's dash dust draws 3 numbers at frames 9 and 15 each;
    // the dummy's landing check draws the 7th at 159.
    let landing = N64Seed {
        tick: 159,
        before: 0x3d6c_1037,
        after: 0x2cd6_0a1e,
    };
    for (name, table, throw, seed) in [
        ("forward", YOSHI_GRAB_THROW_F, (button::A, 0), None),
        ("back", YOSHI_GRAB_THROW_B, (0, -127), Some(landing)),
    ] {
        let pad = yoshi_grab_pad(throw);
        let Some((first_bad, log, max_err)) =
            replay_pair_rows(FighterKind::Yoshi, &pad, table, 60, seed)
        else {
            return;
        };
        if std::env::var_os("SCENE_TRACE").is_some() {
            println!("yoshi grab, {name} throw (max position error {max_err}):\n{log}");
        }
        assert_eq!(first_bad, None, "{name} throw\n{log}");
    }
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

/// `b34cpu/sp_link` (1P stage 0, Hyrule: Mario P1 idle against CPU Link,
/// level 2, `nFTComputerTraitLink`): both in Wait from frame 330; Go at
/// 390 (the Twister's Sleep draws its wait then; it sleeps past 1500).
/// Link stands (behaviour 15) while `behavior_change_wait` runs down from
/// 960, wakes at 1350 (objective 2, behaviour 0), walks left at 1379,
/// stops at 1482, turns and walks back. Columns: `t seed | Mario st x y |
/// Link st x y lr objective objective_base behaviour bcw`; every 10th
/// frame and around every change.
const LINK_STAGE_1P: &str = "
330 c823f683 | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 0 960
331 c823f683 | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 0 960
332 c823f683 | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 0 960
333 c823f683 | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 0 960
340 c823f683 | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 0 960
350 c823f683 | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 0 960
360 c823f683 | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 0 960
370 c823f683 | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 0 960
380 c823f683 | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 0 960
388 c823f683 | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 0 960
389 c823f683 | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 0 960
390 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 0 960
391 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 959
392 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 958
393 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 957
394 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 956
400 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 950
410 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 940
420 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 930
430 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 920
440 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 910
450 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 900
460 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 890
470 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 880
480 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 870
490 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 860
500 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 850
510 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 840
520 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 830
530 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 820
540 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 810
550 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 800
560 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 790
570 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 780
580 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 770
590 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 760
600 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 750
610 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 740
620 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 730
630 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 720
640 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 710
650 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 700
660 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 690
670 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 680
680 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 670
690 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 660
700 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 650
710 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 640
720 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 630
730 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 620
740 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 610
750 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 600
760 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 590
770 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 580
780 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 570
790 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 560
800 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 550
810 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 540
820 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 530
830 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 520
840 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 510
850 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 500
860 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 490
870 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 480
880 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 470
890 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 460
900 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 450
910 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 440
920 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 430
930 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 420
940 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 410
950 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 400
960 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 390
970 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 380
980 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 370
990 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 360
1000 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 350
1010 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 340
1020 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 330
1030 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 320
1040 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 310
1050 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 300
1060 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 290
1070 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 280
1080 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 270
1090 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 260
1100 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 250
1110 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 240
1120 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 230
1130 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 220
1140 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 210
1150 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 200
1160 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 190
1170 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 180
1180 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 170
1190 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 160
1200 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 150
1210 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 140
1220 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 130
1230 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 120
1240 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 110
1250 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 100
1260 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 90
1270 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 80
1280 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 70
1290 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 60
1300 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 50
1310 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 40
1320 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 30
1330 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 20
1340 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 10
1348 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 2
1349 18be873a | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 0 0 15 1
1350 3d6c1037 | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 2 2 0 0
1351 3d6c1037 | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 2 2 0 0
1352 3d6c1037 | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 2 2 0 0
1353 3d6c1037 | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 2 2 0 0
1360 3d6c1037 | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 2 2 0 0
1370 3d6c1037 | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 2 2 0 0
1377 3d6c1037 | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 2 2 0 0
1378 3d6c1037 | 10 -1495.00 1022.00 | 10 595.00 1022.00 -1 2 2 0 0
1379 2cd60a1e | 10 -1495.00 1022.00 | 12 580.20 1022.00 -1 2 2 0 0
1380 2cd60a1e | 10 -1495.00 1022.00 | 12 565.40 1022.00 -1 2 2 0 0
1381 2cd60a1e | 10 -1495.00 1022.00 | 12 550.60 1022.00 -1 2 2 0 0
1382 2cd60a1e | 10 -1495.00 1022.00 | 12 535.80 1022.00 -1 2 2 0 0
1390 2cd60a1e | 10 -1495.00 1022.00 | 12 417.40 1022.00 -1 2 2 0 0
1400 2cd60a1e | 10 -1495.00 1022.00 | 12 269.40 1022.00 -1 2 2 0 0
1410 2cd60a1e | 10 -1495.00 1022.00 | 12 121.40 1022.00 -1 2 2 0 0
1420 2cd60a1e | 10 -1495.00 1022.00 | 12 -26.60 1022.00 -1 2 2 0 0
1430 2cd60a1e | 10 -1495.00 1022.00 | 12 -174.60 1022.00 -1 2 2 0 0
1440 2cd60a1e | 10 -1495.00 1022.00 | 12 -322.60 1022.00 -1 2 2 0 0
1450 2cd60a1e | 10 -1495.00 1022.00 | 12 -470.60 1022.00 -1 2 2 0 0
1460 2cd60a1e | 10 -1495.00 1022.00 | 12 -618.60 1022.00 -1 2 2 0 0
1470 2cd60a1e | 10 -1495.00 1022.00 | 12 -766.60 1022.00 -1 2 2 0 0
1480 2cd60a1e | 10 -1495.00 1022.00 | 12 -914.60 1022.00 -1 2 2 0 0
1481 2cd60a1e | 10 -1495.00 1022.00 | 12 -929.40 1022.00 -1 2 2 0 0
1482 72ae7869 | 10 -1495.00 1022.00 | 10 -936.20 1022.00 -1 2 2 0 0
1483 72ae7869 | 10 -1495.00 1022.00 | 18 -936.20 1022.00 -1 2 2 0 0
1484 a6e9c9a6 | 10 -1495.00 1022.00 | 18 -936.20 1022.00 -1 2 2 0 0
1485 0bb31550 | 10 -1495.00 1022.00 | 18 -936.20 1022.00 -1 2 2 0 0
1486 0bb31550 | 10 -1495.00 1022.00 | 18 -936.20 1022.00 -1 2 2 0 0
1487 0bb31550 | 10 -1495.00 1022.00 | 18 -936.20 1022.00 -1 2 2 0 0
1488 0bb31550 | 10 -1495.00 1022.00 | 18 -936.20 1022.00 1 2 2 0 0
1489 0bb31550 | 10 -1495.00 1022.00 | 18 -936.20 1022.00 1 2 2 0 0
1490 0bb31550 | 10 -1495.00 1022.00 | 18 -936.20 1022.00 1 2 2 0 0
1491 0bb31550 | 10 -1495.00 1022.00 | 18 -936.20 1022.00 1 2 2 0 0
1492 0bb31550 | 10 -1495.00 1022.00 | 18 -936.20 1022.00 1 2 2 0 0
1493 0bb31550 | 10 -1495.00 1022.00 | 18 -936.20 1022.00 1 2 2 0 0
1494 0bb31550 | 10 -1495.00 1022.00 | 12 -921.40 1022.00 1 2 2 0 0
1495 0bb31550 | 10 -1495.00 1022.00 | 12 -906.60 1022.00 1 2 2 0 0
1496 0bb31550 | 10 -1495.00 1022.00 | 12 -891.80 1022.00 1 2 2 0 0
1497 0bb31550 | 10 -1495.00 1022.00 | 12 -877.00 1022.00 1 2 2 0 0
1500 0bb31550 | 10 -1495.00 1022.00 | 12 -832.60 1022.00 1 2 2 0 0
";

/// `nFTComputerBehavior*` as the N64 numbers it: the port's enum leaves
/// out the four unused values (10-12, 14), which nothing reads.
fn n64_behavior(b: ssb_game::computer::Behavior) -> f32 {
    use ssb_game::computer::Behavior as B;
    match b {
        B::Bonus3 => 13.0,
        B::Stand => 15.0,
        B::Walk => 16.0,
        B::Evade => 17.0,
        B::Jump => 18.0,
        B::Unk5 => 19.0,
        _ => f32::from(b as u8),
    }
}

/// The 1P Link stage's CPU from the countdown through its wake and walks:
/// statuses, positions, objective, behaviour and the RNG it draws.
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn link_stage_cpu_wakes_and_walks_as_on_the_n64() {
    use ssb_game::computer::attack::Trait;
    use ssb_game::fighter::Facing;
    let Some(bytes) = open_pack() else {
        return;
    };
    let pack = ssb_rom::pack::Pack::open(&bytes).unwrap();
    let (map, _, mut pair) = hazard_pair(&pack, 4);
    let index = pack
        .stage_of_file(ssb_rom::stage::VS_GROUND_FILES[4])
        .unwrap();
    let (stage, segments) = stage_floors(&pack, index);
    let bounds = stage_bounds(&pack, &stage);
    let make = |kind, port, x, facing| {
        let mut f = vs_fighter(&pack, &stage, kind, port, (x, 1022.0, facing), &segments);
        f.dead.bounds = Some(bounds);
        Posed::new(&pack, f)
    };
    let mut fighters = vec![
        make(FighterKind::Mario, 0, -1495.0, Facing::Right),
        make(FighterKind::Link, 1, 595.0, Facing::Left),
    ];
    pair.items.normal_switches.toggles = 0;
    pair.cpu = Some(HostCpu::one_player(
        &fighters[1].fighter,
        &stage,
        bounds,
        (&map, None),
        2,
        Trait::Link,
    ));
    let want: Vec<(u64, u32, Vec<f32>)> = LINK_STAGE_1P
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let mut head = l.split('|').next().unwrap().split_whitespace();
            let t = head.next().unwrap().parse().unwrap();
            let seed = u32::from_str_radix(head.next().unwrap(), 16).unwrap();
            let cols = l
                .split('|')
                .skip(1)
                .flat_map(|g| g.split_whitespace().map(|v| v.parse().unwrap()))
                .collect();
            (t, seed, cols)
        })
        .collect();
    let mut want = want.into_iter().peekable();
    let first = want.next().unwrap().0;
    let mut log = String::new();
    let mut first_bad = None;
    for tick in first + 1..=1500 {
        // The seed Go's draw left (frame 390's): no host draw replaces it.
        if tick == 391 {
            ssb_game::rng::set_seed(0x18BE_873Au32 as i32);
        }
        pair.locked = tick <= 390;
        let _ = tick_pair(&pack, &map, &mut fighters, &|_| (0, 0, 0), tick, &mut pair);
        let Some((_, seed, cols)) = want.next_if(|r| r.0 == tick) else {
            continue;
        };
        let (m, l) = (&fighters[0].fighter, &fighters[1].fighter);
        let c = &pair.cpu.as_ref().unwrap().computer;
        let got = vec![
            f32::from(m.status.status.id()),
            m.pos.x,
            m.pos.y,
            f32::from(l.status.status.id()),
            l.pos.x,
            l.pos.y,
            l.facing.sign(),
            c.objective as u8 as f32,
            c.objective_base as u8 as f32,
            n64_behavior(c.behavior),
            f32::from(c.behavior_change_wait),
        ];
        let host_seed = ssb_game::rng::seed() as u32;
        // Before Go the host's seed is its setup's (`hazard_pair`).
        let ok = (tick < 391 || host_seed == seed)
            && got.len() == cols.len()
            && got.iter().zip(&cols).all(|(a, b)| (a - b).abs() <= 0.01);
        let mark = if ok { " " } else { "X" };
        log.push_str(&format!(
            "{tick} {mark} rust {host_seed:08x} {got:?}\n{tick} {mark}  n64 {seed:08x} {cols:?}\n"
        ));
        if !ok && first_bad.is_none() {
            first_bad = Some(tick);
        }
    }
    assert!(want.next().is_none(), "every row compared");
    if std::env::var_os("SCENE_TRACE").is_some() {
        println!("{log}");
    }
    assert_eq!(first_bad, None, "{log}");
}

/// Replays a 1P stage from Go (tick 391) against an `sp1.py` trace's rows
/// `t seed | st x y lr pc | st x y lr pc obj base beh bcw | ...` (P1 idle,
/// then each CPU): the roster, levels, traits, teams and handicaps from
/// `Game::setup_stage_all` (Easy, Mario), each fighter standing where the
/// first row (frame 390) has it, the moving groups `groups` ticks in.
/// Returns the first bad tick and the log.
fn one_player_replay(
    sp_stage: ssb_game::spgame::Stage,
    groups: Option<usize>,
    table: &str,
) -> Option<(Option<u64>, String)> {
    use ssb_game::fighter::Facing;
    use ssb_game::spgame::{manager::Manager, Backup, BattleState, Difficulty, Game, SceneData};
    let bytes = open_pack()?;
    let pack = ssb_rom::pack::Pack::open(&bytes).unwrap();
    let mut state = BattleState::default();
    let mut scene = SceneData {
        stage: sp_stage as u8,
        fkind: FighterKind::Mario,
        ..Default::default()
    };
    Manager::new(&mut scene, &mut state, &Backup::default());
    let game = Game::setup_stage_all(
        &mut state,
        &mut scene,
        Difficulty::Easy,
        0,
        FighterKind::Ness,
    );
    let rows: Vec<(u64, u32, Vec<f32>)> = table
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let mut head = l.split('|').next().unwrap().split_whitespace();
            let t = head.next().unwrap().parse().unwrap();
            let seed = u32::from_str_radix(head.next().unwrap(), 16).unwrap();
            let cols = l
                .split('|')
                .skip(1)
                .flat_map(|g| g.split_whitespace().map(|v| v.parse().unwrap()))
                .collect();
            (t, seed, cols)
        })
        .collect();
    let gkind = usize::from(state.gkind);
    let (map, _, mut pair) = hazard_pair(&pack, gkind);
    let index = pack
        .stage_of_file(ssb_rom::stage::VS_GROUND_FILES[gkind])
        .unwrap();
    // The stage ran frames 1..=390 before the first row: take its state at
    // frame 390 from the trace's `gGRCommonStruct` dump.
    let mut barrel_path = None;
    match &mut pair.stage.controller {
        ssb_game::stage::Controller::Jungle(j) => {
            use ssb_rom::ground_obj as g;
            j.status = ssb_game::stage::jungle::BarrelStatus::Move;
            j.wait = JUNGLE_1P_BARREL_WAIT;
            j.rotate_step = 0.07;
            let source = pack.stage(index).unwrap().source_file;
            let mut objects = Box::new(g::GroundObjects::new(&pack, source));
            objects.play(&pack, g::TARUCANN_DEFAULT).unwrap();
            // Frame 0's play, then frames 1..=390.
            for _ in 0..=390 {
                objects.advance(&pack).unwrap();
            }
            barrel_path = Some(objects);
        }
        ssb_game::stage::Controller::Pupupu(p) => {
            p.wind_wait = PUPUPU_1P_WIND_WAIT;
            p.blink_wait = PUPUPU_1P_BLINK_WAIT;
            p.status = ssb_game::stage::pupupu::WindStatus::Wait;
            // Whispy's blinks and the flowers wait for their clips to end.
            let source = pack.stage(index).unwrap().source_file;
            let mut objects = Box::new(ssb_rom::ground_obj::GroundObjects::new(&pack, source));
            for _ in 0..=390 {
                objects.advance(&pack).unwrap();
            }
            pair.objects = Some(objects);
        }
        _ => {}
    }
    let barrel_at = |o: &ssb_rom::ground_obj::GroundObjects| {
        let [x, y, z] = o
            .instance(ssb_rom::ground_obj::TARUCANN, 0)
            .unwrap()
            .translate();
        ssb_engine::math::Vec3::new(x, y, z)
    };
    if let Some(o) = &barrel_path {
        pair.barrel = Some(barrel_at(o));
        if std::env::var_os("SCENE_TRACE").is_some() {
            println!("barrel at 390: {:?}", pair.barrel);
        }
    }
    let (stage, segments) = stage_floors(&pack, index);
    let bounds = stage_bounds(&pack, &stage);
    if let Some(n) = groups {
        let mut g = ssb_pose::ground::StageGroups::new(&pack, index, &stage);
        for _ in 0..n {
            g.tick(&pack).unwrap();
        }
        pair.groups = Some((g, map_groups(&pack, &stage)));
    }
    let team_rules = ssb_game::team::TeamRules {
        is_team_battle: true,
        is_team_attack: state.is_team_attack,
    };
    pair.effects = Some(posed::HostEffects::default());
    pair.items.team_rules = team_rules;
    pair.weapons.team_rules = team_rules;
    pair.items.normal_switches = ssb_game::item::normal::Switches {
        appearance: state.item_appearance,
        toggles: state.item_toggles,
    };
    // `grCommonSetupInitAll`: the appearance actor follows the ground; its
    // spawn wait at frame 390 is the trace's (`gITManagerAppearActor`).
    {
        let s = pack.stage(index).unwrap();
        pair.items.make_appear_actor(
            s.item_weights.as_ref(),
            pack.stage_points(&s)
                .filter(|p| p.kind == ssb_game::stage::mapobj::ITEM)
                .map(|p| ssb_engine::math::Vec3::new(p.x as f32, p.y as f32, 0.0)),
        );
        let wait = match sp_stage {
            ssb_game::spgame::Stage::Donkey => Some(APPEAR_1P_DONKEY_SPAWN_WAIT),
            ssb_game::spgame::Stage::Kirby => Some(APPEAR_1P_KIRBY_SPAWN_WAIT),
            _ => None,
        };
        if let (Some(a), Some(w)) = (pair.items.appear.as_mut(), wait) {
            a.spawn_wait = w;
        }
    }
    let (_, first_seed, start) = &rows[0];
    let present: Vec<usize> = (0..4)
        .filter(|&i| state.players[i].pkind != ssb_game::spgame::PlayerKind::Not)
        .collect();
    let mut fighters: Vec<Posed> = Vec::new();
    let mut col = 0;
    for &i in &present {
        let p = state.players[i];
        let (x, y, lr) = (start[col + 1], start[col + 2], start[col + 3]);
        col += if i == 0 { 5 } else { 9 };
        let facing = if lr < 0.0 {
            Facing::Left
        } else {
            Facing::Right
        };
        // The row's y is rounded (to 0.01): place the fighter just above
        // it and let the placement snap it to its floor.
        let y = y + 0.5;
        let mut f = vs_fighter(&pack, &stage, p.fkind, i as u8, (x, y, facing), &segments);
        f.dead.bounds = Some(bounds);
        // `Session::configure_fighter`.
        f.stocks = p.stock_count;
        f.team = p.team;
        f.handicap = p.handicap;
        f.costume = p.costume;
        // `ftManagerMakeFighter`'s `FTCreateDesc::copy_kind`: the Kirby
        // Team's copies.
        if ssb_game::kirby::is_kirby(f.kind) {
            ssb_game::kirby::init_copy(&mut f, game.setups[i].copy_kind);
        }
        f.dead.stock_rule = false;
        f.dead.spgame_rule = true;
        f.dead.team_battle = true;
        let mut posed = Posed::new(&pack, f);
        posed.set_damage_detect_size(&pack);
        fighters.push(posed);
    }
    let map_groups_now = pair
        .groups
        .as_ref()
        .map(|(g, y)| (g.groups.clone(), y.clone()));
    let mut cpus: Vec<Option<HostCpu>> = present
        .iter()
        .zip(&fighters)
        .map(|(&i, p)| {
            (i != 0).then(|| {
                HostCpu::one_player(
                    &p.fighter,
                    &stage,
                    bounds,
                    (
                        &map,
                        map_groups_now
                            .as_ref()
                            .map(|(g, y)| (g.as_slice(), y.as_slice())),
                    ),
                    state.players[i].level,
                    game.setups[i].cp_trait,
                )
            })
        })
        .collect();
    // `sc1PGameSetPlayerInterfacePositions` and the damage displays: what
    // a fall's `ftCommonDeadUpdateScore` reads (its break draws, its stock
    // snap's particle).
    let positions = ssb_game::spgame::wait::interface_positions(0, present.len() as u8);
    let mut displays: [ssb_game::hud::DamageDisplay; 4] = core::array::from_fn(|port| {
        if positions[port] != 0 {
            ssb_game::hud::DamageDisplay::at(port, 0, positions[port])
        } else {
            ssb_game::hud::DamageDisplay::new(port, 0)
        }
    });
    let mut want = rows.iter().skip(1).peekable();
    let last = rows.last().unwrap().0;
    let mut log = String::new();
    let mut first_bad = None;
    // The seed frame 390 left: Go's draw is in it, no host draw replaces it.
    ssb_game::rng::set_seed(*first_seed as i32);
    for tick in 391..=last {
        if let Some(o) = &mut barrel_path {
            o.advance(&pack).unwrap();
            pair.barrel = Some(barrel_at(o));
        }
        ssb_game::hud::set_fall_interface(&ssb_game::hud::FallInterface::of(&displays, false));
        posed::tick_cpus(
            &pack,
            &map,
            &mut fighters,
            &|_| (0, 0, 0),
            &mut cpus,
            tick,
            &mut pair,
        );
        // `ifCommonPlayerDamageProcUpdate`, after the fighters.
        for p in fighters.iter() {
            let f = &p.fighter;
            let d = &mut displays[usize::from(f.port & 3)];
            if let Some(v) = ssb_game::hud::take_break_vel_x(usize::from(f.port)) {
                d.start_break_anim_from(&v);
            }
            if d.is_update_anim
                && f.status.status
                    == ssb_game::status::AnyStatus::Common(ssb_game::status::Status::RebirthDown)
            {
                d.stop_break_anim();
            }
            d.update(i32::from(f.damage), false);
        }
        let Some((_, seed, cols)) = want.next_if(|r| r.0 == tick) else {
            continue;
        };
        let mut got = Vec::new();
        for (p, cpu) in fighters.iter().zip(&cpus) {
            let f = &p.fighter;
            got.extend([
                f32::from(f.status.status.id()),
                f.pos.x,
                f.pos.y,
                f.facing.sign(),
                f32::from(f.damage),
            ]);
            if let Some(cpu) = cpu {
                let c = &cpu.computer;
                got.extend([
                    c.objective as u8 as f32,
                    c.objective_base as u8 as f32,
                    n64_behavior(c.behavior),
                    f32::from(c.behavior_change_wait),
                ]);
            }
        }
        let host_seed = ssb_game::rng::seed() as u32;
        let ok = host_seed == *seed
            && got.len() == cols.len()
            && got.iter().zip(cols).all(|(a, b)| (a - b).abs() <= 0.01);
        let mark = if ok { " " } else { "X" };
        log.push_str(&format!(
            "{tick} {mark} rust {host_seed:08x} {got:?}\n{tick} {mark}  n64 {seed:08x} {cols:?}\n"
        ));
        if !ok && first_bad.is_none() {
            first_bad = Some(tick);
        }
    }
    assert!(want.next().is_none(), "every row compared");
    Some((first_bad, log))
}

/// N64 `sp4l` (warp ROM `warp1p_4`: 1P stage 4, Peach's Castle, Easy):
/// Mario (P1, idle), Fox (ally, level 5, Ally trait), Mario and Luigi
/// (level 3, Mario Bros trait). See [`one_player_replay`].
const MARIO_BROS_STAGE_1P: &str = "
390 e7847115 | 10 -648.00 503.13 1 0 | 10 -693.00 1400.00 1 0 0 0 0 240 | 10 596.00 510.00 -1 0 0 0 0 720 | 10 851.00 1420.68 -1 0 0 0 0 720
391 daf1b4cc | 10 -648.00 503.13 1 0 | 10 -693.00 1400.00 1 0 9 9 3 239 | 10 596.00 510.00 -1 0 2 2 0 719 | 10 851.00 1420.68 -1 0 9 9 3 719
400 daf1b4cc | 10 -648.00 503.13 1 0 | 10 -693.00 1400.00 1 0 9 9 3 230 | 10 596.00 510.00 -1 0 2 2 0 710 | 10 851.00 1420.68 -1 0 9 9 3 710
410 daf1b4cc | 10 -648.00 503.13 1 0 | 10 -693.00 1400.00 1 0 9 9 3 220 | 10 596.00 510.00 -1 0 2 2 0 700 | 10 851.00 1420.68 -1 0 9 9 3 700
420 12dbce4a | 10 -648.00 503.13 1 0 | 15 -612.00 1399.88 1 0 9 9 3 210 | 18 596.00 510.00 -1 0 2 2 0 690 | 10 851.00 1420.68 -1 0 9 9 3 690
430 0f3e862e | 10 -648.00 503.13 1 0 | 15 48.00 1398.85 1 0 9 9 3 200 | 18 596.00 510.00 1 0 2 2 0 680 | 18 851.00 1420.68 1 0 9 9 3 680
440 0f3e862e | 10 -648.00 503.13 1 0 | 15 81.00 1398.80 1 0 9 9 3 190 | 12 713.23 487.12 1 0 2 2 0 670 | 12 958.58 1430.17 1 0 9 9 3 670
450 0f3e862e | 10 -648.00 503.13 1 0 | 12 261.00 1398.52 1 0 9 9 3 180 | 12 829.77 458.51 1 0 2 2 0 660 | 12 1078.12 1440.72 1 0 9 9 3 660
460 54ded7a1 | 10 -648.00 503.13 1 0 | 12 441.00 1398.24 1 0 9 9 3 170 | 12 946.31 429.90 1 0 2 2 0 650 | 18 1189.17 1439.98 1 0 9 9 3 650
470 54ded7a1 | 10 -648.00 503.13 1 0 | 195 579.00 1398.02 1 0 9 9 3 160 | 12 1062.85 401.30 1 0 2 2 0 640 | 18 1193.54 1439.47 -1 0 9 9 3 640
480 9ad4bb73 | 10 -648.00 503.13 1 0 | 195 579.00 1398.02 1 0 9 9 3 150 | 18 1080.34 397.01 -1 0 2 2 0 630 | 41 1157.78 1443.67 -1 9 9 9 3 630
490 2213eb38 | 10 -648.00 503.13 1 0 | 195 579.00 1398.02 1 0 9 9 3 140 | 12 998.76 417.03 -1 0 2 2 0 620 | 41 1348.58 1419.23 -1 9 9 9 3 620
500 ca8024b3 | 10 -648.00 503.13 1 0 | 195 579.00 1398.02 1 0 9 9 3 130 | 12 882.22 445.64 -1 0 2 2 0 610 | 12 1371.44 1409.61 -1 9 9 9 3 610
510 89753ad5 | 10 -648.00 503.13 1 0 | 225 579.00 1398.02 1 0 9 9 3 120 | 12 765.68 474.24 -1 0 2 2 0 600 | 12 1305.08 1426.34 -1 9 9 9 3 600
520 470e052a | 10 -648.00 503.13 1 0 | 225 579.00 1398.02 1 0 9 9 3 110 | 12 649.14 502.85 -1 0 2 2 0 590 | 12 1187.65 1440.16 -1 9 9 9 3 590
530 66fa6bc0 | 10 -648.00 503.13 1 0 | 225 579.00 1398.02 1 0 9 9 3 100 | 225 544.58 725.93 -1 0 2 2 0 580 | 40 1080.24 1440.90 -1 15 9 9 3 580
540 85881569 | 10 -648.00 503.13 1 0 | 54 623.96 1553.80 -1 2 9 9 3 90 | 225 307.51 1426.95 -1 0 2 2 0 570 | 40 1168.32 1442.43 -1 15 9 9 3 570
541 ef3c16d1 | 10 -648.00 503.13 1 0 | 54 791.96 1553.80 -1 2 2 9 3 89 | 225 307.51 1426.95 -1 0 2 2 0 569 | 40 1183.81 1440.61 -1 15 9 9 3 569
542 d20be72e | 10 -648.00 503.13 1 0 | 54 791.96 1553.80 -1 2 9 9 3 88 | 225 307.51 1426.95 -1 0 2 2 0 568 | 40 1198.60 1438.87 -1 15 9 9 3 568
550 b087996d | 10 -648.00 503.13 1 0 | 54 961.22 2041.52 -1 2 9 9 3 80 | 225 -17.81 2162.22 -1 0 2 2 0 560 | 10 1291.92 1427.89 -1 15 9 9 3 560
560 1cdf3b81 | 10 -648.00 503.13 1 0 | 54 1133.21 2167.38 -1 2 9 9 3 70 | 225 -248.50 2193.60 -1 0 2 2 0 550 | 208 1345.83 1420.39 -1 15 9 9 3 550
570 1d5eeb26 | 10 -648.00 503.13 1 0 | 54 1261.19 2045.03 -1 2 9 9 3 60 | 225 -350.76 1751.26 -1 0 2 2 0 540 | 208 1348.30 1419.35 -1 15 9 9 3 540
580 71f2efa7 | 10 -648.00 503.13 1 0 | 54 1345.18 1758.47 -1 2 9 9 3 50 | 59 -396.18 1399.54 -1 0 2 2 0 530 | 208 1348.30 1419.35 -1 15 9 9 3 530
581 71f2efa7 | 10 -648.00 503.13 1 0 | 54 1351.16 1720.78 -1 2 9 9 3 49 | 59 -396.18 1399.54 -1 0 2 2 0 529 | 208 1348.30 1419.35 -1 15 9 9 3 529
582 9c7522fc | 10 -648.00 503.13 1 0 | 54 1356.70 1681.45 -1 2 2 9 3 48 | 59 -396.18 1399.54 -1 0 2 2 0 528 | 208 1348.30 1419.35 -1 15 9 9 3 528
590 3a721fbf | 10 -648.00 503.13 1 0 | 68 1390.32 1401.66 -1 2 2 9 3 40 | 59 -396.18 1399.54 -1 0 2 2 0 520 | 208 1341.55 1422.05 -1 15 9 9 3 520
595 44aa63fd | 10 -648.00 503.13 1 0 | 68 1424.07 1387.45 -1 2 2 9 3 35 | 59 -396.18 1399.54 -1 0 2 2 0 515 | 208 1307.80 1426.02 -1 15 9 9 3 515
596 994d482e | 10 -648.00 503.13 1 0 | 68 1430.82 1384.61 -1 2 2 9 3 34 | 59 -396.18 1399.54 -1 0 2 2 0 514 | 208 1301.05 1426.82 -1 15 2 9 3 514
600 73b19c4e | 10 -648.00 503.13 1 0 | 68 1457.82 1373.25 -1 2 2 9 3 30 | 59 -396.18 1399.54 -1 0 2 2 0 510 | 208 1274.05 1429.99 -1 15 2 9 3 510
610 0665a8a2 | 10 -648.00 503.13 1 0 | 68 1484.82 1361.88 -1 2 2 9 3 20 | 10 -396.18 1399.54 -1 0 2 2 0 500 | 10 1247.05 1433.17 -1 15 2 9 3 500
620 6469e2b1 | 10 -648.00 503.13 1 0 | 80 1484.82 1361.88 -1 2 2 9 3 10 | 10 -396.18 1399.54 -1 0 2 2 0 490 | 157 1247.05 1433.17 -1 15 2 9 3 490
630 ffa6e58f | 10 -648.00 503.13 1 0 | 80 1464.57 1370.40 -1 2 2 9 3 0 | 33 -397.76 1349.14 -1 0 2 2 0 480 | 157 1951.03 1226.72 -1 15 2 9 3 480
635 ffa6e58f | 10 -648.00 503.13 1 0 | 80 1464.57 1370.40 -1 2 2 9 3 0 | 33 -401.13 1241.14 -1 0 2 2 0 475 | 157 2204.57 1229.81 -1 15 2 9 3 475
636 836bbe04 | 10 -648.00 503.13 1 0 | 80 1464.57 1370.40 -1 2 9 9 3 0 | 33 -402.03 1212.34 -1 0 2 2 0 474 | 157 2220.00 1230.00 -1 15 2 9 3 474
640 836bbe04 | 10 -648.00 503.13 1 0 | 80 1464.57 1370.40 -1 2 9 9 3 0 | 33 -406.38 1073.14 -1 0 2 2 0 470 | 157 2220.00 1230.00 -1 15 2 9 3 470
641 edd04ec2 | 10 -648.00 503.13 1 0 | 80 1464.57 1370.40 -1 2 9 9 3 0 | 33 -407.65 1032.34 -1 0 2 2 0 469 | 157 2218.54 1229.98 -1 15 9 9 3 469
650 ca92bb53 | 10 -648.00 503.13 1 0 | 80 1464.57 1370.40 -1 2 9 9 3 0 | 26 -431.76 559.14 -1 0 2 2 0 460 | 152 2202.09 1229.78 -1 15 9 9 3 460
660 1db5d021 | 10 -661.50 499.81 1 0 | 80 1464.57 1370.40 -1 2 9 9 3 0 | 32 -429.55 510.00 -1 0 2 2 0 450 | 153 2202.09 1229.78 -1 15 9 9 3 450
670 176d7c89 | 10 -661.50 499.81 1 0 | 204 1464.57 1370.40 1 2 9 9 3 0 | 10 -429.55 510.00 -1 0 2 2 0 440 | 154 2202.09 1229.78 -1 15 9 9 3 440
680 c9622c43 | 10 -661.50 499.81 1 0 | 204 1675.72 1281.52 1 2 9 9 3 0 | 208 -429.55 510.00 -1 0 2 2 0 430 | 51 2190.09 1229.64 -1 32 9 9 3 430
690 02ee36d8 | 52 -661.50 499.81 1 17 | 204 1854.66 1225.54 1 2 9 9 3 0 | 208 -429.55 510.00 -1 0 2 2 0 420 | 51 2439.18 1426.39 -1 32 9 9 3 420
691 02ee36d8 | 52 -704.93 537.21 1 17 | 204 1892.84 1226.01 1 2 9 9 3 0 | 208 -429.55 510.00 -1 0 2 2 0 419 | 51 2485.24 1456.00 -1 32 4 9 3 419
700 ee9efef4 | 52 -1039.43 714.12 1 17 | 204 2220.00 1230.00 1 2 9 9 3 0 | 208 -429.55 510.00 -1 0 2 2 0 410 | 51 2843.37 1576.26 -1 32 4 9 3 410
710 02b26b7b | 52 -1292.02 579.18 1 17 | 204 2220.00 1230.00 1 2 9 9 3 0 | 208 -429.55 510.00 -1 0 2 2 0 400 | 51 3122.23 1432.78 -1 32 4 9 3 400
720 f8761dc7 | 52 -1419.27 255.79 1 17 | 204 2220.00 1230.00 1 2 9 9 3 0 | 208 -429.55 510.00 -1 0 2 2 0 390 | 51 3275.74 1153.46 -1 32 4 9 3 390
730 f604b1b4 | 68 -1439.50 -101.00 1 17 | 18 2220.00 1230.00 -1 2 9 9 3 0 | 10 -429.55 510.00 -1 0 2 2 0 380 | 24 3157.58 1435.89 -1 32 4 9 3 380
740 dc040fe5 | 68 -1475.54 -101.00 1 17 | 13 2040.01 1227.81 -1 2 9 9 3 0 | 10 -429.55 510.00 -1 0 2 2 0 370 | 66 2934.08 1861.84 -1 32 4 9 3 370
750 a1be427e | 68 -1511.57 -101.00 1 17 | 13 1688.49 1276.15 -1 2 9 9 3 0 | 12 -537.55 510.00 -1 0 2 2 0 360 | 27 2710.58 1815.70 -1 32 4 9 3 360
760 79c4cece | 70 -1547.61 -101.00 1 17 | 13 1356.69 1415.82 -1 2 9 9 3 0 | 12 -656.52 501.04 -1 0 2 2 0 350 | 27 2487.07 1574.20 -1 32 4 9 3 350
770 90e4af1e | 70 -1583.65 -101.00 1 17 | 13 1001.41 1433.95 -1 2 9 9 3 0 | 28 -708.96 488.16 -1 0 2 2 0 340 | 27 2263.57 1166.80 -1 32 4 9 3 340
780 cd51aa71 | 70 -1619.68 -101.00 1 17 | 13 642.80 1402.31 -1 2 9 9 3 0 | 29 -708.96 488.16 -1 0 2 2 0 330 | 27 2040.07 746.80 -1 32 4 9 3 330
790 3a137692 | 70 -1655.72 -101.00 1 17 | 30 543.05 1398.08 -1 2 9 9 3 0 | 20 -708.96 488.16 -1 0 2 2 0 320 | 226 1824.17 368.30 -1 32 4 9 3 320
800 339ae3b4 | 70 -1691.75 -101.00 1 17 | 10 543.05 1398.08 -1 2 9 9 3 0 | 22 -936.76 1176.16 -1 0 2 2 0 310 | 226 1206.41 1096.74 -1 32 4 9 3 310
801 350a58fa | 70 -1695.36 -101.00 1 17 | 10 543.05 1398.08 -1 2 9 9 3 0 | 22 -966.56 1231.76 -1 0 2 2 0 309 | 226 1120.80 1206.10 -1 32 9 9 3 309
810 49c23845 | 70 -1727.79 -101.00 1 17 | 38 543.05 1398.08 1 3 9 9 3 0 | 22 -1135.76 1624.16 -1 0 2 2 0 300 | 226 620.89 1711.40 -1 32 9 9 3 300
811 e1e2ee01 | 70 -1731.39 -101.00 1 17 | 38 543.05 1398.08 1 3 9 9 3 0 | 22 -1143.56 1655.76 -1 0 2 2 0 299 | 226 620.89 1711.40 -1 32 2 9 3 299
817 392565b6 | 70 -1753.02 -101.00 1 17 | 38 475.85 1398.18 1 3 9 9 3 0 | 22 -1146.56 1794.96 -1 0 2 2 0 293 | 226 540.33 1723.88 -1 32 2 9 3 293
818 d0a09062 | 70 -1756.62 -101.00 1 17 | 38 457.45 1398.21 1 3 2 9 3 0 | 22 -1140.56 1809.76 -1 0 2 2 0 292 | 226 521.44 1714.94 -1 32 2 9 3 292
820 69b9f5d9 | 70 -1763.83 -101.00 1 17 | 38 426.65 1398.26 1 3 2 9 3 0 | 22 -1123.16 1832.16 -1 0 2 2 0 290 | 226 494.10 1680.24 -1 32 2 9 3 290
830 07ec1bf5 | 70 -1799.86 -101.00 1 17 | 152 368.35 1398.35 1 3 2 9 3 0 | 22 -958.16 1800.16 -1 0 2 2 0 280 | 59 503.04 1398.14 -1 32 2 9 3 280
840 439022aa | 70 -1835.90 -101.00 1 17 | 154 321.10 1398.42 1 3 2 9 3 0 | 224 -715.16 1528.16 1 0 2 2 0 270 | 59 550.29 1398.07 -1 32 2 9 3 270
850 23afcc5b | 70 -1871.94 -101.00 1 17 | 10 321.10 1398.42 1 3 2 9 3 0 | 223 -577.56 1399.82 1 0 2 2 0 260 | 59 550.29 1398.07 -1 32 2 9 3 260
851 23afcc5b | 70 -1875.54 -101.00 1 17 | 10 321.10 1398.42 1 3 2 9 3 0 | 223 -577.56 1399.82 1 0 2 2 0 259 | 59 550.29 1398.07 -1 32 2 9 3 259
852 ac7065b2 | 70 -1879.14 -101.00 1 17 | 10 321.10 1398.42 1 3 7 9 3 0 | 223 -577.56 1399.82 1 0 2 2 0 258 | 59 550.29 1398.07 -1 32 2 9 3 258
860 5acd1065 | 70 -1907.97 -101.00 1 17 | 236 321.10 1398.42 1 3 7 9 3 0 | 223 -577.56 1399.82 1 0 2 2 0 250 | 50 382.29 1398.33 -1 37 2 9 3 250
870 da7d2e6c | 70 -1944.01 -101.00 1 17 | 239 321.10 1398.42 1 3 7 9 3 0 | 223 -577.56 1399.82 1 0 2 2 0 240 | 67 985.15 1432.51 -1 37 2 9 3 240
872 e7d2abf3 | 70 -1951.21 -101.00 1 17 | 239 321.10 1398.42 1 3 7 9 3 0 | 223 -577.56 1399.82 1 0 2 2 0 238 | 67 1104.79 1443.07 -1 37 2 9 3 238
873 0d4490ce | 70 -1954.82 -101.00 1 17 | 239 321.10 1398.42 1 3 7 9 3 0 | 223 -577.56 1399.82 1 0 2 2 0 237 | 67 1163.56 1442.99 -1 37 9 9 3 237
880 9e58e522 | 70 -1980.04 -101.00 1 17 | 239 321.10 1398.42 1 3 7 9 3 0 | 10 -577.56 1399.82 1 0 2 2 0 230 | 67 1542.51 1337.60 -1 37 9 9 3 230
890 45a88dff | 70 -2016.08 -101.00 1 17 | 238 321.10 1398.42 1 3 7 9 3 0 | 225 -577.56 1399.82 1 0 2 2 0 220 | 67 2020.72 1227.57 -1 37 9 9 3 220
900 59ab22c8 | 70 -2052.12 -101.00 1 17 | 10 321.10 1398.42 1 3 7 9 3 0 | 225 -270.89 2316.77 1 0 2 2 0 210 | 79 2220.00 1230.00 -1 37 9 9 3 210
901 59ab22c8 | 70 -2055.72 -101.00 1 17 | 10 321.10 1398.42 1 3 7 9 3 0 | 225 -220.76 2454.10 1 0 2 2 0 209 | 79 2220.00 1230.00 -1 37 9 9 3 209
902 c310e89f | 70 -2059.32 -101.00 1 17 | 10 321.10 1398.42 1 3 9 9 3 0 | 225 -173.81 2577.34 1 0 2 2 0 208 | 79 2220.00 1230.00 -1 37 9 9 3 208
910 9b82abc5 | 70 -2088.15 -101.00 1 17 | 10 321.10 1398.42 1 3 9 9 3 0 | 225 115.34 3122.58 1 0 2 2 0 200 | 79 2220.00 1230.00 -1 37 9 9 3 200
920 5c8c5ded | 70 -2124.19 -101.00 1 17 | 20 321.10 1398.42 1 3 9 9 3 0 | 225 312.66 3015.28 1 0 2 2 0 190 | 79 2220.00 1230.00 -1 37 9 9 3 190
930 71ca2464 | 70 -2160.22 -101.00 1 17 | 212 321.10 2208.42 1 3 9 9 3 0 | 58 399.35 2534.09 1 0 2 2 0 180 | 79 2220.00 1230.00 -1 37 9 9 3 180
934 dab8fc2a | 70 -2174.64 -101.00 1 17 | 212 321.10 2267.42 1 3 9 9 3 0 | 54 404.04 2476.99 -1 2 2 2 0 176 | 79 2220.00 1230.00 -1 37 9 9 3 176
935 9a4eaf32 | 72 -2178.24 -101.00 1 17 | 212 321.10 2267.42 1 3 2 9 3 0 | 54 404.04 2476.99 -1 2 2 2 0 175 | 79 2220.00 1230.00 -1 37 9 9 3 175
940 1ff82f96 | 72 -2196.26 -101.00 1 17 | 212 321.10 2502.42 1 3 2 9 3 0 | 48 404.04 2862.49 -1 15 2 2 0 170 | 79 2220.00 1230.00 -1 37 9 9 3 170
941 606ca784 | 72 -2199.86 -101.00 1 17 | 212 321.10 2502.42 1 3 2 9 3 0 | 48 404.04 3030.49 -1 15 2 2 0 169 | 79 2220.00 1230.00 -1 37 9 9 3 169
942 b1c17ccc | 72 -2203.47 -101.00 1 17 | 212 321.10 2502.42 1 3 9 9 3 0 | 48 404.04 3030.49 -1 15 2 2 0 168 | 79 2220.00 1230.00 -1 37 9 9 3 168
944 0d682c39 | 72 -2210.67 -101.00 1 17 | 212 321.10 2502.42 1 3 9 9 3 0 | 48 404.04 3030.49 -1 15 2 2 0 166 | 26 2229.30 1230.00 -1 37 9 9 3 166
945 08583e18 | 72 -2214.28 -101.00 1 17 | 212 321.10 2502.42 1 3 9 9 3 0 | 48 404.04 3030.49 -1 15 2 2 0 165 | 26 2238.75 1227.99 -1 37 4 9 3 165
950 1713eecd | 72 -2232.30 -101.00 1 17 | 212 321.10 2568.42 1 3 9 9 3 0 | 48 404.04 3109.32 -1 15 2 2 0 160 | 26 2233.65 1186.16 -1 37 4 9 3 160
960 8d17d15a | 72 -2268.33 -101.00 1 17 | 212 321.10 2658.42 1 3 9 9 3 0 | 48 404.04 3257.45 -1 15 2 2 0 150 | 26 2063.15 944.66 -1 37 4 9 3 150
970 ac88d725 | 10 -2304.37 -101.00 1 17 | 212 321.10 2348.42 1 3 9 9 3 0 | 48 404.04 3016.37 -1 15 2 2 0 140 | 26 1839.65 537.26 -1 37 4 9 3 140
980 06efc1d0 | 10 -2340.40 -101.00 1 17 | 26 342.10 1758.42 1 3 9 9 3 0 | 27 390.54 2592.02 -1 15 2 2 0 130 | 24 1602.90 894.26 -1 37 4 9 3 130
986 11469246 | 10 -2362.03 -101.00 1 17 | 26 436.60 1398.42 1 3 9 9 3 0 | 27 375.14 2328.02 -1 15 2 2 0 124 | 24 1468.80 1226.36 -1 37 4 9 3 124
987 5d85b3f2 | 10 -2365.63 -101.00 1 17 | 31 459.70 1398.21 1 3 9 9 3 0 | 27 377.04 2284.02 -1 15 2 2 0 123 | 24 1446.60 1274.36 -1 37 9 9 3 123
990 e6fb4147 | 10 -2376.44 -101.00 1 17 | 31 481.90 1398.17 1 3 9 9 3 0 | 27 393.54 2152.02 -1 15 2 2 0 120 | 24 1380.90 1405.76 -1 37 9 9 3 120
1000 391794f4 | 10 -2405.27 -101.00 1 17 | 13 830.30 1418.85 1 3 9 9 3 0 | 213 475.54 1712.02 -1 15 2 2 0 110 | 213 1171.65 1707.26 -1 37 9 9 3 110
1010 289575cc | 10 -2405.27 -101.00 1 17 | 41 989.88 1432.93 1 6 9 9 3 0 | 219 526.74 1398.10 -1 15 2 2 0 100 | 213 1053.30 1787.36 -1 37 9 9 3 100
1020 4f32b7cb | 10 -2405.27 -101.00 1 17 | 41 948.77 1429.30 1 9 9 9 3 0 | 219 526.74 1398.10 -1 15 2 2 0 90 | 213 977.40 1798.76 -1 37 9 9 3 90
1030 7f8bf0d6 | 10 -2405.27 -101.00 1 17 | 41 901.87 1425.16 -1 15 9 9 3 0 | 219 526.74 1398.10 -1 15 2 2 0 80 | 213 885.90 1765.76 -1 37 9 9 3 80
1031 f775606b | 10 -2405.27 -101.00 1 17 | 41 901.87 1425.16 -1 15 2 9 3 0 | 219 526.74 1398.10 -1 15 2 2 0 79 | 213 885.90 1765.76 -1 37 9 9 3 79
1036 e193ce11 | 10 -2405.27 -101.00 1 17 | 41 913.64 1426.20 -1 15 2 9 3 0 | 219 526.74 1398.10 -1 15 2 2 0 74 | 213 868.05 1752.86 -1 37 9 9 3 74
1037 498ca079 | 10 -2405.27 -101.00 1 17 | 41 923.42 1427.07 -1 15 2 9 3 0 | 219 526.74 1398.10 -1 15 2 2 0 73 | 213 850.35 1737.86 -1 37 2 9 3 73
1040 498ca079 | 10 -2405.27 -101.00 1 17 | 41 940.80 1428.60 -1 15 2 9 3 0 | 219 526.74 1398.10 -1 15 2 2 0 70 | 213 798.15 1680.26 -1 37 2 9 3 70
1050 4c6f0f20 | 10 -2405.27 -101.00 1 17 | 41 942.61 1428.76 -1 15 2 9 3 0 | 153 519.99 1398.12 -1 15 2 2 0 60 | 31 643.35 1402.35 -1 37 2 9 3 60
1060 02d6d5aa | 10 -2405.27 -101.00 1 17 | 153 942.61 1428.76 -1 15 2 9 3 0 | 157 486.24 1398.17 -1 15 2 2 0 50 | 10 686.54 1406.17 -1 37 2 9 3 50
1070 6a89f678 | 10 -2405.27 -101.00 1 17 | 166 935.86 1428.16 -1 15 2 9 3 0 | 157 1270.76 1430.38 -1 15 2 2 0 40 | 157 686.54 1406.17 -1 37 2 9 3 40
1071 6a89f678 | 10 -2405.27 -101.00 1 17 | 167 935.86 1428.16 -1 15 2 9 3 0 | 157 1325.26 1423.97 -1 15 2 2 0 39 | 171 554.22 1402.11 1 37 2 9 3 39
1072 ee332381 | 10 -2405.27 -101.00 1 17 | 167 935.86 1428.16 -1 15 2 9 3 0 | 157 1380.08 1405.97 -1 15 2 2 0 38 | 171 666.87 1404.43 1 37 9 9 3 38
1080 78e99bdc | 10 -2405.27 -101.00 1 17 | 168 935.86 1428.16 -1 15 2 9 3 0 | 157 1497.66 1356.48 -1 15 2 2 0 30 | 172 769.06 1413.45 1 37 9 9 3 30
1083 95332c28 | 10 -2405.27 -101.00 1 17 | 170 935.86 1428.16 -1 15 2 9 3 0 | 157 1488.56 1360.31 -1 15 2 2 0 27 | 186 786.66 1663.55 1 37 9 9 3 27
1084 1426bcf1 | 10 -2405.27 -101.00 1 17 | 170 935.86 1428.16 -1 15 2 9 3 0 | 157 1485.53 1361.58 -1 15 2 2 0 26 | 186 801.39 1669.71 1 37 2 9 3 26
1090 1ab83156 | 10 -2405.27 -101.00 1 17 | 170 1140.06 1445.76 -1 15 2 9 3 0 | 153 1482.50 1362.86 -1 15 2 2 0 20 | 186 1324.89 1625.52 1 37 2 9 3 20
1096 39543a80 | 10 -2405.27 -101.00 1 17 | 170 1234.42 1434.66 -1 15 2 9 3 0 | 155 1482.50 1362.86 -1 15 2 2 0 14 | 186 1496.05 1751.78 1 37 2 9 3 14
1097 2c751d1f | 10 -2405.27 -101.00 1 17 | 170 1234.42 1434.66 -1 15 2 9 3 0 | 155 1482.50 1362.86 -1 15 2 2 0 13 | 186 1496.05 1751.78 1 37 9 9 3 13
1100 2c751d1f | 10 -2405.27 -101.00 1 17 | 170 1234.42 1434.66 -1 15 2 9 3 0 | 155 1514.20 1349.51 -1 15 2 2 0 10 | 186 1504.42 1749.66 1 37 9 9 3 10
1109 5199bd3b | 10 -2405.27 -101.00 1 17 | 170 1234.42 1434.66 -1 15 2 9 3 0 | 155 1589.78 1317.70 -1 15 2 2 0 1 | 52 1575.43 1575.32 -1 52 9 9 3 1
1110 622eb7fe | 10 -2405.27 -101.00 1 17 | 170 1234.42 1434.66 -1 15 9 9 3 0 | 155 1589.78 1317.70 -1 15 2 2 0 0 | 52 1575.43 1575.32 -1 52 9 9 3 0
1120 c38264e7 | 10 -2405.27 -101.00 1 17 | 170 1234.42 1434.66 -1 15 9 9 3 0 | 155 1635.64 1298.39 -1 15 2 2 0 0 | 52 2166.57 2092.96 -1 52 9 9 3 0
1121 a6b1793c | 10 -2405.27 -101.00 1 17 | 170 1234.42 1434.66 -1 15 9 9 3 0 | 155 1635.64 1298.39 -1 15 2 2 0 0 | 52 2246.21 2153.70 -1 52 9 9 3 0
1122 a6b1793c | 10 -2405.27 -101.00 1 17 | 170 1234.42 1434.66 -1 15 9 9 3 0 | 155 1635.64 1298.39 -1 15 2 2 0 0 | 52 2324.65 2211.14 -1 52 4 9 3 0
1130 ca9ff315 | 10 -2405.27 -101.00 1 17 | 152 1234.42 1434.66 -1 15 9 9 3 0 | 154 1635.64 1298.39 -1 15 2 2 0 0 | 52 2908.88 2551.77 -1 52 4 9 3 0
1140 ca9ff315 | 10 -2405.27 -101.00 1 17 | 157 1226.99 1435.53 -1 15 9 9 3 0 | 10 1635.64 1298.39 -1 15 2 2 0 0 | 56 3193.47 2889.70 -1 52 4 9 3 0
1150 0a55a42b | 10 -2405.27 -101.00 1 17 | 157 1679.19 1280.06 -1 15 9 9 3 0 | 10 1655.89 1289.87 -1 15 2 2 0 0 | 56 3254.63 2988.79 -1 52 4 9 3 0
1153 82a353a6 | 10 -2405.27 -101.00 1 17 | 157 1815.61 1225.07 -1 15 9 9 3 0 | 10 1635.64 1298.39 -1 15 2 2 0 0 | 56 3267.12 2944.94 -1 52 4 9 3 0
1154 f3a8502e | 10 -2405.27 -101.00 1 17 | 157 1863.05 1225.65 -1 15 2 9 3 0 | 10 1628.89 1301.23 -1 15 2 2 0 0 | 56 3270.68 2922.78 -1 52 4 9 3 0
1160 f3a8502e | 10 -2405.27 -101.00 1 17 | 157 2138.01 1229.00 -1 15 2 9 3 0 | 10 1628.89 1301.23 -1 15 2 2 0 0 | 56 3237.33 2782.80 1 52 4 9 3 0
1170 f3a8502e | 10 -2405.27 -101.00 1 17 | 166 2220.00 1230.00 -1 15 2 9 3 0 | 152 1628.89 1301.23 -1 15 2 2 0 0 | 56 3103.54 2634.74 1 52 4 9 3 0
1171 f3a8502e | 10 -2405.27 -101.00 1 17 | 166 2220.00 1230.00 -1 15 2 9 3 0 | 152 1628.89 1301.23 -1 15 2 2 0 0 | 56 3099.47 2609.19 1 52 4 9 3 0
1172 0579c8cd | 10 -2405.27 -101.00 1 17 | 166 2220.00 1230.00 -1 15 9 9 3 0 | 157 1628.89 1301.23 -1 15 2 2 0 0 | 57 3083.96 2581.69 1 52 4 9 3 0
1180 ec345462 | 10 -2405.27 -101.00 1 17 | 166 2220.00 1230.00 -1 15 9 9 3 0 | 157 2040.85 1227.82 -1 15 2 2 0 0 | 57 2889.31 2289.73 1 52 4 9 3 0
1186 ec345462 | 10 -2405.27 -101.00 1 17 | 166 2199.75 1229.75 -1 15 9 9 3 0 | 157 2220.00 1230.00 -1 15 2 2 0 0 | 57 2761.21 2037.73 1 52 4 9 3 0
1187 bb21a26a | 10 -2405.27 -101.00 1 17 | 166 2193.00 1229.67 -1 15 2 9 3 0 | 157 2220.00 1230.00 -1 15 2 2 0 0 | 57 2738.86 1995.73 1 52 4 9 3 0
1190 bb21a26a | 10 -2405.27 -101.00 1 17 | 166 2172.75 1229.42 -1 15 2 9 3 0 | 157 2220.00 1230.00 -1 15 2 2 0 0 | 57 2671.81 1869.73 1 52 4 9 3 0
1200 bb21a26a | 10 -2398.07 -101.00 1 17 | 166 2105.25 1228.60 -1 15 2 9 3 0 | 157 2205.38 1229.82 -1 15 2 2 0 0 | 57 2448.31 1449.73 1 52 4 9 3 0
1210 e2b021c3 | 10 -2362.07 -101.00 1 17 | 166 2071.50 1228.19 -1 15 2 9 3 0 | 26 2253.09 1194.00 -1 15 2 2 0 0 | 57 2224.81 1029.73 1 52 4 9 3 0
1216 9b03f655 | 10 -2340.47 -101.00 1 17 | 155 2071.50 1228.19 -1 15 2 9 3 0 | 212 2278.84 1098.00 -1 15 2 2 0 0 | 57 2090.70 777.73 1 52 4 9 3 0
1217 9b03f655 | 10 -2336.87 -101.00 1 17 | 155 2071.50 1228.19 -1 15 2 9 3 0 | 212 2278.84 1098.00 -1 15 4 2 0 0 | 57 2068.35 735.73 1 52 4 9 3 0
1220 9b03f655 | 10 -2326.07 -101.00 1 17 | 155 2071.50 1228.19 -1 15 2 9 3 0 | 212 2278.84 1098.00 -1 15 4 2 0 0 | 57 2001.30 609.73 1 52 4 9 3 0
1230 30f8e56b | 10 -2290.07 -101.00 1 17 | 155 1957.11 1226.79 -1 15 2 9 3 0 | 212 2256.49 862.80 -1 15 4 2 0 0 | 25 1767.01 535.33 1 52 4 9 3 0
1240 deb18b11 | 10 -2254.06 -101.00 1 17 | 155 1957.11 1226.79 -1 15 2 9 3 0 | 212 2070.99 423.60 -1 15 4 2 0 0 | 25 1541.06 1130.83 1 52 4 9 3 0
1243 2e358190 | 10 -2243.26 -101.00 1 17 | 155 1957.11 1226.79 -1 15 2 9 3 0 | 212 1982.94 291.60 -1 15 4 2 0 0 | 25 1474.01 1268.53 1 52 4 9 3 0
1244 46d57d79 | 10 -2239.66 -101.00 1 17 | 155 1957.11 1226.79 -1 15 2 9 3 0 | 212 1953.14 247.60 -1 15 4 2 0 0 | 25 1451.66 1310.23 1 52 9 9 3 0
1250 babe1fbb | 10 -2218.06 -101.00 1 17 | 155 1957.11 1226.79 -1 15 2 9 3 0 | 212 1774.34 -16.40 -1 15 4 2 0 0 | 212 1340.36 1487.23 1 52 9 9 3 0
1260 64d0eb4c | 10 -2182.06 -101.00 1 17 | 155 2049.74 1227.92 -1 15 2 9 3 0 | 212 1476.34 -456.40 -1 15 4 2 0 0 | 212 1275.11 1568.23 1 52 9 9 3 0
1270 9bb59302 | 10 -2146.06 -101.00 1 17 | 155 2071.50 1228.19 -1 15 2 9 3 0 | 24 1320.00 -40.40 -1 15 4 2 0 0 | 212 1063.11 1701.73 1 52 9 9 3 0
1280 9fe0323c | 10 -2110.06 -101.00 1 17 | 155 2071.50 1228.19 -1 15 2 9 3 0 | 24 1171.11 373.60 -1 15 4 2 0 0 | 212 839.61 1625.23 1 52 9 9 3 0
1281 632a4d83 | 10 -2106.46 -101.00 1 17 | 154 2071.50 1228.19 -1 15 2 9 3 0 | 24 1170.00 401.80 -1 15 4 2 0 0 | 212 817.26 1606.03 1 52 9 9 3 0
1282 2e3967cc | 10 -2102.86 -101.00 1 17 | 154 2071.50 1228.19 -1 15 2 9 3 0 | 24 1140.40 427.60 -1 15 2 2 0 0 | 212 794.91 1584.73 1 52 9 9 3 0
1290 39651871 | 10 -2074.06 -101.00 1 17 | 154 2071.50 1228.19 -1 15 2 9 3 0 | 24 910.80 547.60 -1 15 2 2 0 0 | 31 618.98 1400.20 1 52 9 9 3 0
1300 99406a6b | 10 -2038.06 -101.00 1 17 | 10 2071.50 1228.19 -1 15 2 9 3 0 | 10 641.80 504.65 -1 15 2 2 0 0 | 223 560.60 1398.05 1 52 9 9 3 0
1301 e9f33e25 | 10 -2034.46 -101.00 1 17 | 12 2053.50 1227.97 -1 15 9 9 3 0 | 10 622.38 509.42 -1 15 2 2 0 0 | 223 560.60 1398.05 1 52 9 9 3 0
1310 5f00ebab | 10 -2002.06 -101.00 1 17 | 15 1411.55 1392.72 -1 15 9 9 3 0 | 10 598.78 510.00 -1 15 2 2 0 0 | 223 560.60 1398.05 1 52 9 9 3 0
1320 2cc2b521 | 10 -1966.06 -101.00 1 17 | 37 1313.91 1425.30 -1 21 9 9 3 0 | 18 598.78 510.00 1 15 2 2 0 0 | 223 560.60 1398.05 1 52 9 9 3 0
1330 590baef7 | 10 -1930.06 -101.00 1 17 | 37 1378.21 1406.76 -1 21 9 9 3 0 | 12 704.36 489.29 1 15 2 2 0 0 | 223 560.60 1398.05 1 52 9 9 3 0
1340 e800533a | 10 -1894.06 -101.00 1 17 | 10 1378.21 1406.76 -1 21 9 9 3 0 | 12 820.90 460.69 1 15 2 2 0 0 | 10 560.60 1398.05 1 52 9 9 3 0
1350 86038634 | 10 -1858.06 -101.00 1 17 | 10 1378.21 1406.76 -1 21 9 9 3 0 | 12 937.44 432.08 1 15 2 2 0 0 | 10 560.60 1398.05 1 52 9 9 3 0
1360 119ca606 | 10 -1822.06 -101.00 1 17 | 18 1378.21 1406.76 1 21 9 9 3 0 | 12 1053.98 403.48 1 15 2 2 0 0 | 223 560.60 1398.05 1 52 9 9 3 0
1370 38b03dda | 10 -1786.06 -101.00 1 17 | 12 1511.35 1350.72 1 21 9 9 3 0 | 18 1083.12 396.33 -1 15 2 2 0 0 | 223 560.60 1398.05 1 52 9 9 3 0
1380 fd5ef48d | 10 -1750.06 -101.00 1 17 | 12 1677.25 1280.88 1 21 9 9 3 0 | 12 1013.19 413.49 -1 15 2 2 0 0 | 223 560.60 1398.05 1 52 9 9 3 0
1390 dddcc15f | 10 -1714.06 -101.00 1 17 | 12 1844.56 1225.42 1 21 9 9 3 0 | 12 896.65 442.09 -1 15 2 2 0 0 | 223 560.60 1398.05 1 52 9 9 3 0
1400 ab9433ca | 10 -1678.06 -101.00 1 17 | 37 1927.92 1226.44 -1 26 9 9 3 0 | 224 950.52 756.75 1 15 2 2 0 0 | 10 593.80 1398.00 1 52 9 9 3 0
1410 c9fbb9d8 | 10 -1642.06 -101.00 1 17 | 18 1968.03 1226.93 -1 26 9 9 3 0 | 224 1131.52 1191.75 1 15 2 2 0 0 | 10 604.58 1398.93 1 52 9 9 3 0
1416 9446d601 | 10 -1620.46 -101.00 1 17 | 18 1968.03 1226.93 1 26 9 9 3 0 | 224 1241.32 1337.55 1 15 2 2 0 0 | 10 604.58 1398.93 1 52 9 9 3 0
1417 962d60c0 | 10 -1616.86 -101.00 1 17 | 18 1968.03 1226.93 1 26 7 9 3 0 | 224 1258.92 1353.45 1 15 2 2 0 0 | 10 604.58 1398.93 1 52 9 9 3 0
1420 6d917c83 | 10 -1606.06 -101.00 1 17 | 236 1968.03 1226.93 1 26 7 9 3 0 | 224 1312.52 1386.75 1 15 2 2 0 0 | 10 604.58 1398.93 1 52 9 9 3 0
1430 b0953470 | 10 -1570.06 -101.00 1 17 | 239 1968.03 1226.93 -1 26 7 9 3 0 | 46 1386.52 1397.55 1 28 2 2 0 0 | 223 604.58 1398.93 1 52 9 9 3 0
1440 d3c7320b | 10 -1534.06 -101.00 1 17 | 238 1968.03 1226.93 -1 26 7 9 3 0 | 46 1471.43 1388.62 1 28 2 2 0 0 | 223 604.58 1398.93 1 52 9 9 3 0
1450 e65803a3 | 10 -1498.06 -101.00 1 17 | 238 1968.03 1226.93 -1 26 7 9 3 0 | 223 1460.70 1372.03 1 28 2 2 0 0 | 223 604.58 1398.93 1 52 9 9 3 0
1458 a2d58a10 | 10 -1469.26 -101.00 1 17 | 10 1968.03 1226.93 -1 26 7 9 3 0 | 223 1460.70 1372.03 1 28 2 2 0 0 | 223 604.58 1398.93 1 52 9 9 3 0
1459 11eace76 | 10 -1465.66 -101.00 1 17 | 10 1968.03 1226.93 -1 26 9 9 3 0 | 223 1460.70 1372.03 1 28 2 2 0 0 | 223 604.58 1398.93 1 52 9 9 3 0
1460 7e2d8a75 | 10 -1462.06 -101.00 1 17 | 10 1968.03 1226.93 -1 26 9 9 3 0 | 223 1460.70 1372.03 1 28 2 2 0 0 | 223 604.58 1398.93 1 52 9 9 3 0
1470 8bc4cda8 | 10 -1426.06 -101.00 1 17 | 40 1968.03 1226.93 -1 38 9 9 3 0 | 223 1460.70 1372.03 1 28 2 2 0 0 | 10 625.70 1400.80 1 52 9 9 3 0
1480 59be9313 | 10 -1390.06 -101.00 1 17 | 40 2062.39 1228.08 -1 38 9 9 3 0 | 223 1460.70 1372.03 1 28 2 2 0 0 | 22 753.26 1944.95 1 52 9 9 3 0
1490 27000707 | 10 -1354.06 -101.00 1 17 | 156 2062.87 1228.08 -1 38 9 9 3 0 | 223 1460.70 1372.03 1 28 2 2 0 0 | 22 914.51 2542.45 1 52 9 9 3 0
1500 71238129 | 10 -1320.00 -101.00 1 17 | 156 1378.49 1406.64 -1 38 9 9 3 0 | 195 1433.70 1383.40 1 28 2 2 0 0 | 22 1066.76 2929.95 1 52 9 9 3 0
";

/// The Mario Bros stage's three CPUs from Go (`ftComputerProcessTrait`'s
/// Ally and Mario Bros behaviours, team targeting, objective switches) as
/// the N64 plays them.
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn mario_bros_stage_cpus_play_as_on_the_n64() {
    let Some((first_bad, log)) = one_player_replay(
        ssb_game::spgame::Stage::Mario,
        Some(391),
        MARIO_BROS_STAGE_1P,
    ) else {
        return;
    };
    if std::env::var_os("SCENE_TRACE").is_some() {
        println!("{log}");
    }
    assert_eq!(first_bad, None, "{log}");
}

/// `sp6g` frame 390 (`gGRCommonStruct.jungle`, 0x80131410): the Barrel in
/// Move with `tarucann_wait` 238 and `rotate_step` 0.07.
const JUNGLE_1P_BARREL_WAIT: u16 = 238;

/// `sp8g` frame 390 (`gGRCommonStruct.pupupu`, 0x80131410): Whispy just
/// woke (Wait) with these waits; the rest are `grPupupuInitAll`'s.
const PUPUPU_1P_WIND_WAIT: u16 = 1851;
const PUPUPU_1P_BLINK_WAIT: i16 = 197;

/// `gITManagerAppearActor.spawn_wait` (0x8018D0B8 + 8) at frame 390 of
/// `sp6a` and `sp8a`: the actor counts from frame 390 on.
const APPEAR_1P_DONKEY_SPAWN_WAIT: u16 = 939;
const APPEAR_1P_KIRBY_SPAWN_WAIT: u16 = 1253;

/// N64 `sp6l` (warp ROM `warp1p_6`: 1P stage 6, Kongo Jungle, Easy):
/// Mario (P1, idle), Fox and Kirby (allies, Ally trait) against Giant
/// Donkey Kong (`nFTComputerTraitGiantDonkey`). See [`one_player_replay`].
const GIANT_DONKEY_STAGE_1P: &str = "
390 e6c46dbb | 10 -1739.00 0.00 1 0 | 10 -840.00 -137.78 1 0 0 0 0 480 | 10 0.00 -299.00 1 0 0 0 0 480 | 10 1859.00 1522.09 -1 0 0 0 0 480
391 8822e463 | 10 -1739.00 0.00 1 0 | 10 -840.00 -137.78 1 0 9 9 3 479 | 10 0.00 -299.00 1 0 9 9 3 479 | 10 1859.00 1522.09 -1 0 2 2 0 479
400 59913d9a | 10 -1739.00 0.00 1 0 | 10 -840.00 -137.78 1 0 9 9 3 470 | 10 0.00 -299.00 1 0 9 9 3 470 | 10 1859.00 1522.09 -1 0 2 2 0 470
410 59913d9a | 10 -1739.00 0.00 1 0 | 10 -840.00 -137.78 1 0 9 9 3 460 | 10 0.00 -299.00 1 0 9 9 3 460 | 10 1859.00 1522.09 -1 0 2 2 0 460
420 92e148e4 | 10 -1739.00 0.00 1 0 | 18 -840.00 -137.78 1 0 9 9 3 450 | 12 14.00 -299.00 1 0 9 9 3 450 | 222 1818.03 1529.32 -1 0 2 2 0 450
430 73dadb49 | 10 -1739.00 0.00 1 0 | 18 -840.00 -137.78 -1 0 9 9 3 440 | 12 154.00 -299.00 1 0 9 9 3 440 | 224 1818.03 1529.32 -1 0 2 2 0 440
440 fbb95a2c | 10 -1739.00 0.00 1 0 | 12 -1009.50 -77.20 -1 0 9 9 3 430 | 12 293.49 -292.88 1 0 9 9 3 430 | 224 1818.03 1529.32 -1 0 2 2 0 430
450 704968b1 | 10 -1739.00 0.00 1 0 | 12 -1182.86 -33.68 -1 0 9 9 3 420 | 12 430.92 -266.19 1 0 9 9 3 420 | 224 1818.03 1529.32 -1 0 2 2 0 420
460 187ebcaa | 10 -1739.00 0.00 1 0 | 12 -1360.07 -2.13 -1 0 9 9 3 410 | 12 567.23 -235.27 1 0 9 9 3 410 | 224 1818.03 1529.32 -1 0 2 2 0 410
470 187ebcaa | 10 -1745.75 0.00 1 0 | 12 -1533.04 0.00 -1 0 9 9 3 400 | 12 699.06 -188.15 1 0 9 9 3 400 | 224 1818.03 1529.32 -1 0 2 2 0 400
480 1916b6ae | 10 -1813.25 0.00 1 0 | 10 -1637.54 0.00 -1 0 9 9 3 390 | 22 805.03 237.93 1 0 9 9 3 390 | 224 1818.03 1529.32 -1 0 2 2 0 390
490 99dad475 | 10 -1840.25 0.00 1 0 | 18 -1612.54 0.00 1 0 9 9 3 380 | 22 912.83 741.93 1 0 9 9 3 380 | 224 1818.03 1529.32 -1 0 2 2 0 380
500 f5c1bb4e | 10 -1840.25 0.00 1 0 | 12 -1450.54 0.00 1 0 9 9 3 370 | 22 970.63 1005.93 1 0 9 9 3 370 | 224 1818.03 1529.32 -1 0 2 2 0 370
510 6da68d7d | 10 -1840.25 0.00 1 0 | 12 -1271.93 -17.82 1 0 9 9 3 360 | 22 956.43 1029.93 1 0 9 9 3 360 | 224 1818.03 1529.32 -1 0 2 2 0 360
520 f613c58d | 10 -1840.25 0.00 1 0 | 12 -1094.72 -49.38 1 0 9 9 3 350 | 26 728.11 813.93 1 0 9 9 3 350 | 18 1818.03 1529.32 -1 0 2 2 0 350
530 97b8b83e | 10 -1840.25 0.00 1 0 | 12 -924.45 -107.59 1 0 9 9 3 340 | 26 453.11 377.13 1 0 9 9 3 340 | 154 1818.03 1529.32 1 0 2 2 0 340
540 95b44c2c | 10 -1840.25 0.00 1 0 | 12 -754.95 -168.18 1 0 9 9 3 330 | 223 240.61 5.13 1 0 9 9 3 330 | 154 1818.03 1529.32 1 0 2 2 0 330
550 fa0811f1 | 10 -1840.25 0.00 1 0 | 12 -585.45 -228.76 1 0 9 9 3 320 | 223 521.11 473.13 1 0 9 9 3 320 | 12 2014.99 1494.57 1 0 2 2 0 320
560 6cde71e7 | 10 -1840.25 0.00 1 0 | 12 -410.91 -270.07 1 0 9 9 3 310 | 223 743.61 701.13 1 0 9 9 3 310 | 10 2205.65 1460.93 1 0 2 2 0 310
570 f8761dc7 | 10 -1840.25 0.00 1 0 | 12 -233.89 -299.00 1 0 9 9 3 300 | 223 916.11 689.13 1 0 9 9 3 300 | 224 2197.77 1462.32 -1 0 2 2 0 300
580 0f77aa24 | 10 -1840.25 0.00 1 0 | 12 -53.89 -299.00 1 0 9 9 3 290 | 223 1036.29 437.13 1 0 9 9 3 290 | 224 2197.77 1462.32 -1 0 2 2 0 290
590 ec4e2633 | 10 -1840.25 0.00 1 0 | 12 126.11 -299.00 1 0 9 9 3 280 | 26 922.35 -18.87 1 0 9 9 3 280 | 224 2197.77 1462.32 -1 0 2 2 0 280
600 19ca5772 | 10 -1840.25 0.00 1 0 | 12 305.45 -290.56 1 0 9 9 3 270 | 224 1085.62 278.73 1 0 9 9 3 270 | 224 2197.77 1462.32 -1 0 2 2 0 270
610 cded3ae9 | 10 -1840.25 0.00 1 0 | 12 482.15 -256.24 1 0 9 9 3 260 | 224 1303.12 554.73 1 0 9 9 3 260 | 224 2197.77 1462.32 -1 0 2 2 0 260
620 09b108d8 | 10 -1840.25 0.00 1 0 | 10 638.43 -209.82 1 0 9 9 3 250 | 224 1470.62 590.73 1 0 9 9 3 250 | 33 2195.97 1450.32 -1 0 2 2 0 250
630 dd303215 | 10 -1840.25 0.00 1 0 | 10 639.04 -209.60 1 0 9 9 3 240 | 224 1541.84 386.73 1 0 9 9 3 240 | 33 2151.67 1098.32 -1 0 2 2 0 240
631 19568969 | 10 -1840.25 0.00 1 0 | 10 639.04 -209.60 1 0 9 9 3 239 | 224 1536.56 353.13 1 0 9 9 3 239 | 33 2142.57 998.32 -1 0 2 2 0 239
632 9d6e6afd | 10 -1840.25 0.00 1 0 | 10 639.04 -209.60 1 0 9 9 3 238 | 224 1529.22 317.13 1 0 9 9 3 238 | 33 2131.77 898.32 -1 0 4 2 0 238
633 55e70ad1 | 10 -1840.25 0.00 1 0 | 10 639.04 -209.60 1 0 9 9 3 237 | 224 1519.82 278.73 1 0 9 9 3 237 | 33 2119.27 798.32 -1 0 4 2 0 237
634 9e2fdce5 | 10 -1840.25 0.00 1 0 | 10 639.04 -209.60 1 0 9 9 3 236 | 224 1508.36 237.93 1 0 9 9 3 236 | 33 2105.07 698.32 -1 0 2 2 0 236
640 d3410423 | 10 -1840.25 0.00 1 0 | 10 639.04 -209.60 1 0 9 9 3 230 | 31 1398.26 0.00 1 0 9 9 3 230 | 33 1984.17 98.32 -1 0 2 2 0 230
650 3dfd9f62 | 10 -1840.25 0.00 1 0 | 18 639.04 -209.60 1 0 9 9 3 220 | 18 1366.24 -1.03 -1 0 9 9 3 220 | 32 1917.67 0.00 -1 0 2 2 0 220
660 48caa370 | 10 -1840.25 0.00 1 0 | 12 622.09 -215.66 -1 0 9 9 3 210 | 12 1283.54 -15.75 -1 0 9 9 3 210 | 207 1917.67 0.00 -1 0 2 2 0 210
670 e3807473 | 10 -1840.25 0.00 1 0 | 12 449.00 -262.68 -1 0 9 9 3 200 | 12 1145.71 -40.30 -1 0 9 9 3 200 | 207 1917.67 0.00 -1 0 2 2 0 200
680 363598d9 | 10 -1840.25 0.00 1 0 | 12 272.30 -297.00 -1 0 9 9 3 190 | 12 1010.88 -76.70 -1 0 9 9 3 190 | 207 1917.67 0.00 -1 0 2 2 0 190
690 40d328c3 | 10 -1840.25 0.00 1 0 | 12 92.63 -299.00 -1 0 9 9 3 180 | 12 879.05 -123.82 -1 0 9 9 3 180 | 207 1917.67 0.00 -1 0 2 2 0 180
700 f33411c5 | 10 -1840.25 0.00 1 0 | 12 -87.37 -299.00 -1 0 9 9 3 170 | 12 747.21 -170.94 -1 0 9 9 3 170 | 207 1917.67 0.00 -1 0 2 2 0 170
710 15a21a57 | 10 -1840.25 0.00 1 0 | 12 -267.37 -297.96 -1 0 9 9 3 160 | 12 615.38 -218.06 -1 0 9 9 3 160 | 207 1917.67 0.00 -1 0 2 2 0 160
720 ff5f7393 | 10 -1840.25 0.00 1 0 | 12 -444.07 -263.63 -1 0 9 9 3 150 | 12 481.31 -256.40 -1 0 9 9 3 150 | 12 1797.67 0.00 -1 0 2 2 0 150
730 2e085a42 | 10 -1840.25 0.00 1 0 | 18 -561.39 -237.36 -1 0 9 9 3 140 | 12 343.88 -283.10 -1 0 9 9 3 140 | 12 1597.67 0.00 -1 0 2 2 0 140
740 a1264263 | 10 -1840.25 0.00 1 0 | 12 -527.49 -247.43 1 0 9 9 3 130 | 12 205.42 -299.00 -1 0 9 9 3 130 | 12 1397.67 0.00 -1 0 2 2 0 130
750 a5ba1046 | 10 -1840.25 0.00 1 0 | 12 -350.79 -281.75 1 0 9 9 3 120 | 204 143.95 -299.00 1 0 9 9 3 120 | 12 1200.15 -30.60 -1 0 2 2 0 120
760 d0e6f591 | 10 -1840.25 0.00 1 0 | 12 -172.77 -299.00 1 0 9 9 3 110 | 204 612.70 -219.02 1 0 9 9 3 110 | 208 1023.89 -72.05 -1 12 2 2 0 110
770 12552275 | 10 -1840.25 0.00 1 0 | 227 -45.77 -299.00 1 0 9 9 3 100 | 204 670.37 -198.41 1 0 9 9 3 100 | 208 1023.14 -72.32 -1 12 2 2 0 100
780 fb9f62f4 | 10 -1840.25 0.00 1 0 | 229 -45.77 -299.00 1 0 9 9 3 90 | 51 791.32 -155.18 1 19 9 9 3 90 | 208 1043.39 -65.08 -1 12 2 2 0 90
781 a0196c2d | 10 -1840.25 0.00 1 0 | 229 -45.77 -299.00 1 0 9 9 3 89 | 51 791.32 -155.18 1 19 2 9 3 89 | 208 1043.39 -65.08 -1 12 2 2 0 89
790 359121d3 | 10 -1840.25 0.00 1 0 | 229 -45.77 -299.00 1 0 9 9 3 80 | 51 915.27 -21.25 1 19 2 9 3 80 | 208 1043.39 -65.08 -1 12 2 2 0 80
794 359121d3 | 10 -1840.25 0.00 1 0 | 229 -45.77 -299.00 1 0 9 9 3 76 | 51 747.60 235.58 1 19 2 9 3 76 | 208 1043.39 -65.08 -1 12 2 2 0 76
795 fb2bd518 | 10 -1840.25 0.00 1 0 | 229 -45.77 -299.00 1 0 9 9 3 75 | 51 707.80 290.10 1 19 9 9 3 75 | 208 1043.39 -65.08 -1 12 2 2 0 75
800 9e28805a | 10 -1840.25 0.00 1 0 | 53 -45.77 -299.00 1 19 9 9 3 70 | 51 521.58 504.64 1 19 9 9 3 70 | 208 1043.39 -65.08 -1 12 2 2 0 70
810 a2b04f55 | 10 -1840.25 0.00 1 0 | 53 122.23 -299.00 1 19 9 9 3 60 | 51 212.89 645.71 1 19 9 9 3 60 | 208 1043.39 -65.08 -1 12 2 2 0 60
820 00150f58 | 10 -1840.25 0.00 1 0 | 53 -250.63 126.82 1 19 9 9 3 50 | 51 -10.80 553.15 1 19 9 9 3 50 | 208 1043.39 -65.08 -1 12 2 2 0 50
830 dd93cdca | 10 -1840.25 0.00 1 0 | 53 -538.50 65.41 1 19 9 9 3 40 | 51 -149.49 313.37 1 19 9 9 3 40 | 208 1043.39 -65.08 -1 12 2 2 0 40
840 d9e74002 | 10 -1840.25 0.00 1 0 | 68 -741.36 -173.03 1 19 9 9 3 30 | 57 -194.68 -73.64 1 19 9 9 3 30 | 208 1043.39 -65.08 -1 12 2 2 0 30
850 c66c585f | 10 -1840.25 0.00 1 0 | 68 -797.57 -152.94 1 19 9 9 3 20 | 68 -195.17 -299.00 1 19 9 9 3 20 | 208 1043.39 -65.08 -1 12 2 2 0 20
860 97fb0827 | 10 -1840.25 0.00 1 0 | 68 -797.57 -152.94 1 19 9 9 3 10 | 68 -195.17 -299.00 1 19 9 9 3 10 | 232 1043.39 -65.08 -1 12 2 2 0 10
870 62f5f298 | 10 -1840.25 0.00 1 0 | 80 -797.57 -152.94 1 19 9 9 3 0 | 68 -195.17 -299.00 1 19 9 9 3 0 | 233 1043.39 -65.08 -1 12 2 2 0 0
880 caea8d2a | 10 -1840.25 0.00 1 0 | 80 -797.57 -152.94 1 19 9 9 3 0 | 80 -195.17 -299.00 1 19 9 9 3 0 | 233 1043.39 -65.08 -1 12 2 2 0 0
890 ad665abb | 10 -1840.25 0.00 1 0 | 80 -797.57 -152.94 1 19 9 9 3 0 | 82 -195.17 -299.00 1 19 9 9 3 0 | 233 1043.39 -65.08 -1 12 2 2 0 0
900 8eea33ce | 10 -1840.25 0.00 1 0 | 80 -797.57 -152.94 1 19 9 9 3 0 | 83 -248.93 -299.00 1 19 9 9 3 0 | 233 1043.39 -65.08 -1 12 2 2 0 0
910 515db388 | 10 -1840.25 0.00 1 0 | 80 -797.57 -152.94 1 19 9 9 3 0 | 10 -248.93 -299.00 1 19 9 9 3 0 | 12 968.06 -92.01 -1 12 2 2 0 0
920 82ac1070 | 10 -1840.25 0.00 1 0 | 12 -695.87 -189.29 1 19 9 9 3 0 | 10 -248.93 -299.00 1 19 9 9 3 0 | 12 779.73 -159.32 -1 12 2 2 0 0
930 44cf9dc8 | 10 -1840.25 0.00 1 0 | 12 -526.37 -247.65 1 19 9 9 3 0 | 204 -240.40 -299.00 1 19 9 9 3 0 | 233 684.05 -193.51 -1 12 2 2 0 0
940 30f8e56b | 10 -1840.25 0.00 1 0 | 227 -384.03 -275.30 1 19 9 9 3 0 | 204 237.84 -299.00 1 19 9 9 3 0 | 233 684.05 -193.51 -1 12 2 2 0 0
950 826d42b0 | 10 -1840.25 0.00 1 0 | 229 -384.03 -275.30 1 19 9 9 3 0 | 54 323.71 -287.01 1 29 9 9 3 0 | 233 684.05 -193.51 -1 21 2 2 0 0
960 c6bcf216 | 10 -1840.25 0.00 1 0 | 229 -384.03 -275.30 1 19 9 9 3 0 | 54 323.71 167.90 1 29 9 9 3 0 | 233 684.05 -193.51 -1 21 2 2 0 0
970 87c480ae | 10 -1840.25 0.00 1 0 | 229 -384.03 -275.30 1 19 9 9 3 0 | 54 323.71 1417.79 1 29 9 9 3 0 | 233 684.05 -193.51 -1 21 2 2 0 0
980 1583a2cf | 10 -1840.25 0.00 1 0 | 229 -384.03 -275.30 1 19 9 9 3 0 | 54 323.71 2272.08 1 29 9 9 3 0 | 233 684.05 -193.51 -1 21 2 2 0 0
990 f43c19e7 | 10 -1840.25 0.00 1 0 | 232 -384.03 738.10 1 19 9 9 3 0 | 54 323.71 2905.97 1 29 9 9 3 0 | 233 684.05 -193.51 -1 21 2 2 0 0
1000 0169db6d | 10 -1840.25 0.00 1 0 | 232 -384.03 1447.92 1 19 9 9 3 0 | 54 323.71 3369.85 1 29 9 9 3 0 | 233 684.05 -193.51 -1 21 2 2 0 0
1010 33737e19 | 10 -1840.25 0.00 1 0 | 232 -384.03 1854.17 1 19 9 9 3 0 | 54 323.71 3663.74 1 29 9 9 3 0 | 233 684.05 -193.51 -1 21 2 2 0 0
1020 594ce2b8 | 10 -1840.25 0.00 1 0 | 234 -268.53 1903.81 1 19 9 9 3 0 | 54 323.71 3787.63 1 29 9 9 3 0 | 233 684.05 -193.51 -1 21 2 2 0 0
1030 2201dff4 | 10 -1840.25 0.00 1 0 | 234 44.37 1553.46 1 19 9 9 3 0 | 54 323.71 3741.52 1 29 9 9 3 0 | 233 684.05 -193.51 -1 21 2 2 0 0
1040 5db3486d | 10 -1840.25 0.00 1 0 | 58 396.87 956.42 1 19 9 9 3 0 | 54 323.71 3525.41 1 29 9 9 3 0 | 233 684.05 -193.51 -1 21 2 2 0 0
1050 dafcd017 | 10 -1840.25 0.00 1 0 | 58 722.37 356.42 1 19 9 9 3 0 | 57 445.21 3139.30 1 29 9 9 3 0 | 20 684.05 -193.51 -1 21 2 2 0 0
1057 5dba22e9 | 10 -1840.25 0.00 1 0 | 59 932.37 -104.76 1 19 9 9 3 0 | 57 637.21 2803.30 1 29 9 9 3 0 | 22 677.53 -64.51 -1 21 2 2 0 0
1058 2316e5d0 | 10 -1840.25 0.00 1 0 | 59 952.24 -97.66 1 19 2 9 3 0 | 57 664.71 2755.30 1 29 9 9 3 0 | 22 670.98 60.49 -1 21 2 2 0 0
1060 f35742ae | 10 -1840.25 0.00 1 0 | 59 969.38 -91.54 1 19 2 9 3 0 | 57 716.01 2659.30 1 29 9 9 3 0 | 22 657.80 298.49 -1 21 2 2 0 0
1070 a1b3d259 | 10 -1840.25 0.00 1 0 | 152 969.38 -91.54 1 19 2 9 3 0 | 57 782.91 2179.30 1 29 9 9 3 0 | 209 592.38 1248.49 -1 21 2 2 0 0
1074 e7f7c7bd | 10 -1840.25 0.00 1 0 | 157 976.43 -89.02 1 19 2 9 3 0 | 53 614.91 2035.30 -1 44 9 9 3 0 | 209 576.55 1455.49 -1 21 2 2 0 0
1075 fb996391 | 10 -1840.25 0.00 1 0 | 157 979.95 -87.76 1 19 9 9 3 0 | 53 614.91 2035.30 -1 44 9 9 3 0 | 209 576.55 1455.49 -1 21 2 2 0 0
1080 bf98aea0 | 10 -1840.25 0.00 1 0 | 157 856.38 -131.92 1 19 9 9 3 0 | 53 614.91 2203.30 -1 44 9 9 3 0 | 209 576.55 1455.49 -1 21 2 2 0 0
1090 ee357333 | 10 -1840.25 0.00 1 0 | 157 203.59 -299.00 1 19 9 9 3 0 | 53 1089.48 2559.44 -1 44 9 9 3 0 | 209 547.55 1831.49 -1 21 2 2 0 0
1100 45e5332b | 10 -1840.25 0.00 1 0 | 157 -17.03 -299.00 1 19 9 9 3 0 | 53 1570.80 2684.28 -1 44 9 9 3 0 | 209 541.65 1941.49 -1 21 2 2 0 0
1110 bfa83e1b | 10 -1840.25 0.00 1 0 | 20 -22.28 -299.00 1 19 9 9 3 0 | 53 1927.79 2539.58 -1 44 9 9 3 0 | 209 558.65 1651.49 -1 21 2 2 0 0
1120 b45e864d | 10 -1840.25 0.00 1 0 | 213 34.24 448.00 1 19 9 9 3 0 | 53 2160.44 2276.53 -1 44 9 9 3 0 | 209 609.95 997.49 -1 21 2 2 0 0
1130 da48d6c1 | 10 -1840.25 0.00 1 0 | 213 67.36 766.00 1 19 9 9 3 0 | 57 2268.77 1897.55 -1 44 9 9 3 0 | 209 568.25 565.49 -1 25 2 2 0 0
1135 ad18d13b | 10 -1840.25 0.00 1 0 | 213 73.03 805.00 1 19 9 9 3 0 | 57 2317.79 1665.49 -1 44 9 9 3 0 | 209 561.35 493.49 -1 25 2 2 0 0
1136 600ccd76 | 10 -1840.25 0.00 1 0 | 213 78.40 840.00 1 19 9 9 3 0 | 57 2333.99 1617.49 -1 44 7 9 3 0 | 209 554.75 421.49 -1 25 2 2 0 0
1138 926fbbac | 10 -1840.25 0.00 1 0 | 213 83.47 871.00 1 19 9 9 3 0 | 57 2374.49 1521.49 -1 44 7 9 3 0 | 209 548.45 349.49 -1 27 2 2 0 0
1139 f1a30786 | 10 -1840.25 0.00 1 0 | 213 83.47 871.00 1 19 9 9 3 0 | 57 2398.79 1473.49 -1 44 9 9 3 0 | 209 548.45 349.49 -1 27 2 2 0 0
1140 6d346699 | 10 -1840.25 0.00 1 0 | 213 83.47 871.00 1 19 9 9 3 0 | 57 2419.39 1425.49 -1 44 9 9 3 0 | 209 548.45 349.49 -1 27 2 2 0 0
1150 e529a300 | 10 -1840.25 0.00 1 0 | 213 104.32 966.00 1 19 9 9 3 0 | 73 2148.77 1470.97 -1 44 9 9 3 0 | 209 521.45 -10.51 -1 29 2 2 0 0
1160 5a155ce9 | 10 -1840.25 0.00 1 0 | 213 119.44 948.00 1 19 9 9 3 0 | 73 1515.36 1582.71 -1 44 9 9 3 0 | 31 505.25 -251.75 -1 31 2 2 0 0
1170 8540c8b9 | 10 -1840.25 0.00 1 0 | 31 127.19 775.08 1 19 9 9 3 0 | 73 1348.93 1612.08 -1 44 9 9 3 0 | 10 505.25 -251.75 -1 31 2 2 0 0
1180 8540c8b9 | 10 -1840.25 0.00 1 0 | 10 128.29 860.98 1 19 9 9 3 0 | 73 1345.19 1612.73 -1 44 9 9 3 0 | 10 505.25 -251.75 -1 31 2 2 0 0
1190 5ad8f0d5 | 10 -1840.25 0.00 1 0 | 204 281.02 950.45 1 19 9 9 3 0 | 18 1345.19 1612.73 1 44 9 9 3 0 | 224 505.25 -251.75 -1 31 2 2 0 0
1200 03d3c399 | 10 -1840.25 0.00 1 0 | 204 662.84 1039.92 1 19 9 9 3 0 | 12 1469.28 1590.84 1 44 9 9 3 0 | 228 129.15 -299.00 -1 31 2 2 0 0
1210 61c343ef | 10 -1840.25 0.00 1 0 | 204 705.00 1129.40 1 19 9 9 3 0 | 33 1463.59 1550.47 1 44 9 9 3 0 | 228 49.15 -299.00 -1 31 2 2 0 0
1220 70b8f9e9 | 10 -1840.25 0.00 1 0 | 204 705.00 1218.87 1 19 9 9 3 0 | 33 1228.09 1298.47 1 44 9 9 3 0 | 228 49.15 -299.00 -1 31 2 2 0 0
1230 e24f21f0 | 10 -1840.25 0.00 1 0 | 204 705.00 1308.34 1 19 9 9 3 0 | 26 967.09 842.47 1 44 9 9 3 0 | 228 49.15 -299.00 -1 31 2 2 0 0
1240 a9b7d0c1 | 10 -1840.25 0.00 1 0 | 18 705.00 1397.80 -1 19 9 9 3 0 | 212 754.59 362.47 1 44 9 9 3 0 | 228 49.15 -299.00 -1 31 2 2 0 0
1250 a9ca58a5 | 10 -1840.25 0.00 1 0 | 12 561.00 1487.27 -1 19 9 9 3 0 | 212 579.29 -117.53 1 44 9 9 3 0 | 228 49.15 -299.00 -1 31 2 2 0 0
1260 b1287dfe | 10 -1840.25 0.00 1 0 | 12 381.00 1576.75 -1 19 9 9 3 0 | 219 516.36 -249.59 1 44 9 9 3 0 | 228 49.15 -299.00 -1 34 2 2 0 0
1270 fa46adac | 10 -1840.25 0.00 1 0 | 12 201.00 1666.22 -1 19 9 9 3 0 | 219 516.36 -249.59 1 44 9 9 3 0 | 228 49.15 -299.00 -1 34 2 2 0 0
1280 22818986 | 10 -1840.25 0.00 1 0 | 18 99.00 1755.70 -1 19 9 9 3 0 | 152 516.36 -249.59 1 44 9 9 3 0 | 10 49.15 -299.00 -1 34 2 2 0 0
1290 98d243f4 | 10 -1840.25 0.00 1 0 | 12 153.00 1845.17 1 19 9 9 3 0 | 153 516.36 -249.59 1 44 9 9 3 0 | 233 49.15 -299.00 -1 34 2 2 0 0
1300 db98a2e7 | 10 -1840.25 0.00 1 0 | 12 333.00 1934.64 1 19 9 9 3 0 | 154 516.36 -249.59 1 44 9 9 3 0 | 233 49.15 -299.00 -1 34 2 2 0 0
1310 9330c442 | 10 -1840.25 0.00 1 0 | 12 513.00 2024.11 1 19 9 9 3 0 | 54 516.36 -93.85 -1 53 9 9 3 0 | 233 49.15 -299.00 -1 34 2 2 0 0
1320 ebd76fb6 | 10 -1840.25 0.00 1 0 | 12 692.91 2087.35 1 19 9 9 3 0 | 54 516.36 1238.03 -1 53 9 9 3 0 | 233 49.15 -299.00 -1 34 2 2 0 0
1330 7d61ad85 | 10 -1840.25 0.00 1 0 | 33 642.67 2158.22 1 19 9 9 3 0 | 54 516.36 2162.32 -1 53 9 9 3 0 | 234 49.15 -299.00 -1 34 2 2 0 0
1340 cf71aa06 | 10 -1840.25 0.00 1 0 | 33 642.67 1898.22 1 19 9 9 3 0 | 54 516.36 2830.21 -1 53 9 9 3 0 | 10 49.15 -299.00 -1 34 2 2 0 0
1350 48548e2f | 10 -1840.25 0.00 1 0 | 33 586.27 1070.22 1 19 9 9 3 0 | 54 516.36 3328.10 -1 53 9 9 3 0 | 12 9.15 -299.00 -1 34 2 2 0 0
1360 d213ca66 | 10 -1840.25 0.00 1 0 | 212 479.77 110.22 1 19 9 9 3 0 | 54 516.36 3655.99 -1 53 9 9 3 0 | 12 -190.85 -299.00 -1 34 2 2 0 0
1370 843d6832 | 10 -1840.25 0.00 1 0 | 32 445.47 -263.36 1 19 9 9 3 0 | 54 516.36 3813.88 -1 53 9 9 3 0 | 12 -388.65 -274.40 -1 34 2 2 0 0
1380 5a6bef18 | 10 -1840.25 0.00 1 0 | 10 445.47 -263.36 1 19 9 9 3 0 | 54 516.36 3801.77 -1 53 9 9 3 0 | 12 -583.38 -229.50 -1 34 2 2 0 0
1390 f97e696d | 10 -1840.25 0.00 1 0 | 207 445.47 -263.36 1 19 9 9 3 0 | 54 516.36 3619.65 -1 53 9 9 3 0 | 12 -771.71 -162.18 -1 34 2 2 0 0
1400 965ac35b | 10 -1840.25 0.00 1 0 | 207 445.47 -263.36 1 19 9 9 3 0 | 57 440.76 3267.54 -1 53 9 9 3 0 | 12 -960.04 -94.87 -1 34 2 2 0 0
1410 6763a398 | 10 -1840.25 0.00 1 0 | 207 445.47 -263.36 1 19 9 9 3 0 | 57 175.36 2792.76 -1 53 9 9 3 0 | 12 -1150.95 -39.37 -1 34 2 2 0 0
1420 f1e43956 | 10 -1840.25 0.00 1 0 | 207 445.47 -263.36 1 19 9 9 3 0 | 57 -99.64 2312.76 -1 53 9 9 3 0 | 12 -1347.85 -4.30 -1 34 2 2 0 0
1430 ea015397 | 10 -1840.25 0.00 1 0 | 207 445.47 -263.36 1 19 9 9 3 0 | 68 -318.59 2021.62 -1 53 9 9 3 0 | 230 -1547.23 0.00 -1 34 2 2 0 0
1440 30964785 | 51 -1840.25 0.00 1 12 | 207 445.47 -263.36 1 19 9 9 3 0 | 68 -318.59 1932.15 -1 53 9 9 3 0 | 230 -1607.23 0.00 -1 34 2 2 0 0
1450 22142c06 | 51 -2469.58 468.68 1 12 | 18 445.47 -263.36 1 19 9 9 3 0 | 68 -318.59 1842.67 -1 53 9 9 3 0 | 230 -1787.23 0.00 -1 34 2 2 0 0
1460 e28e14de | 51 -3049.77 653.92 1 12 | 12 374.79 -277.09 -1 19 9 9 3 0 | 70 -318.59 1753.20 -1 53 9 9 3 0 | 230 -1942.23 0.00 -1 34 2 2 0 0
1470 8df8b92e | 51 -3504.62 630.71 1 12 | 12 197.10 -299.00 -1 19 9 9 3 0 | 70 -318.59 1663.73 -1 53 9 9 3 0 | 230 -1997.23 0.00 -1 34 2 2 0 0
1480 6e7f4b0b | 51 -3834.13 492.65 1 12 | 12 17.10 -299.00 -1 19 9 9 3 0 | 78 -297.15 1574.26 -1 53 9 9 3 0 | 230 -1952.23 0.00 -1 34 2 2 0 0
1490 f23824fc | 51 -4038.30 239.75 1 12 | 12 -162.90 -299.00 -1 19 9 9 3 0 | 78 -27.00 1484.78 -1 53 9 9 3 0 | 230 -1807.23 0.00 -1 34 2 2 0 0
1500 fd394d88 | 57 -4117.14 -128.01 1 12 | 12 -333.72 -285.07 -1 19 9 9 3 0 | 78 -27.00 1395.32 -1 53 9 9 3 0 | 230 -1568.23 0.00 -1 34 2 2 0 0
";

/// The Giant Donkey Kong stage's three CPUs from Go as the N64 plays them.
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn giant_donkey_stage_cpus_play_as_on_the_n64() {
    let Some((first_bad, log)) = one_player_replay(
        ssb_game::spgame::Stage::Donkey,
        Some(391),
        GIANT_DONKEY_STAGE_1P,
    ) else {
        return;
    };
    if std::env::var_os("SCENE_TRACE").is_some() {
        println!("{log}");
    }
    assert_eq!(first_bad, None, "{log}");
}

/// N64 `sp8l` (warp ROM `warp1p_8`: 1P stage 8, Dream Land, Easy):
/// Mario (P1, idle) against the Kirby Team's first two Kirbys
/// (`nFTComputerTraitKirbyTeam`). See [`one_player_replay`].
const KIRBY_TEAM_STAGE_1P: &str = "
390 6df109fd | 10 0.00 0.00 1 0 | 10 1.00 1542.00 -1 0 0 0 0 480 | 10 -1397.00 904.00 1 0 0 0 0 480
391 aea69ed3 | 10 0.00 0.00 1 0 | 10 1.00 1542.00 -1 0 10 10 8 479 | 10 -1397.00 904.00 1 0 10 10 8 479
400 12dbce4a | 10 0.00 0.00 1 0 | 10 1.00 1542.00 -1 0 10 10 8 470 | 10 -1397.00 904.00 1 0 10 10 8 470
410 12dbce4a | 10 0.00 0.00 1 0 | 10 1.00 1542.00 -1 0 10 10 8 460 | 12 -1383.00 904.00 1 0 10 10 8 460
420 0f3e862e | 10 0.00 0.00 1 0 | 18 1.00 1542.00 -1 0 10 10 8 450 | 244 -1371.80 904.00 1 0 10 10 8 450
430 0f3e862e | 10 0.00 0.00 1 0 | 18 1.00 1542.00 1 0 10 10 8 440 | 244 -1371.80 904.00 1 0 10 10 8 440
440 c40df552 | 10 0.00 0.00 1 0 | 12 141.00 1542.00 1 0 10 10 8 430 | 244 -1371.80 904.00 1 0 10 10 8 430
450 4db793af | 10 0.00 0.00 1 0 | 12 281.00 1542.00 1 0 10 10 8 420 | 244 -1371.80 904.00 1 0 10 10 8 420
460 074d495a | 10 0.00 0.00 1 0 | 18 334.20 1542.00 -1 0 10 10 8 410 | 244 -1371.80 904.00 1 0 10 10 8 410
470 e6bb16d7 | 10 0.00 0.00 1 0 | 12 264.20 1542.00 -1 0 10 10 8 400 | 244 -1371.80 904.00 1 0 10 10 8 400
480 701fe6a8 | 10 0.00 0.00 1 0 | 12 124.20 1542.00 -1 0 10 10 8 390 | 244 -1371.80 904.00 1 0 10 10 8 390
490 dcfd7fa9 | 10 0.00 0.00 1 0 | 12 -15.80 1542.00 -1 0 10 10 8 380 | 244 -1371.80 904.00 1 0 10 10 8 380
500 58b070e8 | 10 0.00 0.00 1 0 | 10 -150.20 1542.00 -1 0 10 10 8 370 | 244 -1371.80 904.00 1 0 10 10 8 370
510 2fff3adf | 10 0.00 0.00 1 0 | 33 -153.00 1474.80 -1 0 10 10 8 360 | 244 -1371.80 904.00 1 0 10 10 8 360
520 19d927f7 | 10 0.00 0.00 1 0 | 33 -125.40 878.00 -1 0 10 10 8 350 | 244 -1371.80 904.00 1 0 10 10 8 350
530 dc468abf | 10 0.00 0.00 1 0 | 212 -118.40 118.00 -1 0 10 10 8 340 | 244 -1371.80 904.00 1 0 10 10 8 340
531 1f16ccf4 | 52 0.00 0.00 -1 10 | 212 -118.40 42.00 -1 0 10 10 8 339 | 244 -1371.80 904.00 1 0 10 10 8 339
532 dd242b1e | 52 0.00 0.00 -1 10 | 212 -118.40 42.00 -1 0 2 10 8 338 | 244 -1371.80 904.00 1 0 10 10 8 338
540 f5c1bb4e | 48 52.53 45.74 -1 13 | 219 -118.40 0.00 -1 0 2 10 8 330 | 244 -1371.80 904.00 1 0 10 10 8 330
550 d1d14f0d | 48 269.26 197.44 -1 13 | 219 -118.40 0.00 -1 0 2 10 8 320 | 12 -1357.80 904.00 1 0 10 10 8 320
560 6d76735b | 48 531.01 165.52 -1 13 | 219 -118.40 0.00 -1 0 2 10 8 310 | 12 -1217.80 904.00 1 0 10 10 8 310
561 4963d4bc | 48 550.34 142.75 -1 13 | 219 -118.40 0.00 -1 0 2 10 8 309 | 12 -1203.80 904.00 1 0 10 10 8 309
562 f296bb9e | 48 568.44 116.43 -1 13 | 219 -118.40 0.00 -1 0 10 10 8 308 | 12 -1189.80 904.00 1 0 10 10 8 308
570 06076136 | 48 665.86 0.00 -1 13 | 153 -118.40 0.00 -1 0 10 10 8 300 | 12 -1077.80 904.00 1 0 10 10 8 300
580 d84158bd | 10 679.34 0.00 -1 13 | 166 -118.40 0.00 -1 0 10 10 8 290 | 10 -996.60 904.00 1 0 10 10 8 290
590 7f0d0445 | 10 679.34 0.00 -1 13 | 166 -118.40 0.00 -1 0 10 10 8 280 | 10 -996.60 904.00 1 0 10 10 8 280
600 f987eb5a | 10 679.34 0.00 -1 13 | 157 -50.15 0.00 -1 0 10 10 8 270 | 10 -996.60 904.00 1 0 10 10 8 270
610 8a2f5be6 | 10 679.34 0.00 -1 13 | 157 380.46 0.00 -1 0 10 10 8 260 | 18 -996.60 904.00 1 0 10 10 8 260
614 d8d56392 | 10 692.84 0.00 -1 13 | 157 563.47 0.00 -1 0 10 10 8 256 | 18 -996.60 904.00 -1 0 10 10 8 256
615 fc27e7f5 | 10 699.59 0.00 -1 13 | 157 618.70 0.00 -1 0 2 10 8 255 | 18 -996.60 904.00 -1 0 10 10 8 255
620 3e09deaa | 10 692.84 0.00 -1 13 | 157 828.51 0.00 -1 0 2 10 8 250 | 18 -996.60 904.00 -1 0 10 10 8 250
630 c8db7c98 | 10 625.34 0.00 -1 13 | 157 850.60 0.00 -1 0 2 10 8 240 | 12 -954.60 904.00 1 0 10 10 8 240
640 814f0cdc | 10 618.59 0.00 -1 13 | 157 1273.24 0.00 -1 0 2 10 8 230 | 36 -991.00 904.00 1 0 10 10 8 230
644 814f0cdc | 10 618.59 0.00 -1 13 | 157 1383.46 0.00 -1 0 2 10 8 226 | 35 -991.00 904.00 1 0 10 10 8 226
645 6f5713da | 10 618.59 0.00 -1 13 | 157 1427.31 0.00 -1 0 10 10 8 225 | 35 -991.00 904.00 1 0 10 10 8 225
650 e18a54d5 | 10 618.59 0.00 -1 13 | 157 1692.86 0.00 -1 0 10 10 8 220 | 20 -991.00 904.00 1 0 10 10 8 220
660 0260014a | 10 618.59 0.00 -1 13 | 152 1806.10 0.00 -1 0 10 10 8 210 | 22 -733.40 1552.00 1 0 10 10 8 210
670 5b161170 | 10 618.59 0.00 -1 13 | 153 1806.10 0.00 -1 0 10 10 8 200 | 22 -458.40 1960.00 1 0 10 10 8 200
680 71d56011 | 10 618.59 0.00 -1 13 | 154 1806.10 0.00 -1 0 10 10 8 190 | 22 -183.40 2128.00 1 0 10 10 8 190
690 e6bf5d06 | 10 618.59 0.00 -1 13 | 10 1806.10 0.00 -1 0 10 10 8 180 | 22 91.10 1998.00 1 0 10 10 8 180
700 46c30480 | 10 618.59 0.00 -1 13 | 12 1792.10 0.00 -1 0 10 10 8 170 | 32 292.60 1542.00 1 0 10 10 8 170
710 f27d233c | 10 618.59 0.00 -1 13 | 12 1652.10 0.00 -1 0 10 10 8 160 | 32 292.60 1542.00 1 0 10 10 8 160
720 9002f17c | 10 618.59 0.00 -1 13 | 12 1512.10 0.00 -1 0 10 10 8 150 | 33 292.60 1491.60 1 0 10 10 8 150
730 7122c4c5 | 10 618.59 0.00 -1 13 | 12 1372.10 0.00 -1 0 10 10 8 140 | 33 295.30 1178.00 1 0 10 10 8 140
740 c7a1cb40 | 10 618.59 0.00 -1 13 | 12 1232.10 0.00 -1 0 10 10 8 130 | 212 326.80 418.00 1 0 10 10 8 130
750 4b519aca | 10 618.59 0.00 -1 13 | 208 1192.90 0.00 -1 0 10 10 8 120 | 32 327.20 0.00 1 0 10 10 8 120
751 6eec11d2 | 10 618.59 0.00 -1 13 | 208 1192.90 0.00 -1 0 10 10 8 119 | 32 327.20 0.00 1 0 2 10 8 119
760 ab383c6c | 41 618.59 0.00 1 23 | 208 1192.90 0.00 -1 0 10 10 8 110 | 32 327.20 0.00 1 0 2 10 8 110
770 036a46de | 41 364.44 63.42 1 23 | 208 1192.90 0.00 -1 0 10 10 8 100 | 166 327.20 0.00 1 0 2 10 8 100
780 63cb7b91 | 41 217.31 0.00 1 23 | 208 1192.90 0.00 -1 0 10 10 8 90 | 52 271.28 35.32 1 10 2 10 8 90
788 1718c76f | 10 199.54 0.00 1 23 | 208 1192.90 0.00 -1 0 10 10 8 82 | 52 -125.32 197.23 1 10 2 10 8 82
789 68b69d5d | 10 199.54 0.00 1 23 | 208 1192.90 0.00 -1 0 10 10 8 81 | 52 -168.55 202.39 1 10 10 10 8 81
790 ef744b33 | 10 199.54 0.00 1 23 | 208 1192.90 0.00 -1 0 10 10 8 80 | 52 -210.38 204.20 1 10 10 10 8 80
800 a50f4c6b | 10 199.54 0.00 1 23 | 208 1192.90 0.00 -1 0 10 10 8 70 | 52 -551.10 40.42 1 10 10 10 8 70
810 a706d7fb | 10 199.54 0.00 1 23 | 208 1192.90 0.00 -1 0 10 10 8 60 | 68 -751.22 0.00 1 10 10 10 8 60
820 0ab5a661 | 10 199.54 0.00 1 23 | 12 1080.90 0.00 -1 0 10 10 8 50 | 68 -811.62 0.00 1 10 10 10 8 50
830 1d5a7cd3 | 10 199.54 0.00 1 23 | 12 940.90 0.00 -1 0 10 10 8 40 | 78 -816.55 0.00 1 10 10 10 8 40
840 403e14cd | 10 199.54 0.00 1 23 | 208 845.70 0.00 -1 0 10 10 8 30 | 78 -1055.85 0.00 1 10 10 10 8 30
850 a2c0a90c | 41 199.54 0.00 1 31 | 208 845.70 0.00 -1 0 10 10 8 20 | 78 -1685.59 0.00 1 10 10 10 8 20
860 37752006 | 41 10.91 76.83 1 31 | 208 845.70 0.00 -1 0 10 10 8 10 | 78 -1903.62 0.00 1 10 10 10 8 10
870 7e743128 | 41 -190.45 0.00 1 31 | 208 845.70 0.00 -1 0 10 10 8 0 | 153 -1903.62 0.00 1 10 10 10 8 0
880 38ab668d | 10 -245.71 0.00 1 31 | 208 845.70 0.00 -1 0 10 10 8 0 | 154 -1903.62 0.00 1 10 10 10 8 0
890 3a1484d1 | 10 -245.71 0.00 1 31 | 208 845.70 0.00 -1 0 10 10 8 0 | 12 -1889.62 0.00 1 10 10 10 8 0
900 a2b04f55 | 10 -245.71 0.00 1 31 | 12 817.70 0.00 -1 0 10 10 8 0 | 12 -1749.62 0.00 1 10 10 10 8 0
910 8ccc3db7 | 10 -245.71 0.00 1 31 | 10 750.50 0.00 -1 0 10 10 8 0 | 12 -1609.62 0.00 1 10 10 10 8 0
920 baae9929 | 10 -245.71 0.00 1 31 | 12 666.50 0.00 -1 0 10 10 8 0 | 12 -1469.62 0.00 1 10 10 10 8 0
930 b7271e83 | 10 -245.71 0.00 1 31 | 20 571.30 0.00 -1 0 10 10 8 0 | 12 -1329.62 0.00 1 10 10 10 8 0
940 6be5946d | 10 -245.71 0.00 1 31 | 22 544.30 648.00 -1 0 10 10 8 0 | 195 -1195.22 0.00 1 10 10 10 8 0
950 8a8e7882 | 10 -245.71 0.00 1 31 | 22 431.80 1056.00 -1 0 10 10 8 0 | 195 -1192.42 0.00 1 10 10 10 8 0
960 8a8e7882 | 10 -245.71 0.00 1 31 | 22 369.30 1224.00 -1 0 10 10 8 0 | 195 -1192.42 0.00 1 10 10 10 8 0
970 47ed7a8a | 10 -245.71 0.00 1 31 | 22 355.30 1152.00 -1 0 10 10 8 0 | 10 -1192.42 0.00 1 10 10 10 8 0
980 6694b49c | 10 -245.71 0.00 1 31 | 26 368.30 730.80 -1 0 10 10 8 0 | 204 -1055.92 0.00 1 10 10 10 8 0
990 1e83c349 | 52 -245.71 0.00 -1 48 | 32 389.90 0.00 -1 0 10 10 8 0 | 204 -826.92 0.00 1 10 10 10 8 0
1000 238b44aa | 52 166.95 310.93 -1 48 | 18 389.90 0.00 -1 0 10 10 8 0 | 204 -596.09 0.00 1 10 10 10 8 0
1010 656f8fad | 52 649.92 453.50 -1 48 | 12 417.90 0.00 1 0 10 10 8 0 | 204 -465.30 0.00 1 10 10 10 8 0
1020 656f8fad | 52 1007.56 342.02 -1 48 | 12 557.90 0.00 1 0 10 10 8 0 | 204 -381.61 0.00 1 10 10 10 8 0
1030 41ff7ff1 | 52 1239.87 114.88 -1 48 | 12 697.90 0.00 1 0 10 10 8 0 | 18 -379.02 0.00 -1 10 10 10 8 0
1040 e44f7ee3 | 68 1341.65 0.00 -1 48 | 12 837.90 0.00 1 0 10 10 8 0 | 18 -379.02 0.00 1 10 10 10 8 0
1050 b0072e52 | 68 1344.30 0.00 -1 48 | 12 977.90 0.00 1 0 10 10 8 0 | 12 -281.02 0.00 1 10 10 10 8 0
1060 10bf940c | 70 1344.30 0.00 -1 48 | 152 1112.30 0.00 1 0 10 10 8 0 | 12 -141.02 0.00 1 10 10 10 8 0
1063 a8f148f6 | 70 1351.05 0.00 -1 48 | 166 1108.35 0.00 1 0 10 10 8 0 | 12 -99.02 0.00 1 10 10 10 8 0
1064 41789b2c | 70 1351.05 0.00 -1 48 | 166 1108.35 0.00 1 0 2 10 8 0 | 12 -85.02 0.00 1 10 10 10 8 0
1070 e017dd51 | 70 1351.05 0.00 -1 48 | 166 1108.35 0.00 1 0 2 10 8 0 | 12 -1.02 0.00 1 10 10 10 8 0
1080 e017dd51 | 70 1351.05 0.00 -1 48 | 201 1108.35 0.00 1 0 2 10 8 0 | 12 138.98 0.00 1 10 10 10 8 0
1090 78483160 | 42 1435.49 23.53 -1 53 | 201 1108.35 0.00 1 0 2 10 8 0 | 195 248.18 0.00 1 10 10 10 8 0
1099 5cbfba4b | 42 1736.50 0.00 -1 53 | 201 1108.35 0.00 1 0 2 10 8 0 | 195 248.18 0.00 1 10 10 10 8 0
1100 a6821af3 | 42 1762.14 0.00 -1 53 | 201 1108.35 0.00 1 0 10 10 8 0 | 195 248.18 0.00 1 10 10 10 8 0
1110 2c751d1f | 42 1936.05 0.00 -1 53 | 201 1108.35 0.00 1 0 10 10 8 0 | 195 248.18 0.00 1 10 10 10 8 0
1120 a9a7a8d7 | 45 1968.54 0.00 -1 60 | 201 1108.35 0.00 1 0 10 10 8 0 | 12 304.18 0.00 1 10 10 10 8 0
1130 69cdd663 | 45 2156.25 32.32 -1 60 | 201 1108.35 0.00 1 0 10 10 8 0 | 12 444.18 0.00 1 10 10 10 8 0
1140 61bc4465 | 45 2411.30 -24.00 -1 60 | 201 1108.35 0.00 1 0 10 10 8 0 | 10 539.38 0.00 1 10 10 10 8 0
1150 91ef3ff3 | 45 2498.38 -252.00 -1 60 | 201 1108.35 0.00 1 0 10 10 8 0 | 10 539.38 0.00 1 10 10 10 8 0
1160 34cf9761 | 26 2498.38 -674.40 -1 60 | 204 1369.96 0.00 1 0 10 10 8 0 | 10 539.38 0.00 1 10 10 10 8 0
1170 e4b0894a | 26 2498.38 -1114.40 -1 60 | 204 1724.74 0.00 1 0 10 10 8 0 | 12 609.38 0.00 1 10 10 10 8 0
1180 cf54b477 | 26 2498.38 -1554.40 -1 60 | 204 1845.45 0.00 1 0 10 10 8 0 | 12 749.38 0.00 1 10 10 10 8 0
1190 33737e19 | 26 2498.38 -1994.40 -1 60 | 18 1921.75 0.00 1 0 10 10 8 0 | 12 889.38 0.00 1 10 10 10 8 0
1200 e6ddc44b | 26 2498.38 -2434.40 -1 60 | 18 1921.75 0.00 -1 0 10 10 8 0 | 12 1029.38 0.00 1 10 10 10 8 0
1210 0a87078d | 26 2498.38 -2874.40 -1 60 | 12 1781.75 0.00 -1 0 10 10 8 0 | 12 1169.38 0.00 1 10 10 10 8 0
1220 0924ac5f | 26 2498.38 -3314.40 -1 60 | 10 1756.55 0.00 -1 0 10 10 8 0 | 12 1309.38 0.00 1 10 10 10 8 0
1230 e26d3c3f | 0 2498.38 -3534.40 -1 60 | 10 1756.55 0.00 -1 0 10 10 8 0 | 12 1449.38 0.00 1 10 10 10 8 0
1240 c81925f9 | 0 2498.38 -3534.40 -1 60 | 10 1790.30 0.00 -1 0 10 10 8 0 | 12 1555.63 0.00 1 10 10 10 8 0
1250 2bfe1072 | 0 2498.38 -3534.40 -1 60 | 12 1717.80 0.00 -1 0 10 10 8 0 | 12 1628.13 0.00 1 10 10 10 8 0
1260 fb3eebb9 | 0 2498.38 -3534.40 -1 60 | 12 1604.80 0.00 -1 0 10 10 8 0 | 12 1741.13 0.00 1 10 10 10 8 0
1270 62f0bea5 | 7 0.00 8300.00 -1 0 | 12 1444.55 0.00 -1 0 10 10 8 0 | 12 1901.38 0.00 1 10 10 10 8 0
1280 0121afd1 | 7 0.00 7088.17 -1 0 | 12 1304.55 0.00 -1 0 10 10 8 0 | 12 2041.38 0.00 1 10 10 10 8 0
1290 48f5b5c3 | 7 0.00 6018.91 -1 0 | 12 1164.55 0.00 -1 0 10 10 8 0 | 10 2150.58 0.00 1 10 10 10 8 0
1300 f463c6c5 | 7 0.00 5092.22 -1 0 | 12 1024.55 0.00 -1 0 10 10 8 0 | 10 2150.58 0.00 1 10 10 10 8 0
1310 c2de3757 | 7 0.00 4308.10 -1 0 | 12 884.55 0.00 -1 0 10 10 8 0 | 10 2150.58 0.00 1 10 10 10 8 0
1320 202f22d8 | 7 0.00 3666.54 -1 0 | 12 744.55 0.00 -1 0 10 10 8 0 | 18 2150.58 0.00 1 10 10 10 8 0
1330 059c5342 | 7 0.00 3167.56 -1 0 | 12 604.55 0.00 -1 0 10 10 8 0 | 12 2108.58 0.00 -1 10 10 10 8 0
1340 3acfe47c | 7 0.00 2811.14 -1 0 | 12 464.55 0.00 -1 0 10 10 8 0 | 12 1968.58 0.00 -1 10 10 10 8 0
1350 1df2fbb1 | 8 0.00 2597.28 -1 0 | 18 355.35 0.00 -1 0 10 10 8 0 | 12 1828.58 0.00 -1 10 10 10 8 0
1360 5a4ce8a3 | 8 0.00 2526.00 -1 0 | 12 369.35 0.00 1 0 10 10 8 0 | 12 1688.58 0.00 -1 10 10 10 8 0
1370 a9ca58a5 | 8 0.00 2526.00 -1 0 | 10 408.55 0.00 1 0 10 10 8 0 | 12 1548.58 0.00 -1 10 10 10 8 0
1380 876b6037 | 9 0.00 2526.00 -1 0 | 10 408.55 0.00 1 0 10 10 8 0 | 12 1408.58 0.00 -1 10 10 10 8 0
1390 e42197d9 | 9 0.00 2526.00 -1 0 | 10 408.55 0.00 1 0 10 10 8 0 | 12 1268.58 0.00 -1 10 10 10 8 0
1400 01e522dd | 9 0.00 2526.00 -1 0 | 12 520.55 0.00 1 0 10 10 8 0 | 12 1128.58 0.00 -1 10 10 10 8 0
1410 477a7036 | 9 0.00 2526.00 -1 0 | 18 643.75 0.00 1 0 10 10 8 0 | 12 988.58 0.00 -1 10 10 10 8 0
1420 af598a93 | 9 0.00 2526.00 -1 0 | 18 630.25 0.00 -1 0 10 10 8 0 | 12 862.08 0.00 -1 10 10 10 8 0
1430 0d0ace15 | 9 0.00 2526.00 -1 0 | 12 483.50 0.00 -1 0 10 10 8 0 | 12 728.83 0.00 -1 10 10 10 8 0
1440 42b1f4d9 | 9 0.00 2526.00 -1 0 | 12 343.50 0.00 -1 0 10 10 8 0 | 18 703.63 0.00 1 10 10 10 8 0
1450 0983410b | 9 0.00 2526.00 -1 0 | 12 203.50 0.00 -1 0 10 10 8 0 | 10 703.63 0.00 1 10 10 10 8 0
1460 d2523a4d | 9 0.00 2526.00 -1 0 | 12 63.50 0.00 -1 0 10 10 8 0 | 10 703.63 0.00 1 10 10 10 8 0
1470 54c68b66 | 9 0.00 2526.00 -1 0 | 12 -76.50 0.00 -1 0 10 10 8 0 | 18 703.63 0.00 1 10 10 10 8 0
1480 c94784c0 | 9 0.00 2526.00 -1 0 | 12 -216.50 0.00 -1 0 10 10 8 0 | 12 675.63 0.00 -1 10 10 10 8 0
1490 30964785 | 9 0.00 2526.00 -1 0 | 12 -356.50 0.00 -1 0 10 10 8 0 | 12 535.63 0.00 -1 10 10 10 8 0
1500 518d8e17 | 9 0.00 2526.00 -1 0 | 12 -496.50 0.00 -1 0 10 10 8 0 | 12 395.63 0.00 -1 10 10 10 8 0
";

/// The Kirby Team stage's CPUs from Go as the N64 plays them.
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn kirby_team_stage_cpus_play_as_on_the_n64() {
    let Some((first_bad, log)) =
        one_player_replay(ssb_game::spgame::Stage::Kirby, None, KIRBY_TEAM_STAGE_1P)
    else {
        return;
    };
    if std::env::var_os("SCENE_TRACE").is_some() {
        println!("{log}");
    }
    assert_eq!(first_bad, None, "{log}");
}
