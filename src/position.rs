//! Go position representation and move execution.
//!
//! This module provides the core game logic for Go, including:
//! - Board state representation using a 1D array with padding
//! - Stone placement and capture detection
//! - Ko rule enforcement
//! - Eye detection for playout optimization
//!
//! The board uses a color-swapping scheme where the current player's stones
//! are always `'X'` and the opponent's stones are `'x'`. This simplifies
//! move generation by always checking from the perspective of `'X'`.

use crate::constants::*;

/// A point on the board, represented as an index into the 1D board array.
pub type Point = usize;

/// Result of attempting to play a move.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MoveError {
    /// Point is not empty
    Occupied,
    /// Move violates ko rule
    Ko,
    /// Move would be suicide (no liberties after capture resolution)
    Suicide,
}

impl std::fmt::Display for MoveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MoveError::Occupied => write!(f, "Error Illegal move: point not EMPTY"),
            MoveError::Ko => write!(f, "Error Illegal move: retakes ko"),
            MoveError::Suicide => write!(f, "Error Illegal move: suicide"),
        }
    }
}

/// A Go position (board state).
///
/// The board is represented as a 1D array with padding around the edges.
/// Colors are swapped after each move so that the current player is always `'X'`.
#[derive(Clone)]
pub struct Position {
    /// Board state: 'X' = current player, 'x' = opponent, '.' = empty, ' ' = out of bounds
    pub color: [u8; BOARDSIZE],
    /// Encoded colors of 4 orthogonal neighbors (N, E, S, W) for pattern matching.
    /// Each neighbor uses 2 bits: 0=WHITE, 1=BLACK, 2=EMPTY, 3=OUT.
    /// Updated incrementally when stones are placed/removed.
    pub env4: [u8; BOARDSIZE],
    /// Encoded colors of 4 diagonal neighbors (NE, SE, SW, NW) for pattern matching.
    /// Uses same encoding as `env4`.
    pub env4d: [u8; BOARDSIZE],
    /// Move number (0 = start of game)
    pub n: usize,
    /// Ko point (0 if no ko)
    pub ko: Point,
    /// Previous ko point (for restoration on undo)
    pub ko_old: Point,
    /// Last move played
    pub last: Point,
    /// Second-to-last move
    pub last2: Point,
    /// Third-to-last move
    pub last3: Point,
    /// Captures by current player ('X')
    pub cap: u32,
    /// Captures by opponent ('x')
    pub cap_x: u32,
    /// Komi (compensation points for White)
    pub komi: f32,
}

impl Default for Position {
    fn default() -> Self {
        Self::new()
    }
}

impl Position {
    pub fn new() -> Self {
        let mut p = Position {
            color: [b' '; BOARDSIZE],
            env4: [0; BOARDSIZE],
            env4d: [0; BOARDSIZE],
            n: 0,
            ko: 0,
            ko_old: 0,
            last: 0,
            last2: 0,
            last3: 0,
            cap: 0,
            cap_x: 0,
            komi: 7.5,
        };
        empty_position(&mut p);
        p
    }
}

// =============================================================================
// Env4/Env4d: Neighbor color encoding for fast pattern matching
// =============================================================================

/// Color encoding for env4/env4d arrays.
/// These encode neighbor colors using absolute colors (BLACK/WHITE) not relative (X/x).
#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Env4Color {
    White = 0,
    Black = 1,
    Empty = 2,
    Out = 3,
}

impl From<u8> for Env4Color {
    fn from(c: u8) -> Self {
        match c {
            0 => Env4Color::White,
            1 => Env4Color::Black,
            2 => Env4Color::Empty,
            _ => Env4Color::Out,
        }
    }
}

