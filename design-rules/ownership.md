# Ownership

From D49, D80, D84, D91, D92, D125–D127, D129, D130, D132–D136 and D138.

- **No hidden ownership.** Whenever a value that isn't copied automatically goes to another holder (a parameter,
  another name with `=`, a `return`, a field or an item), the hand-off is written: `Give`, `Copy`, `Lend` or
  `Lend.mut`. The words start with a capital, so every hand-off stands out (D84).
- **Always-copied values take no word**: integers and `bool` (D92).
- **Reading in place isn't a hand-off**: `$r.celsius`, `$xs[pos: 1]`, `match $s:`, `for $x in $xs:`. Nor is a fresh
  value just built, since it has no holder yet.
- **A parameter says how it holds the value** (D91, D92): bare (lent, read-only), `Lent.mut`, or `Given`. The call's
  word matches it. A bare parameter writes nothing more, since the call's `Lend` already shows it (D125).
- **`=`, `return`, fields and items take `Give`, `Copy`, or a stored borrow (`Lend`, `Lend.mut`)** whose type names
  its source (D125, D126, D135). Only what is owned can be given.
- **A borrow names where it comes from** (D49, D126): a parameter (`Lend from $article`), or, inside a struct, a
  **named source** in backticks (`` `text` ``). Nothing is left for the checker to fill in. A borrow may come from
  either of several sources, listed in `< >` (`Lend from <$x, $y>`, D138), and sources can be compared
  (``struct index <`text`, `table`: lasts_at_least `text`>``, D138), including `program`, which lasts until
  the program ends (D127).
- **A value lasts until the end of its block** (D140); **a borrow ends at its last use** (D129), on any path from
  there (D144), and until then its source can't change or be given away (D5).
- **A borrow comes only from a source its type names** (D130), even if another would last long enough. A borrow stands
  for the value it reaches: lending it passes on its source, and `Copy` of it gives a value of its own (D142, D143).
- **A function states the `lasts_at_least` rules its parameters must meet** when it fills a struct with them, so its
  contract shows everything a caller must meet: `requires $list lasts_at_least $article;` (D132). The same fact
  written twice is refused (D127).
- **Every source a struct declares has its own name and is used by a field** (D133, D134).
- **A stored `Lend.mut from` is exclusive**: the value changes only through it, and its source can't be used at all
  while it lasts. A plain `Lend from` is read-only (D135).
- **A parameter's inner borrow is named by the function's own sources**, declared before its parameters like
  placeholders (D138): ``fn count_words <`t`> [$view: word_view<`text`: `t`>]`` (D136).
- **No borrow from a fresh value**: when a result borrows from a parameter, its argument is a named value (D139).
- A new feature that moves values around (lists, maps, text) is checked against this rule.
