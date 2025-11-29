//! Monte Carlo Tree Search (MCTS) implementation with RAVE.
//!
//! This module implements MCTS with:
//! - UCB1-RAVE for node selection (combining UCB with All-Moves-As-First heuristic)
//! - Progressive widening for tree expansion
//! - Simple random playouts for value estimation
//!
//! The search maintains a tree where each node represents a game position.
//! The tree is expanded incrementally, and leaf nodes are evaluated using playouts.

use crate::constants::{
    BOARD_IMAX, BOARD_IMIN, BOARDSIZE, EXPAND_VISITS, PASS_MOVE, PRIOR_EVEN, RAVE_EQUIV,
};
use crate::playout::mcplayout;
use crate::position::{Position, is_eye, pass_move, play_move, str_coord};

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
pub fn expand(node: &mut TreeNode) {
    if !node.children.is_empty() {
        return;
    }

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
            node.children.push(TreeNode::new(&child_pos));
        }
    }

    // Always allow passing if no other moves
    if node.children.is_empty() {
        let mut child_pos = node.pos.clone();
        pass_move(&mut child_pos);
        node.children.push(TreeNode::new(&child_pos));
    }
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
pub fn tree_search(root: &mut TreeNode, sims: usize) -> usize {
    // Initialize root if necessary
    if root.children.is_empty() {
        expand(root);
    }

    for _ in 0..sims {
        let mut amaf_map = vec![0i8; BOARDSIZE];

        // Descend to a leaf
        let path = tree_descend(root, &mut amaf_map);

        // Get position at the leaf and run a playout
        let mut pos = get_leaf_position(root, &path);
        let score = mcplayout(&mut pos);

        // Update tree with the result
        tree_update(root, &path, &amaf_map, score);
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