/// Compute the env4 value for a point from scratch.
///
/// `offset` determines which neighbors to encode:
/// - `0` = orthogonal neighbors (N, E, S, W) for `env4`
/// - `4` = diagonal neighbors (NE, SE, SW, NW) for `env4d`
///
/// The encoding uses absolute colors:
/// - 0: WHITE
/// - 1: BLACK
/// - 2: EMPTY
/// - 3: OUT (off-board)
///
/// Each of the 4 neighbors uses 2 bits, stored in a single byte:
/// - Bits 0,4: First neighbor (high bit in position 4, low bit in position 0)
/// - Bits 1,5: Second neighbor
/// - Bits 2,6: Third neighbor
/// - Bits 3,7: Fourth neighbor
pub fn compute_env4(pos: &Position, pt: Point, offset: usize) -> u8 {
    let mut env4: u8 = 0;

    for k in 0..4 {
        let n = (pt as isize + DELTA[offset + k]) as usize;

        // Determine color code: 0=WHITE, 1=BLACK, 2=EMPTY, 3=OUT
        let c: u8 = if pos.color[n] == EMPTY {
            2 // EMPTY
        } else if pos.color[n] == OUT {
            3 // OUT
        } else {
            // env4 uses absolute colors based on move number
            if pos.n % 2 == 0 {
                // BLACK to play (X=BLACK, x=WHITE)
                if pos.color[n] == STONE_BLACK { 1 } else { 0 }
            } else {
                // WHITE to play (X=WHITE, x=BLACK)
                if pos.color[n] == STONE_BLACK { 0 } else { 1 }
            }
        };

        // Pack into the byte: high bit at position k+4, low bit at position k
        let hi = c >> 1;
        let lo = c & 1;
        env4 |= ((hi << 4) | lo) << k;
    }

    env4
}

/// Place a stone on the board and update env4/env4d arrays incrementally.
///
/// Always places a stone of color 'X' (current player).
/// Updates the neighbor encodings of all adjacent points.
pub fn put_stone(pos: &mut Position, pt: Point) {
    // Update env4 for orthogonal neighbors
    // When a stone is placed, neighbors see this point change from EMPTY to a stone
    //
    // Neighbor layout for env4 updates:
    // - South neighbor (pt + N+1) sees pt at its North (bit position 0)
    // - West neighbor (pt - 1) sees pt at its East (bit position 1)
    // - North neighbor (pt - N-1) sees pt at its South (bit position 2)
    // - East neighbor (pt + 1) sees pt at its West (bit position 3)
    //
    // For env4d:
    // - SW neighbor (pt + N) sees pt at its NE (bit position 0)
    // - NW neighbor (pt - W) sees pt at its SE (bit position 1)
    // - NE neighbor (pt - N) sees pt at its SW (bit position 2)
    // - SE neighbor (pt + W) sees pt at its NW (bit position 3)

    let pt = pt as isize;
    let n_plus_1 = (N + 1) as isize;
    let w = W as isize;
    let n = N as isize;

    if pos.n % 2 == 0 {
        // BLACK to play (X=BLACK)
        // EMPTY (0b10) -> BLACK (0b01): XOR with 0x11 for position 0, 0x22 for 1, etc.
        pos.env4[(pt + n_plus_1) as usize] ^= 0x11; // South neighbor
        pos.env4[(pt - 1) as usize] ^= 0x22;        // West neighbor
        pos.env4[(pt - n_plus_1) as usize] ^= 0x44; // North neighbor
        pos.env4[(pt + 1) as usize] ^= 0x88;        // East neighbor
        pos.env4d[(pt + n) as usize] ^= 0x11;       // SW neighbor
        pos.env4d[(pt - w) as usize] ^= 0x22;       // NW neighbor
        pos.env4d[(pt - n) as usize] ^= 0x44;       // NE neighbor
        pos.env4d[(pt + w) as usize] ^= 0x88;       // SE neighbor
    } else {
        // WHITE to play (X=WHITE)
        // EMPTY (0b10) -> WHITE (0b00): AND with complement to clear high bit
        pos.env4[(pt + n_plus_1) as usize] &= 0xEE;
        pos.env4[(pt - 1) as usize] &= 0xDD;
        pos.env4[(pt - n_plus_1) as usize] &= 0xBB;
        pos.env4[(pt + 1) as usize] &= 0x77;
        pos.env4d[(pt + n) as usize] &= 0xEE;
        pos.env4d[(pt - w) as usize] &= 0xDD;
        pos.env4d[(pt - n) as usize] &= 0xBB;
        pos.env4d[(pt + w) as usize] &= 0x77;
    }
    pos.color[pt as usize] = STONE_BLACK;
}

