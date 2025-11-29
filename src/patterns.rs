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
//! Loaded from `patterns.prob` and `patterns.spat` files.

use crate::constants::N;
use crate::position::{Point, Position};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::sync::{OnceLock, RwLock};

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

// =============================================================================
// Large Pattern Support
// =============================================================================

/// Zobrist hash type (64 bits).
pub type ZobristHash = u64;

/// Hash table key size in bits.
const KSIZE: usize = 25;

/// Hash table length (2^KSIZE).
const HASHTABLE_LENGTH: usize = 1 << KSIZE;

/// Mask for extracting key from hash.
const KMASK: usize = HASHTABLE_LENGTH - 1;

/// Large board size with 7-layer border for pattern computation.
const LARGE_BOARDSIZE: usize = (N + 14) * (N + 7);

/// Maximum pattern neighborhood size (141 points).
const MAX_PATTERN_DIST: usize = 141;

/// Displacements for gridcular pattern neighborhoods.
/// Each entry is (x, y) offset from the center point.
#[rustfmt::skip]
const PAT_GRIDCULAR_SEQ: [(i32, i32); MAX_PATTERN_DIST] = [
    (0,0),      // d=1,2,3 is not considered separately                size 1
    (0,1), (0,-1), (1,0), (-1,0), (1,1), (-1,1), (1,-1), (-1,-1),
    (0,2), (0,-2), (2,0), (-2,0),                                   // size 2
    (1,2), (-1,2), (1,-2), (-1,-2), (2,1), (-2,1), (2,-1), (-2,-1), // size 3
    (0,3), (0,-3), (2,2), (-2,2), (2,-2), (-2,-2), (3,0), (-3,0),   // size 4
    (1,3), (-1,3), (1,-3), (-1,-3), (3,1), (-3,1), (3,-1), (-3,-1), // size 5
    (0,4), (0,-4), (2,3), (-2,3), (2,-3), (-2,-3), (3,2), (-3,2),   // size 6
    (3,-2), (-3,-2), (4,0), (-4,0),
    (1,4), (-1,4), (1,-4), (-1,-4), (3,3), (-3,3), (3,-3), (-3,-3), // size 7
    (4,1), (-4,1), (4,-1), (-4,-1),
    (0,5), (0,-5), (2,4), (-2,4), (2,-4), (-2,-4), (4,2), (-4,2),   // size 8
    (4,-2), (-4,-2), (5,0), (-5,0),
    (1,5), (-1,5), (1,-5), (-1,-5), (3,4), (-3,4), (3,-4), (-3,-4), // size 9
    (4,3), (-4,3), (4,-3), (-4,-3), (5,1), (-5,1), (5,-1), (-5,-1),
    (0,6), (0,-6), (2,5), (-2,5), (2,-5), (-2,-5), (4,4), (-4,4),   // size 10
    (4,-4), (-4,-4), (5,2), (-5,2), (5,-2), (-5,-2), (6,0), (-6,0),
    (1,6), (-1,6), (1,-6), (-1,-6), (3,5), (-3,5), (3,-5), (-3,-5), // size 11
    (5,3), (-5,3), (5,-3), (-5,-3), (6,1), (-6,1), (6,-1), (-6,-1),
    (0,7), (0,-7), (2,6), (-2,6), (2,-6), (-2,-6), (4,5), (-4,5),   // size 12
    (4,-5), (-4,-5), (5,4), (-5,4), (5,-4), (-5,-4), (6,2), (-6,2),
    (6,-2), (-6,-2), (7,0), (-7,0)
];

/// Cumulative sizes of gridcular neighborhoods.
/// pat_gridcular_size[s] = number of points in neighborhood of size s.
const PAT_GRIDCULAR_SIZE: [usize; 13] = [0, 9, 13, 21, 29, 37, 49, 61, 73, 89, 105, 121, 141];

/// Primes used for double hashing.
const PRIMES: [usize; 32] = [
    5, 11, 37, 103, 293, 991, 2903, 9931,
    7, 19, 73, 10009, 11149, 12553, 6229, 10181,
    1013, 1583, 2503, 3491, 4637, 5501, 6571, 7459,
    8513, 9433, 10433, 11447, 11887, 12409, 2221, 4073,
];

