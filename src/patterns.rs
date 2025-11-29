//! Pattern matching for Go move generation.
//!
//! This module implements pattern-based move heuristics from the C code:
//!
//! ## 3x3 Patterns (`pat3`)
//! Fast pattern matching using the 8 neighbors encoded into a lookup table.
//! Used for both playout move generation and MCTS priors.
//!
//! The patterns are stored in a 8192-byte bitfield (`PAT3SET`), where each bit
//! corresponds to a possible 16-bit encoding of the 8 neighbors (env8).
//!
//! ## Large Patterns
//! Larger patterns (up to ~17 points) loaded from pattern files.
//! These provide probability estimates for how likely a move is to be good.
//! (Not yet implemented)

use crate::position::{Point, Position};
use std::sync::OnceLock;

/// The 3x3 pattern source definitions from michi-c.
/// Each pattern is a 9-character string representing a 3x3 grid:
/// - X: current player (BLACK or WHITE depending on turn)
/// - O: opponent
/// - .: empty
/// - x: not X (i.e., O or . or #)
/// - o: not O (i.e., X or . or #)
/// - ?: any (X, O, ., or #)
/// - #: edge of board (out of bounds)
const PAT3_SRC: &[&str] = &[
    // 1- hane pattern - enclosing hane
    "XOX...???",
    // 2- hane pattern - non-cutting hane
    "XO....?.?",
    // 3- hane pattern - magari
    "XO?X..x.?",
    // 4- generic pattern - katatsuke or diagonal attachment
    ".O.X.....",
    // 5- cut1 pattern (kiri) - unprotected cut
    "XO?O.o?o?",
    // 6- cut1 pattern (kiri) - peeped cut
    "XO?O.X???",
    // 7- cut2 pattern (de)
    "?X?O.Oooo",
    // 8- cut keima
    "OX?o.O???",
    // 9- side pattern - chase
    "X.?O.?##?",
    // 10- side pattern - block side cut
    "OX?X.O###",
    // 11- side pattern - block side connection
    "?X?x.O###",
    // 12- side pattern - sagari
    "?XOx.x###",
    // 13- side pattern - cut
    "?OXX.O###",
];

/// Static storage for the pattern bitfield.
static PAT3SET: OnceLock<[u8; 8192]> = OnceLock::new();

/// Check if a point matches any 3x3 pattern.
///
/// Uses the precomputed pattern table for fast lookup.
/// The env4 and env4d fields encode the 8 neighbors, which are combined
/// into a 16-bit index for the lookup table.
#[inline]
pub fn pat3_match(pos: &Position, pt: Point) -> bool {
    let pat3set = PAT3SET.get_or_init(make_pat3set);

    // Combine env4 (orthogonal) and env4d (diagonal) into env8
    let env8 = (pos.env4[pt] as u16) | ((pos.env4d[pt] as u16) << 8);

    // Look up in the bitfield
    let byte_idx = (env8 >> 3) as usize;
    let bit_idx = (env8 & 7) as u8;

    (pat3set[byte_idx] & (1 << bit_idx)) != 0
}

/// Initialize pattern tables.
///
/// This is called automatically on first use of pat3_match.
pub fn init_patterns() {
    PAT3SET.get_or_init(make_pat3set);
}

/// Build the 3x3 pattern lookup table.
fn make_pat3set() -> [u8; 8192] {
    let mut pat3set = [0u8; 8192];

    for pat_src in PAT3_SRC {
        pat_enumerate(pat_src, &mut pat3set);
    }

    pat3set
}

/// Enumerate all rotations, reflections, and color swaps of a pattern.
fn pat_enumerate(src: &str, pat3set: &mut [u8; 8192]) {
    let mut src: [u8; 9] = src.as_bytes().try_into().unwrap();

    // Apply all symmetries
    pat_enumerate1(&src, pat3set);
    rot90(&mut src);
    pat_enumerate1(&src, pat3set);
}

fn pat_enumerate1(src: &[u8; 9], pat3set: &mut [u8; 8192]) {
    let mut src = *src;
    pat_enumerate2(&src, pat3set);
    vertflip(&mut src);
    pat_enumerate2(&src, pat3set);
}

fn pat_enumerate2(src: &[u8; 9], pat3set: &mut [u8; 8192]) {
    let mut src = *src;
    pat_enumerate3(&src, pat3set);
    horizflip(&mut src);
    pat_enumerate3(&src, pat3set);
}

fn pat_enumerate3(src: &[u8; 9], pat3set: &mut [u8; 8192]) {
    let mut src = *src;
    pat_wildexp(&src, 0, pat3set);
    swapcolor(&mut src);
    pat_wildexp(&src, 0, pat3set);
}