/// Remove a stone from the board and update env4/env4d arrays incrementally.
///
/// Always removes a stone of color 'x' (opponent).
/// Updates the neighbor encodings of all adjacent points.
pub fn remove_stone(pos: &mut Position, pt: Point) {
    // Update env4 for orthogonal neighbors
    // When a stone is removed, neighbors see this point change from a stone to EMPTY

    let pt = pt as isize;
    let n_plus_1 = (N + 1) as isize;
    let w = W as isize;
    let n = N as isize;

    if pos.n % 2 == 0 {
        // BLACK to play (x=WHITE)
        // WHITE (0b00) -> EMPTY (0b10): OR with 0x10 for position 0 to set high bit
        pos.env4[(pt + n_plus_1) as usize] |= 0x10;
        pos.env4[(pt - 1) as usize] |= 0x20;
        pos.env4[(pt - n_plus_1) as usize] |= 0x40;
        pos.env4[(pt + 1) as usize] |= 0x80;
        pos.env4d[(pt + n) as usize] |= 0x10;
        pos.env4d[(pt - w) as usize] |= 0x20;
        pos.env4d[(pt - n) as usize] |= 0x40;
        pos.env4d[(pt + w) as usize] |= 0x80;
    } else {
        // WHITE to play (x=BLACK)
        // BLACK (0b01) -> EMPTY (0b10): XOR with 0x11 for each position
        pos.env4[(pt + n_plus_1) as usize] ^= 0x11;
        pos.env4[(pt - 1) as usize] ^= 0x22;
        pos.env4[(pt - n_plus_1) as usize] ^= 0x44;
        pos.env4[(pt + 1) as usize] ^= 0x88;
        pos.env4d[(pt + n) as usize] ^= 0x11;
        pos.env4d[(pt - w) as usize] ^= 0x22;
        pos.env4d[(pt - n) as usize] ^= 0x44;
        pos.env4d[(pt + w) as usize] ^= 0x88;
    }
    pos.color[pt as usize] = EMPTY;
}

/// Verify that env4/env4d arrays are consistent with the board state.
///
/// This is a debug function that recomputes env4/env4d from scratch
/// and compares with the stored values. Returns true if consistent.
#[cfg(debug_assertions)]
pub fn env4_ok(pos: &Position) -> bool {
    for pt in BOARD_IMIN..BOARD_IMAX {
        if pos.color[pt] == OUT {
            continue;
        }
        let computed_env4 = compute_env4(pos, pt, 0);
        if pos.env4[pt] != computed_env4 {
            return false;
        }
        let computed_env4d = compute_env4(pos, pt, 4);
        if pos.env4d[pt] != computed_env4d {
            return false;
        }
    }
    true
}

#[cfg(not(debug_assertions))]
pub fn env4_ok(_pos: &Position) -> bool {
    true
}

/// Reset a position to the initial empty board state.
///
/// The board is laid out as a 1D array with padding:
/// - Index 0 to N: top padding (out of bounds)
/// - Each row: left padding + N playable points
/// - Bottom padding
///
/// Returns an empty string for compatibility (can be used in a chain).
pub fn empty_position(pos: &mut Position) -> &'static str {
    // Reset to initial position with C padding layout
    let mut k = 0;
    for _col in 0..=N {
        pos.color[k] = b' ';
        k += 1;
    }
    for _row in 1..=N {
        pos.color[k] = b' ';
        k += 1;
        for _col in 1..=N {
            pos.color[k] = b'.';
            k += 1;
        }
    }
    for _col in 0..W {
        pos.color[k] = b' ';
        k += 1;
    }

    // Initialize env4/env4d arrays
    for pt in BOARD_IMIN..BOARD_IMAX {
        if pos.color[pt] == OUT {
            continue;
        }
        pos.env4[pt] = compute_env4(pos, pt, 0);
        pos.env4d[pt] = compute_env4(pos, pt, 4);
    }

    pos.ko = 0;
    pos.last = 0;
    pos.last2 = 0;
    pos.last3 = 0;
    pos.cap = 0;
    pos.cap_x = 0;
    pos.n = 0;

    debug_assert!(env4_ok(pos), "env4/env4d initialization failed");
    ""
}

/// Swap stone colors (X <-> x) to change the current player.
///
/// This is called after each move so that the current player is always 'X'.
/// This simplifies move generation and evaluation logic.
fn swap_color(pos: &mut Position) {
    for c in &mut pos.color {
        *c = match *c {
            STONE_BLACK => STONE_WHITE,
            STONE_WHITE => STONE_BLACK,
            other => other,
        };
    }
}

/// Execute a pass move.
///
/// This increments the move counter, swaps colors, and clears the ko.
/// Returns an empty string for compatibility.
pub fn pass_move(pos: &mut Position) -> &'static str {
    swap_color(pos);
    pos.n += 1;
    pos.last3 = pos.last2;
    pos.last2 = pos.last;
    pos.last = PASS_MOVE;
    pos.ko = 0; // Ko is cleared on pass
    std::mem::swap(&mut pos.cap, &mut pos.cap_x);
    ""
}

