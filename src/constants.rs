pub const N: usize = 13;
pub const W: usize = N + 2;
pub const BOARDSIZE: usize = (N + 1) * W + 1; // matches C layout size
pub const BOARD_IMIN: usize = N + 1;
pub const BOARD_IMAX: usize = BOARDSIZE - N - 1;
pub const MAX_GAME_LEN: usize = N * N * 3;
pub const PASS_MOVE: usize = 0;
pub const RESIGN_MOVE: usize = usize::MAX;

// MCTS Constants
pub const N_SIMS: usize = 1400;
pub const RAVE_EQUIV: usize = 3500;
pub const EXPAND_VISITS: u32 = 8;
pub const RESIGN_THRES: f64 = 0.2;
pub const FASTPLAY20_THRES: f64 = 0.8;
pub const FASTPLAY5_THRES: f64 = 0.95;

// Prior values
pub const PRIOR_EVEN: u32 = 10;
pub const PRIOR_SELFATARI: u32 = 10;
pub const PRIOR_CAPTURE_ONE: u32 = 15;
pub const PRIOR_CAPTURE_MANY: u32 = 30;
pub const PRIOR_PAT3: u32 = 10;
pub const PRIOR_LARGEPATTERN: u32 = 100;
pub const PRIOR_CFG: [u32; 3] = [24, 22, 8];
pub const PRIOR_EMPTYAREA: u32 = 10;

// Playout probabilities
pub const PROB_HEURISTIC_CAPTURE: f64 = 0.9;
pub const PROB_HEURISTIC_PAT3: f64 = 0.95;
pub const PROB_SSAREJECT: f64 = 0.9;
pub const PROB_RSAREJECT: f64 = 0.5;

// Neighbor deltas (N, E, S, W, NE, SE, SW, NW)
pub const DELTA: [isize; 8] = [
    -(N as isize) - 1, // North
    1,                 // East
    (N as isize) + 1,  // South
    -1,                // West
    -(N as isize),     // NE
    W as isize,        // SE
    N as isize,        // SW
    -(W as isize),     // NW
];
