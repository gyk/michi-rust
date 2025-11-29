//! Michi-Rust: A minimalistic Go MCTS engine.
//!
//! This crate provides a Monte Carlo Tree Search (MCTS) based Go engine,
//! reimplemented in Rust from the original Michi project.
//!
//! ## Modules
//!
//! - [`constants`] - Board dimensions and engine parameters
//! - [`position`] - Core game logic (board state, moves, captures)
//! - [`mcts`] - Monte Carlo Tree Search with RAVE
//! - [`playout`] - Random game simulation for position evaluation
//! - [`patterns`] - Pattern matching (partially implemented)
//! - [`board`] - Alternative 2D board representation
//!
//! ## Example
//!
//! ```
//! use michi_rust::position::{Position, play_move, parse_coord, str_coord};
//! use michi_rust::mcts::{TreeNode, tree_search};
//!
//! // Create a new game
//! let mut pos = Position::new();
//!
//! // Play a move
//! play_move(&mut pos, parse_coord("D4"));
//!
//! // Run MCTS to find the best response
//! let mut root = TreeNode::new(&pos);
//! let best = tree_search(&mut root, 100);
//! println!("Best move: {}", str_coord(best));
//! ```

pub mod board;
pub mod constants;
pub mod mcts;
pub mod patterns;
pub mod playout;
pub mod position;