/// Check if a point is "eyeish" (surrounded by stones of one color).
///
/// A point is eyeish if all its orthogonal neighbors are either:
/// - Out of bounds (padding), or
/// - Stones of the same color
///
/// Returns the color of the surrounding stones, or 0 if not eyeish.
/// Note: This may return true for false eyes.
pub fn is_eyeish(pos: &Position, pt: Point) -> u8 {
    let mut eyecolor: u8 = 0;
    let mut othercolor: u8 = 0;
    for n in neighbors(pt) {
        let c = pos.color[n];
        if c == OUT {
            continue; // Ignore out-of-bounds neighbors
        }
        if c == EMPTY {
            return 0;
        }
        if eyecolor == 0 {
            eyecolor = c;
            othercolor = if c == STONE_BLACK {
                STONE_WHITE
            } else {
                STONE_BLACK
            };
        } else if c == othercolor {
            return 0;
        }
    }
    eyecolor
}

/// Check if a point is a true eye.
///
/// A true eye is eyeish and has at most one "bad" diagonal:
/// - At edge: one bad diagonal allowed
/// - In center: zero bad diagonals allowed
///
/// A diagonal is "bad" if it contains an opponent stone.
/// Returns the color of the eye, or 0 if not a true eye.
pub fn is_eye(pos: &Position, pt: Point) -> u8 {
    let eyecolor = is_eyeish(pos, pt);
    if eyecolor == 0 {
        return 0;
    }
    let falsecolor = if eyecolor == STONE_BLACK {
        STONE_WHITE
    } else {
        STONE_BLACK
    };
    let mut at_edge = false;
    let mut false_count = 0;

    for d in diagonal_neighbors(pt) {
        if pos.color[d] == OUT {
            at_edge = true;
        } else if pos.color[d] == falsecolor {
            false_count += 1;
        }
    }

    // At edge, we tolerate one bad diagonal; in center, zero
    let tolerance = if at_edge { 1 } else { 0 };
    if false_count > tolerance {
        return 0;
    }
    eyecolor
}

/// Play a move at the given point.
///
/// Handles pass moves, legality checking, captures, ko detection, and color swapping.
/// Returns an empty string on success, or an error message on failure.
///
/// # Errors
/// - "Error Illegal move: point not EMPTY" - if the point is occupied
/// - "Error Illegal move: retakes ko" - if the move violates the ko rule
/// - "Error Illegal move: suicide" - if the move would have no liberties
pub fn play_move(pos: &mut Position, pt: Point) -> &'static str {
    if pt == PASS_MOVE {
        return pass_move(pos);
    }
    if pos.color[pt] != EMPTY {
        return "Error Illegal move: point not EMPTY";
    }

    // Check ko
    pos.ko_old = pos.ko;
    if pt == pos.ko {
        return "Error Illegal move: retakes ko";
    }

    // Check if playing into enemy eye (for ko detection)
    let in_enemy_eye = is_eyeish(pos, pt);

    // Place the stone using put_stone (updates env4/env4d)
    put_stone(pos, pt);

    let mut captured = 0u32;
    let mut capture_point: Point = 0;
    let mut to_remove: Vec<Point> = Vec::new();
    let mut capture_visited = [false; BOARDSIZE]; // Track which stones we've already marked for capture

    for n in neighbors(pt) {
        // Skip if we've already processed this stone (part of a group we already captured)
        if capture_visited[n] {
            continue;
        }
        if pos.color[n] == STONE_WHITE && group_liberties(pos, n) == 0 {
            let group_size = collect_group_with_visited(pos, n, &mut to_remove, &mut capture_visited);
            captured += group_size;
            capture_point = n;
        }
    }

    // Remove captured stones using remove_stone (updates env4/env4d)
    for &r in &to_remove {
        remove_stone(pos, r);
    }

    if captured > 0 {
        // Set ko if captured exactly one stone in an eye
        if captured == 1 && in_enemy_eye != 0 {
            pos.ko = capture_point;
        } else {
            pos.ko = 0;
        }
    } else {
        // Test for suicide
        pos.ko = 0;
        if group_liberties(pos, pt) == 0 {
            // Undo the stone placement (need to restore env4/env4d too)
            pos.color[pt] = EMPTY;
            // Restore env4/env4d by recomputing (simpler than inverse of put_stone)
            for k in 0..4 {
                let n = (pt as isize + DELTA[k]) as usize;
                if pos.color[n] != OUT {
                    pos.env4[n] = compute_env4(pos, n, 0);
                }
            }
            for k in 4..8 {
                let n = (pt as isize + DELTA[k]) as usize;
                if pos.color[n] != OUT {
                    pos.env4d[n] = compute_env4(pos, n, 4);
                }
            }
            pos.ko = pos.ko_old;
            return "Error Illegal move: suicide";
        }
    }

    // Update captures (cumulative)
    let total_captured = captured + pos.cap_x;
    pos.cap_x = pos.cap;
    pos.cap = total_captured;

    swap_color(pos);
    pos.n += 1;
    pos.last3 = pos.last2;
    pos.last2 = pos.last;
    pos.last = pt;

    debug_assert!(env4_ok(pos), "env4/env4d inconsistent after play_move");
    ""
}

