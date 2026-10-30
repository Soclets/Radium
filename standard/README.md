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
