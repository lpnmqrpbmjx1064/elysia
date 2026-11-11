//! Elysia simulation core.
//!
//! Contains no clock, no I/O, no concurrency, no floating point (MANIFESTO P3).
//! Depends on nothing — the A10 layer table is enforced by this crate having no
//! dependencies at all, so a wrong-way dependency cannot compile.

/// Deterministic PRNG, implemented in-repo on purpose: a toolchain upgrade must
/// never be able to change the sequence (P3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rng {
    state: u64,
}

impl Rng {
    /// `0` is a fixed point of xorshift, so it is substituted.
    pub fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 { 0x9E37_79B9_7F4A_7C15 } else { seed },
        }
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Integer-only helper: a value in `0..n`. `n == 0` yields `0`.
    pub fn below(&mut self, n: u32) -> u32 {
        if n == 0 {
            0
        } else {
            (self.next_u64() % u64::from(n)) as u32
        }
    }

    fn peek(&self) -> u64 {
        self.state
    }
}

/// Minimal world: enough state for the replay test to be non-vacuous.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct World {
    pub tick: u64,
    pub rng: Rng,
    pub cells: Vec<i32>,
}

impl World {
    pub fn new(seed: u64, size: usize) -> Self {
        let mut rng = Rng::new(seed);
        let mut cells = Vec::with_capacity(size);
        for _ in 0..size {
            cells.push(rng.below(9) as i32);
        }
        Self {
            tick: 0,
            rng,
            cells,
        }
    }

    /// One tick. Integer math only; no wall clock, no iteration over unordered
    /// containers. Every input to the result is either state or the PRNG.
    pub fn tick(&mut self) {
        self.tick = self.tick.wrapping_add(1);
        let n = self.cells.len();
        if n == 0 {
            return;
        }
        for i in 0..n {
            let left = self.cells[(i + n - 1) % n];
            let right = self.cells[(i + 1) % n];
            let here = self.cells[i];
            // Deterministic integer cellular update.
            let next = (here * 3 + left + right) % 97;
            self.cells[i] = if next < 0 { -next } else { next };
        }
        // The PRNG is state, so it must advance exactly once per tick.
        let _ = self.rng.next_u64();
    }

    /// FNV-1a over the ordered serialisation of the whole world.
    pub fn hash(&self) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        let eat = |h: &mut u64, v: u64| {
            for b in v.to_le_bytes() {
                *h ^= u64::from(b);
                *h = h.wrapping_mul(0x0000_0100_0000_01b3);
            }
        };
        eat(&mut h, self.tick);
        for c in &self.cells {
            eat(&mut h, u64::from(*c as u32));
        }
        eat(&mut h, self.rng.peek());
        h
    }
}
