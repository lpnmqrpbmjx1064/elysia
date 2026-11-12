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
}

