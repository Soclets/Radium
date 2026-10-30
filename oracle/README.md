# The oracle

`DECISIONS.md` made runnable (D48, D93). It reads a Radium program, refuses what the compiler would refuse, and runs
the rest with every check site checked as it runs, as in mode 2 (D9). It proves nothing; proving is Geiger's job.
Rules for this code: [`implementation-rules/`](../implementation-rules/).

```bash
cargo run -q -- run tests/programs/hello.ra
```

The program's output goes to stdout and the verdict to stderr.

## Verdicts

| Verdict | Meaning |
|---|---|
| `runs` | It ran to the end of `fn.main`. |
| `refused at line N` | It breaks a decision; the compiler would refuse it. |
| `check fails at line N` | A check site failed while it ran (bounds, exact `div`, `mod`, a contract, `decreases`, a range). |
| `not decided at line N` | It uses something the design hasn't decided. Never guessed. |
| `not built at line N` | It uses something decided that the oracle doesn't run yet. |
| `gives up at line N` | The oracle hit one of its own limits (10 million steps, 200 nested calls, 128-bit numbers). |
| `oracle fault` | A bug in the oracle, never in the program. |

## Tests

```bash
cargo test
```

Each program in `tests/programs/` says what should happen, in comments at the top:

```
// trace D37
// expect: check fails at line 4
fn.main [] -> void:
    std::print(7 div 2, newline); // here
    return void;
end fn.main
```

- `// trace D<n>`: the decisions it tests (at least one).
- `// expect: <verdict>`: exactly one.
- `// output: <text>`: one per printed line, in order.
- `// here` marks the line a verdict points at, for readers.

## What the first slice runs (D93)

`fn.main` (D101), blocks and `end` (D13), `;` (D29), comments (D86, D87), `var`/`var.immut`/`var.mut`/`const` and `$` (D30, D31),
`int[min: …, max: …]` and `bool` (D14, D27, D60), `+`, `-`, `x`, `div` and its narrowings, `mod` and its narrowings,
comparisons, `and`/`or`/`not` (D34–D40), `if` (D71), `for … in range[…]`, `while … decreases`, `forever`, `break` and
named loops (D73–D77), `fn`/`fn.pure`, labels, defaults, `requires`, `ensures`, `old[…]` (D51–D54), `-> void` and `return void;`
(D98–D100), `return` and `result`, with calls inside values (D95), every path ending in `return` (D96, D102), results
never ignored (D97), `my.` on clashes
(D43), ownership words where they apply to integers (D84, D91, D92), and `std::print`.

**Not built yet:** `struct`, `union.tagged`, `match`, `array`, going through collections, narrowed integers
(`int.signed.32_bit`), failures and `otherwise`, `decreases` on functions, hex numbers.

**Not decided, so reported:** parameters for `fn.main`, and anything it gives back but `void` (D101); a bare `int`;
text types (`string`); shadowing; `fails … exactly when`; shields; `invariant`/`assert` spellings; a way to throw a
result away on purpose (D97).

## Provisional readings

The oracle had to read these one way to run at all. None is a decision; each waits for the owner.

1. **The test format above** (`// expect:`, `// output:`, `// here`).
2. **A local without bounds holds an exact integer.** D14 makes bounds optional on locals because Geiger infers them;
   the oracle can't infer, so it checks bounds only where they are written.
3. **Arithmetic is exact until a value is stored.** `$a + $b` is computed exactly, and checked when it lands in a
   bounded value or parameter.
4. **Operator strength:** `x`, `div`, `mod` bind tighter than `+`, `-` (school maths, D34); comparisons come next;
   then `not`, then `and`, then `or`. Mixing `and` and `or` without brackets is allowed.
5. **`std::print`:** prints its items one after another with nothing between, adds no line break of its own (`newline`
   does), and is the only place text may appear while text types are open.
6. **Range labels keep their declared order** (`from:`, then `to:` or `before:`, then `step_by:`), as D54 says for
   labels; `from:` is required.
7. **A failed check stops the program** and reports its line and rule. What a mode 1 or 2 binary does when a check
   fails isn't decided.
8. **A loop's own value is unbounded in the checker**, and changing it is reported as not decided.
