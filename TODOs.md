# TODOs - Remaining Work for Full C Parity

## Pattern Matching

- [x] **`env4`/`env4d` arrays** - Encode neighbor colors (4 orthogonal + 4 diagonal) for fast pattern matching. These are updated incrementally when stones are placed/removed.

- [x] **Pattern tables (`pat3set`)** - 3x3 pattern matching infrastructure from `patterns.c`. Implemented:
  - `make_pat3set()` - Initialize the pattern bitfield
  - `pat3_match()` - Fast pattern lookup using env4/env4d encoding

- [x] **Large patterns** - Full `patterns.c` functionality:
  - `init_large_patterns()` - Initialize large pattern database
  - `load_large_patterns()` - Load pattern probability tables from .prob and .spat files
  - `large_pattern_probability()` - Get move probability from pattern database
  - Supports all 8 rotations/reflections of patterns

## Go Heuristics

- [x] **`fix_atari` and ladder reading** - Complex capture/escape analysis:
  - `fix_atari()` - Analyze if a group is in atari and find escape/capture moves
  - `read_ladder_attack()` - Check if a group can be captured in a ladder
  - `line_height()` - Check distance from board edge for ladder optimization
  - `fix_atari_ext()` - Extended version with `twolib_test` and `twolib_edgeonly` options
  - `make_list_neighbor_blocks_in_atari()` - Find opponent blocks in atari (as `find_neighbor_blocks_in_atari`)

- [ ] **`compute_cfg_distances`** - Common fate graph distances for locality priors. Used to give higher priority to moves near the last move.

## Playout Improvements

- [ ] **Full playout heuristics** - The C code uses sophisticated move generation:
  - `gen_playout_moves_capture()` - Generate capture/atari-related moves
  - `gen_playout_moves_pat3()` - Generate moves matching 3x3 patterns
  - `gen_playout_moves_random()` - Generate random moves avoiding eyes
  - `choose_from()` - Select from suggested moves with self-atari rejection
  - `make_list_last_moves_neighbors()` - Prioritize moves near recent play

## MCTS Enhancements

- [x] **Prior initialization** - When expanding nodes, set priors based on:
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

- [x] **GTP protocol** - For integration with Go GUIs
- [x] **Zobrist hashing** - For large pattern matching (used in `patterns.rs`)
- [x] **AMAF map in playouts** - Track which player played each point first
- [ ] **Owner map** - Track territory ownership across simulations

---

# Bug Analysis: Why michi-rust is Weaker than michi-c

*Analysis performed on 2025-11-29*

This section documents the critical differences between michi-rust and michi-c that cause
michi-rust to play significantly weaker, even when michi-c has no pattern files loaded.

## 🔴 Critical Issues

### 1. Playout doesn't update AMAF map (`playout.rs`)

**Location:** `src/playout.rs` - `mcplayout()` function

**C version:**
```c
double mcplayout(Position *pos, int amaf_map[], int owner_map[], int disp)
{
    // ...
    if (amaf_map[move] == 0)
        amaf_map[move] = ((pos->n-1)%2==0 ? 1 : -1);
    // ...
}
```

**Rust version:**
```rust
pub fn mcplayout(pos: &mut Position) -> f64 {
    // No amaf_map parameter, no updates!
}
```

**Impact:** The RAVE (Rapid Action Value Estimation) heuristic only tracks moves made during
tree descent, completely missing all moves made during the playout phase. This is a **massive
information loss** that significantly weakens the RAVE heuristic's effectiveness.

**Fix Required:** Change `mcplayout()` to accept `&mut [i8]` for amaf_map and update it when
moves are played.

---

### 2. Missing ladder attack detection (`position.rs`) ✅ FIXED

**Location:** `src/position.rs` - `fix_atari()` and `fix_atari_ext()` functions

**C version has `read_ladder_attack()`:**
```c
Point read_ladder_attack(Position *pos, Point pt, Slist libs)
// Check if a capturable ladder is being pulled out at pt and return a move
// that continues it in that case. Expects its two liberties in libs.
// Actually, this is a general 2-lib capture exhaustive solver.
{
    FORALL_IN_SLIST(libs, l) {
        Position pos_l = *pos;
        char *ret = play_move(&pos_l, l);
        if (ret[0]!=0) continue;
        // Recursively call fix_atari to check escape
        int is_atari = fix_atari(&pos_l, pt, SINGLEPT_NOK, TWOLIBS_TEST_NO, 0, moves, sizes);
        if (is_atari && slist_size(moves) == 0)
            move = l;  // Ladder attack successful
    }
    return move;
}
```

