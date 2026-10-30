# The oracle

From D16 and D48.

- **Purpose:** a runnable form of `DECISIONS.md`, so the design is enforced by something that doesn't drift.
- **Written by the design session**, in Rust, in `oracle/`. It grows as decisions are made.
- **Tests are small Radium programs** the owner can read, each tagged `// trace D<n>`.
- **Independent from the compiler:** no shared code, and the compiler session never reads the oracle's source, only
  its tests.
- **It has its own `std`**, written in Rust, kept even after Radium's real `std` exists, so a disagreement catches a
  bug in either. Isotopes and the bridge to the operating system get their own oracle versions too.
- **When it starts:** after the syntax phase (D48).
