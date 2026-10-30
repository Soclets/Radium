# Defaults

From D27, D39, D42, D57 and D92.

## When a bare form has a default

A bare name has a default **only where one is genuinely sensible** (owner, D27). A sensible default:

1. **never hides a choice** — `div` is exact rather than secretly rounding; `mod` is only allowed where every
   definition agrees; an inexact literal is an error until marked;
2. **is what almost every use wants and expects** — `and`/`or` short-circuit; a parameter is `var.immut` and lent (D92);
   `std::print` prints the shortest way to the screen.

Where no default is sensible, there is none: a bare `int` takes its width and sign from its bounds, never a fixed 32
bits.

## Spelling out a default

- A default the **language** fixes is never spelled out: `div.exact` is refused, "write `div`"; a parameter's `Lent`
  is refused, "write the bare parameter".
- A collection's default may be spelled out to pin it against later changes — **unless** its writer declares
  `defaults.fixed`, a promise it never changes; then spelling it out is refused too. `std` uses `defaults.fixed`.
- Narrowings that happen to agree in one place (`div.floor` and `div.trunc` on non-negatives) are not refused.

## Narrowing from a default

The default is the bare form; narrowing states something different, and the compiler checks it: `var` →
`var.immut`, `labels` → `labels.any_order`, `fn` → `fn.pure`.
