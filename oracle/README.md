# The oracle

`DECISIONS.md` made runnable (D48, D93). It reads a Radium program, refuses what the compiler would refuse, and runs
the rest with every check site checked as it runs, as in mode 2 (D9). It proves nothing; proving is Geiger's job.
Rules for this code: [`implementation-rules/`](../implementation-rules/).

```bash
cargo run -q -- run tests/programs/hello.ra
```

The program's output goes to stdout. If it stops, its message goes to stderr, laid out as Radium's messages are
(D116–D119, D122):

```
check failed: bounds_checked.ra:5:13, line 5 column 13
 5 |     $byte = $byte + 56; // here
   |             ^^^^^^^^^^
 what: `$byte` holds int[min: 0, max: 255], and this value doesn't fit
 why:  every value stays within its bounds (D14)
 when: it would be 256
 because: bounds_checked.ra:4:5, line 4 column 5
 4 |     var $byte: int[min: 0, max: 255] = 200;
   |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
     `$byte`'s bounds are set here
```

Besides Radium's `refused:` and `check failed:`, the oracle has three kinds of its own: `not decided:`, `not built:`
and `gave up:`. It never says `unproven:`, since it proves nothing.

Dependencies (D15 allows tools written by others), all MIT OR Apache-2.0: `num-bigint`, `num-integer` and
`num-traits`, so integers are exact at any size (D104); `unicode-segmentation`, so columns for people count grapheme
clusters (D117).

## Verdicts

| Verdict | Meaning |
|---|---|
| `runs` | It ran to the end of `fn.main`. |
| `refused at line N` | It breaks a decision; the compiler would refuse it. |
| `check fails at line N` | A check site failed while it ran (bounds, exact `div`, `mod`, a contract, `decreases`, a range). |
| `not decided at line N` | It uses something the design hasn't decided. Never guessed. |
| `not built at line N` | It uses something decided that the oracle doesn't run yet. |
| `gives up at line N` | The oracle hit one of its own limits (10 million steps, 200 nested calls). Numbers have no size limit (D104). |
| `oracle fault` | A bug in the oracle, never in the program. |

## Tests

```bash
cargo test
```

`tests/messages.rs` also checks the message layout itself.

Each program in `tests/programs/` says what should happen, in comments at the top (D103). The oracle drops comments;
the test runner reads them as text:

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

## What it runs

`fn.main` (D101), blocks and `end` (D13), `;` (D29), comments (D86, D87), `var`/`var.immut`/`var.mut`/`const`, `$`, and a type or `: infer` on every value (D30, D31, D108),
`int[min: …, max: …]` up to 64 bits, `int.wide[…]` beyond, and `int.signed.N_bit[…]` / `int.unsigned.N_bit[…]` at
any width (D14, D27, D104, D105, D107), and `bool` (D60), `+`, `-`, `x`, `div` and its narrowings, `mod` and its narrowings,
comparisons, `and`/`or`/`not` (D34–D40), `if` (D71), `for … in range[…]`, `while … decreases`, `forever`, `break` and
named loops (D73–D77), `fn`/`fn.pure`, labels, defaults, `requires`, `ensures`, `old[…]` (D51–D54), `-> void` and `return void;`
(D98–D100), `return` and `result`, with calls inside values (D95), every path ending in `return` (D96, D102), results
never ignored (D97), `my.` on clashes
(D43), and `std::print`.

The second slice adds `struct` with fields, building, `.immut` fields and fields that follow their holder (D58);
`union.tagged` and `match` with every case covered (D59, D72); arrays, with `[…]` and `array[…]` values (D124),
`pos:`/`offset:` items and `for $x in $xs` (D64, D70, D74); and ownership: `Give`, `Copy`,
`Lend`, `Lend.mut`, bare (lent), `Lent.mut` and `Given` parameters, no use after `Give`, and `must_Give` values handed
off on every path (D5, D80, D84, D91, D92), with no hidden ownership: `=`, `return`, fields and items take `Give` or
`Copy` (D125).

**Not built yet:** `std::list` and the maps, failures and `otherwise`, function collections, generics, `decreases` on functions, `Lend from`, hex numbers.

**Not decided, so reported:** parameters for `fn.main`, and anything it gives back but `void` (D101); a bare `int`;
text types (`string`); shadowing; `fails … exactly when`; shields; `invariant`/`assert` spellings; a way to throw a
result away on purpose (D97); building a struct without every field; comparing or printing a whole struct, union or
array; storing a borrow with `Lend` in a name, result, field or item (D49, D125); pulling a struct out of a `case`; going through items that are structs, unions or arrays; how a `must_Give` value's duty ends in the function that receives
it; a `const` struct.

## Provisional readings

The oracle had to read these one way to run at all. None is a decision; each waits for the owner.

1. ~~The test format~~: decided, D103.
2. **An `infer` value holds an exact integer, of any size.** Geiger works out its bounds (D108); the oracle can't, so it
   checks bounds only where they are written, and a value that grows without limit runs into the step limit instead of
   being refused.
3. **Arithmetic is exact until a value is stored.** `$a + $b` is computed exactly, and checked when it lands in a
   bounded value or parameter.
4. ~~Operator strength~~: decided, D111.
5. ~~`std::print`'s joining~~: decided, D112. Text may still appear only as a `print` item while text types are open.
6. ~~Range label order~~: decided, D113.
7. ~~What a failed check does~~: decided, D114. The oracle stops and reports `check fails at line N`, the same as a
   mode 1 or 2 program would.
8. ~~Changing a loop's own value~~: decided, D115.
9. **Fields may be written in any order** when building a struct or a case.
10. **`$r.field = value;` changes one field**, reading D41 (`=` sets a value) with D26 (`.` narrows).
11. **A value declared outside a loop can't be handed off with `Give` inside it**, since a later trip would hand it
    off again, even if the trip that gives it then leaves the loop.
12. **A value pulled out by `case` doesn't change.**
13. **`Lend.mut` hands the function a copy and takes the changed copy back** when the call ends. With nothing else
    allowed to use the value meanwhile (D5), that behaves the same as lending the value itself.
