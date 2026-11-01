# Clear syntax

From D33, and D146 for rule 4. The repo's tagline promises "glow in the dark via the clear syntax"; this is what that means.

**Radium is clear to someone who has learned Radium** — not readable without learning, and not Python. Clear means:

1. **No traps.** Nothing looks like one thing and does another (`+` joining text, `%` that isn't percent, `=` vs `==`).
2. **No hidden choices.** Rounding, sign rules and defaults are written where they happen, or the code is refused
   (D25, D37, D38).
3. **One meaning per symbol or word.** A symbol that already means something keeps that meaning everywhere.
4. **Nothing is never a gap.** An empty or absent value is written as a word (`void`, `string.empty`), never left as
   a blank (a bare `return;`, `""`, a missing `->`), so a deliberate nothing can't be mistaken for something
   forgotten (D98–D100, D145, D146).

A subject that is hard (inline assembly, hardware registers) stays hard; syntax can't fix that and doesn't try.

**The one deliberate exception:** `.l.`, an Easter-egg spelling of `satisfies` (D149), kept as a joke in the spirit
of Rust's turbofish. It breaks rules 1 and 3 on purpose, and nothing else may cite it as a precedent.
