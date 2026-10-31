# Choosing words

From D34, D58, D61, D63 and D94.

## Check a word three ways first

Before a word is chosen, check what it means in **programming**, in **maths** and in **English** (owner, D58). The
table goes in the decision.

## Keywords: the settled programming word

Radium's readers are programmers, so a keyword is **the settled programming word**: `fn`, `struct`, `bool`. Where a
stricter meaning matters (often the maths one), **narrowing adds it**: `fn` → `fn.pure` (D51, D58).

A word whose programming meaning is split or misleading is avoided: `record` (Java/C# make it unchangeable), `enum`
(C's constants vs Rust's data-carrying cases), bare `union` (C's untagged kind, kept for shields only, D59).

## Short forms

> 1. **If programming has a settled word for it, use that word**, short or not: `bool`, `const`, `struct`, `int`.
> 2. **Otherwise, write the full word, unless the short form saves 4 or more letters *and* the owner reads it
>    correctly cold.** (D94, refining D63; three letters isn't real length, as `len` → `length` showed.)

**Kept:** `bool`, `char`, `config`, `const`, `div`, `int`, `max`, `min`, `mod`, `std`, `stderr`, `stdout`, `struct`,
`var` (settled words); `fn`, `immut`, `mut`, `ptr` (save 4 or more, read cold).

**Written in full:** `bin` → `binary`, `deci` → `decimal`, `err` → `error`, `fmt` → `format`, `newln` → `newline`,
`str` → `string`; and, from D63, `buf` → `buffer`, `cfg` →
`config`, `ctx` → `context`, `idx` → `index`, `len` → `length`.

All of D63's words have been re-judged (D94).

Only Radium's own words are judged: a writer's names (`$msg`) aren't, so `msg` has no verdict (D94).

A new short form goes through both steps and is added to one list or the other. This is for the language's own
words; names in the implementation's code follow D19.

## Symbols or words

- **A symbol only where school maths writes it the same way, with no hidden choice**: `+`, `-`, `<`, `>`, and `x` for
  multiply (spaces around it, so `0x3` stays hex). `<` and `>` need spaces too (D137).
- **A word where there is a hidden choice, a trap, or no school symbol**: `div`, `mod`, `equal`, `not_equal`,
  `at_most`, `at_least`, `and`, `or`, `not`, `concat`.
