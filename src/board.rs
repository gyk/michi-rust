//! Alternative board representation (2D with explicit coordinates).
//!
//! This module provides a simpler, more intuitive board representation
//! using 2D coordinates (x, y) instead of the 1D array with padding used
//! in the main `position` module.
//!
//! **Note:** This implementation is separate from the main engine and is
//! primarily useful for:
//! - Testing and debugging with clearer coordinate semantics
//! - Integration with external tools that expect 2D coordinates
//! - Learning/reference purposes
//!
//! For the main Go engine logic, see the `position` module which uses
//! the C-compatible 1D representation.

use std::fmt;

/// Stone color.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Color {
    Black,
    White,
}

impl Color {
    /// Get the opponent's color.
    #[inline]
    pub fn opponent(self) -> Color {
        match self {
            Color::Black => Color::White,
            Color::White => Color::Black,
        }
    }
}

/// A point on the board as (x, y) coordinates.
pub type Point = (usize, usize);

/// A Go board with 2D coordinate access.
pub struct Board {
    /// Board size (NxN).
    pub size: usize,
    /// Board state (None = empty, Some(color) = occupied).
    cells: Vec<Option<Color>>,
}

impl Board {
    /// Create a new empty board of the given size.
    pub fn new(size: usize) -> Self {
        Self {
            size,
            cells: vec![None; size * size],
        }
    }

    /// Convert (x, y) coordinates to array index.
    #[inline]
    fn idx(&self, x: usize, y: usize) -> usize {
        y * self.size + x
    }

    /// Get the stone at (x, y), or None if empty or out of bounds.
    pub fn get(&self, x: usize, y: usize) -> Option<Color> {
        if x >= self.size || y >= self.size {
            return None;
        }
        self.cells[self.idx(x, y)]
    }

    /// Get orthogonal neighbors of a point.
    fn neighbors(&self, x: usize, y: usize) -> impl Iterator<Item = Point> + '_ {
        let s = self.size;
        [
            (x > 0).then(|| (x - 1, y)),
            (x + 1 < s).then(|| (x + 1, y)),
            (y > 0).then(|| (x, y - 1)),
            (y + 1 < s).then(|| (x, y + 1)),
        ]
        .into_iter()
        .flatten()
    }

    /// Play a stone at (x, y).
    ///
    /// Returns `MoveResult` indicating legality, captures, and suicide status.
    pub fn play(&mut self, x: usize, y: usize, color: Color) -> MoveResult {
        if x >= self.size || y >= self.size {
            return MoveResult::illegal();
        }
        if self.get(x, y).is_some() {
            return MoveResult::illegal();
        }

        let idx = self.idx(x, y);
        self.cells[idx] = Some(color);

        // Capture opponent stones
        let opp = color.opponent();
        let mut total_captures = 0;
        let mut to_remove = Vec::new();

        for (nx, ny) in self.neighbors(x, y) {
            if self.get(nx, ny) == Some(opp) && self.group_liberties(nx, ny) == 0 {
                total_captures += self.collect_group(nx, ny, &mut to_remove);
            }
        }

        for (rx, ry) in to_remove {
            let i = self.idx(rx, ry);
            self.cells[i] = None;
        }

        // Check for suicide
        if total_captures == 0 && self.group_liberties(x, y) == 0 {
            self.cells[idx] = None;
            return MoveResult {
                legal: false,
                captures: 0,
                suicide: true,
            };
        }

        MoveResult {
            legal: true,
            captures: total_captures,
            suicide: false,
        }
    }

    /// Collect all stones in a group using flood fill.
    fn collect_group(&self, x: usize, y: usize, out: &mut Vec<Point>) -> usize {
        let color = match self.get(x, y) {
            Some(c) => c,
            None => return 0,
        };

        let mut stack = vec![(x, y)];
        let mut visited = vec![false; self.size * self.size];
        let mut count = 0;

        while let Some((cx, cy)) = stack.pop() {
            let i = self.idx(cx, cy);
            if visited[i] {
                continue;
            }
            visited[i] = true;

            if self.get(cx, cy) == Some(color) {
                out.push((cx, cy));
                count += 1;
                for (nx, ny) in self.neighbors(cx, cy) {
                    let ni = self.idx(nx, ny);
                    if !visited[ni] && self.get(nx, ny) == Some(color) {
                        stack.push((nx, ny));
                    }
                }
            }
        }
        count
    }

    /// Count liberties of the group at (x, y).
    fn group_liberties(&self, x: usize, y: usize) -> usize {
        let color = match self.get(x, y) {
            Some(c) => c,
            None => return 0,
        };

        let mut stack = vec![(x, y)];
        let mut visited = vec![false; self.size * self.size];
        let mut liberty_visited = vec![false; self.size * self.size];
        let mut liberties = 0;

        while let Some((cx, cy)) = stack.pop() {
            let i = self.idx(cx, cy);
            if visited[i] {
                continue;
            }
            visited[i] = true;

            if self.get(cx, cy) == Some(color) {
                for (nx, ny) in self.neighbors(cx, cy) {
                    let ni = self.idx(nx, ny);
                    match self.get(nx, ny) {
                        None => {
                            if !liberty_visited[ni] {
                                liberty_visited[ni] = true;
                                liberties += 1;
                            }
                        }
                        Some(c) if c == color && !visited[ni] => stack.push((nx, ny)),
                        _ => {}
                    }
                }
            }
        }
        liberties
    }
}

/// Result of attempting to play a move.
#[derive(Debug)]
pub struct MoveResult {
    /// Whether the move was legal.
    pub legal: bool,
    /// Number of stones captured.
    pub captures: usize,
    /// Whether the move was rejected due to suicide.
    pub suicide: bool,
}

impl MoveResult {
    /// Create an illegal move result.
    fn illegal() -> Self {
        MoveResult {
            legal: false,
            captures: 0,
            suicide: false,
        }
    }
}

impl fmt::Display for Board {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for y in 0..self.size {
            for x in 0..self.size {
                let ch = match self.get(x, y) {
                    Some(Color::Black) => 'X',
                    Some(Color::White) => 'O',
                    None => '.',
                };
                write!(f, "{ch} ")?;
            }
            writeln!(f)?;
        }
        Ok(())
    }
}
