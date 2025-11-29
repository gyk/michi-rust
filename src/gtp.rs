//! Go Text Protocol (GTP) implementation.
//!
//! GTP is a text-based protocol for communicating with Go-playing programs.
//! This module implements GTP version 2, allowing the engine to be used
//! with graphical Go interfaces like Sabaki, GoGui, or Lizzie.
//!
//! ## Supported Commands
//!
//! - `name` - Return engine name
//! - `version` - Return engine version
//! - `protocol_version` - Return GTP protocol version (2)
//! - `list_commands` - List all supported commands
//! - `known_command <cmd>` - Check if a command is supported
//! - `quit` - Exit the program
//! - `boardsize <size>` - Set board size (only 13 is supported currently)
//! - `clear_board` - Reset the board to empty
//! - `komi <value>` - Set komi (only 7.5 is supported currently)
//! - `play <color> <vertex>` - Play a move
//! - `genmove <color>` - Generate and play a move for the given color
//!
//! ## Example
//!
//! ```ignore
//! use michi_rust::gtp::GtpEngine;
//! let mut engine = GtpEngine::new();
//! engine.run();
//! ```

use std::io::{self, BufRead, Write};

use crate::constants::{BOARDSIZE, N, N_SIMS, PASS_MOVE, RESIGN_MOVE, RESIGN_THRES};
use crate::mcts::{TreeNode, tree_search_with_display};
use crate::position::{
    Position, empty_position, format_position_with_owner, parse_coord, pass_move, play_move,
    str_coord,
};

/// The list of known GTP commands.
const KNOWN_COMMANDS: &[&str] = &[
    "boardsize",
    "clear_board",
    "cputime",
    "genmove",
    "help",
    "known_command",
    "komi",
    "list_commands",
    "name",
    "play",
    "protocol_version",
    "quit",
    "showboard",
    "version",
];

/// GTP engine state.
pub struct GtpEngine {
    /// Current game position
    pos: Position,
    /// MCTS tree (recreated after each move)
    tree: Option<TreeNode>,
    /// Number of simulations for MCTS search
    n_sims: usize,
    /// Owner map for territory display
    owner_map: Vec<i32>,
    /// Start time for cputime command
    start_time: std::time::Instant,
}

impl GtpEngine {
    /// Create a new GTP engine with default settings.
    pub fn new() -> Self {
        Self::with_simulations(N_SIMS)
    }

    /// Create a new GTP engine with a specified number of simulations per move.
    pub fn with_simulations(n_sims: usize) -> Self {
        let pos = Position::new();
        let tree = Some(TreeNode::new(&pos));
        Self {
            pos,
            tree,
            n_sims,
            owner_map: vec![0i32; BOARDSIZE],
            start_time: std::time::Instant::now(),
        }
    }

    /// Print the board state to stderr with owner map.
    fn print_board(&self) {
        let board_str = format_position_with_owner(&self.pos, Some(&self.owner_map), self.n_sims);
        eprint!("{}", board_str);
    }

