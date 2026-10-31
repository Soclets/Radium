# Error messages

From D116 to D119 and D122. What Curie (and the oracle) say when they refuse a program or can't prove a check.

- **Only what's certain.** A message never contains advice that could be wrong.
- **Exactly one fix: show it**, as code (`newln` → `newline`).
- **More than one fix: facts only**, with no suggestions:
  - **what** is wrong;
  - **why**: the rule, with its decision number;
  - **when**: the counterexample, the values that break it, where Geiger has one.
- Curie never writes a writer's code for them when there is a choice to make (D33).
- **The place is given twice** on the first line: `file:line:column` for tools (the column counts UTF-8 bytes), then,
  after a `,`, `line N column M` for people (the column counts grapheme clusters): `refused: temps.ra:4:33, line 4
  column 32` (D117).
- **Then the code line**, a `^` pointer under the exact part (counted in grapheme clusters), and lined-up labels:
  `what:`, `why:`, `when:`, or `fix:` (D118):

  ```
  refused: report.ra:5:13, line 5 column 13
   5 |     $byte = $byte + 100;
     |             ^^^^^^^^^^^
   what: this can be 355, but $byte holds int[min: 0, max: 255]
   why:  every value stays within its bounds (D14)
   when: $byte is 255 here
  ```
- **The first word says the kind** (D119): `refused:` (a definite mistake), `unproven:` (Geiger could prove it neither
  way; it needs a hint), `check failed:` (it went wrong while running).
- **`because:` links** show what led to the error (D122): each has its place, its code line, a pointer and one fact
  (`$file was handed over here`), listed in the order they happened. Only links Curie knows for certain.
