# Ownership

From D80, D84, D91, D92 and D125.

- **No hidden ownership.** Whenever a value that isn't copied automatically goes to another holder (a parameter,
  another name with `=`, a `return`, a field or an item), the hand-off is written: `Give`, `Copy`, `Lend` or
  `Lend.mut`. The words start with a capital, so every hand-off stands out (D84).
- **Always-copied values take no word**: integers and `bool` (D92).
- **Reading in place isn't a hand-off**: `$r.celsius`, `$xs[pos: 1]`, `match $s:`, `for $x in $xs:`. Nor is a fresh
  value just built, since it has no holder yet.
- **A parameter says how it holds the value** (D91, D92): bare (lent, read-only), `Lent.mut`, or `Given`. The call's
  word matches it. A bare parameter writes nothing more, since the call's `Lend` already shows it (D125).
- **`=`, `return`, fields and items take `Give` or `Copy`**; storing a borrow there with `Lend` waits for lifetimes
  (D49, D125). Only what is owned can be given.
- A new feature that moves values around (lists, maps, text) is checked against this rule.
