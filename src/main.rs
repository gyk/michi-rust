//! Michi-Rust: A minimalistic Go engine.
//!
//! This is a Rust reimplementation of Michi, originally written in Python
//! and later ported to C.
//!
//! ## Usage
//!
//! - `michi-rust` - Show a demo
//! - `michi-rust gtp` - Start GTP server for GUI integration
//! - `michi-rust demo` - Run the MCTS demo

use clap::{Parser, Subcommand};

use michi_rust::board::{Board, Color};
use michi_rust::gtp::GtpEngine;
use michi_rust::mcts::TreeNode;
use michi_rust::position::{str_coord, Position};

/// Michi-Rust: A minimalistic Go MCTS engine
#[derive(Parser)]
#[command(name = "michi-rust")]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the GTP (Go Text Protocol) server for use with GUI applications
    Gtp,
    /// Run a simple demo of the engine
    Demo,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Gtp) => {
            // Run GTP server
            let mut engine = GtpEngine::new();
            engine.run();
        }
        Some(Commands::Demo) | None => {
            // Run demo
            run_demo();
        }
    }
}

fn run_demo() {
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
