# michi-rust

This project aims to reimplement Michi (Minimalistic Go MCTS Engine) in Rust, based on
[Michi-c](https://github.com/db3108/michi-c), which itself is a recoding in C of the orginal
[Michi](https://github.com/pasky/michi) code in Python by Petr Baudis.

## Steps to Reimplement Michi-c in Rust

- Firstly, we should add michi-c as a Git submodule via SSH if it is not already added.
    - Please keep the michi-c code as-is, and do not modify it.
    - Please don't "accidentally" delete the whole michi-c folder again.
    - The structure of the michi-c code is as follows:
      ```
      📂 .
      ├── 📄 debug.c
      ├── 📄 main.c
      ├── 📄 Makefile
      ├── 📄 michi.c
      ├── 📄 michi.h
      ├── 📄 patterns.c
      ├── 📄 README.md
      └── 📂 tests
          ├── 📄 fix_atari.tst
          ├── 📄 large_pat.tst
          ├── 📄 patterns.prob
          ├── 📄 patterns.spat
          └── 📄 run
      ```
- Then please read the C code carefully to understand its structure and functionality.
- Next, we rewrite the C code in Rust, module by module, ensuring that we maintain the same
  functionality. We should first make it work, so use `unsafe` whenever necessary.
- The original code has some tests, and we should port those tests to Rust as well.
- Build the code and run the tests to ensure everything is functioning correctly.
- After that, we refactor the Rust code to make it more idiomatic and safe.

## Notes

- Prefer clean and readable code over micro-optimizations.
- My old plan to use c2rust to transpile the C code to Rust, but it seems that c2rust generates a
  lot of garbage code, so I would rather rewrite it in Rust from scratch.
