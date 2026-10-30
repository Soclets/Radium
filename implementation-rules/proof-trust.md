# Proof trust

From D12 and D16.

- **A check site is proven only when both Z3 and CVC5 answer `unsat`**, and the proof certificate (CVC5's proof
  output) is re-checked by a small, separate proof checker.
- **The proof checker is Rust, proven correct with Verus**, and cross-checked by an independent outside checker
  (e.g. Carcara).
- **Fallback:** where no certificate can be produced or checked, both solvers agreeing still counts, and the build
  report marks the site "solver-agreed, not certificate-checked".
- **Disagreement means unproven.**
