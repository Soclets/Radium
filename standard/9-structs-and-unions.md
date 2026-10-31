# 9. Structs, unions and arrays

## 9.1 `struct`

1. A `struct` is named fields stored together as one value (D58):

   ```
   struct reading:
       time: int[min: 0, max: 86_399];
       celsius: int[min: -40, max: 125];
   end struct reading
   ```

2. A field is `name: type;`. Fields are named parts of a value, not values of their own, so they carry no `var` and
   no `$`, and their types write their bounds (D14, D58).
3. A struct is built with its fields in `[ ]` and `=`: `reading[time = 3600, celsius = 21]` (D50, D58). Each field is
   set once; each value is checked against its field's bounds. Building without every field: **not decided**. The
   order fields are written in: the oracle accepts any order (provisional).
4. A field is read by narrowing: `$r.celsius` (D26).
5. **A field's changeability follows whatever holds the struct**: the fields of a `var.immut` or a lent parameter
   never change (D58, D92). A field written `id.immut: …` never changes, whatever holds it; there is no `.mut` for
   fields (D58).
6. A struct can't contain itself: a value is stored whole, inside its holder (D58, D70).
   *Tests:* `struct_build_and_read`, `struct_field_change`, `struct_build_bounds_checked`,
   `struct_field_change_bounds_checked`, `struct_fields_follow_holder`, `struct_locked_field`,
   `struct_unknown_field`, `struct_missing_field_open`, `struct_contains_itself`, `type_not_declared`.

## 9.2 `union.tagged`

1. A value that is one of several named cases, which may carry data, is a `union.tagged` (D59):

   ```
   union.tagged status:
       ok;
       warming_up;
       failed[code: int[min: 1, max: 255]];
   end union.tagged status
   ```

2. **A case is always written with its type**: `status.ok`, `status.failed[code = 3]`. A case with no data takes no
   brackets (D59).
3. A bare `union`, C's untagged kind, is allowed only inside shields (D59).
4. A union's data is reached with `match`, never with `.` (D72).
   *Tests:* `union_built_as_a_case`, `union_case_bounds_checked`, `union_data_is_matched`, `bare_union_refused`.

## 9.3 `match`

1. `match $s:` chooses between a union's cases, with one `case …:` per case and one `end match` (D72).
2. **Every case must be covered**, once; branches never fall through (D72).
3. A case pulls out its data by field: `case status.failed[code = $c]:` gives `$c` the code. The oracle treats `$c` as
   unchanging (provisional). Pulling out a field that is a struct, union or array: **not decided**.
   *Tests:* `match_runs`, `match_must_cover_every_case`, `match_case_twice`, `match_needs_a_union`,
   `case_value_is_fixed`.

## 9.4 `array`

1. `array<element>[length: N]` is built in: a fixed number of values in a row, stored inside its holder, with no heap
   (D70). `N` is at least 1. The element is a type, so it goes in `< >`; the length is a value, so it goes in `[ ]`
   (D138). `array[element, length: N]` is refused.
2. **An array value is bare `[1, 2, 3]` where the kind is already known** (a declaration, a parameter, a field or a
   result says it's an array), and its items and length must fit that type. **Where nothing says the kind** (`: infer`)
   it is `array[1, 2, 3]`: Geiger works out the item type, and the length is the number of items. A bare `[…]` there
   is refused, since nothing is guessed, and `array[…]` where the kind is already known is refused too (D33, D39,
   D124).
3. An item is reached by `pos:` (counting from 1) or `offset:` (counting from 0); a bare index is refused (D64). Every
   index is a check site: it must land on an item. Every item stays within its element type's bounds.
4. An item changes like a field, `$xs[pos: 2] = 4;`, and only when its holder may change (D30, D58).
5. `for $x in $xs:` goes through the items front to back (D74). Going through items that are structs, unions or
   arrays: **not decided**.
   *Tests:* `array_values_run`, `array_named_where_nothing_says`, `array_bare_needs_its_kind`,
   `array_kind_not_repeated`, `array_needs_its_length`, `array_item_bounds_checked`, `array_index_needs_its_label`,
   `array_index_outside`, `array_item_change_bounds_checked`, `array_items_follow_holder`,
   `array_lent_to_a_function`, `array_inside_a_struct`, `loop_needs_a_collection`, `array_type_accepted`,
   `array_of_nothing_refused`, `array_type_old_spelling`.

## 9.5 Whole values

A whole struct, union or array goes to a new holder only with a hand-off word (section 8.2, D125). Comparing two with
`equal`, and printing one: **not decided**.
*Tests:* `struct_whole_value_needs_a_word`, `struct_equal_open`, `struct_print_open`.
