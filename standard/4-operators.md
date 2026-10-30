# 4. Operators

## 4.1 Symbols and words

1. A symbol is used only where school maths writes it the same way, with no hidden choice: `+`, `-`, `<`, `>`, and
   `x` for multiply (D34). `x` needs spaces around it.
2. Where there is a hidden choice, a trap, or no school symbol, the operator is a word (D34):

   | Operator | Meaning |
   |---|---|
   | `+`, `-`, `x` | add, subtract, multiply |
   | `div` | exact division (4.2) |
   | `mod` | remainder (4.3) |
   | `<`, `>`, `at_most`, `at_least` | less, greater, at most, at least |
   | `equal`, `not_equal` | the same value, or not; both sides must be the same kind |
   | `and`, `or`, `not` | on `bool`s only (section 5) |

3. `/` has no meaning (D34, D37). `=` only sets a value; comparing is `equal`, so there is no `=`/`==` trap (D41).
   *Tests:* `school_maths`, `comparisons`, `slash_is_not_division`.
4. `-` before a value negates it: `-1`, `-$x`.
5. Brackets `( )` group, as in maths: `($a + $b) x 2`. It is the one place `( )` runs nothing (D50).
6. **How strongly operators bind** without brackets, strongest first (D111):

   | Strength | Operators |
   |---|---|
   | 1 | `-` before a value |
   | 2 | `x`, `div`, `mod` and their narrowings |
   | 3 | `+`, `-` |
   | 4 | `<`, `>`, `at_most`, `at_least`, `equal`, `not_equal` |
   | 5 | `not` |
   | 6 | `and`, `or` |

   Operators of the same strength group from the left: `10 - 3 - 2` is `(10 - 3) - 2`. **Mixing `and` and `or`
   without brackets is refused**: write `$a or ($b and $c)`.
   *Tests:* `school_maths`, `same_strength_groups_left`, `and_or_need_brackets`, `and_or_with_brackets`.
7. A chain of comparisons such as `1 < $x < 5`: **not decided**.
8. **Whether a result in the middle of an expression is itself a check site** (for example `$a + $b` before it is
   stored): **not decided**. The oracle computes it exactly and checks it where it is stored.

## 4.2 Division

1. **Bare `div` is exact**: a division that leaves a remainder is a check site that fails (D37).
2. Rounding is chosen by narrowing (D37): `div.floor` (down), `div.trunc` (toward zero), `div.ceil` (up). Writing
   `div.exact` is refused: it spells out the default (D39).
3. Division by zero is a check site that fails.
   *Tests:* `div_exact`, `div_exact_leaves_remainder`, `div_rounding`, `div_exact_spelled_out`, `division_by_zero`.

## 4.3 Remainder

1. **Bare `mod` needs both numbers non-negative**, where every definition agrees; otherwise it is a check site that
   fails (D38).
2. Otherwise, narrow (D38): `mod.floor` (takes the divisor's sign), `mod.trunc` (takes the dividend's sign),
   `mod.euclid` (never negative).
3. `div.floor` pairs with `mod.floor`, and `div.trunc` with `mod.trunc`, so that
   dividend = divisor x quotient + remainder always holds (D38).
   *Tests:* `mod_rules`, `mod_bare_needs_non_negative`, `divide_and_remainder_agree`.

## 4.4 Order of evaluation

1. Evaluation order is fixed: arguments run left to right, as written (D55).
2. **`and` and `or` short-circuit**: the right side runs only when needed, and Geiger uses the left side as a fact
   when checking the right (D40). A narrowing that always evaluates both sides exists (D40); its spelling is **not
   decided**.
   *Tests:* `short_circuit`, `check_order_left_to_right`.
