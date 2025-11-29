//! Monte Carlo Tree Search (MCTS) implementation with RAVE.
//!
//! This module implements MCTS with:
//! - UCB1-RAVE for node selection (combining UCB with All-Moves-As-First heuristic)
//! - Progressive widening for tree expansion
//! - Pattern-based priors for move prioritization
//! - Simple random playouts for value estimation
//!
//! The search maintains a tree where each node represents a game position.
//! The tree is expanded incrementally, and leaf nodes are evaluated using playouts.

use crate::constants::{
    BOARD_IMAX, BOARD_IMIN, BOARDSIZE, EXPAND_VISITS, N, PASS_MOVE, PRIOR_CFG,
    PRIOR_CAPTURE_MANY, PRIOR_CAPTURE_ONE, PRIOR_EMPTYAREA, PRIOR_EVEN, PRIOR_LARGEPATTERN,
    PRIOR_PAT3, PRIOR_SELFATARI, RAVE_EQUIV, W, EMPTY, OUT,
};
use crate::patterns::{large_pattern_probability, pat3_match};
use crate::playout::mcplayout;
use crate::position::{
    all_neighbors, fix_atari, gen_capture_moves, is_eye, pass_move, play_move, str_coord,
    Point, Position,
};

/// A node in the MCTS search tree.
///
/// Each node stores statistics for both regular visits (v, w) and AMAF visits (av, aw),
/// as well as prior values (pv, pw) for initialization.
pub struct TreeNode {
    /// The game position at this node
    pub pos: Position,
    /// Number of visits
    pub v: u32,
    /// Number of wins (winrate = w/v)
    pub w: u32,
    /// Prior visits (for initialization)
    pub pv: u32,
    /// Prior wins
    pub pw: u32,
    /// AMAF (All Moves As First) visits
    pub av: u32,
    /// AMAF wins
    pub aw: u32,
    /// Child nodes (one per legal move)
    pub children: Vec<TreeNode>,
}

impl Default for TreeNode {
    fn default() -> Self {
        Self::new(&Position::new())
    }
}

impl TreeNode {
    /// Create a new tree node for the given position.
    pub fn new(pos: &Position) -> Self {
        Self {
            pos: pos.clone(),
            v: 0,
            w: 0,
            pv: PRIOR_EVEN,
            pw: PRIOR_EVEN / 2,
            av: 0,
            aw: 0,
            children: Vec::new(),
        }
    }

    /// Calculate the winrate for this node.
    #[inline]
    pub fn winrate(&self) -> f64 {
        if self.v > 0 {
            self.w as f64 / self.v as f64
        } else {
            -0.1 // Indicate unvisited
        }
    }
}

/// Expand a node by generating all legal child moves.
///
/// Each legal move becomes a child node. If no moves are available,
/// a pass move is added.
///
/// Applies priors based on:
/// - Capture moves (PRIOR_CAPTURE_ONE, PRIOR_CAPTURE_MANY)
/// - 3x3 patterns (PRIOR_PAT3)
/// - CFG distance from last move (PRIOR_CFG)
/// - Self-atari detection (PRIOR_SELFATARI as negative prior)
pub fn expand(node: &mut TreeNode) {
    if !node.children.is_empty() {
        return;
    }

    // Compute CFG distances from last move
    let cfg_map = if node.pos.last != PASS_MOVE {
        Some(compute_cfg_distances(&node.pos, node.pos.last))
    } else {
        None
    };

    // Generate all legal moves
    for pt in BOARD_IMIN..BOARD_IMAX {
        if node.pos.color[pt] != b'.' {
            continue;
        }
        // Skip true eyes for current player (never a good move)
        if is_eye(&node.pos, pt) == b'X' {
            continue;
        }

        let mut child_pos = node.pos.clone();
        if play_move(&mut child_pos, pt).is_empty() {
            let mut child = TreeNode::new(&child_pos);

            // Apply priors
            apply_priors(&mut child, &node.pos, pt, &cfg_map);

            node.children.push(child);
        }
    }

    // Always allow passing if no other moves
    if node.children.is_empty() {
        let mut child_pos = node.pos.clone();
        pass_move(&mut child_pos);
        node.children.push(TreeNode::new(&child_pos));
    }
}

