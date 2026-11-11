//! World tests: determinism (P3), chunk residency (A4), movement (A3).

use elysia_world::{Man, Terrain, World, MAPH, MAPW};

#[test]
fn same_seed_same_map() {
    let mut a = World::new(42);
    let mut b = World::new(42);
    assert_eq!(a.hash(), b.hash(), "same seed produced different maps");
}

#[test]
fn different_seed_different_map() {
    let mut a = World::new(1);
    let mut b = World::new(2);
    assert_ne!(a.hash(), b.hash(), "different seeds produced the same map");
}

/// The order-independence guard: if generation depended on call order, touching
/// the map corner-first and centre-first would diverge. This is the test that
/// catches a HashMap or an order-dependent RNG creeping in.
#[test]
fn generation_order_does_not_change_the_map() {
    let mut forward = World::new(7);
    for y in 0..MAPH {
        for x in 0..MAPW {
            let _ = forward.tile(x, y);
        }
    }
    let mut backward = World::new(7);
    for y in (0..MAPH).rev() {
        for x in (0..MAPW).rev() {
            let _ = backward.tile(x, y);
        }
    }
    let mut sparse = World::new(7);
    let _ = sparse.tile(99, 99);
    let _ = sparse.tile(0, 0);
    let _ = sparse.tile(50, 50);

    assert_eq!(forward.hash(), backward.hash(), "order changed the map");
    assert_eq!(forward.hash(), sparse.hash(), "partial touch changed the map");
}

/// A4: chunks are generated on first touch and then kept, not rebuilt per read.
#[test]
fn chunks_are_resident_after_touch() {
    let mut w = World::new(3);
    assert_eq!(w.chunks_resident(), 0, "world started with chunks allocated");
    let _ = w.tile(0, 0);
    assert_eq!(w.chunks_resident(), 1, "first touch did not allocate a chunk");
    let _ = w.tile(9, 9);
    assert_eq!(w.chunks_resident(), 1, "same chunk was allocated twice");
    let _ = w.tile(10, 0);
    assert_eq!(w.chunks_resident(), 2, "neighbouring chunk was not allocated");
}

#[test]
fn map_is_100_by_100() {
    assert_eq!((MAPW, MAPH), (100, 100));
    let mut w = World::new(5);
    let rows = w.rows();
    assert_eq!(rows.len(), MAPH as usize);
    assert!(rows.iter().all(|r| r.chars().count() == MAPW as usize));
}

#[test]
fn spawn_is_reproducible_and_walkable() {
    let mut a = World::new(11);
    let mut b = World::new(11);
    let ma = Man::spawn(&mut a);
    let mb = Man::spawn(&mut b);
    assert_eq!(ma, mb, "spawn was not reproducible for one seed");
    assert!(a.tile(ma.x, ma.y).walkable(), "spawned on unwalkable terrain");
}

#[test]
fn edges_are_water_and_block_movement() {
    let mut w = World::new(9);
    let mut m = Man { x: 0, y: 50 };
    assert_eq!(w.tile(-1, 50), Terrain::Water, "off-map tile was not water");
    assert!(!m.step(&mut w, -1, 0), "character walked off the map");
    assert_eq!((m.x, m.y), (0, 50), "character moved when it should not have");
}

#[test]
fn cannot_walk_into_blocked_terrain() {
    let mut w = World::new(9);
    // Find any blocked tile and one walkable neighbour to step from.
    let mut found = None;
    'outer: for y in 1..MAPH - 1 {
        for x in 1..MAPW - 1 {
            if !w.tile(x, y).walkable() && w.tile(x - 1, y).walkable() {
                found = Some((x, y));
                break 'outer;
            }
        }
    }
    let (bx, by) = found.expect("no blocked tile with a walkable neighbour");
    let mut m = Man { x: bx - 1, y: by };
    assert!(!m.step(&mut w, 1, 0), "character walked into blocked terrain");
    assert_eq!((m.x, m.y), (bx - 1, by));
}

#[test]
fn walks_on_open_ground() {
    let mut w = World::new(9);
    let mut found = None;
    'outer: for y in 1..MAPH - 1 {
        for x in 1..MAPW - 2 {
            if w.tile(x, y).walkable() && w.tile(x + 1, y).walkable() {
                found = Some((x, y));
                break 'outer;
            }
        }
    }
    let (x, y) = found.expect("no two adjacent walkable tiles");
    let mut m = Man { x, y };
    assert!(m.step(&mut w, 1, 0), "character failed to walk on open ground");
    assert_eq!((m.x, m.y), (x + 1, y));
}

/// Not an assertion about taste — prints the mix so thresholds are tuned from
/// data. Run with `-- --nocapture`.
#[test]
fn terrain_mix_report() {
    let mut w = World::new(20261111);
    let mut c = [0usize; 4];
    for y in 0..MAPH {
        for x in 0..MAPW {
            c[match w.tile(x, y) {
                Terrain::Grass => 0,
                Terrain::Dirt => 1,
                Terrain::Rock => 2,
                Terrain::Water => 3,
            }] += 1;
        }
    }
    let total = (MAPW * MAPH) as f64;
    println!(
        "TERRAIN MIX  grass={:.1}%  dirt={:.1}%  rock={:.1}%  water={:.1}%",
        c[0] as f64 * 100.0 / total,
        c[1] as f64 * 100.0 / total,
        c[2] as f64 * 100.0 / total,
        c[3] as f64 * 100.0 / total
    );
    println!("FIRST ROW   {}", &w.rows()[0][..40]);
    println!("WINDOW 40..60 x 10..70 — is it coherent ground or static?");
    let rows = w.rows();
    for r in rows.iter().skip(40).take(20) {
        println!("  |{}|", &r[10..70]);
    }
    assert_eq!(c.iter().sum::<usize>(), (MAPW * MAPH) as usize);
}

/// L01: the fingerprint the app reports must cover the entity, not just the
/// terrain. Before this, two different game states — same world, player in a
/// different place — reported the same hash, so the number in `/state` could not
/// tell "the player moved" from "nothing happened".
#[test]
fn state_hash_covers_the_entity() {
    let mut a = World::new(9);
    let mut ma = Man::spawn(&mut a);
    let mut b = World::new(9);
    let _ = Man::spawn(&mut b);

    assert_eq!(
        a.state_hash(&ma),
        b.state_hash(&ma),
        "two runs of the same seed at the same position must agree"
    );

    let before = a.state_hash(&ma);
    ma.x += 1;
    assert_ne!(
        before,
        a.state_hash(&ma),
        "state hash ignores the entity position — it is a map hash wearing a state hash's name"
    );
}

/// The map hash on its own must keep its old meaning: terrain only, so a pure
/// terrain comparison is still available beside the state comparison.
#[test]
fn map_hash_ignores_the_entity() {
    let mut a = World::new(9);
    let mut ma = Man::spawn(&mut a);
    let before = a.hash();
    ma.x += 1;
    assert_eq!(before, a.hash(), "hash() leaked entity state into the map hash");
    assert_ne!(
        before,
        a.state_hash(&ma),
        "state_hash must differ from hash() once an entity is present"
    );
}
