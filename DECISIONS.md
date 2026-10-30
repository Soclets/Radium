# Radium — design decisions

The owner decides; radium design claude proposes and records. Each entry says what was chosen and whose idea it was.
Numbers are permanent: implementation code points at them with `trace D<n>` tags (D18), so a decision is
never renumbered. A changed decision gets a new entry that says which one it replaces.

Identity (repo description, `Soclets/Radium`): *"Radium: slowly kills you via the steep learning curve, and slow
compile times. Glow in the dark via the clear syntax. It also has a safety radius."* A steep learning curve and slow
compiles are accepted, never an argument against an option. Clear syntax is required. Final products should be fast
(D81).

Licences (D65): Apache-2.0 WITH LLVM-exception for code; CC BY 4.0 for the Markdown text.

Syntax in code samples below is placeholder unless a decision says otherwise.

---

## D1 — Purpose (2026-10-29)

Radium is for final products that cannot have any mistakes: high-assurance, safety-critical in spirit. No real
certification (e.g. DO-178C) will be sought, and Radium is not recommended for professional use.

## D2 — Checking model: Precog, but more diabolical (2026-10-29)

Owner: "Precog (ask CyborgPL design claude) but more diabolical." Starting point is CyborgPL's Precog
(`/Users/ts/CyborgPL/design/precog.md`): a whole-program compile-time prover in three layers (execute known values;
track outside values as sets and intervals; ask the rest as bit-precise SMT questions to Z3 and CVC5). D3–D12 are
where Radium goes further.

## D3 — The writer helps the prover (2026-10-29)

Owner: contracts, and easier ways than Precog's for the writer to help the prover (Precog has only guardrails and no
invariant syntax).

## D4 — The proof toolkit, and ownership (2026-10-29)

Owner: "All of them + Ownership." Radium has all seven writer aids:

1. preconditions (`requires`)
2. postconditions (`ensures`)
3. loop invariants (`invariant`)
4. assertions as proof hints, never runtime checks (`assert`)
5. ghost code, erased from the binary
6. lemmas
7. termination measures (`decreases`)

The prover still infers as much as Precog; the aids are for where it gets stuck. Ownership, no garbage collector.

## D5 — Ownership style (2026-10-29)

Proposed by design claude, owner: "yes". Rust-style borrowing (shared and exclusive borrows, which can be stored in
structs and returned), plus linear types (some values must be used exactly once: closed, committed, answered).
Lifetimes are written out wherever a borrow leaves a function or sits in a struct, not hidden by inference.

## D6 — The safety radius: shields (2026-10-29)

Proposed by design claude from the repo description, owner: "yes". All unproven or dangerous code (C calls, raw
pointers, memory-mapped I/O, inline assembly) lives only inside a shield, which carries a written reason.

```
shield sensor_driver:
    reason "talks to the temperature chip over raw memory-mapped I/O"
    fn read_celsius() -> i32[min: -40, max: 125]:
        …
    end fn read_celsius
end shield sensor_driver
```

Three walls:

- **Entry** — callers outside are proven to meet the shield's `requires`.
- **Exit** — the shield's `ensures` are checked at the door at run time; a violation is a failure the caller must
  handle. Outside the shield, the `ensures` are facts the prover builds on.
- **Containment** — raw pointers and unchecked borrows can't cross.

Nothing outside a shield can be unproven, so there is no scattered `no_check`. The build report lists every shield
with its reason, its size (its radius) and its wall contracts. Sources: Rust's `unsafe`, SPARK's `SPARK_Mode`,
CyborgPL's checked ranged extern returns, trusted-computing-base minimisation.

Open: the keyword; a cap on shield size; whether an exit wall may ever be trusted instead of checked.

## D7 — Termination (2026-10-29)

Proposed by design claude, owner: "yes". Every ordinary loop and recursion must be proven to end; the prover infers
the measure where it can and asks for `decreases` where it can't. The only way to run forever is an explicit
`forever` loop, and each of its trips must be proven to end.

## D8 — Memory (2026-10-29)

Proposed by design claude, owner: "yup".

- **Stack**: always proven. The compiler computes worst-case stack use (recursion depth from D7's measures × frame
  sizes) against the target's declared stack size; a possible overflow is a compile error.
- **Heap**: the target declares a heap budget. Where the prover bounds allocations within it, allocation cannot fail
  and needs no handling. Elsewhere allocation returns a failure the caller must handle (D11).

Out-of-memory is never a silent crash.

## D9 — Build modes (2026-10-29)

Owner's own design.

| Mode | Prover | Runtime checks in the binary | Purpose |
|---|---|---|---|
| 1 | Not run | At every check site | The fast loop while writing |
| 2 | Must prove everything (unproven = refused) | At every check site, kept anyway | Testing; a check that fires is a prover bug |
| 3 — final | Must prove everything | None | The product |

Owner: "runtime perf doesn't matter much if it's not final product." Shield exit walls (D6) are checked in every mode.

## D10 — Marking non-final binaries (2026-10-29)

Owner chose option 3 of four. Mode 1 and mode 2 binaries carry a metadata stamp of their mode (readable by a tool such
as `curie inspect`, D68) and print one warning line to stderr every time they start, e.g.
`⚠ Radium mode 1 build — unproven, not a final product`. Only a final binary starts silently.

## D11 — Run-time failures (2026-10-29)

Proposed by design claude, extended by the owner with CyborgPL's handler collections, owner: "yes".

- **No exceptions, no catch-all error type.**
- **Precise failure sets** — every signature lists exactly the failures it can produce; the compiler checks the list
  is exact (nothing missing, nothing impossible).
- **Failure contracts** — each failure says when it happens (`fails not_found exactly when key not in list`),
  prover-checked. A caller who has proven the condition false need not handle that failure.
