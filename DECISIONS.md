# Radium — design decisions

The owner decides; radium design claude proposes and records. Each entry says what was chosen and whose idea it was.
Numbers are permanent: implementation code points at them with `trace D<n>` tags (D18), so a decision is
never renumbered. A changed decision gets a new entry that says which one it replaces.

Identity (repo description, `Soclets/Radium`): *"Radium: slowly kills you via the steep learning curve, and slow
compile times. Glow in the dark via the clear syntax. It also has a safety radius."* A steep learning curve and slow
compiles are accepted, never an argument against an option. Clear syntax is required.

License: Apache 2.0.

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
as `radium inspect`) and print one warning line to stderr every time they start, e.g.
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

## D13 — Block delimiters (2026-10-29)

Owner's own design.

- A block opens with a line ending in `:` and closes with `end …`.
- Named blocks state their kind and name at both ends: `fn read_celsius() -> …:` … `end fn read_celsius`;
  `shield sensor_driver:` … `end shield sensor_driver`.
- Unnamed blocks state their kind: `if i > 5:` … `end if`; `for …:` … `end for`.
- `main` is special: `main:` … `end main`, no kind word. (Spelled `MAIN` at first; owner's sketch, 2026-10-29,
  settled it as lowercase `main`.)
- The compiler checks that every `end` matches its opener exactly.

Contracts sit between the signature and the `:` (provisional).

## D14 — Integers (2026-10-29; width amended by D27)

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

## D28 — `%` marks what an isotope provides (2026-10-29)

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

## D36 — `newline` (2026-10-30)

Owner: `newln` becomes `newline` (by the owner's `len` rule, D19: two letters saved isn't worth decoding).

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
  the default later.
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

## D43 — Your own functions are marked where used (2026-10-30)

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
it comes from (`my.` own, `std::` the standard compound, `%` an isotope). Quoted names (D32) remain for odd
characters and C names. Owner: if `my.` gets too heavy, it could become one symbol, or be needed only for names that
clash; open. Whether types follow the same rule is open.

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

## D45 — Branches and a local merge bot (2026-10-30)

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

---

## Open

- Whether the writer's own types carry the D43 mark like functions (asked, not answered yet).

- What "more diabolical" covers beyond D6–D12 (e.g. timing and side channels).
- **Syntax phase** (owner: spellings get designed together, later). Collected so far: the contract keywords
  (`requires`, `ensures`, `invariant`, `assert`, `decreases`, ghost code, lemmas — D4); the ownership words (D5);
  `shield` and `reason` (D6); `forever` (D7); failure sets, failure contracts, handlers (D11); integer and float ranges
  (D14, D24); the two import kinds (D23); the inexact-literal mark and its directions (D25); where contracts sit
  relative to the block's `:` (D13); the evaluate-both narrowing of `and`/`or` (D40); the own-function mark
  (`my.`, D43).
