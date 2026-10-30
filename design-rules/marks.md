# Marks

From D28, D31, D43, D32 and D84. A mark says where a name comes from.

| Mark | Means | Example |
|---|---|---|
| `$` | a value the writer named: a variable, constant or parameter (declared and used) | `$count` |
| `@` | where an isotope (a trusted compiler extension) enters a name; each `@` is one isotope | `@float.@deci.32_bit` |
| `?` | a generic placeholder type, at its declaration and every use | `?T` |
| `std::` | the standard compound | `std::print` |
| `my.` | the writer's own function or type, **only when its name clashes with a built-in** | `my.reading` |
| capital letter | an ownership word, so every hand-off stands out: `Give`, `Copy`, `Lend`, `Lend.mut`, and `must_Give` | `close(Give $f)` |
| none | built into the language, or the writer's own name with no clash | `int`, `div`, `reading` |

- Values always carry `$`, so a value name never clashes with a language word, now or later.
- A clash between a writer's function or type and a built-in is always a compile error with an exact fix (`my.…`),
  never a silent change of meaning.
- Names the language provides carry no `$`, even values: `result`, `true`, `newln`.
- A name that can't be written plainly (a C function called `end`, a name with a dash) is quoted (D32).
- Letter case means something only for the ownership words (D84). Their lowercase spellings are refused; a
  narrowing after them stays lowercase (`Lend.mut`).
