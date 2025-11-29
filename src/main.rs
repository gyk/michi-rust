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
//! - `michi-rust gtp --patterns michi-c` - Load patterns from michi-c folder

use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

use michi_rust::board::{Board, Color};
use michi_rust::gtp::GtpEngine;
use michi_rust::mcts::TreeNode;
use michi_rust::patterns::{load_large_patterns, load_large_patterns_from};
use michi_rust::position::{str_coord, Position};

/// Predefined intelligence levels
#[derive(Debug, Clone, Copy, ValueEnum)]
enum Level {
    /// Very weak: 10 simulations (for testing)
    Weak,
    /// Medium strength: 500 simulations
    Medium,
    /// Strong: 1400 simulations (default)
    Strong,
    /// Very strong: 5000 simulations
    VeryStrong,
    /// Maximum: 20000 simulations (slow but strongest)
    Max,
}

impl Level {
    fn to_sims(self) -> usize {
        match self {
            Level::Weak => 10,
            Level::Medium => 500,
            Level::Strong => 1400,
            Level::VeryStrong => 5000,
            Level::Max => 20000,
        }
    }
}

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
    Gtp {
        /// Number of MCTS simulations per move (higher = stronger but slower)
        #[arg(short = 's', long, default_value_t = 1400)]
        simulations: usize,

        /// Predefined intelligence level (overrides --simulations if set)
        #[arg(short = 'l', long, value_enum)]
        level: Option<Level>,

        /// Directory containing patterns.prob and patterns.spat files
        #[arg(short = 'p', long)]
        patterns: Option<PathBuf>,
    },
    /// Run a simple demo of the engine
    Demo {
        /// Directory containing patterns.prob and patterns.spat files
        #[arg(short = 'p', long)]
        patterns: Option<PathBuf>,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Gtp { simulations, level, patterns }) => {
            // Load patterns if specified
            load_patterns_from_arg(&patterns);

            // Determine number of simulations
            let n_sims = if let Some(lvl) = level {
                lvl.to_sims()
            } else {
                simulations
            };

            eprintln!("michi-rust: Starting GTP with {} simulations per move", n_sims);

            // Run GTP server
            let mut engine = GtpEngine::with_simulations(n_sims);
            engine.run();
        }
        Some(Commands::Demo { patterns }) => {
            load_patterns_from_arg(&patterns);
            run_demo();
        }
        None => {
            // Try to auto-load patterns from common locations
            let _ = load_large_patterns();
            run_demo();
        }
    }
}

/// Load pattern files from the specified directory or try default locations.
fn load_patterns_from_arg(patterns: &Option<PathBuf>) {
    if let Some(dir) = patterns {
        let prob_path = dir.join("patterns.prob");
        let spat_path = dir.join("patterns.spat");

        match load_large_patterns_from(&prob_path, &spat_path) {
            Ok(n) => eprintln!("michi-rust: Loaded {} large patterns from {:?}", n, dir),
            Err(e) => eprintln!("michi-rust: Warning: Could not load patterns: {}", e),
        }
    } else {
        // Try default locations
        match load_large_patterns() {
            Ok(n) => eprintln!("michi-rust: Loaded {} large patterns", n),
            Err(_) => eprintln!("michi-rust: Running without large patterns (use --patterns to specify)"),
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
    let mut pos = Position::new();
    println!("Initial position:");
    println!("{pos}");

    // Play a few moves to show the board display
    michi_rust::position::play_move(&mut pos, michi_rust::position::parse_coord("D4"));
    michi_rust::position::play_move(&mut pos, michi_rust::position::parse_coord("F6"));
    michi_rust::position::play_move(&mut pos, michi_rust::position::parse_coord("E5"));
    println!("After 3 moves:");
    println!("{pos}");

    // Run MCTS
    let mut root = TreeNode::new(&pos);
    println!("Running 100 MCTS simulations...");
    let best_move = michi_rust::mcts::tree_search(&mut root, 100);
    println!("Best move: {}", str_coord(best_move));
    println!("Root winrate: {:.1}%", root.winrate() * 100.0);
}
