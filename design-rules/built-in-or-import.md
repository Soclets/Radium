# Built in or imported

From D22, D23 and D104. Decides where a new type lives.

1. **If it can be written in Radium on top of the built-in types, it's a compound** (a library in Radium, proven like
   any code): lists, maps, decimals, fixed-point, probably text.
2. **Otherwise, if it gives identical results on every target** (emulated where the hardware lacks it), **it's built
   in**: integers, `bool`, `array`, `struct`, `union.tagged`, `range`.
3. **Otherwise, it's an isotope** (a trusted compiler extension, marked `@`): floats.
4. **A wider version of a built-in type stays in that type**: a 256-bit integer is still an `int`.

How common or rare a type is doesn't decide where it lives.
