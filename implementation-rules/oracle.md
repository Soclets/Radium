# The oracle

From D16, D48 and D93.

- **Purpose:** a runnable form of `DECISIONS.md`, so the design is enforced by something that doesn't drift.
- **Written by the design session**, in Rust, in `oracle/`. It grows as decisions are made.
- **Tests are small Radium programs** the owner can read, each tagged `// trace D<n>`.
- **Independent from the compiler:** no shared code, and the compiler session never reads the oracle's source, only
  its tests.
- **It has its own `std`**, written in Rust, kept even after Radium's real `std` exists, so a disagreement catches a
  bug in either. Isotopes and the bridge to the operating system get their own oracle versions too.
- **It grows with the design** (D93): only settled parts run. Anything open is reported as "not decided", never
  guessed; anything decided but not yet built is "not built".
- **Readings are listed, not hidden:** where the oracle had to read a decision one way to run at all, the reading is
  in `oracle/README.md` as provisional until the owner rules on it.
