# TODOs - Remaining Work for Full C Parity

## Pattern Matching

- [ ] **`env4`/`env4d` arrays** - Encode neighbor colors (4 orthogonal + 4 diagonal) for fast pattern matching. These are updated incrementally when stones are placed/removed.

- [ ] **Pattern tables (`pat3set`)** - 3x3 pattern matching infrastructure from `patterns.c`. Requires:
  - `make_pat3set()` - Initialize the pattern bitfield
  - `pat3_match()` - Fast pattern lookup using env4/env4d encoding

- [ ] **Large patterns** - Full `patterns.c` functionality:
  - `init_large_patterns()` - Load pattern probability tables
  - `copy_to_large_board()` - Copy position to large board for pattern matching
  - `large_pattern_probability()` - Get move probability from pattern database

## Go Heuristics

- [ ] **`fix_atari` and ladder reading** - Complex capture/escape analysis:
  - `fix_atari()` - Analyze if a group is in atari and find escape/capture moves
  - `read_ladder_attack()` - Check if a group can be captured in a ladder
  - `make_list_neighbor_blocks_in_atari()` - Find opponent blocks in atari

- [ ] **`compute_cfg_distances`** - Common fate graph distances for locality priors. Used to give higher priority to moves near the last move.

## Playout Improvements

- [ ] **Full playout heuristics** - The C code uses sophisticated move generation:
  - `gen_playout_moves_capture()` - Generate capture/atari-related moves
  - `gen_playout_moves_pat3()` - Generate moves matching 3x3 patterns
  - `gen_playout_moves_random()` - Generate random moves avoiding eyes
  - `choose_from()` - Select from suggested moves with self-atari rejection
  - `make_list_last_moves_neighbors()` - Prioritize moves near recent play

## MCTS Enhancements

- [ ] **Prior initialization** - When expanding nodes, set priors based on:
  - Capture heuristics (`PRIOR_CAPTURE_ONE`, `PRIOR_CAPTURE_MANY`)
  - 3x3 pattern matches (`PRIOR_PAT3`)
  - Large pattern probabilities (`PRIOR_LARGEPATTERN`)
  - CFG distance from last move (`PRIOR_CFG`)
  - Self-atari detection (`PRIOR_SELFATARI` as negative prior)
  - Empty area detection (`PRIOR_EMPTYAREA`)

- [ ] **Early termination** - Stop search early if winrate is very high:
  - `FASTPLAY5_THRES` (0.95) at 5% of simulations
  - `FASTPLAY20_THRES` (0.8) at 20% of simulations

## Testing

- [ ] **Port C test files** - The `michi-c/tests/` directory contains:
  - `fix_atari.tst` - Atari/ladder tests
  - `large_pat.tst` - Large pattern tests
  - `patterns.prob` / `patterns.spat` - Pattern data files

## Optional Enhancements

- [ ] **GTP protocol** - For integration with Go GUIs
- [ ] **Zobrist hashing** - For position comparison and superko detection
- [ ] **AMAF map in playouts** - Track which player played each point first
- [ ] **Owner map** - Track territory ownership across simulations
