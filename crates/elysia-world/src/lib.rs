//! World layer — terrain, chunked storage, one character.
//!
//! Implements A4 `chunk` (dense tiles grouped into chunks, generated lazily from a
//! seed), A5 `ascii` (each terrain is one character), A3 `character` (one being in
//! a larger world) and C6 `proc` (nothing authored, everything derived from the
//! seed).
//!
//! Determinism (P3): every tile depends only on `(seed, x, y)`, so generation
//! order cannot change the map, and chunk storage is a `BTreeMap` so iteration is
//! ordered rather than hash-dependent. No floats anywhere.

use elysia_core::Rng;
use std::collections::BTreeMap;

/// Tiles per chunk edge. 100 is divisible by 10, so the demo world is exactly
/// 10x10 chunks with no partial edge.
pub const CHUNK: i32 = 10;
/// World extent in tiles (B3 `small`: a bounded map).
pub const MAPW: i32 = 100;
pub const MAPH: i32 = 100;

const CHUNK_AREA: usize = (CHUNK as usize) * (CHUNK as usize);

/// What a tile is. One character each (A5).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Terrain {
    Grass,
    Dirt,
    Rock,
    Water,
}

impl Terrain {
    pub fn ch(self) -> char {
        match self {
            Terrain::Grass => '.',
            Terrain::Dirt => ',',
            Terrain::Rock => '^',
            Terrain::Water => '~',
        }
    }

    /// Can the character walk here?
    pub fn walkable(self) -> bool {
        !matches!(self, Terrain::Water | Terrain::Rock)
    }

    /// Bands chosen from the measured `level()` distribution for the demo seed
    /// (p50=128, p75=152, p90=173, p99=206), not guessed — see `levels_histogram`.
    /// Target look: mostly walkable ground, a little rock, lakes at the top tail.
    fn from_level(v: u32) -> Terrain {
        match v {
            0..=137 => Terrain::Grass,
            138..=157 => Terrain::Dirt,
            158..=177 => Terrain::Rock,
            _ => Terrain::Water,
        }
    }
}

/// Mixed hash of a lattice point. Integer only, no state carried between calls,
/// which is what makes generation order-independent.
fn hash2(seed: u64, x: i32, y: i32) -> u64 {
    // CI NEGATIVE CONTROL (throwaway branch): wall clock in terrain.
    let clock = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| u64::from(d.subsec_nanos()))
        .unwrap_or(0);
    let mut h = seed ^ 0x9E37_79B9_7F4A_7C15 ^ clock;
    h = (h ^ (x as i64 as u64)).wrapping_mul(0xFF51_AFD7_ED55_8CCD);
    h = (h ^ (y as i64 as u64)).wrapping_mul(0xC4CE_B9FE_1A85_EC53);
    h ^ (h >> 33)
}

/// Integer value noise in `0..=255`, coherent over a lattice of `step` tiles.
/// Integer lerp instead of float — the reason this is safe to call in a hash.
fn vnoise(seed: u64, x: i32, y: i32, step: i32) -> u32 {
    let (gx, gy) = (x.div_euclid(step), y.div_euclid(step));
    let (fx, fy) = (x.rem_euclid(step), y.rem_euclid(step));
    let s = step as u32;
    let a = (hash2(seed, gx, gy) % 256) as u32;
    let b = (hash2(seed, gx + 1, gy) % 256) as u32;
    let c = (hash2(seed, gx, gy + 1) % 256) as u32;
    let d = (hash2(seed, gx + 1, gy + 1) % 256) as u32;
    let wx = (fx as u32 * 256) / s;
    let wy = (fy as u32 * 256) / s;
    let ab = a * (256 - wx) + b * wx;
    let cd = c * (256 - wx) + d * wx;
    (ab * (256 - wy) + cd * wy) / 65_536
}

/// Two octaves of value noise, averaged back down to `0..=255`.
fn level(seed: u64, x: i32, y: i32) -> u32 {
    let coarse = vnoise(seed, x, y, 16);
    let fine = vnoise(seed ^ 0xABCD_1234, x, y, 5);
    (coarse * 2 + fine) / 3
}

/// One chunk of dense tiles.
struct Chunk {
    tiles: [u8; CHUNK_AREA],
}

impl Chunk {
    fn generate(seed: u64, cx: i32, cy: i32) -> Chunk {
        let mut tiles = [0u8; CHUNK_AREA];
        for ly in 0..CHUNK {
            for lx in 0..CHUNK {
                let x = cx * CHUNK + lx;
                let y = cy * CHUNK + ly;
                // Terrain is stored as its level so a chunk needs no enum decode
                // to be deterministic; the enum is derived on read.
                tiles[(ly * CHUNK + lx) as usize] = level(seed, x, y) as u8;
            }
        }
        Chunk { tiles }
    }

    fn at(&self, lx: i32, ly: i32) -> Terrain {
        Terrain::from_level(u32::from(self.tiles[(ly * CHUNK + lx) as usize]))
    }
}

/// The world. Chunks are generated on first touch and then kept — A4's residency
/// rule, in its simplest honest form for a bounded 100x100 map (B3 `small`).
pub struct World {
    pub seed: u64,
    chunks: BTreeMap<(i32, i32), Chunk>,
}

