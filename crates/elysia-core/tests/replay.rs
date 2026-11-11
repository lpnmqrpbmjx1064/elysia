//! Determinism harness (D3 `builddet`).
//!
//! If any test here fails, the merge gate must fail: P3 treats determinism as a
//! property of the engine, not a goal to approach.

use elysia_core::World;

#[test]
fn same_seed_same_hash() {
    let mut a = World::new(0xDEAD_BEEF, 256);
    let mut b = World::new(0xDEAD_BEEF, 256);
    for _ in 0..1000 {
        a.tick();
        b.tick();
    }
    assert_eq!(a.hash(), b.hash(), "identical seed and ticks diverged");
}

#[test]
fn different_seed_different_hash() {
    let mut a = World::new(1, 256);
    let mut b = World::new(2, 256);
    for _ in 0..100 {
        a.tick();
        b.tick();
    }
    assert_ne!(a.hash(), b.hash(), "different seeds produced the same world");
}

#[test]
fn replay_is_stable_across_20000_ticks() {
    let mut a = World::new(42, 128);
    let mut b = World::new(42, 128);
    for _ in 0..20_000 {
        a.tick();
        b.tick();
        assert_eq!(a.hash(), b.hash(), "diverged at tick {}", a.tick);
    }
}

/// Anti-vacuity guard: a hash that never changes would satisfy the three tests
/// above for free. This one proves the hash actually covers the state.
#[test]
fn hash_actually_covers_state() {
    let mut w = World::new(7, 64);
    let before = w.hash();
    w.tick();
    assert_ne!(before, w.hash(), "hash ignores the state it claims to cover");
}

/// The seed is an input, so it must reach the state. Guards against a PRNG that
/// is constructed but never mixed in.
#[test]
fn seed_reaches_initial_state() {
    let a = World::new(1, 64);
    let b = World::new(2, 64);
    assert_ne!(
        a.cells, b.cells,
        "two different seeds produced identical initial terrain"
    );
}
