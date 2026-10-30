# Radium — terms

One name per concept, the same in the design docs, the implementation, comments and error messages (D19). When a
concept is renamed, its old name moves to the "Not used" column so the D18 checker can flag it.

## Concepts

| Term | In code | Meaning | Decision | Not used |
|---|---|---|---|---|
| SSS: Radium | — | The written standard (D55): the Soclets Software Standard for Radium | D109 | |
| Geiger | `geiger` | Radium's prover | D67 | Precog (CyborgPL's), Radinum |
| Curie | `curie` | Radium's compiler, and its command | D68 | `radium` (as a command), KAYO (CyborgPL's) |
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
| deadline | `Deadline` | The longest a marked function may ever take; Geiger proves the worst case fits | D82 | time limit, WCET (as a writer-facing word) |
| timing model | `TimingModel` | What a target declares about its chip's timing, needed to prove deadlines | D82 | |
| translation validation | `TranslationValidation` | Proving an optimized function does exactly what the unoptimized one did, for this program | D17, D83 | |
| fallback | `Fallback` | Using the conservative pipeline's output for a function whose optimized version couldn't be validated | D83 | |
| build mode | `BuildMode` | Mode 1, mode 2 or final | D9 | |
| final build | `BuildMode::Final` | Mode 3: everything proven, no runtime checks | D9 | |
| failure | `Failure` | A declared, handled way a function can end early | D11 | exception, panic |
| failure set | `FailureSet` | The exact failures a signature lists | D11 | |
| failure contract | `FailureContract` | When a failure happens (`fails x exactly when …`) | D11 | |
| handler narrowings | `HandlerNarrowings` | A named, reusable group of handler arms (`narrowings.handler`) | D11, D57 | handler collection |
| preset | `Preset` | A narrowing that only fixes some arguments, one line ending in `;` (`heat.popcorn [$power = 800];`) | D57 | |
| fixed defaults | `FixedDefaults` | `defaults.fixed`: defaults promised never to change; spelling them out is refused | D57 | |
| function narrowings | `FunctionNarrowings` | A name and its narrowed functions (`narrowings.fn`) | D57 | function collection, `collection.fn`, `coll.fn`, member |
| proof certificate | `ProofCertificate` | A solver's step-by-step proof, re-checked by the proof checker | D12 | |
| proof checker | `ProofChecker` | The small, Verus-verified program that re-checks proof certificates | D12, D16 | |
| isotope | `Isotope` | A compiler extension: trusted, not proven | D23 | plugin, limb |
| compound | `Compound` | A library written in Radium: proven like any other code | D23 | kit, package |
| struct | `Struct` | Named fields stored together as one value | D58 | record |
| tagged union | `TaggedUnion` | `union.tagged`: a value that is one of several named cases, which may carry data | D59 | enum, sum type, variant |
| union | `RawUnion` | Bare `union`: C's untagged union, allowed only inside shields | D59 | |
| interface | `interface` | A list of operations a type must have; no code; used in generic bounds | D69 | sig, signature, trait, protocol, concept, typeclass |
| law | `law` | A rule an interface's operations must obey, proven by Geiger at each `satisfies` | D69 | |
| type slot | — | A type an interface leaves for each joining type to fill in (`type item;`) | D69 | associated type |
| type set | `type_set` | A closed list of types; used in generic bounds | D69 | set (CyborgPL's), union type |
| array | `array` | Built-in fixed-length sequence, stored inside its holder, no heap | D70 | |
| list | `std::list` | Growable sequence in `std`, on the heap, within a length range | D64, D70 | vector, `Vec` |
| map | `std::map.sorted` / `.inserted` / `.unordered` | Key–value lookup in `std`; the kind names its order | D66, D70 | dict, hash map |
| position | `pos:` | An index counting from 1: `pos: 1` is the first item | D64 | index |
| offset | `offset:` | An index counting from 0: `offset: 0` is the start | D64 | index |
| program start | `fn.main` | The function where the program starts; nothing may call it | D101 | `main:` … `end main`, `fn main` |
| void | `-> void` | What a function that gives nothing back writes as its result (D98) | D99 | `nothing`, `none`, `unit`, leaving out `->` |
| give | `Give` | Hand over ownership; the giver can't use the value afterwards | D80, D84 | move, `give` (lowercase) |
| copy | `Copy` | Hand over a separate copy; the original is untouched | D80, D84 | clone, `copy` (lowercase) |
| lend | `Lend` | Temporary read-only access; the owner keeps the value | D80, D84 | borrow, `&`, `lend` (lowercase) |
| lend.mut | `Lend.mut` | Temporary access that may change the value; the owner keeps it | D80, D84 | mutable borrow, `&mut`, `lend.mut` (lowercase) |
| lent (parameter) | bare (`Lent` is refused) | A parameter the function only reads; the caller keeps the value (call side: `Lend`). The default for a parameter | D91, D92 | `Lend` in parameters, `Lent` written alone |
| lent.mut (parameter) | `Lent.mut` | A parameter the function may change; the caller keeps the value (call side: `Lend.mut`) | D91 | `Lend.mut` in parameters |
| given (parameter) | `Given` | A parameter the function owns from then on (call side: `Give` or `Copy`) | D91 | `Give` in parameters, owned |
| linear value | `must_Give` | A value that must be handed off with `Give` exactly once before it goes out of scope | D5, D84 | `linear`, `must_use`, `once`, `must_finish` |
| break | `break` | Stops the loop it is in | D77 | `stop`, `exit`, `leave`, `last` |
| — | — | (No skip-this-trip statement: write the condition around the work) | D77 | `continue`, `skip`, `next` (as a statement) |
| range | `range[from:, to:, before:, step_by:]` | A sequence of numbers a loop goes through (`for $i in range[…]`): `to:` includes the end, `before:` stops before it, `step_by:` is the step (default 1, negative counts down) | D73, D74 | `until:`, `by:`, `step:`, `steps:`, `every:`; "loop bounds" |
| infer | `: infer` | Asks Geiger to work out a local value's type and bounds; locals only | D108 | leaving the type out |
| wide integer | `int.wide[…]` | An integer whose bounds need more than 64 bits; Curie picks its width | D105, D107 | `int.any`, `int.auto`, `int.big` |
| bounds | `min:`, `max:` | A type's lower and upper limits (`int[min: 0, max: 100]`) | D14, D74 | range (for limits) |
| requirement | — | What a generic placeholder needs (`[?T: ordered]`: `?T` requires `ordered`) | D69, D74 | bound, trait bound |
| narrowing | — | A `.` that makes the thing before it more specific (`int.signed`, `div.floor`) | D26, D35 | |
| own-name mark | — | `my.`, needed only when the writer's function or type name clashes with a built-in (`my.reading`) | D43 | own-function mark |
| newline | `newline` | The line break `std::print` writes when given it | D94 | `newln` |
| comment | `// ` | Text for readers, from `//` and a space to the end of the line | D85, D87 | `#` (as a comment mark) |
| comment block | `comment:` … `end comment` | A comment that spans lines; blank lines inside belong to it | D86 | `#[ … ]`, `/* */` |
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