/// A large pattern entry in the hash table.
#[derive(Clone, Copy, Default)]
pub struct LargePat {
    /// 64-bit Zobrist hash key.
    pub key: ZobristHash,
    /// Pattern ID (from .spat file).
    pub id: u32,
    /// Probability of move triggered by this pattern.
    pub prob: f32,
}

/// Large pattern database.
pub struct LargePatternDb {
    /// Hash table for pattern lookup (double hashing).
    patterns: Vec<LargePat>,
    /// Zobrist hash random data [displacement][color].
    zobrist_hashdata: [[ZobristHash; 4]; MAX_PATTERN_DIST],
    /// Precomputed 1D offsets for gridcular sequence.
    gridcular_seq1d: [isize; MAX_PATTERN_DIST],
    /// Whether patterns were successfully loaded.
    pub loaded: bool,
}

impl Default for LargePatternDb {
    fn default() -> Self {
        Self::new()
    }
}

/// Global large pattern database instance.
static LARGE_PATTERN_DB: OnceLock<RwLock<LargePatternDb>> = OnceLock::new();

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

// =============================================================================
// Large Pattern Implementation
// =============================================================================

impl LargePatternDb {
    /// Create a new empty pattern database.
    pub fn new() -> Self {
        let mut db = Self {
            patterns: vec![LargePat::default(); HASHTABLE_LENGTH],
            zobrist_hashdata: [[0; 4]; MAX_PATTERN_DIST],
            gridcular_seq1d: [0; MAX_PATTERN_DIST],
            loaded: false,
        };
        db.init_zobrist_hashdata();
        db.init_gridcular();
        db
    }

    /// Initialize Zobrist hash random data.
    fn init_zobrist_hashdata(&mut self) {
        // Use a simple PRNG matching the C code
        let mut idum: u32 = 1;
        let mut qdrandom = || {
            idum = idum.wrapping_mul(1664525).wrapping_add(1013904223);
            idum
        };

        for d in 0..MAX_PATTERN_DIST {
            for c in 0..4 {
                let d1 = qdrandom() as u64;
                let d2 = qdrandom() as u64;
                self.zobrist_hashdata[d][c] = (d1 << 32) | d2;
            }
        }
    }

    /// Initialize gridcular 1D offsets.
    fn init_gridcular(&mut self) {
        let large_w = (N + 7) as isize;
        for i in 0..MAX_PATTERN_DIST {
            let (x, y) = PAT_GRIDCULAR_SEQ[i];
            self.gridcular_seq1d[i] = (x as isize) - (y as isize) * large_w;
        }
    }

    /// Map stone color to Zobrist color index.
    /// 0: EMPTY, 1: OUT, 2: Other/opponent, 3: current player
    #[inline]
    fn stone_color(c: u8) -> usize {
        match c {
            b'.' => 0,          // EMPTY
            b'#' | b' ' => 1,   // OUT
            b'O' | b'x' => 2,   // Other/opponent
            b'X' => 3,          // Current player
            _ => 0,
        }
    }

    /// Compute Zobrist hash for a pattern string.
    fn zobrist_hash(&self, pat: &[u8]) -> ZobristHash {
        let mut k: ZobristHash = 0;
        for (i, &c) in pat.iter().enumerate() {
            k ^= self.zobrist_hashdata[i][Self::stone_color(c)];
        }
        k
    }

    /// Find pattern in hash table using double hashing.
    /// Returns the index where the key is found or should be inserted.
    fn find_pat(&self, key: ZobristHash) -> usize {
        debug_assert!(key != 0);

        let mut h = ((key >> 20) as usize) & KMASK;
        let h2 = PRIMES[((key >> (20 + KSIZE)) as usize) & 15];

        while self.patterns[h].key != key {
            if self.patterns[h].key == 0 {
                return h;
            }
            h = (h + h2) % HASHTABLE_LENGTH;
        }
        h
    }

    /// Insert a pattern into the hash table.
    fn insert_pat(&mut self, pat: LargePat) -> bool {
        let i = self.find_pat(pat.key);
        if self.patterns[i].key == 0 {
            self.patterns[i] = pat;
            true
        } else {
            false // Already exists
        }
    }