/// Expand wildcards and add all matching patterns to the set.
fn pat_wildexp(src: &[u8; 9], i: usize, pat3set: &mut [u8; 8192]) {
    if i == 9 {
        // All positions processed - compute env8 and set the bit
        let env8 = compute_code(src);
        let byte_idx = (env8 >> 3) as usize;
        let bit_idx = (env8 & 7) as u8;
        pat3set[byte_idx] |= 1 << bit_idx;
        return;
    }

    match src[i] {
        b'?' => {
            // Any of X, O, ., #
            for &c in &[b'X', b'O', b'.', b'#'] {
                let mut new_src = *src;
                new_src[i] = c;
                pat_wildexp(&new_src, i + 1, pat3set);
            }
        }
        b'x' => {
            // Not X (O, ., or #)
            for &c in &[b'O', b'.', b'#'] {
                let mut new_src = *src;
                new_src[i] = c;
                pat_wildexp(&new_src, i + 1, pat3set);
            }
        }
        b'o' => {
            // Not O (X, ., or #)
            for &c in &[b'X', b'.', b'#'] {
                let mut new_src = *src;
                new_src[i] = c;
                pat_wildexp(&new_src, i + 1, pat3set);
            }
        }
        _ => {
            // Fixed character - continue
            pat_wildexp(src, i + 1, pat3set);
        }
    }
}

/// Compute the 16-bit env8 code from a 9-character pattern string.
///
/// The pattern layout is:
/// ```text
/// 0 1 2     bits: 7 0 4
/// 3 4 5  ->       3 . 1
/// 6 7 8           6 2 5
/// ```
///
/// Low 8 bits = env4 (orthogonal neighbors)
/// High 8 bits = env4d (diagonal neighbors)
fn compute_code(src: &[u8; 9]) -> u16 {
    let mut env8: u16 = 0;

    // Orthogonal neighbors (env4)
    env8 |= code(src[1], 0);  // North
    env8 |= code(src[5], 1);  // East
    env8 |= code(src[7], 2);  // South
    env8 |= code(src[3], 3);  // West

    // Diagonal neighbors (env4d) - shifted to high byte
    env8 |= code(src[2], 0) << 8;  // NE
    env8 |= code(src[8], 1) << 8;  // SE
    env8 |= code(src[6], 2) << 8;  // SW
    env8 |= code(src[0], 3) << 8;  // NW

    env8
}

/// Encode a single neighbor color into the appropriate bit positions.
///
/// Color encoding:
/// - O (WHITE): 0
/// - X (BLACK): 1
/// - . (EMPTY): 2
/// - # (OUT): 3
///
/// Each neighbor uses 2 bits stored at positions p and p+4.
fn code(color: u8, p: u8) -> u16 {
    let c = match color {
        b'O' => 0,  // WHITE
        b'X' => 1,  // BLACK
        b'.' => 2,  // EMPTY
        b'#' => 3,  // OUT
        _ => 0,     // Shouldn't happen
    };

    let hi = (c >> 1) & 1;
    let lo = c & 1;
    ((hi << 4) | lo) << p
}

/// Swap X and O colors in a pattern.
fn swapcolor(src: &mut [u8; 9]) {
    for c in src.iter_mut() {
        *c = match *c {
            b'X' => b'O',
            b'O' => b'X',
            b'x' => b'o',
            b'o' => b'x',
            other => other,
        };
    }
}

/// Horizontal flip of a pattern.
fn horizflip(src: &mut [u8; 9]) {
    src.swap(0, 6);
    src.swap(1, 7);
    src.swap(2, 8);
}

/// Vertical flip of a pattern.
fn vertflip(src: &mut [u8; 9]) {
    src.swap(0, 2);
    src.swap(3, 5);
    src.swap(6, 8);
}

/// 90-degree rotation of a pattern.
fn rot90(src: &mut [u8; 9]) {
    let t = src[0];
    src[0] = src[2];
    src[2] = src[8];
    src[8] = src[6];
    src[6] = t;

    let t = src[1];
    src[1] = src[5];
    src[5] = src[7];
    src[7] = src[3];
    src[3] = t;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_make_pat3set() {
        let pat3set = make_pat3set();
        // The set should have some bits set
        let count: usize = pat3set.iter().map(|b| b.count_ones() as usize).sum();
        assert!(count > 0, "Pattern set should have some patterns");
        // Based on the C code, there should be many patterns
        assert!(count > 1000, "Expected many pattern matches, got {}", count);
    }

    #[test]
    fn test_compute_code_empty() {
        // All empty pattern
        let src = *b".........";
        let code = compute_code(&src);
        // All EMPTY (2) = bits 4,5,6,7 set for each position
        // env4: 0xF0, env4d: 0xF0 -> 0xF0F0
        assert_eq!(code, 0xF0F0);
    }

    #[test]
    fn test_pat3_match_hane() {
        use crate::position::{Position, play_move, parse_coord};

        // Set up a position where pattern #1 (hane) should match
        // Pattern: XOX / ... / ???
        // This is an enclosing hane pattern
        let mut pos = Position::new();

        // Play moves to create the pattern around D5
        // Black at C5, E5; White at D6
        play_move(&mut pos, parse_coord("C5")); // Black
        play_move(&mut pos, parse_coord("D6")); // White
        play_move(&mut pos, parse_coord("E5")); // Black

        // Now at D5, we should have:
        // North: White (D6)
        // East: Black (E5)
        // West: Black (C5)
        // South: Empty
        // This matches "XOX / ... / ???" rotated

        let pt = parse_coord("D5");
        let matches = pat3_match(&pos, pt);

        // Debug: print the env values
        eprintln!("env4[D5] = 0x{:02X}", pos.env4[pt]);
        eprintln!("env4d[D5] = 0x{:02X}", pos.env4d[pt]);

        assert!(matches, "Hane pattern should match at D5");
    }
}
