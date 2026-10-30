# Dots and underscores

From D26 and D35 (the owner's rule).

- **`.` narrows the thing before it**: a more specific version of it.
  `int` → `int.signed` → `int.signed.32_bit`; `var` → `var.immut`; `fn` → `fn.pure`; `div` → `div.floor`;
  `print` → `print.stderr`; a value to its field, `$r.celsius`; a union to its case, `status.ok`.
- **`_` joins words into one name**: `not_equal`, `at_most`, `32_bit`. Nothing is narrowed.

Consequences:

- **No aliases outside a chain.** There is no `i32` beside `int.signed.32_bit`; a short name would stand outside the
  narrowing.
- **Fewer dots, the default decides; more dots, you decide** (see [defaults](defaults.md)).
- If `a.b` doesn't read as "`a`, made more specific", it should be `a_b` or a different word.
