//! The game's globals are one per thread in a host build, also where a
//! crate links `ssb-game` without `cfg(test)` (as every other crate's tests
//! do): each test thread is its own game, so a parallel test's draws never
//! move this one's (the RE-508 Whispy wind replay once stopped at frame
//! 1260 instead of 1269 beside the Zebes acid test).

#[test]
fn another_thread_starts_its_own_game() {
    ssb_game::rng::set_seed(0x1234_5678);
    let mine = ssb_game::rng::rand_ushort();
    ssb_game::stale::set_damage_ratio(200);
    let theirs = std::thread::spawn(|| {
        let fresh = (ssb_game::rng::seed(), ssb_game::stale::damage_ratio());
        ssb_game::rng::set_seed(0x1234_5678);
        let draw = ssb_game::rng::rand_ushort();
        ssb_game::rng::set_seed(7);
        ssb_game::stale::set_damage_ratio(50);
        (fresh, draw)
    })
    .join()
    .unwrap();
    // `sSYUtilsRandomSeed` starts at 1; the battle state's ratio at 100.
    assert_eq!(theirs, ((1, ssb_game::stale::DAMAGE_RATIO_DEFAULT), mine));
    // The other thread's writes stay there.
    assert_eq!(ssb_game::stale::damage_ratio(), 200);
    ssb_game::rng::set_seed(0x1234_5678);
    assert_eq!(ssb_game::rng::rand_ushort(), mine);
}
