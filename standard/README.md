# SSS: Radium

The **Soclets Software Standard** for Radium (D55, D109): what every Radium construct does, in plain words.

- **Why** each rule exists is in [`DECISIONS.md`](../DECISIONS.md); each rule here cites its decisions as (D*n*).
- **What runs** is the [oracle](../oracle/); a rule names the oracle test programs that show it, as *tests:*.
- This standard grows with the decisions (D110). Anything still open says **not decided**.

## Kinds of behaviour

Radium has only two (D55):

1. **Defined**: exactly one behaviour, the same on every target.
2. **Target-defined**: only what the declared target describes, such as stack size, heap budget and pointer size.
   Written in the target's description, never chosen silently by the compiler.

There is no unspecified behaviour and no undefined behaviour. Anything that could go wrong is a **check site** that
Geiger must prove can't fail (D2). The one exception is the inside of a shield (D6): the standard specifies its walls,
and the inside is its writer's responsibility.

## Words used here

- **Refused**: the compiler, Curie, won't build the program.
- **Check site**: a place that could go wrong. In the final build each one is proven; in modes 1 and 2 it is also
  checked as the program runs (D9).
- **A failed check** (modes 1 and 2 only) stops the program at once, reports on stderr in the message layout below,
  and exits with a failure status (target-defined). It can't be handled; nothing runs after it (D114, D123).

## Messages

Every refusal and failed check is reported in one layout (D116–D119, D122), described in
[`design-rules/error-messages.md`](../design-rules/error-messages.md):

1. the kind (`refused:`, `unproven:` or `check failed:`) and the place twice: `file:line:column` for tools, counted in
   UTF-8 bytes, then `line N column M` for people, counted in grapheme clusters;
2. the line itself, with a `^` pointer under the exact part;
3. `what:` (or `fix:` when there is exactly one fix), `why:` with the rule's decision number, and `when:` with the
   values that break it;
4. `because:` links to earlier steps that led to it, in the order they happened.

A message says only what's certain: no suggestions when there is more than one possible fix.
- **Writer**: whoever writes a Radium program (D21).

## Contents

1. [Programs and blocks](1-programs-and-blocks.md)
2. [Values](2-values.md)
3. [Integers](3-integers.md)
4. [Operators](4-operators.md)
5. [Conditions](5-conditions.md)
6. [Loops](6-loops.md)
7. [Functions](7-functions.md)
8. [Ownership](8-ownership.md)
9. [Not written yet](9-not-written-yet.md)
