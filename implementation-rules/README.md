# Implementation rules

The rules for the code that implements Radium: the compiler (Curie), the prover (Geiger), the proof checker, the
runtime and the oracle. `DECISIONS.md` records each decision with the owner's words; these files collect the rule
itself, so new code can be checked against it. The rules for the language's own design are in
[`design-rules/`](../design-rules/).

| Rule | In short | From |
|---|---|---|
| [Languages](languages.md) | Rust, with minimal C++ for LLVM; Radium for `std` and tests; nothing else without the owner's OK | D15, D16, D46 |
| [Naming](naming.md) | Full words, one name per concept, module paths match section markers | D19 |
| [Comments and tracing](comments-and-tracing.md) | Section markers and `trace D<n>` tags, checked by a script | D18 |
| [Code style](code-style.md) | rustfmt, pedantic clippy, no `unwrap` in trusted code, comments say why | D20 |
| [C++](cpp.md) | Small, C++23, hardened, banned constructs, sanitizers, behind a shield | D44 |
| [Backend](backend.md) | LLVM only; no optimizer trusted: every risky pass is validated or falls back | D17, D83 |
| [Proof trust](proof-trust.md) | Both solvers plus a re-checked certificate; the checker is itself proven | D12, D16 |
| [The oracle](oracle.md) | An independent interpreter that makes the decisions runnable | D16, D48 |
| [Branches and merging](branches-and-merging.md) | The design session commits to `main`; other sessions use own branches and a merge bot; commit messages are `kind.folder.narrowing: sentence` | D45, D89, D90 |

When a rule changes, `DECISIONS.md` gets a new entry saying so, and the rule's file here is updated to match. Code
that breaks a rule must say why, in a comment at that spot.
