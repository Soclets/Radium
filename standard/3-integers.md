# 3. Integers

## 3.1 Bounds

1. An integer type has **bounds**: its lowest and highest value, both included, written `int[min: …, max: …]` (D14,
   D74). `min:` comes before `max:`. `max:` first: **not decided**.
2. `min` above `max` is refused (D14).
3. **Every value an integer holds is a check site**: it must stay within its bounds. Geiger proves it; modes 1 and 2
   also check it as the program runs (D9, D14).
4. Bounds are required on parameters, results, struct fields and globals (D14). A local value may write `: infer`
   instead (D108).
5. An `int` with no bounds at all: **not decided** (D26).
   *Tests:* `bounds_hold`, `bounds_checked`, `bounds_backwards`, `bare_int_open`, `argument_outside_bounds`.

## 3.2 Widths

1. Integers have **no width cap** (D104). Every integer has one fixed width, chosen when compiling; it never grows
   while the program runs.
2. **Up to 64 bits:** a bare `int[…]` takes the smallest width that holds its bounds, unsigned when `min` is 0 or more
   and signed when it is negative (D27). Bounds that need more than 64 bits are refused on a bare `int` (D105).
3. **Beyond 64 bits:** `int.wide[…]` takes whatever width its bounds need. Bounds that fit in 64 bits are refused on
   `int.wide`: write `int[…]` (D107).
4. **An exact width:** `int.signed.N_bit[…]` or `int.unsigned.N_bit[…]`, for formats and hardware (D26, D27). Bounds
   the width can't hold are refused; an unsigned width holds no negative bound. A signedness without a width
   (`int.signed[…]`): **not decided**.
5. Signed and unsigned integers mix freely: every value is checked against its own bounds, so nothing converts
   silently (D27).

| Bounds need | Written |
|---|---|
| up to 64 bits | `int[min: …, max: …]` |
| more than 64 bits | `int.wide[min: …, max: …]` |
| an exact width | `int.signed.N_bit[…]` / `int.unsigned.N_bit[…]` |

*Tests:* `bare_int_up_to_64_bits`, `bare_int_over_64_bits_refused`, `signed_needs_65_bits_refused`,
`wide_picks_its_width`, `wide_refused_when_64_bits_fit`, `wide_bounds_still_checked`, `wide_int_written_out`,
`width_too_small_refused`, `unsigned_holds_no_negatives`, `signed_width`, `signedness_without_width_open`,
`big_numbers_stay_exact`.

## 3.3 Literals

1. A number is written in decimal digits, with `_` allowed between them for reading: `86_399`.
2. A number may not run straight into a word: `3x4` is refused; `x` needs spaces around it (D34).
3. Hex numbers (`0x…`) are numbers (D34); the oracle doesn't read them yet.
4. Decimal literals such as `1.5` need an imported float or fixed-point type (D22, D25); see section 9.
