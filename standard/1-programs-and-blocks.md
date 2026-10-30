# 1. Programs and blocks

## 1.1 A program

1. A program has **exactly one `fn.main`**, where it starts (D101). Nothing may call it, and `main` is not a reserved
   name: a writer's own function may be called `main`.
   *Tests:* `one_main_only`, `main_is_not_a_reserved_name`, `old_main_block_refused`.
2. `fn.main` is written `fn.main [] -> void:` … `end fn.main`, and follows every rule for functions (section 7).
   Parameters for `fn.main`, and a result other than `void`: **not decided**.
   *Tests:* `hello`, `main_needs_return_void`, `main_parameters_open`.

## 1.2 Blocks

1. A block opens with a line ending in `:` and closes with `end` and the block's kind (D13): `end if`, `end for`,
   `end while`, `end forever`, `end fn.main`.
2. A named block names itself at both ends: `fn first_word …:` … `end fn first_word`; a named loop
   `for rows …:` … `end for rows` (D77).
3. Every `end` must match its opener exactly; a mismatch is refused (D13).
   *Tests:* `end_must_match`, `end_names_the_function`.

## 1.3 Statements

1. Every statement ends with `;`. A line break never ends a statement (D29). Block lines (`…:` and `end …`) take no
   `;`.
   *Tests:* `semicolon_required`.

## 1.4 Comments

1. A one-line comment is `//` followed by a space and text, to the end of the line, on its own line or after code
   (D87).
2. `//` with no space after it, and a lone `//` with nothing after it, are refused (D86, D87).
3. A comment that spans lines is a block: `comment:` … `end comment`. Blank lines inside belong to it (D86).
4. `#` has no meaning: outside text and comments it is refused (D87).
   *Tests:* `comments`, `comment_no_space`, `comment_lone`, `comment_block_unclosed`, `hash_has_no_meaning`.
