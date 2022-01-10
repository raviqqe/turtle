# Turtle

[![GitHub Action](https://img.shields.io/github/workflow/status/raviqqe/turtle/test?style=flat-square)](https://github.com/raviqqe/turtle/actions)
[![License](https://img.shields.io/github/license/raviqqe/turtle.svg?style=flat-square)](LICENSE)

Clone of the [Ninja build system](https://github.com/ninja-build/ninja) written in Rust

## Goals

- Safe (no `unsafe`) and fast reimplementation of the Ninja build system in Rust
- Improved frontend support
  - Full output from build rules and no output from Turtle by default
  - More customizable build/rule/progress/error output

## Install

```sh
cargo install turtle-build
```

## Usage

```sh
turtle
```

For more information, see `turtle --help`.

## Compatibility

Turtle aims to support full syntax of the Ninja build files. Command line arguments are supported only partially excluding ones for debugging purposes.

- Syntax
  - [x] `build` statement
    - [x] Explicit outputs
    - [x] Explicit inputs
    - [x] Implicit outputs
    - [x] Implicit inputs
    - [ ] Order-only inputs
    - [ ] `phony` rule
  - [x] `rule` statement
  - [x] `default` statement
  - [x] `include` statement
  - [x] `subninja` statement
  - [ ] `pool` statement
  - [x] Global variables
  - [x] Build-local variables
  - [x] `in` and `out` special variable
  - [ ] `builddir` special variable
- Command line arguments
  - [x] `-f` custom build file option
  - [ ] `-j` job limit option
  - [ ] `-k` keep-going option
- Others
  - [ ] Dynamic dependencies
  - [ ] Circular output dependency detection
  - [ ] Circular build file dependency detection
  - [ ] Windows support

For more information, see [issues](https://github.com/raviqqe/turtle/issues).

## Similar projects

- [`ninja-rs/ninja-rs`](https://github.com/ninja-rs/ninja-rs)
- [`nikhilm/ninja-rs`](https://github.com/nikhilm/ninja-rs)

## License

Dual-licensed under [MIT](LICENSE-MIT) and [Apache 2.0](LICENSE-APACHE).
