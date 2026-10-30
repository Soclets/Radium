# 9. Not written yet

## 9.1 Decided, not yet written here

| Area | Decisions |
|---|---|
| Shields: unproven code, its walls and its reason | D6 |
| Failures: precise failure sets, `fails` lines, `otherwise`, handler narrowings | D11, D56, D57, D78 |
| `struct` and its fields | D58 |
| `union.tagged` and `match`; bare `union` in shields only | D59, D72 |
| `array`, `std::list`, the maps; `pos:` and `offset:` | D64, D66, D70 |
| Function collections (`narrowings.fn`), presets, `defaults.fixed` | D57 |
| Generics: `?T`, `interface`, `type_set` | D69 |
| Imports: isotopes (`@`) and compounds; floats, decimals and decimal literals | D22–D25, D28, D104 |
| Termination, stack and heap | D7, D8 |
| Build modes and marked binaries | D9, D10 |
| Deadlines | D82 |

## 9.2 Not decided

Listed where they come up above, and in `DECISIONS.md`'s Open list. Among them: how strongly operators bind; text
types; how imports are spelled; the shield spelling; the `~` mark for inexact literals; the evaluate-both narrowing of
`and`/`or`; the deadline mark.
