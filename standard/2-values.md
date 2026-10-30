# 2. Values

## 2.1 Names

1. Every value's name carries `$`, where it is declared and wherever it is used (D31): `$count`. Names the language
   provides carry no `$`: `result`, `true`, `false`, `newline`.
2. Using a value that isn't declared is refused (D31).
   *Tests:* `undeclared_value`.

## 2.2 Declaring a value

1. A value is declared with one of (D30):

   | Written | Means |
   |---|---|
   | `var` | may change; no promise either way |
   | `var.immut` | never changes after it is set; changing it is refused |
   | `var.mut` | must change somewhere; one that nothing changes is refused |
   | `const` | known when compiling; its value may use only numbers, `true`, `false` and other `const`s; it never changes |

2. **Every value states its type, or `: infer`** (D108). `: infer` asks Geiger to work out the type and bounds, and is
   allowed on local values only. A value with neither is refused.
3. Every declaration gives a starting value with `=` (D41). A declaration without one: **not decided**.
4. `=` sets a value, at declarations and changes alike: `$count = $count + 1;` (D41).
5. Declaring a name again while the first is still visible (shadowing): **not decided**.

```
var $count: infer = 0;
var.immut $limit: int[min: 0, max: 100] = 50;
const $size: int[min: 1, max: 8] = 4;
```

*Tests:* `var_kinds`, `var_immut_never_changes`, `const_never_changes`, `var_mut_must_change`,
`const_known_when_compiling`, `missing_type_refused`, `infer_only_for_locals`.

## 2.3 Types

1. The types written so far are integers (section 3) and `bool` (section 5).
2. `void` is only a function's result (`-> void`), never a value's type (D99).
   *Tests:* `void_is_only_a_result`.
3. Text types (`string`): **not decided**. `str` is written in full as `string` (D94).
   *Tests:* `string_open`, `str_written_in_full`.
