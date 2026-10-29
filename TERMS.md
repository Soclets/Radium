# Radium — terms

One name per concept, the same in the design docs, the implementation, comments and error messages (D19). When a
concept is renamed, its old name moves to the "Not used" column so the D18 checker can flag it.

## Concepts

| Term | In code | Meaning | Decision | Not used |
|---|---|---|---|---|
| check site | `CheckSite` | A place that could go wrong: overflow, out-of-range, bounds, division by zero, narrowing | D2 | |
| proven | `Proven` | A check site the prover has shown can never fail | D2, D12 | |
| unproven | `Unproven` | A check site the prover could not prove either way | D2 | |
| counterexample | `cex` allowed | Concrete inputs that make a check site fail | D2 | |
| contract | `Contract` | A `requires`, `ensures` or failure contract | D3, D11 | |
| precondition | `Precondition` | What callers must guarantee (`requires`) | D4 | |
| postcondition | `Postcondition` | What a function promises back (`ensures`) | D4 | |
| loop invariant | `LoopInvariant` | A fact true on every trip of a loop | D4 | |
| ghost code | `Ghost` | Code and values that exist only for proofs; erased from the binary | D4 | |
| lemma | `Lemma` | A proven helper fact that can be invoked | D4 | |
| termination measure | `TerminationMeasure` | A value that shrinks every trip or recursive call (`decreases`) | D4, D7 | |
| shield | `Shield` | The only place unproven or dangerous code may live | D6 | `no_check` |
| safety radius | `Radius` | A shield's size: how much unproven code it holds | D6 | |
| entry wall | `EntryWall` | A shield's preconditions, proven at every caller | D6 | |
| exit wall | `ExitWall` | A shield's postconditions, checked at the door at run time | D6 | |
| containment wall | `ContainmentWall` | The rule that raw pointers and unchecked borrows can't leave a shield | D6 | |
| forever loop | `ForeverLoop` | The only loop allowed to run without end; each trip must end | D7 | |
| heap budget | `HeapBudget` | The heap size a target declares | D8 | |
| build mode | `BuildMode` | Mode 1, mode 2 or final | D9 | |
| final build | `BuildMode::Final` | Mode 3: everything proven, no runtime checks | D9 | |
| failure | `Failure` | A declared, handled way a function can end early | D11 | exception, panic |
| failure set | `FailureSet` | The exact failures a signature lists | D11 | |
| failure contract | `FailureContract` | When a failure happens (`fails x exactly when …`) | D11 | |
| handler collection | `HandlerCollection` | A named, reusable group of handler arms | D11 | |
| proof certificate | `ProofCertificate` | A solver's step-by-step proof, re-checked by the proof checker | D12 | |
| proof checker | `ProofChecker` | The small, Verus-verified program that re-checks proof certificates | D12, D16 | |
| isotope | `Isotope` | A compiler extension: trusted, not proven | D23 | plugin, limb |
| compound | `Compound` | A library written in Radium: proven like any other code | D23 | kit, package |
| range | `Range` | The `min`/`max` refinement on a fixed-width integer | D14 | |
| narrowing | — | A `.` that makes the thing before it more specific (`int.signed`, `div.floor`) | D26, D35 | |
| own-function mark | — | The mark on every use of the writer's own function (`my.x`) | D43 | |
| section marker | — | `// section a.b:` … `// end section a.b` | D18 | |
| trace tag | — | `// trace D<n>` | D18 | |

## People

Same words as CyborgPL (D21).

| Term | Meaning |
|---|---|
| owner | The one person who decides Radium's design |
| author | The founder |
| authors | The main contributors: the owner and the Claude sessions |
| contributors | Outside people who contribute |
| writer | Whoever writes a Radium program |
| user | Whoever uses a writer's final product |

## Allowed abbreviations

Only these may be used in place of the full word (D19). An abbreviation must be well known and save real length.

| Abbreviation | Full form |
|---|---|
| `id` | identifier |
| `ir` | intermediate representation |
| `ast` | abstract syntax tree |
| `smt` | satisfiability modulo theories |
| `cex` | counterexample (owner, 2026-10-29) |
| `llvm` | LLVM (a name) |

Write out everything else, e.g. `length` not `len`, `context` not `ctx`.
