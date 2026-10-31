# Brackets

From D50 (CyborgPL's rule, adopted), extended to three brackets by D138.

> **`( )` runs something; `[ ]` describes or holds values; `< >` fills in what a definition leaves open: types and
> sources.**

- **`( )`**: a call. `first_word("hello there", 20)`.
- **`[ ]`**: values — a definition's parameters (`fn first_word [$text: …] -> …:`), a type's bounds and length
  (`int[min: 1, max: 99]`, `array<reading>[length: 3]`), indexes, building a struct or a case (`reading[time = 3600]`),
  array values (`[1, 2, 3]`), a preset (`print.stderr [$target = stderr];`), a value from before a function ran
  (`old[$list]`), a range for a loop (`range[from: 1, to: 5]`).
- **`< >`**: types and sources a definition leaves open, declared on it and filled where it is used: placeholders
  (`fn largest <?T: ordered> [$items: …] -> ?T:`), element types (`std::list<?T>`, `array<reading>[length: 3]`),
  named sources (``struct index <`text`>:``, ``index<`text`: $article>``), interfaces and their type slots
  (`interface ordered <?T>:`, `std::list<?E> satisfies walkable<item: ?E>;`).
- **Spacing tells `< >` from comparisons:** an opening `<` has no space after it, and a closing `>` has no space before
  it; a comparison has a space on each side (D137, D138). `$a<$b` and `holder< int >` are refused.
- **Order inside `< >`** is the writer's, and every use keeps it, as labels and parameters do (D54, D113, D136).
- **The test for a new construct:** a type or a source goes in `< >`; a value goes in `[ ]`.
- **Both `[ ]` and `( )`**: a conversion describes its target and passes its input, `to.…[min: 1, max: 99]($text)`.
- **Inside `[ ]` and `< >`**: `:` labels, attaches a rule where something is declared, or fills in where it is used
  (`min: 1`, `?T: ordered`, `item: ?E`); `=` sets a value's field (`time = 3600`).
- **The one exception: grouping in maths**, `$x - ($a + $b)`.

Carried from CyborgPL's experience: treat anything that acts at build time as running something.