/// Get the 4 orthogonal neighbors (N, E, S, W) of a point.
#[inline]
fn neighbors(pt: Point) -> [Point; 4] {
    [
        (pt as isize + DELTA[0]) as usize,
        (pt as isize + DELTA[1]) as usize,
        (pt as isize + DELTA[2]) as usize,
        (pt as isize + DELTA[3]) as usize,
    ]
}

/// Get the 4 diagonal neighbors (NE, SE, SW, NW) of a point.
#[inline]
fn diagonal_neighbors(pt: Point) -> [Point; 4] {
    [
        (pt as isize + DELTA[4]) as usize,
        (pt as isize + DELTA[5]) as usize,
        (pt as isize + DELTA[6]) as usize,
        (pt as isize + DELTA[7]) as usize,
    ]
}

/// Get all 8 neighbors (4 orthogonal + 4 diagonal) of a point.
#[inline]
pub fn all_neighbors(pt: Point) -> [Point; 8] {
    std::array::from_fn(|i| (pt as isize + DELTA[i]) as usize)
}

/// Collect all stones in a group starting from a point.
///
/// Uses flood-fill to find all connected stones of the same color.
/// Returns the number of stones in the group and appends them to `out`.
#[allow(dead_code)]
fn collect_group(pos: &Position, start: Point, out: &mut Vec<Point>) -> u32 {
    let mut visited = [false; BOARDSIZE];
    collect_group_with_visited(pos, start, out, &mut visited)
}

/// Collect all stones in a group, using a provided visited array.
///
/// This version allows sharing the visited array across multiple calls,
/// which prevents collecting the same stone twice when processing multiple
/// adjacent groups.
fn collect_group_with_visited(
    pos: &Position,
    start: Point,
    out: &mut Vec<Point>,
    visited: &mut [bool; BOARDSIZE],
) -> u32 {
    let color = pos.color[start];
    let mut stack = vec![start];
    let mut count = 0u32;

    while let Some(pt) = stack.pop() {
        if visited[pt] {
            continue;
        }
        visited[pt] = true;

        if pos.color[pt] == color {
            out.push(pt);
            count += 1;
            for n in neighbors(pt) {
                if !visited[n] && pos.color[n] == color {
                    stack.push(n);
                }
            }
        }
    }
    count
}

/// Count the number of liberties (empty adjacent points) of a group.
///
/// Uses flood-fill to traverse the group and count unique empty neighbors.
fn group_liberties(pos: &Position, start: Point) -> u32 {
    let color = pos.color[start];
    let mut stack = vec![start];
    let mut visited = [false; BOARDSIZE];
    let mut liberty_visited = [false; BOARDSIZE];
    let mut libs = 0u32;

    while let Some(pt) = stack.pop() {
        if visited[pt] {
            continue;
        }
        visited[pt] = true;

        if pos.color[pt] == color {
            for n in neighbors(pt) {
                match pos.color[n] {
                    EMPTY => {
                        if !liberty_visited[n] {
                            liberty_visited[n] = true;
                            libs += 1;
                        }
                    }
                    c if c == color && !visited[n] => stack.push(n),
                    _ => {}
                }
            }
        }
    }
    libs
}

// =============================================================================
// Atari Detection and Capture Heuristics
// =============================================================================

/// Compute a block (group) of stones at a given point.
///
/// Returns the stones in the group and their liberties (up to `max_libs` liberties).
/// This is similar to the C `compute_block` function.
pub fn compute_block(
    pos: &Position,
    start: Point,
    max_libs: usize,
) -> (Vec<Point>, Vec<Point>) {
    let color = pos.color[start];
    let mut stones = Vec::new();
    let mut libs = Vec::new();
    let mut visited = [false; BOARDSIZE];
    let mut lib_visited = [false; BOARDSIZE];
    let mut stack = vec![start];
    visited[start] = true;

    while let Some(pt) = stack.pop() {
        stones.push(pt);
        for n in neighbors(pt) {
            if visited[n] {
                continue;
            }
            visited[n] = true;
            if pos.color[n] == color {
                stack.push(n);
            } else if pos.color[n] == EMPTY && !lib_visited[n] {
                lib_visited[n] = true;
                libs.push(n);
                if libs.len() >= max_libs {
                    return (stones, libs);
                }
            }
        }
    }

    (stones, libs)
}