- **Handler collections** (from CyborgPL's `collection.handler`) — named, reusable groups of handler arms, used at a
  call site as if written there (arms may `return`, `break`, `continue`, `fail` from there). A handler must cover the
  failures still possible at that site after proofs; extra arms are allowed and reported as dead there.

A final build has no panics: the only early stop is a declared, handled failure.

## D12 — How far the prover is trusted (2026-10-29)

Proposed by design claude, owner: "yes". In the final build a site is proven only when both Z3 and CVC5 answer
`unsat` and the proof certificate (CVC5's proof output) is re-checked by a small, separate proof checker (the de
Bruijn criterion). Fallback: where no certificate can be produced or checked, both solvers agreeing still counts, and
the report marks the site "solver-agreed, not certificate-checked". Disagreement = unproven.

## D13 — Block delimiters (2026-10-29; `main` replaced by D101: `fn.main`)

Owner's own design.

- A block opens with a line ending in `:` and closes with `end …`.
- Named blocks state their kind and name at both ends: `fn read_celsius() -> …:` … `end fn read_celsius`;
  `shield sensor_driver:` … `end shield sensor_driver`.
- Unnamed blocks state their kind: `if i > 5:` … `end if`; `for …:` … `end for`.
- `main` is special: `main:` … `end main`, no kind word. (Spelled `MAIN` at first; owner's sketch, 2026-10-29,
  settled it as lowercase `main`.)
- The compiler checks that every `end` matches its opener exactly.

Contracts are the first lines of the body (D53).

## D14 — Integers (2026-10-29; width amended by D27; local bounds need `infer`, D108)

Owner: a fixed width written by the writer, refined by a range — conceptually `i32[min: 100, max: 200]` (owner: "not
syntax"; the spelling is open). The prover checks every value stays within the range and the width.

- No integer whose size depends on the platform (no C `long`). A pointer-sized type takes its size from the declared
  target.
- Proposed by design claude, owner: "yes": ranges are **required at the edges** (parameters, return values, struct
  fields, globals) and optional on locals, where the prover infers them. Functions are proven once against their own
  signatures, not per call site.

## D15 — Implementation languages (2026-10-29)

Owner: code written by the Radium project is **Rust or C++ only**. Tools written by others (Z3, CVC5, LLVM,
third-party proof checkers) may be dependencies. Lean 4 and CompCert-style toolchains were considered for proving the
compiler itself and declined for our own code.

## D16 — Which part is in which language (2026-10-29)

Proposed by design claude, owner: "yes".

| Part | Language |
|---|---|
| Compiler: parser, type and ownership checker, prover, lowering | Rust |
| LLVM wrapper, only what LLVM's C API lacks; custom LLVM passes (D17) | C++, kept minimal |
| Runtime linked into every program | `no_std` Rust |
| Proof checker (D12) | Rust, proven correct with Verus; cross-checked by an independent external checker (e.g. Carcara) |
| Oracles (independent interpreter, etc.) | Rust, written separately from the compiler, no shared code |

## D17 — Backend (2026-10-29)

Owner: **LLVM only**, for all three modes (mode 1 at `-O0`). CompCert (proven to assembly, fewer bugs) was considered
and declined: non-commercial license, slower code; Radium is Apache 2.0 and not recommended professionally. Safeguards:

1. translation validation of optimizer passes (Alive2 or our own);
2. differential testing against an independent oracle interpreter;
3. mode 2's runtime checks as a safety net;
4. a conservative final pipeline: a hand-picked, frozen list of LLVM passes, part of the build's identity; risky
   passes (e.g. InstCombine, the loop vectorizer) only with evidence.

Custom passes are allowed as additions (owner). Preference: Radium-specific optimizations run on Radium's own IR
before the prover, which re-checks the result; C++ LLVM passes after the prover only when they need LLVM's view,
under the same safeguards. Proven facts are handed to LLVM (`nsw`, `nuw`, `!range`, `noalias`).

## D18 — Comment markers in the implementation (2026-10-29)

Owner asked for comments that let scripts find each part; design claude proposed two kinds, owner: "yes".

- **Section markers**, mirroring D13:
  ```rust
  // section prover.layer2.loops:
  …
  // end section prover.layer2.loops
  ```
- **Trace tags**, linking code and tests to these decisions:
  ```rust
  // trace D7: forever loops must prove each trip ends
  ```
- A checker script in the test suite fails the build on an unclosed or misnamed section, or a tag naming a decision
  that doesn't exist; it reports every decision with no implementation or no test as a gap.

## D19 — Naming inside the implementation (2026-10-29)

Owner asked about naming; design claude proposed, owner adjusted the abbreviation rule.

- **Case**: Rust's standard conventions (enforced by the compiler's lints); C++ in LLVM's own style, so the wrapper reads
  like the LLVM code beside it.
- **Full words.** An abbreviation is allowed only when it is well known and saves real length (owner: "`len` saves two
  letters, `id` saves a bajillion"). The allowed list is in `TERMS.md`; anything not on it is written out.
- **Names say what, not how**: `proven_range`, not `tmp2`.
- **One vocabulary**: every concept has exactly one name, the same in the design docs, code, comments and error
  messages (`Shield`, `ExitWall`, `CheckSite`). `TERMS.md` lists them, and names no longer used for a concept.
- **Module paths match D18's section markers**: `// section prover.layer2.loops:` lives in `prover::layer2::loops`.

## D20 — Implementation code style (2026-10-29)

Proposed by design claude, owner: "yes".

- **Tools**: `rustfmt` (default settings); `clippy` at its pedantic level, with warnings as errors; `clang-format` in
  LLVM's style for the C++.
- **`unsafe` Rust only in named modules**, each with a written reason — D6's shields, applied to the implementation.
- **No `unwrap()` or `expect()` in trusted code** (the prover, the proof checker, the ownership checker): every failure
  is a handled error, as in D11.
- **A doc comment on every public item**, saying what it does and, in trusted code, what it assumes.
- **Comments explain why, not what.**

## D21 — Words for people (2026-10-29)

Proposed by design claude, owner: "yes". Same as CyborgPL: **owner** (decides the design), **author** (the founder),
**authors** (the owner and the sessions), **contributors** (outside people), **writer** (writes a Radium program),
**user** (uses a writer's final product). Listed in `TERMS.md`.

## D22 — Floats and decimals are imported (2026-10-29)

Owner: "floats & decimals are an import." After learning that integers work on every target with identical results
(emulated where the hardware lacks a width, multiply or divide), while many small chips have no float hardware or only
`f32`. Integers (D14) are built into the language; floating-point and decimal/fixed-point types come only from an
import. What those types guarantee, and how a literal like `1.5` behaves without the import, is open.

## D23 — Two kinds of import (2026-10-29)

Owner: there are two kinds of import:

- an **isotope** — a compiler extension: adds something the compiler itself must know about. Trusted, not proven —
  like an isotope, the radioactive, handle-with-care kind.
- a **compound** — a library written in Radium: ordinary Radium source, proven like any other code. Stable.

Names proposed by design claude, owner: "yes". (CyborgPL's words for the same two are *limb* and *kit*.) Principle
from the discussion: anything that can be a compound should be one, so isotopes stay the rare exception. Under D22,
floats would be an isotope; decimals and fixed-point can be a compound built on D14's integers. How an import is
spelled in code is open.

## D24 — What the float isotope guarantees (2026-10-29)

Proposed by design claude, owner: "opt 2". Floats (an isotope, D22/D23) are ranged like D14's integers: conceptually
`f64[min: -40.0, max: 125.0]`. Producing NaN or infinity is a check site, so a final build is proven never to produce
either; rounding still happens. A raw float from C enters only through a shield's exit wall (D6), checked there for
NaN, infinity and range. No raw IEEE float type exists outside shields. Owner, "yes": as with integers (D14), ranges are
required at the edges (parameters, return values, struct fields, globals) and optional on locals. Open: which widths
(`f32`, `f64`, others).

## D25 — Decimal literals (2026-10-29)

Proposed by design claude, owner: "yes". With no float isotope or fixed-point compound imported, a literal like `1.5`
is a compile error naming the imports that could hold it. With one imported, the literal becomes the type of wherever
it goes. Owner: a literal that can't be stored exactly (`0.1` in a float, `0.125` in hundredths) is
**an error by default**; a mark on the literal allows it. Proposed by design claude, owner: "yes": the bare mark rounds to the
nearest storable value; the mark may also name a direction (always down / always up), which the prover then uses as a
fact (e.g. stored value ≤ the written one). The build report lists every marked literal with the value actually stored.
Open: the mark's spelling (e.g. `~0.1`).

## D26 — `.` narrows (2026-10-29)

Owner's rule: a `.` narrows the thing before it. `int` → `int.signed` → `int.signed.32_bit`; `str` → `str.utf8`;
`std::print` → `std::print.shortest` → `std::print.shortest.stdout`. So there are no short aliases that stand outside
the chain (no `i32` beside `int.signed.32_bit`).

Owner: `std` is a compound of **function collections**, as in CyborgPL's `collection.fn`: a collection has its own
function, members (`print.stdout`) and presets (fixed arguments), and narrowing picks a member or preset. A bare
collection name is its own function, which the collection's writer defines. What a built-in type name means when it
is not narrowed all the way (`int` alone) is open.

## D27 — A bare name's default; `int`'s width comes from its range (2026-10-29)

Owner: a bare name has a default only where one is genuinely sensible ("I genuinely don't think picking 32-bit for
`int` is a sensible default, though `std::print` meaning `std::print.shortest.stdout` may be"). A function
collection's writer decides its own default (D26).

Proposed by design claude, owner: "yes": **amends D14**. A bare `int` takes its width from its range by a fixed rule:
`int[min: 100, max: 1000]` gets the smallest width that holds the range. Narrowing (`int.signed.32_bit[…]`) sets the
width exactly, for hardware registers, file formats and C. The build report lists every width chosen this way. Ranges
are still required at the edges (D14). Owner, "yes": **signedness also comes from the range** — unsigned when `min`
is 0 or more, signed when `min` is negative — then the smallest width that holds it (`int[min: 0, max: 255]` → 8 bits
unsigned; `int[min: -1, max: 255]` → 16 bits signed). Mixing signed and unsigned is safe because the prover checks
every value against its range; nothing converts silently.

## D28 — `@` marks what an isotope provides (2026-10-29; `%` changed to `@` on 2026-10-30)

**Changed 2026-10-30** (owner proposed, "yes"): the mark is **`@`**, not `%`: `@float.bin.32_bit`,
`@float.@deci.32_bit`. `%` reads as remainder or a format placeholder in other languages; `@` already means "handled
specially by the compiler" in several (annotations, decorators, attributes). The rules below are unchanged, with `@` in
place of `%`.

Owner's sketch:

```
main:
var $f32: %float.bin.32_bit = 10;
var $d32: %float.%deci.32_bit = 0.1;
end main
```

A name provided by an isotope (D23) carries `%`, so trusted, compiler-extension things are visible wherever they are
used. Here `%float` is the float isotope (`.bin.32_bit`: binary, 32 bits) and `%deci` narrows it to decimal floating
point. `0.1` is exact in a decimal float, so it needs no D25 mark.

Proposed by design claude, owner: "yes": **`%` marks where an isotope enters the chain.** Everything narrowed after it
belongs to that isotope until another `%` shows a different isotope entering. So `%float.bin.32_bit` is all the float
isotope; in `%float.%deci.32_bit`, `%deci` is a separate isotope extending `%float`, and `32_bit` is deci's own
narrowing. Each `%` means a new piece of trusted code starts here; counting them counts the isotopes a name depends
on.

## D29 — Semicolons (2026-10-29)

From the owner's sketches; design claude proposed keeping them, owner: "yes". Every statement ends with `;`. Block
lines don't: `…:` opens a block, `end …` closes it. A line break never ends a statement, so there are no
automatic-semicolon rules.

## D30 — Named values: `var`, `var.immut`, `var.mut`, `const` (2026-10-29)

Owner's design (option B), following D26. Owner: "`var` variable, vary."

- `var` — a value that may vary; no promise either way, nothing forced.
- `var.immut` — forced never to change after it is set (it may be set from run-time input). Changing it is a compile
  error.
- `var.mut` — forced to change somewhere; one that nothing changes is a compile error.
- `const` — a value known at compile time, folded in by the prover. Not a `var`: it never varies at all.

Possible help, not language: the build report or tooling may mark which bare `var`s actually change.

## D31 — `$` on value names (2026-10-29)

From the owner's sketches; design claude proposed, owner: "yes". Every value name (variables, constants, parameters)
carries `$`, where it is declared and everywhere it is used: `var $str: str.utf8 = "Hello";` … `concat $str`. A value
name can never clash with a language word (`$i32`, `$str` are fine), so new keywords can be added later without
breaking any program. Function and type names stay bare (D32).

Owner, "yes": **parameters carry `$` too**. Owner, by D27's sensible-defaults rule: **a parameter is `var.immut` by
default** — it can't be reassigned inside the function. Changing the caller's data through an exclusive borrow (D5) is
the borrow's business, not the parameter's. Owner, "yes":
a parameter may narrow differently (`var $count`, `var.mut $count`) to be reassigned inside the function; `var.mut`
is still forced to change (D30).

## D32 — Reserved words and quoted names (2026-10-29; reserved list for functions replaced by D43)

Proposed by design claude after the owner asked whether `$""` helps with odd names; owner: "yes".

- Function and type names stay bare (`std::print…`). Radium publishes a generous list of reserved words early,
  including words it may use one day; a bare name can't be one of them.
- A **quoted name** reaches anything a plain name can't: a C function called `end`, old code after a new keyword is
  reserved, names with dashes or spaces. `$"…"` is a value with an odd name; the form for a function or type with an
  odd name is quotes without `$` (exact spelling in the syntax phase), since `$` marks values.
- Open: what is allowed inside the quotes.

## D33 — What "clear syntax" means (2026-10-30)

Owner: "there should be a line", shown with CyborgPL inline-assembly code no syntax could make readable to a
non-programmer; "am I gonna make CyborgPL become Python? I don't think so." Design claude proposed the line, owner:
"yes". **Radium is clear to someone who has learned Radium**, not readable without learning. Clear means:

- **no traps** — nothing that looks like one thing and does another;
- **no hidden choices** — rounding, sign rules and defaults are visible or refused;
- **one meaning per symbol or word.**

## D34 — Symbols and word operators (2026-10-30)

Owner, after "as a non programmer, I find myself hard to read their symbols"; design claude proposed, owner refined.

- **Symbols only where school maths writes them the same way, with no hidden choice**: `+`, `-`, `<`, `>`, and **`x`**
  for multiply (owner: "If you want school maths, you use `x` as the letter"). `x` needs spaces around it, so `0x3`
  is always a hex number and `0 x 3` always multiplication. `$` on values (D31) keeps `$a x $b` unambiguous.
- **Words where there is a hidden choice, a trap, or no school symbol**: `div`, `mod`, `equal`, `not_equal`
  (owner's spelling), `at_most`, `at_least`, `and`, `or`, `not`, `concat`.

## D35 — `.` narrows, `_` joins (2026-10-30)

Owner asked why `not.equals` wasn't `_`. **`.` narrows** (D26): a specific version of the thing before it
(`div.floor`). **`_` joins** words into one name (`not_equal`, `at_most`, `32_bit`).

## D36 — `newline` (2026-10-30; replaced by D63: `newln`)

Owner: `newln` becomes `newline` (by the owner's `len` rule, D19: two letters saved isn't worth decoding).
**Replaced by D63** the same day: under the new abbreviation rule the owner chose `newln`.

## D37 — Division (2026-10-30)

Proposed by design claude, owner: "yes". **Bare `div` is exact**: a division that could leave a remainder is a check
site, and the prover must show the remainder is always zero. Rounding is only by narrowing: `div.floor` (down),
`div.trunc` (toward zero), `div.ceil` (up). Same idea as D25: nothing rounds unless the code says how.

## D38 — Remainder (2026-10-30)

Proposed by design claude, owner: "yes". **Bare `mod` is allowed only where both numbers are proven non-negative**,
where every definition agrees. Otherwise narrow: `mod.floor` (sign of the divisor), `mod.trunc` (sign of the
dividend), `mod.euclid` (never negative). `div.floor` pairs with `mod.floor` and `div.trunc` with `mod.trunc`, so
dividend = divisor × quotient + remainder always holds.

## D39 — Spelling out a default (2026-10-30)

Owner asked whether narrowings that are the same should be refused; design claude proposed, owner: "yes".

- **Spelling out a default the language fixes is refused** — `div.exact` is an error saying "write `div`", since that
  default can never change.
- **A function collection's default may be spelled out** (`std::print.shortest.stdout`), since its writer may change
  the default later. *Refined by D57:* not when the writer declares `defaults.fixed`, which `std` does — so
  `std::print.shortest.stdout` is refused.
- Narrowings that happen to agree in one place (`div.floor` and `div.trunc` on proven non-negatives) are not refused:
  they differ in general and say which rule is meant.

## D40 — `and` and `or` short-circuit (2026-10-30)

Owner asked what's consistent, noting a narrowing either way would be. Design claude proposed, owner: "yes". **Bare
`and` / `or` short-circuit**: the right side runs only when needed, and the prover uses the left side as a fact when
checking the right (`$i < length($list) and $list[$i] equal 0` is proven safe). A narrowing (spelled in the syntax
phase) always evaluates both sides, e.g. for timing-safe code.

## D41 — `=` means "set to" (2026-10-30)

Proposed by design claude under D33, owner: "yes". `=` sets a value, at declarations and changes alike:
`var.mut $count: int[min: 0, max: 100] = 0;` … `$count = $count + 1;`. There is no `=`/`==` trap because comparing is
the word `equal` (D34). (`change … to …` was considered and dropped.)

## D42 — Pattern for defaults (2026-10-30)

Design claude's summary of D25, D27, D31 and D37–D40, shown with D40 (owner said "yes" to D40): a bare form's default **never hides a choice**, and is **what
almost every use wants and expects**.

## D43 — Your own functions are marked where used (2026-10-30; mark only on clashes, amended the same day)

Owner: "you can name your fn shit anything, it just needs a mark like `my.x` or something to use." **Replaces D32's
reserved list for function names.** A function may have any name, `x` or `if` included; every use of the writer's own
function carries a mark:

```
fn x($a: int[…], $b: int[…]) -> int[…]:
    …
end fn x

main:
    var $area = $width x $height;        // x: multiply
    var $mine = my.x($width, $height);   // my.x: the writer's own function
end main
```

A built-in word and a writer's name can never clash, so new keywords never break old code, and every call shows where
it comes from (`my.` own, `std::` the standard compound, `%` an isotope; now `@`, D28). Quoted names (D32) remain for odd
characters and C names. Owner: if `my.` gets too heavy, it could become one symbol, or be needed only for names that
clash.

**Amended 2026-10-30** (owner, on marking types too: "That's a bit too heavy"; then "opt 2" after seeing a
one-symbol `@` mark): **the mark is needed only when a name clashes.** The writer's own functions and types are bare
(`var $r: reading = read_sensor();`). When a name is the same as a built-in one, `my.` resolves it (`my.reading`). If
a later Radium adds a built-in with the same name as the writer's, old code gets a compile error saying to write
`my.…`, never a silent change of meaning; a tool can make that fix. Functions and types follow the same rule.

## D44 — C++ rules (2026-10-30)

Owner raised C++ as the place undefined behaviour could bite; design claude proposed, owner: "yes". Adds to D16
and D20.

1. **Small** — C++ only where LLVM's C API falls short, plus custom LLVM passes (D17).
2. **Modern and hardened** — C++23 (`std::span`, `std::expected`, concepts); libc++ hardening on in every build, so
   out-of-bounds access on standard containers traps; no exceptions, no RTTI, matching LLVM's build. C++26 once clang
   supports the parts used.
3. **Restricted** — banned: raw `new`/`delete`, C-style casts, `reinterpret_cast`, unions, pointer arithmetic,
   uninitialised variables, `goto`, variadic functions. Enforced by `clang-tidy` (C++ Core Guidelines, `bugprone`,
   `cert` checks) and compiler warnings, all as errors.
4. **Tested for UB** — UBSan, ASan and TSan on every test run; fuzzing through the C interface.
5. **Shielded** — the wrapper exposes only small `extern "C"` functions, each with a written contract (pointer
   ownership, lifetime, allowed values). Rust wraps each once in a safe function that checks the contract; the rest of
   the compiler never touches C++ directly. D6's shield, applied to the compiler.

Added the same day (owner chose option 1 of three): **Coverity Scan** (free for open-source projects) as a second,
independent analyser. The **AUTOSAR C++14 guidelines** were added as a rule source, then **dropped** the same day
(owner: "Drop"): their terms allow use "for informational purposes only", and other use needs AUTOSAR's written
permission. Our rules are clang-tidy's checks (LLVM's license) plus D44's own list, written in our own words.
The **C++ Core Guidelines** document's license (checked 2026-10-30) allows copying, use and modification "for your
personal or internal business use only" and says nothing about AI; it grants no right to redistribute. So it may be
read as a reference, but its text is not copied into the public repo. clang-tidy's `cppcoreguidelines` checks are
part of LLVM and unaffected. Commercial MISRA checkers (PC-lint Plus, Perforce QAC, etc.) are quote-only and not used. The
owner is checking the MISRA C++:2023 document itself (reportedly about £15 per person). Found the same day: MISRA's
terms of sale (clause 3.2.4) forbid letting any AI system process or access the documents "or any content from the
Documents", which clashes with Radium being written largely by Claude sessions. Owner: "MISRA doc is
definitely not a good idea anymore" — **MISRA's documents are not used**, bought or otherwise, and its rule text must
not enter the project.

## D45 — Branches and a local merge bot (2026-10-30; amended by D89: the design session commits to `main`)

Owner suggested a merge bot and automatic tests, noting CyborgPL's tests are slow enough to run locally; design
claude proposed the shape, owner: "yes". No pull requests: sessions work with the owner present, so a PR would only
ask for the same approval twice.

1. A session commits to **its own branch** (e.g. `compiler/termination-check`), never to `main`, and pushes that
   branch.
2. A **local merge bot** on the owner's Mac notices the new commit, merges it with the latest `main` in a separate
   scratch folder, and runs the **full** test suite there.
3. It reports the result on the commit through GitHub's commit-status API (`radium/full-tests: passed` / `failed`,
   with a summary). GitHub runs nothing; the tick means the whole suite passed. (A tick for quick checks only was
   rejected, owner: "What's the green tick for, if it doesn't say everything passed".)
4. Passed: the bot moves `main` forward and pushes it. Failed: `main` is untouched; the branch is fixed and pushed
   again.

The bot is the only thing that merges into `main`. No GitHub Actions and no self-hosted runner, so nothing from GitHub
runs on the Mac. Takes effect with the first code; until then the design session commits and pushes when the owner
says so.

## D46 — Languages in the repo, and the two builds (2026-10-30)

Proposed by design claude, owner: "yes"; CMake by the owner ("CMake when C++ comes").

| Kind of file | Language |
|---|---|
| Compiler, runtime, proof checker, oracles | Rust |
| LLVM wrapper and custom passes | C++ |
| Building the C++ | CMake, from the first C++ file |
| `std` compound, examples, tests | Radium |
| Tools and scripts (merge bot, D18 checker, generators, benchmarks) | Rust; no shell, no Python |
| Docs | Markdown |
| Config | TOML (Cargo), and only the formats a tool requires (`.clang-tidy`, `.clang-format`, `.gitignore`) |

Anything else (Python, shell, Go, …) needs the owner's explicit OK as an exception.

**The two builds stay separate** (owner, on running CMake through Cargo: "What's next? Compiling SPARK via Cargo?"):
CMake builds the C++ into a library; `cargo build` builds the Rust and links to it where CMake left it. Neither drives
the other. The order (CMake first, then Cargo) is written in the README and run by the merge bot (D45).

## D47 — When `std` is built (2026-10-30)

Proposed by design claude, owner: "opt 3". `std`'s shape is **sketched on paper during the design phase**: its
function collections, defaults, contracts and failure sets. That tests the language's rules on a real library before
anything is built. Once the compiler exists, a dedicated `std` session writes it alongside the compiler, piece by
piece as each part of the language works.

## D48 — The oracle (2026-10-30)

Owner: the oracle is "for getting the design to be enforced by something that can't be 'wrong'", before memory (the
owner's and the session's) drifts. So it is **written by the design session**, as a runnable form of these
decisions, growing as decisions are made; each one gets tests tagged `trace D<n>` (D18), written as small Radium
programs the owner can read. Rust (D46), in `oracle/`. The compiler session never reads the oracle's source, only its
tests, so the two stay independent implementations. **Not started until the syntax phase is done** (owner: "don't
build it now. We haven't done syntax yet"); replaced by D93: it starts with the settled parts.

Revised the same day, owner: "yes". **The oracle has its own `std`, written in Rust**, built before Radium's real
`std` exists (owner: CyborgPL "didn't get `std` written in itself for dozens of days"). It is D47's sketch made
runnable: `std`'s collections, defaults, contracts and failure sets pinned down as behaviour. It is **kept after the
real `std` exists**, as a second independent implementation, so a disagreement catches a bug in either; the cost is
keeping the two in step. Isotopes and the bridge to the operating system likewise get their own oracle versions. In
the syntax phase, function collections move up beside declarations, since the oracle's `std` needs them early.

## D49 — Lifetimes name their source (2026-10-30)

Owner proposed `` `a` `` for lifetime names; design claude raised that backticks fight Markdown (all docs) and some
keyboards, and proposed naming the source instead; owner: "yes". A borrow that comes from a parameter **names that
parameter**: `-> lend from $text str.utf8` (placeholder words). No abstract lifetime labels to match up. A **named
lifetime** is used only where there is no parameter to point at, mainly a struct holding a borrow; its spelling is
open but is not a backtick. Close to Swift's lifetime dependencies. The ownership words themselves are syntax step 5.

## D50 — `( )` runs, `[ ]` describes or holds (2026-10-30)

Owner pointed to CyborgPL's rule (its design session briefed the details); design claude proposed adopting it with
grouping declared up front; owner: "yes".

- **`( )` runs something: a call.** `first_word("hello there", 20)`, `std::print("hi", newln)`.
- **`[ ]` describes or holds everything else:** a definition's parameters (`fn first_word [$text: str.utf8, $limit:
  int[min: 1, max: 80]] -> str.utf8:`), types and a function's type, ranges (`int[min: 1, max: 99]`), indexes,
  building a struct or a failure, a loop's range.
- **Both:** a conversion describes its target and passes its input: `to.…[min: 1, max: 99]($text)` (placeholder name).
- **Inside `[ ]`:** `:` labels an argument or bound (`min: 1`); `=` sets a field.
- **The one exception, declared from the start: grouping in maths**, `$x - ($a + $b)`, where `( )` runs nothing.
- Lessons carried from CyborgPL: place generics in this rule when they are designed (CyborgPL's `< >` was missed);
  something that acts at build time (e.g. reading a file into the program) runs something, so `( )`.

## D51 — `fn` and `fn.pure` (2026-10-30)

Owner: "Programming-wise, it's accurate. Maths-wise, it's not. Guess who knows Ownership. So, maybe `fn` and
`fn.pure`?" A function starts with **`fn`** (its readers are programmers, who know "function" loosely); **`fn.pure`**
narrows it (D26) to a function in the maths sense: no side effects, the same result for the same arguments. The
compiler checks the promise, as with `var.immut` (D30); bare `fn` promises nothing either way.

Proposed by design claude, owner: "yes": **contracts may call only `fn.pure` functions** — `requires`, `ensures`, loop
invariants, failure conditions, termination measures. A contract must never do anything, or a program would behave
differently in mode 2 (contracts checked at run time) and the final build (proven away). As in Dafny and SPARK.

## D52 — The result type: `->` (2026-10-30; "leave it out" replaced by D98)

Proposed by design claude, owner: "yes". A function's result type follows `->` after the parameters:
`fn first_word [$text: str.utf8, …] -> str.utf8:`. A function with no result leaves it out:
`fn log [$msg: str.utf8]:`. (`: type` was rejected: `:` already gives a name its type and opens a block. `returns`
was rejected as long.)

## D53 — Contracts are the first lines of the body (2026-10-30)

Proposed by design claude (option 3), owner: "yes". Replaces D13's provisional placement. The header is one line
ending in `:`; contracts are ordinary statements ending in `;`, and **must come first**, before any other statement
(only a `labels` line, D54, may come before them):

```
fn first_word [$text: str.utf8, $limit: int[min: 1, max: 80]] -> str.utf8:
    requires length($text) > 0;
    ensures length(result) at_most $limit;
    fails too_long exactly when length($text) > $limit;
    …
end fn first_word
```

A contract placed after another statement is refused. Tools and docs show contracts as part of the function's
interface.

Owner, "yes" (same day): the words are **`requires`** (what callers must guarantee) and **`ensures`** (what the
function promises back). Owner: in an `ensures`, the value the function gives back is **`result`**, bare — it comes
from the language, not the writer, so it carries no `$` (D31) and never clashes with a writer's `$result`.
`fails … exactly when` is still a placeholder.

Proposed by design claude, owner: "Yes". A contract refers to a value **as it was when the function started** with
**`old[…]`** (`[ ]` describes, D50): `ensures length($list) equal length(old[$list]) + 1;`.

## D54 — Labels (2026-10-30)

Owner: "the things that need clarification will have labels (writers could also be able to make their own labels).
Like `min:, max:`". A value whose meaning wouldn't be clear from its position gets a label, written `name:` before it
(`int[min: 1, max: 99]`). Built-in things use labels where they need clarifying, and writers can give their own
functions labels too. Owner first said writer-made labels are only available in function collections (as in
CyborgPL). After CyborgPL's design session pointed out the asymmetry this causes (the language's own functions all
have labels, so writers must build a whole collection to label one argument), owner: **"any `fn`"** — any function
may declare labels. Collections keep members, presets and versions.

Proposed by design claude (option 1), owner: "yes": **a `labels` line at the top of the body**, before the contracts
(D53), the same line in a plain `fn` and in a collection. The caller's word may differ from the writer's `$name`:

```
fn first_word [$text: str.utf8, $limit: int[min: 1, max: 80]] -> str.utf8:
    labels [$limit as limit];
    requires length($text) > 0;
    …
end fn first_word

first_word("hello there", limit: 20)
```

The top of the body is where "how to call me" lives: labels, then contracts.

Proposed by design claude (option 3), owner: "yes": **bare `labels` keeps the declared order**; **`labels.any_order`**
lets callers write labelled arguments in any order (e.g. `box(height: 4, width: 10)`). `labels.in_order` is refused
as a spelled-out language default (D39).

Proposed by design claude (option 3), owner: "yes": **any `fn` may declare defaults, only for labelled parameters**,
on a `defaults` line after `labels`: `defaults [$limit = 40];` → `first_word("hello there")` gives `limit` 40.
Defaults are the only optional arguments. Top-of-body order: `labels`, `defaults`, then contracts.

Proposed by design claude (CyborgPL's rule), owner: "yes": **Radium's own functions leave the first argument — the
thing worked on — bare, and label every other argument with its role**: `list.append($readings, value: 42)`,
`str.replace($name, from: "a", to: "b")`. Exception: lists where every item has the same role, like `print`'s pieces.
This binds Radium's own functions (`std`, built-ins); writers label theirs as they like.

From CyborgPL, for reference: a label is the caller's word, the `$param` stays the writer's name (`labels [$text as
value]`); labels are required once declared; the language's own functions leave the first argument (the thing worked
on) bare and label every other argument's role (`list.append($xs, value: 2)`), except lists where every item has the
same role.

## D55 — A standard and documented behaviour (2026-10-30)

Owner: "we would also have documented behaviour and a standard", then "Not just for `fn`, bro. The entire lang …
How ISO says C stuff." Radium will have a **written standard for the entire language**, in the manner of the ISO C
standard: the documented behaviour of every construct, not only of functions. It sits alongside DECISIONS.md (why)
and the oracle (D48, what runs).

Proposed by design claude, owner: "yes". Of ISO C's four kinds of behaviour, Radium's standard has only two:

1. **Defined** — every construct has exactly one specified behaviour.
2. **Target-defined** (in place of implementation-defined) — only what the declared target describes may vary
   (stack size, heap budget D8, pointer size D14), written in the target's description, never chosen silently by the
   compiler.

There is **no unspecified behaviour** (evaluation order is fixed; e.g. arguments left to right, `and`/`or` per D40)
and **no undefined behaviour** (anything that could go wrong is a check site the prover must rule out, D2). The one
place outside this is **inside a shield** (D6): the standard specifies the walls; the inside is the shield writer's
responsibility. The standard's form and when it is written are open. Named "SSS: Radium" by D109.

## D56 — Failures are written as `fails` lines in the body (2026-10-30)

Proposed by design claude, owner: "yes". A function's failure set (D11) is its `fails` lines, among the contracts at
the top of the body (D53); each names one failure and says when it happens. Failures are not repeated in the header.
Docs and the standard (D55) show them as a list.

Proposed by design claude, owner: "yes". Two forms:

- **`fails X exactly when C;`** — a failure decided by the arguments. The promise goes both ways: when `C` holds the
  function always fails with `X`, and when it doesn't, it never does. A caller who has proven `C` false needn't
  handle `X` (D11). (Plain `when` was avoided: in English it reads one-way, a D33 trap.) `C` may call only `fn.pure`
  functions (D51).
- **`fails X;`** — a failure caused from outside (a missing file, a network error), which no condition on the
  arguments can express. Callers must always handle it.

## D57 — `narrowings`: function and handler collections (2026-10-30)

Owner proposed **`narrowings`** as the keyword (a collection is the set of narrowings of one name, D26), then chose
to narrow it by kind "so we could remove the `fn` spam": **`narrowings.fn`** and **`narrowings.handler`** (D11's
handler collections). Since the outer line states the kind, the blocks inside don't repeat it — a D13 exception, like
`main`:

```
narrowings.fn print:
    print [$items: …]:                 // the name itself (the collection's own function)
        …
    end print
    print.stdout [$items: …]:          // a narrowing of it
        …
    end print.stdout
end narrowings.fn print
```

Replaces CyborgPL's `collection.fn` / `coll.fn`.

Proposed by design claude (option 3), owner: "yes": **pure members.** Inside `narrowings.fn`, a member is a plain `fn`
by default. A pure one writes **`fn.pure`** in front (`fn.pure area.square [ … ] -> …:` … `end fn.pure
area.square`). When every member is pure, the collection says so once: **`narrowings.fn.pure area:`**.
Proposed by design claude, owner: "yes": **presets** — a narrowing that only fills in some of the name's arguments,
with no code of its own, written as one line ending in `;`: `heat.popcorn [$power = 800, $minutes = 3];`,
`heat.defrost [$power = 200];`. A preset takes the arguments it leaves open, in order: `heat.defrost(10)` is
`heat(200, 10)`. A collection's narrowings exist only if its writer defines them.

Owner asked "Presets or narrowings?"; design claude proposed (option 2), owner: "yes". **Everything inside
`narrowings.fn` is a narrowing.** One with its own code is simply a narrowing (a block ending in `:`); one that only
fixes arguments is called a **preset** (one line ending in `;`) — the word the standard and error messages use. The
word "member" (CyborgPL's) is not used.

Proposed by design claude, owner: "yes": **presets stack.** Each `.` applies one more preset, so a writer defines each
choice once and combines them: `print.full [$format = full];`, `print.stderr [$target = stderr];` →
`std::print.full.stderr("oops")`. Only presets stack (a narrowing with its own code is a different function); a chain
setting the same parameter twice is refused (`print.full.shortest`); with `defaults.fixed`, a chain spelling out a
default is refused (`print.shortest.stderr` → "write `print.stderr`").

Owner asked why the collection writer doesn't refuse `std::print.shortest.stdout`; design claude proposed, owner:
"yes". **`defaults.fixed [ … ];`** promises a function's defaults will never change; in return, **spelling them out is
refused** (`print.shortest.stdout("hi")` → "write `print("hi")`"), as D39 does for language-fixed defaults. Plain
`defaults` may change in a later version, so spelling them out stays allowed, to pin behaviour. **`std` uses
`defaults.fixed`**, since its behaviour is part of the standard (D55). Bare `std::print(…)` is the everyday form.

## D58 — `struct`, and the keyword rule (2026-10-30)

Owner asked first how the word reads in programming, maths and English; then which choice is consistent with `fn`.
Design claude, owner: "yes". **`struct`**: named fields stored together as one value.

```
struct reading:
    time: int[min: 0, max: 86_399];
    celsius: int[min: -40, max: 125];
end struct reading

var $r: reading = reading[time = 3600, celsius = 21];
var $t = $r.celsius;
```

**The keyword rule** (from D51 and this): a keyword is **the settled programming word** — Radium's readers are
programmers — and narrowing adds a stricter (e.g. maths) meaning where it matters, as `fn` → `fn.pure`. (`record` was
considered: better English, but Java/C# give it a "can't be changed" meaning.) Fields have ranges (D14), are built
with `[ ]` and `=` (D50), and are read by narrowing, `$r.celsius` (D26).

Proposed by design claude, owner: "yes". **Fields are not vars**: they are named parts of a value, so they carry no
`var` and no `$`. **A field's changeability follows whatever holds the struct** (`var.immut $r` → no field of `$r`
changes). A field may be locked for good with **`.immut`** (`id.immut: int[…];`), even inside a `var.mut` holder;
the compiler checks it. There is no `.mut` for fields.

## D59 — `union.tagged`, and a shield-only `union` (2026-10-30)

Owner proposed `union.tag` / `union_tagged` over design claude's `enum`; design claude proposed the family below,
owner: "yes". A value that is one of several named cases, which may carry data, is a **`union.tagged`** — the
established term "tagged union":

```
union.tagged status:
    ok;
    warming_up;
    failed[code: int[min: 1, max: 255]];
end union.tagged status
```

**Bare `union`** is C's kind — no tag, no record of which case is in use — and is allowed **only inside shields** (D6),
for C interop, like raw pointers. (`enum` was considered: common, but its meaning splits between C's constants and
Rust's data-carrying cases. CyborgPL writes `union.tag`.)

Proposed by design claude, owner: "yes": **a case is always written with its type** — `status.ok` (narrowing, D26),
`status.failed[code = 3]` (built like a struct, D50). A case with no data takes no brackets. No shortcut like `.ok`
when the type is known. How a value is matched against its cases is control flow (step 4).

## D60 — `bool`, and no truthiness (2026-10-30)

Proposed by design claude, owner: "yes". True/false values are **`bool`**, with **`true`** and **`false`**. A
condition accepts only a `bool`: numbers, text and lists never count as true or false (`if $count:` is refused;
write `if $count > 0:`), since "truthy" values are a trap (D33).

## D61 — Abbreviations in the language (2026-10-30; test replaced by D63)

Owner noticed `bool` saves only three letters, which D19's rule wouldn't allow, yet `bool` and `struct` feel right;
then that `fn` is on keyboards (the Fn key), and that "normal people don't know `struct`". Design claude proposed,
owner: "yes". For the language's own words (D19 stays the rule for implementation code):

> A short form is used only when it is **already settled for programmers** (the same spelling across major
> languages) **or for everyone** (maths notation, everyday use). Either one is enough. Otherwise, the full word.

| Short form | Settled for | |
|---|---|---|
| `int`, `bool`, `struct`, `const`, `var`, `std` | programmers | ✅ |
| `fn` | everyone (the Fn key) | ✅ |
| `min`, `max`, `mod` | everyone (maths, everyday) | ✅ |
| `len`, `newln` | neither | ❌ — `length`, `newline` |

`div` (D34) follows the programming side (Pascal, Haskell).

## D62 — A folder for design rules (2026-10-30)

Owner: "Make a separate folder just for design rules, in the GH repo." **`design-rules/`** collects the general rules
the design is judged by, one file each, citing the decisions they came from: clear syntax (D33), choosing words (D34,
D58, D61), dots and underscores (D26, D35), brackets (D50), marks (D28, D31, D43), defaults (D27, D39, D42, D57).
DECISIONS.md stays the record; when a rule changes, it gets a new entry here and the rule's file is updated.

## D63 — Abbreviations: what the owner reads correctly (2026-10-30; replaces D61's test; test refined by D94)

Owner, after comparing a strict rule ("same spelling across languages") with a loose one ("programmers read it
instantly"): "This one is cursed, but it's technically consistent. Whatever I can read correctly." Design claude
added two safeguards; owner: "Let's go". **A short form is allowed if the owner reads it correctly, shown cold** —
on its own, without context or the full word — and **each verdict is recorded** in the list below and in
`design-rules/choosing-words.md`. Once recorded, a word is not re-judged, so the rule doesn't drift as the owner
learns. Anyone can apply the rule by reading the list. Replaces D61's test; D61's table becomes the first set of
words to judge.

Verdicts (2026-10-30, three words at a time):

| Short form | Owner read it as | Verdict |
|---|---|---|
| `buf` | — ("nope") | ❌ `buffer` |
| `deci` | decimal | ✅ |
| `ctx` | — ("nope") | ❌ `context` |
| `mut` | mutable | ✅ |
| `idx` | — ("nope") | ❌ `index` |
| `bin` | binary | ✅ |
| `fmt` | format | ✅ |
| `immut` | "same as [mut], but false" — immutable | ✅ |
| `ptr` | pointer | ✅ |
| `err` | error | ✅ |
| `msg` | message | ✅ |
| `len` | — ("nope", shown cold) | ❌ `length` |
| `char` | character | ✅ |
| `div` | divide | ✅ |
| `cfg` | — ("nope") | ❌ `config` |
| `config` | config — "I thought that was the full word actually" (short for configuration, read as a word) | ✅ |
| `stdout` | standard output | ✅ |
| `mod` | modulo | ✅ |
| `const` | constant | ✅ |
| `std` | standard | ✅ |
| `int` | integer | ✅ |
| `struct` | struct — "thought that was the full word" (read as a word) | ✅ |
| `var` | variable | ✅ |
| `fn` | function | ✅ |
| `bool` | boolean | ✅ |
| `stderr` | standard error output | ✅ |
| `min` | minimum | ✅ |
| `max` | maximum | ✅ |
| `newln` | newline — "most langs already do `ln` for line" (not cold: discussed in D36) | ✅ — and **replaces D36's `newline`** (owner: "the rule only exists now, and I think it's a better one. `newln`") |

Result: 25 allowed (`deci`, `mut`, `bin`, `fmt`, `immut`, `ptr`, `err`, `msg`, `char`, `div`, `str`, `stdout`,
`mod`, `const`, `std`, `config`, `int`, `struct`, `var`, `fn`, `bool`, `stderr`, `min`, `max`, `newln`); 5 refused
(`buf`, `ctx`, `idx`, `len`, `cfg` — written `buffer`, `context`, `index`, `length`, `config`). New short forms are judged the same way
as they come up.
| `str` | string (not fully cold: seen in an earlier table; owner's first remark, "why do I see `str` as `string`", came before it) | ✅ |

## D64 — List types (2026-10-30)

Proposed by design claude, owner: "yes". A list type is **one bracket**: what it holds first, unlabelled, then its
length range with labels — `list[int[min: 0, max: 100], min: 0, max: 50]`. `[ ]` describes (D50); the main thing
stays bare and the rest is labelled (D54); `min:`/`max:` read as they do on `int`. As with integers (D14), a list at
the edges (parameters, return values, fields, globals) needs its length range.

**Positions** (2026-10-30). Shown cold, with `["apple", "banana", "cherry"]`, the owner read `pos: 1` → apple,
`pos: 0` → "the thing before apple, which doesn't exist", `offset: 1` → banana, `offset: 0` → apple; "So, honestly?
Labels are important." So: **`pos:` counts from 1** (a position: first, second) and **`offset:` counts from 0** (a
distance from the start), each label with one meaning, and **an index always has its label** — a bare `$xs[1]` is
refused, since its counting would be invisible. Proposed by design claude, owner: "yes": **both labels exist** —
`pos:` for ordinary code ("the second item"), `offset:` for byte-level work where specs speak in offsets (file
formats, packets, C interop), so neither kind of code needs `+ 1` / `- 1` adjustments.

## D65 — Licences (2026-10-30)

Owner: "where's the LLVM exception, and CC BY 4.0?" Design claude proposed matching CyborgPL; owner: "yes". Replaces
the plain Apache 2.0 of the first commit.

- **`LICENSE`: Apache-2.0 WITH LLVM-exception**, for all code. Radium's runtime and `std` are compiled into every
  program; the exception waives Apache's notice requirements for that compiled output, so **programs built with
  Radium owe nothing**. (The LLVM project's own licence.)
- **`LICENSE-CC-BY-4.0`: CC BY 4.0**, for the repo's writing — every Markdown file. Credit line:
  `Radium GitHub repo text: https://github.com/Soclets/Radium`. Code and code comments stay under `LICENSE`.

## D66 — Maps (2026-10-30)

Owner asked whether both kinds of map should exist; design claude proposed, owner: "yes". A classic hash map, whose
iteration order is unspecified, can't exist as-is (D55). The kinds differ by **the order you get going through
them**: **`map.sorted`** (key order) and **`map.inserted`** (insertion order). **No bare `map`**: the order is a
visible choice, so there is no default to hide it (D42). The data structure underneath is the compiler's business.

Owner asked where the fastest map would be; design claude proposed, owner: "yes": a third kind, **`map.unordered`** —
the fastest (a plain hash map), with lookup, adding and removing, but **no way to go through its entries**. With no
order to observe, nothing is unspecified (D55). To go through the entries, convert it to `map.sorted` or
`map.inserted`, which is where the order gets defined. Its hash function is fixed, the same on every run and machine
(keys crafted to collide can slow it, never change its results).

Proposed by design claude, owner: "yes": **key and value are both labelled** —
`map.sorted[key: str.utf8, value: int[min: 0, max: 999_999_999], max: 500]` — since neither is the main thing.
`min:`/`max:` give the number of entries, required at the edges like a list's length (D64).

## D67 — The prover is Geiger (2026-10-30)

Owner chose **Geiger** over Radinum (design claude's recommendation: Radinum reads as a misspelling of Radium and is
easily confused with it). A Geiger counter reveals danger that is already there, as the prover reveals bugs; the name
is distinct from the language while belonging to the radium world. Messages read
`Geiger: line 12, overflow possible when $count = 255`. Other candidates: Curie, Assay, Dosimeter, Cherenkov,
Becquerel.

## D68 — The compiler is Curie; files end in `.ra` (2026-10-30)

Owner asked for another name than plain `radium`, and for the file extension; design claude proposed both, owner:
"yes to both".

- **The compiler is Curie** — Marie Curie refined radium out of pitchblende ore, as the compiler refines source into
  a program. *Curie builds it, Geiger (D67) checks it.* The command is `curie` (`curie build`, `curie final`,
  `curie inspect`); where earlier decisions wrote `radium build` / `radium inspect`, read `curie`.
- **Source files end in `.ra`** — radium's chemical symbol. (The old RealAudio format also used `.ra`; it is long
  obsolete.) To check before the first code lands: conflicts in GitHub's language detection.

## D69 — Writers' own generics (2026-10-30)

Proposed by design claude, owner: "yes". Writers can make their own generic functions and types (one `largest`
for lists of any element type), as `std` needs to be written in Radium (D23).

Proposed by design claude, owner: "yes": **a placeholder type is marked `?` everywhere it appears**, and declared in
**its own `[ ]` before the parameters** (one kind per bracket, a CyborgPL lesson):

```
fn largest [?T] [$items: std::list[?T, min: 1, max: 100]] -> ?T:
```

`?` reads as "a type left open"; `<T>` is unavailable since `<` and `>` are comparisons (D34). Placed in the bracket
rule (D50): placeholders describe, so `[ ]`.

**What a placeholder needs** (2026-10-30; owner pointed to CyborgPL's design, its design session briefed it; owner:
"yes, but we have to look at the names first"). Radium starts from CyborgPL's shape:

- a group listing **operations** a type must have (CyborgPL's `sig`) — signatures only, no code;
- a group listing **types**, closed (CyborgPL's `set`);
- the two never mix in one block;
- a bound is written once, in the placeholder's `[ ]`, joined with `and`: `[?T: … and …]`;
- a generic body may use on a `?T` value only what its bound lists.

Owner asked "is `interface` accurate?"; design claude checked it three ways, owner: "yes". **The operations group is
an `interface`** — the settled programming word (D58) for "a set of operations a type promises to have", and in
English the boundary through which generic code touches a `?T`. (CyborgPL dropped `interface` for `sig` because its
version could list types; Radium's can't. Unlike Java/C#, a Radium interface holds no code.) The stricter maths
meaning, a signature with laws, could come in by narrowing (e.g. `interface.lawful`, placeholder).

Proposed by design claude, owner: "yes": **the types group is a `type_set`** — Go's term, exact in maths ("a set
whose members are types"), joined with `_` (D35). Plain `set` was avoided: it is a common data structure (`std`
will likely want one) and a verb, which would give it two meanings (D33).

```
type_set whole:
    int.signed.32_bit;
    int.signed.64_bit;
end type_set whole

fn smaller [?T: whole and ordered] [$a: ?T, $b: ?T] -> ?T:
```

Proposed by design claude, owner: "yes": **a type joins an interface only when declared**, with one line —
`reading satisfies ordered;` — which the compiler checks there. Having the right operations by accident doesn't
count: a `<` meaning "older than" must not silently make a type `ordered` (a D33 trap; CyborgPL joins automatically).
The `satisfies` line is where laws would be proven if interfaces later carry them.

Proposed by design claude (option 1), owner: "yes": **an interface may contain laws** — rules its operations must
obey, proven by Geiger (D67) for each type at its `satisfies` line. Optional; an interface without laws works as
before. Generic code may rely on them (`largest` can trust `<` is a real ordering).

```
interface ordered [?T]:
    op < [$a: ?T, $b: ?T] -> bool;
    law if $a < $b and $b < $c then $a < $c;
end interface ordered
```

This replaces the earlier idea of a narrowed `interface.lawful`: the `law` lines are visible inside the interface. The
spelling of `law` and of operator signatures (`op`) is provisional.

Proposed by design claude, owner: "yes": **an interface can include another** — `includes equatable;` inside
`ordered` — and gets its operations and laws. A type satisfying `ordered` must satisfy `equatable` too. (`includes`
is provisional.)

Proposed by design claude, owner: "yes": **an interface can declare a type slot** that each joining type fills in
(Rust's associated types): `type item;` in the interface, `std::list[?E] satisfies walkable [item = ?E];` at the
join. The item type is a fact about the type, not a choice the user makes. Spellings provisional.

## D70 — `array` is built in; `list` and the maps live in `std` (2026-10-30)

Owner asked "where's `array`, and who owns `list`?"; design claude proposed, owner: "yes". **Amends D64 and D66.**

- **`array`** — built into the language: a fixed number of values in a row, its length part of its type, stored
  right inside its holder, **no heap**: `array[int[min: 0, max: 255], length: 16]`. What small boards use.
- **`std::list[…]`** and **`std::map.sorted` / `.inserted` / `.unordered`** — in `std`, written in Radium (D23), so
  proven. Growing needs raw memory, which sits in a small shield (D6) inside `std`; they use the heap and count
  against the heap budget (D8). D64's and D66's spellings gain the `std::` prefix.
- A list owns its items (D5); they are dropped with it.

## D71 — `if`, `else if`, `else` (2026-10-30)

Proposed by design claude, owner: "yes". Other branches of an `if` are **`else if …:`** and **`else:`**, and the
whole chain closes with **one `end if`**:

```
if $celsius > 30:
    std::print("hot");
else if $celsius > 20:
    std::print("warm");
else:
    std::print("cold");
end if
```

The condition must be a `bool` (D60).

## D72 — `match` (2026-10-30)

Proposed by design claude, owner: "yes". Choosing between a `union.tagged`'s cases (D59) is **`match`**, with
**`case …:`** branches continuing the block and one **`end match`**. A case pulls out its data by label
(`[code = $c]` binds `$c`):

```
match $s:
    case status.ok:
        std::print("all good");
    case status.warming_up:
        std::print("wait");
    case status.failed[code = $c]:
        std::print("failed with code ", $c);
end match
```

**Every case must be covered**, or the compiler refuses. **Branches never fall through.** (`switch` was avoided:
C's fall-through trap; `when` is Kotlin-only.)

## D73 — Counting loops (2026-10-30)

Chosen by cold tests with the owner. Round 1: `to:` read correctly, but `until:` read as including the end, `by: 2`
read as "times 2", and `from: 5, to: 1` drew "what". Round 2: `before:` read correctly; `every: 3` read as time;
the owner proposed a negative step for counting down, then `step_by` ("step_by?"), over `step:` and `steps:`
(which risk reading as a count). Owner: "yes".

```
for $i [from: 1, to: 5]:                   // 1, 2, 3, 4, 5
for $i [from: 1, before: 5]:               // 1, 2, 3, 4
for $i [from: 0, to: 10, step_by: 2]:      // 0, 2, 4, 6, 8, 10
for $i [from: 5, to: 1, step_by: -1]:      // 5, 4, 3, 2, 1
```

(Loop headers only; each loop closes with `end for`.) **Respelled by D74:** `for $i in range[from: 1, to: 5]:`.

## D74 — `for … in`, `range`, and "bounds" (2026-10-30)

Owner proposed `for $r in $readings`; design claude pointed out it would give `for` two shapes and proposed `in`
everywhere with a range value; owner asked "Is `in range` accurate?" — design claude found "range" already meant a
type's `min`/`max` (D14) — and the owner proposed calling those `bounds` instead. Owner: "yes".

- **Every loop is `for $x in …:`** — a collection or a range:
  ```
  for $r in $readings:                              // a std::list, front to back
  for $i in range[from: 1, to: 5]:                  // 1, 2, 3, 4, 5
  for $i in range[from: 0, to: 10, step_by: 2]:     // D73's labels, inside range[…]
  ```
  A `map.sorted`/`map.inserted` goes in its named order; a `map.unordered` can't be gone through (D66), so is
  refused.
- **`range[…]`** is a value describing a sequence of numbers (`[ ]` describes, D50), so it can be stored and passed.
- **A type's `min`/`max` are its "bounds"** (exact in maths: lower and upper bounds), not its "range". Earlier
  decisions that say "range" for these (D14, D24, D27, D64, …) mean bounds.
- **A generic's `[?T: ordered]` is a "requirement"** ("`?T` requires `ordered`"), not a "bound", which now means
  value limits. (D69's wording is read that way.)

## D75 — `while` (2026-10-30)

Proposed by design claude, owner: "yup". **`while …:` … `end while`** runs while its `bool` condition holds. It must
provably end (D7): Geiger works out why where it can, and otherwise asks for a **`decreases`** line — a value that
shrinks every trip and can't go below zero. `decreases` and loop **invariants** (D4) sit at the top of the loop's
body, like contracts at the top of a function (D53):

```
while $remaining > 0:
    decreases $remaining;
    …
end while
```

`for` over a range or a collection ends by its nature. A loop meant to run forever is `forever` (D7).

## D76 — `forever` (2026-10-30)

Proposed by design claude, owner: "yes". The loop allowed to run without end (D7) is **`forever:` … `end forever`**;
each trip must provably end. One searchable word lists every place a program can run endlessly. (Rust's `loop` is the
settled word but says nothing special — every loop is a loop; `while true` would hide the exception inside an
ordinary `while`.)

## D77 — `break`, and no skip statement (2026-10-30)

Chosen by cold tests with the owner, owner: "yes". **`break;`** stops the loop it is in (read correctly cold: "the
loop stops when `$i` reaches 3"). It works in `for`, `while` and `forever`.

**There is no "skip to the next trip" statement.** Five candidates failed the cold test: `continue` (read as carrying
on), `skip` (read as skipping the whole loop), `next`, `skip_rest`, `skip_this`. (`stop`, tested as an alternative to
`break`, read as ending the whole program.) Writing the condition around the work instead read correctly at once ("if `$i` doesn't
equal 3, print `$i`"):

```
for $i in range[from: 1, to: 5]:
    if $i not_equal 3:
        std::print($i);
    end if
end for
```

Proposed by design claude, owner: "yes": **a loop may be named** (a named block, D13) — `for rows $row in $grid:` …
`end for rows` — and **`break rows;`** stops that loop and every loop inside it. A bare `break;` stops the innermost
loop.

```
for rows $row in $grid:
    for $cell in $row:
        if $cell equal $target:
            break rows;
        end if
    end for
end for rows
```

## D78 — `otherwise`: handling failures at a call (2026-10-30)

Proposed by design claude, owner: "`otherwise`". A call that can fail (D56) is followed by **`otherwise:`**, with a
**`case`** per failure still possible at that call (D11), closed by **`end otherwise`**:

```
var $w = first_word($text, limit: 20) otherwise:
    case too_long:
        "…";
    case empty:
        "(none)";
end otherwise
```

(`catch` was avoided: it implies exceptions, which D11 rules out; `else` belongs to `if`.) What an arm's last line
means (the value used in place of the result, as above) is provisional.

Proposed by design claude, owner: "yes": **handler narrowings at a call** (D11, D57). A `narrowings.handler` block
names sets of arms, each written with `case` as in `match`; a call uses one with a one-line
**`otherwise word_failed.use_placeholder;`**:

```
narrowings.handler word_failed:
    word_failed.use_placeholder:
        case too_long:
            "…";
        case empty:
            "(none)";
    end word_failed.use_placeholder
end narrowings.handler word_failed

var $a = first_word($text, limit: 20) otherwise word_failed.use_placeholder;
```

The arms act as if written at the call, so `return`, `break` and failing apply there. They must cover every failure
still possible at that call; extra arms are allowed and reported as unused there. A bare collection name is its own
handler, if it defines one.

- **`to:`** includes the end; **`before:`** stops before it.
- **`step_by:`** is the amount added each time; it defaults to 1 (D42); a negative step counts down (as Rust's
  `.step_by`).
- A loop whose step can never reach its end (`from: 5, to: 1` with no negative step) is refused.
- Such a loop provably ends (D7).

## D79 — The logo (2026-10-30)

Owner's design, drawn out by design claude in rounds (`brand/concepts/`). An **R** whose front leg's foot sends a line
**straight up and over, following the R's head** (owner: "a line that goes up from the front leg, and curve by
following the R's head"), and **a line along the bottom** from the stem's foot to the leg's foot, dipping slightly
(owner's drawing). The R sits inside its own closed wall — readable as the safety radius (D6).

**One version everywhere** (owner: "Small size ver everywhere is good"): the rising line is merged into the head, so
nothing is doubled; strokes are thick enough for 16 px; the lower counter is opened so it doesn't read as a "B".
Files: `brand/mark-black.svg`, `brand/mark-white.svg` (240×240 grid, stroke 22, round caps and joins).

Colours: **warm amber is the main colour** — `#ff9a3c` on `#1a0f05` (owner, from a shortlist of radium's pale-blue
glow and Cherenkov blue). The mono versions (white on black, black on white) are kept for single-colour use. File:
`brand/mark-amber.svg`.

Proposed by design claude (CyborgPL's arrangement), owner: "yes" (extends D65): **the logo files in `brand/` are
under CC BY 4.0**, credited `Radium logo: https://github.com/Soclets/Radium`. **The name "Radium" and the logo are
trademarks (not registered)**: anyone may use them, anywhere, but not in a way that pretends to be the project or
speaks for it. Neither licence grants trademark rights.

## D80 — Ownership words: `give`, `copy`, `lend`, `lend.mut` (2026-10-30)

Cold-tested with the owner (4 of 4 read correctly: whether `$buffer` can be used afterwards, and whether the callee
can change it); owner: "yes". CyborgPL's words, from the owner's side (D5):

```
take_reading(give $buffer);       // ownership moves; $buffer can't be used afterwards
take_reading(copy $buffer);       // a separate copy is handed over; yours is untouched
take_reading(lend $buffer);       // temporary read-only access; you keep it
take_reading(lend.mut $buffer);   // temporary access that may change it; you keep it
```

Words, not symbols (D34); `lend.mut` narrows `lend` (D26). Lifetimes name their source with `lend from $text`
(D49). Rust's receiver-side `move` / `clone` / `borrow` weren't tested once these scored 4 of 4. Linear values (D5)
are open. (Capitalised, and linear values settled, by D84.)

## D81 — Final products should be fast (2026-10-30)

Owner's own goal: "final product speed is also a goal for Radium. Like, what if a car brakes in an emergency?" (Not a
claim that Radium will be used in cars; D1 still holds.) Only the final build (mode 3, D9) is meant to be fast; modes 1
and 2 still trade speed for checks. Speed comes after correctness, never instead of it.

What already serves it: the final build carries no runtime checks because everything is proven (D9); proven facts are
handed to LLVM (D17); stack use is proven and the heap is budgeted, with no garbage collector (D8).

Open: whether "fast" means fast on average, a proven worst-case time (a deadline), or both (settled by D82); and
whether D17's conservative final pipeline (risky passes only with evidence) still fits (settled by D83).

## D82 — Fast means both: average speed and proven deadlines (2026-10-30)

Proposed by design claude, owner: "yes".

- **Average speed:** the final build is optimized for speed everywhere.
- **Deadlines:** a writer may mark a function with a deadline, the longest it may ever take (e.g. 2 ms). Geiger must
  prove the worst case fits, or the build is refused, like any unproven check. Termination (D7) and stack (D8) are
  already proven; this extends the same idea to time.
- Worst-case time depends on the exact chip (caches, pipeline), so deadlines can only be proven on a target that
  declares a **timing model**, the way a target declares its heap budget (D8). On a target without one, a deadline
  mark is a compile error, never silently ignored.
- The proof is about the final machine code, after optimization, since that is what runs.

Code without a deadline mark only has to be fast. The spelling of the mark is for the syntax phase.

## D83 — Risky optimizer passes, checked every run (2026-10-30; replaces D17's point 4)

Proposed by design claude, owner: "yes". Follows from D81.

D17's point 4 kept the final build to a conservative list of LLVM passes, with risky ones (e.g. InstCombine, the loop
vectorizer) only with evidence. Now the final build **also runs the risky passes**, and every run is checked:

1. The optimizer produces the fast version.
2. A translation validator (Alive2, or our own) proves the fast version does exactly what the unoptimized version
   does, for this program.
3. Where it proves that, the fast version is used.
4. Where it can't, for some function, that function falls back to the conservative pipeline's output, and the build
   reports which functions fell back and why.

No optimizer is trusted; slower compiles are the accepted cost. The conservative pipeline stays as the fallback, and
stays frozen as part of the build's identity. D17's other safeguards are unchanged.

## D84 — Ownership words are capitalised; linear values are `must_Give` (2026-10-30; amends D80)

Owner's design, after seeing both versions side by side ("a lot" better, "yes"). Design claude had recommended the
lowercase `must_use`, which passed a cold test 4 of 4; `linear`, `once` and `must_finish` failed.

**The ownership words start with a capital**, so every hand-off stands out from the code around it. Lowercase
spellings are refused (one spelling each, D33). A narrowing after them stays lowercase:

```
write(Lend.mut $f, $text);
var $copy_of_text = Copy $text;
archive(Give $copy_of_text);
close(Give $f);
```

A parameter that takes ownership was sketched as `fn close [$f: Give open_file]:`; replaced by D91
(`[$f: Given open_file]`).

**Linear values (D5)**: a type narrowed with **`must_Give`** must be handed off with `Give` exactly once before it goes
out of scope: lending doesn't count, forgetting or giving twice is a compile error (not a warning, unlike Rust's
`#[must_use]`). Giving it to a function passes the duty along: the receiver must `Give` it in turn. Checked entirely
at compile time, so it costs nothing at run time (D81).

```
struct.must_Give open_file:
    handle: int[min: 0, max: 65_535];
end struct.must_Give open_file
```

It works on any type that narrows, e.g. `union.tagged.must_Give`. The capital is what makes `must_Give` read as "must
do a `Give`" rather than "the file must give something". This is the first place letter case carries meaning in
Radium: only these words, and names that contain them.

## D85 — Comments: `#` and a space (2026-10-30; mark replaced by D87: `//`)

Owner's design. A comment starts with **`#` followed by a space** and runs to the end of the line, on its own line or
after code. `#` with no space after it is refused:

```
# a comment                      ✅
var $x = 3;   # also a comment   ✅
#not a comment                   ❌ refused
```

Trace tags in Radium code, e.g. the oracle's tests (D48), are written `# trace D<n>`. The `//` in earlier code samples
is a placeholder. (The implementation's own comments, D18, follow Rust and C++ and are unaffected.)

## D86 — Comment blocks: `comment:` … `end comment` (2026-10-30; `#` read as `//` since D87)

Owner's design ("just plain `comment:` honestly"), after design claude pointed out that a `#[ … ]` block would be the
only multi-line `[ ]`, the only `[ ]` holding prose, and could be closed early by a `]` in the text. A comment that
spans lines is a block, like everything else that spans lines (D13):

```
comment:
    Reads the sensor and returns the average of the last 10 readings.

    Fails with too_hot above 125 °C, see int[min: -40, max: 125].
end comment
```

Blank lines inside are part of the comment. One-line comments stay `# ` (D85).

Proposed by design claude, owner: "yes": **a lone `#` (an empty comment line) is refused.** `#` is always followed by a
space and text; a comment that needs paragraph breaks is a `comment:` block, so there is one way to write it.

## D87 — One-line comments are `//` and a space; `#` is kept free (2026-10-30; replaces D85's mark)

Owner: a one-line or trailing comment should use a mark "we would genuinely not use", so `#` stays free "jic" it fits
something else. Design claude proposed `//`, owner: "yes". Radium divides with the word `div` (D34, D37), so `/` has no
other job and `//` can never collide; most programmers already read it as a comment. (`--` was passed over: it clashes
with minus. `%` and `;;` were passed over too.)

D85's rules carry over with the new mark: `//` is followed by a space and text, on its own line or after code; `//text`
and a lone `//` are refused; paragraph breaks go in a `comment:` block (D86).

```
// a comment                      ✅
var $x = 3;   // also a comment   ✅
//not a comment                   ❌ refused
```

Trace tags in Radium code are `// trace D<n>`. **`#` has no meaning for now**, so any `#` outside a string or comment is
refused until a decision gives it one.

## D88 — A folder for implementation rules (2026-10-30)

Owner: "Move implementations rule into a specific folder." **`implementation-rules/`** collects the rules for the code
that implements Radium, one file each, citing the decisions they came from: languages (D15, D16, D46), naming (D19),
comments and tracing (D18), code style (D20), C++ (D44), backend (D17, D83), proof trust (D12, D16), the oracle (D16,
D48), branches and merging (D45). As with `design-rules/` (D62), DECISIONS.md stays the record; when a rule changes,
it gets a new entry here and the rule's file is updated.

## D89 — The design session controls `main` (2026-10-30; amends D45)

Owner: "you're supposed to do the oracle yourself, and you control main." The design session commits straight to
`main`, its oracle work included, and still only when the owner says so. D45's own-branch and merge-bot flow applies
to the other sessions (compiler, `std`, …); the bot is the only thing that merges *their* branches into `main`.

## D90 — Commit style: CyborgPL's `kind.folder.narrowing: sentence` (2026-10-30)

Owner asked for a commit style before the oracle starts; design claude first proposed a flat `area: summary` style,
then, after reading CyborgPL's (its CLAUDE.md, owner 2026-09-23), recommended taking CyborgPL's instead, since it uses
`.` narrowing exactly as D26 does. Owner: "yes".

- **Subject:** `kind.folder.narrowing: sentence`. **Kinds:** `decision`, `design`, `feature`, `fix`, `test`, `house`.
  The second word is the folder (`oracle`, later `curie`, `geiger`, `std`, …), or the topic for `decision`, `design`
  and `house` (`decision.ownership`, `house.readme`, `house.brand`). The third word is optional; there is never a
  fourth.
- **Who decided** goes in brackets at the end, with the decision number in Radium: `(owner: yes; D84)`.
- **Body** (optional): wrapped lines saying what was done and why. Commits that carry out decisions in code add a
  **`Trace: D<n>, …`** line, like the `// trace D<n>` tags (D18).
- **Last line:** the session's house-name co-author line (`CLAUDE.md`).

```
decision.ownership.capitals: ownership words start with a capital; linear values are must_Give (owner; D84)
fix.oracle.bounds: an assignment outside the type's bounds was accepted in mode 1
```

## D91 — Parameters say `Lent`, `Lent.mut`, `Given` (2026-10-30)

Design claude showed two options: the call's own words in parameters (`[$text: Lend str.utf8]`), or describing words,
since a call `( )` does something and a definition `[ ]` describes (D50). Owner: "`Lent`, and `Given` at params".

```
fn first_word [$text: Lent str.utf8, …] -> str.utf8:
fn add_row [$t: Lent.mut transaction, …]:
fn close [$f: Given open_file]:

first_word(Lend $text, limit: 20);
add_row(Lend.mut $t, "hi");
close(Give $f);
```

- The call says what the caller does (`Lend`, `Lend.mut`, `Give`, `Copy`, D80, D84); the parameter says how the
  function holds it (`Lent`, `Lent.mut`, `Given`). Capitalised like the call words (D84).
- `Copy` has no parameter word: a copy is handed over as the function's own, so it goes to a `Given` parameter.
- `Lent.mut` narrows `Lent` (D26), matching `Lend.mut`.

Open: what a parameter with no ownership word means (`[$text: str.utf8]`). Integers are copied, so it matters from
structs, arrays and text on. (Settled by D92.)

## D92 — A bare parameter is lent; a written-out `Lent` is refused (2026-10-30; amends D91)

Design claude first recommended refusing a bare parameter; owner asked whether bare = `Lent` is sensible. Checked
against the defaults rule, design claude changed its recommendation; owner: "yes".

- **A parameter with no ownership word is lent**: the function can only read it, and the caller keeps it. It is the
  least a function can do with a value, so it hides no choice (D27); most parameters are only read; and it matches a
  parameter being `var.immut` by default.
- **Writing `Lent` alone is refused**, as a language default spelled out (D39): "write the bare parameter". `Lent`
  remains the word `Lent.mut` narrows from (D26).

```
fn first_word [$text: str.utf8, …] -> str.utf8:   // lent
fn add_row [$t: Lent.mut transaction, …]:
fn close [$f: Given open_file]:
```

The call still says `Lend $text` (D80, D84). Values that are always copied (integers, `bool`) take no word.

## D93 — The oracle starts now, with the settled parts (2026-10-30; replaces D48's start date)

Design claude listed what a first slice would cover and leave out; owner: "yes, go". D48 said the oracle starts after
the syntax phase; it **starts now**, in `oracle/`, with only what is settled, and grows as decisions land:

- **First slice:** blocks and `end` (D13), `;` (D29), comments (D86, D87), `var`/`var.immut`/`var.mut`/`const` and
  `$` (D30, D31), `int[min: …, max: …]` and `bool` (D14, D27, D60), `+`, `-`, `x`, `div`, `mod`, comparisons and
  `and`/`or`/`not` (D34–D40), `if` (D71), `for … in range[…]`, `while … decreases`, `forever`, `break` (D73–D77),
  `fn`/`fn.pure`, labels, defaults, `requires`, `ensures`, `old[…]` (D51–D54), `std::print`.
- **Every check site is checked as the program runs** (bounds, exact `div`, `mod`, contracts, `decreases`), as in
  mode 2 (D9). The oracle proves nothing; that is Geiger's job.
- **Anything still open is reported as "not decided"**, never guessed; anything decided but not yet in the oracle is
  "not built".
- Where the oracle had to read a decision one way to run at all, the reading is listed in `oracle/README.md` as
  provisional, for the owner to rule on.

D48's other parts stand: the design session writes it, in Rust, tests are small Radium programs tagged
`// trace D<n>`, and the compiler session never reads its source.

## D94 — Abbreviations: the settled word, or the full word unless it saves 4+ letters (2026-10-30; refines D63)

Owner: `str` could become `string` ("adds potential clarity, and `str` saves three chars anyways"), as `len` became
`length`; then asked for the refined rule. Design claude proposed it, owner: "yes". For Radium's own words:

1. **If programming has a settled word for it, use that word**, short or not (D58's keyword rule): `bool`, `const`,
   `struct`, `int`.
2. **Otherwise, write the full word, unless the short form saves 4 or more letters *and* the owner reads it
   correctly cold** (D63). Three letters isn't real length: `len` → `length` saved three and was refused (D19).

D63's allowed list is re-judged by this rule, five words per turn. First five:

| Word | Settled word? | Saves | Result |
|---|---|---|---|
| `err` | no | 2 | **`error`** |
| `newln` | no | 2 | **`newline`** (reverses D63's `newln`) |
| `bin` | no | 3 | **`binary`** (`@float.binary.32_bit`) |
| `bool` | yes | — | **`bool`** |
| `const` | yes | — | **`const`** |

Second five, owner: "yes":

| Word | Settled word? | Saves | Result |
|---|---|---|---|
| `str` | no: `string` is (C#, Go, Java, Swift, TypeScript, C++) | 3 | **`string`** (`string.utf8`) |
| `deci` | no | 3 | **`decimal`** (`@float.@decimal.32_bit`) |
| `fmt` | no: `format` is more common | 3 | **`format`** |
| `div` | yes: the word-operator for integer division (Pascal, Haskell, Oberon) | — | **`div`** |
| `mod` | yes: the word-operator for remainder (Pascal, Haskell, Ada, Fortran) | — | **`mod`** |

Earlier samples that say `str.utf8`, `deci` or `bin` mean the full words now.

Third five, owner: "yes":

| Word | Settled word? | Saves | Result |
|---|---|---|---|
| `int` | yes (C, C++, Java, C#, Go, Swift) | — | **`int`** |
| `max` | yes | — | **`max`** |
| `min` | yes | — | **`min`** |
| `mut` | no: only Rust; C++ and OCaml say `mutable` | 4, read cold (D63) | **`mut`** |
| `immut` | no | 4, read cold (D63) | **`immut`** |

Owner, on `msg`: "what is `msg` even for? A message, for what", and "`msg` isn't in the same category as those. Those
are real syntax, `msg` isn't". **`msg` is taken off the list with no verdict**: it came from design claude's starting
table (D61), and its only appearance was `$msg`, a writer's own value name, which the rule doesn't cover. `ptr`,
`char` and `config` stay on the list, as words Radium's syntax will use.

Fourth five, owner: "yes":

| Word | Settled word? | Saves | Result |
|---|---|---|---|
| `struct` | yes (D58) | — | **`struct`** |
| `var` | yes (JavaScript, C#, Go, Swift, Kotlin) | — | **`var`** |
| `char` | yes (C, C++, Java, C#, Rust) | — | **`char`** |
| `ptr` | fairly (`unique_ptr`, Rust's `std::ptr`) | 4, read cold (D63) | **`ptr`** |
| `config` | yes, read as a word of its own | — | **`config`** |

Last four, owner: "yes":

| Word | Settled word? | Saves | Result |
|---|---|---|---|
| `fn` | split (`fn`, `func`, `fun`, `function`) | 6, read cold (D63) | **`fn`** |
| `std` | yes (C++, Rust) | — | **`std`** |
| `stdout` | yes (C, Unix) | — | **`stdout`** |
| `stderr` | yes | — | **`stderr`** |

**Re-judging done.** Written in full: `binary`, `decimal`, `error`, `format`, `newline`, `string` (and D63's `buffer`,
`config` for `cfg`, `context`, `index`, `length`). Kept: `bool`, `char`, `config`, `const`, `div`, `fn`, `immut`,
`int`, `max`, `min`, `mod`, `mut`, `ptr`, `std`, `stderr`, `stdout`, `struct`, `var`. `msg` has no verdict.

## D95 — `return` gives back a function's result (2026-10-30)

Design claude asked whether a function hands back its value with `return` or another way (e.g. the last line);
owner: "yes" to `return`. A function with a result (`->`, D52) gives it back with **`return value;`**, which ends the
function there:

```
fn double [$n: int[min: 0, max: 9]] -> int[min: 0, max: 18]:
    ensures result equal $n x 2;
    return $n x 2;
end fn double

main:
    var $answer = double(4);
end main
```

The value is checked against the result type's bounds (D14), and `result` in an `ensures` is that value (D53). Such a
function can now be called inside a value (`double(4) + 1`); contracts may call only `fn.pure` functions (D51).

Open: whether every path through a function must end in `return` (settled by D96); a bare `return;` in a function
without a result; `return` in `main`; calling a function with a result and ignoring it (settled by D97).

## D96 — Every path through a function with `->` ends in `return` (2026-10-30)

Proposed by design claude, owner: "yes". A function with a result is **refused if any path can reach its `end`
without `return`**. Paths are judged by the code's shape, as in Rust and Java: the last line returns, or is an
`if` … `else` whose every branch returns, or is a `forever` that nothing breaks out of. A loop that might run zero
times, or an `if` with no `else`, doesn't count as returning.

```
fn sign [$n: int[min: -9, max: 9]] -> int[min: -1, max: 1]:
    if $n < 0:
        return -1;
    end if
end fn sign          // refused: a non-negative $n reaches here with nothing to give back
```

## D97 — A result can't be ignored (2026-10-30)

Proposed by design claude, owner: "yes". **Calling a function with a result as a statement on its own, throwing the
result away, is refused**, for every function (Rust only warns, and only for some):

```
main:
    double(4);       // refused: the result is thrown away
end main
```

Open: a way to throw a result away on purpose, so it shows in the code.

## D98 — Every function states its result (2026-10-30; amends D52)

Owner's idea ("What if just make it say `-> nothing`?"), after asking how a forgotten `->` together with a forgotten
value would be caught; owner: "yes". **Every function writes `->` and what it gives back; a function with no `->` is
refused.** D52's "a function with no result leaves it out" no longer holds. Giving nothing back is written, not
implied by a missing part (D33), so forgetting `->` is caught at the definition. `main` keeps its own form (D13).

Open: the word for "gives nothing back" (settled by D99).

## D99 — `-> void` (2026-10-30)

Design claude compared `void`, `nothing`, `none`, `unit` and `empty` (D58's three-way check) and recommended `void`,
the settled programming word for "no value"; `nothing` reads well in English but means "never returns" in Kotlin and
Scala. Owner: "yes". A function that gives nothing back is written **`-> void`**:

```
fn show [$n: int[min: 0, max: 9]] -> void:
    std::print($n, newline);
    return void;
end fn show
```

## D100 — `return void;` (2026-10-30)

Owner asked whether to add a word to a bare `return;`; design claude proposed `return void;`, owner: "yes". **A `void`
function returns with `return void;`**, mirroring its `-> void`. **A bare `return;` is refused**, so every `return`
says what it gives back, and a forgotten value is never mistaken for an empty return.

Open: whether a `void` function must end every path with `return void;` (as D96 asks of every function with `->`) or
may reach `end fn` (settled by D102); and `main` (settled by D101).

## D101 — The program starts at `fn.main` (2026-10-30; replaces D13's `main:` block)

Owner asked whether `main` should become a `fn`; design claude proposed `fn main`; owner proposed **`fn.main`**,
"because you can't call it, but you can call the other `fn`". Owner: "yes". The start of the program is a function
narrowed with `.main` (D26): a function with one more promise, checked by the compiler, that **it is where the
program starts and nothing may call it**. It is a kind, not a name, so `main` isn't reserved as a function name.

```
fn.main [] -> void:
    show(3);
    return void;
end fn.main
```

- Exactly one `fn.main` per program; it has no name of its own, and closes with `end fn.main` (D13).
- It follows the same `->` and `return` rules as every function (D96, D98, D100).
- D13's `main:` … `end main` is no longer used.

## D102 — `void` functions end every path with `return void;` (2026-10-30)

Design claude recommended one rule for every function; owner: "yes". **D96 covers `-> void` too**: every path
through every function, `fn.main` included, ends in `return`, which for `void` is `return void;` (D100). Reaching
`end fn` is never an implied return. Settles D100's open question.

## D103 — Oracle tests carry their expectations in comments (2026-10-30)

Design claude explained that the oracle drops comments while a separate test runner reads them as text, like D18's
trace tags, and recommended keeping one file per test; owner: "Ah, yes". An oracle test is one `.ra` file whose
comments say what should happen:

- `// trace D<n>` — the decisions it tests (at least one);
- `// expect: <verdict>` — exactly one: `runs`, `refused at line N`, `check fails at line N`, `not decided at line N`,
  `not built at line N`, `gives up at line N`;
- `// output: <text>` — one per printed line, in order;
- `// here` — marks the line a verdict points at, for readers.

To Radium they are ordinary comments (D87).

## D104 — What is built in, and what is imported (2026-10-30)

Owner asked whether very wide integers should be an import "like even binary floats", then whether D22's test ("same
results on every target") or a new rule was better. Design claude found that D22's test, read alone, would pull lists,
maps and decimals into the core against D70 and D23, and proposed this order; owner: "Yes".

1. **If it can be written in Radium on top of the built-in types, it's a compound**: lists, maps (D70), decimals,
   fixed-point (D23), probably text.
2. **Otherwise, if it gives identical results on every target** (emulated where the hardware lacks it, D22), **it's
   built in**: integers, `bool`, `array`, `struct`, `union.tagged`, `range`.
3. **Otherwise, it's an isotope**: floats (D22, D24).
4. **Exception: a wider version of a built-in type stays in that type.** A 256-bit integer is an `int`, with the same
   rules and Geiger's same proofs, compiled straight to LLVM's wide integers; not a library.

So **integers have no width cap** in the language: very wide ones are built in. "Rare" or "niche" is not a reason to
import something. Open: whether widths above a normal limit must be written out, and that limit (settled by D105).

## D105 — Integers wider than 64 bits write their width out (2026-10-30)

Proposed by design claude, owner: "Yes". A bare `int[…]` takes the smallest width that holds its bounds (D27), **up to
64 bits**, the widest every chip does natively. **Bounds that need more than 64 bits are refused on a bare `int`**,
with the fix: write the width.

```
var $count: int[min: 0, max: 1000] = 0;            // Curie picks 16 bits
var $huge: int[min: 0, max: 10_000_000_000_000_000_000_000_000_000_000_000_000_000] = 0;
    // refused: 10 to the 40th needs 133 bits; write the width
var $huge: int.unsigned.256_bit[min: 0, max: 10_000_000_000_000_000_000_000_000_000_000_000_000_000] = 0;   // accepted
```

Wide integers run in pieces and are slower, so they're visible where they're declared (D33, D81), never chosen
silently.

## D106 — A narrowing that lets Curie pick any width (2026-10-30; adds to D105)

Owner's idea (`int.any[]`, "so they don't have to know the exact width"); owner: "yes". Besides a bare `int[…]` (up to
64 bits) and an exact width (`int.unsigned.256_bit[…]`), **a narrowing of `int` lets Curie pick whatever width the
bounds need, beyond 64 bits**, as D27 does below it. The narrowing keeps wide integers visible (D105) without making
writers work out the bit count.

Open: the word. Design claude flagged `any`, which means "type checking off" in TypeScript and "any type" in Go and
Rust. Settled by D107.

## D107 — `int.wide[…]`, only for bounds beyond 64 bits (2026-10-30)

Cold test of `int.any`, `int.wide`, `int.auto`, `int.big`, `int.fit`; `auto` read as "pick for me", `wide` as "a wide
width only". Owner then asked why a narrowing should allow small widths when bare `int` already covers them; design
claude proposed making it wide-only, owner: "yes". **`int.wide[…]` is only for bounds that need more than 64 bits**;
Curie picks the sign and width from the bounds (D27). Bounds that fit in 64 bits are refused on `int.wide`: "write
`int[…]`". Each case has one spelling:

| Bounds need | Written |
|---|---|
| up to 64 bits | `int[min: …, max: …]` |
| more than 64 bits | `int.wide[min: …, max: …]` |
| an exact width (formats, hardware) | `int.signed.N_bit[…]` / `int.unsigned.N_bit[…]` |

## D108 — Every value states its type, or `infer` (2026-10-30; amends D14)

Owner asked whether letting Geiger work out local bounds is useful; design claude listed the costs (a hidden choice,
ranges readers can't see, confusing failures, an oracle that can't copy it). Owner proposed `var $count: infer = 0`;
owner: "yes". **Every value states its type, or writes `: infer` to ask Geiger to work out its type and bounds. A value
with neither is refused.** `infer` is allowed on **local values only**; parameters, results, fields and globals write
their types (D14's edges).

```
var $count: infer = 0;                      // Geiger works out the type and bounds
var $area: infer = $width x $height;
var $limit: int[min: 0, max: 100] = 50;     // written out
var $x = 3;                                  // refused: say the type, or `infer`
```

D14's "optional on locals, where the prover infers them" now needs `infer` written. Same idea as D98: nothing is left
out silently.

## D109 — The standard is "SSS: Radium" (2026-10-30)

Owner: "SSS: Radium is the standard's name (Soclets Software Standard)". The written standard of D55 is called
**SSS: Radium**, the **Soclets Software Standard** for Radium.

## D110 — SSS: Radium starts now, alongside the oracle (2026-10-30)

Design claude proposed, owner: "yes". The standard (D55, D109) is written **now**, in `standard/`, in plain Markdown,
and **grows with the decisions** the way the oracle does (D93): one file per area. Each rule cites the decisions it
comes from and the oracle tests that show it, so the decisions, the standard and the oracle point at each other.
Only settled decisions go in; anything open says **"not decided"**. Behaviour is only defined or target-defined (D55).

## D111 — How strongly operators bind (2026-10-30)

Proposed by design claude, owner: "yes". Without brackets, operators bind in this order, strongest first:

1. `-` before a value (`-$x`);
2. `x`, `div`, `mod` (with their narrowings), as in school maths;
3. `+`, `-`;
4. comparisons: `<`, `>`, `at_most`, `at_least`, `equal`, `not_equal`;
5. `not`;
6. `and`, `or`.

Operators of the same strength group from the left, as in school maths: `10 - 3 - 2` is `(10 - 3) - 2`. **Mixing
`and` and `or` without brackets is refused**: write `$a or ($b and $c)`. Chained comparisons (`1 < $x < 5`) remain
open.

---

## Open


- What "more diabolical" covers beyond D6–D12 (e.g. timing and side channels).
- **Syntax phase** (owner: spellings get designed together, later). Collected so far: the contract keywords
  (`requires`, `ensures`, `invariant`, `assert`, `decreases`, ghost code, lemmas — D4); the ownership words (D5; done: D80, D84);
  `shield` and `reason` (D6); `forever` (D7); failure sets, failure contracts, handlers (D11); integer and float ranges
  (D14, D24); the two import kinds (D23); the inexact-literal mark and its directions (D25); where contracts sit
  relative to the block's `:` (D13); the evaluate-both narrowing of `and`/`or` (D40); the own-function mark
  (`my.`, D43); the deadline mark (D82); comments (done: D86, D87); a use for `#`, if any (D87).
