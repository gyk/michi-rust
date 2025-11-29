//! Monte Carlo playouts (random game simulation).
//!
//! This module implements random playouts for evaluating positions.
//! A playout plays random legal moves until the game ends, then scores the result.
//!
//! TODO: Add heuristics from the C implementation:
//! - Capture moves prioritization
//! - 3x3 pattern matching
//! - Self-atari rejection

use crate::constants::{BOARD_IMAX, BOARD_IMIN, EMPTY, MAX_GAME_LEN, N, W};
use crate::position::{is_eye, is_eyeish, pass_move, play_move, Position};

/// Simple fast random number generator (32-bit Linear Congruential Generator).
/// Same algorithm as michi-c for reproducibility.
static mut RNG_STATE: u32 = 1;

/// Seed the random number generator.
#[allow(dead_code)]
pub fn seed_rng(seed: u32) {
    unsafe {
        RNG_STATE = if seed == 0 { 1 } else { seed };
    }
}

/// Generate a random u32.
#[inline]
fn qdrandom() -> u32 {
    unsafe {
        RNG_STATE = RNG_STATE.wrapping_mul(1664525).wrapping_add(1013904223);
        RNG_STATE
    }
}

/// Generate a random integer in [0, n).
#[inline]
fn random_int(n: u32) -> u32 {
    let r = qdrandom() as u64;
    ((r * n as u64) >> 32) as u32
}

/// Perform a Monte Carlo playout from the given position.
///
/// Plays random legal moves until two consecutive passes or the game length limit.
/// Returns a score from the perspective of the player to move at the start:
/// - Positive score = starting player wins
/// - Negative score = starting player loses
pub fn mcplayout(pos: &mut Position) -> f64 {
    let start_n = pos.n;
    let mut passes = 0;

    while passes < 2 && pos.n < MAX_GAME_LEN {
        if let Some(pt) = choose_random_move(pos) {
            play_move(pos, pt);
            passes = 0;
        } else {
            pass_move(pos);
            passes += 1;
        }
    }

    // Compute score and adjust for perspective
    let s = score(pos);
    if start_n % 2 != pos.n % 2 {
        -s
    } else {
        s
    }
}

/// Choose a random legal move that is not a true eye.
///
/// Uses random starting index for fairness, similar to the C implementation.
fn choose_random_move(pos: &Position) -> Option<usize> {
    // Collect candidate moves (empty points that aren't true eyes)
    let mut candidates = Vec::with_capacity(N * N);

    // Start from a random index for better randomization
    let start = BOARD_IMIN + random_int((N * W) as u32) as usize;

    // Scan from start to end
    for pt in start..BOARD_IMAX {
        if pos.color[pt] == EMPTY && is_eye(pos, pt) != b'X' {
            candidates.push(pt);
        }
    }
    // Wrap around from beginning to start
    for pt in BOARD_IMIN..start {
        if pos.color[pt] == EMPTY && is_eye(pos, pt) != b'X' {
            candidates.push(pt);
        }
    }

    if candidates.is_empty() {
        return None;
    }

    // Shuffle and try moves until we find a legal one
    // (some candidates might be suicide moves)
    let n = candidates.len();
    for i in 0..n {
        // Pick a random remaining candidate
        let j = i + random_int((n - i) as u32) as usize;
        candidates.swap(i, j);

        let pt = candidates[i];
        // Test if move is legal by cloning position
        let mut test_pos = pos.clone();
        if play_move(&mut test_pos, pt).is_empty() {
            return Some(pt);
        }
    }

    None
}

/// Compute the score for the current player.
///
/// Uses area scoring (Chinese rules):
/// - Stones on the board count as territory
/// - Eyeish empty points belong to the surrounding color
/// - Komi is applied (negative for Black, positive for White)
///
/// Returns a positive score if the current player ('X') is winning.
fn score(pos: &Position) -> f64 {
    // Start with komi adjustment
    let mut s = if pos.n % 2 == 0 {
        -pos.komi as f64 // Black to play, komi counts against Black
    } else {
        pos.komi as f64 // White to play, komi counts for White
    };

    for pt in BOARD_IMIN..BOARD_IMAX {
        let c = pos.color[pt];
        // For empty points, check if they're controlled by one side
        let effective = if c == EMPTY { is_eyeish(pos, pt) } else { c };

        match effective {
            b'X' => s += 1.0,
            b'x' => s -= 1.0,
            _ => {} // Empty or neutral
        }
    }

    s
}