/// Apply priors to a child node based on various heuristics.
fn apply_priors(child: &mut TreeNode, parent_pos: &Position, pt: Point, cfg_map: &Option<[i8; BOARDSIZE]>) {
    // 1. CFG distance prior - moves near the last move get a bonus
    if let Some(cfg) = cfg_map {
        let dist = cfg[pt];
        if dist >= 1 && (dist as usize) <= PRIOR_CFG.len() {
            let bonus = PRIOR_CFG[(dist - 1) as usize];
            child.pv += bonus;
            child.pw += bonus;
        }
    }

    // 2. 3x3 pattern prior
    if pat3_match(parent_pos, pt) {
        child.pv += PRIOR_PAT3;
        child.pw += PRIOR_PAT3;
    }

    // 3. Large pattern prior - use probability from pattern database
    let pattern_prob = large_pattern_probability(parent_pos, pt);
    if pattern_prob >= 0.0 {
        let pattern_prior = pattern_prob as u32;
        child.pv += pattern_prior * PRIOR_LARGEPATTERN;
        child.pw += pattern_prior * PRIOR_LARGEPATTERN;
    }

    // 4. Capture prior - check if this move captures or saves stones
    let capture_moves = gen_capture_moves(parent_pos);
    for (mv, size) in capture_moves {
        if mv == pt {
            if size == 1 {
                child.pv += PRIOR_CAPTURE_ONE;
                child.pw += PRIOR_CAPTURE_ONE;
            } else {
                child.pv += PRIOR_CAPTURE_MANY;
                child.pw += PRIOR_CAPTURE_MANY;
            }
            break;
        }
    }

    // 5. Self-atari prior (negative) - penalize moves that put us in atari
    let atari_moves = fix_atari(&child.pos, pt, true);
    if !atari_moves.is_empty() {
        child.pv += PRIOR_SELFATARI;
        // pw stays at pw, giving a lower winrate
    }

    // 6. Empty area prior - penalize moves on 1st/2nd line with no stones nearby
    let height = line_height(pt);
    if height <= 2 && empty_area(parent_pos, pt, 3) {
        child.pv += PRIOR_EMPTYAREA;
        if height == 2 {
            // 3rd line is OK in empty areas
            child.pw += PRIOR_EMPTYAREA;
        }
        // 1st/2nd line in empty area gets no pw bonus (negative prior)
    }
}

/// Compute CFG (Common Fate Graph) distances from a given point.
///
/// CFG distance is like Manhattan distance but groups of same-colored stones
/// count as distance 0 from each other.
fn compute_cfg_distances(pos: &Position, start: Point) -> [i8; BOARDSIZE] {
    let mut cfg_map = [-1i8; BOARDSIZE];
    let mut queue = Vec::with_capacity(BOARDSIZE);

    cfg_map[start] = 0;
    queue.push(start);
    let mut head = 0;

    while head < queue.len() {
        let pt = queue[head];
        head += 1;

        for n in all_neighbors(pt) {
            let c = pos.color[n];
            if c == OUT {
                continue;
            }

            let old_dist = cfg_map[n];
            let new_dist = if c != EMPTY && c == pos.color[pt] {
                // Same color stone - distance doesn't increase
                cfg_map[pt]
            } else {
                cfg_map[pt] + 1
            };

            if old_dist < 0 || new_dist < old_dist {
                cfg_map[n] = new_dist;
                queue.push(n);
            }
        }
    }

    cfg_map
}

/// Return the line number (0-indexed) from nearest board edge.
fn line_height(pt: Point) -> usize {
    let row = pt / W;
    let col = pt % W;

    // Distance from edges
    let row_dist = row.min(N + 1 - row);
    let col_dist = col.min(N + 1 - col);

    row_dist.min(col_dist).saturating_sub(1)
}

/// Check if there are no stones within Manhattan distance `dist` of point.
fn empty_area(pos: &Position, pt: Point, dist: usize) -> bool {
    if dist == 0 {
        return true;
    }

    for n in all_neighbors(pt) {
        let c = pos.color[n];
        if c == b'X' || c == b'x' {
            return false;
        }
        if c == EMPTY && dist > 1 && !empty_area(pos, n, dist - 1) {
            return false;
        }
    }

    true
}

/// Compute the RAVE-UCB urgency score for node selection.
///
/// Combines the node's empirical winrate with AMAF (All Moves As First) statistics.
/// The balance between empirical and AMAF is controlled by the beta parameter,
/// which decreases as the node gets more visits.
fn rave_urgency(node: &TreeNode) -> f64 {
    let v = (node.v + node.pv) as f64;
    let expectation = (node.w + node.pw) as f64 / v;

    if node.av == 0 {
        return expectation;
    }

    let rave_expectation = node.aw as f64 / node.av as f64;
    let beta = node.av as f64 / (node.av as f64 + v + v * node.av as f64 / RAVE_EQUIV as f64);
    beta * rave_expectation + (1.0 - beta) * expectation
}

/// Select the child with the highest urgency score.
fn most_urgent(children: &[TreeNode]) -> usize {
    children
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| {
            rave_urgency(a)
                .partial_cmp(&rave_urgency(b))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(i, _)| i)
        .unwrap_or(0)
}

