//! Determinism harness (D3 `builddet`), against the world that actually ships.
//!
//! This harness used to live in `elysia-core` and exercise `elysia_core::World` —
//! a scaffold type with its own tick loop that no binary ever constructed. The
//! gate went green over that toy while the real `elysia_world::World` had no
//! replay test at all, so a change that broke the shipped state would not have
//! been caught by the thing claiming to enforce determinism (L22).
//!
//! "Replay" here means: same seed + same input sequence ⇒ same state, step by
//! step. The engine's step today is a player move — E4 `nextver` defers the
//! autonomous tick loop, so an elapsed-time replay is a declared gap, not a
//! passing test. When the tick loop lands, that replay belongs in this file.
//!
//! If any test here fails, the merge gate must fail: P3 treats determinism as a
//! property of the engine, not a goal to approach.

use elysia_world::{Man, World};

const SEED: u64 = 0x00E1_1B5A_5EED;

/// A fixed walk, not a random one: a failure has to be reproducible from this
/// source alone, which a PRNG-driven input sequence would not give.
const HEADINGS: [(i32, i32); 8] = [
    (0, -1),
    (1, 0),
    (0, 1),
    (-1, 0),
    (1, -1),
    (-1, 1),
    (1, 1),
    (-1, -1),
];

fn walk(steps: usize) -> impl Iterator<Item = (i32, i32)> {
    (0..steps).map(|i| HEADINGS[i % HEADINGS.len()])
}

#[test]
fn same_seed_same_inputs_same_state() {
    let mut a = World::new(SEED);
    let mut ma = Man::spawn(&mut a);
    let mut b = World::new(SEED);
    let mut mb = Man::spawn(&mut b);

    assert_eq!(
        a.state_hash(&ma),
        b.state_hash(&mb),
        "same seed produced different initial state"
    );

    for (step, (dx, dy)) in walk(2_000).enumerate() {
        ma.step(&mut a, dx, dy);
        mb.step(&mut b, dx, dy);
        assert_eq!(
            a.state_hash(&ma),
            b.state_hash(&mb),
            "replay diverged at step {step} after heading ({dx},{dy})"
        );
    }
}

/// 20,000 steps — the shipped equivalent of the old scaffold's 20,000-tick test.
/// Hash comparison is sampled: `state_hash` walks the whole map, so comparing all
/// 20,000 steps would cost 400M mixing rounds for no extra information. The final
/// state is always compared.
#[test]
fn replay_is_stable_over_a_long_walk() {
    let mut a = World::new(SEED);
    let mut ma = Man::spawn(&mut a);
    let mut b = World::new(SEED);
    let mut mb = Man::spawn(&mut b);

    for (step, (dx, dy)) in walk(20_000).enumerate() {
        ma.step(&mut a, dx, dy);
        mb.step(&mut b, dx, dy);
        if step % 500 == 0 {
            assert_eq!(
                a.state_hash(&ma),
                b.state_hash(&mb),
                "long replay diverged at step {step}"
            );
        }
    }
    assert_eq!(
        a.state_hash(&ma),
        b.state_hash(&mb),
        "long replay ended in different final states"
    );
}

/// Anti-vacuity guard. Every equality test above passes for free if the walk never
/// changes anything, so this proves the input sequence actually moves the state.
#[test]
fn replay_actually_exercises_state() {
    let mut w = World::new(SEED);
    let mut m = Man::spawn(&mut w);
    let mut seen = std::collections::BTreeSet::new();
    seen.insert(w.state_hash(&m));

    for (dx, dy) in walk(512) {
        m.step(&mut w, dx, dy);
        seen.insert(w.state_hash(&m));
    }

    assert!(
        seen.len() > 1,
        "512 steps produced a single state — the replay is vacuous"
    );
}

/// The seed is an input, so it must reach the state.
#[test]
fn different_seed_different_state() {
    let mut a = World::new(1);
    let ma = Man::spawn(&mut a);
    let mut b = World::new(2);
    let mb = Man::spawn(&mut b);
    assert_ne!(
        a.state_hash(&ma),
        b.state_hash(&mb),
        "two different seeds produced the same state"
    );
}

/// Hashing must be a read, not a step: if `state_hash` mutated state, every
/// comparison above would be measuring the test rather than the world.
#[test]
fn hashing_is_idempotent() {
    let mut w = World::new(SEED);
    let m = Man::spawn(&mut w);
    let first = w.state_hash(&m);
    assert_eq!(first, w.state_hash(&m), "state_hash is not idempotent");
    let map_first = w.hash();
    assert_eq!(map_first, w.hash(), "hash() is not idempotent");
}

/// The map hash and the state hash must stay distinct: one is terrain, the other
/// is terrain plus the entity. Collapsing them would make `state_hash` silently
/// equivalent to `hash`.
#[test]
fn map_hash_and_state_hash_stay_distinct() {
    let mut w = World::new(SEED);
    let mut m = Man::spawn(&mut w);
    let map = w.hash();
    let mut moved = false;
    for (dx, dy) in walk(64) {
        let before = (m.x, m.y);
        m.step(&mut w, dx, dy);
        if (m.x, m.y) != before {
            moved = true;
            break;
        }
    }
    assert!(moved, "the man never moved, so the distinction cannot be observed");
    assert_eq!(map, w.hash(), "hash() changed when only the entity moved");
    assert_ne!(
        map,
        w.state_hash(&m),
        "state_hash equals hash() with a displaced entity"
    );
}
