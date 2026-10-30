# Naming

From D19.

- **Case:** Rust's standard conventions, enforced by its lints. C++ follows LLVM's own style, so the wrapper reads
  like the LLVM code beside it.
- **Full words.** An abbreviation is allowed only when it is well known and saves real length. The allowed list is in
  [`TERMS.md`](../TERMS.md); anything not on it is written out (`length`, not `len`).
- **Names say what, not how:** `proven_range`, not `tmp2`.
- **One vocabulary:** every concept has exactly one name, the same in the design docs, code, comments and error
  messages (`Shield`, `ExitWall`, `CheckSite`). `TERMS.md` lists them, and the names no longer used.
- **Module paths match section markers:** `// section prover.layer2.loops:` lives in `prover::layer2::loops`.
