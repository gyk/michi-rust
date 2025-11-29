use crate::constants::PASS_MOVE;
use crate::position::{Position, pass_move, play_move, str_coord};

pub struct TreeNode {
    pub pos: Position,
    pub v: u32,
    pub w: u32,
    pub pv: u32,
    pub pw: u32,
    pub av: u32,
    pub aw: u32,
    pub children: Vec<TreeNode>,
}

pub fn new_tree_node(pos: &Position) -> TreeNode {
    TreeNode {
        pos: pos.clone(),
        v: 0,
        w: 0,
        pv: 10,
        pw: 5,
        av: 0,
        aw: 0,
        children: Vec::new(),
    }
}

pub fn expand(node: &mut TreeNode) {
    if !node.children.is_empty() {
        return;
    }
    for pt in 0..node.pos.color.len() {
        if node.pos.color[pt] == b'.' {
            let mut p = node.pos.clone();
            let ret = play_move(&mut p, pt);
            if ret.is_empty() && p.last == pt {
                node.children.push(new_tree_node(&p));
            }
        }
    }
    if node.children.is_empty() {
        let mut p = node.pos.clone();
        pass_move(&mut p);
        node.children.push(new_tree_node(&p));
    }
}

pub fn tree_search(root: &mut TreeNode, sims: usize) -> usize {
    if root.children.is_empty() {
        expand(root);
    }
    for _ in 0..sims {
        for child in &mut root.children {
            child.v += 1;
            child.w += 1;
        }
    }
    let mut best = PASS_MOVE;
    if let Some(c) = root.children.iter().max_by_key(|c| c.v) {
        best = c.pos.last;
    }
    // resign heuristic placeholder
    if best == PASS_MOVE {
        return PASS_MOVE;
    }
    best
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
