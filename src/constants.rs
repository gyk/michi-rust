//! Constants for board dimensions, MCTS parameters, and priors.
//!
//! This module contains all the configuration constants for the Go engine.
//! The board uses a 1D array representation with padding for boundary detection.
//!
//! # Board Size Configuration
//!
//! The board size is controlled by Cargo features:
//! - `board9x9` (default): 9x9 board
//! - `board13x13`: 13x13 board
//!
//! To compile for a specific board size:
//! ```sh
//! cargo build                           # 9x9 (default)
//! cargo build --no-default-features --features board13x13  # 13x13
//! ```

// =============================================================================
// Board Geometry
// =============================================================================

/// Board size (NxN). Standard Go sizes are 9, 13, or 19.
#[cfg(feature = "board9x9")]
pub const N: usize = 9;

#[cfg(feature = "board13x13")]
pub const N: usize = 13;

// Compile-time check: exactly one board size feature must be enabled
#[cfg(all(feature = "board9x9", feature = "board13x13"))]
compile_error!("Cannot enable both 'board9x9' and 'board13x13' features at the same time");

#[cfg(not(any(feature = "board9x9", feature = "board13x13")))]
compile_error!("Must enable exactly one board size feature: 'board9x9' or 'board13x13'");

/// Board width including left padding (N + 2 for padding on both sides).
pub const W: usize = N + 2;

/// Total board array size including all padding. Matches C layout for compatibility.
pub const BOARDSIZE: usize = (N + 1) * W + 1;

/// First valid board index (skips top and left padding).
pub const BOARD_IMIN: usize = N + 1;

/// Last valid board index (before bottom padding).
pub const BOARD_IMAX: usize = BOARDSIZE - N - 1;

/// Maximum game length (3 times board area to allow for captures and replays).
pub const MAX_GAME_LEN: usize = N * N * 3;

// =============================================================================
// Special Move Values
// =============================================================================

/// Pass move marker (index 0 is padding, so safe to use).
pub const PASS_MOVE: usize = 0;

/// Resign move marker.
pub const RESIGN_MOVE: usize = usize::MAX;

// =============================================================================
// MCTS (Monte Carlo Tree Search) Parameters
// =============================================================================

/// Default number of simulations per move.
pub const N_SIMS: usize = 1400;

/// RAVE equivalence parameter - controls RAVE vs UCB balance.
pub const RAVE_EQUIV: usize = 3500;

/// Minimum visits before expanding a node.
pub const EXPAND_VISITS: u32 = 8;

/// Progress report period (number of simulations between reports).
pub const REPORT_PERIOD: usize = 200;

/// Winrate threshold below which the engine resigns.
pub const RESIGN_THRES: f64 = 0.2;

/// Fast-play threshold at 20% of simulations.
pub const FASTPLAY20_THRES: f64 = 0.8;

/// Fast-play threshold at 5% of simulations.
pub const FASTPLAY5_THRES: f64 = 0.95;

// =============================================================================
// Prior Values (for MCTS node initialization)
// =============================================================================

/// Base prior for all moves (ensures exploration).
pub const PRIOR_EVEN: u32 = 10;

/// Negative prior for self-atari moves.
pub const PRIOR_SELFATARI: u32 = 10;

/// Prior bonus for capturing a single stone.
pub const PRIOR_CAPTURE_ONE: u32 = 15;

/// Prior bonus for capturing multiple stones.
pub const PRIOR_CAPTURE_MANY: u32 = 30;

/// Prior bonus for moves matching 3x3 patterns.
pub const PRIOR_PAT3: u32 = 10;

/// Prior bonus for moves matching large patterns.
pub const PRIOR_LARGEPATTERN: u32 = 100;

/// Prior bonus by distance from last move (CFG distance 1, 2, 3).
pub const PRIOR_CFG: [u32; 3] = [24, 22, 8];

/// Negative prior for moves in empty areas.
pub const PRIOR_EMPTYAREA: u32 = 10;

// =============================================================================
// Playout Heuristic Probabilities
// =============================================================================

/// Probability of using capture heuristic in playouts.
pub const PROB_HEURISTIC_CAPTURE: f64 = 0.9;

/// Probability of using 3x3 pattern heuristic in playouts.
pub const PROB_HEURISTIC_PAT3: f64 = 0.95;

/// Probability of rejecting self-atari in playouts.
pub const PROB_SSAREJECT: f64 = 0.9;

/// Probability of rejecting random self-atari.
pub const PROB_RSAREJECT: f64 = 0.5;

// =============================================================================
// Neighbor Offsets
// =============================================================================

/// Offsets to neighboring points in the 1D board array.
/// Order: North, East, South, West, NE, SE, SW, NW
pub const DELTA: [isize; 8] = [
    -(N as isize) - 1, // North (up one row)
    1,                 // East (right one column)
    (N as isize) + 1,  // South (down one row)
    -1,                // West (left one column)
    -(N as isize),     // NE (diagonal)
    W as isize,        // SE (diagonal)
    N as isize,        // SW (diagonal)
    -(W as isize),     // NW (diagonal)
];

// =============================================================================
// Stone Color Constants (as bytes for direct comparison)
// =============================================================================

/// Black stone (current player to move).
pub const STONE_BLACK: u8 = b'X';

/// White stone (opponent).
pub const STONE_WHITE: u8 = b'x';

/// Empty point.
pub const EMPTY: u8 = b'.';

/// Out of bounds (padding).
pub const OUT: u8 = b' ';