**Rust version now has:**
```rust
pub fn read_ladder_attack(pos: &Position, pt: Point, libs: &[Point]) -> Point
pub fn fix_atari_ext(pos, pt, singlept_ok, twolib_test, twolib_edgeonly) -> Vec<Point>
pub fn line_height(pt: Point) -> i32  // For edge optimization
```

**Status:** Implemented with full ladder detection, including:
- `twolib_test` parameter for 2-liberty group testing
- `twolib_edgeonly` parameter for edge-only ladder optimization
- Verification that escape moves don't lead into ladders

---

### 3. Wrong self-atari rejection probability for random moves (`playout.rs`)

**Location:** `src/playout.rs` - `try_move_with_self_atari_check()` function

**C version:**
```c
Point choose_from(Position *pos, Slist moves, char *kind, int disp)
{
    // ...
    if (strcmp(kind,"random") == 0)
        tstrej = r <= 10000.0*PROB_RSAREJECT;  // 0.5 for random
    else
        tstrej = r <= 10000.0*PROB_SSAREJECT;  // 0.9 for heuristic
    // ...
}
```

**Rust version:**
```rust
fn try_move_with_self_atari_check(pos: &Position, pt: Point) -> bool {
    // ...
    if random_float() < PROB_SSAREJECT {  // Always 0.9, never 0.5!
        // ...
    }
}
```

**Impact:** Random moves are over-rejected (90% vs 50%), reducing valid nakade and other
tactical random moves that are intentionally self-atari.

**Fix Required:** Pass a flag to distinguish random vs heuristic moves, use `PROB_RSAREJECT`
(0.5) for random moves.

---

### 4. Missing `sqrt()` in large pattern prior (`mcts.rs`)

**Location:** `src/mcts.rs` - `apply_priors()` function

**C version:**
```c
double patternprob = large_pattern_probability(pt);
if (patternprob > 0.0) {
    double pattern_prior = sqrt(patternprob);  // "tone up"
    node->pv += pattern_prior * PRIOR_LARGEPATTERN;
    node->pw += pattern_prior * PRIOR_LARGEPATTERN;
}
```

**Rust version:**
```rust
let pattern_prob = large_pattern_probability(parent_pos, pt);
if pattern_prob >= 0.0 {
    let pattern_prior = pattern_prob as u32;  // Missing sqrt()!
    child.pv += pattern_prior * PRIOR_LARGEPATTERN;
    child.pw += pattern_prior * PRIOR_LARGEPATTERN;
}
```

**Impact:** Pattern priors are not "toned up" with sqrt(), changing their relative strength.
Low-probability patterns get proportionally less prior than they should.

**Fix Required:** Add `.sqrt()` call before casting to u32.

---

## 🟡 Medium Issues

### 5. Missing shuffle in `most_urgent()` (`mcts.rs`)

**Location:** `src/mcts.rs` - `most_urgent()` function

**C version:**
```c
TreeNode* most_urgent(TreeNode **children, int nchildren, int disp)
{
    // Randomize the order of the nodes
    SHUFFLE(TreeNode *, children, nchildren);
    // Then find max urgency
}
```

**Rust version:**
```rust
fn most_urgent(children: &[TreeNode]) -> usize {
    children
        .iter()
        .enumerate()
        .max_by(...)  // No shuffle!
        .map(|(i, _)| i)
        .unwrap_or(0)
}
```

**Impact:** When multiple nodes have equal urgency (common early in search), Rust always
picks the first one instead of randomly choosing. This reduces exploration diversity.

---

### 6. Self-atari prior uses edge-only ladder optimization (`mcts.rs`) ✅ FIXED

**Location:** `src/mcts.rs` - `apply_priors()` function

**C version:**
```c
fix_atari(&node->pos, pt, SINGLEPT_OK, TWOLIBS_TEST, !TWOLIBS_EDGE_ONLY, moves, sizes);
```
Note: `!TWOLIBS_EDGE_ONLY` = false, meaning full ladder analysis for interior groups.

