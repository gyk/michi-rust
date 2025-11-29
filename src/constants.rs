pub const N: usize = 13;
pub const W: usize = N + 2;
pub const BOARDSIZE: usize = (N + 1) * W + 1; // matches C layout size
pub const MAX_GAME_LEN: usize = N * N * 3;
pub const PASS_MOVE: usize = 0;
pub const RESIGN_MOVE: usize = usize::MAX;

pub const N_SIMS: usize = 1400;
pub const EXPAND_VISITS: u32 = 8;
pub const RESIGN_THRES: f64 = 0.2;
