# Radium

*Slowly kills you via the steep learning curve, and slow compile times. Glow in the dark via the clear syntax. It
also has a safety radius.*

Radium is a programming language, in design, for final products that cannot have any mistakes. Every check that could
fail at run time is proven at compile time, including termination, stack depth and memory use. Code that can't be
proven, such as C calls and raw memory access, is fenced into shields with checked walls. Radium is not certified,
and it is not recommended for professional use.

There is no compiler yet. This repository holds the design:

- [`DECISIONS.md`](DECISIONS.md) — every design decision, numbered. Code will point at these numbers.
- [`TERMS.md`](TERMS.md) — one name per concept, and the allowed abbreviations.
- [`design-rules/`](design-rules/) — the general rules the design is judged by: clear syntax, choosing words,
  dots and underscores, brackets, marks, defaults.

## Licences

- [`LICENSE`](LICENSE) — **Apache-2.0 WITH LLVM-exception**, for all code. Programs you compile with Radium owe
  nothing: the exception waives the notice requirements for the parts of Radium built into your program.
- [`LICENSE-CC-BY-4.0`](LICENSE-CC-BY-4.0) — **CC BY 4.0**, for this repo's writing: its Markdown files
  (this README, `DECISIONS.md`, `TERMS.md`, `design-rules/` and the other notes). Copy, translate, remix and use them
  anywhere, commercially too, with credit. Credit them as:
  `Radium GitHub repo text: https://github.com/Soclets/Radium`
  As the licence asks, also say they are under CC BY 4.0, and whether you changed them. Code and code comments stay
  under `LICENSE`.