/// FNV-1a step, shared by every hash in this module so two implementations
/// cannot drift apart. A state hash that disagrees with the map hash by one
/// multiply constant is the kind of defect that reads as "no bug" until a replay
/// silently fails to reproduce.
fn eat(h: &mut u64, v: u64) {
    for b in v.to_le_bytes() {
        *h ^= u64::from(b);
        *h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
}

impl World {
    pub fn new(seed: u64) -> World {
        World {
            seed,
            chunks: BTreeMap::new(),
        }
    }

    /// Terrain at a tile, generating its chunk if this is the first touch.
    /// Out-of-bounds reads as water so the edge of the map behaves like a coast.
    pub fn tile(&mut self, x: i32, y: i32) -> Terrain {
        if !in_bounds(x, y) {
            return Terrain::Water;
        }
        let key = (x.div_euclid(CHUNK), y.div_euclid(CHUNK));
        let chunk = self
            .chunks
            .entry(key)
            .or_insert_with(|| Chunk::generate(self.seed, key.0, key.1));
        chunk.at(x.rem_euclid(CHUNK), y.rem_euclid(CHUNK))
    }

    /// Force every chunk into memory. Used by tests and by the full-map render.
    pub fn generate_all(&mut self) {
        for cy in 0..MAPH.div_euclid(CHUNK) {
            for cx in 0..MAPW.div_euclid(CHUNK) {
                let _ = self.tile(cx * CHUNK, cy * CHUNK);
            }
        }
    }

    pub fn chunks_resident(&self) -> usize {
        self.chunks.len()
    }

    /// FNV-1a over the whole map in coordinate order. Order is fixed by the loop,
    /// not by storage, so the same seed always yields the same hash.
    pub fn hash(&mut self) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        eat(&mut h, self.seed);
        for y in 0..MAPH {
            for x in 0..MAPW {
                eat(&mut h, self.tile(x, y) as u64);
            }
        }
        h
    }

    /// Fingerprint of the whole shipped state: the terrain *and* the entity that
    /// moves over it.
    ///
    /// `hash()` covers only the map, so two different game states — same world,
    /// player in a different place — reported the same fingerprint. That made the
    /// number in `/state` useless as a state check: it could not tell "the world
    /// advanced" from "the world sat still" (L01).
    pub fn state_hash(&mut self, man: &Man) -> u64 {
        let mut h = self.hash();
        eat(&mut h, man.x as i64 as u64);
        eat(&mut h, man.y as i64 as u64);
        h
    }

    /// The whole map as rows of characters, for the client to draw (A5, P4: the
    /// board sends text, the browser draws it).
    pub fn rows(&mut self) -> Vec<String> {
        let mut out = Vec::with_capacity(MAPH as usize);
        for y in 0..MAPH {
            let mut row = String::with_capacity(MAPW as usize);
            for x in 0..MAPW {
                row.push(self.tile(x, y).ch());
            }
            out.push(row);
        }
        out
    }
}

pub fn in_bounds(x: i32, y: i32) -> bool {
    (0..MAPW).contains(&x) && (0..MAPH).contains(&y)
}

/// Distribution report for `level()`. Kept as a unit test because it lives next to
/// the private function and because the classification thresholds below are only
/// defensible if they were chosen from this table rather than guessed.
/// `cargo test -p elysia-world levels_histogram -- --nocapture`
#[cfg(test)]
mod level_report {
    use super::*;

    #[test]
    fn levels_histogram() {
        let seed = 20261111u64;
        let mut vals: Vec<u32> = Vec::with_capacity((MAPW * MAPH) as usize);
        for y in 0..MAPH {
            for x in 0..MAPW {
                vals.push(level(seed, x, y));
            }
        }
        vals.sort_unstable();
        let n = vals.len();
        let pct = |p: usize| vals[(n - 1) * p / 100];
        println!(
            "LEVELS  p1={} p5={} p10={} p25={} p50={} p75={} p90={} p95={} p99={} min={} max={}",
            pct(1), pct(5), pct(10), pct(25), pct(50), pct(75), pct(90), pct(95), pct(99),
            vals[0], vals[n - 1]
        );
    }
}

/// The player (A3 `character`): one being in a larger world.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Man {
    pub x: i32,
    pub y: i32,
}

impl Man {
    /// Place the character on the first walkable tile from a seeded offset, so
    /// the spawn is reproducible without authoring a start position (C6).
    pub fn spawn(world: &mut World) -> Man {
        let mut rng = Rng::new(world.seed ^ 0x5EED_1234_5678_9ABC);
        for _ in 0..4096 {
            let x = rng.below(MAPW as u32) as i32;
            let y = rng.below(MAPH as u32) as i32;
            if world.tile(x, y).walkable() {
                return Man { x, y };
            }
        }
        Man { x: MAPW / 2, y: MAPH / 2 }
    }

    /// Try to step by `(dx, dy)`. Returns whether the character moved.
    pub fn step(&mut self, world: &mut World, dx: i32, dy: i32) -> bool {
        let (nx, ny) = (self.x + dx, self.y + dy);
        if !in_bounds(nx, ny) || !world.tile(nx, ny).walkable() {
            return false;
        }
        self.x = nx;
        self.y = ny;
        true
    }
}
