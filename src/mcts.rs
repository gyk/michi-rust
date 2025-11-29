use crate::constants::{BOARD_IMAX, BOARD_IMIN, EXPAND_VISITS, PASS_MOVE, PRIOR_EVEN, RAVE_EQUIV};
use crate::playout::mcplayout;
use crate::position::{Position, is_eye, pass_move, play_move, str_coord};

pub struct TreeNode {
    pub pos: Position,
    pub v: u32,  // number of visits
    pub w: u32,  // number of wins (expected reward is w/v)
    pub pv: u32, // prior visits
    pub pw: u32, // prior wins (node value = w/v + pw/pv)
    pub av: u32, // AMAF visits
    pub aw: u32, // AMAF wins
    pub children: Vec<TreeNode>,
}

impl Default for TreeNode {
    fn default() -> Self {
        Self::new(&Position::new())
    }
}

impl TreeNode {
    pub fn new(pos: &Position) -> Self {
        TreeNode {
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
}

pub fn new_tree_node(pos: &Position) -> TreeNode {
    TreeNode::new(pos)
}

pub fn expand(node: &mut TreeNode) {
    if !node.children.is_empty() {
        return;
    }

    // Generate all legal moves
    for pt in BOARD_IMIN..BOARD_IMAX {
        if node.pos.color[pt] != b'.' {
            continue;
        }
        // Skip true eyes for current player
        if is_eye(&node.pos, pt) == b'X' {
            continue;
        }

        let mut p = node.pos.clone();
        let ret = play_move(&mut p, pt);
        if ret.is_empty() {
            node.children.push(TreeNode::new(&p));
        }
    }

    // If no moves available, add pass
    if node.children.is_empty() {
        let mut p = node.pos.clone();
        pass_move(&mut p);
        node.children.push(TreeNode::new(&p));
    }
}

/// Compute RAVE urgency for node selection
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

/// Select the most urgent child using RAVE/UCB policy
fn most_urgent(children: &mut [TreeNode]) -> usize {
    let mut best_idx = 0;
    let mut best_urgency = f64::NEG_INFINITY;

    for (i, child) in children.iter().enumerate() {
        let urgency = rave_urgency(child);
        if urgency > best_urgency {
            best_urgency = urgency;
            best_idx = i;
        }
    }

    best_idx
}

/// Descend through the tree to a leaf, returning the path of indices
fn tree_descend(tree: &mut TreeNode, amaf_map: &mut [i8]) -> Vec<usize> {
    let mut path = Vec::new();
    let mut node = tree;
    let mut passes = 0;

    loop {
        if node.children.is_empty() || passes >= 2 {
            break;
        }

        let child_idx = most_urgent(&mut node.children);
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

/// Update tree statistics after a playout
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

/// Get the leaf position from the tree following the given path
fn get_leaf_position(tree: &TreeNode, path: &[usize]) -> Position {
    let mut node = tree;
    for &idx in path {
        node = &node.children[idx];
    }
    node.pos.clone()
}

pub fn tree_search(root: &mut TreeNode, sims: usize) -> usize {
    use crate::constants::BOARDSIZE;

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

/// Find the most visited child (best move)
fn best_move(tree: &TreeNode) -> usize {
    if tree.children.is_empty() {
        return PASS_MOVE;
    }

    let mut best_idx = 0;
    let mut best_visits = 0;

    for (i, child) in tree.children.iter().enumerate() {
        if child.v > best_visits {
            best_visits = child.v;
            best_idx = i;
        }
    }

    tree.children[best_idx].pos.last
}

pub fn winrate(node: &TreeNode) -> f64 {
    if node.v > 0 {
        node.w as f64 / node.v as f64
    } else {
        -0.1
    }
}

pub fn dump_children(root: &TreeNode) {
    for c in &root.children {
        eprintln!(
            "move {} v={} w={} wr={:.3}",
            str_coord(c.pos.last),
            c.v,
            c.w,
            winrate(c)
        );
    }
}
