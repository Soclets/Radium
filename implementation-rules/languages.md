# Languages

From D15, D16, D46 and D147.

Code written by the Radium project is **Rust or C++ only**, except the runtime, which is **SPARK** (D147). Tools written by others (Z3, CVC5, LLVM, outside proof
checkers) may be dependencies.

| Part | Language |
|---|---|
| Compiler: parser, type and ownership checker, Geiger, lowering | Rust |
| Runtime linked into every program | SPARK, proven with GNATprove to gold: no run-time errors, plus its key properties (D147); its targets are GNAT's until a Radium-written runtime replaces it |
| Proof checker | Rust, proven correct with Verus |
| Oracles | Rust, separate from the compiler, no shared code |
| LLVM wrapper (only what LLVM's C API lacks) and custom LLVM passes | C++, kept minimal |
| Building the C++ | CMake, from the first C++ file |
| `std` compound, examples, tests | Radium |
| Tools and scripts (merge bot, D18 checker, generators, benchmarks) | Rust; no shell, no Python |
| Docs | Markdown |
| Config | TOML (Cargo), and only the formats a tool requires (`.clang-tidy`, `.clang-format`, `.gitignore`) |

- Anything else (Python, shell, Go, …) needs the owner's explicit OK as an exception.
- **The builds stay separate.** CMake builds the C++ into a library; `cargo build` builds the Rust and links to
  it. Neither drives the other. The order (CMake, then Cargo) is written in the README and run by the merge bot. The
  SPARK runtime is built and proven with its own tools (gprbuild, GNATprove), outside both (D147).