    /// Load patterns from .prob and .spat files.
    pub fn load_patterns(&mut self, prob_path: &Path, spat_path: &Path) -> Result<usize, String> {
        // First, load probability file to get max id
        let prob_file = File::open(prob_path)
            .map_err(|e| format!("Cannot open prob file: {}", e))?;
        let reader = BufReader::new(prob_file);

        // Find max id and load probs
        let mut max_id: u32 = 0;
        let mut prob_entries = Vec::new();

        for line in reader.lines() {
            let line = line.map_err(|e| format!("Read error: {}", e))?;
            if line.starts_with('#') || line.is_empty() {
                continue;
            }
            // Format: "prob t1 t2 (s:id)"
            // Example: "1.000 2 2 (s:410926)"
            if let Some((prob, id)) = Self::parse_prob_line(&line) {
                if id > max_id {
                    max_id = id;
                }
                prob_entries.push((id, prob));
            }
        }

        // Create probs array
        let mut probs = vec![0.0f32; (max_id + 1) as usize];
        for (id, prob) in prob_entries {
            probs[id as usize] = prob;
        }

        // Now load spatial patterns
        let spat_file = File::open(spat_path)
            .map_err(|e| format!("Cannot open spat file: {}", e))?;
        let reader = BufReader::new(spat_file);

        // Compute the 8 permutations for rotations/reflections
        let permutations = self.compute_permutations();

        let mut npats = 0;
        for line in reader.lines() {
            let line = line.map_err(|e| format!("Read error: {}", e))?;
            if line.starts_with('#') || line.is_empty() {
                continue;
            }
            // Format: "id d pattern hash1 hash2 ..."
            // Example: "410926 5 .OOXXXX..O...XX...... bd31fe8 fad3be8 ..."
            if let Some((id, pat_str)) = Self::parse_spat_line(&line) {
                let prob = if (id as usize) < probs.len() {
                    probs[id as usize]
                } else {
                    0.0
                };

                // Insert all 8 rotations/reflections
                for perm in &permutations {
                    let permuted = self.permute_pattern(&pat_str, perm);
                    let key = self.zobrist_hash(&permuted);
                    if key != 0 {
                        self.insert_pat(LargePat { key, id, prob });
                    }
                }
                npats += 1;
            }
        }

        self.loaded = true;
        Ok(npats)
    }

    /// Parse a line from the .prob file.
    fn parse_prob_line(line: &str) -> Option<(f32, u32)> {
        // Format: "prob t1 t2 (s:id)"
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 4 {
            return None;
        }
        let prob: f32 = parts[0].parse().ok()?;
        // Extract id from "(s:id)"
        let id_part = parts[3];
        if !id_part.starts_with("(s:") || !id_part.ends_with(')') {
            return None;
        }
        let id: u32 = id_part[3..id_part.len() - 1].parse().ok()?;
        Some((prob, id))
    }

    /// Parse a line from the .spat file.
    fn parse_spat_line(line: &str) -> Option<(u32, Vec<u8>)> {
        // Format: "id d pattern hash1 hash2 ..."
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 3 {
            return None;
        }
        let id: u32 = parts[0].parse().ok()?;
        let pat_str = parts[2].as_bytes().to_vec();
        Some((id, pat_str))
    }

    /// Compute the 8 permutations for pattern rotations/reflections.
    fn compute_permutations(&self) -> Vec<Vec<usize>> {
        let large_w = (N + 7) as isize;
        let base_seq1d: Vec<isize> = PAT_GRIDCULAR_SEQ
            .iter()
            .map(|(x, y)| (*x as isize) - (*y as isize) * large_w)
            .collect();

        let gridcular_index = |disp: isize| -> usize {
            base_seq1d.iter().position(|&d| d == disp).unwrap_or(0)
        };

        let mut permutations = Vec::new();
        let mut seqs = vec![PAT_GRIDCULAR_SEQ.to_vec()];

        // Generate all 8 permutations (4 rotations x 2 reflections)
        // Horizontal flip
        let h_flip = |seq: &[(i32, i32)]| -> Vec<(i32, i32)> {
            seq.iter().map(|(x, y)| (*x, -*y)).collect()
        };
        // 90 degree rotation
        let rot = |seq: &[(i32, i32)]| -> Vec<(i32, i32)> {
            seq.iter().map(|(x, y)| (-*y, *x)).collect()
        };

        // Start with identity
        let mut current = PAT_GRIDCULAR_SEQ.to_vec();

        // Apply transformations to get all 8 permutations
        for _ in 0..2 {
            for _ in 0..4 {
                seqs.push(current.clone());
                current = rot(&current);
            }
            current = h_flip(&seqs[0]);
        }

        // Convert to index permutations
        for seq in seqs.iter().take(8) {
            let seq1d: Vec<isize> = seq
                .iter()
                .map(|(x, y)| (*x as isize) - (*y as isize) * large_w)
                .collect();
            let perm: Vec<usize> = seq1d.iter().map(|&d| gridcular_index(d)).collect();
            permutations.push(perm);
        }

        permutations
    }

