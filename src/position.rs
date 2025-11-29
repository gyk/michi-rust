use crate::constants::*;

pub type Point = usize;

#[derive(Clone)]
pub struct Position {
    pub color: [u8; BOARDSIZE], // 'X', 'x', '.', ' ' like C variant
    pub n: usize,
    pub ko: Point,
    pub ko_old: Point,
    pub last: Point,
    pub last2: Point,
    pub last3: Point,
    pub cap: u32,
    pub cap_x: u32,
    pub komi: f32,
}

impl Position {
    pub fn new() -> Self {
        let mut p = Position {
            color: [b' '; BOARDSIZE],
            n: 0,
            ko: 0,
            ko_old: 0,
            last: 0,
            last2: 0,
            last3: 0,
            cap: 0,
            cap_x: 0,
            komi: 7.5,
        };
        empty_position(&mut p);
        p
    }
}

pub fn empty_position(pos: &mut Position) -> &'static str {
    // Reset to initial position with C padding layout
    let mut k = 0;
    for _col in 0..=N {
        pos.color[k] = b' ';
        k += 1;
    }
    for _row in 1..=N {
        pos.color[k] = b' ';
        k += 1;
        for _col in 1..=N {
            pos.color[k] = b'.';
            k += 1;
        }
    }
    for _col in 0..W {
        pos.color[k] = b' ';
        k += 1;
    }
    pos.ko = 0;
    pos.last = 0;
    pos.last2 = 0;
    pos.last3 = 0;
    pos.cap = 0;
    pos.cap_x = 0;
    pos.n = 0;
    ""
}

fn swap_color(pos: &mut Position) {
    for c in pos.color.iter_mut() {
        if *c == b'X' {
            *c = b'x';
        } else if *c == b'x' {
            *c = b'X';
        }
    }
}

pub fn pass_move(pos: &mut Position) -> &'static str {
    swap_color(pos);
    pos.n += 1;
    pos.last2 = pos.last;
    pos.last = PASS_MOVE;
    std::mem::swap(&mut pos.cap, &mut pos.cap_x);
    ""
}

pub fn play_move(pos: &mut Position, pt: Point) -> &'static str {
    if pt == PASS_MOVE {
        return pass_move(pos);
    }
    if pos.color[pt] != b'.' {
        return "Error Illegal move: point not EMPTY";
    }
    pos.ko_old = pos.ko;
    pos.color[pt] = b'X';
    let mut captured = 0u32;
    let mut to_remove: Vec<Point> = Vec::new();
    let opp = b'x';
    for n in neighbors(pt) {
        if pos.color[n] == opp {
            if group_liberties(pos, n) == 0 {
                captured += collect_group(pos, n, &mut to_remove);
            }
        }
    }
    for r in to_remove {
        pos.color[r] = b'.';
    }
    if captured == 0 && group_liberties(pos, pt) == 0 {
        pos.color[pt] = b'.';
        pos.ko = pos.ko_old;
        return "Error Illegal move: suicide";
    }
    captured += pos.cap_x;
    pos.cap_x = pos.cap;
    pos.cap = captured;
    swap_color(pos);
    pos.n += 1;
    pos.last2 = pos.last;
    pos.last = pt;
    ""
}

fn neighbors(pt: Point) -> [Point; 4] {
    let delta = [-(N as isize) - 1, 1, (N as isize) + 1, -1];
    let mut arr = [0usize; 4];
    for (i, d) in delta.iter().enumerate() {
        arr[i] = (pt as isize + d) as usize;
    }
    arr
}

fn collect_group(pos: &Position, start: Point, out: &mut Vec<Point>) -> u32 {
    let color = pos.color[start];
    let mut stack = vec![start];
    let mut visited = [false; BOARDSIZE];
    let mut count = 0u32;
    while let Some(pt) = stack.pop() {
        if visited[pt] {
            continue;
        }
        visited[pt] = true;
        if pos.color[pt] == color {
            out.push(pt);
            count += 1;
            for n in neighbors(pt) {
                if !visited[n] && pos.color[n] == color {
                    stack.push(n);
                }
            }
        }
    }
    count
}

fn group_liberties(pos: &Position, start: Point) -> u32 {
    let color = pos.color[start];
    let mut stack = vec![start];
    let mut visited = [false; BOARDSIZE];
    let mut libs = 0u32;
    while let Some(pt) = stack.pop() {
        if visited[pt] {
            continue;
        }
        visited[pt] = true;
        if pos.color[pt] == color {
            for n in neighbors(pt) {
                match pos.color[n] {
                    b'.' => libs += 1,
                    c if c == color && !visited[n] => stack.push(n),
                    _ => {}
                }
            }
        }
    }
    libs
}

pub fn parse_coord(s: &str) -> Point {
    if s.eq_ignore_ascii_case("pass") {
        return PASS_MOVE;
    }
    let bytes = s.as_bytes();
    if bytes.len() < 2 {
        return PASS_MOVE;
    }
    let col = (bytes[0].to_ascii_uppercase() - b'A' + 1) as usize;
    let mut row = 0usize;
    for b in &bytes[1..] {
        if b.is_ascii_digit() {
            row = row * 10 + (b - b'0') as usize;
        }
    }
    (N - row + 1) * (N + 1) + col
}

pub fn str_coord(pt: Point) -> String {
    if pt == PASS_MOVE {
        return "pass".into();
    }
    let row = pt / (N + 1);
    let col = pt % (N + 1);
    let mut c = (b'@' + col as u8) as char;
    if c > 'H' {
        c = ((c as u8) + 1) as char;
    }
    format!("{c}{}", N + 1 - row)
}
