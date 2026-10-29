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
- `MAIN` is special: `MAIN:` … `end MAIN`, no kind word.
- The compiler checks that every `end` matches its opener exactly.

Contracts sit between the signature and the `:` (provisional).

## D14 — Integers (2026-10-29)

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

---

## Open

- What "more diabolical" covers beyond D6–D12 (e.g. timing and side channels).
- Whether "owner" and "writer" (borrowed from CyborgPL in `TERMS.md`) are Radium's terms too.
- Every keyword and spelling marked placeholder above.
