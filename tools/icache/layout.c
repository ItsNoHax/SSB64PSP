// Function order for psp-game/link-order.txt (RE-494): simulated annealing
// over the order of the functions a block trace (ppsspp-icache.patch,
// PPSSPP_ICACHE_TRACE) touches, scored by replaying the trace through the
// PSP-2000's I-cache (16 KB, two-way, 64-byte lines, LRU). The hot functions
// go first, packed back to back on 4-byte boundaries as lld places them;
// functions outside ORDERABLE (those in rlibs, which tools/psp-link.py cannot
// order: compiler_builtins) keep their linked address. HELDOUT, a second
// trace, is only scored, to show the order generalises.
// usage: layout trace.bin nm.txt orderable.txt ITERS SEED out.txt [heldout.bin [initial-order.txt]]
//   nm.txt: psp-nm -S --defined-only elf | awk '$3 ~ /^[tTwW]$/' | sort |
//     tools/psp-link.py --stable   (names as psp-link.py matches them)
//   orderable.txt: tools/icache/relink.py --functions DIR
// LAYOUT_TAIL=file names hot functions whose code differs between the traced
// build and the shipped one (capture-only code, feature-gated paths); they go
// after the searched block in that file's order, so their sizes cannot move it.
//   cc -O2 -o layout tools/icache/layout.c -lm
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <math.h>
#define BASE 0x08804000u
typedef struct { uint32_t a, s; char *n; } F;
// v0 crate hashes (`Cs<base62>_` before the crate name's length) become `C`:
// a no-op on nm.txt from `psp-link.py --stable`, which also re-points the
// back-references.
static char *stable(const char *n) {
  char *r = malloc(strlen(n) + 1), *o = r;
  for (const char *p = n; *p; p++) {
    if (p[0] == 'C' && p[1] == 's') {
      const char *q = p + 2;
      while ((*q >= '0' && *q <= '9') || (*q >= 'a' && *q <= 'z') || (*q >= 'A' && *q <= 'Z')) q++;
      if (*q == '_' && q[1] >= '0' && q[1] <= '9') { *o++ = 'C'; p = q; continue; }
    }
    *o++ = *p;
  }
  *o = 0; return r;
}
static F f[4000]; static int nf;
typedef struct { uint32_t off; uint16_t words, phase; int fn; } B;
static B *load(const char *p, long *nb) {
  FILE *h = fopen(p, "rb"); fseek(h, 0, SEEK_END); long n = ftell(h) / 8; rewind(h);
  uint32_t *t = malloc(n * 8); fread(t, 8, n, h); fclose(h);
  B *b = malloc(n * sizeof(B)); int last = 0;
  for (long i = 0; i < n; i++) {
    uint32_t a = t[2 * i] - BASE; int j = -1;
    if (a >= f[last].a && a < f[last].a + f[last].s) j = last;
    else { int lo = 0, hi = nf - 1; while (lo <= hi) { int m = (lo + hi) / 2; if (f[m].a <= a) { j = m; lo = m + 1; } else hi = m - 1; }
      if (j >= 0 && a >= f[j].a + f[j].s) j = -1;
      if (j >= 0) last = j; }
    b[i].fn = j; b[i].off = j >= 0 ? a - f[j].a : t[2 * i]; b[i].words = t[2 * i + 1] & 0xffff; b[i].phase = t[2 * i + 1] >> 16;
  }
  free(t); *nb = n; return b;
}
static uint32_t start[4000];
static long sim(B *b, long n, long *pm) {
  static uint32_t tags[128][2]; static uint8_t lru[128];
  for (int i = 0; i < 128; i++) { tags[i][0] = tags[i][1] = 0xffffffff; lru[i] = 0; }
  long miss = 0, p = 0;
  for (long i = 0; i < n; i++) {
    uint32_t na = b[i].fn >= 0 ? start[b[i].fn] + b[i].off : b[i].off;
    uint32_t l0 = na >> 6, l1 = (na + b[i].words * 4 - 1) >> 6;
    for (uint32_t l = l0; l <= l1; l++) {
      uint32_t s = l & 127;
      if (tags[s][0] == l) { lru[s] = 1; continue; }
      if (tags[s][1] == l) { lru[s] = 0; continue; }
      miss++; p += b[i].phase; int w = lru[s]; tags[s][w] = l; lru[s] = w ^ 1;
    }
  }
  if (pm) *pm = p;
  return miss;
}
// seq: the orderable functions in link order, the H searched ones first.
static int seq[4000];
static int nseq; static char pinned[4000];
static void place(void) {
  uint32_t a = 0;
  for (int i = 0; i < nseq; i++) { start[seq[i]] = a + BASE; a += (f[seq[i]].s + 3) & ~3u; }
  for (int i = 0; i < nf; i++) if (pinned[i]) start[i] = f[i].a + BASE;
}
int main(int argc, char **argv) {
  FILE *h = fopen(argv[2], "r"); char line[4096];
  while (fgets(line, sizeof line, h)) { unsigned a, s; char t, n[4000];
    if (sscanf(line, "%x %x %c %3999s", &a, &s, &t, n) != 4) continue;
    if (nf && f[nf - 1].a == a) continue; // alias
    f[nf].a = a; f[nf].s = s; f[nf].n = stable(n); nf++; }
  fclose(h);
  // Everything not orderable stays put.
  static char orderable[4000]; h = fopen(argv[3], "r");
  while (fgets(line, sizeof line, h)) { line[strcspn(line, "\n")] = 0;
    for (int i = 0; i < nf; i++) if (!strcmp(f[i].n, line)) orderable[i] = 1; }
  fclose(h);
  for (int i = 0; i < nf; i++) pinned[i] = !orderable[i];
  long iters = atol(argv[4]); srand(atoi(argv[5]));
  long nb, nb2 = 0; B *b = load(argv[1], &nb); B *b2 = argc > 7 ? load(argv[7], &nb2) : 0;
  // Hot: the functions the trace runs.
  static char hot[4000]; for (long i = 0; i < nb; i++) if (b[i].fn >= 0) hot[b[i].fn] = 1;
  for (int i = 0; i < nf; i++) start[i] = f[i].a + BASE;
  nseq = 0; long pm0, m0 = sim(b, nb, &pm0);
  fprintf(stderr, "as linked: misses %ld phase %ld\n", m0, pm0);
  static char tail[4000]; static int tails[4000]; int T = 0;
  if (getenv("LAYOUT_TAIL")) {
    FILE *q = fopen(getenv("LAYOUT_TAIL"), "r");
    while (fgets(line, sizeof line, q)) { line[strcspn(line, "\n")] = 0;
      for (int i = 0; i < nf; i++) if (hot[i] && !pinned[i] && !tail[i] && !strcmp(f[i].n, line)) { tail[i] = 1; tails[T++] = i; break; } }
    fclose(q); }
  int H = 0; for (int i = 0; i < nf; i++) if (hot[i] && !pinned[i] && !tail[i]) seq[H++] = i;
  if (argc > 8) { // initial hot order from a link-order file
    FILE *q = fopen(argv[8], "r"); int k = 0; static char used[4000];
    while (fgets(line, sizeof line, q)) { line[strcspn(line, "\n")] = 0;
      for (int i = 0; i < nf; i++) if (hot[i] && !pinned[i] && !tail[i] && !used[i] && !strcmp(f[i].n, line)) { seq[k++] = i; used[i] = 1; break; } }
    for (int i = 0; i < nf; i++) if (hot[i] && !pinned[i] && !tail[i] && !used[i]) seq[k++] = i;
    fclose(q); }
  // The cold orderable functions follow in their linked order.
  nseq = H; for (int i = 0; i < T; i++) seq[nseq++] = tails[i];
  for (int i = 0; i < nf; i++) if (!hot[i] && !pinned[i]) seq[nseq++] = i;
  place(); long pm, best = sim(b, nb, &pm), cur = best;
  long pm2 = 0, m2 = b2 ? sim(b2, nb2, &pm2) : 0;
  fprintf(stderr, "functions %d searched %d start misses %ld phase %ld | heldout %ld phase %ld\n", nf, H, best, pm, m2, pm2);
  static int bestseq[4000]; memcpy(bestseq, seq, sizeof seq);
  double T0 = 300, T1 = 1;
  for (long it = 0; it < iters; it++) {
    double T = T0 * pow(T1 / T0, (double)it / iters);
    int i = rand() % H, j = rand() % H; if (i == j) continue;
    int kind = rand() & 1; static int save[4000];
    memcpy(save, seq, H * sizeof(int));
    if (kind) { int t = seq[i]; seq[i] = seq[j]; seq[j] = t; }
    else { int t = seq[i]; if (i < j) memmove(seq + i, seq + i + 1, (j - i) * sizeof(int)); else memmove(seq + j + 1, seq + j, (i - j) * sizeof(int)); seq[j] = t; }
    place(); long c = sim(b, nb, 0);
    if (c <= cur || exp((cur - c) / T) * RAND_MAX > rand()) { cur = c; if (c < best) { best = c; memcpy(bestseq, seq, sizeof seq); } }
    else memcpy(seq, save, H * sizeof(int));
    if (it % 500 == 0) fprintf(stderr, "it %ld T %.1f cur %ld best %ld\n", it, T, cur, best);
  }
  memcpy(seq, bestseq, sizeof seq); place();
  long bp, bm = sim(b, nb, &bp); long bp2 = 0, bm2 = b2 ? sim(b2, nb2, &bp2) : 0;
  fprintf(stderr, "final misses %ld phase %ld | heldout %ld phase %ld\n", bm, bp, bm2, bp2);
  FILE *o = fopen(argv[6], "w");
  fprintf(o, "# Function order for tools/psp-link.py (RE-494), from tools/icache/layout.c.\n");
  for (int i = 0; i < H + T; i++) fprintf(o, "%s\n", f[seq[i]].n);
  fclose(o);
  return 0;
}