    /// Apply a permutation to a pattern string.
    fn permute_pattern(&self, pat: &[u8], perm: &[usize]) -> Vec<u8> {
        let len = pat.len();
        let mut result = vec![b'.'; len];
        for k in 0..len {
            if perm[k] < len {
                result[k] = pat[perm[k]];
            }
        }
        result
    }

    /// Compute pattern probability at a point.
    /// Returns the probability from the largest matching pattern, or -1.0 if none.
    pub fn large_pattern_probability(&self, pos: &Position, pt: Point) -> f64 {
        if !self.loaded {
            return -1.0;
        }

        // Build large board representation for this point
        let large_board = self.build_large_board(pos);
        let large_pt = self.point_to_large_coord(pt);

        let mut prob = -1.0;
        let mut matched_len = 0;
        let mut non_matched_len = 0;
        let mut k: ZobristHash = 0;

        for s in 1..13 {
            let len = PAT_GRIDCULAR_SIZE[s];
            k = self.update_zobrist_hash(&large_board, large_pt, s, k);
            let i = self.find_pat(k);
            if self.patterns[i].key == k {
                prob = self.patterns[i].prob as f64;
                matched_len = len;
            } else if matched_len < non_matched_len && non_matched_len < len {
                break;
            } else {
                non_matched_len = len;
            }
        }

        prob
    }

    /// Build a large board representation with 7-layer border.
    fn build_large_board(&self, pos: &Position) -> Vec<u8> {
        let mut large_board = vec![b'#'; LARGE_BOARDSIZE];
        let large_w = N + 7;

        // Copy position to large board
        for y in 0..N {
            for x in 0..N {
                let pt = (y + 1) * (N + 1) + x + 1;
                let lpt = (y + 7) * large_w + x + 7;
                large_board[lpt] = pos.color[pt];
            }
        }

        large_board
    }

    /// Convert a board point to large board coordinate.
    fn point_to_large_coord(&self, pt: Point) -> usize {
        let y = pt / (N + 1) - 1;
        let x = pt % (N + 1) - 1;
        (y + 7) * (N + 7) + x + 7
    }

    /// Update Zobrist hash for points in a neighborhood size.
    fn update_zobrist_hash(
        &self,
        large_board: &[u8],
        pt: usize,
        size: usize,
        mut k: ZobristHash,
    ) -> ZobristHash {
        let imin = PAT_GRIDCULAR_SIZE[size - 1];
        let imax = PAT_GRIDCULAR_SIZE[size];

        for i in imin..imax {
            let offset = self.gridcular_seq1d[i];
            let lpt = (pt as isize + offset) as usize;
            let c = if lpt < large_board.len() {
                Self::stone_color(large_board[lpt])
            } else {
                1 // OUT
            };
            k ^= self.zobrist_hashdata[i][c];
        }

        k
    }
}

/// Initialize the global large pattern database.
pub fn init_large_patterns() {
    let _ = LARGE_PATTERN_DB.get_or_init(|| RwLock::new(LargePatternDb::new()));
}

/// Load large patterns from files.
/// Tries common paths: current directory, michi-c folder, tests folder.
pub fn load_large_patterns() -> Result<usize, String> {
    let db = LARGE_PATTERN_DB.get_or_init(|| RwLock::new(LargePatternDb::new()));
    let mut db = db.write().map_err(|e| format!("Lock error: {}", e))?;

    // Try different paths for pattern files
    let paths_to_try = [
        ("patterns.prob", "patterns.spat"),
        ("michi-c/patterns.prob", "michi-c/patterns.spat"),
        ("michi-c/tests/patterns.prob", "michi-c/tests/patterns.spat"),
    ];

    for (prob_path, spat_path) in &paths_to_try {
        let prob = Path::new(prob_path);
        let spat = Path::new(spat_path);
        if prob.exists() && spat.exists() {
            return db.load_patterns(prob, spat);
        }
    }

    Err("Pattern files not found".to_string())
}

