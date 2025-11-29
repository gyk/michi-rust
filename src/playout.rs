use crate::constants::MAX_GAME_LEN;
use crate::position::{Position, pass_move, play_move};

pub fn mcplayout(pos: &mut Position) -> f64 {
    // Extremely simplified placeholder playout
    let mut passes = 0;
    while passes < 2 && pos.n < MAX_GAME_LEN {
        // naive: play first empty point
        let mut played = false;
        for pt in 0..pos.color.len() {
            if pos.color[pt] == b'.' {
                let ret = play_move(pos, pt);
                if ret.is_empty() {
                    played = true;
                    break;
                }
            }
        }
        if !played {
            pass_move(pos);
            passes += 1;
        } else {
            passes = 0;
        }
    }
    // crude scoring: difference in captures
    (pos.cap as f64) - (pos.cap_x as f64)
}
