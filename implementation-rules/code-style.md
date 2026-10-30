# Code style

From D20.

- **Tools:** `rustfmt` with default settings; `clippy` at its pedantic level, warnings as errors; `clang-format` in
  LLVM's style for the C++.
- **`unsafe` Rust only in named modules**, each with a written reason: Radium's shields (D6), applied to the
  implementation.
- **No `unwrap()` or `expect()` in trusted code** (Geiger, the proof checker, the ownership checker). Every failure is
  a handled error, as in D11.
- **A doc comment on every public item**, saying what it does and, in trusted code, what it assumes.
- **Comments explain why, not what.**
