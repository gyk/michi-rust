//! Pattern matching for Go move generation.
//!
//! This module will implement pattern-based move heuristics from the C code:
//!
//! ## 3x3 Patterns (`pat3`)
//! Fast pattern matching using the 8 neighbors encoded into a lookup table.
//! Used for both playout move generation and MCTS priors.
//!
//! ## Large Patterns
//! Larger patterns (up to ~17 points) loaded from pattern files.
//! These provide probability estimates for how likely a move is to be good.
//!
//! ## Pattern Files
//! - `patterns.prob` - Pattern probabilities (from professional game analysis)
//! - `patterns.spat` - Spatial pattern definitions
//!
//! See `TODOs.md` for implementation status.

use crate::position::{Point, Position};

/// Check if a point matches any 3x3 pattern.
///
/// TODO: Implement pattern tables from `patterns.c`:
/// - `make_pat3set()` - Build the pattern lookup table
/// - Use `env4`/`env4d` encoding for fast lookup
///
/// Currently returns `false` (no pattern match).
#[inline]
pub fn pat3_match(_pos: &Position, _pt: Point) -> bool {
    // TODO: Implement pattern matching
    false
}

/// Initialize pattern tables.
///
/// TODO: Load and initialize:
/// - 3x3 pattern bitfield (`pat3set`)
/// - Large pattern probability tables
pub fn init_patterns() {
    // TODO: Implement pattern loading
}
