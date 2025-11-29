# michi-rust

A Rust reimplementation of [Michi](https://github.com/pasky/michi) — a Minimalistic Go MCTS Engine.

This project is based on [Michi-c](https://github.com/db3108/michi-c), which itself is a C port of the original Python code by Petr Baudis.

## Features

- **Monte Carlo Tree Search (MCTS)** based Go engine
- **GTP (Go Text Protocol)** support for GUI integration
- Supports **9x9** (default) and **13x13** board sizes
- Pattern-based move priors for stronger play
- Configurable playing strength via simulation count

## Quick Start

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (1.85+)
- Pattern files (optional but recommended for stronger play)

### Building

```bash
# 9x9 board (default)
cargo build --release

# 13x13 board
cargo build --release --no-default-features --features board13x13
```

### Running Tests

```bash
# 9x9 board (default)
cargo test

# 13x13 board
cargo test --no-default-features --features board13x13
```

### Playing with a Go GUI (e.g., Sabaki)

1. Download the large pattern files from https://pasky.or.cz/dev/pachi/michi-pat/ (recommended)
2. Extract `patterns.prob` and `patterns.spat` to a folder (e.g., `michi-c/`)
3. Run the GTP server:

```bash
cargo run --release -- gtp --patterns michi-c
```

4. In your Go GUI (like [Sabaki](https://sabaki.yichuanshen.de/)):
   - Go to **Engines** → **Manage Engines**
   - Add a new engine with the path to `michi-rust` and arguments: `gtp --patterns <pattern_folder>`
   - Start a new game and attach the engine

### Command Line Options

```bash
# Basic GTP server
cargo run --release -- gtp

# GTP server with patterns
cargo run --release -- gtp --patterns <pattern_folder>

# GTP server with custom simulation count
cargo run --release -- gtp --simulations 2000

# GTP server with predefined strength level
cargo run --release -- gtp --level strong

# Run a demo
cargo run --release -- demo
```

#### Strength Levels

| Level       | Simulations | Description                |
|-------------|-------------|----------------------------|
| `weak`      | 10          | Very weak (for testing)    |
| `medium`    | 500         | Medium strength            |
| `strong`    | 1400        | Strong (default)           |
| `very-strong` | 5000      | Very strong                |
| `max`       | 20000       | Maximum (slow but strongest) |

## Board Size Configuration

The board size is set at compile time using Cargo features:

| Feature      | Board Size | Command                                              |
|--------------|------------|------------------------------------------------------|
| `board9x9`   | 9×9        | `cargo build` (default)                              |
| `board13x13` | 13×13      | `cargo build --no-default-features --features board13x13` |

> **Note:** Only one board size can be enabled at a time.

## Pattern Files

For stronger play, michi-rust can use large-scale pattern files:

1. Download from: http://pachi.or.cz/michi-pat/
2. Extract `patterns.prob` and `patterns.spat` to a folder
3. Use `--patterns <folder>` when running

The pattern files are also included in the `michi-c/` folder for convenience.

## GTP Commands

The engine supports the following GTP commands:

- `name` - Engine name
- `version` - Engine version
- `protocol_version` - GTP protocol version (2)
- `list_commands` - List supported commands
- `known_command <cmd>` - Check command support
- `quit` - Exit
- `boardsize <size>` - Set board size
- `clear_board` - Reset board
- `komi <value>` - Set komi
- `play <color> <vertex>` - Play a move
- `genmove <color>` - Generate and play a move

## Example Session

```
$ cargo run --release -- gtp --patterns michi-c
michi-rust: Loaded 54544 large patterns from "michi-c"
michi-rust: Starting GTP with 1400 simulations per move
name
= michi-rust

version
= 0.1.0

boardsize 9
=

genmove black
= E5

quit
=
```

## License

This project is distributed under the MIT License, following the original Michi project.

## Acknowledgments

- [Petr Baudis](https://github.com/pasky) for the original [Michi](https://github.com/pasky/michi) in Python
- [db3108](https://github.com/db3108) for the [Michi-c](https://github.com/db3108/michi-c) port
