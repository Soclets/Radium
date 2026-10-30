# Brackets

From D50 (CyborgPL's rule, adopted).

> **`( )` runs something; `[ ]` describes or holds something.**

- **`( )`**: a call. `first_word("hello there", 20)`.
- **`[ ]`**: everything else — a definition's parameters (`fn first_word [$text: …] -> …:`), types and ranges
  (`int[min: 1, max: 99]`), indexes, building a struct or a case (`reading[time = 3600]`), a preset
  (`print.stderr [$target = stderr];`), a value from before a function ran (`old[$list]`), a loop's range.
- **Both**: a conversion describes its target and passes its input, `to.…[min: 1, max: 99]($text)`.
- **Inside `[ ]`**: `:` labels (`min: 1`); `=` sets (`time = 3600`).
- **The one exception: grouping in maths**, `$x - ($a + $b)`.

Carried from CyborgPL's experience: place generics in this rule when they are designed, and treat anything that acts
at build time as running something.
