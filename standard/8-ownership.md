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
2. `Lend` there stores a borrow, only where the type says so (section 8.5). `Lend` into a holder of a value of its own
   is refused, and so is a value of its own into a borrow (D125, D126).
3. A hand-off word goes only in front of a whole value going to a new holder; `1 + Copy $a` is refused.
   *Tests:* `copy_into_a_name`, `give_into_a_name`, `return_gives_a_struct`, `give_into_a_field`,
   `struct_whole_value_needs_a_word`, `lend_into_an_owned_name`, `copy_into_a_borrow`, `copied_values_take_no_word`,
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

## 8.5 Stored borrows

A borrow kept in a name, a result or a field always says where it comes from (D5, D49, D126). The oracle runs a
stored `Lend` as a copy, which behaves the same, since its source can't change while it is used (provisional).

1. **A stored borrow's type is `Lend from` its source and the value's type**: `Lend from $r reading`. A borrow of a
   value that is always copied is refused (D92). The source is a `$name` in scope, `program` (it lasts until the
   program ends), or a named source in backticks. Several go in `< >`: `Lend from <$a, $b> reading`, which may borrow
   from either and lasts only while both do; several in `[ ]` are refused, since `[ ]` is for values (D127, D138).
   *Tests:* `borrow_stored_in_a_name`, `borrow_from_either`, `sources_in_square_brackets`, `borrow_from_program`.
2. **A borrow goes only where its type names its source**, even if another source would last long enough (D130).
   *Tests:* `borrow_names_its_source`.
3. **A borrow ends at its holder's last use** (D129). While it is still used, its source can't change, be given away,
   or be lent to change (D5). A use on a later trip of a loop counts. Through a stored `Lend`, nothing changes
   (D80, D135).
   *Tests:* `borrow_ends_at_last_use`, `borrow_still_used_blocks_change`, `borrow_still_used_blocks_give`,
   `borrow_still_used_blocks_lend_mut`, `borrow_used_on_the_next_trip`, `borrow_is_read_only`.
4. **A struct that holds a borrow declares its named sources** in `< >` after its name, and each field borrows from
   one of them or from `program`, never from a `$name` (D126, D138). Each source has its own name (D134) and is used
   by a field (D133). A source may carry rules, `` `table`: lasts_at_least `text` ``, joined with `and`; the same
   fact written twice is refused (D127).
   ```
   struct index <`text`, `table`: lasts_at_least `text`>:
       words: Lend from `text` reading;
       lookup: Lend from `table` reading;
   end struct index
   ```
   *Tests:* `struct_with_a_source`, `struct_source_unused`, `struct_source_twice`, `struct_source_undeclared`,
   `struct_field_borrows_a_name`, `struct_fact_written_twice`.
5. **The struct's type fills in every source, with `:`, in the declared order**:
   ``index<`text`: $article, `table`: $list>`` (D5, D126, D136). Leaving one out is refused. The fills keep the
   struct's rules: the oracle takes a name declared earlier, still in view, to last at least as long as one declared
   later, a parameter to outlast the function's own names, and `program` to outlast everything (provisional).
   *Tests:* `struct_source_left_out`, `struct_sources_in_order`, `struct_rule_kept`.
6. **A result borrows only from a parameter, a function's own source or `program`**, never from a value given to the
   function, and its body is checked against what it claims (D49, D127, D130).
   *Tests:* `result_borrows_from_a_parameter`, `result_borrows_a_local`, `result_names_another_parameter`,
   `result_borrows_a_given_parameter`.
7. **When a result borrows from a parameter, its argument is a named value**; a fresh value there is refused (D139).
   *Tests:* `borrow_from_a_fresh_value`.
8. **A function states which parameter lasts at least as long** when it fills a struct with them, as a `requires`
   line, `requires $list lasts_at_least $article;`, and every call meets it (D132).
   *Tests:* `requires_lasts_at_least`, `requires_lasts_at_least_missing`, `requires_lasts_at_least_not_met`.
9. **A borrow inside a parameter is named by the function's own sources**, declared in `< >` before its parameters,
   where placeholders also go, in the writer's order (D136, D138). Every one is used by a parameter's type; a
   parameter's type naming a `$name` is refused.
   ```
   fn shown_celsius <`t`> [$v: view<`text`: `t`>] -> int[min: -40, max: 125]:
   ```
   *Tests:* `function_own_source`, `function_source_unused`, `parameter_names_a_value_source`.
10. **A stored `Lend.mut from`** is exclusive: the value changes only through it, and its source isn't used at all while
    it lasts (D135). Not built in the oracle yet.
    *Tests:* `lend_mut_stored_not_built`.
11. **Not decided:** a parameter whose type is itself a stored borrow; a `union.tagged` case holding a borrow; a
    source said to last as long as itself; whether a `const` lasts until the program ends.
