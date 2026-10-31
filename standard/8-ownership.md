# 8. Ownership

Integers and `bool` are always copied, so none of these words apply to them (D92). Structs, unions and arrays are not
copied unless a call says `Copy`.

## 8.1 At a call

| Written | Means |
|---|---|
| `Give $x` | ownership moves to the function; `$x` can't be used afterwards |
| `Copy $x` | a separate copy is handed over; yours is untouched |
| `Lend $x` | temporary read-only access; you keep it |
| `Lend.mut $x` | temporary access that may change it; you keep it |

The ownership words start with a capital, so every hand-off stands out; lowercase spellings are refused (D80, D84).

1. **The call's word matches how the parameter holds the value** (D91, D92): `Lend` for a bare parameter, `Lend.mut`
   for `Lent.mut`, `Give` or `Copy` for `Given`.
2. **A value handed over with `Give` can't be used afterwards**, on any path (D80). The oracle also refuses handing
   off, inside a loop, a value declared outside it, since a later trip would hand it off again (provisional).
3. **`Lend.mut` lends a value that may change**, and what the function changes comes back to the lender (D80). While
   it is lent, nothing else in the same call uses it (D5).
4. **No hidden ownership** (D125): every hand-off of a value that isn't copied is written, so passing a struct, union
   or array by its `$name` with no word is refused. A value just built (`reading[…]`, `[1, 2, 3]`) has no holder yet
   and needs no word.
5. **`Give` hands off only what is owned**: a bare or `Lent.mut` parameter can be copied, but not given (D125).
   *Tests:* `ownership_words_are_capitalised`, `lend_a_struct`, `lend_mut_changes_the_lender`, `give_a_struct`,
   `given_value_is_gone`, `copy_keeps_the_original`, `call_word_matches_parameter`, `struct_needs_a_call_word`,
   `fresh_value_needs_no_word`, `cant_give_what_is_lent`,
   `lend_mut_needs_a_changeable_value`, `given_on_one_path`, `give_inside_a_loop`, `lend_mut_used_twice_in_a_call`.

## 8.2 Into a name, a result, a field or an item

1. `=`, `return`, a field and an item take `Give` or `Copy` too (D125): `var $b: reading = Copy $a;`,
   `return Give $out;`, `week[rain = Give $xs]`. A whole value there with no word is refused.
2. `Lend` or `Lend.mut` there would store a borrow, which needs lifetimes (D49): **not decided**.
3. A hand-off word goes only in front of a whole value going to a new holder; `1 + Copy $a` is refused.
   *Tests:* `copy_into_a_name`, `give_into_a_name`, `return_gives_a_struct`, `give_into_a_field`,
   `struct_whole_value_needs_a_word`, `lend_into_a_name_open`, `copied_values_take_no_word`,
   `handoff_inside_a_value`.

## 8.3 In a parameter

1. A parameter says how the function holds the value (D91): `Lent.mut` (may change it; the caller keeps it) or
   `Given` (the function owns it from then on).
2. **A bare parameter is lent**: read-only, and the caller keeps it. Writing `Lent` alone is refused (D92).
3. `Copy` has no parameter word: a copy is handed over as the function's own, so it goes to a `Given` parameter (D91).
   *Tests:* `lent_is_the_default`, `lent_parameter_is_read_only`, `given_on_a_copied_value`.

```
fn first_word [$text: string, …] -> …:          // lent
fn add_row [$t: Lent.mut transaction, …] -> void:
fn close [$f: Given open_file] -> void:
```

## 8.4 Values that must be given

1. A type narrowed with `must_Give` must be handed off with `Give` **exactly once** before it goes out of scope (D5,
   D84). Lending doesn't count; forgetting it, giving it twice, or copying it is refused. Giving it to a function
   passes the duty along; how that duty ends inside the receiving function: **not decided**.
   *Tests:* `must_give_never_given`, `must_give_cant_be_copied`, `must_give_duty_open`.
2. A borrow that comes from a parameter names it: `Lend from $text` (D49, D84).
