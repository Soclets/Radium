# Backend

From D17 and D83.

**LLVM only**, for all three build modes (mode 1 at `-O0`). No optimizer is trusted:

1. **Translation validation on every run.** The final build runs LLVM's risky passes too (e.g. InstCombine, the loop
   vectorizer). After optimizing, a validator (Alive2, or our own) proves each optimized function does exactly what
   the unoptimized one does.
2. **Fallback.** Where that can't be proven, the function uses the conservative pipeline's output instead: a
   hand-picked, frozen list of passes that is part of the build's identity. The build reports which functions fell
   back and why.
3. **Differential testing** against the independent oracle (D48).
4. **Mode 2's runtime checks** as a safety net: a check that fires is a bug in Geiger or the backend.

- **Custom passes** are allowed as additions. Radium-specific optimizations run on Radium's own IR before Geiger,
  which re-checks the result. C++ LLVM passes run after Geiger only when they need LLVM's view, under the same
  safeguards.
- **Proven facts are handed to LLVM** (`nsw`, `nuw`, `!range`, `noalias`), so proofs make the code faster (D81).
- Slower compiles are the accepted cost.