/// Find neighbor blocks in atari (opponent blocks with only 1 liberty).
///
/// Given a list of stones, finds all opponent blocks adjacent to them that
/// have exactly one liberty. Returns pairs of (representative stone, liberty).
pub fn find_neighbor_blocks_in_atari(pos: &Position, stones: &[Point]) -> Vec<(Point, Point)> {
    let color = pos.color[stones[0]];
    let opponent = if color == STONE_BLACK { STONE_WHITE } else { STONE_BLACK };

    let mut result = Vec::new();
    let mut block_visited = [false; BOARDSIZE];

    for &stone in stones {
        for n in neighbors(stone) {
            if pos.color[n] == opponent && !block_visited[n] {
                let (block_stones, libs) = compute_block(pos, n, 2);
                // Mark all stones in this block as visited
                for &s in &block_stones {
                    block_visited[s] = true;
                }
                // If exactly one liberty, it's in atari
                if libs.len() == 1 {
                    result.push((block_stones[0], libs[0]));
                }
            }
        }
    }

    result
}

/// Check if a group is in atari and find moves that can save it or capture neighbors.
///
/// Returns a list of suggested moves. This is a simplified version of the C `fix_atari`.
///
/// Parameters:
/// - `pos`: Current position
/// - `pt`: A point in the group to check
/// - `singlept_ok`: If true, don't try to save single-stone groups
///
/// Returns moves that can:
/// - Capture opponent stones (if the group belongs to opponent)
/// - Escape by playing on the last liberty
/// - Counter-capture adjacent opponent groups in atari
pub fn fix_atari(pos: &Position, pt: Point, singlept_ok: bool) -> Vec<Point> {
    let mut moves = Vec::new();

    // Compute the block
    let (stones, libs) = compute_block(pos, pt, 3);

    // If single stone and singlept_ok, don't bother
    if singlept_ok && stones.len() == 1 {
        return moves;
    }

    // If 2 or more liberties, not in atari
    if libs.len() >= 2 {
        return moves;
    }

    // Block is in atari (exactly 1 liberty)
    let lib = libs[0];

    if pos.color[pt] == STONE_WHITE {
        // This is opponent's group - we can capture it!
        moves.push(lib);
        return moves;
    }

    // This is our group and it's in atari
    // Try counter-capturing neighbor blocks first
    let atari_neighbors = find_neighbor_blocks_in_atari(pos, &stones);
    for (_, capture_lib) in atari_neighbors {
        if !moves.contains(&capture_lib) {
            moves.push(capture_lib);
        }
    }

    // Try escaping by playing on our liberty
    // First check if it would actually give us more liberties
    let mut test_pos = pos.clone();
    if play_move(&mut test_pos, lib).is_empty() {
        let (_, new_libs) = compute_block(&test_pos, lib, 3);
        if new_libs.len() >= 2 {
            // Good, we escape
            if !moves.contains(&lib) {
                moves.push(lib);
            }
        }
    }

    moves
}

/// Generate capture moves in the neighborhood of recent moves.
///
/// Looks at groups near `last` and `last2` moves and finds:
/// - Opponent groups in atari (can capture)
/// - Own groups in atari (need to save)
///
/// Returns (move, group_size) pairs for prioritization.
pub fn gen_capture_moves(pos: &Position) -> Vec<(Point, usize)> {
    let mut moves = Vec::new();
    let mut checked = [false; BOARDSIZE];

    // Get neighbor points of last moves
    let mut points_to_check = Vec::new();

    if pos.last != 0 {
        points_to_check.push(pos.last);
        for n in all_neighbors(pos.last) {
            if pos.color[n] != OUT {
                points_to_check.push(n);
            }
        }
    }

    if pos.last2 != 0 {
        for n in all_neighbors(pos.last2) {
            if pos.color[n] != OUT && !points_to_check.contains(&n) {
                points_to_check.push(n);
            }
        }
    }

    for pt in points_to_check {
        if checked[pt] {
            continue;
        }

        if pos.color[pt] == STONE_BLACK || pos.color[pt] == STONE_WHITE {
            checked[pt] = true;
            let atari_moves = fix_atari(pos, pt, false);

            for m in atari_moves {
                // Get the size of the group that would be affected
                let (stones, _) = compute_block(pos, pt, 1);
                if !moves.iter().any(|(mv, _)| *mv == m) {
                    moves.push((m, stones.len()));
                }
            }
        }
    }

    moves
}

