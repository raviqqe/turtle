# Turtle

[![GitHub Action](https://img.shields.io/github/workflow/status/raviqqe/turtle/test?style=flat-square)](https://github.com/raviqqe/turtle/actions)
[![License](https://img.shields.io/github/license/raviqqe/turtle.svg?style=flat-square)](LICENSE)

Clone of the [Ninja build system](https://github.com/ninja-build/ninja) in Rust

## Install

```sh
cargo install --git https://github.com/raviqqe/turtle
```

## Usage

```sh
turtle
```

For more information, see `turtle --help`.

## Compatibility

Turtle aims to support full syntax of the Ninja build files.

- Syntax
  - [x] `build` statement
    - [x] Explicit outputs
    - [x] Explicit inputs
    - [ ] Implicit outputs
    - [ ] Implicit inputs
    - [ ] Order-only inputs
    - [ ] `phony` rule
  - [x] `rule` statement
  - [x] `default` statement
  - [x] `include` statement
  - [x] `subninja` statement
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

[MIT](LICENSE)
