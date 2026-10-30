# Choosing words

From D34, D58, D61 and D63.

## Check a word three ways first

Before a word is chosen, check what it means in **programming**, in **maths** and in **English** (owner, D58). The
table goes in the decision.

## Keywords: the settled programming word

Radium's readers are programmers, so a keyword is **the settled programming word**: `fn`, `struct`, `bool`. Where a
stricter meaning matters (often the maths one), **narrowing adds it**: `fn` → `fn.pure` (D51, D58).

A word whose programming meaning is split or misleading is avoided: `record` (Java/C# make it unchangeable), `enum`
(C's constants vs Rust's data-carrying cases), bare `union` (C's untagged kind, kept for shields only, D59).

## Short forms

> A short form is allowed if **the owner reads it correctly, shown cold** — on its own, without context or the full
> word. Each verdict is recorded below; once recorded, a word isn't re-judged. (D63, replacing D61's "settled for
> programmers or for everyone".)

**Allowed:** `bin`, `bool`, `char`, `config`, `const`, `deci`, `div`, `err`, `fmt`, `fn`, `immut`, `int`, `max`,
`min`, `mod`, `msg`, `mut`, `newln`, `ptr`, `std`, `stderr`, `stdout`, `str`, `struct`, `var`.

**Refused, written in full:** `buf` → `buffer`, `cfg` → `config`, `ctx` → `context`, `idx` → `index`, `len` →
`length`.

A new short form is shown to the owner cold and added to one list or the other. This is for the language's own words;
names in the implementation's code follow D19.

## Symbols or words

- **A symbol only where school maths writes it the same way, with no hidden choice**: `+`, `-`, `<`, `>`, and `x` for
  multiply (spaces around it, so `0x3` stays hex).
- **A word where there is a hidden choice, a trap, or no school symbol**: `div`, `mod`, `equal`, `not_equal`,
  `at_most`, `at_least`, `and`, `or`, `not`, `concat`.
