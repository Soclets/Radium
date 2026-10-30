# Design rules

The general rules Radium's design is judged by. Each one grew out of specific decisions, which `DECISIONS.md` records
with the owner's words; these files collect the rule itself, so a new question can be checked against it. Rules for the code that implements
Radium are in [`implementation-rules/`](../implementation-rules/).

| Rule | In short | From |
|---|---|---|
| [Clear syntax](clear-syntax.md) | Clear once learned: no traps, no hidden choices, one meaning each | D33 |
| [Choosing words](choosing-words.md) | The settled programming word; short forms only if the owner reads them cold; symbols only from school maths | D34, D58, D63 |
| [Dots and underscores](dots-and-underscores.md) | `.` narrows the thing before it; `_` joins words into one name | D26, D35 |
| [Brackets](brackets.md) | `( )` runs; `[ ]` describes or holds; maths grouping is the one exception | D50 |
| [Marks](marks.md) | `$` your values, `@` an isotope, `my.` only on a clash, a capital for ownership words | D28, D31, D43, D84 |
| [Built in or imported](built-in-or-import.md) | Compound if it can be written in Radium; built in if identical on every target; otherwise an isotope | D22, D23, D104 |
| [Defaults](defaults.md) | A bare form's default never hides a choice and is what almost every use wants | D27, D39, D42, D57 |

When a rule changes, `DECISIONS.md` gets a new entry saying so, and the rule's file here is updated to match. A
decision that breaks a rule must say why.
