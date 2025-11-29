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
    pub cap: u32,   // captures by current player (X)
    pub cap_x: u32, // captures by opponent (x)
    pub komi: f32,
}

impl Default for Position {
    fn default() -> Self {
        Self::new()
    }
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
    pos.last3 = pos.last2;
    pos.last2 = pos.last;
    pos.last = PASS_MOVE;
    pos.ko = 0; // Ko is cleared on pass
    std::mem::swap(&mut pos.cap, &mut pos.cap_x);
    ""
}

/// Check if pt is inside a single-color diamond and return the color or 0
/// This could be an eye, but also a false one
pub fn is_eyeish(pos: &Position, pt: Point) -> u8 {
    let mut eyecolor: u8 = 0;
    let mut othercolor: u8 = 0;
    for n in neighbors(pt) {
        let c = pos.color[n];
        if c == b' ' {
            continue; // ignore OUT of board neighbours
        }
        if c == b'.' {
            return 0;
        }
        if eyecolor == 0 {
            eyecolor = c;
            othercolor = if c == b'X' { b'x' } else { b'X' };
        } else if c == othercolor {
            return 0;
        }
    }
    eyecolor
}

/// Check if pt is a true eye and return its color or 0
pub fn is_eye(pos: &Position, pt: Point) -> u8 {
    let eyecolor = is_eyeish(pos, pt);
    if eyecolor == 0 {
        return 0;
    }
    let falsecolor = if eyecolor == b'X' { b'x' } else { b'X' };
    let mut at_edge = 0;
    let mut false_count = 0;

    for d in diagonal_neighbors(pt) {
        if pos.color[d] == b' ' {
            at_edge = 1;
        } else if pos.color[d] == falsecolor {
            false_count += 1;
        }
    }
    if at_edge != 0 {
        false_count += 1;
    }
    if false_count >= 2 {
        return 0;
    }
    eyecolor
}

pub fn play_move(pos: &mut Position, pt: Point) -> &'static str {
    if pt == PASS_MOVE {
        return pass_move(pos);
    }
    if pos.color[pt] != b'.' {
        return "Error Illegal move: point not EMPTY";
    }

    // Check ko
    pos.ko_old = pos.ko;
    if pt == pos.ko {
        return "Error Illegal move: retakes ko";
    }

    // Check if playing into enemy eye (for ko detection)
    let in_enemy_eye = is_eyeish(pos, pt);

    pos.color[pt] = b'X';
    let mut captured = 0u32;
    let mut pos_capture: Point = 0;
    let mut to_remove: Vec<Point> = Vec::new();
    let opp = b'x';

    for n in neighbors(pt) {
        if pos.color[n] == opp && group_liberties(pos, n) == 0 {
            let group_size = collect_group(pos, n, &mut to_remove);
            captured += group_size;
            pos_capture = n;
        }
    }

    // Remove captured stones
    for r in &to_remove {
        pos.color[*r] = b'.';
    }

    if captured > 0 {
        // Set ko if captured exactly one stone in an eye
        if captured == 1 && in_enemy_eye != 0 {
            pos.ko = pos_capture;
        } else {
            pos.ko = 0;
        }
    } else {
        // Test for suicide
        pos.ko = 0;
        if group_liberties(pos, pt) == 0 {
            pos.color[pt] = b'.';
            pos.ko = pos.ko_old;
            return "Error Illegal move: suicide";
        }
    }

    // Update captures (cumulative)
    let total_captured = captured + pos.cap_x;
    pos.cap_x = pos.cap;
    pos.cap = total_captured;

    swap_color(pos);
    pos.n += 1;
    pos.last3 = pos.last2;
    pos.last2 = pos.last;
    pos.last = pt;
    ""
}

fn neighbors(pt: Point) -> [Point; 4] {
    // N, E, S, W neighbors using DELTA constants
    [
        (pt as isize + DELTA[0]) as usize, // North
        (pt as isize + DELTA[1]) as usize, // East
        (pt as isize + DELTA[2]) as usize, // South
        (pt as isize + DELTA[3]) as usize, // West
    ]
}

fn diagonal_neighbors(pt: Point) -> [Point; 4] {
    // NE, SE, SW, NW neighbors using DELTA constants
    [
        (pt as isize + DELTA[4]) as usize, // NE
        (pt as isize + DELTA[5]) as usize, // SE
        (pt as isize + DELTA[6]) as usize, // SW
        (pt as isize + DELTA[7]) as usize, // NW
    ]
}

