# Choosing words

From D34, D58 and D61.

## Check a word three ways first

Before a word is chosen, check what it means in **programming**, in **maths** and in **English** (owner, D58). The
table goes in the decision.

## Keywords: the settled programming word

Radium's readers are programmers, so a keyword is **the settled programming word**: `fn`, `struct`, `bool`. Where a
stricter meaning matters (often the maths one), **narrowing adds it**: `fn` → `fn.pure` (D51, D58).

A word whose programming meaning is split or misleading is avoided: `record` (Java/C# make it unchangeable), `enum`
(C's constants vs Rust's data-carrying cases), bare `union` (C's untagged kind, kept for shields only, D59).

## Short forms

> A short form is used only when it is **already settled for programmers** (the same spelling across major
> languages) **or for everyone** (maths notation, everyday use). Either one is enough. Otherwise, the full word.

| Short form | Settled for | |
|---|---|---|
| `int`, `bool`, `struct`, `const`, `var`, `std` | programmers | ✅ |
| `fn` | everyone (the Fn key) | ✅ |
| `min`, `max`, `mod` | everyone (maths, everyday) | ✅ |
| `len`, `newln` | neither | ❌ — `length`, `newline` |

This is for the language's own words. Names in the implementation's code follow D19.

## Symbols or words

- **A symbol only where school maths writes it the same way, with no hidden choice**: `+`, `-`, `<`, `>`, and `x` for
  multiply (spaces around it, so `0x3` stays hex).
- **A word where there is a hidden choice, a trap, or no school symbol**: `div`, `mod`, `equal`, `not_equal`,
  `at_most`, `at_least`, `and`, `or`, `not`, `concat`.