**Rust version (before fix):**
```rust
let atari_moves = fix_atari(&child.pos, pt, true);
// fix_atari defaults to twolib_edgeonly=true, skipping interior ladder analysis!
```

**Impact:** Self-atari detection for MCTS priors was missing ladders in interior positions.
The C version does full ladder analysis for the self-atari prior check, but Rust was using
the edge-only optimization.

**Fix Applied:** Changed to use `fix_atari_ext(&child.pos, pt, true, true, false)` which
matches the C behavior with `twolib_edgeonly=false` for full ladder analysis.

---

### 7. Missing group size tracking (`position.rs`)

**C version returns sizes:**
```c
int fix_atari(Position *pos, Point pt, int singlept_ok,
        int twolib_test, int twolib_edgeonly, Slist moves, Slist sizes)
```

**Rust version only returns moves:**
```rust
pub fn fix_atari(pos: &Position, pt: Point, singlept_ok: bool) -> Vec<Point>
```

**Impact:** Cannot prioritize captures of larger groups over smaller ones.

---

### 7. Escape move doesn't verify ladder safety (`position.rs`) ✅ FIXED

**C version checks ladder after escape:**
```c
if (slist_size(libs) >= 2) {
    if (slist_size(moves)>1
    || (slist_size(libs)==2 && read_ladder_attack(&escpos,l,libs) == 0)
    || (slist_size(libs)>=3))
        // Accept escape move
}
```

**Rust version now checks ladder:**
```rust
if new_libs.len() >= 2 {
    // Good, we escape - but check we're not walking into a ladder
    if !moves.is_empty()
        || new_libs.len() >= 3
        || read_ladder_attack(&test_pos, lib, &new_libs) == 0
    {
        // Accept escape move
    }
}
```

**Status:** Fixed - escape moves are now verified to not lead into working ladders.

---

### 8. Capture priors only check neighbors, not whole board (`mcts.rs`) ✅ FIXED

**Location:** `src/mcts.rs` - `apply_priors()` function

**C version scans entire board for capture priors:**
```c
// In expand():
gen_playout_moves_capture(&tree->pos, allpoints, 1, 1, moves, sizes);
// allpoints contains all board positions, enabling detection of atari anywhere
```

**Rust version only checked neighbors of last moves:**
```rust
let capture_moves = gen_capture_moves(parent_pos);
// gen_capture_moves only looked at neighbors of last and last2
```

**Impact:** Missed capture opportunities away from recent play. The C version gives capture
priors to moves that capture groups in atari anywhere on the board, while Rust only found
captures near the last two moves.

**Fix Applied:** Added `gen_capture_moves_all()` function that scans all board positions for
groups in atari. MCTS priors now use this function with `twolib_edgeonly=false` for full
ladder analysis (matching C's `expensive_ok=1` parameter).

---

## Summary Table

| Issue | Severity | Location | Status |
|-------|----------|----------|--------|
| AMAF not updated in playout | 🔴 Critical | `playout.rs` | ✅ Fixed |
| Missing ladder attack detection | 🔴 Critical | `position.rs` | ✅ Fixed |
| Wrong PROB_RSAREJECT usage | 🔴 Critical | `playout.rs` | ✅ Fixed |
| Missing sqrt() in pattern prior | 🔴 Critical | `mcts.rs` | ✅ Fixed |
| Self-atari prior uses edge-only optimization | 🟡 Medium | `mcts.rs` | ✅ Fixed |
| Missing shuffle in most_urgent() | 🟡 Medium | `mcts.rs` | ✅ Fixed |
| Missing group size tracking | 🟡 Medium | `position.rs` | TODO |
| No ladder check on escape | 🟡 Medium | `position.rs` | ✅ Fixed |
| Capture priors only check neighbors | 🔴 Critical | `mcts.rs` | ✅ Fixed |

---

## Recommended Fix Priority

1. ~~**Fix AMAF in playout** - Highest impact, relatively simple fix~~ ✅
2. ~~**Use PROB_RSAREJECT for random moves** - Quick fix, good impact~~ ✅
3. ~~**Add sqrt() to pattern prior** - One-line fix~~ ✅
4. ~~**Add shuffle in most_urgent()** - Simple fix~~ ✅
5. ~~**Implement ladder reading** - Complex but important for tactical strength~~ ✅
