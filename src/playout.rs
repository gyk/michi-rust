use crate::constants::{BOARD_IMAX, BOARD_IMIN, MAX_GAME_LEN};
use crate::position::{Position, is_eye, pass_move, play_move};

/// Perform a Monte Carlo playout from the given position.
/// Returns a score from the perspective of the player to move at the start.
pub fn mcplayout(pos: &mut Position) -> f64 {
    let start_n = pos.n;
    let mut passes = 0;

    while passes < 2 && pos.n < MAX_GAME_LEN {
        let mut played = false;

        // Try to find a legal move (simple random policy for now)
        // In the real implementation, this should use heuristics like the C code
        for pt in BOARD_IMIN..BOARD_IMAX {
            if pos.color[pt] != b'.' {
                continue; // Not empty
            }
            // Skip true eyes for current player
            if is_eye(pos, pt) == b'X' {
                continue;
            }
            let ret = play_move(pos, pt);
            if ret.is_empty() {
                played = true;
                break;
            }
        }

        if !played {
            pass_move(pos);
            passes += 1;
        } else {
            passes = 0;
        }
    }

    // Compute score
    let s = score(pos);
    // Adjust for whose perspective we're scoring from
    if start_n % 2 != pos.n % 2 { -s } else { s }
}

/// Compute score for to-play player
/// This assumes a final position with all dead stones captured
/// and only single point eyes on the board
fn score(pos: &Position) -> f64 {
    use crate::position::is_eyeish;

    let mut s = if pos.n % 2 == 0 {
        -pos.komi as f64 // komi counts negatively for BLACK
    } else {
        pos.komi as f64
    };

    for pt in BOARD_IMIN..BOARD_IMAX {
        let c = pos.color[pt];
        let effective = if c == b'.' { is_eyeish(pos, pt) } else { c };

        if effective == b'X' {
            s += 1.0;
        } else if effective == b'x' {
            s -= 1.0;
        }
    }

    s
}
