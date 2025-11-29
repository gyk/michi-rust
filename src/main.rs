//! Michi-Rust: A minimalistic Go engine.
//!
//! This is a Rust reimplementation of Michi, originally written in Python
//! and later ported to C.
//!
//! Run with `cargo run` for a simple demonstration.

use michi_rust::board::{Board, Color};
use michi_rust::mcts::TreeNode;
use michi_rust::position::{Position, str_coord};

fn main() {
    println!("Michi-Rust: Minimalistic Go MCTS Engine\n");

    // Demo 1: Simple 2D board
    println!("=== 2D Board Demo ===");
    let mut board = Board::new(9);
    let r1 = board.play(2, 2, Color::Black);
    let r2 = board.play(6, 6, Color::White);
    println!("Black at (2,2): {:?}", r1);
    println!("White at (6,6): {:?}", r2);
    println!("{board}");

    // Demo 2: Position with MCTS
    println!("=== MCTS Demo ===");
    let pos = Position::new();
    let mut root = TreeNode::new(&pos);

    println!("Running 100 MCTS simulations...");
    let best_move = michi_rust::mcts::tree_search(&mut root, 100);
    println!("Best move: {}", str_coord(best_move));
    println!("Root winrate: {:.1}%", root.winrate() * 100.0);
}