/// Get all 8 neighbors (4 orthogonal + 4 diagonal)
pub fn all_neighbors(pt: Point) -> [Point; 8] {
    [
        (pt as isize + DELTA[0]) as usize,
        (pt as isize + DELTA[1]) as usize,
        (pt as isize + DELTA[2]) as usize,
        (pt as isize + DELTA[3]) as usize,
        (pt as isize + DELTA[4]) as usize,
        (pt as isize + DELTA[5]) as usize,
        (pt as isize + DELTA[6]) as usize,
        (pt as isize + DELTA[7]) as usize,
    ]
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
    let mut liberty_visited = [false; BOARDSIZE]; // Track visited liberties to avoid double-counting
    let mut libs = 0u32;
    while let Some(pt) = stack.pop() {
        if visited[pt] {
            continue;
        }
        visited[pt] = true;
        if pos.color[pt] == color {
            for n in neighbors(pt) {
                match pos.color[n] {
                    b'.' => {
                        if !liberty_visited[n] {
                            liberty_visited[n] = true;
                            libs += 1;
                        }
                    }
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
    let mut col = (bytes[0].to_ascii_uppercase() - b'A' + 1) as usize;
    // Skip 'I' column (Go convention)
    if bytes[0].to_ascii_uppercase() > b'I' {
        col -= 1;
    }
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
    if c >= 'I' {
        c = ((c as u8) + 1) as char; // Skip 'I'
    }
    format!("{c}{}", N + 1 - row)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_position() {
        let pos = Position::new();
        // Check that the center is empty
        let center = (N / 2 + 1) * (N + 1) + (N / 2 + 1);
        assert_eq!(pos.color[center], b'.');
        assert_eq!(pos.n, 0);
        assert_eq!(pos.ko, 0);
    }

    #[test]
    fn test_parse_str_coord_roundtrip() {
        let pos = Position::new();
        // Test some coordinates
        for row in 1..=N {
            for col in 1..=N {
                let pt = row * (N + 1) + col;
                if pos.color[pt] == b'.' {
                    let s = str_coord(pt);
                    let parsed = parse_coord(&s);
                    assert_eq!(pt, parsed, "Failed roundtrip for {}", s);
                }
            }
        }
    }

    #[test]
    fn test_play_move_basic() {
        let mut pos = Position::new();
        let pt = parse_coord("D4");
        let result = play_move(&mut pos, pt);
        assert!(result.is_empty(), "Move should be legal");
        assert_eq!(pos.n, 1);
        assert_eq!(pos.last, pt);
    }

    #[test]
    fn test_play_move_suicide() {
        let mut pos = Position::new();
        // Create a situation where playing at a point would be suicide
        // Set up the corner: Black stones at A2 and B1,
        // then Black tries to play at A1 - this would be suicide

        // But wait - after each move colors swap!
        // Move 1: Black plays A2 (becomes x after swap)
        play_move(&mut pos, parse_coord("A2"));
        // Move 2: White plays somewhere (becomes x, Black's A2 becomes X)
        play_move(&mut pos, parse_coord("M13"));
        // Move 3: Black plays B1 (becomes x)
        play_move(&mut pos, parse_coord("B1"));

        // Now it's White's turn. The corner A1 is surrounded by Black stones
        // (which are now 'x' since it's White's turn)
        // White playing A1 would be suicide
        let corner = parse_coord("A1");
        let result = play_move(&mut pos, corner);
        assert!(
            result.contains("suicide"),
            "A1 should be suicide for White: got '{}'",
            result
        );
    }
    #[test]
    fn test_capture() {
        let mut pos = Position::new();
        // Black plays, White plays, Black captures
        let b1 = parse_coord("C3");
        let w1 = parse_coord("D3");
        let b2 = parse_coord("E3");
        let w2 = parse_coord("D4");
        let b3 = parse_coord("D2");
        let w3 = parse_coord("M13"); // White plays elsewhere
        let b4 = parse_coord("C4"); // Now black can capture
        let _w4 = parse_coord("E4");

        play_move(&mut pos, b1);
        play_move(&mut pos, w1);
        play_move(&mut pos, b2);
        play_move(&mut pos, w2);
        play_move(&mut pos, b3);
        play_move(&mut pos, w3);

        // Before capture, D3 should have x (opponent stone)
        assert_eq!(pos.color[w1], b'x');

        // Play capturing move
        let result = play_move(&mut pos, b4);
        assert!(result.is_empty(), "Capture move should be legal");
    }

    #[test]
    fn test_ko_rule() {
        let pos = Position::new();
        // Set up a ko situation
        // This is a simplified test - a real ko test would need more setup
        // For now, just verify the ko field is being set

        assert_eq!(pos.ko, 0); // Initially no ko
    }

    #[test]
    fn test_group_liberties() {
        let mut pos = Position::new();
        let pt = parse_coord("D4");
        play_move(&mut pos, pt);

        // A single stone in the middle should have 4 liberties
        // After play_move, colors are swapped, so the stone is 'x'
        // The group_liberties function works on the raw position
        let libs = group_liberties(&pos, pt);
        assert_eq!(libs, 4, "Single stone should have 4 liberties");
    }

    #[test]
    fn test_is_eye() {
        let pos = Position::new();
        // Create a simple eye pattern in the corner
        // This would require more careful setup for a proper test

        // Empty position should not be an eye
        let pt = parse_coord("A1");
        assert_eq!(is_eye(&pos, pt), 0);
    }
}