/// Descend through the tree to a leaf node, recording the path taken.
///
/// Returns the path of child indices from root to leaf.
/// Updates the AMAF map with moves played during descent.
fn tree_descend(tree: &mut TreeNode, amaf_map: &mut [i8]) -> Vec<usize> {
    let mut path = Vec::new();
    let mut node = tree;
    let mut passes = 0;

    loop {
        if node.children.is_empty() || passes >= 2 {
            break;
        }

        let child_idx = most_urgent(&node.children);
        path.push(child_idx);

        let child = &node.children[child_idx];
        let mv = child.pos.last;

        if mv == PASS_MOVE {
            passes += 1;
        } else {
            passes = 0;
            if amaf_map[mv] == 0 {
                // Mark with 1 for black, -1 for white
                amaf_map[mv] = if node.pos.n % 2 == 0 { 1 } else { -1 };
            }
        }

        // Expand if this node has enough visits
        {
            let child = &mut node.children[child_idx];
            if child.children.is_empty() && child.v >= EXPAND_VISITS {
                expand(child);
            }
        }

        // Move to child (we need to reborrow)
        node = &mut node.children[child_idx];
    }

    path
}

/// Update tree statistics after a playout.
///
/// Propagates the playout result back up the tree, updating visit and win counts.
/// Also updates AMAF statistics for sibling moves that appeared in the playout.
fn tree_update(tree: &mut TreeNode, path: &[usize], amaf_map: &[i8], mut score: f64) {
    // Update root
    tree.v += 1;
    if score < 0.0 {
        tree.w += 1;
    }

    // Update AMAF for root's children
    let amaf_value = if tree.pos.n % 2 == 0 { 1i8 } else { -1i8 };
    for child in &mut tree.children {
        if child.pos.last != 0 && amaf_map[child.pos.last] == amaf_value {
            child.av += 1;
            if score > 0.0 {
                child.aw += 1;
            }
        }
    }

    score = -score;

    // Walk down the path updating nodes
    let mut node = tree;
    for &idx in path {
        node = &mut node.children[idx];
        node.v += 1;
        if score < 0.0 {
            node.w += 1;
        }

        // Update AMAF for this node's children
        let amaf_value = if node.pos.n % 2 == 0 { 1i8 } else { -1i8 };
        for child in &mut node.children {
            if child.pos.last != 0 && amaf_map[child.pos.last] == amaf_value {
                child.av += 1;
                if score > 0.0 {
                    child.aw += 1;
                }
            }
        }

        score = -score;
    }
}

/// Get the position at the leaf node reached by following the given path.
fn get_leaf_position(tree: &TreeNode, path: &[usize]) -> Position {
    path.iter()
        .fold(tree, |node, &idx| &node.children[idx])
        .pos
        .clone()
}

/// Run MCTS search from the given root position.
///
/// Performs the specified number of simulations and returns the best move found.
/// The best move is the most-visited child of the root.
///
/// Includes early stopping: if the best move has a very high winrate early
/// in the search, we stop early to save time.
pub fn tree_search(root: &mut TreeNode, sims: usize) -> usize {
    use crate::constants::{FASTPLAY5_THRES, FASTPLAY20_THRES};

    // Initialize root if necessary
    if root.children.is_empty() {
        expand(root);
    }

    for i in 0..sims {
        let mut amaf_map = vec![0i8; BOARDSIZE];

        // Descend to a leaf
        let path = tree_descend(root, &mut amaf_map);

        // Get position at the leaf and run a playout
        let mut pos = get_leaf_position(root, &path);
        let score = mcplayout(&mut pos, Some(&mut amaf_map));

        // Update tree with the result
        tree_update(root, &path, &amaf_map, score);

        // Early stop test (same as michi-c)
        // If best move has very high winrate, stop early
        let best_wr = root
            .children
            .iter()
            .filter(|c| c.v > 0)
            .map(|c| c.winrate())
            .fold(0.0_f64, f64::max);

        if (i > sims / 20 && best_wr > FASTPLAY5_THRES)
            || (i > sims / 5 && best_wr > FASTPLAY20_THRES)
        {
            break;
        }
    }

    // Return the best move (most visited child)
    best_move(root)
}

/// Find the best move (most visited child).
fn best_move(tree: &TreeNode) -> usize {
    tree.children
        .iter()
        .max_by_key(|c| c.v)
        .map(|c| c.pos.last)
        .unwrap_or(PASS_MOVE)
}

/// Calculate the winrate for a node.
///
/// Returns -0.1 for unvisited nodes to indicate they haven't been explored.
#[deprecated(note = "Use TreeNode::winrate() method instead")]
pub fn winrate(node: &TreeNode) -> f64 {
    node.winrate()
}

/// Print debug information about the root's children.
pub fn dump_children(root: &TreeNode) {
    for child in &root.children {
        eprintln!(
            "move {} v={} w={} wr={:.3}",
            str_coord(child.pos.last),
            child.v,
            child.w,
            child.winrate()
        );
    }
}
