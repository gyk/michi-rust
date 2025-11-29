//! Monte Carlo playouts (random game simulation).
//!
//! This module implements random playouts for evaluating positions.
//! A playout plays random legal moves until the game ends, then scores the result.
//!
//! Heuristics used during playouts:
//! - Capture moves prioritization (fix_atari)
//! - 3x3 pattern matching
//! - Self-atari rejection

use crate::constants::{
    BOARD_IMAX, BOARD_IMIN, EMPTY, MAX_GAME_LEN, N, W,
    PROB_HEURISTIC_CAPTURE, PROB_HEURISTIC_PAT3, PROB_RSAREJECT, PROB_SSAREJECT,
    STONE_BLACK,
};
use crate::patterns::pat3_match;
use crate::position::{
    all_neighbors, fix_atari, is_eye, is_eyeish, pass_move, play_move, Point, Position,
};

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
pub fn random_int(n: u32) -> u32 {
    let r = qdrandom() as u64;
    ((r * n as u64) >> 32) as u32
}

/// Generate a random float in [0, 1).
#[inline]
fn random_float() -> f64 {
    (qdrandom() as f64) / (u32::MAX as f64)
}

/// Perform a Monte Carlo playout from the given position.
///
/// Plays moves using heuristics until two consecutive passes or the game length limit.
/// Returns a score from the perspective of the player to move at the start:
/// - Positive score = starting player wins
/// - Negative score = starting player loses
///
/// If `amaf_map` is provided, updates it with who played at each position first
/// (1 for Black, -1 for White). This is used for RAVE/AMAF heuristic in MCTS.
pub fn mcplayout(pos: &mut Position, mut amaf_map: Option<&mut [i8]>) -> f64 {
    let start_n = pos.n;
    let mut passes = 0;

    while passes < 2 && pos.n < MAX_GAME_LEN {
        if let Some(pt) = choose_playout_move(pos) {
            // Update AMAF map before playing the move
            if let Some(ref mut amaf) = amaf_map {
                if amaf[pt] == 0 {
                    // Mark with 1 for black, -1 for white
                    // pos.n % 2 == 0 means it's Black's turn (move 0, 2, 4, ...)
                    amaf[pt] = if pos.n % 2 == 0 { 1 } else { -1 };
                }
            }
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

/// Choose a move for the playout using heuristics.
///
/// Tries moves in this order of preference:
/// 1. Capture moves (atari responses)
/// 2. 3x3 pattern moves
/// 3. Random legal move
///
/// Also rejects self-atari moves with high probability.
fn choose_playout_move(pos: &Position) -> Option<Point> {
    // Get the neighborhood of the last two moves for focused heuristics
    let neighbors = make_list_last_moves_neighbors(pos);

    // 1. Try capture heuristics (with probability PROB_HEURISTIC_CAPTURE)
    if random_float() < PROB_HEURISTIC_CAPTURE {
        if let Some(mv) = try_capture_moves(pos, &neighbors) {
            return Some(mv);
        }
    }

    // 2. Try 3x3 pattern moves (with probability PROB_HEURISTIC_PAT3)
    if random_float() < PROB_HEURISTIC_PAT3 {
        if let Some(mv) = try_pattern_moves(pos, &neighbors) {
            return Some(mv);
        }
    }

    // 3. Fall back to random move
    choose_random_move(pos)
}

/// Generate a list of points in the neighborhood of the last two moves.
fn make_list_last_moves_neighbors(pos: &Position) -> Vec<Point> {
    let mut points = Vec::with_capacity(20);

    // Add last move and its neighbors
    if pos.last != 0 {
        points.push(pos.last);
        for n in all_neighbors(pos.last) {
            if pos.color[n] != b' ' && !points.contains(&n) {
                points.push(n);
            }
        }
    }

    // Add last2 move and its neighbors
    if pos.last2 != 0 {
        if !points.contains(&pos.last2) {
            points.push(pos.last2);
        }
        for n in all_neighbors(pos.last2) {
            if pos.color[n] != b' ' && !points.contains(&n) {
                points.push(n);
            }
        }
    }

    // Shuffle for randomization
    let len = points.len();
    for i in 0..len {
        let j = i + random_int((len - i) as u32) as usize;
        points.swap(i, j);
    }

    points
}

/// Try to find a capture move among the neighbor points.
fn try_capture_moves(pos: &Position, neighbors: &[Point]) -> Option<Point> {
    for &pt in neighbors {
        if pos.color[pt] == STONE_BLACK || pos.color[pt] == b'x' {
            let moves = fix_atari(pos, pt, false);
            for mv in moves {
                if try_move_with_self_atari_check(pos, mv, false) {
                    return Some(mv);
                }
            }
        }
    }
    None
}

/// Try to find a 3x3 pattern move among the neighbor points.
fn try_pattern_moves(pos: &Position, neighbors: &[Point]) -> Option<Point> {
    for &pt in neighbors {
        if pos.color[pt] == EMPTY && pat3_match(pos, pt) {
            if try_move_with_self_atari_check(pos, pt, false) {
                return Some(pt);
            }
        }
    }
    None
}

/// Check if a move is legal and not a self-atari (with probability-based rejection).
///
/// `is_random`: if true, uses lower rejection probability (PROB_RSAREJECT = 0.5)
///              if false, uses higher rejection probability (PROB_SSAREJECT = 0.9)
fn try_move_with_self_atari_check(pos: &Position, pt: Point, is_random: bool) -> bool {
    let mut test_pos = pos.clone();
    if !play_move(&mut test_pos, pt).is_empty() {
        return false; // Illegal move
    }

    // Check for self-atari and reject with probability based on move type
    // Random moves use lower rejection rate to allow more nakade/tactical moves
    let reject_prob = if is_random { PROB_RSAREJECT } else { PROB_SSAREJECT };
    if random_float() < reject_prob {
        let moves = fix_atari(&test_pos, pt, true);
        if !moves.is_empty() {
            // This move puts us in atari - reject it
            return false;
        }
    }

    true
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
    // (some candidates might be suicide moves or self-atari)
    let n = candidates.len();
    for i in 0..n {
        // Pick a random remaining candidate
        let j = i + random_int((n - i) as u32) as usize;
        candidates.swap(i, j);

        let pt = candidates[i];

        // Use is_random=true for lower self-atari rejection rate
        if try_move_with_self_atari_check(pos, pt, true) {
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
