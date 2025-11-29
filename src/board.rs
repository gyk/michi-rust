use std::fmt;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Color {
    Black,
    White,
}

pub type Point = (usize, usize);

pub struct Board {
    pub size: usize,
    cells: Vec<Option<Color>>,
}

impl Board {
    pub fn new(size: usize) -> Self {
        Self {
            size,
            cells: vec![None; size * size],
        }
    }
    fn idx(&self, x: usize, y: usize) -> usize {
        y * self.size + x
    }
    pub fn get(&self, x: usize, y: usize) -> Option<Color> {
        if x >= self.size || y >= self.size {
            return None;
        }
        self.cells[self.idx(x, y)]
    }

    fn neighbors(&self, x: usize, y: usize) -> impl Iterator<Item = Point> + '_ {
        let s = self.size;
        let mut v = Vec::new();
        if x > 0 {
            v.push((x - 1, y));
        }
        if x + 1 < s {
            v.push((x + 1, y));
        }
        if y > 0 {
            v.push((x, y - 1));
        }
        if y + 1 < s {
            v.push((x, y + 1));
        }
        v.into_iter()
    }

    pub fn play(&mut self, x: usize, y: usize, color: Color) -> MoveResult {
        if x >= self.size || y >= self.size {
            return MoveResult::illegal();
        }
        if self.get(x, y).is_some() {
            return MoveResult::illegal();
        }
        let idx = self.idx(x, y);
        self.cells[idx] = Some(color);

        let opp = match color {
            Color::Black => Color::White,
            Color::White => Color::Black,
        };
        let mut total_captures = 0;
        let mut to_remove: Vec<Point> = Vec::new();
        for (nx, ny) in self.neighbors(x, y) {
            if self.get(nx, ny) == Some(opp) {
                if self.group_liberties(nx, ny) == 0 {
                    total_captures += self.collect_group(nx, ny, &mut to_remove);
                }
            }
        }
        for (rx, ry) in to_remove {
            let i = self.idx(rx, ry);
            self.cells[i] = None;
        }

        if total_captures == 0 && self.group_liberties(x, y) == 0 {
            self.cells[idx] = None; // undo suicidal move
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

    fn collect_group(&self, x: usize, y: usize, out: &mut Vec<Point>) -> usize {
        let color = self.get(x, y).unwrap();
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

    fn group_liberties(&self, x: usize, y: usize) -> usize {
        if self.get(x, y).is_none() {
            return 0;
        }
        let color = self.get(x, y).unwrap();
        let mut stack = vec![(x, y)];
        let mut visited = vec![false; self.size * self.size];
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
                        None => liberties += 1,
                        Some(c) if c == color && !visited[ni] => stack.push((nx, ny)),
                        _ => {}
                    }
                }
            }
        }
        liberties
    }
}

#[derive(Debug)]
pub struct MoveResult {
    pub legal: bool,
    pub captures: usize,
    pub suicide: bool,
}

impl MoveResult {
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
