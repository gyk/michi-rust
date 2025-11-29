//! Monte Carlo playouts (random game simulation).
//!
//! This module implements random playouts for evaluating positions.
//! A playout plays random legal moves until the game ends, then scores the result.
//!
//! TODO: Add heuristics from the C implementation:
//! - Capture moves prioritization
//! - 3x3 pattern matching
//! - Self-atari rejection

use crate::constants::{BOARD_IMAX, BOARD_IMIN, EMPTY, MAX_GAME_LEN};
use crate::position::{Position, is_eye, is_eyeish, pass_move, play_move};

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
        if let Some(pt) = find_random_move(pos) {
            play_move(pos, pt);
            passes = 0;
        } else {
            pass_move(pos);
            passes += 1;
        }
    }

    // Compute score and adjust for perspective
    let s = score(pos);
    if start_n % 2 != pos.n % 2 { -s } else { s }
}

/// Find a random legal move that is not a true eye.
///
/// This is a simple random policy. The C implementation uses more sophisticated
/// heuristics (captures, patterns, locality).
fn find_random_move(pos: &mut Position) -> Option<usize> {
    // TODO: Use heuristics like the C code (capture, pat3, locality)
    for pt in BOARD_IMIN..BOARD_IMAX {
        if pos.color[pt] != EMPTY {
            continue;
        }
        // Skip true eyes for current player
        if is_eye(pos, pt) == b'X' {
            continue;
        }
        if play_move(pos, pt).is_empty() {
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
