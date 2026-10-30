# Design rules

The general rules Radium's design is judged by. Each one grew out of specific decisions, which `DECISIONS.md` records
with the owner's words; these files collect the rule itself, so a new question can be checked against it.

| Rule | In short | From |
|---|---|---|
| [Clear syntax](clear-syntax.md) | Clear once learned: no traps, no hidden choices, one meaning each | D33 |
| [Choosing words](choosing-words.md) | The settled programming word; short forms only when settled; symbols only from school maths | D34, D58, D61 |
| [Dots and underscores](dots-and-underscores.md) | `.` narrows the thing before it; `_` joins words into one name | D26, D35 |
| [Brackets](brackets.md) | `( )` runs; `[ ]` describes or holds; maths grouping is the one exception | D50 |
| [Marks](marks.md) | `$` your values, `@` an isotope, `my.` only on a clash | D28, D31, D43 |
| [Defaults](defaults.md) | A bare form's default never hides a choice and is what almost every use wants | D27, D39, D42, D57 |

When a rule changes, `DECISIONS.md` gets a new entry saying so, and the rule's file here is updated to match. A
decision that breaks a rule must say why.
