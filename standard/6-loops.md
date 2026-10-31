# 6. Loops

Every loop must provably end, except `forever` (D7, D76).

## 6.1 `for` over a range

1. `for $i in range[…]:` … `end for` goes through a sequence of numbers (D73, D74):

   | Label | Meaning |
   |---|---|
   | `from:` | the first number |
   | `to:` | the last number, included |
   | `before:` | stop just before this number |
   | `step_by:` | the amount added each time; 1 when left out; negative counts down |

2. A range has `to:` or `before:`, never both. **`from:` is required** (D113). Other labels (`until:`, `by:`, …) are
   refused (D73).
3. **A loop whose step can never reach its end is refused** when its numbers are known while compiling, for example
   `from: 5, to: 1` with no negative step, or a step of 0 (D74). Otherwise it is a check site.
4. The labels are written in their declared order: `from:`, then `to:` or `before:`, then `step_by:`; out of order is
   refused (D54, D113).
5. `for $x in $xs` goes through an array, front to back (D74); see section 9.4.
6. The loop's own value is `var.immut` for each trip: changing it inside the loop is refused (D115).

```
for $i in range[from: 1, to: 5]:                // 1, 2, 3, 4, 5
for $i in range[from: 1, before: 5]:            // 1, 2, 3, 4
for $i in range[from: 5, to: 1, step_by: -1]:   // 5, 4, 3, 2, 1
```

*Tests:* `range_to_and_before`, `range_never_ends`, `range_never_ends_at_run_time`, `range_label_order`,
`range_needs_from`, `range_until_is_not_used`, `loop_value_is_fixed`.

## 6.2 `while`

1. `while …:` … `end while` runs while its `bool` condition holds (D75).
2. It must provably end. Where Geiger can't see why, the loop's first line is `decreases …;`: a value that shrinks on
   every trip and never goes below zero. A `decreases` that doesn't shrink, or goes below zero, is a check site that
   fails (D75).
   *Tests:* `while_decreases`, `while_decreases_must_shrink`.

## 6.3 `forever`

1. `forever:` … `end forever` is the only loop allowed to run without end; each trip must provably end (D7, D76).
   *Tests:* `forever_and_break`.

## 6.4 `break`

1. `break;` stops the innermost loop it is in (D77). Outside a loop it is refused.
2. A loop may be named (`for rows $row in …:` … `end for rows`); `break rows;` stops that loop and every loop inside
   it (D77). A loop named the same as one around it: **not decided**.
3. There is no "skip to the next trip" statement: write the condition around the work instead (D77).
   *Tests:* `break_named_loop`, `break_outside_loop`, `no_skip_statement`.