/// Parse a coordinate string (e.g., "D4", "pass") into a Point.
///
/// Go coordinates use letters A-T (skipping I) for columns and 1-19 for rows.
/// Returns `PASS_MOVE` for "pass" or invalid input.
pub fn parse_coord(s: &str) -> Point {
    if s.eq_ignore_ascii_case("pass") {
        return PASS_MOVE;
    }

    let bytes = s.as_bytes();
    if bytes.len() < 2 {
        return PASS_MOVE;
    }

    let col_char = bytes[0].to_ascii_uppercase();
    let mut col = (col_char - b'A' + 1) as usize;

    // Skip 'I' column (Go convention to avoid confusion with 'J')
    if col_char > b'I' {
        col -= 1;
    }

    // Parse row number
    let row: usize = bytes[1..]
        .iter()
        .filter(|b| b.is_ascii_digit())
        .fold(0, |acc, &b| acc * 10 + (b - b'0') as usize);

    (N - row + 1) * (N + 1) + col
}

/// Convert a Point to a coordinate string (e.g., "D4").
///
/// Returns "pass" for `PASS_MOVE`.
pub fn str_coord(pt: Point) -> String {
    if pt == PASS_MOVE {
        return "pass".into();
    }

    let row = pt / (N + 1);
    let col = pt % (N + 1);

    // Convert column to letter, skipping 'I'
    let mut c = (b'@' + col as u8) as char;
    if c >= 'I' {
        c = (c as u8 + 1) as char;
    }

    format!("{c}{}", N + 1 - row)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_position() {
        let pos = Position::new();
        // Check that the center is empty
        let center = (N / 2 + 1) * (N + 1) + (N / 2 + 1);
        assert_eq!(pos.color[center], b'.');
        assert_eq!(pos.n, 0);
        assert_eq!(pos.ko, 0);
    }

    #[test]
    fn test_parse_str_coord_roundtrip() {
        let pos = Position::new();
        // Test some coordinates
        for row in 1..=N {
            for col in 1..=N {
                let pt = row * (N + 1) + col;
                if pos.color[pt] == b'.' {
                    let s = str_coord(pt);
                    let parsed = parse_coord(&s);
                    assert_eq!(pt, parsed, "Failed roundtrip for {}", s);
                }
            }
        }
    }

    #[test]
    fn test_play_move_basic() {
        let mut pos = Position::new();
        let pt = parse_coord("D4");
        let result = play_move(&mut pos, pt);
        assert!(result.is_empty(), "Move should be legal");
        assert_eq!(pos.n, 1);
        assert_eq!(pos.last, pt);
    }

    #[test]
    fn test_play_move_suicide() {
        let mut pos = Position::new();
        // Create a situation where playing at a point would be suicide
        // Set up the corner: Black stones at A2 and B1,
        // then Black tries to play at A1 - this would be suicide

        // But wait - after each move colors swap!
        // Move 1: Black plays A2 (becomes x after swap)
        play_move(&mut pos, parse_coord("A2"));
        // Move 2: White plays somewhere (becomes x, Black's A2 becomes X)
        play_move(&mut pos, parse_coord("E5")); // Valid on both 9x9 and 13x13
        // Move 3: Black plays B1 (becomes x)
        play_move(&mut pos, parse_coord("B1"));

        // Now it's White's turn. The corner A1 is surrounded by Black stones
        // (which are now 'x' since it's White's turn)
        // White playing A1 would be suicide
        let corner = parse_coord("A1");
        let result = play_move(&mut pos, corner);
        assert!(
            result.contains("suicide"),
            "A1 should be suicide for White: got '{}'",
            result
        );
    }
    #[test]
    fn test_capture() {
        let mut pos = Position::new();
        // Black plays, White plays, Black captures
        let b1 = parse_coord("C3");
        let w1 = parse_coord("D3");
        let b2 = parse_coord("E3");
        let w2 = parse_coord("D4");
        let b3 = parse_coord("D2");
        let w3 = parse_coord("E5"); // White plays elsewhere (valid on both 9x9 and 13x13)
        let b4 = parse_coord("C4"); // Now black can capture
        let _w4 = parse_coord("E4");

        play_move(&mut pos, b1);
        play_move(&mut pos, w1);
        play_move(&mut pos, b2);
        play_move(&mut pos, w2);
        play_move(&mut pos, b3);
        play_move(&mut pos, w3);

        // Before capture, D3 should have x (opponent stone)
        assert_eq!(pos.color[w1], b'x');

        // Play capturing move
        let result = play_move(&mut pos, b4);
        assert!(result.is_empty(), "Capture move should be legal");
    }

    #[test]
    fn test_ko_rule() {
        let pos = Position::new();
        // Set up a ko situation
        // This is a simplified test - a real ko test would need more setup
        // For now, just verify the ko field is being set

        assert_eq!(pos.ko, 0); // Initially no ko
    }

    #[test]
    fn test_group_liberties() {
        let mut pos = Position::new();
        let pt = parse_coord("D4");
        play_move(&mut pos, pt);

        // A single stone in the middle should have 4 liberties
        // After play_move, colors are swapped, so the stone is 'x'
        // The group_liberties function works on the raw position
        let libs = group_liberties(&pos, pt);
        assert_eq!(libs, 4, "Single stone should have 4 liberties");
    }

    #[test]
    fn test_is_eye() {
        let pos = Position::new();
        // Create a simple eye pattern in the corner
        // This would require more careful setup for a proper test

        // Empty position should not be an eye
        let pt = parse_coord("A1");
        assert_eq!(is_eye(&pos, pt), 0);
    }

    #[test]
    fn test_env4_after_moves() {
        let mut pos = Position::new();

        // Play a few moves and verify env4 consistency after each
        let moves = ["D4", "E4", "D5", "E5", "C4", "F4"];
        for m in moves {
            let pt = parse_coord(m);
            let result = play_move(&mut pos, pt);
            assert!(result.is_empty(), "Move {} should be legal: {}", m, result);
            assert!(env4_ok(&pos), "env4 inconsistent after move {}", m);
        }
    }

    #[test]
    fn test_env4_after_capture() {
        let mut pos = Position::new();

        // Set up a capture scenario
        play_move(&mut pos, parse_coord("B1")); // Black
        assert!(env4_ok(&pos), "env4 inconsistent after B1");
        play_move(&mut pos, parse_coord("A1")); // White in corner
        assert!(env4_ok(&pos), "env4 inconsistent after A1");
        play_move(&mut pos, parse_coord("A2")); // Black captures

        // After capture, env4 should still be consistent
        assert!(env4_ok(&pos), "env4 inconsistent after capture");
    }

    #[test]
    fn test_env4_many_captures() {
        // Test many captures to catch edge cases
        let mut pos = Position::new();
        let moves = [
            "D4", "E4", // Black, White
            "D5", "E5", // Black, White
            "D6", "F4", // Black, White far
            "E6", "F5", // Black, White (E5 group loses liberty)
            "F6", // Black captures E4, E5
        ];
        for (i, m) in moves.iter().enumerate() {
            let result = play_move(&mut pos, parse_coord(m));
            assert!(result.is_empty() || result.contains("suicide"),
                    "Move {} ({}) failed: {}", i, m, result);
            assert!(env4_ok(&pos), "env4 inconsistent after move {} ({})", i, m);
        }
    }

    #[test]
    fn test_env4_clone() {
        let mut pos = Position::new();
        play_move(&mut pos, parse_coord("D4"));
        play_move(&mut pos, parse_coord("E4"));
        play_move(&mut pos, parse_coord("D5"));

        // Clone the position
        let mut cloned = pos.clone();
        assert!(env4_ok(&cloned), "cloned env4 inconsistent");

        // Play more moves on the clone
        play_move(&mut cloned, parse_coord("E5"));
        assert!(env4_ok(&cloned), "cloned env4 inconsistent after more moves");

        // Original should be unchanged
        assert!(env4_ok(&pos), "original env4 affected by clone");
    }

    #[test]
    fn test_env4_playout_simulation() {
        use crate::constants::MAX_GAME_LEN;

        // Simulate what mcplayout does
        let mut pos = Position::new();
        let mut passes = 0;

        while passes < 2 && pos.n < MAX_GAME_LEN {
            let mut found_move = false;
            for pt in BOARD_IMIN..BOARD_IMAX {
                if pos.color[pt] != EMPTY {
                    continue;
                }
                if is_eye(&pos, pt) == b'X' {
                    continue;
                }
                if play_move(&mut pos, pt).is_empty() {
                    // Move succeeded
                    assert!(env4_ok(&pos), "env4 inconsistent after move at {} (n={})", pt, pos.n);
                    found_move = true;
                    break;
                }
            }

            if found_move {
                passes = 0;
            } else {
                pass_move(&mut pos);
                passes += 1;
            }
        }

        assert!(env4_ok(&pos), "env4 inconsistent after playout simulation");
    }
}

