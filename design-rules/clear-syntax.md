# Clear syntax

From D33. The repo's tagline promises "glow in the dark via the clear syntax"; this is what that means.

**Radium is clear to someone who has learned Radium** — not readable without learning, and not Python. Clear means:

1. **No traps.** Nothing looks like one thing and does another (`+` joining text, `%` that isn't percent, `=` vs `==`).
2. **No hidden choices.** Rounding, sign rules and defaults are written where they happen, or the code is refused
   (D25, D37, D38).
3. **One meaning per symbol or word.** A symbol that already means something keeps that meaning everywhere.

A subject that is hard (inline assembly, hardware registers) stays hard; syntax can't fix that and doesn't try.
