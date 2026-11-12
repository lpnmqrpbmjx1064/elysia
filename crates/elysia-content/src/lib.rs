//! Placeholder for the content layer (A10). Filled in later increments.
//!
//! Declares no dependencies at all. That keeps the boundary fence intact and wider
//! than before: cargo resolves only declared edges, so reaching for net or app fails
//! to resolve — and so does reaching for core or world until an increment needs one.
//! When it does, the inward edge is a one-line manifest addition a reviewer sees,
//! which is the enforcement A10 wanted instead of a note asking people to remember.
