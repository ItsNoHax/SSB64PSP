/*
 * n64_srand.so: LD_PRELOAD shim that pins the Mupen64Plus core's random seed.
 *
 * The core calls srand(time(NULL)) once at ROM start and, with its default
 * RandomizeInterrupt = True, draws PI/SI interrupt delays from rand(). Under
 * an LLE RSP that moves the game's audio-frame schedule run to run (the
 * 552/368-sample frame pattern and the frame of each sound call), so an LLE
 * capture is reproducible only with the seed it ran with. HLE runs happen to
 * be insensitive to it.
 *
 * N64_SRAND=<u32> (decimal or 0x hex) replaces every srand() argument; unset,
 * the seed passes through. Each call is logged as "[srand] seed X -> Y" on
 * stderr and counted in n64_srand_calls, so a driver can check the shim is
 * active. n64_driver.py pin_seed() sets this up (--seed).
 */
#define _GNU_SOURCE
#include <dlfcn.h>
#include <stdio.h>
#include <stdlib.h>

volatile unsigned int n64_srand_calls = 0;
volatile unsigned int n64_srand_last = 0;

void srand(unsigned int seed)
{
    static void (*real_srand)(unsigned int);
    if (!real_srand)
        real_srand = (void (*)(unsigned int)) dlsym(RTLD_NEXT, "srand");
    const char *pinned = getenv("N64_SRAND");
    unsigned int use = pinned ? (unsigned int) strtoul(pinned, NULL, 0) : seed;
    fprintf(stderr, "[srand] seed %u -> %u\n", seed, use);
    n64_srand_calls++;
    n64_srand_last = use;
    real_srand(use);
}