/// Load large patterns from specific file paths.
pub fn load_large_patterns_from(prob_path: &Path, spat_path: &Path) -> Result<usize, String> {
    let db = LARGE_PATTERN_DB.get_or_init(|| RwLock::new(LargePatternDb::new()));
    let mut db = db.write().map_err(|e| format!("Lock error: {}", e))?;
    db.load_patterns(prob_path, spat_path)
}

/// Get the probability for a large pattern match at a point.
/// Returns -1.0 if no pattern matches or patterns not loaded.
pub fn large_pattern_probability(pos: &Position, pt: Point) -> f64 {
    let db = match LARGE_PATTERN_DB.get() {
        Some(db) => db,
        None => return -1.0,
    };
    let db = match db.read() {
        Ok(db) => db,
        Err(_) => return -1.0,
    };
    db.large_pattern_probability(pos, pt)
}

/// Check if large patterns are loaded.
pub fn large_patterns_loaded() -> bool {
    match LARGE_PATTERN_DB.get() {
        Some(db) => db.read().map(|d| d.loaded).unwrap_or(false),
        None => false,
    }
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

    #[test]
    fn test_large_pattern_db_init() {
        // Test that the database initializes correctly
        let db = LargePatternDb::new();
        assert!(!db.loaded);
        // Check that gridcular 1D offsets are computed
        assert_eq!(db.gridcular_seq1d[0], 0); // Center point
    }

    #[test]
    fn test_zobrist_hash_deterministic() {
        let db = LargePatternDb::new();
        let pat = b".X.O.X.O.";
        let hash1 = db.zobrist_hash(pat);
        let hash2 = db.zobrist_hash(pat);
        assert_eq!(hash1, hash2);
        assert_ne!(hash1, 0);
    }

    #[test]
    fn test_parse_prob_line() {
        let line = "1.000 2 2 (s:410926)";
        let result = LargePatternDb::parse_prob_line(line);
        assert!(result.is_some());
        let (prob, id) = result.unwrap();
        assert!((prob - 1.0).abs() < 0.001);
        assert_eq!(id, 410926);
    }

    #[test]
    fn test_parse_spat_line() {
        let line = "410926 5 .OOXXXX..O...XX...... bd31fe8 fad3be8";
        let result = LargePatternDb::parse_spat_line(line);
        assert!(result.is_some());
        let (id, pat) = result.unwrap();
        assert_eq!(id, 410926);
        assert_eq!(pat, b".OOXXXX..O...XX......");
    }

    #[test]
    fn test_load_test_patterns() {
        use std::path::Path;

        // Try to load the test patterns
        let prob_path = Path::new("michi-c/tests/patterns.prob");
        let spat_path = Path::new("michi-c/tests/patterns.spat");

        if prob_path.exists() && spat_path.exists() {
            let mut db = LargePatternDb::new();
            let result = db.load_patterns(prob_path, spat_path);
            assert!(result.is_ok(), "Failed to load patterns: {:?}", result);
            assert!(db.loaded);
            let npats = result.unwrap();
            assert!(npats > 0, "Should load some patterns");
            eprintln!("Loaded {} patterns from test files", npats);
        } else {
            eprintln!("Skipping test_load_test_patterns: pattern files not found");
        }
    }

    #[test]
    fn test_large_pattern_not_loaded() {
        use crate::position::Position;

        // Without loading patterns, probability should be -1.0
        let pos = Position::new();
        let db = LargePatternDb::new();
        let prob = db.large_pattern_probability(&pos, 45); // Some point
        assert!(prob < 0.0);
    }

    #[test]
    fn test_stone_color_mapping() {
        assert_eq!(LargePatternDb::stone_color(b'.'), 0); // EMPTY
        assert_eq!(LargePatternDb::stone_color(b'#'), 1); // OUT
        assert_eq!(LargePatternDb::stone_color(b' '), 1); // OUT
        assert_eq!(LargePatternDb::stone_color(b'O'), 2); // Other
        assert_eq!(LargePatternDb::stone_color(b'x'), 2); // Other
        assert_eq!(LargePatternDb::stone_color(b'X'), 3); // Current player
    }
}