    /// Get the prompt indicator based on whose turn it is.
    /// Returns ○ for White to play, ● for Black to play.
    fn get_turn_indicator(&self) -> &'static str {
        if self.pos.is_black_to_play() {
            "●" // Black to play
        } else {
            "○" // White to play
        }
    }

    /// Run the GTP command loop, reading from stdin and writing to stdout.
    pub fn run(&mut self) {
        let stdin = io::stdin();
        let mut stdout = io::stdout();
        let mut stderr = io::stderr();

        for line in stdin.lock().lines() {
            let line = match line {
                Ok(l) => l,
                Err(_) => break,
            };

            // Skip empty lines and comments
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            // Parse optional command ID
            let (id, command_line) = Self::parse_id(line);

            // Parse command and arguments
            let parts: Vec<&str> = command_line.split_whitespace().collect();
            if parts.is_empty() {
                continue;
            }

            let command = parts[0].to_lowercase();
            let args = &parts[1..];

            // Execute command
            let response = self.execute(&command, args);

            // Print board after command execution (to stderr, like michi-c)
            self.print_board();
            stderr.flush().unwrap();

            // Format and send response
            let (success, message) = response;
            let prefix = if success { '=' } else { '?' };
            let id_str = id.map(|i| i.to_string()).unwrap_or_default();

            writeln!(stdout, "{prefix}{id_str} {message}\n").unwrap();
            stdout.flush().unwrap();

            // Print turn indicator prompt to stderr
            write!(stderr, "{} michi-rust> ", self.get_turn_indicator()).unwrap();
            stderr.flush().unwrap();

            // Quit if requested
            if command == "quit" {
                break;
            }
        }
    }

    /// Parse an optional numeric command ID from the beginning of the line.
    fn parse_id(line: &str) -> (Option<u32>, &str) {
        let trimmed = line.trim();
        let mut chars = trimmed.char_indices();

        // Check if line starts with a digit
        if let Some((_, c)) = chars.next() {
            if c.is_ascii_digit() {
                // Find end of number
                let end = chars
                    .find(|(_, c)| !c.is_ascii_digit())
                    .map(|(i, _)| i)
                    .unwrap_or(trimmed.len());

                if let Ok(id) = trimmed[..end].parse::<u32>() {
                    return (Some(id), trimmed[end..].trim());
                }
            }
        }

        (None, trimmed)
    }

    /// Execute a GTP command and return (success, response).
    fn execute(&mut self, command: &str, args: &[&str]) -> (bool, String) {
        match command {
            "name" => (true, "michi-rust".to_string()),

            "version" => (true, env!("CARGO_PKG_VERSION").to_string()),

            "protocol_version" => (true, "2".to_string()),

            "list_commands" => {
                let commands = KNOWN_COMMANDS.join("\n");
                (true, commands)
            }

            "known_command" => {
                if args.is_empty() {
                    return (false, "missing argument".to_string());
                }
                let known = KNOWN_COMMANDS.contains(&args[0].to_lowercase().as_str());
                (true, if known { "true" } else { "false" }.to_string())
            }

            "quit" => (true, String::new()),

            "boardsize" => {
                if args.is_empty() {
                    return (false, "missing argument".to_string());
                }
                match args[0].parse::<usize>() {
                    Ok(size) if size == N => (true, String::new()),
                    Ok(size) => (
                        false,
                        format!("unacceptable size, only {N} is supported (got {size})"),
                    ),
                    Err(_) => (false, "invalid size".to_string()),
                }
            }

            "clear_board" => {
                empty_position(&mut self.pos);
                self.tree = Some(TreeNode::new(&self.pos));
                self.owner_map.iter_mut().for_each(|x| *x = 0);
                (true, String::new())
            }

            "komi" => {
                if args.is_empty() {
                    return (false, "missing argument".to_string());
                }
                match args[0].parse::<f32>() {
                    Ok(komi) => {
                        self.pos.komi = komi;
                        (true, String::new())
                    }
                    Err(_) => (false, "invalid komi".to_string()),
                }
            }

            "play" => {
                if args.len() < 2 {
                    return (false, "missing arguments".to_string());
                }

                // Parse color (ignored - we use alternating play)
                let _color = args[0].to_lowercase();

                // Parse vertex
                let vertex = args[1].to_lowercase();
                let pt = parse_coord(&vertex);

                // Handle pass
                if vertex == "pass" || pt == PASS_MOVE {
                    pass_move(&mut self.pos);
                    self.tree = None; // Invalidate tree
                    return (true, String::new());
                }

                // Check if point is empty
                if self.pos.color[pt] != b'.' {
                    return (false, "illegal move".to_string());
                }

                // Try to play the move
                let result = play_move(&mut self.pos, pt);
                if result.is_empty() {
                    self.tree = None; // Invalidate tree
                    (true, String::new())
                } else {
                    (false, result.to_string())
                }
            }

            "genmove" => {
                if args.is_empty() {
                    return (false, "missing argument".to_string());
                }

                // If opponent passed and we're past the opening, pass too
                if self.pos.last == PASS_MOVE && self.pos.n > 2 {
                    pass_move(&mut self.pos);
                    return (true, "pass".to_string());
                }

                // Create fresh tree for search with display
                let mut tree = TreeNode::new(&self.pos);
                // Clear owner map before search
                self.owner_map.iter_mut().for_each(|x| *x = 0);
                let pt = tree_search_with_display(&mut tree, self.n_sims, &mut self.owner_map);

                // Check for resignation
                let winrate = tree
                    .children
                    .iter()
                    .max_by_key(|c| c.v)
                    .map(|c| c.winrate())
                    .unwrap_or(0.0);

                if winrate < RESIGN_THRES && pt != PASS_MOVE {
                    return (true, "resign".to_string());
                }

                // Play the move
                if pt == PASS_MOVE || pt == RESIGN_MOVE {
                    pass_move(&mut self.pos);
                    (true, "pass".to_string())
                } else {
                    play_move(&mut self.pos, pt);
                    (true, str_coord(pt))
                }
            }

            "showboard" => {
                // Output the board to stderr (GTP debug output) and return empty success
                let board_str =
                    format_position_with_owner(&self.pos, Some(&self.owner_map), self.n_sims);
                eprint!("{}", board_str);
                (true, format!("\n{}", board_str.trim_end()))
            }

            "cputime" => {
                let elapsed = self.start_time.elapsed().as_secs_f64();
                (true, format!("{:.3}", elapsed))
            }

            "help" => {
                let commands = KNOWN_COMMANDS.join("\n");
                (true, commands)
            }

            _ => (false, format!("unknown command: {command}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_id_with_id() {
        let (id, cmd) = GtpEngine::parse_id("123 name");
        assert_eq!(id, Some(123));
        assert_eq!(cmd, "name");
    }

    #[test]
    fn test_parse_id_without_id() {
        let (id, cmd) = GtpEngine::parse_id("name");
        assert_eq!(id, None);
        assert_eq!(cmd, "name");
    }

    #[test]
    fn test_name_command() {
        let mut engine = GtpEngine::new();
        let (success, response) = engine.execute("name", &[]);
        assert!(success);
        assert_eq!(response, "michi-rust");
    }

    #[test]
    fn test_protocol_version() {
        let mut engine = GtpEngine::new();
        let (success, response) = engine.execute("protocol_version", &[]);
        assert!(success);
        assert_eq!(response, "2");
    }

    #[test]
    fn test_known_command() {
        let mut engine = GtpEngine::new();

        let (success, response) = engine.execute("known_command", &["name"]);
        assert!(success);
        assert_eq!(response, "true");

        let (success, response) = engine.execute("known_command", &["unknown_cmd"]);
        assert!(success);
        assert_eq!(response, "false");
    }

    #[test]
    fn test_boardsize() {
        let mut engine = GtpEngine::new();

        // Correct size
        let (success, _) = engine.execute("boardsize", &[&N.to_string()]);
        assert!(success);

        // Wrong size
        let (success, _) = engine.execute("boardsize", &["19"]);
        assert!(!success);
    }

    #[test]
    fn test_play_and_clear() {
        let mut engine = GtpEngine::new();

        // Play a move
        let (success, _) = engine.execute("play", &["black", "D4"]);
        assert!(success);

        // Clear board
        let (success, _) = engine.execute("clear_board", &[]);
        assert!(success);
        assert_eq!(engine.pos.n, 0);
    }
}
