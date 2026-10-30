# 8. Ownership

Integers and `bool` are always copied, so none of these words apply to them (D92). The oracle has only those types so
far, so most of this section isn't shown by tests yet.

## 8.1 At a call

| Written | Means |
|---|---|
| `Give $x` | ownership moves to the function; `$x` can't be used afterwards |
| `Copy $x` | a separate copy is handed over; yours is untouched |
| `Lend $x` | temporary read-only access; you keep it |
| `Lend.mut $x` | temporary access that may change it; you keep it |

The ownership words start with a capital, so every hand-off stands out; lowercase spellings are refused (D80, D84).
*Tests:* `ownership_words_are_capitalised`.

## 8.2 In a parameter

1. A parameter says how the function holds the value (D91): `Lent.mut` (may change it; the caller keeps it) or
   `Given` (the function owns it from then on).
2. **A bare parameter is lent**: read-only, and the caller keeps it. Writing `Lent` alone is refused (D92).
3. `Copy` has no parameter word: a copy is handed over as the function's own, so it goes to a `Given` parameter (D91).
   *Tests:* `lent_is_the_default`.

```
fn first_word [$text: string, …] -> …:          // lent
fn add_row [$t: Lent.mut transaction, …] -> void:
fn close [$f: Given open_file] -> void:
```

## 8.3 Values that must be given

1. A type narrowed with `must_Give` must be handed off with `Give` **exactly once** before it goes out of scope (D5,
   D84). Lending doesn't count; forgetting it, or giving it twice, is refused. Giving it to a function passes the duty
   along.
2. A borrow that comes from a parameter names it: `Lend from $text` (D49, D84).
